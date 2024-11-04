use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, error};
use anyhow::Result;
use futures::future::join_all;

use crate::{
    config::AppConfig,
    data_sources::{
        dex::uniswap::UniswapHandler,
        cex::kraken::run_kraken_with_sender,
    },
    types::PriceUpdate,
    volatility::estimator::VolatilityProcessor,
};

pub struct Runtime {
    config: Arc<AppConfig>,
}

impl Runtime {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting application...");

        let (price_tx, price_rx) = mpsc::channel(100);
        let processor = VolatilityProcessor::new(self.config.volatility.clone());

        let handles = self.spawn_tasks(price_tx, price_rx, processor).await?;

        join_all(handles).await;

        Ok(())
    }

    async fn spawn_tasks(
        &self,
        price_tx: mpsc::Sender<PriceUpdate>,
        price_rx: mpsc::Receiver<PriceUpdate>,
        processor: VolatilityProcessor,
    ) -> Result<Vec<tokio::task::JoinHandle<()>>> {
        let mut handles = Vec::new();

        // Spawn Kraken handler
        handles.push(tokio::spawn({
            let config = self.config.clone();
            let tx = price_tx.clone();
            async move {
                if let Err(e) = run_kraken_with_sender(tx, config).await {
                    error!("Kraken handler error: {}", e);
                }
            }
        }));

        // Spawn Uniswap handler
        handles.push(tokio::spawn({
            let config = self.config.clone();
            let tx = price_tx;
            async move {
                match UniswapHandler::new(&config).await {
                    Ok(handler) => {
                        if let Err(e) = handler.start_listening_with_sender(tx).await {
                            error!("Uniswap handler error: {}", e);
                        }
                    },
                    Err(e) => error!("Failed to initialize Uniswap handler: {}", e),
                }
            }
        }));

        // Spawn price processor
        handles.push(tokio::spawn({
            let config = self.config.clone();
            async move {
                process_price_updates(price_rx, processor, config).await;
            }
        }));

        Ok(handles)
    }
}

async fn process_price_updates(
    mut price_rx: mpsc::Receiver<PriceUpdate>,
    mut processor: VolatilityProcessor,
    config: Arc<AppConfig>,
) {
    let mut latest_kraken = None;
    let mut latest_uniswap = None;

    while let Some(update) = price_rx.recv().await {
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

            if let Err(e) = crate::io::append_volatility_to_csv(
                &estimate,
                config.output.data_dir
                    .join(&config.output.volatility_file)
                    .to_str()
                    .unwrap()
            ) {
                error!("Failed to write volatility estimate: {}", e);
            }
        }
    }
}