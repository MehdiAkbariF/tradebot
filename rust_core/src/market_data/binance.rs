// مسیر: rust_core/src/market_data/binance.rs
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
            http_client: HttpClient::builder().timeout(Duration::from_secs(4)).build().unwrap_or_default(),
            event_tx,
        }
    }

    pub async fn fetch_snapshot(&self, symbol: &str) -> Result<(u64, Vec<(rust_decimal::Decimal, rust_decimal::Decimal)>, Vec<(rust_decimal::Decimal, rust_decimal::Decimal)>)> {
        let url = format!("{}/v5/market/orderbook?category=spot&symbol={}&limit=50", self.settings.binance_rest_url, symbol.to_uppercase());
        let res = self.http_client.get(&url).send().await?.json::<Value>().await?;

        let result_obj = &res["result"];
        let ts = result_obj["ts"].as_u64().unwrap_or(0);

        let bids = parse_bybit_levels(&result_obj["b"])?;
        let asks = parse_bybit_levels(&result_obj["a"])?;

        Ok((ts, bids, asks))
    }

    pub async fn run_stream(&self) {
        let ws_url = &self.settings.binance_ws_url;
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

                    let sub_msg = json!({
                        "op": "subscribe",
                        "args": topics
                    }).to_string();

                    if let Err(e) = ws_stream.send(Message::Text(sub_msg)).await {
                        error!("Failed to send subscription: {}", e);
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
                                                    if let Ok(trade) = parse_bybit_trade(item, received_ts) {
                                                        let _ = self.event_tx.send(MarketEvent::Trade(trade)).await;
                                                    }
                                                }
                                            }
                                        } else if topic.starts_with("orderbook") {
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
                                warn!("WebSocket server closed connection. Reconnecting...");
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
                    error!("WebSocket connection failed: {}. Retrying...", e);
                }
            }
            sleep(Duration::from_millis(self.settings.reconnect_interval_ms)).await;
        }
    }
}

fn parse_bybit_levels(val: &Value) -> Result<Vec<(rust_decimal::Decimal, rust_decimal::Decimal)>> {
    let mut levels = Vec::new();
    if let Some(arr) = val.as_array() {
        for item in arr {
            if let (Some(p_str), Some(q_str)) = (item[0].as_str(), item[1].as_str()) {
                if let (Ok(p), Ok(q)) = (p_str.parse(), q_str.parse()) {
                    levels.push((p, q));
                }
            }
        }
    }
    Ok(levels)
}

fn parse_bybit_trade(val: &Value, received_ts: chrono::DateTime<Utc>) -> Result<TradeTick> {
    let symbol = val["s"].as_str().unwrap_or("BTCUSDT").to_string();
    let price = val["p"].as_str().and_then(|p| p.parse().ok()).unwrap_or_default();
    let quantity = val["v"].as_str().and_then(|v| v.parse().ok()).unwrap_or_default();
    let side = val["S"].as_str().unwrap_or("");
    let is_buyer_maker = side == "Sell";

    Ok(TradeTick {
        symbol,
        trade_id: val["i"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0),
        price,
        quantity,
        is_buyer_maker,
        exchange_ts: received_ts,
        received_ts,
    })
}

fn parse_bybit_depth(val: &Value, received_ts: chrono::DateTime<Utc>) -> Result<DepthDelta> {
    let symbol = val["s"].as_str().unwrap_or("BTCUSDT").to_string();
    let data = &val["data"];
    let update_id = data["u"].as_u64().unwrap_or(0);

    let bids = parse_bybit_levels(&data["b"])?;
    let asks = parse_bybit_levels(&data["a"])?;

    Ok(DepthDelta {
        symbol,
        first_update_id: update_id,
        final_update_id: update_id,
        bids,
        asks,
        exchange_ts: received_ts,
        received_ts,
    })
}