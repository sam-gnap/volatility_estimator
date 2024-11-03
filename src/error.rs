use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Data source error: {0}")]
    DataSource(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Volatility calculation error: {0}")]
    Calculation(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Web3(#[from] web3::Error),

    #[error(transparent)]
    Csv(#[from] csv::Error),

    #[error("External service error: {0}")]
    External(String),
}

pub type Result<T> = std::result::Result<T, Error>;

// Helper methods for error creation
impl Error {
    pub fn data_source<T: ToString>(msg: T) -> Self {
        Error::DataSource(msg.to_string())
    }

    pub fn storage<T: ToString>(msg: T) -> Self {
        Error::Storage(msg.to_string())
    }

    pub fn calculation<T: ToString>(msg: T) -> Self {
        Error::Calculation(msg.to_string())
    }
}