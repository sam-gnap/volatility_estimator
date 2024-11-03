use async_trait::async_trait;
use tokio::sync::mpsc;
use crate::core::domain::{Price, Trade};
use crate::error::Result;

#[async_trait]
pub trait PriceSource {
    async fn init(&mut self) -> Result<()>;
    async fn start(&mut self, price_tx: mpsc::Sender<Price>) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
}

#[async_trait]
pub trait TradeSource {
    async fn init(&mut self) -> Result<()>;
    async fn start(&mut self, trade_tx: mpsc::Sender<Trade>) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
}
