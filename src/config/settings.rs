use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use chrono::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub volatility: VolatilityConfig,
    pub data_sources: DataSourcesConfig,
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VolatilityConfig {
    pub cex_weight: f64,
    pub dex_weight: f64,
    #[serde(with = "duration_serde")]
    pub rolling_window: Duration,  // 6 hours in seconds
    #[serde(with = "duration_serde")]
    pub sampling_interval: Duration,  // 1 minute in seconds
    pub annualization_factor: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DataSourcesConfig {
    pub uniswap: UniswapConfig,
    pub kraken: KrakenConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UniswapConfig {
    pub pool_address: String,
    pub websocket_url: String,
    pub abi_path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KrakenConfig {
    pub currency_pair: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub csv_directory: PathBuf,
    pub max_file_size: u64,
    pub compress_old_files: bool,
}

mod duration_serde {
    use chrono::Duration;
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let seconds = i64::deserialize(deserializer)?;
        Ok(Duration::seconds(seconds))
    }
}

impl AppConfig {
    pub fn load() -> Result<Self, config::ConfigError> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name("config/default"))
            // Optional local overrides
            .add_source(config::File::with_name("config/local").required(false))
            // Optional environment-specific overrides
            .add_source(config::Environment::with_prefix("APP").separator("__"))
            .build()?;

        settings.try_deserialize()
    }

    pub fn validate(&self) -> Result<(), String> {
        // Validate volatility weights sum to 1.0
        if (self.volatility.cex_weight + self.volatility.dex_weight - 1.0).abs() > f64::EPSILON {
            return Err("CEX and DEX weights must sum to 1.0".to_string());
        }

        // Validate positive durations
        if self.volatility.rolling_window.num_seconds() <= 0 {
            return Err("Rolling window must be positive".to_string());
        }
        if self.volatility.sampling_interval.num_seconds() <= 0 {
            return Err("Sampling interval must be positive".to_string());
        }

        // Validate sampling interval is less than rolling window
        if self.volatility.sampling_interval >= self.volatility.rolling_window {
            return Err("Sampling interval must be less than rolling window".to_string());
        }

        Ok(())
    }
}
impl Default for KrakenConfig {
    fn default() -> Self {
        Self {
            currency_pair: "ETH/USD".to_string(),
        }
    }
}
impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            csv_directory: "data".into(),
            max_file_size: 104857600,
            compress_old_files: true,
        }
    }
}