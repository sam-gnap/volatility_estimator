use std::collections::{VecDeque};
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use super::types::{VolatilityConfig, VolatilityWindow};

#[derive(Debug, Clone, Serialize)]
pub struct WeightedPrice {
    pub timestamp: DateTime<Utc>,
    pub weighted_price: f64,
    pub cex_price: Option<f64>,
    pub dex_price: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VolatilityEstimate {
    pub timestamp: DateTime<Utc>,
    pub volatility: f64,
    pub num_observations: usize,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub window_name: String,  // Added this field
}

pub struct VolatilityProcessor {
    config: VolatilityConfig,
    price_history: BTreeMap<DateTime<Utc>, f64>,
    window_processors: HashMap<String, WindowProcessor>,
}

struct WindowProcessor {
    name: String,
    length: Duration,
    sampling_interval: Duration,
    last_calculation: Option<DateTime<Utc>>,
    returns: VecDeque<f64>,
}

impl WindowProcessor {
    fn new(window: &VolatilityWindow) -> Self {
        Self {
            name: window.name.clone(),
            length: Duration::seconds(window.length_seconds),
            sampling_interval: Duration::seconds(window.sampling_interval),
            last_calculation: None,
            returns: VecDeque::new(),
        }
    }

    fn process_window(&mut self, price_history: &BTreeMap<DateTime<Utc>, f64>, current_time: DateTime<Utc>)
                      -> Option<VolatilityEstimate>
    {
        // Check if it's time to calculate for this window
        if let Some(last_calc) = self.last_calculation {
            if current_time - last_calc < self.sampling_interval {
                return None;
            }
        }

        // Get window of prices based on sampling interval
        let window_start = current_time - self.length;
        let prices: Vec<(DateTime<Utc>, f64)> = price_history
            .range(window_start..=current_time)
            .step_by(self.sampling_interval.num_seconds() as usize / 60) // Convert to minutes
            .map(|(k, v)| (*k, *v))
            .collect();

        if prices.len() < 2 {
            return None;
        }

        // Calculate returns for the window
        self.returns.clear();
        for window in prices.windows(2) {
            let return_value = (window[1].1 / window[0].1).ln();
            self.returns.push_back(return_value);
        }

        // Calculate volatility
        let volatility = self.calculate_volatility();

        self.last_calculation = Some(current_time);

        Some(VolatilityEstimate {
            timestamp: current_time,
            volatility,
            num_observations: self.returns.len(),
            window_start: window_start,
            window_end: current_time,
            window_name: self.name.clone(), // Add this to VolatilityEstimate
        })
    }

    fn calculate_volatility(&self) -> f64 {
        let mean_return: f64 = self.returns.iter().sum::<f64>() / self.returns.len() as f64;
        let variance: f64 = self.returns.iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>() / (self.returns.len() - 1) as f64;

        (variance * self.annualization_factor()).sqrt()
    }

    fn annualization_factor(&self) -> f64 {
        // Calculate based on sampling interval
        (365.0 * 24.0 * 60.0 * 60.0) / self.sampling_interval.num_seconds() as f64
    }
}
impl VolatilityProcessor {
    pub fn new(config: VolatilityConfig) -> Self {
        let mut window_processors = HashMap::new();
        for window in &config.windows {
            window_processors.insert(
                window.name.clone(),
                WindowProcessor::new(window)
            );
        }

        Self {
            config,
            price_history: BTreeMap::new(),
            window_processors,
        }
    }

    pub fn process_vwaps(
        &mut self,
        timestamp: DateTime<Utc>,
        cex_vwap: Option<f64>,
        dex_vwap: Option<f64>,
    ) -> Vec<VolatilityEstimate> {
        // Calculate weighted price as before
        let weighted_price = match (cex_vwap, dex_vwap) {
            (Some(cex), Some(dex)) => {
                (cex * self.config.cex_weight + dex * self.config.dex_weight)
                    / (self.config.cex_weight + self.config.dex_weight)
            }
            (Some(cex), None) => cex,
            (None, Some(dex)) => dex,
            (None, None) => return vec![],
        };

        // Store the 1-minute data
        self.price_history.insert(timestamp, weighted_price);

        // Clean old data
        self.clean_old_data();

        // Calculate volatility for each window
        let mut estimates = Vec::new();
        for processor in self.window_processors.values_mut() {
            if let Some(estimate) = processor.process_window(&self.price_history, timestamp) {
                estimates.push(estimate);
            }
        }

        estimates
    }

    fn clean_old_data(&mut self) {
        // Find the longest window
        let max_length = self.config.windows.iter()
            .map(|w| w.length_seconds)
            .max()
            .unwrap_or(21600); // 6 hours default

        let cutoff = Utc::now() - Duration::seconds(max_length);
        self.price_history.retain(|&k, _| k >= cutoff);
    }
}
