use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
// #[derive(Debug, Clone)]
// pub enum DataSource {
//     Uniswap(String),  // pool address
//     Binance,
//     // Add more sources as needed
// }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardizedTrade {
    pub source: String,
    pub timestamp: DateTime<Utc>,
    pub side: String,
    pub qty: f64,
    pub price: f64,
}

#[derive(Debug, Clone)]
pub enum PriceUpdate {
    Kraken(DateTime<Utc>, f64),
    Uniswap(DateTime<Utc>, f64),
}