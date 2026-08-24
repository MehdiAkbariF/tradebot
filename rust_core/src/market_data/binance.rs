use super::normalizer::BinanceNormalizer; // ماژول نرمال‌ساز قابل استفاده مجدد
use crate::config::MarketDataSettings;
use crate::domain::types::{DepthDelta, TradeTick};
use crate::error::{AppError, Result};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use reqwest::Client as HttpClient;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};

pub enum MarketEvent {
    Trade(TradeTick),
    Depth(DepthDelta),
}

pub struct BinanceClient {
    settings: MarketDataSettings,
    http_client: HttpClient,
    event_tx: mpsc::Sender<MarketEvent>,
}

impl BinanceClient {
    pub fn new(settings: MarketDataSettings, event_tx: mpsc::Sender<MarketEvent>) -> Self {
        Self {
            settings,
            http_client: HttpClient::new(),
            event_tx,
        }
    }
pub async fn fetch_snapshot(&self, symbol: &str) -> Result<(u64, Vec<(rust_decimal::Decimal, rust_decimal::Decimal)>, Vec<(rust_decimal::Decimal, rust_decimal::Decimal)>)> {
        // Bybit Spot Orderbook REST API endpoint
        let url = format!("{}/v5/market/orderbook?category=spot&symbol={}&limit=50", self.settings.binance_rest_url, symbol.to_uppercase());
        let res = self.http_client.get(&url).send().await?.json::<Value>().await?;

        let result_obj = &res["result"];
        let ts = result_obj["ts"].as_u64().unwrap_or(0);

        let bids_raw = &result_obj["b"];
        let asks_raw = &result_obj["a"];

        let bids = BinanceNormalizer::parse_levels(bids_raw)?;
        let asks = BinanceNormalizer::parse_levels(asks_raw)?;

        Ok((ts, bids, asks))
    }

    pub async fn run_stream(&self) {
        // Bybit Spot Public WebSocket Endpoint
        let ws_url = "wss://stream.bybit.com/v5/public/spot";

        let mut topics = Vec::new();
        for s in &self.settings.symbols {
            let sym = s.to_uppercase();
            topics.push(format!("publicTrade.{}", sym));
            topics.push(format!("orderbook.50.{}", sym));
        }

        loop {
            info!("Connecting to Bybit WebSocket: {}", ws_url);
            match connect_async(ws_url).await {
                Ok((mut ws_stream, _)) => {
                    info!("Successfully connected to Bybit WebSocket.");

                    // Subscribe to topics
                    let sub_msg = json!({
                        "op": "subscribe",
                        "args": topics
                    }).to_string();

                    if let Err(e) = ws_stream.send(Message::Text(sub_msg)).await {
                        error!("Failed to send subscription message: {}", e);
                        continue;
                    }

                    while let Some(msg) = ws_stream.next().await {
                        let received_ts = Utc::now();
                        match msg {
                            Ok(Message::Text(text)) => {
                                if let Ok(val) = serde_json::from_str::<Value>(&text) {
                                    if let Some(topic) = val.get("topic").and_then(|t| t.as_str()) {
                                        if topic.starts_with("publicTrade") {
                                            if let Some(data_arr) = val.get("data").and_then(|d| d.as_array()) {
                                                for item in data_arr {
                                                    // نرمال‌سازی ترید بای‌بیت
                                                    if let Ok(trade) = parse_bybit_trade(item, received_ts) {
                                                        let _ = self.event_tx.send(MarketEvent::Trade(trade)).await;
                                                    }
                                                }
                                            }
                                        } else if topic.starts_with("orderbook") {
                                            // پردازش دلتا/اسنپ‌شات اردر بوک بای‌بیت
                                            if let Ok(delta) = parse_bybit_depth(&val, received_ts) {
                                                let _ = self.event_tx.send(MarketEvent::Depth(delta)).await;
                                            }
                                        }
                                    }
                                }
                            }
                            Ok(Message::Ping(p)) => {
                                let _ = ws_stream.send(Message::Pong(p)).await;
                            }
                            Ok(Message::Close(_)) => {
                                warn!("Bybit server sent close frame. Reconnecting...");
                                break;
                            }
                            Err(e) => {
                                error!("WebSocket read error: {}. Reconnecting...", e);
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    error!("Bybit WebSocket connection failed: {}. Retrying in {}ms...", e, self.settings.reconnect_interval_ms);
                }
            }
            sleep(Duration::from_millis(self.settings.reconnect_interval_ms)).await;
        }
    }
}

// توابع کمکی پارس کردن ساختار داده‌های Bybit
fn parse_bybit_trade(val: &Value, received_ts: chrono::DateTime<Utc>) -> Result<TradeTick> {
    let symbol = val["s"].as_str().unwrap_or("UNKNOWN").to_string();
    let price_str = val["p"].as_str().unwrap_or("0");
    let price = rust_decimal::Decimal::from_str_exact(price_str).unwrap_or_default();
    
    let qty_str = val["v"].as_str().unwrap_or("0");
    let quantity = rust_decimal::Decimal::from_str_exact(qty_str).unwrap_or_default();
    
    let side = val["S"].as_str().unwrap_or("");
    let is_buyer_maker = side == "Sell"; // در بای‌بیت تیک سل یعنی خریدار میکر بوده
    
    let ts_millis = val["T"].as_i64().unwrap_or(received_ts.timestamp_millis());
    let exchange_ts = chrono::TimeZone::timestamp_millis_opt(&Utc, ts_millis).single().unwrap_or(received_ts);

    Ok(TradeTick {
        symbol,
        trade_id: val["i"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0),
        price,
        quantity,
        is_buyer_maker,
        exchange_ts,
        received_ts,
    })
}

fn parse_bybit_depth(val: &Value, received_ts: chrono::DateTime<Utc>) -> Result<DepthDelta> {
    let symbol = val["s"].as_str().unwrap_or("UNKNOWN").to_string();
    let data = &val["data"];
    
    let update_id = data["u"].as_u64().unwrap_or(0);
    let ts_millis = val["ts"].as_i64().unwrap_or(received_ts.timestamp_millis());
    let exchange_ts = chrono::TimeZone::timestamp_millis_opt(&Utc, ts_millis).single().unwrap_or(received_ts);

    let bids = BinanceNormalizer::parse_levels(&data["b"])?;
    let asks = BinanceNormalizer::parse_levels(&data["a"])?;

    Ok(DepthDelta {
        symbol,
        first_update_id: update_id,
        final_update_id: update_id,
        bids,
        asks,
        exchange_ts,
        received_ts,
    })
}