// src/core/processors/vwap/calculator.rs
use std::collections::BTreeMap;
use chrono::{DateTime, TimeZone, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use tracing::{debug, trace};

use crate::core::domain::{Trade, Source};
use super::types::VWAPData;

#[derive(Debug)]
struct MinuteBar {
    start_time: DateTime<Utc>,
    volume_sum: Decimal,
    volume_price_sum: Decimal,
    trade_count: u32,
    source: Source,
}

impl MinuteBar {
    fn new(start_time: DateTime<Utc>, source: Source) -> Self {
        Self {
            start_time,
            volume_sum: Decimal::ZERO,
            volume_price_sum: Decimal::ZERO,
            trade_count: 0,
            source,
        }
    }

    fn add_trade(&mut self, trade: &Trade) {
        self.volume_sum += trade.quantity;
        self.volume_price_sum += trade.quantity * trade.price;
        self.trade_count += 1;
    }

    fn calculate_vwap(&self) -> Option<Decimal> {
        if self.volume_sum.is_zero() {
            None
        } else {
            Some(self.volume_price_sum / self.volume_sum)
        }
    }
}

#[derive(Debug)]
pub struct VWAPCalculator {
    current_bar: MinuteBar,
    historical_bars: BTreeMap<DateTime<Utc>, Decimal>,
    source: Source,
}

impl VWAPCalculator {
    pub fn new(source: Source) -> Self {
        let start_time = Self::normalize_to_minute(Utc::now());
        Self {
            current_bar: MinuteBar::new(start_time, source),
            historical_bars: BTreeMap::new(),
            source,
        }
    }

    fn normalize_to_minute(timestamp: DateTime<Utc>) -> DateTime<Utc> {
        let secs = timestamp.timestamp();
        let normalized_secs = (secs / 60) * 60;
        match Utc.timestamp_opt(normalized_secs, 0).single() {
            Some(dt) => dt,
            None => timestamp // Fallback to original timestamp if normalization fails
        }
    }

    pub fn process_trades(&mut self, trades: &[Trade]) -> Option<VWAPData> {
        if trades.is_empty() {
            return None;
        }

        trace!("Processing {} trades for source {:?}", trades.len(), self.source);
        let mut completed_bar = None;

        for trade in trades {
            if trade.source != self.source {
                continue;
            }

            let bar_start = Self::normalize_to_minute(trade.timestamp);

            // If trade belongs to a new minute
            if bar_start > self.current_bar.start_time {
                // Complete current bar if it has trades
                if let Some(vwap) = self.current_bar.calculate_vwap() {
                    self.historical_bars.insert(self.current_bar.start_time, vwap);
                    completed_bar = Some(VWAPData {
                        start_time: self.current_bar.start_time,
                        source: self.source,
                        vwap,
                        volume: self.current_bar.volume_sum,
                        trade_count: self.current_bar.trade_count,
                    });

                    debug!(
                        source = ?self.source,
                        time = %self.current_bar.start_time,
                        vwap = %vwap,
                        volume = %self.current_bar.volume_sum,
                        trades = self.current_bar.trade_count,
                        "Completed VWAP bar"
                    );
                }
                // Start new bar
                self.current_bar = MinuteBar::new(bar_start, self.source);
            }

            self.current_bar.add_trade(trade);
        }

        completed_bar
    }

    #[cfg(test)]
    pub fn get_historical_bars(&self) -> &BTreeMap<DateTime<Utc>, Decimal> {
        &self.historical_bars
    }
}
