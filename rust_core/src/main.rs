use rust_core::config::Settings;
use rust_core::domain::book::OrderBook;
use rust_core::market_data::binance::{BinanceClient, MarketEvent};
use rust_core::storage::postgres::PostgresStore;
use rust_core::storage::redis::RedisPublisher;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize Structured Logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .json()
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting MI-EDTE Market Data Engine (Milestone 1 & 2)...");

    // 2. Load Configuration
    let settings = Settings::new()?;
    info!("Configuration successfully loaded.");

    // 3. Initialize Storages
    let mut redis_pub = match RedisPublisher::new(&settings.storage.redis_url).await {
        Ok(publ) => {
            info!("Connected to Redis at {}", settings.storage.redis_url);
            Some(publ)
        }
        Err(e) => {
            error!("Failed to connect to Redis: {}. Continuing in headless mode.", e);
            None
        }
    };

    let pg_store = if settings.storage.enable_db_persistence {
        match PostgresStore::new(&settings.storage.database_url).await {
            Ok(store) => {
                info!("Connected to PostgreSQL / TimescaleDB.");
                Some(store)
            }
            Err(e) => {
                error!("Failed to connect to Postgres: {}. Persistence disabled.", e);
                None
            }
        }
    } else {
        None
    };

    // 4. Initialize In-Memory Order Books
    let mut order_books: HashMap<String, OrderBook> = HashMap::new();
    for s in &settings.market_data.symbols {
        order_books.insert(s.to_uppercase(), OrderBook::new(s.to_uppercase()));
    }

    // 5. Setup Channels & Ingestion Client
    let (event_tx, mut event_rx) = mpsc::channel::<MarketEvent>(settings.market_data.channel_buffer_size);
    let binance_client = BinanceClient::new(settings.market_data.clone(), event_tx);

    // Initial Snapshot fetch for all symbols
    for s in &settings.market_data.symbols {
        let sym_upper = s.to_uppercase();
        match binance_client.fetch_snapshot(&sym_upper).await {
            Ok((last_id, bids, asks)) => {
                if let Some(book) = order_books.get_mut(&sym_upper) {
                    book.apply_snapshot(last_id, bids, asks);
                    info!(symbol = %sym_upper, last_update_id = last_id, "L2 OrderBook Snapshot applied.");
                }
            }
            Err(e) => {
                error!(symbol = %sym_upper, error = %e, "Failed to load initial snapshot. Will rebuild on stream.");
            }
        }
    }

    // 6. Spawn Background WebSocket Ingestion
    let binance_worker = tokio::spawn(async move {
        binance_client.run_stream().await;
    });

    // 7. Core Event Processing Loop
    let main_loop = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                MarketEvent::Trade(trade) => {
                    if let Some(ref mut r) = redis_pub {
                        let _ = r.publish_trade(&trade).await;
                    }
                    if let Some(ref pg) = pg_store {
                        let pg_clone = pg.clone();
                        tokio::spawn(async move {
                            if let Err(e) = pg_clone.insert_tick(&trade).await {
                                error!("Failed to write trade to Postgres: {}", e);
                            }
                        });
                    }
                }
                MarketEvent::Depth(delta) => {
                    let sym_upper = delta.symbol.to_uppercase();
                    if let Some(book) = order_books.get_mut(&sym_upper) {
                        match book.apply_delta(&delta) {
                            Ok(_) => {
                                if let Some(metrics) = book.compute_metrics() {
                                    if let Some(ref mut r) = redis_pub {
                                        let _ = r.publish_metrics(&metrics).await;
                                    }
                                    if let Some(ref pg) = pg_store {
                                        let pg_clone = pg.clone();
                                        tokio::spawn(async move {
                                            if let Err(e) = pg_clone.insert_metrics(&metrics).await {
                                                error!("Failed to write metrics to Postgres: {}", e);
                                            }
                                        });
                                    }
                                }
                            }
                            Err(e) => {
                                error!(symbol = %sym_upper, error = %e, "OrderBook gap/desync detected!");
                            }
                        }
                    }
                }
            }
        }
    });

    // 8. Graceful Shutdown on CTRL+C
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Shutdown signal received. Terminating gracefully...");
        }
        _ = binance_worker => {}
        _ = main_loop => {}
    }

    info!("MI-EDTE Engine stopped.");
    Ok(())
}