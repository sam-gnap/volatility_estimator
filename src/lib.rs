// src/lib.rs

pub mod config;
pub mod core;
pub mod data_sources;
pub mod error;
pub mod infrastructure;
pub mod services;
pub mod types;

// Re-export commonly used types for easier access
pub use {
    config::AppConfig,
    core::{
        domain::{Price, Source, Trade, VolatilityEstimate},
        processors::{
            estimator::VolatilityCalculator,
            vwap::VWAPCalculator,
        },
    },
    data_sources::{PriceSource, TradeSource},
    error::{Error, Result},
    infrastructure::storage::CSVStorage,
    services::{MarketDataService, MaintenanceService},
    types::{PriceUpdate, MarketStatus, MarketState, TradingPair},
};

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Prelude module for convenient imports
pub mod prelude {
    pub use super::{
        Price,
        Source,
        Trade,
        VolatilityEstimate,
        VolatilityCalculator,
        VWAPCalculator,
        PriceSource,
        TradeSource,
        Error,
        Result,
        CSVStorage,
        MarketDataService,
        MaintenanceService,
        PriceUpdate,
        MarketStatus,
        MarketState,
        TradingPair,
    };
}