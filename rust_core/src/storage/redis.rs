use crate::domain::types::{OrderBookMetrics, TradeTick};
use crate::error::Result;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;

#[derive(Clone)]
pub struct RedisPublisher {
    conn: ConnectionManager,
}

impl RedisPublisher {
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let conn = ConnectionManager::new(client).await?;
        Ok(Self { conn })
    }

    pub async fn publish_trade(&mut self, trade: &TradeTick) -> Result<()> {
        let payload = serde_json::to_string(trade)?;
        let channel = format!("market:trades:{}", trade.symbol.to_lowercase());
        self.conn.publish(channel, payload).await?;
        Ok(())
    }

    pub async fn publish_metrics(&mut self, metrics: &OrderBookMetrics) -> Result<()> {
        let payload = serde_json::to_string(metrics)?;
        let channel = format!("market:metrics:{}", metrics.symbol.to_lowercase());
        self.conn.publish(channel, payload).await?;
        Ok(())
    }
}