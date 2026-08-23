use crate::domain::types::{OrderBookMetrics, TradeTick};
use crate::error::Result;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

#[derive(Clone)]
pub struct PostgresStore {
    pool: PgPool,
}

impl PostgresStore {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn insert_tick(&self, tick: &TradeTick) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO market_ticks (symbol, price, quantity, is_buyer_maker, exchange_ts, received_ts)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            tick.symbol,
            tick.price,
            tick.quantity,
            tick.is_buyer_maker,
            tick.exchange_ts,
            tick.received_ts
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn insert_metrics(&self, metrics: &OrderBookMetrics) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO orderbook_metrics (symbol, bid_price, ask_price, spread_bps, mid_price, micro_price, imbalance, snapshot_ts)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            metrics.symbol,
            metrics.best_bid,
            metrics.best_ask,
            metrics.spread_bps,
            metrics.mid_price,
            metrics.micro_price,
            metrics.imbalance_top10,
            metrics.timestamp
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}