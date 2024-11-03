use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use crate::core::domain::Source;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VWAPData {
    pub start_time: DateTime<Utc>,
    pub source: Source,
    pub vwap: Decimal,
    pub volume: Decimal,
    pub trade_count: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct MinuteBar {
    pub start_time: DateTime<Utc>,
    pub volume_sum: Decimal,
    pub volume_price_sum: Decimal,
    pub trade_count: u32,
    pub source: String,
}