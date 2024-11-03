use std::fs;
use futures::StreamExt;
use web3::{
    contract::Contract,
    ethabi::{ Event, Log, RawLog},
    transports::WebSocket,
    types::{ BlockId, BlockNumber, H160, H256, U256, U64 },
    Web3,
};
use anyhow::{Result};
use tracing::{info, error, instrument};
use chrono::{DateTime, TimeZone, Utc};
use crate::io::{append_to_csv, append_vwap_to_csv};
use crate::types::StandardizedTrade;
use crate::config::AppConfig;
use crate::processors::vwap::VWAPCalculator;
use tokio::sync::mpsc;
// I do not get how there imports work sometimes!
use crate::PriceUpdate;
use crate::processors::cleaning::DataCleaner;


#[derive(PartialEq, Debug, Clone)]
pub struct Block {
    pub number: U64,
    pub hash: H256,
    pub timestamp: DateTime<Utc>,
    pub parsed_logs: Vec<ParsedLog>,
}

#[derive(PartialEq, Clone, Debug)]
pub struct ParsedLog {
    pub timestamp: DateTime<Utc>,
    pub sender: String,
    pub recipient: String,
    pub side: String,
    pub amount0: f64,
    pub amount1: f64,
    pub sqrt_price_x96: U256,
    pub price: f64,
    pub price_inverted: f64,
    pub liquidity: U256,
    pub tick: U256,
}

pub struct UniswapHandler {
    web3: Web3<WebSocket>,
    contract_address: H160,
    swap_event: Event,
    config: AppConfig,
}

impl UniswapHandler {
    pub async fn new(config: &AppConfig) -> Result<Self> {
        info!("Using websocket URL: {}", config.data_sources.uniswap.websocket_url);
        let web3 = Web3::new(WebSocket::new(&config.data_sources.uniswap.websocket_url).await?);

        let contract_address = H160::from_slice(
            &hex::decode(&config.data_sources.uniswap.pool_address).unwrap()[..]
        );

        let abi_contents = fs::read(&config.data_sources.uniswap.abi_path)?;

        let contract = Contract::from_json(
            web3.eth(),
            contract_address,
            abi_contents.as_slice()
        )?;

        let swap_event = contract.abi().events_by_name("Swap")?.first().unwrap().clone();

        Ok(Self {
            web3,
            contract_address,
            swap_event,
            config: config.clone(),
        })
    }

