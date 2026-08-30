// مسیر: rust_core/src/main.rs
use chrono::Utc;
use rust_core::config::Settings;
use rust_core::domain::book::OrderBook;
use rust_core::domain::types::{CanonicalSignalPayload, MicrosecondAudit, OrderBookMetrics};
use rust_core::execution::ExecutionEngine;
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

    info!("Starting MI-EDTE Production Quantitative Core Engine (Canonical v9.0)...");

    let settings = Settings::new()?;
    let mut redis_pub = RedisPublisher::new(&settings.storage.redis_url).await.ok();
    
    let last_known_prices = Arc::new(Mutex::new(HashMap::<String, Decimal>::new()));
    let order_books = Arc::new(Mutex::new(HashMap::<String, OrderBook>::new()));
    
    for s in &settings.market_data.symbols {
        let mut book = OrderBook::new(s.to_uppercase());
        book.is_initialized = true;
        order_books.lock().await.insert(s.to_uppercase(), book);
    }

    let execution_engine = Arc::new(Mutex::new(ExecutionEngine::new(&settings.trading)));

    let (event_tx, mut event_rx) = mpsc::channel::<MarketEvent>(settings.market_data.channel_buffer_size);
    let binance_client = BinanceClient::new(settings.market_data.clone(), event_tx);

    tokio::spawn(async move {
        binance_client.run_stream().await;
    });

    let redis_url_sub = settings.storage.redis_url.clone();
    let engine_clone_signal = execution_engine.clone();
    let books_clone_signal = order_books.clone();
    let prices_clone_signal = last_known_prices.clone();

    tokio::spawn(async move {
        if let Ok(client) = redis::Client::open(redis_url_sub.as_str()) {
            if let Ok(mut pubsub) = client.get_async_pubsub().await {
                let _ = pubsub.subscribe("market:scalp_signals").await;
                info!("Subscribed to Canonical Scalp Signals stream.");

                use futures_util::StreamExt;
                let mut stream = pubsub.on_message();

                while let Some(msg) = stream.next().await {
                    let payload: String = msg.get_payload().unwrap_or_default();
                    if let Ok(signal) = serde_json::from_str::<CanonicalSignalPayload>(&payload) {
                        let symbol = &signal.symbol;

                        let live_price = {
                            let prices = prices_clone_signal.lock().await;
                            prices.get(symbol).cloned().unwrap_or(signal.signal_price)
                        };

                        let audit = MicrosecondAudit {
                            exchange_ts: Utc::now(),
                            receive_ts: Utc::now(),
                            process_ts: Utc::now(),
                            decision_ts: Some(Utc::now()),
                            submit_ts: None,
                            fill_ts: None,
                        };

                        let metrics = {
                            let mut books = books_clone_signal.lock().await;
                            match books.get_mut(symbol).and_then(|b| b.compute_metrics(audit.clone())) {
                                Some(m) => m,
                                None => OrderBookMetrics {
                                    symbol: symbol.to_string(),
                                    best_bid: live_price - dec!(0.1),
                                    best_ask: live_price + dec!(0.1),
                                    spread_bps: 0.50,
                                    mid_price: live_price,
                                    micro_price: live_price,
                                    ofi: 0.0,
                                    ofi_zscore: 0.0,
                                    book_imbalance_top10: 0.0,
                                    imbalance_top10: 0.0,
                                    realized_vol_60s_bps: 1.0,
                                    timestamp: Utc::now(),
                                    audit,
                                },
                            }
                        };

                        let mut engine = engine_clone_signal.lock().await;
                        let _ = engine.handle_signal(&signal, &metrics);
                    }
                }
            }
        }
    });

    while let Some(event) = event_rx.recv().await {
        match event {
            MarketEvent::Trade(trade) => {
                last_known_prices.lock().await.insert(trade.symbol.clone(), trade.price);

                if let Some(ref mut r) = redis_pub {
                    let _ = r.publish_trade(&trade).await;
                }

                let (micro_price, best_bid, best_ask) = {
                    let mut books = order_books.lock().await;
                    if let Some(book) = books.get_mut(&trade.symbol) {
                        let bb = book.best_bid();
                        let ba = book.best_ask();
                        let micro = match (bb, ba) {
                            (Some((bb_p, bb_q)), Some((ba_p, ba_q))) => {
                                let tot = bb_q + ba_q;
                                if tot > dec!(0.0) {
                                    (ba_q * bb_p + bb_q * ba_p) / tot
                                } else {
                                    trade.price
                                }
                            }
                            _ => trade.price,
                        };
                        (micro, bb.map(|b| b.0).unwrap_or(trade.price), ba.map(|a| a.0).unwrap_or(trade.price))
                    } else {
                        (trade.price, trade.price, trade.price)
                    }
                };

                let mut engine = execution_engine.lock().await;
                let _ = engine.on_market_tick(&trade.symbol, trade.price, micro_price);

                if let Some(record) = engine.evaluate_exit(&trade.symbol, best_bid, best_ask) {
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
                    let audit = MicrosecondAudit {
                        exchange_ts: delta.exchange_ts,
                        receive_ts: delta.received_ts,
                        process_ts: Utc::now(),
                        decision_ts: None,
                        submit_ts: None,
                        fill_ts: None,
                    };
                    if let Some(metrics) = book.compute_metrics(audit) {
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