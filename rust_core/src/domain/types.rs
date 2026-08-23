use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeTick {
    pub symbol: String,
    pub trade_id: u64,
    pub price: Decimal,
    pub quantity: Decimal,
    pub is_buyer_maker: bool,
    pub exchange_ts: DateTime<Utc>,
    pub received_ts: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthDelta {
    pub symbol: String,
    pub first_update_id: u64,
    pub final_update_id: u64,
    pub bids: Vec<(Decimal, Decimal)>, // (Price, Size)
    pub asks: Vec<(Decimal, Decimal)>,
    pub exchange_ts: DateTime<Utc>,
    pub received_ts: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookMetrics {
    pub symbol: String,
    pub best_bid: Decimal,
    pub best_ask: Decimal,
    pub spread_bps: f64,
    pub mid_price: Decimal,
    pub micro_price: Decimal,
    pub imbalance_top10: f64,
    pub timestamp: DateTime<Utc>,
}