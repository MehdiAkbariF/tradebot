use chrono::Utc;
use rust_decimal_macros::dec;
use rust_core::domain::portfolio::{SignalAction, SignalCandidate};
use rust_core::risk::engine::{RiskConfig, RiskEngine, RiskCheckResult};

#[test]
fn test_risk_engine_approval() {
    let config = RiskConfig {
        max_position_size: dec!(1000.0),
        max_open_positions: 3,
        max_daily_loss: dec!(500.0),
        max_spread_bps: 4.0,
        kill_switch_active: false,
    };

    let mut engine = RiskEngine::new(config);
    engine.update_state(dec!(0.0), 1); // 1 open position, 0 loss

    let signal = SignalCandidate {
        signal_id: "test-sig-1".into(),
        event_id: None,
        symbol: "BTCUSDT".into(),
        action: SignalAction::Buy,
        confidence: 0.85,
        expected_horizon_sec: 60,
        expected_return: 0.004,
        decision_ts: Utc::now(),
        reason_codes: vec!["BULLISH".into()],
    };

    let result = engine.validate_signal(&signal, 1.5); // spread 1.5 bps

    match result {
        RiskCheckResult::Approved { size } => {
            assert_eq!(size, dec!(1000.0));
        }
        RiskCheckResult::Rejected { reason } => {
            panic!("Expected approval, but got rejection: {}", reason);
        }
    }
}

#[test]
fn test_risk_engine_kill_switch() {
    let config = RiskConfig {
        max_position_size: dec!(1000.0),
        max_open_positions: 3,
        max_daily_loss: dec!(500.0),
        max_spread_bps: 4.0,
        kill_switch_active: true, // KILL SWITCH ON
    };

    let engine = RiskEngine::new(config);
    let signal = SignalCandidate {
        signal_id: "test-sig-2".into(),
        event_id: None,
        symbol: "BTCUSDT".into(),
        action: SignalAction::Buy,
        confidence: 0.9,
        expected_horizon_sec: 60,
        expected_return: 0.004,
        decision_ts: Utc::now(),
        reason_codes: vec![],
    };

    let result = engine.validate_signal(&signal, 1.0);

    match result {
        RiskCheckResult::Approved { .. } => {
            panic!("Expected rejection due to kill switch!");
        }
        RiskCheckResult::Rejected { reason } => {
            assert_eq!(reason, "KILL_SWITCH_ACTIVE");
        }
    }
}