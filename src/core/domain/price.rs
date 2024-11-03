use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use super::source::Source;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Price {
    pub timestamp: DateTime<Utc>,
    pub value: Decimal,
    pub source: Source,
}

impl Price {
    pub fn new(timestamp: DateTime<Utc>, value: Decimal, source: Source) -> Self {
        Self {
            timestamp,
            value,
            source,
        }
    }
}