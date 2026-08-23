use crate::error::Result;
use config::{Config, File};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AppSettings {
    pub environment: String,
    pub log_level: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MarketDataSettings {
    pub symbols: Vec<String>,
    pub binance_ws_url: String,
    pub binance_rest_url: String,
    pub reconnect_interval_ms: u64,
    pub channel_buffer_size: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StorageSettings {
    pub database_url: String,
    pub redis_url: String,
    pub enable_db_persistence: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub app: AppSettings,
    pub market_data: MarketDataSettings,
    pub storage: StorageSettings,
}

impl Settings {
    pub fn new() -> Result<Self> {
        let builder = Config::builder()
            .add_source(File::with_name("config/default").required(false))
            .add_source(config::Environment::default().separator("__"));

        let config = builder.build()?;
        Ok(config.try_deserialize()?)
    }
}