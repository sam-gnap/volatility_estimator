use std::path::PathBuf;
use anyhow::{Context, Result};
use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};
use crate::volatility::types::VolatilityConfig;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub volatility: VolatilityConfig,
    pub data_sources: DataSourcesConfig,
    pub output: OutputConfig,
    pub cleaning: CleaningConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CleaningConfig {
    pub mean_window: usize,
    pub std_dev_threshold: f64,
    pub min_volume: f64,
    pub max_volume: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataSourcesConfig {
    pub uniswap: UniswapConfig,
    pub kraken: KrakenConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UniswapConfig {
    #[serde(default)]
    pub pool_address: String,
    #[serde(default)]
    pub websocket_url: String,
    pub abi_path: PathBuf,
    pub decimal_token0: i8,
    pub decimal_token1: i8,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KrakenConfig {
    pub trading_pair: String,
    pub ws_buffer_size: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutputConfig {
    pub data_dir: PathBuf,
    pub trade_files: TradeFilesConfig,
    pub vwap_files: VWAPFilesConfig,
    pub volatility_file: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TradeFilesConfig {
    pub kraken: String,
    pub uniswap: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VWAPFilesConfig {
    pub kraken: String,
    pub uniswap: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ReturnType {
    LogReturns,
    SimpleReturns,
    AbsoluteReturns,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let websocket_url = std::env::var("INFURA_WEBSOCKET")
            .context("INFURA_WEBSOCKET environment variable not set")?;

        let pool_address = std::env::var("UNISWAP_POOL_ADDRESS")
            .context("UNISWAP_POOL_ADDRESS environment variable not set")?;

        println!("Loading config with:");
        println!("  Websocket URL: {}", websocket_url);
        println!("  Pool address: {}", pool_address);
        // Validate websocket URL
        if !websocket_url.starts_with("wss://") {
            return Err(anyhow::anyhow!("Invalid websocket URL format. Must start with wss://"));
        }

        let config_path = std::env::var("CONFIG_PATH")
            .unwrap_or_else(|_| "src/config/config.toml".to_string());

        println!("Loading config from: {}", config_path);

        let config: AppConfig = Config::builder()
            .add_source(File::with_name(&config_path))
            .add_source(Environment::with_prefix("APP").separator("_").try_parsing(true))
            .add_source(Environment::with_prefix(""))
            .set_override("data_sources.uniswap.websocket_url", websocket_url)?
            .set_override("data_sources.uniswap.pool_address", pool_address)?
            .build()?
            .try_deserialize()?;

        println!("Loaded volatility windows: {:?}", config.volatility.windows);

        Ok(config)
    }
}