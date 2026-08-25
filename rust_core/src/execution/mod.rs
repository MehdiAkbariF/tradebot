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
pub struct PendingMakerOrder {
    pub id: String,
    pub symbol: String,
    pub is_buy: bool,
    pub limit_price: Decimal,
    pub quantity: Decimal,
    pub notional: Decimal,
    pub placed_at: DateTime<Utc>,
    pub timeout_seconds: i64,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosedTradeRecord {
    pub id: String,
    pub symbol: String,
    pub action: String,
    pub entry_price: Decimal,
    pub exit_price: Decimal,
    pub quantity: Decimal,
    pub notional_usd: Decimal,
    pub fee_paid: Decimal,
    pub net_pnl: Decimal,
    pub pnl_percent: f64,
    pub duration_seconds: i64,
    pub exit_reason: String,
    pub opened_at: String,
    pub closed_at: String,
}

pub struct HighFrequencyScalpExecutor {
    positions: HashMap<String, ScalpPosition>,
    pending_orders: HashMap<String, PendingMakerOrder>,
    cash_balance: Decimal,
    total_margin_in_use: Decimal,
    maker_fee_ratio: Decimal,
    tp_ratio: Decimal,
    sl_ratio: Decimal,
    breakeven_ratio: Decimal,
    max_holding_sec: i64,
    max_spread_bps: f64,
    maker_timeout_sec: i64,
}

impl HighFrequencyScalpExecutor {
    pub fn new(config: &TradingSettings) -> Self {
        Self {
            positions: HashMap::new(),
            pending_orders: HashMap::new(),
            cash_balance: config.initial_capital,
            total_margin_in_use: dec!(0.0),
            maker_fee_ratio: Decimal::from_f64_retain(config.maker_fee_bps / 10000.0).unwrap_or(dec!(0.0001)),
            // تارگت سود ۱۸ پیپ (0.0018) معادل حدود ۱۴۵ دلار روی بیت‌کوین
            tp_ratio: dec!(0.0018),
            // حد ضرر ۱۴ پیپ (0.0014) معادل حدود ۱۱۰ دلار روی بیت‌کوین (کاملاً خارج از نویز تصادفی)
            sl_ratio: dec!(0.0014),
            // انتقال به بریک‌ایون پس از ۸ پیپ سود
            breakeven_ratio: dec!(0.0008),
            // مهلت باز بودن پوزیشن ۵ دقیقه
            max_holding_sec: 300,
            max_spread_bps: config.max_spread_bps,
            maker_timeout_sec: config.maker_timeout_seconds,
        }
    }

    /// کاشت سفارش لیمیت میکر در بهترین قیمت دفتر سفارشات
    pub fn place_maker_order(
        &mut self,
        symbol: &str,
        is_buy: bool,
        size: Decimal,
        metrics: &OrderBookMetrics,
    ) -> Option<PendingMakerOrder> {
        if metrics.spread_bps > self.max_spread_bps {
            return None;
        }

        if let Some(pos) = self.positions.get(symbol) {
            if pos.is_open {
                return None;
            }
        }
        if self.pending_orders.contains_key(symbol) {
            return None;
        }

        let limit_price = (if is_buy { metrics.best_bid } else { metrics.best_ask }).round_dp(2);
        if limit_price <= dec!(0.0) {
            return None;
        }

        let notional = limit_price * size;
        let order = PendingMakerOrder {
            id: uuid::Uuid::new_v4().to_string(),
            symbol: symbol.to_string(),
            is_buy,
            limit_price,
            quantity: size,
            notional,
            placed_at: Utc::now(),
            timeout_seconds: self.maker_timeout_sec,
        };

        self.pending_orders.insert(symbol.to_string(), order.clone());
        info!(symbol = %symbol, side = if is_buy { "BUY" } else { "SELL" }, price = %limit_price, "📥 Maker Limit Order PLACED (0.01% Fee Tier)");
        Some(order)
    }

    /// بررسی پر شدن اردر و محاسبه دقیق سطوح قیمتی حد سود و ضرر
    pub fn process_pending_orders_and_fills(&mut self, symbol: &str, trade_price: Decimal) -> Option<ScalpPosition> {
        let order = self.pending_orders.get(symbol)?.clone();
        let now = Utc::now();
        let age = (now - order.placed_at).num_seconds();

        let is_filled = if order.is_buy {
            trade_price <= order.limit_price
        } else {
            trade_price >= order.limit_price
        };

        if is_filled {
            self.pending_orders.remove(symbol);
            let fee = order.notional * self.maker_fee_ratio;

            let tp_price = if order.is_buy {
                (order.limit_price * (dec!(1.0) + self.tp_ratio)).round_dp(2)
            } else {
                (order.limit_price * (dec!(1.0) - self.tp_ratio)).round_dp(2)
            };

            let sl_price = if order.is_buy {
                (order.limit_price * (dec!(1.0) - self.sl_ratio)).round_dp(2)
            } else {
                (order.limit_price * (dec!(1.0) + self.sl_ratio)).round_dp(2)
            };

            let position = ScalpPosition {
                id: order.id,
                symbol: symbol.to_string(),
                quantity: if order.is_buy { order.quantity } else { -order.quantity },
                entry_price: order.limit_price,
                is_buy: order.is_buy,
                take_profit_price: tp_price,
                stop_loss_price: sl_price,
                is_breakeven_active: false,
                is_profit_locked: false,
                is_open: true,
                opened_at: Utc::now(),
                max_holding_sec: self.max_holding_sec,
            };

            self.cash_balance -= order.notional + fee;
            self.total_margin_in_use += order.notional;
            self.positions.insert(symbol.to_string(), position.clone());

            info!(
                symbol = %symbol,
                side = if order.is_buy { "BUY" } else { "SELL" },
                entry = %order.limit_price,
                tp = %tp_price,
                sl = %sl_price,
                "⚡ Maker Order FILLED! Tracking Scalp Targets..."
            );
            return Some(position);
        }

        if age >= order.timeout_seconds {
            self.pending_orders.remove(symbol);
            info!(symbol = %symbol, "⌛ Stale Maker Order CANCELLED");
        }

        None
    }

