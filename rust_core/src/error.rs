use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("Network/WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Serialization/Deserialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Market Data Protocol error: {0}")]
    Protocol(String),

    #[error("OrderBook sequence gap detected on {symbol}: expected {expected}, got {received}")]
    SequenceGap {
        symbol: String,
        expected: u64,
        received: u64,
    },
}

pub type Result<T> = std::result::Result<T, AppError>;