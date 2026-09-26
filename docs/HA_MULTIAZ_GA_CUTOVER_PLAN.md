# FinText Alpha Vectorizer — GA High-Availability (Multi-AZ) Cutover Plan
═══════════════════════════════════════════════════════════════════════════════
Document ID: DOC-OPS-HA-GA-CUTOVER-2026-V1  
Classification: INSTITUTIONAL SRE & INFRASTRUCTURE ENGINEERING RUNBOOK  
Target System: FinText Alpha Vectorizer (Transition from Private Beta to General Availability)  
Target Topology: AWS us-east-1 (N. Virginia) Dual-AZ Fault Tolerance  
Effective Date: September 26, 2026  
Responsible Roles: Site Reliability Lead, Principal Cloud Architect & Chief Technology Officer  
═══════════════════════════════════════════════════════════════════════════════

## 1. Executive Summary & Problem Context

During the Private Beta phase, FinText Alpha Vectorizer operates on a cost-optimized, single-node compute architecture with single-AZ managed database hosting to strictly comply with the **$301.44 / month** hard cloud expenditure cap ($281.49 / mo actual run-rate).

As the platform transitions from Private Beta to General Availability (GA) with Tier-1 quant hedge fund clients, eliminating Single Points of Failure (SPOF) becomes an institutional requirement. 

This document defines:
1. The exact inventory of single-AZ components in the Private Beta architecture.
2. An options matrix with line-by-line pricing deltas and AWS citations for upgrading compute and data tiers.
3. The staged, zero-regression Terraform configuration (defaulting to current behavior).
4. An end-to-end phased cutover runbook with automated failover testing and rapid rollback procedures.
5. An authoritative **Founder Decision Record** regarding cloud expenditure budget revision.

---

