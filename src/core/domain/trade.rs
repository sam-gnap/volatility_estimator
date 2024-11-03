use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use super::source::Source;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub timestamp: DateTime<Utc>,
    pub price: Decimal,
    pub quantity: Decimal,
    pub source: Source,
}

impl Trade {
    pub fn new(
        timestamp: DateTime<Utc>,
        price: Decimal,
        quantity: Decimal,
        source: Source,
    ) -> Self {
        Self {
            timestamp,
            price,
            quantity,
            source,
        }
    }
}