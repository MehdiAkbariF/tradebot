use chrono::Utc;
use rust_decimal_macros::dec;
use rust_core::domain::book::OrderBook;
use rust_core::domain::types::DepthDelta;

#[test]
fn test_orderbook_snapshot_and_sorting() {
    let mut book = OrderBook::new("BTCUSDT".into());
    
    let bids = vec![
        (dec!(65000.0), dec!(1.5)),
        (dec!(65010.0), dec!(2.0)),
        (dec!(64990.0), dec!(5.0)),
    ];
    let asks = vec![
        (dec!(65020.0), dec!(1.0)),
        (dec!(65030.0), dec!(3.0)),
    ];

    book.apply_snapshot(100, bids, asks);

    let (best_bid_p, _) = book.best_bid().unwrap();
    let (best_ask_p, _) = book.best_ask().unwrap();

    assert_eq!(best_bid_p, dec!(65010.0));
    assert_eq!(best_ask_p, dec!(65020.0));
}

#[test]
fn test_orderbook_delta_update_and_metrics() {
    let mut book = OrderBook::new("BTCUSDT".into());
    book.apply_snapshot(100, vec![(dec!(100.0), dec!(1.0))], vec![(dec!(102.0), dec!(1.0))]);

    let delta = DepthDelta {
        symbol: "BTCUSDT".into(),
        first_update_id: 101,
        final_update_id: 101,
        bids: vec![(dec!(101.0), dec!(2.0))],
        asks: vec![(dec!(102.0), dec!(0.0)), (dec!(103.0), dec!(1.0))],
        exchange_ts: Utc::now(),
        received_ts: Utc::now(),
    };

    let res = book.apply_delta(&delta);
    assert!(res.is_ok());

    let metrics = book.compute_metrics().unwrap();
    assert_eq!(metrics.mid_price, dec!(102.0));
}

#[test]
fn test_orderbook_sequence_gap_detection() {
    let mut book = OrderBook::new("BTCUSDT".into());
    book.apply_snapshot(100, vec![(dec!(100.0), dec!(1.0))], vec![(dec!(102.0), dec!(1.0))]);

    let delta = DepthDelta {
        symbol: "BTCUSDT".into(),
        first_update_id: 105, // Gap! Expected 101
        final_update_id: 106,
        bids: vec![],
        asks: vec![],
        exchange_ts: Utc::now(),
        received_ts: Utc::now(),
    };

    let res = book.apply_delta(&delta);
    assert!(res.is_err());
}