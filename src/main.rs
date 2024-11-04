use dotenv::dotenv;
use anyhow::Result;

mod io;
mod processors;
mod data_sources;
mod volatility;
mod types;
mod config;
mod app;
use crate::types::PriceUpdate;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let config = config::AppConfig::load()?;
    let runtime = app::Runtime::new(config);

    runtime.run().await?;

    Ok(())
}
