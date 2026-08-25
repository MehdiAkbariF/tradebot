// مسیر: rust_core/src/main.rs
use rust_core::config::Settings;
use rust_core::domain::book::OrderBook;
use rust_core::domain::types::OrderBookMetrics;
use rust_core::execution::HighFrequencyScalpExecutor;
use rust_core::market_data::binance::{BinanceClient, MarketEvent};
use rust_core::storage::redis::RedisPublisher;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder().with_max_level(Level::INFO).finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting MI-EDTE Bidirectional Scalping Core with Floating PnL Tracker...");

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

    let (event_tx, mut event_rx) = mpsc::channel::<MarketEvent>(settings.market_data.channel_buffer_size);
    let binance_client = BinanceClient::new(settings.market_data.clone(), event_tx);

    tokio::spawn(async move {
        binance_client.run_stream().await;
    });

    let redis_url_sub = settings.storage.redis_url.clone();
    let executor_clone_signal = scalp_executor.clone();
    let books_clone_signal = order_books.clone();
    let prices_clone_signal = last_known_prices.clone();
    let redis_pub_for_signal = redis_pub.clone();
    let notional_budget = settings.trading.notional_per_trade;

    tokio::spawn(async move {
        if let Ok(client) = redis::Client::open(redis_url_sub.as_str()) {
            if let Ok(mut pubsub) = client.get_async_pubsub().await {
                let _ = pubsub.subscribe("market:scalp_signals").await;
                info!("Rust Core subscribed to 'market:scalp_signals' from AI Engine.");

                use futures_util::StreamExt;
                let mut stream = pubsub.on_message();

                while let Some(msg) = stream.next().await {
                    let payload: String = msg.get_payload().unwrap_or_default();
                    if let Ok(val) = serde_json::from_str::<Value>(&payload) {
                        let symbol = val["symbol"].as_str().unwrap_or("BTCUSDT");
                        let action = val["action"].as_str().unwrap_or("BUY");
                        let prob = val["probability"].as_f64().unwrap_or(0.60);
                        let is_buy = action == "BUY";

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
                                imbalance_top10: if is_buy { 0.25 } else { -0.25 },
                                timestamp: chrono::Utc::now(),
                            })
                        } else {
                            continue;
                        };

                        let exec_price = (if is_buy { metrics.best_ask } else { metrics.best_bid }).round_dp(2);
                        let size = (notional_budget / exec_price).round_dp(4);

                        let mut executor = executor_clone_signal.lock().await;
                        if let Some(pos) = executor.try_open_scalp(symbol, is_buy, size, &metrics, prob) {
                            info!(id = %pos.id, symbol = %symbol, action = %action, entry = %pos.entry_price, size = %size, "🎯 Position OPENED");
                            
                            if let Some(mut r) = redis_pub_for_signal.clone() {
                                let pos_event = serde_json::json!({
                                    "symbol": symbol,
                                    "action": action,
                                    "entry_price": pos.entry_price,
                                    "pnl": 0.0
                                });
                                let _ = r.publish_json("market:positions", &pos_event).await;
                            }
                        }
                    }
                }
            }
        }
    });

    let mut last_pnl_log_time = std::time::Instant::now();

    while let Some(event) = event_rx.recv().await {
        match event {
            MarketEvent::Trade(trade) => {
                last_known_prices.lock().await.insert(trade.symbol.clone(), trade.price);

                if let Some(ref mut r) = redis_pub {
                    let _ = r.publish_trade(&trade).await;
                }

                let mut executor = scalp_executor.lock().await;
                
                // ارزیابی بسته شدن معامله
                if let Some(pnl) = executor.evaluate_open_positions(&trade.symbol, trade.price, trade.price) {
                    if let Some(mut r) = redis_pub.clone() {
                        let close_event = serde_json::json!({
                            "symbol": trade.symbol,
                            "action": "CLOSE",
                            "entry_price": trade.price,
                            "exit_price": trade.price,
                            "pnl": pnl
                        });
                        let _ = r.publish_json("market:positions", &close_event).await;
                    }
                }

                // چاپ دوره‌ای وضعیت زنده پورتفوی
                if last_pnl_log_time.elapsed().as_secs() >= 8 {
                    last_pnl_log_time = std::time::Instant::now();
                    info!(balance = %executor.get_cash(), "💼 Paper Portfolio Balance Updated");
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