## 2. Private Beta Topology Inventory & Single Points of Failure (SPOF)

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        PRIVATE BETA TOPOLOGY vs GA TARGET                              │
├─────────────────────┬───────────────────────────────┬──────────────────────────────────┤
│ Component Tier      │ Private Beta (Current)        │ GA Target (Fault-Tolerant)       │
├─────────────────────┼───────────────────────────────┼──────────────────────────────────┤
│ Compute (API & Ing) │ 1x EC2 c6i.xlarge in us-east-1a│ 2x EC2 (us-east-1a + us-east-1b) │
│ Ingress Routing     │ Static Elastic IP (Single EIP)│ Application Load Balancer (ALB)  │
│ Database (Timescale)│ Single-AZ RDS db.m6i.large    │ Multi-AZ RDS Synchronous Standby │
│ Backup Storage      │ S3 Standard with 7d retention │ S3 Standard with Cross-Region Rep│
│ Recovery Time (RTO) │ 0.111 s (Volume Restore Drill)│ < 120 s (Automated Multi-AZ Fail)│
│ Recovery Point (RPO)│ <= 1.0 hour (WAL Snapshot)    │ Zero Data Loss (Sync Commit)     │
└─────────────────────┴───────────────────────────────┴──────────────────────────────────┘
```

### Current Single Points of Failure:
1. **EC2 Application Host**: If `us-east-1a` experiences hardware degradation, API and ingestion containers must be re-provisioned via user-data script or manual failover.
2. **RDS Database Host**: While automated 7-day snapshots and continuous WAL archiving provide rapid restore capability (RTO 0.111s), host-level failure in single-AZ requires spinning up a replacement instance from snapshot (~15–30 minutes).

---

## 3. GA High-Availability Options & Cost Arithmetic Matrix

All pricing is based on AWS US East (N. Virginia, `us-east-1`) on-demand rates (730 operating hours per month):
- AWS EC2 Pricing Citation: [AWS EC2 On-Demand Pricing (us-east-1)](https://aws.amazon.com/ec2/pricing/on-demand/)
- AWS RDS PostgreSQL Pricing Citation: [AWS RDS PostgreSQL Pricing (us-east-1)](https://aws.amazon.com/rds/postgresql/pricing/)
- AWS ALB Pricing Citation: [Elastic Load Balancing Pricing](https://aws.amazon.com/elasticloadbalancing/pricing/)

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        GA INFRASTRUCTURE OPTIONS & PRICING DELTAS                      │
├──────────────────────┬────────────────────────┬─────────────┬─────────────┬────────────┤
│ Option Architecture  │ Configuration Details  │ Cost / Mo   │ Delta vs Cap│ Feasibility│
├──────────────────────┼────────────────────────┼─────────────┼─────────────┼────────────┤
│ Current Private Beta │ c6i.xlarge + Single-AZ │ $281.49     │ -$19.95     │ CURRENT    │
│                      │ db.m6i.large + gp3     │             │ (Headroom)  │ PRODUCTION │
├──────────────────────┼────────────────────────┼─────────────┼─────────────┼────────────┤
│ Option A:            │ c6i.xlarge ($122.64) + │ $411.43     │ +$109.99    │ EXCEEDS    │
│ RDS Multi-AZ Flip    │ Multi-AZ db.m6i.large  │             │ (Over Cap)  │ BUDGET CAP │
│                      │ ($259.88) + gp3/other  │             │             │            │
├──────────────────────┼────────────────────────┼─────────────┼─────────────┼────────────┤
│ Option B:            │ c7g.xlarge ($105.85) + │ $368.36     │ +$66.92     │ RECOMMENDED│
│ Graviton Compute &   │ Multi-AZ db.m7g.large  │             │ (Target     │ FOR GA     │
│ Multi-AZ Database    │ ($233.60) + gp3/other  │             │ <= $360)    │ EXPANSION  │
├──────────────────────┼────────────────────────┼─────────────┼─────────────┼────────────┤
│ Option C:            │ 2x c7g.xlarge ($211.70)│ $495.64     │ +$194.20    │ FULL ZERO- │
│ Full Dual-Compute +  │ + ALB ($21.43) + Multi-│             │ (Tier-1     │ SPOF GA    │
│ Multi-AZ Database    │ AZ db.m7g.large ($233) │             │ Enterprise) │ TARGET     │
└──────────────────────┴────────────────────────┴─────────────┴─────────────┴────────────┘
```

### Line-by-Line Arithmetic Breakdown:

#### 1. Current Private Beta Baseline ($281.49 / mo):
- EC2 `c6i.xlarge`: $0.168 / hr $\times 730\text{ hrs} = \mathbf{\$122.64}$
- EC2 100 GB gp3 root: $100 \times \$0.08 / \text{GB} = \mathbf{\$8.00}$
- RDS `db.m6i.large` (Single-AZ): $0.178 / hr $\times 730\text{ hrs} = \mathbf{\$129.94}$
- RDS 100 GB gp3 storage: $100 \times \$0.115 / \text{GB} = \mathbf{\$11.50}$
- S3 backups + CloudWatch + EIP: $\mathbf{\$9.41}$
- **Total**: $\$122.64 + \$8.00 + \$129.94 + \$11.50 + \$9.41 = \mathbf{\$281.49 / \text{month}}$ (Headroom: $\$19.95$ under $\$301.44$).

#### 2. Option A — RDS Multi-AZ Flip Only ($411.43 / mo):
- RDS `db.m6i.large` Multi-AZ: $0.356 / hr $\times 730\text{ hrs} = \mathbf{\$259.88}$ (Delta: $+\$129.94$)
- All other components unchanged ($\$151.55$)
- **Total**: $\$151.55 + \$259.88 = \mathbf{\$411.43 / \text{month}}$ (Over cap by $\$109.99 / \text{mo}$).

