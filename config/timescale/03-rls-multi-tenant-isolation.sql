-- ═══════════════════════════════════════════════════════════════════════════════
-- FinText-Alpha-Vectorizer — PostgreSQL Row-Level Security (RLS) Migration
-- Problem #5: Multi-Tenant Database Confinement & Role-Based Isolation
-- ═══════════════════════════════════════════════════════════════════════════════
-- Architecture:
-- 1. Roles:
--    - fintext_admin (Owner / Superuser / Ops): BYPASSRLS for backups/migrations.
--    - fintext_app (API Gateway / DML): NOBYPASSRLS, strictly confined by RLS.
--    - fintext_ingest (Market Data Daemon): BYPASSRLS for market tables.
-- 2. Confinement:
--    - RLS ENABLED and FORCED on every TENANT-OWNED (CLASS-T) table.
--    - Deny-by-default: if `app.current_org_id` GUC is unset or empty, 0 rows returned.
--    - Transactions set `SET LOCAL app.current_org_id = 'org_id'` to confine scope.
-- ═══════════════════════════════════════════════════════════════════════════════

-- ─────────────────────────────────────────────────────────────────────────────
-- 1. Create Dedicated Least-Privilege Roles
-- ─────────────────────────────────────────────────────────────────────────────
DO $$ 
BEGIN 
    -- Ensure fintext_app role exists (API execution, strictly non-bypass RLS)
    IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'fintext_app') THEN 
        CREATE ROLE fintext_app WITH LOGIN PASSWORD 'fintext_app_dev_pw' NOBYPASSRLS; 
    ELSE
        ALTER ROLE fintext_app WITH NOBYPASSRLS;
    END IF; 

    -- Ensure fintext_ingest role exists (market data writer)
    IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'fintext_ingest') THEN 
        CREATE ROLE fintext_ingest WITH LOGIN PASSWORD 'fintext_ingest_dev_pw' BYPASSRLS; 
    ELSE
        ALTER ROLE fintext_ingest WITH BYPASSRLS;
    END IF; 

    -- Ensure fintext admin / superuser has BYPASSRLS for maintenance and backups
    IF EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'fintext') THEN 
        ALTER ROLE fintext WITH BYPASSRLS;
    END IF;
END $$;

-- ─────────────────────────────────────────────────────────────────────────────
-- 2. Schema DDL: Ensure Tenant-Owned Tables & Columns Exist
-- ─────────────────────────────────────────────────────────────────────────────

-- Organizations (Root Tenant Entity)
CREATE TABLE IF NOT EXISTS organizations (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    created_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Organization Memberships
CREATE TABLE IF NOT EXISTS organization_members (
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL,
    role TEXT NOT NULL DEFAULT 'member',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (org_id, user_id)
);
CREATE INDEX IF NOT EXISTS idx_org_members_user ON organization_members(user_id);
CREATE INDEX IF NOT EXISTS idx_org_members_org ON organization_members(org_id);

-- Users & Credentials
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
    email TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'institutional',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_active BOOLEAN NOT NULL DEFAULT TRUE
);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);

-- API Keys
CREATE TABLE IF NOT EXISTS api_keys (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL DEFAULT 'Default',
    key_hash TEXT NOT NULL,
    prefix TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at TIMESTAMPTZ NULL,
    expires_at TIMESTAMPTZ NULL,
    rotated_from UUID NULL,
    rotation_status TEXT NOT NULL DEFAULT 'none',
    org_id VARCHAR(64) NULL
);
ALTER TABLE api_keys ADD COLUMN IF NOT EXISTS org_id VARCHAR(64) NULL;
CREATE INDEX IF NOT EXISTS idx_api_keys_org_id ON api_keys(org_id);

