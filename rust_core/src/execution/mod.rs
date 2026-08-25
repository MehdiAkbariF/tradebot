use crate::domain::portfolio::{SignalAction, SignalCandidate};
use crate::domain::types::OrderBookMetrics;
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

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
    pub is_open: bool,
    pub opened_at: DateTime<Utc>,
    pub max_holding_sec: i64,
}

pub struct HighFrequencyScalpExecutor {
    positions: HashMap<String, ScalpPosition>,
    cash_balance: Decimal,
    maker_fee_bps: Decimal,
    taker_fee_bps: Decimal,
    tp_bps: Decimal,
    sl_bps: Decimal,
    max_holding_sec: i64,
}

impl HighFrequencyScalpExecutor {
    pub fn new(initial_capital: Decimal) -> Self {
        Self {
            positions: HashMap::new(),
            cash_balance: initial_capital,
            maker_fee_bps: dec!(0.0002), // 2 bps
            taker_fee_bps: dec!(0.0005), // 5 bps
            tp_bps: dec!(0.0020),        // +0.20% Target
            sl_bps: dec!(0.0012),        // -0.12% Hard Stop
            max_holding_sec: 45,         // 45s Time Barrier Exit
        }
    }

    /// ماشه ورود فوق‌سریع پس از اعتبارسنجی اسکلپ
    pub fn try_open_scalp(
        &mut self,
        symbol: &str,
        is_buy: bool,
        size: Decimal,
        metrics: &OrderBookMetrics,
        ml_confidence: f64,
    ) -> Option<ScalpPosition> {
        // ۱. فیلتر اصطکاک اسپرد (اسپرد بیشتر از ۱.۲ bps معامله نمی‌شود)
        if metrics.spread_bps > 1.2 {
            warn!(symbol = %symbol, spread = %metrics.spread_bps, "Scalp entry rejected: Spread too wide");
            return None;
        }

        // ۲. تاییدیه عدم تعادل دفتر سفارشات (OFI Validation)
        if is_buy && metrics.imbalance_top10 < 0.10 {
            warn!("Scalp Long rejected: L2 Orderbook imbalance not bullish");
            return None;
        }

        // ۳. بررسی نبود پوزیشن باز همزمان روی همین نماد
        if let Some(existing) = self.positions.get(symbol) {
            if existing.is_open {
                return None;
            }
        }

        let exec_price = if is_buy { metrics.best_ask } else { metrics.best_bid };
        let notional = exec_price * size;
        let fee = notional * self.taker_fee_bps;

        let tp_price = if is_buy {
            exec_price * (dec!(1.0) + self.tp_bps)
        } else {
            exec_price * (dec!(1.0) - self.tp_bps)
        };

        let sl_price = if is_buy {
            exec_price * (dec!(1.0) - self.sl_bps)
        } else {
            exec_price * (dec!(1.0) + self.sl_bps)
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
            is_open: true,
            opened_at: Utc::now(),
            max_holding_sec: self.max_holding_sec,
        };

        self.cash_balance -= (notional + fee);
        self.positions.insert(symbol.to_string(), position.clone());

        info!(
            symbol = %symbol,
            entry = %exec_price,
            tp = %tp_price,
            sl = %sl_price,
            conf = %ml_confidence,
            "🚀 High-Frequency Scalp Position OPENED"
        );

        Some(position)
    }

    /// بررسی تیک به تیک پوزیشن برای اعمال حد سود، تریلینگ و خروج زمانی
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
        let mark_price = if position.is_buy { current_bid } else { current_ask };

        // ۱. فعال‌سازی Breakeven در صورت رسیدن به نیمی از تارگت (+0.10%)
        if position.is_buy && mark_price >= position.entry_price * dec!(1.0010) && !position.is_breakeven_active {
            position.stop_loss_price = position.entry_price + (position.entry_price * self.taker_fee_bps * dec!(2));
            position.is_breakeven_active = true;
            info!(symbol = %symbol, "Trailing Stop moved to BREAKEVEN + Fees");
        }

        // ۲. شرط خروج با حد سود (Take Profit)
        let hit_tp = if position.is_buy { mark_price >= position.take_profit_price } else { mark_price <= position.take_profit_price };

        // ۳. شرط خروج با حد ضرر (Stop Loss)
        let hit_sl = if position.is_buy { mark_price <= position.stop_loss_price } else { mark_price >= position.stop_loss_price };

        // ۴. شرط خروج با مانع زمانی (Time Barrier Exit)
        let hit_time_barrier = hold_duration >= position.max_holding_sec;

        if hit_tp || hit_sl || hit_time_barrier {
            let notional_exit = mark_price * position.quantity.abs();
            let exit_fee = notional_exit * self.taker_fee_bps;
            
            let raw_pnl = if position.is_buy {
                (mark_price - position.entry_price) * position.quantity.abs()
            } else {
                (position.entry_price - mark_price) * position.quantity.abs()
            };

            let net_pnl = raw_pnl - exit_fee;
            position.is_open = false;
            self.cash_balance += (notional_exit + net_pnl);

            let reason = if hit_tp { "TAKE_PROFIT" } else if hit_sl { "STOP_LOSS" } else { "TIME_EXPIRATION" };
            info!(
                symbol = %symbol,
                reason = %reason,
                pnl = %net_pnl,
                held_sec = %hold_duration,
                "🛑 Scalp Position CLOSED"
            );

            return Some(net_pnl);
        }

        None
    }

    pub fn get_cash(&self) -> Decimal {
        self.cash_balance
    }
}