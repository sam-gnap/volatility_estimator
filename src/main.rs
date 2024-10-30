use std::{ collections::VecDeque, fmt };

use futures::StreamExt;
use web3::{
    contract::Contract,
    ethabi::{ Event, Log, RawLog, Token, ParamType },
    transports::WebSocket,
    types::{ BlockId, BlockNumber, H160, H256, U256, U64 },
    Web3,
};
use dotenv::dotenv;

#[cfg(debug_assertions)]
fn debug_setup() {
    println!("Debug mode enabled");
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    dotenv().ok();
    let websocket_infura_endpoint: String = std::env::var("INFURA_WEBSOCKET")?;

    // we create a web socket for listening to new blocks
    let web3 = web3::Web3::new(
        web3::transports::ws::WebSocket::new(&websocket_infura_endpoint).await?
    );

    // we create a contract structures that enables us to interact with the Uniswap contract in the
    // specified address
    let contract_address = H160::from_slice(
        &hex::decode("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640").unwrap()[..]
    );
    let contract = Contract::from_json(
        web3.eth(),
        contract_address,
        include_bytes!("contracts/uniswap_abi.json")
    )?;

    // we specify that the uniswap event that we are going to listen to is the "swap"
    let swap_event = contract.abi().events_by_name("Swap")?.first().unwrap();
    let swap_event_signature = swap_event.signature();

    // we subscribe to new blocks
    let mut block_stream = web3.eth_subscribe().subscribe_new_heads().await?;
    let mut queue = VecDeque::<Block>::new();

    // every time a new block is added to the blockchain our endpoint will alert us
    while let Some(Ok(block)) = block_stream.next().await {
        let current_block_num = block.number.expect("Error getting the current block number");
        let block_numbers = vec![current_block_num];
        queue = fetch_block_queue(
            block_numbers,
            web3.clone(),
            contract_address,
            swap_event_signature,
            swap_event.clone()
        ).await;

        assert_eq!(queue.len(), 1, "queue should have length 1 at this point.");

        let block = queue.pop_front().expect("fail in popping element from the queue");
        println!("block: {}", block.number);
        if block.parsed_logs.len() > 0 {
            println!("{:#?}", block.parsed_logs);
        }

        assert_eq!(queue.len(), 0, "queue should have length 0 at this point.");
    }

    Ok(())
}

#[derive(PartialEq, Debug, Clone)]
pub struct Block {
    pub number: U64,
    pub hash: H256,
    pub parsed_logs: Vec<ParsedLog>,
}

/// This function returns a queue of Blocks
/// block_numbers: an array indicating which blocks should be fetched.
pub async fn fetch_block_queue(
    block_numbers: Vec<U64>,
    web3: Web3<WebSocket>,
    contract_address: H160,
    swap_event_signature: H256,
    swap_event: Event
) -> VecDeque<Block> {
    let mut queue = VecDeque::<Block>::new();

    for block_i in block_numbers {
        let block = web3
            .eth()
            .block(BlockId::Number(BlockNumber::Number(block_i))).await
            .unwrap()
            .unwrap();

        let swap_logs_in_block = web3
            .eth()
            .logs(
                web3::types::FilterBuilder
                    ::default()
                    .block_hash(block.hash.unwrap())
                    .address(vec![contract_address])
                    .topics(Some(vec![swap_event_signature]), None, None, None)
                    .build()
            ).await
            .unwrap();

        let mut parsed_logs = vec![];
        for log in swap_logs_in_block {
            let log = swap_event
                .parse_log(RawLog { topics: log.topics, data: log.data.0 })
                .unwrap();

            parsed_logs.push(parse_log(log));
        }

        // assert_eq!(
        //     block_i,
        //     block.number.expect("could not get block number"),
        //     "{block_i} should equal {number} field of block fetched"
        // );

        let hash = block.hash.expect("could not get block number");
        let number = block_i;

        queue.push_back(Block { hash, number, parsed_logs });
    }
    queue
}

