//! Common types used throughout the library

use chrono::{DateTime, Utc};


#[derive(Debug, Clone)]
pub enum PriceUpdate {
    Kraken {
        timestamp: DateTime<Utc>,
        price: f64,
        volume: f64,
    },
    Uniswap {
        timestamp: DateTime<Utc>,
        price: f64,
    }
}

#[derive(Debug, Clone)]
pub struct VWAPData {
    pub timestamp: DateTime<Utc>,
    pub price: f64,
    pub volume: f64,
}

#[derive(Debug, Clone)]
pub struct VolatilityData {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
}