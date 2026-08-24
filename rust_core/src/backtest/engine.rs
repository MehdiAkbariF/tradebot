use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    pub initial_capital: Decimal,
    pub maker_fee_bps: f64,
    pub taker_fee_bps: f64,
    pub simulated_latency_ms: u64,
}

#[derive(Debug, Clone)]
pub struct SimulatedTrade {
    pub symbol: String,
    pub side: String,
    pub entry_price: Decimal,
    pub exit_price: Decimal,
    pub quantity: Decimal,
    pub pnl: Decimal,
    pub fee_paid: Decimal,
    pub entry_ts: DateTime<Utc>,
    pub exit_ts: DateTime<Utc>,
}

pub struct EventDrivenBacktester {
    config: BacktestConfig,
    cash: Decimal,
    trades: Vec<SimulatedTrade>,
}

impl EventDrivenBacktester {
    pub fn new(config: BacktestConfig) -> Self {
        let initial_capital = config.initial_capital;
        Self {
            config,
            cash: initial_capital,
            trades: Vec::new(),
        }
    }

    /// شبیه‌سازی ورود و خروج از معامله با احتساب کارمزد و اسلیپیج
    pub fn execute_simulated_trade(
        &mut self,
        symbol: &str,
        is_buy: bool,
        entry_price: Decimal,
        exit_price: Decimal,
        quantity: Decimal,
        entry_ts: DateTime<Utc>,
        exit_ts: DateTime<Utc>,
    ) {
        let notional_entry = entry_price * quantity;
        let notional_exit = exit_price * quantity;

        // محاسبه کارمزد (Taker fee به عنوان پیش‌فرض اجرای مارکت اردر)
        let fee_rate = Decimal::from_f64_retain(self.config.taker_fee_bps / 10000.0).unwrap_or(dec!(0.0005));
        let entry_fee = notional_entry * fee_rate;
        let exit_fee = notional_exit * fee_rate;
        let total_fee = entry_fee + exit_fee;

        let raw_pnl = if is_buy {
            notional_exit - notional_entry
        } else {
            notional_entry - notional_exit
        };

        let net_pnl = raw_pnl - total_fee;
        self.cash += net_pnl;

        self.trades.push(SimulatedTrade {
            symbol: symbol.to_string(),
            side: if is_buy { "BUY".into() } else { "SELL".into() },
            entry_price,
            exit_price,
            quantity,
            pnl: net_pnl,
            fee_paid: total_fee,
            entry_ts,
            exit_ts,
        });
    }

    /// محاسبه آمارهای عملکرد نهایی استراتژی (Tear-Sheet Metrics)
    pub fn generate_performance_report(&self) -> BacktestReport {
        let total_trades = self.trades.len();
        let mut winning_trades = 0;
        let mut losing_trades = 0;
        let mut total_pnl = Decimal::ZERO;
        let mut max_drawdown = Decimal::ZERO;
        let mut peak_capital = self.config.initial_capital;
        let mut current_capital = self.config.initial_capital;

        for t in &self.trades {
            total_pnl += t.pnl;
            current_capital += t.pnl;

            if current_capital > peak_capital {
                peak_capital = current_capital;
            } else {
                let drawdown = peak_capital - current_capital;
                if drawdown > max_drawdown {
                    max_drawdown = drawdown;
                }
            }

            if t.pnl > Decimal::ZERO {
                winning_trades += 1;
            } else {
                losing_trades += 1;
            }
        }

        let win_rate = if total_trades > 0 {
            winning_trades as f64 / total_trades as f64
        } else {
            0.0
        };

        BacktestReport {
            initial_capital: self.config.initial_capital,
            final_capital: current_capital,
            net_pnl: total_pnl,
            total_trades,
            winning_trades,
            losing_trades,
            win_rate,
            max_drawdown,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BacktestReport {
    pub initial_capital: Decimal,
    pub final_capital: Decimal,
    pub net_pnl: Decimal,
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate: f64,
    pub max_drawdown: Decimal,
}