use rust_decimal::Decimal;
use rust_decimal_macros::dec;

#[derive(Debug, Clone, PartialEq)]
pub enum MarketRegime {
    BullishTrend,
    BearishTrend,
    HighVolatilityRange,
    LowVolatilityConsolidation,
}

pub struct RegimeDetector {
    volatility_threshold: f64,
}

impl RegimeDetector {
    pub fn new(volatility_threshold: f64) -> Self {
        Self { volatility_threshold }
    }

    /// تشخیص رژیم بازار بر اساس تغییرات قیمت و بازه نوسان (ATR / Returns)
    pub fn detect_regime(&self, returns: &[f64], current_spread_bps: f64) -> MarketRegime {
        if returns.is_empty() {
            return MarketRegime::LowVolatilityConsolidation;
        }

        let mean_return: f64 = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance: f64 = returns.iter().map(|r| (r - mean_return).powi(2)).sum::<f64>() / returns.len() as f64;
        let volatility = variance.sqrt();

        if volatility > self.volatility_threshold || current_spread_bps > 5.0 {
            return MarketRegime::HighVolatilityRange;
        }

        if mean_return > 0.001 {
            MarketRegime::BullishTrend
        } else if mean_return < -0.001 {
            MarketRegime::BearishTrend
        } else {
            MarketRegime::LowVolatilityConsolidation
        }
    }
}