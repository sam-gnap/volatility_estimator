# ETH/USDC Volatility Estimator

Real-time volatility estimation matters for market making, where understanding price dynamics drives spread setting and risk management. This project implements a volatility estimator in Rust that fuses on-chain and off-chain data sources into a single view of ETH/USDC volatility.

## Technical approach

### Data sources
Websocket feeds listen to both a CEX and a DEX in real time:
- **CEX**: Kraken trade feed (ETH/USD).
- **DEX**: the most liquid Uniswap v3 pool (USDC/ETH 0.05%), via an Infura websocket.
- Both sides of the book are taken from each venue; since prices are smoothed downstream, this has little effect on the estimates.
- Latency is not modelled — a known area for improvement.

### Pipeline

#### Data collection
- Raw feeds are persisted for auditability. CSV is fine for an MVP but costly long-term; a SQL store with periodic purging would be the natural next step.
- Feeds are normalised with VWAP to 1-minute prices and combined across exchanges.
  - The blend weights CEX 70/30 over DEX, since price discovery happens mostly on the CEX. The ratio is configurable and worth re-examining.
  - A better approach could collect VWAP or dollar bars at 5-minute resolution to mitigate noise.
  - Fee differences between DEX and CEX are implicitly ignored.

#### Cleaning
- Observations are cleaned tick-by-tick, since prices may be contaminated by wrongly reported trades.
- Following Barndorff-Nielsen et al. [1], transactions with zero or negative volume are removed, extended with a (configurable) max-volume filter.
- A simplified version of Brownlees and Gallo [2] filters out ticks more than 3 standard deviations from a rolling mean.

#### Processing
- Realised volatility is computed on sliding windows over the 1-minute VWAP prices. Windows are configurable, so micro, short, and daily volatility can run side by side.
- A more complete version would resample differently per horizon: for hourly and daily frequencies, a time-weighted VWAP from 1-minute bars — as used by Coin Metrics [3] — where weights increase linearly toward the calculation time.

### Output
Realised volatility for each configured window is written to CSV.

## Limitations and future improvements
- Volatility computed from very short intervals (1 minute) is not an unbiased, consistent estimator of daily volatility: variance-reduction arguments favour high-frequency returns, while microstructure noise pushes toward lower sampling frequencies. Per-horizon resampling (above) is the fix.
- A market maker ultimately wants *expected future* volatility — implied volatility would be a highly relevant addition.
- Futures markets are worth adding; several papers suggest price discovery partly happens there.
- Liquidations appear to have predictive power for volatility [4].
- Multiple CEXs would help account for intra-day and intra-week seasonality (e.g. Coinbase follows US trading activity).
- Tests are skipped in this MVP.

### Scope assumptions
- Exchange downtime is not handled.
- Market disruptions (flash crashes) are assumed to be caught by the cleaning filters.
- No Gaussianity assumptions are made about returns.

## References

[1] Barndorff-Nielsen, O., Hansen, P. R., Lunde, A., & Shephard, N. (2009). Realized kernels in practice: trades and quotes. *Econometrics Journal*, 12, C1–C32.

[2] Brownlees, C. T., & Gallo, G. M. (2006). Financial econometric analysis at ultra-high frequency: Data handling concerns. *Computational Statistics & Data Analysis*, 51, 2232–2245.

[3] Coin Metrics. (2024). Single Asset Series Methodology. https://coinmetrics.io/wp-content/uploads/2024/10/cmbi-single-asset-methodology.pdf

[4] OECD. (2023). DeFi Liquidations Report. https://www.oecd.org/content/dam/oecd/en/publications/reports/2023/07/defi-liquidations_89cba79d/0524faaf-en.pdf

# Setup

## Environment variables
Create a `.env` file in the root directory with:
```bash
INFURA_WEBSOCKET=wss://mainnet.infura.io/ws/v3/YOUR_PROJECT_ID
UNISWAP_POOL_ADDRESS=0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640  # USDC/ETH 0.05%
```

## Installation
```bash
cargo build
```

## Configuration
The project uses a TOML configuration file at `src/config/config.toml`:
- Window sizes for volatility calculation
- Exchange weights (CEX/DEX ratio)
- Cleaning parameters
- Output paths

Default config:
```toml
[volatility]
cex_weight = 0.7
dex_weight = 0.3
windows = [
    { name = "micro", length_seconds = 300, sampling_interval = 60 },    # 5 mins
    { name = "short", length_seconds = 1800, sampling_interval = 60 },   # 30 mins
    { name = "daily", length_seconds = 86400, sampling_interval = 300 }, # 24 hrs
]

[cleaning]
mean_window = 100
std_dev_threshold = 3.0
min_volume = 0.0001
max_volume = 1000.0
```

## Running
```bash
cargo run

# Outputs CSV files in the data/ directory:
# - kraken_trades.csv
# - uniswap_trades.csv
# - volatility.csv
```

## Monitoring
Logging via `tracing`: websocket connection status, trade processing, volatility calculations, errors and warnings.
