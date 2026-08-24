-- 1. Raw News Table Enhancement
CREATE TABLE IF NOT EXISTS raw_news_v2 (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    source_id VARCHAR(64) NOT NULL,
    external_id VARCHAR(256) NOT NULL,
    title TEXT NOT NULL,
    body TEXT,
    url TEXT,
    source_ts TIMESTAMPTZ NOT NULL,
    received_ts TIMESTAMPTZ NOT NULL,
    processed_ts TIMESTAMPTZ,
    available_ts TIMESTAMPTZ NOT NULL,
    hash_signature VARCHAR(64) NOT NULL,
    raw_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    CONSTRAINT uq_source_external_v2 UNIQUE (source_id, external_id)
);
CREATE INDEX idx_raw_news_v2_available ON raw_news_v2 (available_ts DESC);

-- 2. Macro / Economic Calendar Table
CREATE TABLE IF NOT EXISTS macro_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_name VARCHAR(128) NOT NULL,
    country VARCHAR(16) NOT NULL,
    currency VARCHAR(16) NOT NULL,
    actual NUMERIC(20, 6),
    forecast NUMERIC(20, 6),
    previous NUMERIC(20, 6),
    surprise_z_score DOUBLE PRECISION,
    source_ts TIMESTAMPTZ NOT NULL,
    available_ts TIMESTAMPTZ NOT NULL
);
CREATE INDEX idx_macro_events_ts ON macro_events (available_ts DESC);

-- 3. Market Events (Correlated cluster of news/macro)
CREATE TABLE IF NOT EXISTS market_events_v2 (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_type VARCHAR(64) NOT NULL, -- e.g. "RATE_CUT", "REGULATORY_APPROVAL", "SECURITY_BREACH"
    primary_asset VARCHAR(32) NOT NULL,
    relevance_score DOUBLE PRECISION NOT NULL CHECK (relevance_score >= 0.0 AND relevance_score <= 1.0),
    novelty_score DOUBLE PRECISION NOT NULL CHECK (novelty_score >= 0.0 AND novelty_score <= 1.0),
    summary TEXT,
    first_seen_ts TIMESTAMPTZ NOT NULL,
    available_ts TIMESTAMPTZ NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX idx_market_events_v2_asset ON market_events_v2 (primary_asset, available_ts DESC);