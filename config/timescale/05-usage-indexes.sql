-- ═══════════════════════════════════════════════════════════════════════════════
-- FinText-Alpha-Vectorizer — PostgreSQL / TimescaleDB Migration 05
-- Problem #11: Per-Tenant Usage & API-Key Audit Indexes (Idempotent)
-- ═══════════════════════════════════════════════════════════════════════════════
-- Optimizes per-tenant usage queries and API key lifecycle audit lookups.
-- 1. Composite index on usage_events(org_id, created_at DESC) for sub-second
--    monthly request aggregation and daily series generation.
-- 2. Additive column api_keys.last_seen_utc for key audit visibility.
-- 3. Composite index on api_keys(org_id, created_at DESC) for audit list queries.
-- ═══════════════════════════════════════════════════════════════════════════════

DO $$
BEGIN
    -- 1. Ensure composite index on usage_events for high-speed monthly tenant aggregation
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes 
        WHERE tablename = 'usage_events' AND indexname = 'idx_usage_events_org_created_at'
    ) THEN
        CREATE INDEX idx_usage_events_org_created_at ON usage_events(org_id, created_at DESC);
    END IF;

    -- 2. Ensure last_seen_utc column on api_keys for audit visibility
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'api_keys' AND column_name = 'last_seen_utc'
    ) THEN
        ALTER TABLE api_keys ADD COLUMN last_seen_utc TIMESTAMPTZ NULL;
    END IF;

    -- 3. Ensure composite index on api_keys for tenant key listings
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes 
        WHERE tablename = 'api_keys' AND indexname = 'idx_api_keys_org_created_at'
    ) THEN
        CREATE INDEX idx_api_keys_org_created_at ON api_keys(org_id, created_at DESC);
    END IF;
END $$;
