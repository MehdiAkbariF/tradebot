// مسیر: rust_core/src/execution/mod.rs
use crate::config::TradingSettings;
use crate::domain::types::OrderBookMetrics;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicRiskConfig {
    pub total_capital: Decimal,
    pub allocation_pct: Decimal, // تخصیص سرمایه در هر معامله
    pub leverage: Decimal,       // ضریب لوریج
    pub min_notional_guard: Decimal, // حداقل اردر مجاز صرافی (5 دلار)
    pub kill_switch: bool,
}

pub struct HighFrequencyScalpExecutor {
    positions: HashMap<String, ScalpPosition>,
    pending_orders: HashMap<String, PendingMakerOrder>,
    pub risk_config: DynamicRiskConfig,
    pub cash_balance: Decimal,
    pub total_margin_in_use: Decimal,
    maker_fee_ratio: Decimal,
    tp_ratio: Decimal,
    sl_ratio: Decimal,
    breakeven_ratio: Decimal,
    early_harvest_ratio: Decimal,
    max_holding_sec: i64,
    max_spread_bps: f64,
    maker_timeout_sec: i64,
}

impl HighFrequencyScalpExecutor {
    pub fn new(config: &TradingSettings) -> Self {
        let initial_cap = config.initial_capital;
        Self {
            positions: HashMap::new(),
            pending_orders: HashMap::new(),
            risk_config: DynamicRiskConfig {
                total_capital: initial_cap,
                allocation_pct: dec!(0.30),
                leverage: dec!(1.0),
                min_notional_guard: dec!(5.0),
                kill_switch: false,
            },
            cash_balance: initial_cap,
            total_margin_in_use: dec!(0.0),
            maker_fee_ratio: Decimal::from_f64_retain(config.maker_fee_bps / 10000.0).unwrap_or(dec!(0.0001)),
            tp_ratio: dec!(0.0018),            // 18 bps حد سود کامل
            sl_ratio: dec!(0.0014),            // 14 bps حد ضرر ایمن
            breakeven_ratio: dec!(0.0008),     // 8 bps فعال‌سازی ریسک‌فری
            early_harvest_ratio: dec!(0.0010), // افزایش به 10 bps (0.10%) برای ۲ برابر شدن سود معاملات برنده
            max_holding_sec: 300,
            max_spread_bps: config.max_spread_bps,
            maker_timeout_sec: config.maker_timeout_seconds,
        }
    }

    /// به‌روزرسانی داینامیک تنظیمات سرمایه و ریسک
    pub fn update_risk_config(&mut self, new_cap: Option<Decimal>, alloc_pct: Option<Decimal>, lev: Option<Decimal>, kill: Option<bool>) {
        if let Some(c) = new_cap {
            self.risk_config.total_capital = c;
            self.cash_balance = c;
            info!(capital = %c, "💵 Capital Balance Updated");
        }
        if let Some(a) = alloc_pct {
            self.risk_config.allocation_pct = a;
            info!(allocation = %a, "⚖️ Allocation % Updated");
        }
        if let Some(l) = lev {
            self.risk_config.leverage = l;
            info!(leverage = %l, "⚡ Leverage Updated");
        }
        if let Some(k) = kill {
            self.risk_config.kill_switch = k;
            if k {
                warn!("🚨 EMERGENCY KILL-SWITCH ACTIVATED!");
            }
        }
    }

    /// محاسبه داینامیک حجم بر اساس موجودی نقد آزاد (رفع قطعی باگ اردرهای منفی)
    pub fn calculate_dynamic_notional(&self) -> Option<Decimal> {
        if self.risk_config.kill_switch {
            return None;
        }

        let free_cash = self.cash_balance.max(dec!(0.0));
        let notional = (free_cash * self.risk_config.allocation_pct * self.risk_config.leverage).round_dp(2);

        if notional < self.risk_config.min_notional_guard {
            return None;
        }

        Some(notional)
    }

    /// کاشت سفارش لیمیت میکر
    pub fn place_maker_order(
        &mut self,
        symbol: &str,
        is_buy: bool,
        metrics: &OrderBookMetrics,
    ) -> Option<PendingMakerOrder> {
        if metrics.spread_bps > self.max_spread_bps || self.risk_config.kill_switch {
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

        let notional = self.calculate_dynamic_notional()?;
        let limit_price = (if is_buy { metrics.best_bid } else { metrics.best_ask }).round_dp(2);
        if limit_price <= dec!(0.0) {
            return None;
        }

        let size = (notional / limit_price).round_dp(4);

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
        info!(
            symbol = %symbol,
            side = if is_buy { "BUY" } else { "SELL" },
            price = %limit_price,
            notional = %notional,
            "📥 Dynamic Maker Order PLACED"
        );
        Some(order)
    }

    /// بررسی پر شدن سفارش و محاسبه تارگت‌ها
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
                notional = %order.notional,
                "⚡ Scalp Position FILLED"
            );
            return Some(position);
        }

        if age >= order.timeout_seconds {
            self.pending_orders.remove(symbol);
            info!(symbol = %symbol, "⌛ Stale Maker Order CANCELLED");
        }

        None
    }

    /// ارزیابی خروج با سود و مدیریت بریک‌ایون
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

        // ۱. انتقال حد ضرر به نقطه سر‌به‌سر پس از ۸ پیپ سود
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
        let notional_entry = position.entry_price * position.quantity.abs();
        let exit_fee = notional_current * self.maker_fee_ratio;
        
        let current_raw_pnl = if position.is_buy {
            (mark_price - position.entry_price) * position.quantity.abs()
        } else {
            (position.entry_price - mark_price) * position.quantity.abs()
        };
        let current_net_pnl = current_raw_pnl - exit_fee;

        // سیو سود در ۱۰ پیپ سود خالص (افزایش سود هر ترید برنده)
        let dynamic_profit_harvest_target = notional_entry * self.early_harvest_ratio;
        let early_profit_take = hold_duration >= 15 && current_net_pnl >= dynamic_profit_harvest_target;
        let hit_time = hold_duration >= position.max_holding_sec;

        if hit_tp || hit_sl || early_profit_take || hit_time {
            let net_pnl = current_net_pnl.round_dp(2);
            let pnl_pct = if notional_entry > dec!(0.0) {
                let ratio = net_pnl / notional_entry;
                ratio.to_string().parse::<f64>().unwrap_or(0.0) * 100.0
            } else {
                0.0
            };

            position.is_open = false;
            self.total_margin_in_use -= notional_entry;
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
                "💰 Scalp CLOSED (PnL: ${})", net_pnl
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