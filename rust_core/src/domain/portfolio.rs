use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use super::types::OrderSide;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SignalAction {
    Buy,
    Sell,
    Hold,
    NoTrade,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalCandidate {
    pub signal_id: String,
    pub event_id: Option<String>,
    pub symbol: String,
    pub action: SignalAction,
    pub confidence: f64,
    pub expected_horizon_sec: u32,
    pub expected_return: f64,
    pub decision_ts: DateTime<Utc>,
    pub reason_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderIntent {
    pub order_id: String,
    pub signal_id: String,
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: Decimal,
    pub limit_price: Option<Decimal>,
    pub submitted_ts: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionState {
    pub symbol: String,
    pub current_qty: Decimal,
    pub avg_entry_price: Decimal,
    pub realized_pnl: Decimal,
    pub unrealized_pnl: Decimal,
    pub is_open: bool,
    pub updated_ts: DateTime<Utc>,
}