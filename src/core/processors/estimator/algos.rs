use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use rust_decimal::prelude::*;
use chrono::Utc;
use crate::core::domain::{Price, VolatilityEstimate};
use crate::error::{Error, Result};
use crate::config::VolatilityConfig;

pub struct VolatilityCalculator {
    config: VolatilityConfig,
}

impl VolatilityCalculator {
    pub fn new(config: VolatilityConfig) -> Self {
        Self { config }
    }

    pub fn calculate_volatility(&self, prices: &[Price]) -> Result<VolatilityEstimate> {
        if prices.len() < 2 {
            return Err(Error::calculation("Insufficient price points"));
        }

        // Calculate returns
        let returns: Vec<Decimal> = prices
            .windows(2)
            .filter_map(|window| {
                let (prev, curr) = (&window[0], &window[1]);
                if prev.value.is_zero() {
                    None
                } else {
                    // Using log returns
                    match (curr.value / prev.value).ln() {
                        Ok(ln_return) => Some(ln_return),
                        Err(_) => None,
                    }
                }
            })
            .collect();

        if returns.is_empty() {
            return Err(Error::calculation("No valid returns calculated"));
        }

        // Calculate mean return
        let mean_return: Decimal = returns.iter().sum::<Decimal>() / Decimal::from(returns.len());

        // Calculate variance
        let variance: Decimal = returns.iter()
            .map(|r| (*r - mean_return) * (*r - mean_return))
            .sum::<Decimal>() / Decimal::from(returns.len() - 1);

        // Calculate volatility and annualize it
        let annualization_factor = Decimal::try_from(self.config.annualization_factor)
            .map_err(|e| Error::calculation(format!("Invalid annualization factor: {}", e)))?;

        let volatility = match (variance.sqrt(), annualization_factor.sqrt()) {
            (Ok(var_sqrt), Ok(factor_sqrt)) => var_sqrt * factor_sqrt,
            _ => return Err(Error::calculation("Failed to calculate square root")),
        };

        Ok(VolatilityEstimate {
            timestamp: Utc::now(),
            value: volatility,
            window_start: prices.first().unwrap().timestamp,
            window_end: prices.last().unwrap().timestamp,
            num_observations: returns.len(),
        })
    }
}