#[derive(PartialEq, Clone)]
pub struct ParsedLog {
    pub sender: String,
    pub recipient: String,
    pub direction: String,
    pub amount0: String,
    pub amount1: String,
    pub sqrt_price_x96: U256,
    pub price: f64,
    pub price_inverted: f64,
    pub liquidity: U256,
    pub tick: U256,
}

impl fmt::Debug for ParsedLog {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Parsed Log: {{\n")?;
        write!(f, " sender: {}\n", self.sender)?;
        write!(f, " recipient: {}\n", self.recipient)?;
        write!(f, " direction: {}\n", self.direction)?;
        write!(f, " amount0: {:}\n", self.amount0)?;
        write!(f, " amount1: {:}\n", self.amount1)?;
        write!(f, " sqrt_price_x96: {:}\n", self.sqrt_price_x96)?;
        write!(f, " price: {:}\n", self.price)?;
        write!(f, " price_inverted: {:}\n", self.price_inverted)?;
        write!(f, " liquidity: {:}\n", self.liquidity)?;
        write!(f, " tick: {:}\n", self.tick)?;
        write!(f, "}}")
    }
}

fn address_to_string(address: H160) -> String {
    let mut a = String::from("0x");
    a.push_str(hex::encode(&address).as_str());
    a
}

pub fn parse_log(log: Log) -> ParsedLog {
    let DECIMAL_0 = 6;
    let DECIMAL_1 = 18;
    let sender = address_to_string(log.params[0].value.clone().into_address().unwrap());
    let recipient = address_to_string(log.params[1].value.clone().into_address().unwrap());
    // the amounts are in base units
    let amount0 = log.params[2].value.clone().into_int().unwrap();
    let amount1 = log.params[3].value.clone().into_int().unwrap();

    let sqrt_price_x96 = log.params[4].value.clone().into_uint().unwrap();
    let price_unadjusted = calculate_price_from_sqrt_x96(sqrt_price_x96);
    let price = adjust_price_by_decimals(DECIMAL_0, DECIMAL_1, price_unadjusted);
    let price_inverted = 1.0/price;
    // I suppose this is net, not sure how it captures if it overflows to another tick
    let liquidity = log.params[5].value.clone().into_uint().unwrap();
    let tick = log.params[6].value.clone().into_int().unwrap();

    // check the sign so we know what is the sale (true = negative, false = positive)

    let is_amount0_negative = u256_is_negative(amount0);
    let is_amount1_negative = u256_is_negative(amount1);

    assert!(is_amount0_negative ^ is_amount1_negative);

    // the negative one is the swap's output
    let direction = if is_amount1_negative {
        "ETH -> USDC".to_string()
    } else {
        "USDC -> ETH".to_string()
    };

    // format the amount according to the decimals of each token
    let amount0 = u256_to_string(amount0, 6);
    let amount1 = u256_to_string(amount1, 18);

    ParsedLog { sender, recipient, direction, amount0, amount1, sqrt_price_x96, price, price_inverted, liquidity, tick}
}

pub fn u256_is_negative(amount: U256) -> bool {
    amount.bit(255)
}

pub fn u256_to_string(amount: U256, decimals: usize) -> String {
    let mut amount = amount;

    if u256_is_negative(amount) {
        // We compute the 2's complement
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

pub fn calculate_price_from_sqrt_x96(sqrt_price_x96: U256) -> f64 {
    let q96 = 2f64.powi(96);
    let sqrt_price = sqrt_price_x96.to_string().parse::<f64>().unwrap();
    (sqrt_price/ q96).powi(2)
}

pub fn adjust_price_by_decimals(decimals0: i8, decimals1: i8, unadjusted_price: f64) -> f64{
    unadjusted_price * 10.0_f64.powi((decimals0 - decimals1) as i32)
}