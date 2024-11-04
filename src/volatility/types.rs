use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub enum ReturnType {
    LogReturns,
    SimpleReturns,
    AbsoluteReturns,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VolatilityWindow {
    pub name: String,
    pub length_seconds: i64,
    pub sampling_interval: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VolatilityConfig {
    pub cex_weight: f64,
    pub dex_weight: f64,
    pub windows: Vec<VolatilityWindow>,
    pub data_retention_seconds: i64,
}