-- Audit Logs
CREATE TABLE IF NOT EXISTS audit_logs (
    id UUID PRIMARY KEY,
    org_id UUID NULL,
    user_id TEXT NOT NULL,
    action TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NULL,
    details JSONB NOT NULL DEFAULT '{}'::jsonb,
    ip_address TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_audit_logs_org_time ON audit_logs(org_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_logs_user_time ON audit_logs(user_id, created_at DESC);

-- Usage Events (Metering)
CREATE TABLE IF NOT EXISTS usage_events (
    id BIGSERIAL PRIMARY KEY,
    user_id TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    method TEXT NOT NULL,
    status_code SMALLINT NOT NULL,
    latency_ms REAL NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    org_id VARCHAR(64) NULL
);
ALTER TABLE usage_events ADD COLUMN IF NOT EXISTS org_id VARCHAR(64) NULL;
CREATE INDEX IF NOT EXISTS idx_usage_events_org_id ON usage_events(org_id);
CREATE INDEX IF NOT EXISTS idx_usage_events_user_time ON usage_events(user_id, created_at DESC);

-- Custom Universes
CREATE TABLE IF NOT EXISTS universes (
    id UUID PRIMARY KEY,
    user_id TEXT NOT NULL,
    name TEXT NOT NULL,
    tickers JSONB NOT NULL DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    org_id VARCHAR(64) NULL
);
ALTER TABLE universes ADD COLUMN IF NOT EXISTS org_id VARCHAR(64) NULL;
CREATE INDEX IF NOT EXISTS idx_universes_org_id ON universes(org_id);

-- Outbound Webhooks
CREATE TABLE IF NOT EXISTS webhooks (
    id UUID PRIMARY KEY,
    user_id TEXT NOT NULL,
    url TEXT NOT NULL,
    events JSONB NOT NULL DEFAULT '[]',
    secret TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    org_id VARCHAR(64) NULL
);
ALTER TABLE webhooks ADD COLUMN IF NOT EXISTS org_id VARCHAR(64) NULL;
CREATE INDEX IF NOT EXISTS idx_webhooks_org_id ON webhooks(org_id);

-- Data Retention Policies
CREATE TABLE IF NOT EXISTS data_retention_policies (
    id UUID PRIMARY KEY,
    org_id UUID NULL,
    user_id TEXT NOT NULL,
    data_category TEXT NOT NULL,
    retention_days INTEGER NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_retention_org ON data_retention_policies(org_id);
CREATE INDEX IF NOT EXISTS idx_retention_user ON data_retention_policies(user_id);

-- Model Retraining Jobs
CREATE TABLE IF NOT EXISTS model_retraining_jobs (
    id UUID PRIMARY KEY,
    org_id UUID NULL,
    user_id TEXT NOT NULL,
    model_type TEXT NOT NULL DEFAULT 'sentiment',
    trigger_type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    config JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ NULL,
    completed_at TIMESTAMPTZ NULL,
    metrics JSONB NULL,
    error_message TEXT NULL
);
CREATE INDEX IF NOT EXISTS idx_retraining_org ON model_retraining_jobs(org_id);

-- Streaming Kafka Credentials
CREATE TABLE IF NOT EXISTS kafka_credentials (
    id UUID PRIMARY KEY,
    user_id TEXT NOT NULL,
    topic TEXT NOT NULL,
    consumer_group TEXT NOT NULL,
    username TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    broker_address TEXT NOT NULL,
    issued_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    org_id VARCHAR(64) NULL
);
ALTER TABLE kafka_credentials ADD COLUMN IF NOT EXISTS org_id VARCHAR(64) NULL;
CREATE INDEX IF NOT EXISTS idx_kafka_creds_org ON kafka_credentials(org_id);

-- IP Whitelist (Network Ingress Confinement)
CREATE TABLE IF NOT EXISTS ip_whitelist (
    id UUID PRIMARY KEY,
    user_id VARCHAR(128) NOT NULL,
    ip_or_cidr TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    org_id VARCHAR(64) NULL
);
ALTER TABLE ip_whitelist ADD COLUMN IF NOT EXISTS org_id VARCHAR(64) NULL;
CREATE INDEX IF NOT EXISTS idx_ip_whitelist_org ON ip_whitelist(org_id);

-- Chat Alerts Subscriptions
CREATE TABLE IF NOT EXISTS chat_alert_subscriptions (
    id UUID PRIMARY KEY,
    user_id VARCHAR(255) NOT NULL,
    channel_type TEXT NOT NULL,
    channel_target TEXT NOT NULL,
    event_types JSONB NOT NULL DEFAULT '[]',
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    org_id VARCHAR(64) NULL
);
ALTER TABLE chat_alert_subscriptions ADD COLUMN IF NOT EXISTS org_id VARCHAR(64) NULL;
CREATE INDEX IF NOT EXISTS idx_chat_alerts_org ON chat_alert_subscriptions(org_id);

-- Polling Webhooks
CREATE TABLE IF NOT EXISTS polling_webhooks (
    id UUID PRIMARY KEY,
    user_id VARCHAR(255) NOT NULL,
    name VARCHAR(255) NOT NULL,
    url TEXT NOT NULL,
    interval_seconds INTEGER NOT NULL,
    query_type VARCHAR(64) NOT NULL,
    query_params JSONB NOT NULL DEFAULT '{}'::jsonb,
    secret VARCHAR(255) NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    last_triggered_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    org_id VARCHAR(64) NULL
);
ALTER TABLE polling_webhooks ADD COLUMN IF NOT EXISTS org_id VARCHAR(64) NULL;
CREATE INDEX IF NOT EXISTS idx_polling_webhooks_org ON polling_webhooks(org_id);

-- Subscriptions (Billing)
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
ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS org_id VARCHAR(64) NULL;
CREATE INDEX IF NOT EXISTS idx_subscriptions_org ON subscriptions(org_id);

-- Email Digest Subscriptions & History
CREATE TABLE IF NOT EXISTS email_digest_subscriptions (
    id UUID PRIMARY KEY,
    user_id TEXT NOT NULL UNIQUE,
    frequency TEXT NOT NULL DEFAULT 'daily',
    tickers JSONB NOT NULL DEFAULT '[]',
    sectors JSONB NOT NULL DEFAULT '[]',
    event_types JSONB NOT NULL DEFAULT '[]',
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    org_id VARCHAR(64) NULL
);
ALTER TABLE email_digest_subscriptions ADD COLUMN IF NOT EXISTS org_id VARCHAR(64) NULL;
CREATE INDEX IF NOT EXISTS idx_email_digest_org ON email_digest_subscriptions(org_id);

CREATE TABLE IF NOT EXISTS digest_send_history (
    id UUID PRIMARY KEY,
    subscription_id UUID NOT NULL,
    user_id TEXT NOT NULL,
    recipient_email TEXT NOT NULL,
    subject TEXT NOT NULL,
    content_summary TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'sent',
    sent_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    org_id VARCHAR(64) NULL
);
ALTER TABLE digest_send_history ADD COLUMN IF NOT EXISTS org_id VARCHAR(64) NULL;
CREATE INDEX IF NOT EXISTS idx_digest_history_org ON digest_send_history(org_id);

-- FIX Protocol Orders
CREATE TABLE IF NOT EXISTS fix_orders (
    id UUID PRIMARY KEY,
    user_id TEXT NOT NULL,
    cl_ord_id TEXT NOT NULL,
    order_id TEXT NOT NULL,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL,
    order_type TEXT NOT NULL,
    qty DOUBLE PRECISION NOT NULL,
    filled_qty DOUBLE PRECISION NOT NULL DEFAULT 0,
    avg_price DOUBLE PRECISION,
    status TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    org_id VARCHAR(64) NULL
);
ALTER TABLE fix_orders ADD COLUMN IF NOT EXISTS org_id VARCHAR(64) NULL;
CREATE INDEX IF NOT EXISTS idx_fix_orders_org ON fix_orders(org_id);

-- PIT Mock/Synthetic Backup Tables (if used by test_restore)
CREATE TABLE IF NOT EXISTS instrument_master (
    id SERIAL PRIMARY KEY,
    ticker VARCHAR(16) NOT NULL,
    cik VARCHAR(10),
    figi VARCHAR(12),
    valid_from TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    valid_to TIMESTAMPTZ,
    is_current BOOLEAN DEFAULT TRUE,
    org_id VARCHAR(64) NOT NULL DEFAULT 'org_global'
);
ALTER TABLE instrument_master ADD COLUMN IF NOT EXISTS org_id VARCHAR(64) NOT NULL DEFAULT 'org_global';

CREATE TABLE IF NOT EXISTS filings_raw (
    id SERIAL PRIMARY KEY,
    accession_number VARCHAR(32) NOT NULL,
    ticker VARCHAR(16) NOT NULL,
    filing_type VARCHAR(16) NOT NULL,
    published_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ingested_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    org_id VARCHAR(64) NOT NULL DEFAULT 'org_global'
);
ALTER TABLE filings_raw ADD COLUMN IF NOT EXISTS org_id VARCHAR(64) NOT NULL DEFAULT 'org_global';

CREATE TABLE IF NOT EXISTS filings_normalized (
    id SERIAL PRIMARY KEY,
    filing_id INT NOT NULL,
    ticker VARCHAR(16) NOT NULL,
    cleaned_text TEXT NOT NULL,
    tokens_count INT NOT NULL,
    published_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    org_id VARCHAR(64) NOT NULL DEFAULT 'org_global'
);
ALTER TABLE filings_normalized ADD COLUMN IF NOT EXISTS org_id VARCHAR(64) NOT NULL DEFAULT 'org_global';

-- ─────────────────────────────────────────────────────────────────────────────
-- 3. Enable and Force Row-Level Security on CLASS-T Tables
-- ─────────────────────────────────────────────────────────────────────────────

-- 3.1 Organizations
ALTER TABLE organizations ENABLE ROW LEVEL SECURITY;
ALTER TABLE organizations FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_iso_organizations ON organizations;
CREATE POLICY tenant_iso_organizations ON organizations
    AS PERMISSIVE FOR ALL
    USING (
        id::TEXT = NULLIF(current_setting('app.current_org_id', TRUE), '')
    )
    WITH CHECK (
        id::TEXT = NULLIF(current_setting('app.current_org_id', TRUE), '')
    );

-- 3.2 Organization Members
ALTER TABLE organization_members ENABLE ROW LEVEL SECURITY;
ALTER TABLE organization_members FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_iso_organization_members ON organization_members;
CREATE POLICY tenant_iso_organization_members ON organization_members
    AS PERMISSIVE FOR ALL
    USING (
        org_id::TEXT = NULLIF(current_setting('app.current_org_id', TRUE), '')
    )
    WITH CHECK (
        org_id::TEXT = NULLIF(current_setting('app.current_org_id', TRUE), '')
    );

-- 3.3 Audit Logs
ALTER TABLE audit_logs ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_logs FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_iso_audit_logs ON audit_logs;
CREATE POLICY tenant_iso_audit_logs ON audit_logs
    AS PERMISSIVE FOR ALL
    USING (
        org_id::TEXT = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR (org_id IS NULL AND user_id = NULLIF(current_setting('app.current_org_id', TRUE), ''))
    )
    WITH CHECK (
        org_id::TEXT = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR (org_id IS NULL AND user_id = NULLIF(current_setting('app.current_org_id', TRUE), ''))
    );

-- 3.4 API Keys
ALTER TABLE api_keys ENABLE ROW LEVEL SECURITY;
ALTER TABLE api_keys FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_iso_api_keys ON api_keys;
CREATE POLICY tenant_iso_api_keys ON api_keys
    AS PERMISSIVE FOR ALL
    USING (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id::TEXT = NULLIF(current_setting('app.current_org_id', TRUE), '')
    )
    WITH CHECK (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id::TEXT = NULLIF(current_setting('app.current_org_id', TRUE), '')
    );

-- 3.5 Usage Events
ALTER TABLE usage_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE usage_events FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_iso_usage_events ON usage_events;
CREATE POLICY tenant_iso_usage_events ON usage_events
    AS PERMISSIVE FOR ALL
    USING (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
    )
    WITH CHECK (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
    );

-- 3.6 Universes
ALTER TABLE universes ENABLE ROW LEVEL SECURITY;
ALTER TABLE universes FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_iso_universes ON universes;
CREATE POLICY tenant_iso_universes ON universes
    AS PERMISSIVE FOR ALL
    USING (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
    )
    WITH CHECK (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
    );

-- 3.7 Webhooks
ALTER TABLE webhooks ENABLE ROW LEVEL SECURITY;
ALTER TABLE webhooks FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_iso_webhooks ON webhooks;
CREATE POLICY tenant_iso_webhooks ON webhooks
    AS PERMISSIVE FOR ALL
    USING (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
    )
    WITH CHECK (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
    );

-- 3.8 Data Retention Policies
ALTER TABLE data_retention_policies ENABLE ROW LEVEL SECURITY;
ALTER TABLE data_retention_policies FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_iso_data_retention_policies ON data_retention_policies;
CREATE POLICY tenant_iso_data_retention_policies ON data_retention_policies
    AS PERMISSIVE FOR ALL
    USING (
        org_id::TEXT = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR (org_id IS NULL AND user_id = NULLIF(current_setting('app.current_org_id', TRUE), ''))
    )
    WITH CHECK (
        org_id::TEXT = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR (org_id IS NULL AND user_id = NULLIF(current_setting('app.current_org_id', TRUE), ''))
    );

-- 3.9 Model Retraining Jobs
ALTER TABLE model_retraining_jobs ENABLE ROW LEVEL SECURITY;
ALTER TABLE model_retraining_jobs FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_iso_model_retraining_jobs ON model_retraining_jobs;
CREATE POLICY tenant_iso_model_retraining_jobs ON model_retraining_jobs
    AS PERMISSIVE FOR ALL
    USING (
        org_id::TEXT = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR (org_id IS NULL AND user_id = NULLIF(current_setting('app.current_org_id', TRUE), ''))
    )
    WITH CHECK (
        org_id::TEXT = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR (org_id IS NULL AND user_id = NULLIF(current_setting('app.current_org_id', TRUE), ''))
    );

-- 3.10 Kafka Credentials
ALTER TABLE kafka_credentials ENABLE ROW LEVEL SECURITY;
ALTER TABLE kafka_credentials FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_iso_kafka_credentials ON kafka_credentials;
CREATE POLICY tenant_iso_kafka_credentials ON kafka_credentials
    AS PERMISSIVE FOR ALL
    USING (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
    )
    WITH CHECK (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
    );

-- 3.11 IP Whitelist
ALTER TABLE ip_whitelist ENABLE ROW LEVEL SECURITY;
ALTER TABLE ip_whitelist FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_iso_ip_whitelist ON ip_whitelist;
CREATE POLICY tenant_iso_ip_whitelist ON ip_whitelist
    AS PERMISSIVE FOR ALL
    USING (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
    )
    WITH CHECK (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
    );

-- 3.12 Chat Alert Subscriptions
ALTER TABLE chat_alert_subscriptions ENABLE ROW LEVEL SECURITY;
ALTER TABLE chat_alert_subscriptions FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_iso_chat_alert_subscriptions ON chat_alert_subscriptions;
CREATE POLICY tenant_iso_chat_alert_subscriptions ON chat_alert_subscriptions
    AS PERMISSIVE FOR ALL
    USING (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
    )
    WITH CHECK (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
    );

-- 3.13 Polling Webhooks
ALTER TABLE polling_webhooks ENABLE ROW LEVEL SECURITY;
ALTER TABLE polling_webhooks FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_iso_polling_webhooks ON polling_webhooks;
CREATE POLICY tenant_iso_polling_webhooks ON polling_webhooks
    AS PERMISSIVE FOR ALL
    USING (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
    )
    WITH CHECK (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
    );

-- 3.14 Subscriptions (Billing)
ALTER TABLE subscriptions ENABLE ROW LEVEL SECURITY;
ALTER TABLE subscriptions FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_iso_subscriptions ON subscriptions;
CREATE POLICY tenant_iso_subscriptions ON subscriptions
    AS PERMISSIVE FOR ALL
    USING (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id::TEXT = NULLIF(current_setting('app.current_org_id', TRUE), '')
    )
    WITH CHECK (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id::TEXT = NULLIF(current_setting('app.current_org_id', TRUE), '')
    );

-- 3.15 Email Digest Subscriptions
ALTER TABLE email_digest_subscriptions ENABLE ROW LEVEL SECURITY;
ALTER TABLE email_digest_subscriptions FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_iso_email_digest ON email_digest_subscriptions;
CREATE POLICY tenant_iso_email_digest ON email_digest_subscriptions
    AS PERMISSIVE FOR ALL
    USING (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
    )
    WITH CHECK (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
    );

-- 3.16 Digest Send History
ALTER TABLE digest_send_history ENABLE ROW LEVEL SECURITY;
ALTER TABLE digest_send_history FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_iso_digest_history ON digest_send_history;
CREATE POLICY tenant_iso_digest_history ON digest_send_history
    AS PERMISSIVE FOR ALL
    USING (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
    )
    WITH CHECK (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
    );

-- 3.17 FIX Orders
ALTER TABLE fix_orders ENABLE ROW LEVEL SECURITY;
ALTER TABLE fix_orders FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_iso_fix_orders ON fix_orders;
CREATE POLICY tenant_iso_fix_orders ON fix_orders
    AS PERMISSIVE FOR ALL
    USING (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
    )
    WITH CHECK (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR user_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
    );

-- 3.18 Synthetic PIT Test Tables (when present)
ALTER TABLE instrument_master ENABLE ROW LEVEL SECURITY;
ALTER TABLE instrument_master FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_iso_instrument_master ON instrument_master;
CREATE POLICY tenant_iso_instrument_master ON instrument_master
    AS PERMISSIVE FOR ALL
    USING (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR org_id = 'org_global'
    )
    WITH CHECK (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR org_id = 'org_global'
    );

ALTER TABLE filings_raw ENABLE ROW LEVEL SECURITY;
ALTER TABLE filings_raw FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_iso_filings_raw ON filings_raw;
CREATE POLICY tenant_iso_filings_raw ON filings_raw
    AS PERMISSIVE FOR ALL
    USING (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR org_id = 'org_global'
    )
    WITH CHECK (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR org_id = 'org_global'
    );

ALTER TABLE filings_normalized ENABLE ROW LEVEL SECURITY;
ALTER TABLE filings_normalized FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_iso_filings_normalized ON filings_normalized;
CREATE POLICY tenant_iso_filings_normalized ON filings_normalized
    AS PERMISSIVE FOR ALL
    USING (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR org_id = 'org_global'
    )
    WITH CHECK (
        org_id = NULLIF(current_setting('app.current_org_id', TRUE), '')
        OR org_id = 'org_global'
    );

-- ─────────────────────────────────────────────────────────────────────────────
-- 4. Grants & Least-Privilege Separation
-- ─────────────────────────────────────────────────────────────────────────────

GRANT USAGE ON SCHEMA public TO fintext_app, fintext_ingest;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO fintext_app, fintext_ingest;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT USAGE, SELECT ON SEQUENCES TO fintext_app, fintext_ingest;

-- fintext_app has standard DML privileges across tables, but is subject to RLS
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO fintext_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO fintext_app;

-- fintext_ingest writes to market data tables
GRANT SELECT, INSERT, UPDATE, DELETE ON sentiment_records TO fintext_ingest;

-- Ops-only table restrictions (if dlq_events exists, revoke from app role)
DO $$
BEGIN
    IF EXISTS (SELECT FROM pg_tables WHERE schemaname = 'public' AND tablename = 'dlq_events') THEN
        REVOKE ALL ON dlq_events FROM fintext_app, fintext_ingest;
        GRANT SELECT, INSERT, UPDATE, DELETE ON dlq_events TO fintext;
    END IF;
END $$;