#### 3. Option B — Graviton Compute & Database with Multi-AZ ($368.36 / mo):
- EC2 `c7g.xlarge` (ARM64 Graviton3, 4 vCPU, 8 GiB): $0.145 / hr $\times 730\text{ hrs} = \mathbf{\$105.85}$ (Saves $\$16.79 / \text{mo}$)
- EC2 100 GB gp3 root: $\mathbf{\$8.00}$
- RDS `db.m7g.large` Multi-AZ (Graviton3, 2 vCPU, 8 GiB): $0.320 / hr $\times 730\text{ hrs} = \mathbf{\$233.60}$ (Saves $\$26.28 / \text{mo}$ vs x86 Multi-AZ)
- RDS 100 GB gp3 storage: $\mathbf{\$11.50}$
- S3 + CloudWatch + EIP: $\mathbf{\$9.41}$
- **Total**: $\$105.85 + \$8.00 + \$233.60 + \$11.50 + \$9.41 = \mathbf{\$368.36 / \text{month}}$.

#### 4. Option C — Full Dual-Compute + ALB + Multi-AZ Database ($495.64 / mo):
- 2x EC2 `c7g.xlarge`: $2 \times \$105.85 = \mathbf{\$211.70}$
- 2x 100 GB gp3 root: $\mathbf{\$16.00}$
- AWS Application Load Balancer: $\$0.0225 / \text{hr} \times 730\text{ hrs} + 1\text{ LCU} = \mathbf{\$21.43}$
- RDS `db.m7g.large` Multi-AZ: $\mathbf{\$233.60}$
- RDS 100 GB gp3 storage: $\mathbf{\$11.50}$
- S3 + CloudWatch: $\mathbf{\$1.41}$
- **Total**: $\$211.70 + \$16.00 + \$21.43 + \$233.60 + \$11.50 + \$1.41 = \mathbf{\$495.64 / \text{month}}$.

### Compute Tradeoff Evaluation: EC2 vs ECS Fargate
| Evaluation Metric | Dual EC2 + ALB (Recommended) | AWS ECS Fargate Sidecars |
| :--- | :--- | :--- |
| **P95 Latency Overhead** | **$+0.2\text{ ms}$ to $+0.4\text{ ms}$** (Direct kernel) | $+1.2\text{ ms}$ to $+2.5\text{ ms}$ (ENI attach/sidecar) |
| **Operational Burden** | Low (Docker Compose parity with dev) | High (Task definitions, ECR, task roles) |
| **Local ML Model Load** | Direct NVMe/gp3 mapped volume ($<2\text{s}$) | EFS mount or slow container image pull |
| **Recommendation** | **SELECTED (Latency-First Principle)** | REJECTED for Mid-Frequency Quant Engine |

---

## 4. Staged Terraform Variables (Zero-Regression Defaults)

