// src/lib.rs
pub mod volatility;  // This needs to come first
pub mod data_sources;
pub mod types;
pub mod processors;
pub mod io;
pub use types::PriceUpdate;
pub use volatility::estimator::VolatilityEstimate;