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

    info!("Starting MI-EDTE Passive Maker & Volatility-Gated Scalp Engine...");

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
    let notional_budget = settings.trading.notional_per_trade;

    // لیسنر دریافت سیگنال و کاشت اردر میکر (Post-Only Limit Order)
    tokio::spawn(async move {
        if let Ok(client) = redis::Client::open(redis_url_sub.as_str()) {
            if let Ok(mut pubsub) = client.get_async_pubsub().await {
                let _ = pubsub.subscribe("market:scalp_signals").await;
                info!("Subscribed to AI Maker scalp signals.");

                use futures_util::StreamExt;
                let mut stream = pubsub.on_message();

                while let Some(msg) = stream.next().await {
                    let payload: String = msg.get_payload().unwrap_or_default();
                    if let Ok(val) = serde_json::from_str::<Value>(&payload) {
                        let symbol = val["symbol"].as_str().unwrap_or("BTCUSDT");
                        let action = val["action"].as_str().unwrap_or("BUY");
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

                        let limit_price = if is_buy { metrics.best_bid } else { metrics.best_ask };
                        let size = (notional_budget / limit_price).round_dp(4);

                        let mut executor = executor_clone_signal.lock().await;
                        let _ = executor.place_maker_order(symbol, is_buy, size, &metrics);
                    }
                }
            }
        }
    });

    // حلقه تیک‌ها و ارزیابی پر شدن اردرهای میکر و بستن سودها
    while let Some(event) = event_rx.recv().await {
        match event {
            MarketEvent::Trade(trade) => {
                last_known_prices.lock().await.insert(trade.symbol.clone(), trade.price);

                if let Some(ref mut r) = redis_pub {
                    let _ = r.publish_trade(&trade).await;
                }

                let mut executor = scalp_executor.lock().await;

                // ۱. بررسی پر شدن اردرهای لیمیت کاشته‌شده
                if let Some(pos) = executor.process_pending_orders_and_fills(&trade.symbol, trade.price) {
                    if let Some(mut r) = redis_pub.clone() {
                        let pos_event = serde_json::json!({
                            "symbol": trade.symbol,
                            "action": if pos.is_buy { "BUY" } else { "SELL" },
                            "entry_price": pos.entry_price,
                            "pnl": 0.0
                        });
                        let _ = r.publish_json("market:positions", &pos_event).await;
                    }
                }
                
                // ۲. ارزیابی خروج و ثبت در دفتر کل رسمی
                if let Some(record) = executor.evaluate_open_positions(&trade.symbol, trade.price, trade.price) {
                    if let Some(mut r) = redis_pub.clone() {
                        let record_json = serde_json::to_value(&record).unwrap_or_default();
                        let _ = r.publish_json("market:trade_ledger", &record_json).await;

                        let close_event = serde_json::json!({
                            "symbol": record.symbol,
                            "action": "CLOSE",
                            "entry_price": record.entry_price,
                            "exit_price": record.exit_price,
                            "pnl": record.net_pnl,
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