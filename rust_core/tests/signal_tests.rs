use chrono::Utc;
use rust_decimal_macros::dec;
use rust_core::domain::portfolio::SignalAction;
use rust_core::domain::types::OrderBookMetrics;
use rust_core::signal::engine::SignalEngine;

#[test]
fn test_signal_generation_buy() {
    let engine = SignalEngine::new(0.6, 5.0); // min_conf: 0.6, max_spread: 5.0 bps

    let metrics = OrderBookMetrics {
        symbol: "BTCUSDT".into(),
        best_bid: dec!(65000.0),
        best_ask: dec!(65001.0),
        spread_bps: 0.15, // Very tight spread
        mid_price: dec!(65000.5),
        micro_price: dec!(65000.8),
        imbalance_top10: 0.45, // Strong bullish order flow
        timestamp: Utc::now(),
    };

    // Evaluate positive news
    let signal = engine.evaluate_signal("BTCUSDT", &metrics, 0.85, true);

    assert_eq!(signal.action, SignalAction::Buy);
    assert!(signal.confidence >= 0.6);
}

#[test]
fn test_signal_generation_no_trade_due_to_spread() {
    let engine = SignalEngine::new(0.6, 2.0); // max_spread: 2.0 bps

    let metrics = OrderBookMetrics {
        symbol: "BTCUSDT".into(),
        best_bid: dec!(65000.0),
        best_ask: dec!(65020.0),
        spread_bps: 3.5, // Spread too wide!
        mid_price: dec!(65010.0),
        micro_price: dec!(65010.0),
        imbalance_top10: 0.5,
        timestamp: Utc::now(),
    };

    let signal = engine.evaluate_signal("BTCUSDT", &metrics, 0.85, true);

    assert_eq!(signal.action, SignalAction::NoTrade);
    assert!(signal.reason_codes.contains(&"HIGH_SPREAD_NO_TRADE".to_string()));
}