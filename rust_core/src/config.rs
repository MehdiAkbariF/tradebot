// مسیر: rust_core/src/config.rs
use crate::error::Result;
use config::{Config, File};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::path::Path;

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
pub struct TradingSettings {
    pub initial_capital: Decimal,
    pub notional_per_trade: Decimal,
    pub take_profit_bps: f64,
    pub stop_loss_bps: f64,
    pub breakeven_trigger_bps: f64,
    pub max_holding_seconds: i64,
    pub max_spread_bps: f64,
    pub min_ml_confidence: f64,
    pub maker_fee_bps: f64,             // کارمزد میکر (۰.۰۱٪)
    pub min_volatility_bps: f64,        // حداقل نوسان مجاز
    pub maker_timeout_seconds: i64,     // زمان لغو اردر لیمیت پرنشده
}

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub app: AppSettings,
    pub market_data: MarketDataSettings,
    pub storage: StorageSettings,
    pub trading: TradingSettings,
}

impl Settings {
    pub fn new() -> Result<Self> {
        let config_path = if Path::new("config/default.toml").exists() {
            "config/default.toml"
        } else if Path::new("../config/default.toml").exists() {
            "../config/default.toml"
        } else {
            "config/default.toml"
        };

        let builder = Config::builder()
            .add_source(File::with_name(config_path).required(true))
            .add_source(config::Environment::default().separator("__"));

        let config = builder.build()?;
        Ok(config.try_deserialize()?)
    }
}