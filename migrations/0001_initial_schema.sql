CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE IF NOT EXISTS market_ticks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    symbol VARCHAR(32) NOT NULL,
    price NUMERIC(20, 8) NOT NULL,
    quantity NUMERIC(20, 8) NOT NULL,
    is_buyer_maker BOOLEAN NOT NULL,
    exchange_ts TIMESTAMPTZ NOT NULL,
    received_ts TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_market_ticks_symbol_ts ON market_ticks (symbol, exchange_ts DESC);

CREATE TABLE IF NOT EXISTS orderbook_metrics (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    symbol VARCHAR(32) NOT NULL,
    bid_price NUMERIC(20, 8) NOT NULL,
    ask_price NUMERIC(20, 8) NOT NULL,
    spread_bps DOUBLE PRECISION NOT NULL,
    mid_price NUMERIC(20, 8) NOT NULL,
    micro_price NUMERIC(20, 8) NOT NULL,
    imbalance DOUBLE PRECISION NOT NULL,
    snapshot_ts TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_orderbook_metrics_symbol_ts ON orderbook_metrics (symbol, snapshot_ts DESC);