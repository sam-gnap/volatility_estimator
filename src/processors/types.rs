use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Price {
    pub timestamp: DateTime<Utc>,
    pub price: f64,
    pub volume: f64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VWAPBar {
    pub timestamp: DateTime<Utc>,
    pub vwap: f64,
    pub volume: f64,
    pub trade_count: u32,
    pub source: String,
}
