use crate::domain::portfolio::{SignalAction, SignalCandidate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    pub max_position_size: Decimal,
    pub max_open_positions: usize,
    pub max_daily_loss: Decimal,
    pub max_spread_bps: f64,
    pub kill_switch_active: bool,
}

pub enum RiskCheckResult {
    Approved { size: Decimal },
    Rejected { reason: String },
}

pub struct RiskEngine {
    config: RiskConfig,
    current_daily_pnl: Decimal,
    open_positions_count: usize,
}

impl RiskEngine {
    pub fn new(config: RiskConfig) -> Self {
        Self {
            config,
            current_daily_pnl: Decimal::ZERO,
            open_positions_count: 0,
        }
    }

    /// به‌روزرسانی وضعیت سود و زیان روزانه و تعداد پوزیشن‌ها
    pub fn update_state(&mut self, daily_pnl: Decimal, open_count: usize) {
        self.current_daily_pnl = daily_pnl;
        self.open_positions_count = open_count;
    }

    /// ارزیابی نهایی سیگنال توسط موتور ریسک قبل از اجرا
    pub fn validate_signal(
        &self,
        signal: &SignalCandidate,
        current_spread_bps: f64,
    ) -> RiskCheckResult {
        // 1. Kill-Switch Check
        if self.config.kill_switch_active {
            return RiskCheckResult::Rejected {
                reason: "KILL_SWITCH_ACTIVE".into(),
            };
        }

        // 2. Daily Loss Limit Check
        if self.current_daily_pnl <= -self.config.max_daily_loss {
            return RiskCheckResult::Rejected {
                reason: "MAX_DAILY_LOSS_EXCEEDED".into(),
            };
        }

        // 3. No Trade Action Check
        if signal.action == SignalAction::NoTrade || signal.action == SignalAction::Hold {
            return RiskCheckResult::Rejected {
                reason: "SIGNAL_IS_NO_TRADE".into(),
            };
        }

        // 4. Spread Friction Check
        if current_spread_bps > self.config.max_spread_bps {
            return RiskCheckResult::Rejected {
                reason: "EXCESSIVE_SPREAD_RISK".into(),
            };
        }

        // 5. Max Open Positions Check
        if self.open_positions_count >= self.config.max_open_positions {
            return RiskCheckResult::Rejected {
                reason: "MAX_OPEN_POSITIONS_REACHED".into(),
            };
        }

        // All checks passed. Approve base position size.
        RiskCheckResult::Approved {
            size: self.config.max_position_size,
        }
    }
}