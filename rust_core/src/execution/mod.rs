use crate::domain::portfolio::{SignalCandidate, SignalAction};
use crate::domain::types::OrderBookMetrics;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub realized_pnl: Decimal,
    pub is_open: bool,
    pub opened_at: DateTime<Utc>,
}

pub struct ExecutionSimulator {
    maker_fee_bps: f64,
    taker_fee_bps: f64,
    positions: std::collections::HashMap<String, Position>,
    cash_balance: Decimal,
}

impl ExecutionSimulator {
    pub fn new(initial_capital: Decimal, maker_fee_bps: f64, taker_fee_bps: f64) -> Self {
        Self {
            maker_fee_bps,
            taker_fee_bps,
            positions: std::collections::HashMap::new(),
            cash_balance: initial_capital,
        }
    }

    /// شبیه‌سازی اجرای دستور معاملاتی بر اساس وضعیت زنده اردر بوک
    pub fn process_signal(
        &mut self,
        signal: &SignalCandidate,
        metrics: &OrderBookMetrics,
        target_size: Decimal,
    ) -> Option<Position> {
        let symbol = &signal.symbol;
        let is_buy = signal.action == SignalAction::Buy;
        let is_sell = signal.action == SignalAction::Sell;

        if !is_buy && !is_sell {
            return None; // NO_TRADE or HOLD -> Do nothing
        }

        // تعیین قیمت اجرای سفارش با احتساب اسلیپیج ساده بر اساس بهترین قیمت bid/ask
        let execution_price = if is_buy {
            metrics.best_ask // خرید روی قیمت Ask انجام می‌شود
        } else {
            metrics.best_bid // فروش روی قیمت Bid انجام می‌شود
        };

        let notional = execution_price * target_size;
        let fee_rate = Decimal::from_f64_retain(self.taker_fee_bps / 10000.0).unwrap_or(dec!(0.0005));
        let fee = notional * fee_rate;

        // بررسی پوزیشن موجود
        let existing_position = self.positions.get_mut(symbol);

        if let Some(pos) = existing_position {
            if pos.is_open {
                // اگر پوزیشنی از قبل باز است و جهت سیگنال مخالف است، پوزیشن بسته می‌شود (Close Position)
                let closing_buy = pos.quantity < Decimal::ZERO;
                if closing_buy == is_buy {
                    // محاسبه سود و زیان محقق شده (Realized PnL)
                    let pnl = if closing_buy {
                        (pos.entry_price - execution_price) * pos.quantity.abs() - fee
                    } else {
                        (execution_price - pos.entry_price) * pos.quantity.abs() - fee
                    };

                    pos.realized_pnl += pnl;
                    pos.is_open = false;
                    pos.quantity = Decimal::ZERO;
                    self.cash_balance += notional + pnl;
                    return Some(pos.clone());
                }
            }
        }

        // باز کردن پوزیشن جدید (Open Position)
        let signed_qty = if is_buy { target_size } else { -target_size };
        let new_position = Position {
            symbol: symbol.clone(),
            quantity: signed_qty,
            entry_price: execution_price,
            realized_pnl: Decimal::ZERO,
            is_open: true,
            opened_at: Utc::now(),
        };

        self.cash_balance -= notional + fee;
        self.positions.insert(symbol.clone(), new_position.clone());

        Some(new_position)
    }

    pub fn get_cash_balance(&self) -> Decimal {
        self.cash_balance
    }
}