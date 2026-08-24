use chrono::Utc;
use rust_decimal_macros::dec;
use rust_core::backtest::engine::{BacktestConfig, EventDrivenBacktester};

#[test]
fn test_backtester_simulation_and_metrics() {
    let config = BacktestConfig {
        initial_capital: dec!(100000.0),
        maker_fee_bps: 2.0,
        taker_fee_bps: 5.0,
        simulated_latency_ms: 10,
    };

    let mut backtester = EventDrivenBacktester::new(config);

    let now = Utc::now();
    // شبیه‌سازی دو معامله (یکی سودده، یکی زیان‌ده)
    backtester.execute_simulated_trade(
        "BTCUSDT",
        true,
        dec!(60000.0),
        dec!(61000.0),
        dec!(1.0),
        now,
        now,
    );

    backtester.execute_simulated_trade(
        "BTCUSDT",
        true,
        dec!(61000.0),
        dec!(60500.0),
        dec!(1.0),
        now,
        now,
    );

    let report = backtester.generate_performance_report();

    assert_eq!(report.total_trades, 2);
    assert_eq!(report.winning_trades, 1);
    assert_eq!(report.losing_trades, 1);
    assert_eq!(report.win_rate, 0.5);
    assert!(report.net_pnl != Decimal::ZERO);
}