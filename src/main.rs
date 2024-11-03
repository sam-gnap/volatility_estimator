use dotenv::dotenv;
use anyhow::Result;
use tokio::sync::{mpsc, Mutex};

mod io;
use std::sync::Arc;
use tracing::{info, error};
mod processors;
mod data_sources;
mod volatility;
mod types;
mod config;


use crate::{
    data_sources::dex::uniswap::UniswapHandler,
    data_sources::cex::kraken::run_kraken_with_sender,
    volatility::estimator::VolatilityProcessor,
    types::PriceUpdate,
    config::AppConfig
};
use crate::io::append_volatility_to_csv;


async fn run_uniswap_with_sender(
    config: Arc<AppConfig>,
    price_tx: mpsc::Sender<PriceUpdate>,
) -> Result<()> {
    info!("Initializing Uniswap handler...");

    let uniswap_handler = match UniswapHandler::new(&config).await {
        Ok(handler) => handler,
        Err(e) => {
            error!("Failed to initialize Uniswap handler: {}", e);
            return Err(e.into());
        }
    };

    info!("Starting Uniswap listener...");
    match uniswap_handler.start_listening_with_sender(price_tx).await {
        Ok(_) => info!("Uniswap listener completed successfully"),
        Err(e) => error!("Uniswap listener failed: {}", e),
    }

    Ok(())
}

struct SharedState {
    volatility_processor: Mutex<VolatilityProcessor>,
}

async fn run_volatility_processor(
    mut price_rx: mpsc::Receiver<PriceUpdate>,
    shared_state: Arc<SharedState>,
    config: Arc<AppConfig>,
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

        let current_timestamp = match (&latest_kraken, &latest_uniswap) {
            (Some((t1, _)), Some((t2, _))) => *t1.max(t2),
            (Some((t, _)), None) => *t,
            (None, Some((t, _))) => *t,
            (None, None) => continue,
        };

        let estimates = processor.process_vwaps(
            current_timestamp,
            latest_kraken.map(|(_, p)| p),
            latest_uniswap.map(|(_, p)| p),
        );

        for estimate in estimates {
            info!(
                "New volatility estimate for window {}: {:.4}% ({} observations)",
                estimate.window_name,
                estimate.volatility * 100.0,
                estimate.num_observations
            );

            let volatility_path = config.output.data_dir
                .join(&config.output.volatility_file);

            if let Err(e) = append_volatility_to_csv(&estimate, volatility_path.to_str().unwrap()) {
                error!(
                    window = %estimate.window_name,
                    error = %e,
                    "Failed to write volatility estimate"
                );
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let config = Arc::new(AppConfig::load()?);
    let processor = VolatilityProcessor::new(config.volatility.clone());
    let shared_state = Arc::new(SharedState {
        volatility_processor: Mutex::new(processor),
    });
    info!(
        "INFURA_WEBSOCKET: {}",
        std::env::var("INFURA_WEBSOCKET").unwrap_or_else(|_| "NOT SET".to_string())
    );
    info!(
        "UNISWAP_POOL_ADDRESS: {}",
        std::env::var("UNISWAP_POOL_ADDRESS").unwrap_or_else(|_| "NOT SET".to_string())
    );


    let config_volatility = Arc::clone(&config);
    let config_uniswap = Arc::clone(&config);
    let config_kraken = Arc::clone(&config);

    let (price_tx, price_rx) = mpsc::channel(100);
    let price_tx_kraken = price_tx.clone();
    let price_tx_uniswap = price_tx;

    let volatility_handle = tokio::spawn(run_volatility_processor(
        price_rx,
        shared_state.clone(),
        config_volatility,
    ));
    let kraken_handle = tokio::spawn(run_kraken_with_sender(price_tx_kraken, config_kraken));
    let uniswap_handle = tokio::spawn(run_uniswap_with_sender(
        config_uniswap,
        price_tx_uniswap,
    ));

    let _ = tokio::join!(volatility_handle, kraken_handle, uniswap_handle);

    Ok(())
}
