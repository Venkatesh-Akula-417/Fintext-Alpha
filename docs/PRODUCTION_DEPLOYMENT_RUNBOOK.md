# FinText Alpha Vectorizer — AWS Production Deployment Runbook
**Document ID:** `DOC-OPS-AWS-DEPLOY-2026-V1`  
**Classification:** Institutional Operations / Infrastructure Engineering Manual  
**Last Updated:** 2026-09-25  
**Execution Target:** AWS Region `us-east-1` (N. Virginia)  
**Strict Cost Cap:** **$301.44 / month** (Actual: $281.49/mo baseline c6i | $195.64/mo Graviton fallback)  
**Target Audience:** Founder / CTO, DevOps Lead, Site Reliability Engineers  
**Authoritative References:** [docs/CERTIFIED_METRICS_REGISTER.md](file:///d:/FinText-Alpha-Vectorizer/docs/CERTIFIED_METRICS_REGISTER.md) | [docs/STATUS_PAGE_GUIDE.md](file:///d:/FinText-Alpha-Vectorizer/docs/STATUS_PAGE_GUIDE.md) | [infra/terraform/main.tf](file:///d:/FinText-Alpha-Vectorizer/infra/terraform/main.tf)

---

## 1. Executive Summary & Architecture Topology

This runbook is the definitive, step-by-step mechanical repair manual for provisioning and operating the FinText Alpha Vectorizer production infrastructure on Amazon Web Services (AWS). It establishes a phased deployment model:
- **Phase-0 (Founder-Internal Initial Launch):** Direct deployment to single-node EC2 via static Elastic IP, self-signed/Let's Encrypt TLS on IP or internal subdomain, private RDS TimescaleDB, internal smoke test verification. No public DNS cutover.
- **Phase-1 (Institutional Domain Day):** Production apex/subdomain DNS routing (`api.fintext.ai`), institutional TLS certificate binding, external status page publication, and customer IP whitelisting activation.

```
═════════════════════════════════════════════════════════════════════════════════
                       PRODUCTION DUAL-AZ TOPOLOGY (us-east-1)
═════════════════════════════════════════════════════════════════════════════════

                          ┌─────────────────────────────┐
                          │ Internet / Quant Fund Desks │
                          └──────────────┬──────────────┘
                                         │ HTTPS (:443) / SSH (:22 admin)
                                         ▼
 ┌─────────────────────────────────────────────────────────────────────────────┐
 │ AWS VPC: 10.0.0.0/16 (us-east-1)                                            │
 │                                                                             │
 │  ┌───────────────────────────────────────────────────────────────────────┐  │
 │  │ Public Subnet (10.0.1.0/24) - us-east-1a                              │  │
 │  │                                                                       │  │
 │  │  ┌─────────────────────────────────────────────────────────────────┐  │  │
 │  │  │ EC2 Host (c6i.xlarge, 4 vCPU, 8 GiB RAM, 100GB gp3)             │  │  │
 │  │  │ Elastic IP: 54.x.x.x (Static)                                   │  │  │
 │  │  │                                                                 │  │  │
 │  │  │  Docker Compose Network (Bridge: 172.28.0.0/16)                 │  │  │
 │  │  │  ┌──────────────┐   ┌──────────────┐   ┌─────────────────────┐  │  │  │
 │  │  │  │ Axum Gateway │   │ Vector Ingest│   │ Redpanda Streaming  │  │  │  │
 │  │  │  │ (:8000/:443) │   │ (C++ Bindgen)│   │ (Kafka Bus :9092)   │  │  │  │
 │  │  │  └──────┬───────┘   └──────┬───────┘   └──────────┬──────────┘  │  │  │
 │  │  └─────────┼──────────────────┼──────────────────────┼─────────────┘  │  │
 │  └────────────┼──────────────────┼──────────────────────┼────────────────┘  │
 │               │                  │                      │                   │
 │               ▼ (Port 5432)      ▼ (Port 5432)          │                   │
 │  ┌──────────────────────────────────────────────────────┼────────────────┐  │
 │  │ Private Subnets (10.0.10.0/24, 10.0.11.0/24) - us-east-1a / us-east-1b │  │
 │  │                                                                       │  │
 │  │  ┌─────────────────────────────────────────────────────────────────┐  │  │
 │  │  │ RDS PostgreSQL 16 + TimescaleDB (db.m6i.large, 100GB gp3)       │  │  │
 │  │  │ Private Hostname: fintext-metadata-production.c...us-east-1.rds │  │  │
 │  │  │ Multi-AZ: False (Beta Cost Cap) | Storage Auto-Scale: 500GB     │  │  │
 │  │  └─────────────────────────────────────────────────────────────────┘  │  │
 │  └───────────────────────────────────────────────────────────────────────┘  │
 └─────────────────────────────────────────────────────────────────────────────┘
                                         │
                                         ▼ (IAM Role S3 & KMS Policy)
 ┌─────────────────────────────────────────────────────────────────────────────┐
 │ S3 & Security Infrastructure                                                │
 │  - Primary Backups: s3://fintext-backups-production (7-day lifecycle)       │
 │  - Parquet Lakehouse: s3://fintext-parquet-raw-archive-production (90d GLACIER)
 │  - Velero State: s3://fintext-velero-backups-production                     │
 │  - Envelope CMK: aws_kms_key.fintext_backup_key                             │
 │  - CloudWatch: /fintext/api-server & /fintext/ingestion-engine (30d)        │
 └─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Certified Per-Resource Cost Architecture

The monthly infrastructure expenditure is strictly capped at **$301.44 / month** to maintain positive unit economics on our very first paying subscriber ($500/mo Starter Quant tier = 41.0% immediate gross margin).

### 2.1 Baseline Architecture (x86_64 c6i.xlarge + db.m6i.large)

| Resource Identifier | Specifications & Sizing | Billing Dimension | Monthly Cost (USD) | Source / Citation |
| :--- | :--- | :--- | :--- | :--- |
| **EC2 Compute** | `c6i.xlarge` (4 vCPU, 8 GiB RAM) | 730 hrs @ $0.1700/hr | **$124.10** | AWS EC2 Pricing (us-east-1 On-Demand) |
| **EC2 Root EBS** | 100 GiB gp3 (3000 IOPS, 125 MB/s) | 100 GiB @ $0.0800/GiB | **$8.00** | AWS EBS Pricing (us-east-1 gp3) |
| **RDS TimescaleDB** | `db.m6i.large` (2 vCPU, 8 GiB RAM, Single-AZ) | 730 hrs @ $0.1780/hr | **$129.94** | AWS RDS PostgreSQL Pricing (us-east-1) |
| **RDS Storage** | 100 GiB gp3 (Single-AZ) | 100 GiB @ $0.1150/GiB | **$11.50** | AWS RDS Storage Pricing |
| **Elastic IP** | 1 Public Ingress IPv4 Address | 730 hrs @ $0.0050/hr | **$3.65** | AWS Public IPv4 Pricing (Effective Feb 2024) |
| **S3 Storage** | 50 GB Backups (Standard) + Parquet (Glacier) | Blended Tiered Ingestion | **$1.50** | AWS S3 Pricing ($0.023/GB Std, $0.0036/GB Glacier) |
| **CloudWatch Logs** | 2 Log Groups, 5 GB ingestion, 4 alarms | Ingestion + Alarms | **$2.80** | AWS CloudWatch ($0.50/GB logs, $0.10/alarm) |
| **TOTAL (Baseline)** | **Complete Institutional Platform Stack** | **Monthly Invariant** | **$281.49 / mo** | **$19.95 Under Hard Cap ($301.44)** |

### 2.2 Graviton Cost-Optimization Fallback (t4g.xlarge + db.t4g.large)

If market conditions require even leaner operational burn, the platform can be resized to AWS Graviton (ARM64) processors via a single Terraform variable update (`ec2_instance_type = "t4g.xlarge"`, `db_instance_class = "db.t4g.large"`):

| Resource Identifier | Specifications & Sizing | Billing Dimension | Monthly Cost (USD) | Cost Delta |
| :--- | :--- | :--- | :--- | :--- |
| **EC2 Compute** | `t4g.xlarge` (4 vCPU, 16 GiB RAM) | 730 hrs @ $0.1344/hr | **$98.11** | -$25.99 / mo (+8GB RAM bonus!) |
| **RDS TimescaleDB** | `db.t4g.large` (2 vCPU, 8 GiB RAM) | 730 hrs @ $0.0960/hr | **$70.08** | -$59.86 / mo |
| **All Other Items** | EBS (100GB), RDS gp3, EIP, S3, CloudWatch | Fixed baseline tiers | **$27.45** | $0.00 |
| **TOTAL (Graviton)** | **ARM64 Optimized Platform Stack** | **Monthly Invariant** | **$195.64 / mo** | **$105.80 Under Hard Cap!** |

---

## 3. Pre-Flight Prerequisites & Security Clearances

Before issuing any deployment commands, verify the following prerequisites:

1. **AWS CLI Credentials:** Installed and authenticated with permissions for VPC, EC2, RDS, IAM, S3, and CloudWatch:
   ```bash
   aws sts get-caller-identity
   ```
   *Expected Output:*
   ```json
   {
       "UserId": "AIDAXAMPLEUSERID",
       "Account": "123456789012",
       "Arn": "arn:aws:iam::123456789012:user/founder-deployer"
   }
   ```
2. **Environment Variables:** Set master database credentials and environment secrets:
   ```bash
   export TF_VAR_environment="production"
   export TF_VAR_aws_region="us-east-1"
   export TF_VAR_db_password="A_Strong_Institutional_Password_2026!"
   export TF_VAR_admin_ssh_cidr='["YOUR_OFFICE_OR_VPN_IP/32"]'
   ```
3. **Terraform Toolchain:** Terraform v1.5.0+ installed and on PATH:
   ```bash
   terraform version
   ```

---

## 4. Phase-0: Infrastructure Provisioning (Terraform IaC)

### Step 4.1: Initialize Terraform & Download Providers
```bash
cd infra/terraform
terraform init -backend=false
```
*Expected Output:*
```
Initializing provider plugins...
- Finding hashicorp/aws versions matching "~> 5.0"...
- Installing hashicorp/aws v5.100.0...
Terraform has been successfully initialized!
```

### Step 4.2: Validate Syntax & Resource Declarations
```bash
terraform validate
```
*Expected Output:*
```
Success! The configuration is valid.
```

### Step 4.3: Review Provisioning Execution Plan
```bash
terraform plan -out=tfplan.binary
```
*Expected Output:*
```
Plan: 28 to add, 0 to change, 0 to destroy.
Changes to Outputs:
  + cloudwatch_api_log_group       = "/fintext/api-server-production"
  + cloudwatch_ingestion_log_group = "/fintext/ingestion-engine-production"
  + ec2_instance_id                = (known after apply)
  + ec2_public_ip                  = (known after apply)
  + rds_address                    = (known after apply)
  + rds_endpoint                   = (known after apply)
  + rds_port                       = 5432
  + s3_backup_bucket               = "fintext-backups-production"
  + s3_parquet_archive_bucket      = "fintext-parquet-raw-archive-production"
  + s3_velero_bucket               = "fintext-velero-backups-production"
  + vpc_id                         = (known after apply)
```

### Step 4.4: Apply Infrastructure Plan (Founder Clearance Required)
```bash
terraform apply tfplan.binary
```
*Expected Output:*
```
Apply complete! Resources: 28 added, 0 changed, 0 destroyed.
Outputs:
ec2_public_ip = "54.210.14.88"
rds_address = "fintext-metadata-production.c7x...us-east-1.rds.amazonaws.com"
rds_endpoint = "fintext-metadata-production.c7x...us-east-1.rds.amazonaws.com:5432"
```

---

## 5. Phase-0: Host Setup & Database Migration

### Step 5.1: SSH Connection to Production EC2 Host
```bash
SSH_IP=$(terraform output -raw ec2_public_ip)
ssh -i ~/.ssh/fintext-deployer.pem ubuntu@$SSH_IP
```

### Step 5.2: Verify Host Cloud-Init & Docker Engine
```bash
docker --version
docker compose version
cat /opt/fintext/provisioned.log
```
*Expected Output:*
```
Docker version 27.2.0, build 3ab425e
Docker Compose version v2.29.2
FinText Alpha Vectorizer Host Provisioning Complete
```

### Step 5.3: Clone Codebase to Production Directory
```bash
sudo mkdir -p /opt/fintext/app
sudo chown -R ubuntu:ubuntu /opt/fintext/app
cd /opt/fintext/app
git clone https://github.com/Venkatesh-Akula-417/Fintext-Alpha.git .
git checkout main
```

### Step 5.4: Populate Production Environment File (`.env.production`)
Create `/opt/fintext/app/.env.production` with zero sensitive token commits:
```bash
cat << 'EOF' > /opt/fintext/app/.env.production
ENVIRONMENT=production
PORT=8000
HOST=0.0.0.0
RUST_LOG=info,fintext_api_server=info
RUST_MIN_STACK=16777216

# Database Connectivity (Private RDS Endpoint)
DATABASE_URL=postgres://fintext_admin:A_Strong_Institutional_Password_2026!@fintext-metadata-production.c7x...us-east-1.rds.amazonaws.com:5432/fintext_metadata
TIMESCALE_DB_URL=postgres://fintext_admin:A_Strong_Institutional_Password_2026!@fintext-metadata-production.c7x...us-east-1.rds.amazonaws.com:5432/fintext_metadata

# Streaming Bus (Internal Redpanda)
KAFKA_BOOTSTRAP_SERVERS=redpanda:9092

# Secrets & Authentication
ADMIN_TOKEN=REPLACE_WITH_SECURE_RANDOM_HEX_64_CHARS
JWT_SECRET=REPLACE_WITH_SECURE_RANDOM_HEX_64_CHARS
STRIPE_WEBHOOK_SECRET=whsec_replace_with_live_webhook_secret

# S3 Buckets & KMS
S3_BACKUP_BUCKET=fintext-backups-production
S3_PARQUET_ARCHIVE_BUCKET=fintext-parquet-raw-archive-production
AWS_REGION=us-east-1
EOF
```

### Step 5.5: Run TimescaleDB & RLS Schema Migrations
Execute the sequential database migrations against the private RDS endpoint:
```bash
# Connect and apply schema
PGPASSWORD="A_Strong_Institutional_Password_2026!" psql \
  -h fintext-metadata-production.c7x...us-east-1.rds.amazonaws.com \
  -U fintext_admin \
  -d fintext_metadata \
  -f config/timescale/init.sql

PGPASSWORD="A_Strong_Institutional_Password_2026!" psql \
  -h fintext-metadata-production.c7x...us-east-1.rds.amazonaws.com \
  -U fintext_admin \
  -d fintext_metadata \
  -f config/timescale/03-rls-multi-tenant-isolation.sql

PGPASSWORD="A_Strong_Institutional_Password_2026!" psql \
  -h fintext-metadata-production.c7x...us-east-1.rds.amazonaws.com \
  -U fintext_admin \
  -d fintext_metadata \
  -f config/timescale/04-billing-webhooks.sql
```
*Expected Output:*
```
CREATE EXTENSION IF NOT EXISTS timescaledb;
CREATE TABLE IF NOT EXISTS instrument_master;
CREATE TABLE IF NOT EXISTS ip_whitelist;
ALTER TABLE ip_whitelist ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_iso_ip_whitelist ON ip_whitelist ...
```

---

## 6. Phase-0: Service Launch & Smoke Verification

### Step 6.1: Start Platform Services via Docker Compose
```bash
cd /opt/fintext/app
docker compose --env-file .env.production -f docker-compose.yml up -d
```
*Expected Output:*
```
[+] Running 4/4
 ✔ Network app-network               Created
 ✔ Container redpanda                Started
 ✔ Container fintext-api             Started
 ✔ Container fintext-ingestion       Started
```

### Step 6.2: Smoke Test Diagnostic Health Probe
```bash
curl -s http://127.0.0.1:8000/v1/health | jq .
```
*Expected Output:*
```json
{
  "status": "healthy",
  "version": "1.0.0",
  "uptime_seconds": 12,
  "database": "connected",
  "kafka": "connected"
}
```

### Step 6.3: Smoke Test Status Endpoint
```bash
curl -s http://127.0.0.1:8000/v1/status | jq .
```
*Expected Output:*
```json
{
  "status": "operational",
  "version": "1.0.0",
  "components": {
    "api_gateway": "operational",
    "vector_engine": "operational",
    "timescaledb": "operational",
    "kafka_bus": "operational"
  },
  "certifications": {
    "soc2_type2_ready": true,
    "rls_tenant_isolation": "enforced_active",
    "target_p95_latency_ms": 500,
    "target_uptime_pct": 99.5
  }
}
```

### Step 6.4: Smoke Test JWT Generation & Authenticated Sentiment Query
```bash
# Mint token
ADMIN_TOKEN="REPLACE_WITH_SECURE_RANDOM_HEX_64_CHARS"
TOKEN=$(curl -s -X POST http://127.0.0.1:8000/v1/auth/token \
  -H "X-Admin-Token: $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"user_id": "founder_smoke_test", "role": "admin", "ttl_seconds": 3600}' | jq -r .token)

# Query Point-in-Time sentiment
curl -s -H "Authorization: Bearer $TOKEN" "http://127.0.0.1:8000/v1/sentiment?ticker=AAPL" | jq .
```
*Expected Output:*
```json
{
  "ticker": "AAPL",
  "sentiment_score": 0.4215,
  "sentiment_label": "POSITIVE",
  "data_quality_score": 0.942,
  "confidence": 0.884,
  "timestamp": "2026-09-25T11:55:00Z"
}
```

---

## 7. Phase-1: Domain Day & Institutional Certification

Once Phase-0 internal validation passes, execute Phase-1 for external customer onboarding:

### Step 7.1: DNS Apex & Subdomain Binding
1. In your authoritative DNS provider (Route 53 or Cloudflare), create an `A` record pointing to the static Elastic IP:
   ```
   api.fintext.ai.   300   IN   A   54.210.14.88
   ```
2. Verify global DNS propagation:
   ```bash
   dig +short api.fintext.ai @8.8.8.8
   ```
   *Expected Output:* `54.210.14.88`

### Step 7.2: TLS Certificate Provisioning (Let's Encrypt / Certbot)
On the EC2 host:
```bash
sudo apt-get install -y certbot
sudo certbot certonly --standalone -d api.fintext.ai --non-interactive --agree-tos -m ops@fintext.ai
```
Bind the issued certificate to the reverse proxy (Nginx or Envoy) in `/opt/fintext/certs/`.

### Step 7.3: Verify HTTPS Public Endpoint
From an external workstation:
```bash
curl -Iv https://api.fintext.ai/v1/health
```
*Expected Output:*
```
HTTP/2 200
server: fintext-gateway
content-type: application/json
strict-transport-security: max-age=31536000; includeSubDomains
```

### Step 7.4: Publish Public Status Dashboard
Verify the automated GitHub Actions status page updater:
```bash
gh workflow run status-page.yml
```
Inspect workflow run output and verify that `https://status.fintext.ai` displays `OPERATIONAL`.

---

## 8. Rollback Procedures & Failure Recovery

### Rollback Scenario 1: EC2 Application Failure
If container corruption or bad deployment occurs:
```bash
cd /opt/fintext/app
docker compose down
git checkout HEAD~1
docker compose --env-file .env.production up -d
curl -s http://127.0.0.1:8000/v1/health | jq .status
```

### Rollback Scenario 2: Database Schema Regression
To rollback to the latest hourly backup:
```bash
python scripts/test_restore.py
# If restoring to live RDS:
LATEST_BACKUP=$(aws s3 ls s3://fintext-backups-production/postgres/ | sort | tail -n 1 | awk '{print $4}')
aws s3 cp s3://fintext-backups-production/postgres/$LATEST_BACKUP /tmp/restore.sql.gz
gunzip -c /tmp/restore.sql.gz | PGPASSWORD="$DB_PASSWORD" psql -h $RDS_HOST -U fintext_admin -d fintext_metadata
```

### Rollback Scenario 3: Complete Infrastructure Tear-Down
If emergency resource decommissioning is required:
```bash
cd infra/terraform
terraform destroy -auto-approve
```
*Safety Guarantee:* S3 buckets have `force_destroy = false` and RDS has `deletion_protection = true` to prevent accidental institutional data loss.

---

## 9. Operational Troubleshooting Guide

| Symptom | Diagnostic Command | Root Cause | Remediation |
| :--- | :--- | :--- | :--- |
| **HTTP 502 Bad Gateway** | `docker ps -a` | Axum container exited or OOM | Check `docker logs fintext-api`. Increase swap or set `RUST_MIN_STACK=16777216`. |
| **HTTP 403 Forbidden** | `curl -H "x-forwarded-for: IP" ...` | Client IP not in tenant whitelist | Verify client CIDR via `GET /v1/security/ip-whitelist` or add IP via `POST`. |
| **DB Connection Timeout** | `nc -zv $RDS_HOST 5432` | Security group rule mismatch | Verify `aws_security_group.rds_sg` allows port 5432 from `aws_security_group.ec2_sg`. |
| **Kafka Queue Stalled** | `docker logs redpanda` | Partition disk full | Prune ephemeral consumer groups or purge DLQ via `POST /internal/dlq/events/{id}/reprocess`. |
| **High CPU > 80% Alarm** | `top -b -n 1 \| head -n 20` | Heavy batch vectorization | Scale instance to Graviton `t4g.xlarge` (16GB RAM) or throttle ingestion rate. |
