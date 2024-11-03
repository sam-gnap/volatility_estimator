use futures::StreamExt;
use tokio::sync::mpsc;
use tracing::error;
use crate::{io, processors, config::AppConfig};
use crate::types::PriceUpdate;
use std::path::PathBuf;
use std::sync::Arc;
use crate::processors::cleaning::DataCleaner;

pub async fn run_kraken_with_sender(
    price_tx: mpsc::Sender<PriceUpdate>,
    config: Arc<AppConfig>
) -> anyhow::Result<()> {
    let mut calculator = processors::vwap::VWAPCalculator::new("kraken");
    let mut cleaner = DataCleaner::new(
        config.cleaning.mean_window,
        config.cleaning.std_dev_threshold,
        config.cleaning.min_volume,
        config.cleaning.max_volume,
    );

    let mut client = kraken_ws_client::connect_public().await?;

    client.send(kraken_ws_client::api::SubscribeTradeRequest::symbol(&config.data_sources.kraken.trading_pair)).await?;

    while let Some(event) = client.trade_events().next().await {
        let trades = io::process_trade_event(&event);
        // Clean trades, use std and volume
        let clean_trades: Vec<_> = trades.iter()
            .filter(|trade| cleaner.clean_trade(trade))
            .collect();

        if clean_trades.is_empty() {
            continue;
        }

        let trade_file = PathBuf::from(&config.output.data_dir)
            .join(&config.output.trade_files.kraken);
        io::append_to_csv(&trades, trade_file.to_str().unwrap())?;

        if let Some(vwap_data) = calculator.process_trades(&trades) {
            let vwap_file = PathBuf::from(&config.output.data_dir)
                .join(&config.output.vwap_files.kraken);
            io::append_vwap_to_csv(&vwap_data, vwap_file.to_str().unwrap())?;

            if let Err(e) = price_tx.send(PriceUpdate::Kraken(
                vwap_data.start_time,
                vwap_data.vwap
            )).await {
                error!("Failed to send Kraken price update: {}", e);
            }
        }
    }

    Ok(())
}
