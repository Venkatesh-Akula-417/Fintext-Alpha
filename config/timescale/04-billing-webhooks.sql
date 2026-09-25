-- ═══════════════════════════════════════════════════════════════════════════════
-- FinText-Alpha-Vectorizer — PostgreSQL / TimescaleDB Migration 04
-- Problem #8: Stripe Billing Webhooks, Dunning State Machine & Reconciliation
-- ═══════════════════════════════════════════════════════════════════════════════
-- Architecture & RLS Classification:
-- 1. Table `subscriptions` (CLASS-T, Multi-Tenant Confined):
--    - RLS already ENABLED & FORCED in Migration 03 (`tenant_iso_subscriptions`).
--    - Additive columns:
--      * dunning_fail_count INT DEFAULT 0 (consecutive payment failure counter)
--      * grace_until_utc TIMESTAMPTZ NULL (72h dunning grace window expiration)
--      * stripe_customer_id TEXT, stripe_subscription_id TEXT (if missing)
-- 2. Table `billing_events` (CLASS-O, Operational System Log):
--    - Ingests inbound raw Stripe webhook deliveries before tenant authentication.
--    - Idempotency key: stripe_event_id (UNIQUE).
--    - Role permissions:
--      * fintext_app: SELECT, INSERT, UPDATE (API gateway webhook intake)
--      * fintext_ingest: REVOKE ALL (market ingestion daemon never touches billing)
--      * fintext (admin/ops): ALL PRIVILEGES
-- ═══════════════════════════════════════════════════════════════════════════════

-- ─────────────────────────────────────────────────────────────────────────────
-- 1. Subscriptions Table Additive Columns & Indexes (CLASS-T)
-- ─────────────────────────────────────────────────────────────────────────────
DO $$
BEGIN
    -- Ensure subscriptions table exists
    CREATE TABLE IF NOT EXISTS subscriptions (
        id UUID PRIMARY KEY,
        user_id UUID NOT NULL,
        stripe_customer_id TEXT,
        stripe_subscription_id TEXT,
        plan_id TEXT NOT NULL DEFAULT 'free',
        status TEXT NOT NULL DEFAULT 'active',
        current_period_start TIMESTAMPTZ,
        current_period_end TIMESTAMPTZ,
        created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
        updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
        org_id VARCHAR(64) NULL
    );

    -- Add dunning failure counter
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'subscriptions' AND column_name = 'dunning_fail_count'
    ) THEN
        ALTER TABLE subscriptions ADD COLUMN dunning_fail_count INT NOT NULL DEFAULT 0;
    END IF;

    -- Add dunning grace expiration timestamp
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'subscriptions' AND column_name = 'grace_until_utc'
    ) THEN
        ALTER TABLE subscriptions ADD COLUMN grace_until_utc TIMESTAMPTZ NULL;
    END IF;

    -- Ensure stripe customer and subscription ID columns exist
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'subscriptions' AND column_name = 'stripe_customer_id'
    ) THEN
        ALTER TABLE subscriptions ADD COLUMN stripe_customer_id TEXT NULL;
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'subscriptions' AND column_name = 'stripe_subscription_id'
    ) THEN
        ALTER TABLE subscriptions ADD COLUMN stripe_subscription_id TEXT NULL;
    END IF;

    -- Ensure org_id exists
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'subscriptions' AND column_name = 'org_id'
    ) THEN
        ALTER TABLE subscriptions ADD COLUMN org_id VARCHAR(64) NULL;
    END IF;
END $$;

-- Performance Indexes on subscriptions
CREATE INDEX IF NOT EXISTS idx_subscriptions_stripe_cust ON subscriptions(stripe_customer_id);
CREATE INDEX IF NOT EXISTS idx_subscriptions_stripe_sub ON subscriptions(stripe_subscription_id);
CREATE INDEX IF NOT EXISTS idx_subscriptions_status_grace ON subscriptions(status, grace_until_utc);
CREATE INDEX IF NOT EXISTS idx_subscriptions_org_status ON subscriptions(org_id, status);

-- ─────────────────────────────────────────────────────────────────────────────
-- 2. Billing Events Idempotency & Audit Table (CLASS-O)
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS billing_events (
    id UUID PRIMARY KEY,
    stripe_event_id TEXT UNIQUE NOT NULL,
    event_type TEXT NOT NULL,
    created_utc TIMESTAMPTZ NOT NULL,
    processed_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    status TEXT NOT NULL DEFAULT 'processed',
    error_text TEXT NULL,
    payload JSONB NULL,
    org_id VARCHAR(64) NULL
);

-- Performance & Idempotency Indexes on billing_events
CREATE UNIQUE INDEX IF NOT EXISTS idx_billing_events_stripe_id ON billing_events(stripe_event_id);
CREATE INDEX IF NOT EXISTS idx_billing_events_type ON billing_events(event_type);
CREATE INDEX IF NOT EXISTS idx_billing_events_processed ON billing_events(processed_utc DESC);
CREATE INDEX IF NOT EXISTS idx_billing_events_org ON billing_events(org_id);
CREATE INDEX IF NOT EXISTS idx_billing_events_status ON billing_events(status);

-- ─────────────────────────────────────────────────────────────────────────────
-- 3. Least-Privilege Role Grants & Invariants
-- ─────────────────────────────────────────────────────────────────────────────
DO $$
BEGIN
    -- Grants for fintext_app (Axum API gateway runtime)
    IF EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'fintext_app') THEN
        GRANT SELECT, INSERT, UPDATE ON billing_events TO fintext_app;
        GRANT SELECT, INSERT, UPDATE, DELETE ON subscriptions TO fintext_app;
    END IF;

    -- Strict Isolation: Ingestion daemon must NEVER have access to commercial billing data
    IF EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'fintext_ingest') THEN
        REVOKE ALL ON billing_events FROM fintext_ingest;
        REVOKE ALL ON subscriptions FROM fintext_ingest;
    END IF;
END $$;

-- ─────────────────────────────────────────────────────────────────────────────
-- 4. Verification View: billing_status_summary
-- ─────────────────────────────────────────────────────────────────────────────
CREATE OR REPLACE VIEW billing_status_summary AS
SELECT
    COUNT(*) AS total_subscriptions,
    COUNT(*) FILTER (WHERE status = 'active') AS active_count,
    COUNT(*) FILTER (WHERE status = 'past_due') AS past_due_count,
    COUNT(*) FILTER (WHERE status = 'canceled') AS canceled_count,
    COUNT(*) FILTER (WHERE status = 'past_due' AND grace_until_utc < NOW()) AS expired_grace_count,
    (SELECT COUNT(*) FROM billing_events) AS total_webhook_events_processed
FROM subscriptions;

COMMENT ON TABLE billing_events IS 'CLASS-O Operational Audit: Inbound Stripe webhook event stream with strict idempotency';
COMMENT ON COLUMN subscriptions.dunning_fail_count IS 'Count of consecutive invoice payment failures under active dunning';
COMMENT ON COLUMN subscriptions.grace_until_utc IS '72-hour grace period deadline before automated service suspension';
