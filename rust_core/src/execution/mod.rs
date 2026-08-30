// مسیر: rust_core/src/execution/mod.rs
use crate::config::TradingSettings;
use crate::domain::types::{CanonicalSignalPayload, ExecutionState, OrderBookMetrics, OrderSide};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionOrder {
    pub order_id: String,
    pub signal_id: String,
    pub symbol: String,
    pub side: OrderSide,
    pub limit_price: Decimal,
    pub quantity: Decimal,
    pub notional: Decimal,
    pub tp_bps: f64,
    pub sl_bps: f64,
    pub state: ExecutionState,
    pub created_at: DateTime<Utc>,
    pub timeout_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub position_id: String,
    pub signal_id: String,
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub target_tp_price: Decimal,
    pub target_sl_price: Decimal,
    pub breakeven_price: Decimal,
    pub is_breakeven_active: bool,
    pub margin_allocated: Decimal,
    pub entry_fee: Decimal,
    pub opened_at: DateTime<Utc>,
    pub max_holding_sec: i64,
    pub max_price_seen: Decimal,
    pub min_price_seen: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedTradeRecord {
    pub trade_id: String,
    pub signal_id: String,
    pub symbol: String,
    pub side: String,
    pub entry_price: Decimal,
    pub exit_price: Decimal,
    pub quantity: Decimal,
    pub notional_usd: Decimal,
    pub margin_allocated: Decimal,
    pub total_fee: Decimal,
    pub gross_pnl: Decimal,
    pub net_pnl: Decimal,
    pub pnl_bps: f64,
    pub mfe_bps: f64,
    pub mae_bps: f64,
    pub latency_rtt_ms: i64,
    pub exit_reason: String,
    pub opened_at: String,
    pub closed_at: String,
}

pub struct ExecutionEngine {
    pub active_positions: HashMap<String, Position>,
    pub pending_orders: HashMap<String, ExecutionOrder>,
    pub cash_balance: Decimal,
    pub total_margin_in_use: Decimal,
    maker_fee_rate: Decimal,
    taker_fee_rate: Decimal,
    min_ev_hurdle_bps: f64,
}

impl ExecutionEngine {
    pub fn new(cfg: &TradingSettings) -> Self {
        Self {
            active_positions: HashMap::new(),
            pending_orders: HashMap::new(),
            cash_balance: cfg.initial_capital,
            total_margin_in_use: dec!(0.0),
            maker_fee_rate: dec!(0.0002),
            taker_fee_rate: dec!(0.0005),
            min_ev_hurdle_bps: 1.2,
        }
    }

    pub fn handle_signal(&mut self, sig: &CanonicalSignalPayload, metrics: &OrderBookMetrics) -> Option<ExecutionOrder> {
        if sig.expected_net_ev_bps < self.min_ev_hurdle_bps { return None; }
        if self.active_positions.contains_key(&sig.symbol) || self.pending_orders.contains_key(&sig.symbol) {
            return None;
        }

        let is_buy = sig.action == "BUY";
        let side = if is_buy { OrderSide::Buy } else { OrderSide::Sell };
        let limit_price = if is_buy { metrics.best_bid } else { metrics.best_ask };

        let max_alloc = self.cash_balance * dec!(0.10);
        if max_alloc < dec!(10.0) { return None; }
        
        let quantity = (max_alloc / limit_price).round_dp(4);
        let notional = limit_price * quantity;

        let tp_bps = sig.tp_bps.unwrap_or(8.0);
        let sl_bps = sig.sl_bps.unwrap_or(5.0);

        let order = ExecutionOrder {
            order_id: uuid::Uuid::new_v4().to_string(),
            signal_id: sig.signal_id.clone(),
            symbol: sig.symbol.clone(),
            side,
            limit_price,
            quantity,
            notional,
            tp_bps,
            sl_bps,
            state: ExecutionState::Submitted,
            created_at: Utc::now(),
            timeout_seconds:10 ,
        };

        self.pending_orders.insert(sig.symbol.clone(), order.clone());
        Some(order)
    }

    pub fn on_market_tick(&mut self, symbol: &str, current_price: Decimal, current_micro_price: Decimal) -> Option<Position> {
        let order = self.pending_orders.get_mut(symbol)?;

        let is_toxic = match order.side {
            OrderSide::Buy => current_micro_price < order.limit_price - dec!(0.5),
            OrderSide::Sell => current_micro_price > order.limit_price + dec!(0.5),
        };

        if is_toxic {
            self.pending_orders.remove(symbol);
            return None;
        }

        let is_filled = match order.side {
            OrderSide::Buy => current_price <= order.limit_price,
            OrderSide::Sell => current_price >= order.limit_price,
        };

        if is_filled {
            let filled_order = self.pending_orders.remove(symbol).unwrap();
            let entry_fee = filled_order.notional * self.maker_fee_rate;

            let tp_ratio = Decimal::from_f64_retain(filled_order.tp_bps / 10000.0).unwrap_or(dec!(0.0008));
            let sl_ratio = Decimal::from_f64_retain(filled_order.sl_bps / 10000.0).unwrap_or(dec!(0.0005));

            let (tp, sl) = match filled_order.side {
                OrderSide::Buy => (
                    (filled_order.limit_price * (dec!(1.0) + tp_ratio)).round_dp(2),
                    (filled_order.limit_price * (dec!(1.0) - sl_ratio)).round_dp(2)
                ),
                OrderSide::Sell => (
                    (filled_order.limit_price * (dec!(1.0) - tp_ratio)).round_dp(2),
                    (filled_order.limit_price * (dec!(1.0) + sl_ratio)).round_dp(2)
                ),
            };

            let pos = Position {
                position_id: uuid::Uuid::new_v4().to_string(),
                signal_id: filled_order.signal_id,
                symbol: symbol.to_string(),
                side: filled_order.side,
                quantity: filled_order.quantity,
                entry_price: filled_order.limit_price,
                target_tp_price: tp,
                target_sl_price: sl,
                breakeven_price: filled_order.limit_price,
                is_breakeven_active: false,
                margin_allocated: filled_order.notional,
                entry_fee,
                opened_at: Utc::now(),
                max_holding_sec: 300,
                max_price_seen: filled_order.limit_price,
                min_price_seen: filled_order.limit_price,
            };

            self.cash_balance -= entry_fee;
            self.total_margin_in_use += pos.margin_allocated;
            self.active_positions.insert(symbol.to_string(), pos.clone());
            return Some(pos);
        }

        if (Utc::now() - order.created_at).num_seconds() > order.timeout_seconds {
            self.pending_orders.remove(symbol);
        }

        None
    }

    pub fn evaluate_exit(&mut self, symbol: &str, best_bid: Decimal, best_ask: Decimal) -> Option<VerifiedTradeRecord> {
        let pos = self.active_positions.get_mut(symbol)?;
        let mark_price = match pos.side {
            OrderSide::Buy => best_bid,
            OrderSide::Sell => best_ask,
        };

        if mark_price > pos.max_price_seen { pos.max_price_seen = mark_price; }
        if mark_price < pos.min_price_seen { pos.min_price_seen = mark_price; }

        let now = Utc::now();
        let hold_duration = (now - pos.opened_at).num_seconds();

        let hit_tp = match pos.side {
            OrderSide::Buy => mark_price >= pos.target_tp_price,
            OrderSide::Sell => mark_price <= pos.target_tp_price,
        };
        let hit_sl = match pos.side {
            OrderSide::Buy => mark_price <= pos.target_sl_price,
            OrderSide::Sell => mark_price >= pos.target_sl_price,
        };
        let hit_to = hold_duration >= pos.max_holding_sec;

        if hit_tp || hit_sl || hit_to {
            let exit_fee = (mark_price * pos.quantity) * self.taker_fee_rate;
            let gross_pnl = match pos.side {
                OrderSide::Buy => (mark_price - pos.entry_price) * pos.quantity,
                OrderSide::Sell => (pos.entry_price - mark_price) * pos.quantity,
            };
            let total_fee = pos.entry_fee + exit_fee;
            let net_pnl = gross_pnl - total_fee;

            let pnl_bps = ((net_pnl / pos.margin_allocated).to_string().parse::<f64>().unwrap_or(0.0)) * 10000.0;

            let (mfe_bps, mae_bps) = match pos.side {
                OrderSide::Buy => {
                    let mfe = ((pos.max_price_seen - pos.entry_price) / pos.entry_price).to_string().parse::<f64>().unwrap_or(0.0) * 10000.0;
                    let mae = ((pos.entry_price - pos.min_price_seen) / pos.entry_price).to_string().parse::<f64>().unwrap_or(0.0) * 10000.0;
                    (mfe.max(0.0), mae.max(0.0))
                }
                OrderSide::Sell => {
                    let mfe = ((pos.entry_price - pos.min_price_seen) / pos.entry_price).to_string().parse::<f64>().unwrap_or(0.0) * 10000.0;
                    let mae = ((pos.max_price_seen - pos.entry_price) / pos.entry_price).to_string().parse::<f64>().unwrap_or(0.0) * 10000.0;
                    (mfe.max(0.0), mae.max(0.0))
                }
            };

            self.cash_balance += net_pnl;
            self.total_margin_in_use -= pos.margin_allocated;

            let exit_reason = if hit_tp { "DYNAMIC_TP_HIT 🎯" } else if hit_sl { "STOP_LOSS 🛑" } else { "TIME_EXPIRATION ⏱️" };

            let record = VerifiedTradeRecord {
                trade_id: uuid::Uuid::new_v4().to_string(),
                signal_id: pos.signal_id.clone(),
                symbol: symbol.to_string(),
                side: format!("{:?}", pos.side),
                entry_price: pos.entry_price,
                exit_price: mark_price,
                quantity: pos.quantity,
                notional_usd: pos.margin_allocated,
                margin_allocated: pos.margin_allocated,
                total_fee,
                gross_pnl,
                net_pnl,
                pnl_bps,
                mfe_bps,
                mae_bps,
                latency_rtt_ms: (now - pos.opened_at).num_milliseconds(),
                exit_reason: exit_reason.to_string(),
                opened_at: pos.opened_at.to_rfc3339(),
                closed_at: now.to_rfc3339(),
            };

            self.active_positions.remove(symbol);
            return Some(record);
        }

        None
    }
}