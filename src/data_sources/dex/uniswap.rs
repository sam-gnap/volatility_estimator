// src/data_sources/dex/uniswap.rs
use futures::StreamExt;
use tokio::sync::mpsc;
use web3::{
    Web3,
    transports::WebSocket,
    types::{BlockId, BlockNumber, H160, U256},
};
use chrono::{TimeZone, Utc};
use tracing::{info, error};

use crate::types::PriceUpdate;
use crate::error::{Error, Result};
use crate::config::UniswapConfig;

pub async fn run_uniswap_with_sender(
    price_tx: mpsc::Sender<PriceUpdate>,
    config: UniswapConfig,
) -> Result<()> {
    // Connect to web3
    let web3 = Web3::new(WebSocket::new(&config.websocket_url).await?);
    let contract_address = H160::from_slice(
        &hex::decode(&config.pool_address).unwrap()[..]
    );

    let contract = web3::contract::Contract::from_json(
        web3.eth(),
        contract_address,
        &tokio::fs::read(&config.abi_path).as_slice()
    )?;

    let swap_signature = contract.abi()
        .events_by_name("Swap")?
        .first()
        .ok_or_else(|| Error::data_source("Swap event not found"))?
        .signature();

    let mut block_stream = web3.eth_subscribe().subscribe_new_heads().await?;
    info!(pool_address = %config.pool_address, "Started Uniswap price source");

    while let Some(Ok(block)) = block_stream.next().await {
        let block_number = block.number
            .ok_or_else(|| Error::data_source("No block number"))?;

        let block = web3.eth().block(BlockId::Number(BlockNumber::Number(block_number)))
            .await?
            .ok_or_else(|| Error::data_source("Block not found"))?;

        let timestamp = Utc.timestamp_opt(block.timestamp.as_u64() as i64, 0)
            .single()
            .ok_or_else(|| Error::data_source("Invalid timestamp"))?;

        let logs = web3.eth().logs(
            web3::types::FilterBuilder::default()
                .block_hash(block.hash.unwrap())
                .address(vec![contract_address])
                .topics(Some(vec![swap_signature]), None, None, None)
                .build()
        ).await?;

        for log in logs {
            let parsed = contract.abi()
                .events_by_name("Swap")?
                .first()
                .unwrap()
                .parse_log(web3::ethabi::RawLog {
                    topics: log.topics,
                    data: log.data.0,
                })?;

            let sqrt_price_x96 = parsed.params[4].value.clone().into_uint().unwrap();
            let price = calculate_price(sqrt_price_x96);

            if let Err(e) = price_tx.send(PriceUpdate::Uniswap(
                timestamp,
                price,
            )).await {
                error!("Failed to send Uniswap price update: {}", e);
            }
        }
    }

    Ok(())
}

fn calculate_price(sqrt_price_x96: U256) -> f64 {
    let q96 = 2f64.powi(96);
    let sqrt_price = sqrt_price_x96.as_u128() as f64;
    (sqrt_price / q96).powi(2)
}