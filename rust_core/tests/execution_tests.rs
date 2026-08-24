use chrono::Utc;
use rust_decimal_macros::dec;
use rust_core::domain::portfolio::{SignalAction, SignalCandidate};
use rust_core::domain::types::OrderBookMetrics;
use rust_core::execution::ExecutionSimulator; // <--- اصلاح مسیر ایمپورت

#[test]
fn test_paper_execution_open_and_close() {
    let mut simulator = ExecutionSimulator::new(dec!(100000.0), 2.0, 5.0);

    let metrics = OrderBookMetrics {
        symbol: "BTCUSDT".into(),
        best_bid: dec!(65000.0),
        best_ask: dec!(65002.0),
        spread_bps: 0.3,
        mid_price: dec!(65001.0),
        micro_price: dec!(65001.0),
        imbalance_top10: 0.5,
        timestamp: Utc::now(),
    };

    let buy_signal = SignalCandidate {
        signal_id: "sig-1".into(),
        event_id: None,
        symbol: "BTCUSDT".into(),
        action: SignalAction::Buy,
        confidence: 0.85,
        expected_horizon_sec: 60,
        expected_return: 0.003,
        decision_ts: Utc::now(),
        reason_codes: vec!["BULLISH".into()],
    };

    // 1. Open Buy Position
    let pos = simulator.process_signal(&buy_signal, &metrics, dec!(0.5));
    assert!(pos.is_some());
    let opened = pos.unwrap();
    assert!(opened.is_open);
    assert_eq!(opened.entry_price, dec!(65002.0)); // Executed at Best Ask

    // 2. Close Position with Sell Signal
    let sell_signal = SignalCandidate {
        signal_id: "sig-2".into(),
        event_id: None,
        symbol: "BTCUSDT".into(),
        action: SignalAction::Sell,
        confidence: 0.85,
        expected_horizon_sec: 60,
        expected_return: 0.003,
        decision_ts: Utc::now(),
        reason_codes: vec!["BEARISH".into()],
    };

    let closed_pos = simulator.process_signal(&sell_signal, &metrics, dec!(0.5));
    assert!(closed_pos.is_some());
    assert!(!closed_pos.unwrap().is_open);
}