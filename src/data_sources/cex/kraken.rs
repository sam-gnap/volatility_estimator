// src/data_sources/cex/kraken.rs
use futures::StreamExt;
use tokio::sync::mpsc;
use tracing::error;
use std::sync::Arc;
use kraken_ws_client::{connect_public, api::SubscribeTradeRequest};

use crate::core::processors::vwap::VWAPCalculator;
use crate::types::PriceUpdate;
use crate::infrastructure::storage::CSVStorage;
use crate::error::Result;
use crate::core::domain::{Trade, Source};

pub async fn run_kraken_with_sender(
    price_tx: mpsc::Sender<PriceUpdate>,
    storage: Arc<CSVStorage>,
) -> Result<()> {
    let mut calculator = VWAPCalculator::new(Source::Kraken);
    let mut client = kraken_ws_client::connect_public().await.map_err(|e| {
        error!("Failed to connect to Kraken: {}", e);
        crate::error::Error::data_source(e.to_string())
    })?;

    client.send(kraken_ws_client::api::SubscribeTradeRequest::symbol("ETH/USD"))
        .await
        .map_err(|e| crate::error::Error::data_source(e.to_string()))?;

    while let Some(event) = client.trade_events().next().await {
        // Convert event trades to our domain Trade type
        let trades: Vec<Trade> = event.data.iter()
            .map(|t| Trade {
                timestamp: t.timestamp.parse().unwrap(),
                price: t.price.into(),
                quantity: t.qty.into(),
                source: Source::Kraken,
            })
            .collect();

        // Store trades
        storage.store_trade(&trades[0]).await?;

        while let Some(event) = client.trade_events().next().await {
            let trade = &event.data[0];
            tx.send(PriceUpdate::Kraken {
                timestamp: chrono::DateTime::parse_from_rfc3339(&trade.timestamp)?.with_timezone(&Utc),
                price: trade.price,
                volume: trade.qty,
            }).await?;
        }
    }

    Ok(())
}