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
        sqlx::query(
            r#"
            INSERT INTO market_ticks (symbol, price, quantity, is_buyer_maker, exchange_ts, received_ts)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(&tick.symbol)
        .bind(&tick.price)
        .bind(&tick.quantity)
        .bind(&tick.is_buyer_maker)
        .bind(&tick.exchange_ts)
        .bind(&tick.received_ts)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn insert_metrics(&self, metrics: &OrderBookMetrics) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO orderbook_metrics (symbol, bid_price, ask_price, spread_bps, mid_price, micro_price, imbalance, snapshot_ts)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(&metrics.symbol)
        .bind(&metrics.best_bid)
        .bind(&metrics.best_ask)
        .bind(&metrics.spread_bps)
        .bind(&metrics.mid_price)
        .bind(&metrics.micro_price)
        .bind(&metrics.imbalance_top10)
        .bind(&metrics.timestamp)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}