    pub async fn start_listening_with_sender(&self, price_tx: mpsc::Sender<PriceUpdate>) -> Result<()> {
        info!("Starting Uniswap handler with price updates");
        let mut calculator = VWAPCalculator::new("uniswap");
        let mut cleaner = DataCleaner::new(
            self.config.cleaning.mean_window,
            self.config.cleaning.std_dev_threshold,
            self.config.cleaning.min_volume,
            self.config.cleaning.max_volume,
        );
        let mut block_stream = self.web3.eth_subscribe().subscribe_new_heads().await?;
        let swap_event_signature = self.swap_event.signature();

        while let Some(Ok(block)) = block_stream.next().await {
            let current_block_num = block.number.expect("Error getting block number");
            match self.process_block(current_block_num, swap_event_signature).await {
                Ok((block_timestamp, trades)) => {
                    if !trades.is_empty() {
                        // Convert to standardized trades
                        let standardized_trades: Vec<StandardizedTrade> = trades.iter()
                            .map(|log| StandardizedTrade {
                                timestamp: block_timestamp, // Use block timestamp instead of now
                                source: "uniswap_v3_0005".to_string(),
                                qty: log.amount1,
                                price: log.price_inverted,
                            })
                            .filter(|trade| cleaner.clean_trade(trade))
                            .collect();

                        // Write raw trades to CSV
                        // Probably better to switch this off if we do not want to use for validation or use sql
                        if let Err(e) = append_to_csv(&standardized_trades, "data/uniswap_trades.csv") {
                            error!("Failed to write trades to CSV: {}", e);
                            continue;
                        }

                        // Process VWAP
                        if let Some(vwap_data) = calculator.process_trades(&standardized_trades) {
                            // Write VWAP to CSV
                            if let Err(e) = append_vwap_to_csv(&vwap_data, "data/uniswap_vwap.csv") {
                                error!("Failed to write VWAP to CSV: {}", e);
                                continue;
                            }

                            // Send to volatility processor
                            if let Err(e) = price_tx.send(PriceUpdate::Uniswap(
                                vwap_data.start_time,
                                vwap_data.vwap
                            )).await {
                                error!("Failed to send Uniswap price update: {}", e);
                            } else {
                                info!(
                                    timestamp = %vwap_data.start_time,
                                    vwap = vwap_data.vwap,
                                    "Sent Uniswap VWAP update"
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    error!(
                        block_number = %current_block_num,
                        error = %e,
                        "Failed to process block"
                    );
                }
            }
        }

        Ok(())
    }

    #[instrument(skip(self))]
    async fn process_block(&self, block_number: U64, swap_event_signature: H256)
                           -> Result<(DateTime<Utc>, Vec<ParsedLog>)> {
        let block = self.web3
            .eth()
            .block(BlockId::Number(BlockNumber::Number(block_number)))
            .await?
            .unwrap();


        let swap_logs = self.web3
            .eth()
            .logs(
                web3::types::FilterBuilder::default()
                    .block_hash(block.hash.unwrap())
                    .address(vec![self.contract_address])
                    .topics(Some(vec![swap_event_signature]), None, None, None)
                    .build()
            )
            .await?;

        let block_timestamp = Utc.timestamp_opt(
            block.timestamp.as_u64() as i64,
            0
        ).single().ok_or_else(|| anyhow::anyhow!("Invalid timestamp"))?;

        let mut parsed_logs = Vec::new();
        for log in swap_logs {
            let parsed = self.swap_event
                .parse_log(RawLog {
                    topics: log.topics,
                    data: log.data.0,
                })?;

            let log = self.parse_log(parsed);
            parsed_logs.push(log);
        }

        Ok((block_timestamp, parsed_logs))
    }

    fn parse_log(&self, log: Log) -> ParsedLog {
        let sender = address_to_string(log.params[0].value.clone().into_address().unwrap());
        let recipient = address_to_string(log.params[1].value.clone().into_address().unwrap());
        let amount0 = log.params[2].value.clone().into_int().unwrap();
        let amount1 = log.params[3].value.clone().into_int().unwrap();

        let sqrt_price_x96 = log.params[4].value.clone().into_uint().unwrap();
        let price_unadjusted = calculate_price_from_sqrt_x96(sqrt_price_x96);
        let price = adjust_price_by_decimals(self.config.data_sources.uniswap.decimal_token0,self.config.data_sources.uniswap.decimal_token1,price_unadjusted);
        let price_inverted = 1.0 / price;
        let liquidity = log.params[5].value.clone().into_uint().unwrap();
        let tick = log.params[6].value.clone().into_int().unwrap();

        let is_amount0_negative = u256_is_negative(amount0);
        let is_amount1_negative = u256_is_negative(amount1);

        assert!(is_amount0_negative ^ is_amount1_negative);

        let side = if is_amount1_negative {
            "sell".to_string()
        } else {
            "buy".to_string()
        };

        let amount0 = u256_to_string(amount0, 6).parse::<f64>()
            .unwrap_or(0.0);
        let amount1 = u256_to_string(amount1, 18).parse::<f64>()
            .unwrap_or(0.0);

        ParsedLog {
            timestamp: Utc::now(),
            sender,
            recipient,
            side,
            amount0,
            amount1,
            sqrt_price_x96,
            price,
            price_inverted,
            liquidity,
            tick,
        }
    }
}

// Helper functions

fn address_to_string(address: H160) -> String {
    let mut a = String::from("0x");
    a.push_str(hex::encode(&address).as_str());
    a
}
fn u256_is_negative(amount: U256) -> bool {
    amount.bit(255)
}

fn u256_to_string(amount: U256, decimals: usize) -> String {
    let mut amount = amount;

    if u256_is_negative(amount) {
        let mut bytes = [0u8; 32];
        amount.to_big_endian(&mut bytes);

        for b in bytes.iter_mut() {
            *b = !*b;
        }

        amount = U256::from_big_endian(&bytes);
        amount += U256::one();
    }

    let decimal_string = amount.to_string();

    let integer: String = match decimal_string.clone().len() > decimals {
        true => decimal_string[..decimal_string.len() - decimals].to_string(),
        false => "0".to_string(),
    };

    let decimals: String = match decimal_string.len() > decimals {
        true => if decimals > 0 {
            decimal_string[decimal_string.len() - decimals..].to_string()
        } else {
            "0".to_string()
        }
        false => {
            format!("{}{}", "0".repeat(decimals - decimal_string.len()), &decimal_string[..])
        }
    };

    format!("{}.{}", integer, decimals)
}

fn calculate_price_from_sqrt_x96(sqrt_price_x96: U256) -> f64 {
    let q96 = 2f64.powi(96);
    let sqrt_price = sqrt_price_x96.to_string().parse::<f64>().unwrap();
    (sqrt_price/ q96).powi(2)
}

fn adjust_price_by_decimals(decimals0: i8, decimals1: i8, unadjusted_price: f64) -> f64 {
    unadjusted_price * 10.0_f64.powi((decimals0 - decimals1) as i32)
}
