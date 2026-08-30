// مسیر: rust_core/src/domain/types.rs
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionState {
    New,
    Submitted,
    Acknowledged,
    PartiallyFilled,
    Filled,
    Canceled,
    Rejected,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrosecondAudit {
    pub exchange_ts: DateTime<Utc>,
    pub receive_ts: DateTime<Utc>,
    pub process_ts: DateTime<Utc>,
    pub decision_ts: Option<DateTime<Utc>>,
    pub submit_ts: Option<DateTime<Utc>>,
    pub fill_ts: Option<DateTime<Utc>>,
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
    pub audit: MicrosecondAudit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthDelta {
    pub symbol: String,
    pub first_update_id: u64,
    pub final_update_id: u64,
    pub bids: Vec<(Decimal, Decimal)>,
    pub asks: Vec<(Decimal, Decimal)>,
    pub exchange_ts: DateTime<Utc>,
    pub received_ts: DateTime<Utc>,
    pub audit: MicrosecondAudit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookMetrics {
    pub symbol: String,
    pub best_bid: Decimal,
    pub best_ask: Decimal,
    pub spread_bps: f64,
    pub mid_price: Decimal,
    pub micro_price: Decimal,
    pub ofi: f64,
    pub ofi_zscore: f64,
    pub book_imbalance_top10: f64,
    pub imbalance_top10: f64,
    pub realized_vol_60s_bps: f64,
    pub timestamp: DateTime<Utc>,
    pub audit: MicrosecondAudit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalSignalPayload {
    pub signal_id: String,
    pub strategy_version: String,
    pub model_version: String,
    pub symbol: String,
    pub action: String,
    pub expected_net_ev_bps: f64,
    pub p_tp: f64,
    pub p_sl: f64,
    pub p_timeout: f64,
    pub tp_bps: Option<f64>,  // تارگت سود داینامیک
    pub sl_bps: Option<f64>,  // حد ضرر داینامیک
    pub friction_bps: f64,
    pub signal_price: Decimal,
    pub timestamp: DateTime<Utc>,
}