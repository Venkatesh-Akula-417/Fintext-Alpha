-- ═══════════════════════════════════════════════════════════════════════════════
-- FinText-Alpha-Vectorizer — PostgreSQL / TimescaleDB Migration 06
-- Problem #14: Tenant API-Key Self-Service Lifecycle (Idempotent)
-- ═══════════════════════════════════════════════════════════════════════════════
-- Supports self-service tenant API key creation, revocation, and rotation with:
-- 1. Lifecycle columns: revoked_at, expires_at, rotated_from, rotation_status, last_seen_utc
-- 2. Fast lookup index on key_hash for authentication verification
-- 3. Filtered index on (org_id) WHERE revoked_at IS NULL for instant quota & active count checks
-- 4. Composite index on (org_id, created_at DESC) for key management listings
-- ═══════════════════════════════════════════════════════════════════════════════

DO $$
BEGIN
    -- 1. Ensure revoked_at column
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'api_keys' AND column_name = 'revoked_at'
    ) THEN
        ALTER TABLE api_keys ADD COLUMN revoked_at TIMESTAMPTZ NULL;
    END IF;

    -- 2. Ensure expires_at column
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'api_keys' AND column_name = 'expires_at'
    ) THEN
        ALTER TABLE api_keys ADD COLUMN expires_at TIMESTAMPTZ NULL;
    END IF;

    -- 3. Ensure rotated_from column
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'api_keys' AND column_name = 'rotated_from'
    ) THEN
        ALTER TABLE api_keys ADD COLUMN rotated_from UUID NULL;
    END IF;

    -- 4. Ensure rotation_status column
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'api_keys' AND column_name = 'rotation_status'
    ) THEN
        ALTER TABLE api_keys ADD COLUMN rotation_status TEXT NOT NULL DEFAULT 'none';
    END IF;

    -- 5. Ensure last_seen_utc column
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'api_keys' AND column_name = 'last_seen_utc'
    ) THEN
        ALTER TABLE api_keys ADD COLUMN last_seen_utc TIMESTAMPTZ NULL;
    END IF;

    -- 6. Ensure fast index on key_hash for high-speed authentication verification
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes 
        WHERE tablename = 'api_keys' AND indexname = 'idx_api_keys_key_hash'
    ) THEN
        CREATE INDEX idx_api_keys_key_hash ON api_keys(key_hash);
    END IF;

    -- 7. Ensure partial index for active keys per tenant (active count <= 10 enforcement)
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes 
        WHERE tablename = 'api_keys' AND indexname = 'idx_api_keys_org_active'
    ) THEN
        CREATE INDEX idx_api_keys_org_active ON api_keys(org_id) WHERE revoked_at IS NULL;
    END IF;

    -- 8. Ensure listing index on (org_id, created_at DESC)
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes 
        WHERE tablename = 'api_keys' AND indexname = 'idx_api_keys_org_created_at'
    ) THEN
        CREATE INDEX idx_api_keys_org_created_at ON api_keys(org_id, created_at DESC);
    END IF;
END $$;
