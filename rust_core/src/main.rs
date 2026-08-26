// مسیر: rust_core/src/main.rs
use rust_core::config::Settings;
use rust_core::domain::book::OrderBook;
use rust_core::domain::types::{CanonicalSignalPayload, OrderBookMetrics};
use rust_core::execution::HighFrequencyScalpExecutor;
use rust_core::market_data::binance::{BinanceClient, MarketEvent};
use rust_core::storage::redis::RedisPublisher;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder().with_max_level(Level::INFO).finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting MI-EDTE Fully-Instrumented Research & Execution Engine...");

    let settings = Settings::new()?;
    let mut redis_pub = RedisPublisher::new(&settings.storage.redis_url).await.ok();
    
    let last_known_prices = Arc::new(Mutex::new(HashMap::<String, Decimal>::new()));
    let order_books = Arc::new(Mutex::new(HashMap::<String, OrderBook>::new()));
    
    for s in &settings.market_data.symbols {
        let mut book = OrderBook::new(s.to_uppercase());
        book.is_initialized = true;
        order_books.lock().await.insert(s.to_uppercase(), book);
    }

    let scalp_executor = Arc::new(Mutex::new(HighFrequencyScalpExecutor::new(&settings.trading)));

    // ⚡ ۱. استعلام و همگام‌سازی خودکار تنظیمات سرمایه از گیت‌وی در لحظه روشن شدن (Boot Sync)
    let executor_bootstrap = scalp_executor.clone();
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .unwrap_or_default();
        
        // چند تلاش تکرار در صورت روشن شدن همزمان سرویس‌ها
        for _ in 0..6 {
            if let Ok(res) = client.get("http://127.0.0.1:8000/api/config/capital").send().await {
                if let Ok(val) = res.json::<serde_json::Value>().await {
                    let cap = val["total_capital"].as_str().and_then(|s| s.parse::<Decimal>().ok())
                        .or_else(|| val["total_capital"].as_f64().and_then(|f| Decimal::from_f64_retain(f)));
                    let alloc = val["allocation_pct"].as_f64().and_then(|f| Decimal::from_f64_retain(f));
                    let lev = val["leverage"].as_f64().and_then(|f| Decimal::from_f64_retain(f));
                    let kill = val["kill_switch"].as_bool();

                    let mut executor = executor_bootstrap.lock().await;
                    executor.update_risk_config(cap, alloc, lev, kill);
                    info!("✅ Bootstrapped dynamic risk parameters directly from Gateway API!");
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    });

    let (event_tx, mut event_rx) = mpsc::channel::<MarketEvent>(settings.market_data.channel_buffer_size);
    let binance_client = BinanceClient::new(settings.market_data.clone(), event_tx);

    tokio::spawn(async move {
        binance_client.run_stream().await;
    });

    let redis_url_sub = settings.storage.redis_url.clone();
    let executor_clone_signal = scalp_executor.clone();
    let books_clone_signal = order_books.clone();
    let prices_clone_signal = last_known_prices.clone();

    // ۲. لیسنر دریافت سیگنال با ردیابی شناسه سیگنال
    tokio::spawn(async move {
        if let Ok(client) = redis::Client::open(redis_url_sub.as_str()) {
            if let Ok(mut pubsub) = client.get_async_pubsub().await {
                let _ = pubsub.subscribe("market:scalp_signals").await;
                info!("Subscribed to Canonical Scalp Signals.");

                use futures_util::StreamExt;
                let mut stream = pubsub.on_message();

                while let Some(msg) = stream.next().await {
                    let payload: String = msg.get_payload().unwrap_or_default();
                    if let Ok(signal) = serde_json::from_str::<CanonicalSignalPayload>(&payload) {
                        let symbol = &signal.symbol;

                        let live_price = {
                            let prices = prices_clone_signal.lock().await;
                            prices.get(symbol).cloned()
                        };

                        let current_price = match live_price {
                            Some(p) if p > dec!(0.0) => p,
                            _ => continue,
                        };

                        let mut books = books_clone_signal.lock().await;
                        let metrics = if let Some(book) = books.get_mut(symbol) {
                            book.compute_metrics().unwrap_or(OrderBookMetrics {
                                symbol: symbol.to_string(),
                                best_bid: current_price - dec!(0.1),
                                best_ask: current_price + dec!(0.1),
                                spread_bps: 0.15,
                                mid_price: current_price,
                                micro_price: current_price,
                                imbalance_top10: if signal.action == "BUY" { 0.25 } else { -0.25 },
                                timestamp: chrono::Utc::now(),
                            })
                        } else {
                            continue;
                        };

                        let mut executor = executor_clone_signal.lock().await;
                        let _ = executor.place_maker_order(&signal, &metrics);
                    }
                }
            }
        }
    });

    // ۳. لیسنر تغییرات لحظه‌ای ریسک و سرمایه از فرانت‌اند
    let redis_url_cfg = settings.storage.redis_url.clone();
    let executor_clone_cfg = scalp_executor.clone();
    tokio::spawn(async move {
        if let Ok(client) = redis::Client::open(redis_url_cfg.as_str()) {
            if let Ok(mut pubsub) = client.get_async_pubsub().await {
                let _ = pubsub.subscribe("config:capital_risk").await;
                info!("Listening for Capital & Risk telemetry...");

                use futures_util::StreamExt;
                let mut stream = pubsub.on_message();

                while let Some(msg) = stream.next().await {
                    let payload: String = msg.get_payload().unwrap_or_default();
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&payload) {
                        let cap = val["total_capital"].as_str().and_then(|s| s.parse::<Decimal>().ok())
                            .or_else(|| val["total_capital"].as_f64().and_then(|f| Decimal::from_f64_retain(f)));
                        let alloc = val["allocation_pct"].as_f64().and_then(|f| Decimal::from_f64_retain(f));
                        let lev = val["leverage"].as_f64().and_then(|f| Decimal::from_f64_retain(f));
                        let kill = val["kill_switch"].as_bool();

                        let mut executor = executor_clone_cfg.lock().await;
                        executor.update_risk_config(cap, alloc, lev, kill);
                    }
                }
            }
        }
    });

    // ۴. پردازش تیک‌ها، به‌روزرسانی MFE / MAE و ثبت خروجی‌ها
    while let Some(event) = event_rx.recv().await {
        match event {
            MarketEvent::Trade(trade) => {
                last_known_prices.lock().await.insert(trade.symbol.clone(), trade.price);

                if let Some(ref mut r) = redis_pub {
                    let _ = r.publish_trade(&trade).await;
                }

                let mut executor = scalp_executor.lock().await;

                // ردیابی دائمی مسیر قیمت جهت محاسبه MFE / MAE
                executor.update_path_dependency(&trade.symbol, trade.price);

                // ارزیابی پر شدن سفارش
                let current_spread = 0.25;
                if let Some(pos) = executor.process_pending_orders_and_fills(&trade.symbol, trade.price, current_spread) {
                    if let Some(mut r) = redis_pub.clone() {
                        let pos_event = serde_json::json!({
                            "position_id": pos.position_id,
                            "signal_id": pos.signal_id,
                            "symbol": trade.symbol,
                            "action": if pos.is_buy { "BUY" } else { "SELL" },
                            "entry_price": pos.entry_price,
                            "pnl": 0.0
                        });
                        let _ = r.publish_json("market:positions", &pos_event).await;
                    }
                }
                
                // ارزیابی خروج و انتشار گزارش به دیتابیس لجر
                if let Some(record) = executor.evaluate_open_positions(&trade.symbol, trade.price, trade.price) {
                    if let Some(mut r) = redis_pub.clone() {
                        let record_json = serde_json::to_value(&record).unwrap_or_default();
                        let _ = r.publish_json("market:trade_ledger", &record_json).await;

                        let close_event = serde_json::json!({
                            "trade_id": record.trade_id,
                            "signal_id": record.signal_id,
                            "symbol": record.symbol,
                            "action": "CLOSE",
                            "entry_price": record.entry_price,
                            "exit_price": record.exit_price,
                            "pnl": record.net_pnl,
                            "mfe_bps": record.mfe_bps,
                            "mae_bps": record.mae_bps,
                            "reason": record.exit_reason
                        });
                        let _ = r.publish_json("market:positions", &close_event).await;
                    }
                }
            }
            MarketEvent::Depth(delta) => {
                let mut books = order_books.lock().await;
                if let Some(book) = books.get_mut(&delta.symbol) {
                    let _ = book.apply_delta(&delta);
                    if let Some(metrics) = book.compute_metrics() {
                        if let Some(ref mut r) = redis_pub {
                            let _ = r.publish_metrics(&metrics).await;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}