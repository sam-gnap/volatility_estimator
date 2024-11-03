use std::path::{Path, PathBuf};
use chrono::{DateTime, TimeZone, Utc};
use rust_decimal::Decimal;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn, error};

use crate::core::domain::{Price, Trade, Source};
use crate::core::processors::vwap::VWAPData;
use crate::error::{Error, Result};
use crate::config::StorageConfig;

pub struct CSVStorage {
    config: StorageConfig,
    trade_path: PathBuf,
    vwap_path: PathBuf,
    volatility_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StorageFile {
    path: PathBuf,
    current_size: u64,
}

impl CSVStorage {
    pub async fn new(config: StorageConfig) -> Result<Self> {
        let base_dir = config.csv_directory.clone();

        // Ensure directory exists
        fs::create_dir_all(&base_dir).await
            .map_err(|e| Error::storage(format!("Failed to create directory: {}", e)))?;

        let storage = Self {
            trade_path: base_dir.join("trades.csv"),
            vwap_path: base_dir.join("vwap.csv"),
            volatility_path: base_dir.join("volatility.csv"),
            config,
        };

        // Initialize files with headers
        storage.initialize_files().await?;

        Ok(storage)
    }

    async fn initialize_files(&self) -> Result<()> {
        self.initialize_file(&self.trade_path, "timestamp,source,price,quantity\n").await?;
        self.initialize_file(&self.vwap_path, "timestamp,source,vwap,volume,trade_count\n").await?;
        self.initialize_file(&self.volatility_path, "timestamp,value,window_start,window_end,num_observations\n").await?;
        Ok(())
    }

    async fn initialize_file(&self, path: &Path, header: &str) -> Result<()> {
        if !path.exists() {
            let mut file = fs::File::create(path).await
                .map_err(|e| Error::storage(format!("Failed to create file {}: {}", path.display(), e)))?;

            file.write_all(header.as_bytes()).await
                .map_err(|e| Error::storage(format!("Failed to write header: {}", e)))?;
        }
        Ok(())
    }

    async fn rotate_file_if_needed(&self, file: &StorageFile) -> Result<()> {
        if file.current_size >= self.config.max_file_size {
            let backup_path = file.path.with_extension(format!(
                "csv.{}",
                Utc::now().format("%Y%m%d_%H%M%S")
            ));

            fs::rename(&file.path, &backup_path).await
                .map_err(|e| Error::storage(format!("Failed to rotate file: {}", e)))?;

            if self.config.compress_old_files {
                let backup_path_clone = backup_path.clone();
                tokio::spawn(async move {
                    if let Err(e) = Self::compress_file(backup_path_clone).await {
                        error!("Failed to compress file: {}", e);
                    }
                });
            }

            info!(
                path = %file.path.display(),
                backup = %backup_path.display(),
                "Rotated file"
            );
        }
        Ok(())
    }

    async fn compress_file(path: PathBuf) -> Result<()> {
        use tokio::process::Command;

        let status = Command::new("gzip")
            .arg(path.to_str().unwrap())
            .status()
            .await
            .map_err(|e| Error::storage(format!("Failed to run gzip: {}", e)))?;

        if !status.success() {
            return Err(Error::storage("Compression failed"));
        }

        Ok(())
    }

    pub async fn store_trade(&self, trade: &Trade) -> Result<()> {
        let source_str = match trade.source {
            Source::Kraken => "kraken",
            Source::Uniswap => "uniswap",
        };

        let line = format!(
            "{},{},{},{}\n",
            trade.timestamp.to_rfc3339(),
            source_str,
            trade.price,
            trade.quantity
        );

        self.append_to_file(&self.trade_path, &line).await
    }

    pub async fn store_vwap(&self, vwap: &VWAPData) -> Result<()> {
        let source_str = match vwap.source {
            Source::Kraken => "kraken",
            Source::Uniswap => "uniswap",
        };

        let line = format!(
            "{},{},{},{},{}\n",
            vwap.start_time.to_rfc3339(),
            source_str,
            vwap.vwap,
            vwap.volume,
            vwap.trade_count
        );

        self.append_to_file(&self.vwap_path, &line).await
    }

    pub async fn store_volatility(
        &self,
        timestamp: DateTime<Utc>,
        value: Decimal,
        window_start: DateTime<Utc>,
        window_end: DateTime<Utc>,
        num_observations: usize,
    ) -> Result<()> {
        let line = format!(
            "{},{},{},{},{}\n",
            timestamp.to_rfc3339(),
            value,
            window_start.to_rfc3339(),
            window_end.to_rfc3339(),
            num_observations
        );

        self.append_to_file(&self.volatility_path, &line).await
    }

    async fn append_to_file(&self, path: &Path, content: &str) -> Result<()> {
        let file_size = match fs::metadata(path).await {
            Ok(metadata) => metadata.len(),
            Err(_) => 0,
        };

        let file = StorageFile {
            path: path.to_path_buf(),
            current_size: file_size,
        };

        self.rotate_file_if_needed(&file).await?;

        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await
            .map_err(|e| Error::storage(format!("Failed to open file: {}", e)))?
            .write_all(content.as_bytes())
            .await
            .map_err(|e| Error::storage(format!("Failed to write to file: {}", e)))?;

        Ok(())
    }

    pub async fn cleanup_old_files(&self, max_age: chrono::Duration) -> Result<()> {
        let cutoff = Utc::now() - max_age;

        let mut dir = fs::read_dir(&self.config.csv_directory).await
            .map_err(|e| Error::storage(format!("Failed to read directory: {}", e)))?;

        while let Some(entry) = dir.next_entry().await
            .map_err(|e| Error::storage(format!("Failed to read directory entry: {}", e)))? {

            let path = entry.path();
            if let Some(extension) = path.extension() {
                if extension == "csv" {
                    if let Ok(metadata) = fs::metadata(&path).await {
                        if let Ok(modified) = metadata.modified() {
                            let modified_datetime = DateTime::<Utc>::from(modified);
                            if modified_datetime < cutoff {
                                if let Err(e) = fs::remove_file(&path).await {
                                    warn!("Failed to remove old file {}: {}", path.display(), e);
                                } else {
                                    info!("Removed old file: {}", path.display());
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::domain::Source;
    use rust_decimal_macros::dec;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_csv_storage() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config = StorageConfig {
            csv_directory: temp_dir.path().to_path_buf(),
            max_file_size: 1024,
            compress_old_files: false,
        };

        let storage = CSVStorage::new(config).await?;

        // Test trade storage
        let trade = Trade {
            timestamp: Utc::now(),
            price: dec!(100.50),
            quantity: dec!(1.5),
            source: Source::Kraken,
        };

        storage.store_trade(&trade).await?;

        // Verify file exists and contains correct data
        let contents = fs::read_to_string(temp_dir.path().join("trades.csv")).await?;
        assert!(contents.contains("kraken"));
        assert!(contents.contains("100.50"));

        Ok(())
    }
}