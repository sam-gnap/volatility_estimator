use std::sync::Arc;
use tokio::time::Interval;
use tracing::error;

use crate::infrastructure::storage::CSVStorage;
use crate::error::Result;

pub struct MaintenanceService {
    storage: Arc<CSVStorage>,
}

impl MaintenanceService {
    pub fn new(storage: Arc<CSVStorage>) -> Self {
        Self { storage }
    }

    pub async fn run_cleanup(
        &self,
        mut interval: Interval,
        shutdown: triggered::Listener
    ) -> Result<()> {
        while !shutdown.is_triggered() {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.storage
                        .cleanup_old_files(chrono::Duration::days(7)).await
                    {
                        error!("Failed to cleanup old files: {}", e);
                    }
                }
                else => break,
            }
        }
        Ok(())
    }
}