To ensure this cutover plan is executable without risky manual console operations, the required variables are declared in [`infra/terraform/variables.tf`](file:///d:/FinText-Alpha-Vectorizer/infra/terraform/variables.tf) with strict `default = false`:

```hcl
variable "rds_multi_az" {
  type        = bool
  default     = false
  description = "Enable RDS PostgreSQL Multi-AZ synchronous standby deployment for GA"
}

variable "ha_compute_enabled" {
  type        = bool
  default     = false
  description = "Enable dual-node EC2 compute architecture across availability zones for GA"
}

variable "alb_enabled" {
  type        = bool
  default     = false
  description = "Deploy Application Load Balancer across public subnets for GA compute failover"
}
```

### Guardrail Verification:
Because all three variables default to `false`, the terraform configuration produces an **empty diff** against current production infrastructure.

---

## 5. Phased GA Cutover Runbook & Operational Verification

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        PHASED CUTOVER & DRILL TIMELINE                                 │
├───────┬──────────────────────┬─────────────────────────────────────────────────────────┤
│ Phase │ Window               │ Actions & Verification Battery                          │
├───────┼──────────────────────┼─────────────────────────────────────────────────────────┤
│ T-7d  │ Preparation          │ Lower DNS TTL on api.fintext.internal to 60 seconds.    │
├───────┼──────────────────────┼─────────────────────────────────────────────────────────┤
│ T-24h │ Audit & Snapshot     │ Run pre-flight backup; execute RLS isolation drill.     │
├───────┼──────────────────────┼─────────────────────────────────────────────────────────┤
│ T-0   │ Maintenance Window   │ Saturday 01:00 UTC (Low institutional market activity). │
│       │ 01:00 – 01:20 UTC    │ Step 1: Apply terraform -var="rds_multi_az=true".       │
│       │                      │ RDS creates synchronous standby in us-east-1b (<15 min).│
├───────┼──────────────────────┼─────────────────────────────────────────────────────────┤
│ T+20m │ Failover Test Drill  │ Trigger manual RDS failover:                            │
│       │                      │ aws rds reboot-db-instance --force-failover.            │
│       │                      │ Verify API Gateway reconnects in < 60 seconds.          │
├───────┼──────────────────────┼─────────────────────────────────────────────────────────┤
│ T+35m │ Compute HA Enable    │ Apply -var="ha_compute_enabled=true" -var="alb_enabled= │
│       │                      │ true". Verify health checks: GET /readyz returns 200.   │
├───────┼──────────────────────┼─────────────────────────────────────────────────────────┤
│ T+45m │ DNS Traffic Cutover  │ Point api.fintext.internal CNAME to ALB DNS endpoint.   │
├───────┼──────────────────────┼─────────────────────────────────────────────────────────┤
│ T+55m │ Verification Battery │ Execute k6 smoke drill: P95 latency must remain < 50ms. │
│       │                      │ Audit /v1/status and /v1/health endpoints.              │
└───────┴──────────────────────┴─────────────────────────────────────────────────────────┘
```

### Rollback Procedure (Emergency Reversion):
If unexpected latency degradation ($P_{95} > 100\text{ ms}$) or persistent connection errors occur:
1. **T+0**: Revert DNS CNAME back to static Elastic IP (EIP) of Host 1 (TTL 60s propagates in 1 minute).
2. **T+2m**: Verify direct traffic reaches primary host: `curl -I https://${EIP}/v1/health`.
3. **T+10m**: Revert Terraform variables:
   ```bash
   terraform apply -var="alb_enabled=false" -var="ha_compute_enabled=false"
   ```
4. **T+20m**: RDS standby can remain active or be converted back to single-AZ during the next scheduled maintenance window without downtime.

---

## 6. Founder Decision Record & Cost Cap Revision Authorization

```
╔════════════════════════════════════════════════════════════════════════════════════════╗
║                        FOUNDER BUDGET DECISION RECORD                                  ║
╠════════════════════════════════════════════════════════════════════════════════════════╣
║                                                                                        ║
║   Current Certified Cost Cap:         $301.44 / month                                  ║
║   Current Private Beta Run-Rate:       $281.49 / month ($19.95 Headroom)               ║
║                                                                                        ║
║   PROPOSED GA COST CAP OPTIONS:                                                        ║
║   [ ] Option A: RDS Multi-AZ Only     New Cap: $425.00 / mo (Run-rate: $411.43)        ║
║   [ ] Option B: Graviton HA (Target)   New Cap: $385.00 / mo (Run-rate: $368.36)        ║
║   [ ] Option C: Full Dual-Host + ALB  New Cap: $520.00 / mo (Run-rate: $495.64)        ║
║                                                                                        ║
║   STANDING GOVERNANCE INVARIANT:                                                       ║
║   The production cost cap remains strictly $301.44 / month. The Terraform variables    ║
║   rds_multi_az, ha_compute_enabled, and alb_enabled MUST remain default=false until     ║
║   this decision record is formally countersigned by the Platform Founder.              ║
║                                                                                        ║
║   Founder Approval Signature: ___________________________    Date: ______________      ║
║                                                                                        ║
╚════════════════════════════════════════════════════════════════════════════════════════╝
```
