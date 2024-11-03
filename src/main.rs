// src/main.rs
use std::sync::Arc;
use tokio::{sync::Mutex, time::interval, signal};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use volatility_estimator::{
    prelude::*,
    config::load_config,
    infrastructure::storage::CSVStorage,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .try_init()?;

    // Load configuration
    let config = load_config().await?;

    // Initialize shared components
    let storage = Arc::new(CSVStorage::new(config.storage.clone()).await?);
    let volatility_calculator: Arc<Mutex<VolatilityCalculator>> = Arc::new(Mutex::new(
        VolatilityCalculator::new(config.volatility.clone())
    ));

    // Initialize services
    let market_data_service = MarketDataService::new(
        storage.clone(),
        volatility_calculator,
    );
    let maintenance_service = MaintenanceService::new(storage);

    // Create shutdown channel
    let (shutdown_send, shutdown_recv) = tokio::sync::broadcast::channel(1);
    let shutdown_send = Arc::new(shutdown_send);

    // Spawn services
    let cleanup_interval = interval(std::time::Duration::from_secs(3600));
    let maintenance_handle = {
        let shutdown = shutdown_send.subscribe();
        tokio::spawn(async move {
            maintenance_service.run_cleanup(cleanup_interval, shutdown).await
        })
    };

    let kraken_handle = {
        let shutdown = shutdown_send.subscribe();
        tokio::spawn(async move {
            market_data_service.run_kraken_handler(
                config.data_sources.kraken,
                shutdown,
            ).await
        })
    };

    let uniswap_handle = {
        let shutdown = shutdown_send.subscribe();
        tokio::spawn(async move {
            market_data_service.run_uniswap_handler(
                config.data_sources.uniswap,
                shutdown,
            ).await
        })
    };

    // Wait for shutdown signal
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
    }

    // Trigger shutdown
    let _ = shutdown_send.send(());

    // Wait for tasks to complete
    let _ = tokio::join!(maintenance_handle, kraken_handle, uniswap_handle);

    info!("Shutdown complete");
    Ok(())
}