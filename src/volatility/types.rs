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
    #[serde(default = "default_cex_weight")]
    pub cex_weight: f64,
    #[serde(default = "default_dex_weight")]
    pub dex_weight: f64,
    pub windows: Vec<VolatilityWindow>,
}

fn default_cex_weight() -> f64 { 0.7 }
fn default_dex_weight() -> f64 { 0.3 }
