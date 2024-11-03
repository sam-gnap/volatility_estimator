use std::collections::BTreeMap;
use chrono::{DateTime, Duration, TimeZone, Utc};
use crate::io::{StandardizedTrade, VWAPData};

#[derive(Debug)]
pub struct MinuteBar {
    start_time: DateTime<Utc>,
    volume_sum: f64,
    volume_price_sum: f64,
    trade_count: u32,
    source: String,
}

#[derive(Debug)]
pub struct VWAPCalculator {
    current_bar: MinuteBar,
    historical_bars: BTreeMap<DateTime<Utc>, f64>,
    source: String,
}

impl MinuteBar {
    fn new(start_time: DateTime<Utc>, source: String) -> Self {
        Self {
            start_time,
            volume_sum: 0.0,
            volume_price_sum: 0.0,
            trade_count: 0,
            source,
        }
    }

    fn add_trade(&mut self, trade: &StandardizedTrade) {
        self.volume_sum += trade.qty;
        self.volume_price_sum += trade.qty * trade.price;
        self.trade_count += 1;
    }
}


impl VWAPCalculator {
    pub fn new(source: &str) -> Self {
        let start_time = Self::normalize_to_minute(Utc::now());
        Self {
            current_bar: MinuteBar::new(start_time, source.to_string()),
            historical_bars: BTreeMap::new(),
            source: source.to_string(),
        }
    }

    pub fn normalize_to_minute(timestamp: DateTime<Utc>) -> DateTime<Utc> {
        let secs = timestamp.timestamp();
        let normalized_secs = (secs / 60) * 60;
        Utc.timestamp_opt(normalized_secs, 0).unwrap()
    }

    pub fn process_trades(&mut self, trades: &[StandardizedTrade]) -> Option<VWAPData> {
        let mut completed_bar = None;

        for trade in trades {
            let bar_start = Self::normalize_to_minute(trade.timestamp);

            // If trade belongs to a new minute
            if bar_start > self.current_bar.start_time {
                // Complete current bar if it has trades
                if self.current_bar.volume_sum > 0.0 {
                    let vwap = self.current_bar.volume_price_sum / self.current_bar.volume_sum;
                    self.historical_bars.insert(self.current_bar.start_time, vwap);
                    completed_bar = Some(VWAPData {
                        start_time: self.current_bar.start_time,
                        source: self.current_bar.source.clone(),
                        vwap,
                        trade_count: self.current_bar.trade_count,
                    });
                }
                // Start new bar
                self.current_bar = MinuteBar::new(bar_start, self.source.clone());
            }

            self.current_bar.add_trade(trade);
        }

        completed_bar
    }

    // fn get_latest_prices(&self, window: Duration) -> Vec<(DateTime<Utc>, f64)> {
    //     let cutoff = Utc::now() - window;
    //     self.historical_bars
    //         .range(cutoff..)
    //         .map(|(k, v)| (*k, *v))
    //         .collect()
    // }
}