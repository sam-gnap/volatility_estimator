use futures::StreamExt;
use tokio::sync::mpsc;
use tracing::error;
use crate::{io, processors};
use crate::types::PriceUpdate;

pub async fn run_kraken_with_sender(price_tx: mpsc::Sender<PriceUpdate>) -> anyhow::Result<()> {
    let mut calculator = processors::vwap::VWAPCalculator::new("kraken");
    let mut client = kraken_ws_client::connect_public().await?;

    client.send(kraken_ws_client::api::SubscribeTradeRequest::symbol("ETH/USD")).await?;

    while let Some(event) = client.trade_events().next().await {
        let trades = io::process_trade_event(&event);
        io::append_to_csv(&trades, "data/kraken_trades.csv")?;

        if let Some(vwap_data) = calculator.process_trades(&trades) {
            io::append_vwap_to_csv(&vwap_data, "data/kraken_vwap.csv")?;

            // Send to volatility processor
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