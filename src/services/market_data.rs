use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::{error, info};
use chrono::Utc;

use crate::{
    config::{KrakenConfig, UniswapConfig},
    types::{PriceUpdate, VWAPData},
    infrastructure::storage::CSVStorage,
    core::{
        domain::{Trade, Source},
        processors::{
            vwap::VWAPCalculator,
            estimator::VolatilityCalculator,
        },
    },

};

pub struct MarketDataService {
    storage: Arc<CSVStorage>,
    volatility_calculator: Arc<Mutex<VolatilityCalculator>>,
}

impl MarketDataService {
    pub fn new(
        storage: Arc<CSVStorage>,
        volatility_calculator: Arc<Mutex<VolatilityCalculator>>,
    ) -> Self {
        Self {
            storage,
            volatility_calculator,
        }
    }

    pub async fn run_kraken_handler(
        &self,
        config: KrakenConfig,
        mut shutdown: broadcast::Receiver,
    ) -> Result> {
    let (tx, mut rx) = mpsc::channel(100);

    let handler = tokio::spawn(async move {
    run_kraken_websocket(config, tx).await
    });

    while !shutdown.is_receiver_closed() {
    tokio::select! {
    Some(update) = rx.recv() => {
    self.process_update(update).await?;
    }
    _ = shutdown.recv() => break,
    }
    }

    handler.abort();
    Ok(())
    }


    pub async fn run_uniswap_handler(
        &self,
        config: UniswapConfig,
        shutdown: triggered::Listener,
    ) -> Result<()> {
        let (price_tx, mut price_rx) = mpsc::channel(100);
        let mut handler = UniswapHandler::new(config);
        let mut vwap = VWAPCalculator::new(Source::Uniswap);

        let handler_task = tokio::spawn(async move {
            if let Err(e) = handler.start(price_tx).await {
                error!("Uniswap handler error: {}", e);
            }
        });

        self.process_price_stream(price_rx, vwap, Source::Uniswap, shutdown).await?;
        handler_task.abort();

        Ok(())
    }

    async fn process_price_stream(
        &self,
        mut price_rx: mpsc::Receiver<Price>,
        mut vwap: VWAPCalculator,
        source: Source,
        shutdown: triggered::Listener,
    ) -> Result<()> {
        while !shutdown.is_triggered() {
            tokio::select! {
                Some(price) = price_rx.recv() => {
                    self.process_single_price(price, &mut vwap).await?;
                }
                else => break,
            }
        }
        Ok(())
    }

    async fn process_single_price(
        &self,
        price: Price,
        vwap: &mut VWAPCalculator,
    ) -> Result<()> {
        let trade = Trade::new(
            price.timestamp,
            price.value,
            price.volume.unwrap_or_default(),
            price.source,
        );

        // Store raw trade
        if let Err(e) = self.storage.store_trade(&trade).await {
            error!("Failed to store trade: {}", e);
            return Err(e);
        }

        // Process VWAP
        if let Some(vwap_data) = vwap.process_trades(&[trade]) {
            if let Err(e) = self.storage.store_vwap(&vwap_data).await {
                error!("Failed to store VWAP: {}", e);
                return Err(e);
            }

            // Update volatility
            let mut vol_calc = self.volatility_calculator.lock().await;
            if let Some(vol_estimate) = vol_calc.process_price(
                vwap_data.start_time,
                vwap_data.vwap,
                price.source,
            ) {
                if let Err(e) = self.storage.store_volatility(
                    vol_estimate.timestamp,
                    vol_estimate.value,
                    vol_estimate.window_start,
                    vol_estimate.window_end,
                    vol_estimate.num_observations,
                ).await {
                    error!("Failed to store volatility estimate: {}", e);
                    return Err(e);
                }
            }
        }

        Ok(())
    }
}
