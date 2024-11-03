use std::path::Path;
use anyhow::Result;
use kraken_ws_client::api::{Trade, TradeEvent};
use chrono::{DateTime, Utc};
use crate::volatility::estimator::VolatilityEstimate;

#[derive(Debug)]
pub struct StandardizedTrade {
    pub timestamp: DateTime<Utc>,
    pub source: String,
    // pub side: String,
    pub qty: f64,
    pub price: f64,
}

#[derive(Debug)]
pub struct VWAPData {
    pub start_time: DateTime<Utc>,
    pub source: String,
    pub vwap: f64,
    pub trade_count: u32,
}

// Conversion from Kraken TradeEvent to StandardizedTrade
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

// Helper function for single trade
// pub fn append_single_trade_to_csv(trade: &StandardizedTrade, file_path: &str) -> Result<()> {
//     append_to_csv(std::slice::from_ref(trade), file_path)
// }

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
            "volatility",
            "num_observations",
            "window_start",
            "window_end",
        ])?;
    }

    wtr.write_record(&[
        &estimate.timestamp.to_rfc3339(),
        &estimate.volatility.to_string(),
        &estimate.num_observations.to_string(),
        &estimate.window_start.to_rfc3339(),
        &estimate.window_end.to_rfc3339(),
    ])?;

    wtr.flush()?;
    Ok(())
}

pub fn append_vwap_to_csv(vwap_data: &VWAPData, file_path: &str) -> Result<()> {
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
            "timestamp",
            "source",
            "vwap",
            "trade_count",
        ])?;
    }

    wtr.write_record(&[
        &vwap_data.start_time.to_rfc3339(),
        &vwap_data.source,
        &vwap_data.vwap.to_string(),
        &vwap_data.trade_count.to_string(),
    ])?;

    wtr.flush()?;
    Ok(())
}