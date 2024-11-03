use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolatilityEstimate {
    pub timestamp: DateTime<Utc>,
    pub value: Decimal,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub num_observations: usize,
}