-- ═══════════════════════════════════════════════════════════════════════════════
-- FinText Alpha Vectorizer — TimescaleDB Time-Series Sentiment Schema
-- Phase 1 Migration: Hypertable with SCD Type 2 Point-in-Time Revision History
-- ═══════════════════════════════════════════════════════════════════════════════

-- 1. Enable TimescaleDB extension if available in PostgreSQL environment
CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;

-- 2. Create base table for time-series sentiment records with SCD Type 2 columns
CREATE TABLE IF NOT EXISTS sentiment_records (
    id BIGSERIAL,
    ticker TEXT NOT NULL,
    published_utc TIMESTAMPTZ NOT NULL,
    ingested_utc TIMESTAMPTZ NOT NULL,
    db_commit_utc TIMESTAMPTZ NOT NULL,
    source TEXT,
    title TEXT,
    sentiment_score DOUBLE PRECISION,
    sentiment_label TEXT,
    confidence DOUBLE PRECISION,
    data_quality_score DOUBLE PRECISION,
    vpin DOUBLE PRECISION,
    gamma_exposure DOUBLE PRECISION,
    valid_from TIMESTAMPTZ NOT NULL,
    valid_to TIMESTAMPTZ,
    revision_number INTEGER DEFAULT 1,
    is_current BOOLEAN DEFAULT TRUE,
    PRIMARY KEY (id, published_utc)
);

-- 3. Convert table to TimescaleDB hypertable partitioned on published_utc (7-day chunks)
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_extension WHERE extname = 'timescaledb'
    ) THEN
        PERFORM create_hypertable(
            'sentiment_records',
            'published_utc',
            chunk_time_interval => INTERVAL '7 days',
            if_not_exists => TRUE
        );
    END IF;
END $$;

-- 4. Create performance indexes for time-series queries and point-in-time validity lookups
CREATE INDEX IF NOT EXISTS idx_sentiment_records_ticker_published 
    ON sentiment_records (ticker, published_utc DESC);

CREATE INDEX IF NOT EXISTS idx_sentiment_records_valid_from 
    ON sentiment_records (valid_from);

CREATE INDEX IF NOT EXISTS idx_sentiment_records_valid_to 
    ON sentiment_records (valid_to);

CREATE INDEX IF NOT EXISTS idx_sentiment_records_is_current 
    ON sentiment_records (is_current);

CREATE INDEX IF NOT EXISTS idx_sentiment_records_pit 
    ON sentiment_records (ticker, published_utc, valid_from, valid_to);

CREATE INDEX IF NOT EXISTS idx_sentiment_records_source 
    ON sentiment_records (source);
