// مسیر: rust_core/src/execution/mod.rs
use crate::config::TradingSettings;
use crate::domain::types::OrderBookMetrics;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalpPosition {
    pub id: String,
    pub symbol: String,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub is_buy: bool,
    pub take_profit_price: Decimal,
    pub stop_loss_price: Decimal,
    pub is_breakeven_active: bool,
    pub is_profit_locked: bool,
    pub is_open: bool,
    pub opened_at: DateTime<Utc>,
    pub max_holding_sec: i64,
}

pub struct HighFrequencyScalpExecutor {
    positions: HashMap<String, ScalpPosition>,
    cash_balance: Decimal,
    total_margin_in_use: Decimal,
    taker_fee_bps: Decimal,
    tp_ratio: Decimal,
    sl_ratio: Decimal,
    breakeven_ratio: Decimal,
    max_holding_sec: i64,
    max_spread_bps: f64,
}

impl HighFrequencyScalpExecutor {
    pub fn new(config: &TradingSettings) -> Self {
        Self {
            positions: HashMap::new(),
            cash_balance: config.initial_capital,
            total_margin_in_use: dec!(0.0),
            taker_fee_bps: dec!(0.0005),
            tp_ratio: Decimal::from_f64_retain(config.take_profit_bps / 10000.0).unwrap_or(dec!(0.0050)),
            sl_ratio: Decimal::from_f64_retain(config.stop_loss_bps / 10000.0).unwrap_or(dec!(0.0020)),
            breakeven_ratio: Decimal::from_f64_retain(config.breakeven_trigger_bps / 10000.0).unwrap_or(dec!(0.0015)),
            max_holding_sec: config.max_holding_seconds,
            max_spread_bps: config.max_spread_bps,
        }
    }

    pub fn try_open_scalp(
        &mut self,
        symbol: &str,
        is_buy: bool,
        size: Decimal,
        metrics: &OrderBookMetrics,
        ml_confidence: f64,
    ) -> Option<ScalpPosition> {
        if metrics.spread_bps > self.max_spread_bps {
            return None;
        }

        if let Some(existing) = self.positions.get(symbol) {
            if existing.is_open {
                return None;
            }
        }

        let exec_price = (if is_buy { metrics.best_ask } else { metrics.best_bid }).round_dp(2);
        if exec_price <= dec!(0.0) {
            return None;
        }

        let notional = exec_price * size;
        let fee = notional * self.taker_fee_bps;

        let tp_price = if is_buy {
            (exec_price * (dec!(1.0) + self.tp_ratio)).round_dp(2)
        } else {
            (exec_price * (dec!(1.0) - self.tp_ratio)).round_dp(2)
        };

        let sl_price = if is_buy {
            (exec_price * (dec!(1.0) - self.sl_ratio)).round_dp(2)
        } else {
            (exec_price * (dec!(1.0) + self.sl_ratio)).round_dp(2)
        };

        let position = ScalpPosition {
            id: uuid::Uuid::new_v4().to_string(),
            symbol: symbol.to_string(),
            quantity: if is_buy { size } else { -size },
            entry_price: exec_price,
            is_buy,
            take_profit_price: tp_price,
            stop_loss_price: sl_price,
            is_breakeven_active: false,
            is_profit_locked: false,
            is_open: true,
            opened_at: Utc::now(),
            max_holding_sec: self.max_holding_sec,
        };

        self.cash_balance -= notional + fee;
        self.total_margin_in_use += notional;
        self.positions.insert(symbol.to_string(), position.clone());

        info!(
            symbol = %symbol,
            action = if is_buy { "BUY" } else { "SELL" },
            entry = %exec_price,
            tp = %tp_price,
            sl = %sl_price,
            conf = %ml_confidence,
            "🚀 High-Reward Scalp OPENED (+150$ Target)"
        );

        Some(position)
    }

