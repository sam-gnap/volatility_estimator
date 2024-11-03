use std::collections::VecDeque;
use crate::types::StandardizedTrade;

pub struct PriceFilter {
    mean_window: usize,
    std_dev_threshold: f64,
    price_history: VecDeque<f64>,
}

pub struct VolumeFilter {
    min_volume: f64,
    max_volume: f64,
}

pub struct DataCleaner {
    price_filter: PriceFilter,
    volume_filter: VolumeFilter,
}

impl PriceFilter {
    pub fn new(mean_window: usize, std_dev_threshold: f64) -> Self {
        Self {
            mean_window,
            std_dev_threshold,
            price_history: VecDeque::with_capacity(mean_window),
        }
    }

    pub fn is_valid_price(&mut self, price: f64) -> bool {
        if self.price_history.len() < self.mean_window {
            self.price_history.push_back(price);
            return true;
        }

        let mean = self.calculate_mean();
        let std_dev = self.calculate_std_dev(mean);
        let lower_bound = mean - (self.std_dev_threshold * std_dev);
        let upper_bound = mean + (self.std_dev_threshold * std_dev);

        let is_valid = price >= lower_bound && price <= upper_bound;

        if is_valid {
            if self.price_history.len() >= self.mean_window {
                self.price_history.pop_front();
            }
            self.price_history.push_back(price);
        }

        is_valid
    }

    fn calculate_mean(&self) -> f64 {
        let sum: f64 = self.price_history.iter().sum();
        sum / self.price_history.len() as f64
    }

    fn calculate_std_dev(&self, mean: f64) -> f64 {
        let variance: f64 = self.price_history
            .iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / self.price_history.len() as f64;
        variance.sqrt()
    }
}

impl VolumeFilter {
    pub fn new(min_volume: f64, max_volume: f64) -> Self {
        Self {
            min_volume,
            max_volume,
        }
    }

    pub fn is_valid_volume(&self, volume: f64) -> bool {
        volume >= self.min_volume && volume <= self.max_volume
    }
}

impl DataCleaner {
    pub fn new(
        mean_window: usize,
        std_dev_threshold: f64,
        min_volume: f64,
        max_volume: f64,
    ) -> Self {
        Self {
            price_filter: PriceFilter::new(mean_window, std_dev_threshold),
            volume_filter: VolumeFilter::new(min_volume, max_volume),
        }
    }

    pub fn clean_trade(&mut self, trade: &StandardizedTrade) -> bool {
        self.price_filter.is_valid_price(trade.price)
            && self.volume_filter.is_valid_volume(trade.qty)
    }
}