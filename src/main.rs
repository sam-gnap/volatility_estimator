use dotenv::dotenv;
use anyhow::Result;
use futures::StreamExt;
use tokio::sync::{mpsc, Mutex};

mod io;
use std::sync::Arc;
use tracing::{info, error};
mod processors;
mod data_sources;  // change from pub mod to mod
mod volatility;    // change from pub mod to mod
mod types;         // add this if not there already

// Use local crate references:
use crate::{
    data_sources::dex::uniswap::UniswapHandler,
    data_sources::cex::kraken::run_kraken_with_sender,
    volatility::estimator::{VolatilityProcessor, VolatilityConfig, ReturnType},
    types::PriceUpdate,
};
use crate::io::append_volatility_to_csv;

struct SharedState {
    volatility_processor: Mutex<VolatilityProcessor>,
}

async fn run_volatility_processor(
    mut price_rx: mpsc::Receiver<PriceUpdate>,
    shared_state: Arc<SharedState>,
) {
    let mut latest_kraken = None;
    let mut latest_uniswap = None;

    while let Some(update) = price_rx.recv().await {
        let mut processor = shared_state.volatility_processor.lock().await;

        match update {
            PriceUpdate::Kraken(timestamp, price) => {
                latest_kraken = Some((timestamp, price));
            }
            PriceUpdate::Uniswap(timestamp, price) => {
                latest_uniswap = Some((timestamp, price));
            }
        }

        // Use the most recent timestamp from either source
        let current_timestamp = match (&latest_kraken, &latest_uniswap) {
            (Some((t1, _)), Some((t2, _))) => *t1.max(t2),
            (Some((t, _)), None) => *t,
            (None, Some((t, _))) => *t,
            (None, None) => continue,
        };

        if let Some(estimate) = processor.process_vwaps(
            current_timestamp,
            latest_kraken.map(|(_, p)| p),
            latest_uniswap.map(|(_, p)| p),
        ) {
            info!(
                "New volatility estimate: {:.4}% ({} observations)",
                estimate.volatility * 100.0,
                estimate.num_observations
            );

            // You could write this to a CSV or handle it as needed
            if let Err(e) = append_volatility_to_csv(&estimate, "data/volatility.csv") {
                error!("Failed to write volatility estimate: {}", e);
            }
        }
    }
}


async fn run_uniswap_with_sender(
    websocket_url: String,
    pool_address: String,
    path_abi: String,
    price_tx: mpsc::Sender<PriceUpdate>,
) -> Result<()> {
    let uniswap_handler = UniswapHandler::new(
        &websocket_url,
        &pool_address,
        &path_abi
    ).await?;

    // Modify UniswapHandler to accept a price sender
    uniswap_handler.start_listening_with_sender(price_tx).await?;

    Ok(())
}


#[tokio::main]
async fn main() -> Result<()> {
    // Initialize environment variables
    dotenv().ok();

    // Setup tracing for better logging
    tracing_subscriber::fmt::init();

    // Get configuration
    let path_abi = "/Users/gnapsamuel/Documents/AMM/volatility_estimator/src/contracts/uniswap_abi.json";
    let pool_address = std::env::var("UNISWAP_POOL_ADDRESS")?;
    let websocket_infura_endpoint = std::env::var("INFURA_WEBSOCKET")?;

    let config = VolatilityConfig {
        cex_weight: 0.7,
        dex_weight: 0.3,
        rolling_window: chrono::Duration::hours(6),
        sampling_interval: chrono::Duration::minutes(1),
        annualization_factor: 525600.0,
        return_type: ReturnType::LogReturns,
    };

    let processor = VolatilityProcessor::new(config);
    let shared_state = Arc::new(SharedState {
        volatility_processor: Mutex::new(processor),
    });

    // Create channel for price updates
    let (price_tx, price_rx) = mpsc::channel(100);
    let price_tx_kraken = price_tx.clone();
    let price_tx_uniswap = price_tx;

    // Spawn handlers
    let volatility_handle = tokio::spawn(run_volatility_processor(
        price_rx,
        shared_state.clone(),
    ));

    let kraken_handle = tokio::spawn(run_kraken_with_sender(price_tx_kraken));

    let uniswap_handle = tokio::spawn(run_uniswap_with_sender(
        websocket_infura_endpoint,
        pool_address,
        path_abi.to_string(),
        price_tx_uniswap,
    ));

    // Wait for all handlers
    let _ = tokio::join!(volatility_handle, kraken_handle, uniswap_handle);

    Ok(())
}