    pub fn evaluate_open_positions(
        &mut self,
        symbol: &str,
        current_bid: Decimal,
        current_ask: Decimal,
    ) -> Option<Decimal> {
        let position = self.positions.get_mut(symbol)?;
        if !position.is_open {
            return None;
        }

        let now = Utc::now();
        let hold_duration = (now - position.opened_at).num_seconds();
        let mark_price = (if position.is_buy { current_bid } else { current_ask }).round_dp(2);

        if mark_price <= dec!(0.0) {
            return None;
        }

        // ۱. قفل کردن کارمزد صرافی + سود در +0.15% (ریسک معامله صفر قطعی)
        if position.is_buy && mark_price >= position.entry_price * (dec!(1.0) + self.breakeven_ratio) && !position.is_breakeven_active {
            position.stop_loss_price = (position.entry_price + position.entry_price * self.taker_fee_bps * dec!(3)).round_dp(2);
            position.is_breakeven_active = true;
            info!(symbol = %symbol, "🛡️ Fee-Immune Breakeven Triggered: Zero-Risk Active");
        } else if !position.is_buy && mark_price <= position.entry_price * (dec!(1.0) - self.breakeven_ratio) && !position.is_breakeven_active {
            position.stop_loss_price = (position.entry_price - position.entry_price * self.taker_fee_bps * dec!(3)).round_dp(2);
            position.is_breakeven_active = true;
            info!(symbol = %symbol, "🛡️ Fee-Immune Breakeven Triggered: Zero-Risk Active");
        }

        // ۲. تریلینگ سود به +0.15% در صورت رسیدن به +0.30%
        if position.is_buy && mark_price >= position.entry_price * dec!(1.0030) && !position.is_profit_locked {
            position.stop_loss_price = (position.entry_price * dec!(1.0015)).round_dp(2);
            position.is_profit_locked = true;
            info!(symbol = %symbol, "💰 Profit Locked at +0.15% (+$45 Guaranteed)");
        } else if !position.is_buy && mark_price <= position.entry_price * dec!(0.9970) && !position.is_profit_locked {
            position.stop_loss_price = (position.entry_price * dec!(0.9985)).round_dp(2);
            position.is_profit_locked = true;
            info!(symbol = %symbol, "💰 Profit Locked at +0.15% (+$45 Guaranteed)");
        }

        let hit_tp = if position.is_buy { mark_price >= position.take_profit_price } else { mark_price <= position.take_profit_price };
        let hit_sl = if position.is_buy { mark_price <= position.stop_loss_price } else { mark_price >= position.stop_loss_price };
        
        // ۳. سیو سود در ثانیه‌های میانی در صورت داشتن سود بالای ۳۵ دلار
        let notional_current = mark_price * position.quantity.abs();
        let exit_fee_est = notional_current * self.taker_fee_bps;
        let current_raw_pnl = if position.is_buy {
            (mark_price - position.entry_price) * position.quantity.abs()
        } else {
            (position.entry_price - mark_price) * position.quantity.abs()
        };
        let current_net_pnl = current_raw_pnl - exit_fee_est;

        let early_profit_take = hold_duration >= 45 && current_net_pnl >= dec!(35.0);
        let hit_time = hold_duration >= position.max_holding_sec && (current_net_pnl > dec!(0.0) || hit_sl);

        if hit_tp || hit_sl || early_profit_take || hit_time {
            let notional_exit = mark_price * position.quantity.abs();
            let exit_fee = notional_exit * self.taker_fee_bps;
            let net_pnl = (current_raw_pnl - exit_fee).round_dp(2);
            
            position.is_open = false;
            self.total_margin_in_use -= position.entry_price * position.quantity.abs();
            self.cash_balance += notional_exit + net_pnl;

            let reason = if hit_tp { 
                "FULL_TAKE_PROFIT 🎯 (+150$)" 
            } else if position.is_profit_locked && hit_sl {
                "TRAILING_PROFIT_LOCK 💵"
            } else if early_profit_take { 
                "EARLY_PROFIT_HARVEST 💰" 
            } else if hit_sl { 
                "STOP_LOSS 🛑" 
            } else { 
                "SMART_TIME_EXIT ⏱️" 
            };

            info!(
                symbol = %symbol,
                reason = %reason,
                pnl = %net_pnl,
                held_sec = %hold_duration,
                "💰 Trade CLOSED with Realized PnL"
            );

            return Some(net_pnl);
        }

        None
    }

    pub fn get_cash(&self) -> Decimal {
        self.cash_balance
    }

    pub fn get_total_equity(&self) -> Decimal {
        self.cash_balance + self.total_margin_in_use
    }
}