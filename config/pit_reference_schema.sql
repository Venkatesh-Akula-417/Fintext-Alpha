-- ═══════════════════════════════════════════════════════════════════════════════
-- FinText-Alpha-Vectorizer — PostgreSQL Bi-Temporal Point-in-Time (PIT) Schema
-- Eliminates Single Point of Failure (SPOF) of static JSON configuration files.
-- Supports As-Of Historical Queries, SCD Type 2 Updates & Audit Replay.
-- ═══════════════════════════════════════════════════════════════════════════════

-- 1. Delisted Securities Bi-Temporal Table
CREATE TABLE IF NOT EXISTS pit_delisted_securities (
    id BIGSERIAL PRIMARY KEY,
    ticker TEXT NOT NULL,
    name TEXT,
    delisting_date DATE NOT NULL,
    reason TEXT,
    final_price_usd DOUBLE PRECISION,
    last_close_usd DOUBLE PRECISION,
    delisting_return DOUBLE PRECISION,
    sec_form25_date TIMESTAMPTZ,
    announcement_date TIMESTAMPTZ,
    valid_from TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    valid_to TIMESTAMPTZ,
    is_current BOOLEAN NOT NULL DEFAULT TRUE,
    source TEXT DEFAULT 'SEC',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_pit_delisted_ticker ON pit_delisted_securities(ticker);
CREATE INDEX IF NOT EXISTS idx_pit_delisted_current ON pit_delisted_securities(is_current) WHERE is_current = TRUE;
CREATE INDEX IF NOT EXISTS idx_pit_delisted_valid ON pit_delisted_securities(ticker, valid_from, valid_to);
CREATE INDEX IF NOT EXISTS idx_pit_delisted_date ON pit_delisted_securities(delisting_date);

-- 2. Ticker History & Symbol Lineage Bi-Temporal Table (SCD Type 2)
CREATE TABLE IF NOT EXISTS pit_ticker_history (
    id BIGSERIAL PRIMARY KEY,
    entity_id TEXT NOT NULL,
    entity_name TEXT,
    cik TEXT,
    figi TEXT,
    isin TEXT,
    ticker TEXT NOT NULL,
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    valid_from TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    valid_to TIMESTAMPTZ,
    is_current BOOLEAN NOT NULL DEFAULT TRUE,
    source TEXT DEFAULT 'SEC',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_pit_ticker_history_ticker ON pit_ticker_history(ticker);
CREATE INDEX IF NOT EXISTS idx_pit_ticker_history_entity ON pit_ticker_history(entity_id);
CREATE INDEX IF NOT EXISTS idx_pit_ticker_history_cik ON pit_ticker_history(cik);
CREATE INDEX IF NOT EXISTS idx_pit_ticker_history_current ON pit_ticker_history(is_current) WHERE is_current = TRUE;
CREATE INDEX IF NOT EXISTS idx_pit_ticker_history_valid ON pit_ticker_history(ticker, valid_from, valid_to);

-- 3. Corporate Actions Bi-Temporal Table
CREATE TABLE IF NOT EXISTS pit_corporate_actions (
    id BIGSERIAL PRIMARY KEY,
    ticker TEXT NOT NULL,
    action_date DATE NOT NULL,
    action_type TEXT NOT NULL,
    split_ratio DOUBLE PRECISION,
    dividend_per_share DOUBLE PRECISION,
    description TEXT,
    valid_from TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    valid_to TIMESTAMPTZ,
    is_current BOOLEAN NOT NULL DEFAULT TRUE,
    source TEXT DEFAULT 'MANUAL',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_pit_corp_actions_ticker ON pit_corporate_actions(ticker);
CREATE INDEX IF NOT EXISTS idx_pit_corp_actions_current ON pit_corporate_actions(is_current) WHERE is_current = TRUE;
CREATE INDEX IF NOT EXISTS idx_pit_corp_actions_valid ON pit_corporate_actions(ticker, valid_from, valid_to);
CREATE INDEX IF NOT EXISTS idx_pit_corp_actions_date ON pit_corporate_actions(action_date);

-- 4. Index Membership Bi-Temporal Table (S&P 500, Custom Universes)
CREATE TABLE IF NOT EXISTS pit_index_membership (
    id BIGSERIAL PRIMARY KEY,
    ticker TEXT NOT NULL,
    index_name TEXT NOT NULL,
    company_name TEXT,
    join_date DATE NOT NULL,
    leave_date DATE,
    reason TEXT,
    valid_from TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    valid_to TIMESTAMPTZ,
    is_current BOOLEAN NOT NULL DEFAULT TRUE,
    source TEXT DEFAULT 'MANUAL',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_pit_index_member_ticker ON pit_index_membership(ticker);
CREATE INDEX IF NOT EXISTS idx_pit_index_member_index ON pit_index_membership(index_name);
CREATE INDEX IF NOT EXISTS idx_pit_index_member_current ON pit_index_membership(is_current) WHERE is_current = TRUE;
CREATE INDEX IF NOT EXISTS idx_pit_index_member_valid ON pit_index_membership(ticker, index_name, valid_from, valid_to);
