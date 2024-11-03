use std::collections::{VecDeque};
use chrono::{DateTime, Duration, Utc};
use anyhow::Result;
use serde::{Deserialize, Serialize, Deserializer};

fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let seconds = i64::deserialize(deserializer)?;
    Ok(Duration::seconds(seconds))
}

#[derive(Debug, Clone, Deserialize)]
pub struct VolatilityConfig {
    pub cex_weight: f64,
    pub dex_weight: f64,
    #[serde(serialize_with = "serialize_duration", deserialize_with = "deserialize_duration")]
    pub rolling_window: Duration,
    #[serde(serialize_with = "serialize_duration", deserialize_with = "deserialize_duration")]
    pub sampling_interval: Duration,
    pub annualization_factor: f64,
    pub return_type: ReturnType,
}

#[derive(Debug, Clone, Deserialize)]
pub enum ReturnType {
    LogReturns,
    SimpleReturns,
    AbsoluteReturns,
}

// Combined price data point
#[derive(Debug, Clone, Serialize)]
pub struct WeightedPrice {
    pub timestamp: DateTime<Utc>,
    pub weighted_price: f64,
    pub cex_price: Option<f64>,
    pub dex_price: Option<f64>,
}

// Volatility result
#[derive(Debug, Clone, Serialize)]
pub struct VolatilityEstimate {
    pub timestamp: DateTime<Utc>,
    pub volatility: f64,
    pub num_observations: usize,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
}

pub struct VolatilityProcessor {
    config: VolatilityConfig,
    price_history: VecDeque<WeightedPrice>,
    last_calculation: Option<DateTime<Utc>>,
}

impl VolatilityProcessor {
    pub fn new(config: VolatilityConfig) -> Self {
        Self {
            config,
            price_history: VecDeque::new(),
            last_calculation: None,
        }
    }

    pub fn process_vwaps(&mut self, timestamp: DateTime<Utc>, cex_vwap: Option<f64>, dex_vwap: Option<f64>)
                         -> Option<VolatilityEstimate> {
        if cex_vwap.is_none() && dex_vwap.is_none() {
            return None;
        }

        let weighted_price = self.calculate_weighted_price(cex_vwap, dex_vwap);
        let price_point = WeightedPrice {
            timestamp,
            weighted_price,
            cex_price: cex_vwap,
            dex_price: dex_vwap,
        };
        self.price_history.push_back(price_point);
        self.clean_old_data();
        self.calculate_volatility_if_needed()
    }

    fn calculate_weighted_price(&self, cex_price: Option<f64>, dex_price: Option<f64>) -> f64 {
        match (cex_price, dex_price) {
            (Some(cex), Some(dex)) => {
                (cex * self.config.cex_weight + dex * self.config.dex_weight)
                    / (self.config.cex_weight + self.config.dex_weight)
            }
            (Some(cex), None) => cex,
            (None, Some(dex)) => dex,
            (None, None) => unreachable!(),
        }
    }

    fn clean_old_data(&mut self) {
        let cutoff = Utc::now() - self.config.rolling_window;
        while let Some(front) = self.price_history.front() {
            if front.timestamp < cutoff {
                self.price_history.pop_front();
            } else {
                break;
            }
        }
    }

    fn calculate_returns(&self, prices: &[WeightedPrice]) -> Vec<f64> {
        if prices.len() < 2 {
            return vec![];
        }

        prices.windows(2)
            .map(|window| {
                let (prev, curr) = (&window[0], &window[1]);
                match self.config.return_type {
                    ReturnType::LogReturns => {
                        (curr.weighted_price / prev.weighted_price).ln()
                    },
                    ReturnType::SimpleReturns => {
                        (curr.weighted_price / prev.weighted_price) - 1.0
                    },
                    ReturnType::AbsoluteReturns => {
                        (curr.weighted_price - prev.weighted_price).abs()
                    }
                }
            })
            .collect()
    }

    fn calculate_volatility_if_needed(&mut self) -> Option<VolatilityEstimate> {
        let now = Utc::now();
        if let Some(last_calc) = self.last_calculation {
            if now - last_calc < self.config.sampling_interval {
                return None;
            }
        }
        let window_data: Vec<_> = self.price_history.iter().cloned().collect();
        if window_data.len() < 2 {
            return None;
        }
        let returns = self.calculate_returns(&window_data);
        if returns.is_empty() {
            return None;
        }

        let mean_return: f64 = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance: f64 = returns.iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>() / (returns.len() - 1) as f64;

        let volatility = (variance * self.config.annualization_factor).sqrt();

        self.last_calculation = Some(now);

        Some(VolatilityEstimate {
            timestamp: now,
            volatility,
            num_observations: returns.len(),
            window_start: window_data.first().unwrap().timestamp,
            window_end: window_data.last().unwrap().timestamp,
        })
    }
}


// Default
impl Default for VolatilityConfig {
    fn default() -> Self {
        Self {
            cex_weight: 0.7,
            dex_weight: 0.3,
            rolling_window: Duration::hours(6),
            sampling_interval: Duration::minutes(1),
            annualization_factor: 525600.0, // For minute data: 365 * 24 * 60
            return_type: ReturnType::LogReturns,
        }
    }
}