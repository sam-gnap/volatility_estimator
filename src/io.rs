use std::path::Path;
use anyhow::Result;
use kraken_ws_client::api::{Trade, TradeEvent};
use chrono::{DateTime, Utc};
use crate::volatility::estimator::VolatilityEstimate;
use crate::types::StandardizedTrade;

#[derive(Debug)]
pub struct VWAPData {
    pub start_time: DateTime<Utc>,
    pub source: String,
    pub vwap: f64,
    pub trade_count: u32,
    pub is_filled_forward: bool,
}

impl From<&Trade> for StandardizedTrade {
    fn from(trade: &Trade) -> Self {
        let timestamp = DateTime::parse_from_rfc3339(&trade.timestamp)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        StandardizedTrade {
            timestamp,
            source: "kraken".to_string(),
            qty: trade.qty,
            price: trade.price,
        }
    }
}

pub fn process_trade_event(event: &TradeEvent) -> Vec<StandardizedTrade> {
    event.data.iter()
        .map(|trade| trade.into())
        .collect()
}

pub fn append_to_csv(trades: &[StandardizedTrade], file_path: &str) -> Result<()> {
    // Create directory if it doesn't exist
    if let Some(parent) = Path::new(file_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file_exists = Path::new(file_path).exists();
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(file_path)?;

    let mut wtr = csv::WriterBuilder::new()
        .has_headers(!file_exists)
        .from_writer(file);

    if !file_exists {
        wtr.write_record(&[
            "source",
            "timestamp",
            "qty",
            "price",
        ])?;
    }

    // Write each trade in the slice
    for trade in trades {
        wtr.write_record(&[
            &trade.source,
            &trade.timestamp.to_rfc3339(),
            &trade.qty.to_string(),
            &trade.price.to_string(),
        ])?;
    }

    wtr.flush()?;
    Ok(())
}

pub fn append_volatility_to_csv(estimate: &VolatilityEstimate, file_path: &str) -> Result<()> {
    if let Some(parent) = Path::new(file_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file_exists = Path::new(file_path).exists();
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(file_path)?;

    let mut wtr = csv::WriterBuilder::new()
        .has_headers(!file_exists)
        .from_writer(file);

    if !file_exists {
        wtr.write_record(&[
            "timestamp",
            "window_name",
            "volatility",
            "num_observations",
            "window_start",
            "window_end",
        ])?;
    }

    wtr.write_record(&[
        &estimate.timestamp.to_rfc3339(),
        &estimate.window_name,
        &estimate.volatility.to_string(),
        &estimate.num_observations.to_string(),
        &estimate.window_start.to_rfc3339(),
        &estimate.window_end.to_rfc3339(),
    ])?;

    wtr.flush()?;
    Ok(())
}

pub fn append_vwap_to_csv(vwap_data: &VWAPData, file_path: &str) -> Result<()> {
    if let Some(parent) = Path::new(file_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file_exists = Path::new(file_path).exists();
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(file_path)?;

    let mut wtr = csv::WriterBuilder::new()
        .has_headers(!file_exists)
        .from_writer(file);

    if !file_exists {
        wtr.write_record(&[
            "timestamp",
            "source",
            "vwap",
            "trade_count",
            "filled_forward"
        ])?;
    }

    wtr.write_record(&[
        &vwap_data.start_time.to_rfc3339(),
        &vwap_data.source,
        &vwap_data.vwap.to_string(),
        &vwap_data.trade_count.to_string(),
        &vwap_data.is_filled_forward.to_string(),
    ])?;

    wtr.flush()?;
    Ok(())
}