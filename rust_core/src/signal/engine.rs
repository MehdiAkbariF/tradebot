use crate::domain::portfolio::{SignalAction, SignalCandidate};
use crate::domain::types::OrderBookMetrics;
use chrono::Utc;
use uuid::Uuid;

pub struct SignalEngine {
    min_confidence: f64,
    max_spread_bps: f64,
}

impl SignalEngine {
    pub fn new(min_confidence: f64, max_spread_bps: f64) -> Self {
        Self {
            min_confidence,
            max_spread_bps,
        }
    }

    /// ارزیابی ترکیب خبر و وضعیت اردر بوک برای تولید سیگنال
    pub fn evaluate_signal(
        &self,
        symbol: &str,
        metrics: &OrderBookMetrics,
        news_relevance: f64,
        is_positive_news: bool,
    ) -> SignalCandidate {
        let mut reason_codes = Vec::new();

        // 1. Check Spread Friction
        if metrics.spread_bps > self.max_spread_bps {
            reason_codes.push("HIGH_SPREAD_NO_TRADE".to_string());
            return self.no_trade_signal(symbol, reason_codes);
        }

        // 2. Check Order Flow Imbalance (OFI) confirmation
        let ofi_confirmed = if is_positive_news {
            metrics.imbalance_top10 > 0.15
        } else {
            metrics.imbalance_top10 < -0.15
        };

        if !ofi_confirmed {
            reason_codes.push("MICROSTRUCTURE_MISMATCH".to_string());
            return self.no_trade_signal(symbol, reason_codes);
        }

        // 3. Evaluate Confidence
        let confidence = (news_relevance + metrics.imbalance_top10.abs()) / 2.0;
        if confidence < self.min_confidence {
            reason_codes.push("LOW_CONFIDENCE".to_string());
            return self.no_trade_signal(symbol, reason_codes);
        }

        // 4. Generate Trade Action
        let action = if is_positive_news {
            reason_codes.push("POSITIVE_NEWS_WITH_OFI_BULLISH".to_string());
            SignalAction::Buy
        } else {
            reason_codes.push("NEGATIVE_NEWS_WITH_OFI_BEARISH".to_string());
            SignalAction::Sell
        };

        SignalCandidate {
            signal_id: Uuid::new_v4().to_string(), // اصلاح شده از toString به to_string
            event_id: None,
            symbol: symbol.to_string(),
            action,
            confidence,
            expected_horizon_sec: 60,
            expected_return: 0.003,
            decision_ts: Utc::now(),
            reason_codes,
        }
    }

    fn no_trade_signal(&self, symbol: &str, reason_codes: Vec<String>) -> SignalCandidate {
        SignalCandidate {
            signal_id: Uuid::new_v4().to_string(), // اصلاح شده از toString به to_string
            event_id: None,
            symbol: symbol.to_string(),
            action: SignalAction::NoTrade,
            confidence: 0.0,
            expected_horizon_sec: 0,
            expected_return: 0.0,
            decision_ts: Utc::now(),
            reason_codes,
        }
    }
}