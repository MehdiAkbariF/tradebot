use super::normalizer::BinanceNormalizer;
use crate::config::MarketDataSettings;
use crate::domain::book::OrderBook;
use crate::domain::types::{DepthDelta, TradeTick};
use crate::error::{AppError, Result};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use reqwest::Client as HttpClient;
use serde_json::Value;
use std::collections::HashMap;
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
        let url = format!("{}/api/v3/depth?symbol={}&limit=1000", self.settings.binance_rest_url, symbol.to_uppercase());
        let res = self.http_client.get(&url).send().await?.json::<Value>().await?;

        let last_update_id = res["lastUpdateId"].as_u64().ok_or_else(|| AppError::Protocol("Snapshot missing lastUpdateId".into()))?;
        let bids = BinanceNormalizer::parse_levels(&res["bids"])?;
        let asks = BinanceNormalizer::parse_levels(&res["asks"])?;

        Ok((last_update_id, bids, asks))
    }

    pub async fn run_stream(&self) {
        let streams: Vec<String> = self.settings.symbols.iter().flat_map(|s| {
            let lower = s.to_lowercase();
            vec![format!("{}@trade", lower), format!("{}@depth@100ms", lower)]
        }).collect();

        let stream_path = streams.join("/");
        let ws_url = format!("{}/{}", self.settings.binance_ws_url, stream_path);

        loop {
            info!("Connecting to Binance WebSocket: {}", ws_url);
            match connect_async(&ws_url).await {
                Ok((mut ws_stream, _)) => {
                    info!("Successfully connected to Binance WebSocket.");
                    while let Some(msg) = ws_stream.next().await {
                        let received_ts = Utc::now();
                        match msg {
                            Ok(Message::Text(text)) => {
                                if let Ok(val) = serde_json::from_str::<Value>(&text) {
                                    if let Some(event_type) = val.get("e").and_then(|e| e.as_str()) {
                                        match event_type {
                                            "trade" => {
                                                if let Ok(trade) = BinanceNormalizer::parse_trade(&val, received_ts) {
                                                    let _ = self.event_tx.send(MarketEvent::Trade(trade)).await;
                                                }
                                            }
                                            "depthUpdate" => {
                                                if let Ok(delta) = BinanceNormalizer::parse_depth_delta(&val, received_ts) {
                                                    let _ = self.event_tx.send(MarketEvent::Depth(delta)).await;
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            Ok(Message::Ping(p)) => {
                                let _ = ws_stream.send(Message::Pong(p)).await;
                            }
                            Ok(Message::Close(_)) => {
                                warn!("Binance server sent close frame. Reconnecting...");
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
                    error!("WebSocket connection failed: {}. Retrying in {}ms...", e, self.settings.reconnect_interval_ms);
                }
            }
            sleep(Duration::from_millis(self.settings.reconnect_interval_ms)).await;
        }
    }
}