    /// ارزیابی خروج با سود و مدیریت حد ضرر متحرک (Trailing / Breakeven)
    pub fn evaluate_open_positions(
        &mut self,
        symbol: &str,
        current_bid: Decimal,
        current_ask: Decimal,
    ) -> Option<ClosedTradeRecord> {
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

        // ۱. قفل کردن ریسک در نقطه سر‌به‌سر پس از ۸ پیپ سود اولیه
        if position.is_buy && mark_price >= position.entry_price * (dec!(1.0) + self.breakeven_ratio) && !position.is_breakeven_active {
            position.stop_loss_price = (position.entry_price + (position.entry_price * self.maker_fee_ratio * dec!(2))).round_dp(2);
            position.is_breakeven_active = true;
            info!(symbol = %symbol, "🛡️ Breakeven Triggered: Zero-Risk Active");
        } else if !position.is_buy && mark_price <= position.entry_price * (dec!(1.0) - self.breakeven_ratio) && !position.is_breakeven_active {
            position.stop_loss_price = (position.entry_price - (position.entry_price * self.maker_fee_ratio * dec!(2))).round_dp(2);
            position.is_breakeven_active = true;
            info!(symbol = %symbol, "🛡️ Breakeven Triggered: Zero-Risk Active");
        }

        let hit_tp = if position.is_buy { mark_price >= position.take_profit_price } else { mark_price <= position.take_profit_price };
        let hit_sl = if position.is_buy { mark_price <= position.stop_loss_price } else { mark_price >= position.stop_loss_price };
        
        let notional_current = mark_price * position.quantity.abs();
        let exit_fee = notional_current * self.maker_fee_ratio;
        let current_raw_pnl = if position.is_buy {
            (mark_price - position.entry_price) * position.quantity.abs()
        } else {
            (position.entry_price - mark_price) * position.quantity.abs()
        };
        let current_net_pnl = current_raw_pnl - exit_fee;

        // سیو سود زودهنگام فقط در صورت کسب سود خالص بالای ۸ دلار بعد از ۱۵ ثانیه
        let early_profit_take = hold_duration >= 15 && current_net_pnl >= dec!(8.0);
        let hit_time = hold_duration >= position.max_holding_sec;

        if hit_tp || hit_sl || early_profit_take || hit_time {
            let net_pnl = (current_raw_pnl - exit_fee).round_dp(2);
            let notional_entry = position.entry_price * position.quantity.abs();
            let pnl_pct = if notional_entry > dec!(0.0) {
                let ratio = net_pnl / notional_entry;
                ratio.to_string().parse::<f64>().unwrap_or(0.0) * 100.0
            } else {
                0.0
            };

            position.is_open = false;
            self.total_margin_in_use -= position.entry_price * position.quantity.abs();
            self.cash_balance += notional_current + net_pnl;

            let total_round_trip_fee = (exit_fee + (notional_entry * self.maker_fee_ratio)).round_dp(2);

            let reason = if hit_tp { 
                "MAKER_TAKE_PROFIT 🎯" 
            } else if early_profit_take { 
                "EARLY_PROFIT_HARVEST 💰" 
            } else if hit_sl { 
                "STOP_LOSS 🛑" 
            } else { 
                "TIME_EXPIRATION ⏱️" 
            };

            let record = ClosedTradeRecord {
                id: position.id.clone(),
                symbol: symbol.to_string(),
                action: if position.is_buy { "BUY".into() } else { "SELL".into() },
                entry_price: position.entry_price,
                exit_price: mark_price,
                quantity: position.quantity.abs(),
                notional_usd: notional_entry,
                fee_paid: total_round_trip_fee,
                net_pnl,
                pnl_percent: (pnl_pct * 100.0).round() / 100.0,
                duration_seconds: hold_duration,
                exit_reason: reason.to_string(),
                opened_at: position.opened_at.to_rfc3339(),
                closed_at: now.to_rfc3339(),
            };

            info!(
                symbol = %symbol,
                reason = %reason,
                pnl = %net_pnl,
                held_sec = %hold_duration,
                "💰 Trade CLOSED (Fee: ${})", total_round_trip_fee
            );

            return Some(record);
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