# Configuration Guide

> **Last Verified**: 2026-09-10 (Suite #96) | **Audit Readiness**: Certified Clean | **Authoritative Deprecations**: [`docs/DEPRECATED.md`](../docs/DEPRECATED.md)

## Canonical (Primary)
- `config.yaml` — Master configuration
- `feature_flags.yaml` — Runtime toggles
- `timescale/init.sql` — Canonical TimescaleDB schema
- `pit_reference_schema.sql` — Canonical PIT schema

## Fallback (JSON — Used Only When pit_database.enabled=false)
- `ticker_history.json`
- `delisted_securities.json`
- `corporate_actions.json`
- `index_membership.json`

The JSON files are historical fallbacks. The PostgreSQL tables are canonical. When `pit_database.enabled=true`, JSON files are ignored.

See `docs/DEPRECATED.md` for removed configs.
