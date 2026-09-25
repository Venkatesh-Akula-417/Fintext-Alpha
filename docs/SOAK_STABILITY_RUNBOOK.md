# FinText Alpha Vectorizer — Soak Stability & Memory-Leak Surveillance Runbook
═══════════════════════════════════════════════════════════════════════════════
Document ID: RUNBOOK-SRE-SOAK-001  
Classification: INSTITUTIONAL SITE RELIABILITY & GA GATING  
Status: PRODUCTION OPERATIONAL (P2 GA-EVIDENCE CLOCK STARTER)  
Audience: Site Reliability Engineers, Platform Architects, Quant Infrastructure Leads, CTO  
Repository: `FinText-Alpha-Vectorizer` (`git@github.com:Venkatesh-Akula-417/Fintext-Alpha.git`)  
Cross-References: [CERTIFIED_METRICS_REGISTER.md](file:///d:/FinText-Alpha-Vectorizer/docs/CERTIFIED_METRICS_REGISTER.md), [STATUS_PAGE_GUIDE.md](file:///d:/FinText-Alpha-Vectorizer/docs/STATUS_PAGE_GUIDE.md), [OPERATIONS.md](file:///d:/FinText-Alpha-Vectorizer/docs/OPERATIONS.md)  
═══════════════════════════════════════════════════════════════════════════════

## 1. Overview & GA Operating Principle

### 1.1 Objective & The 30-Day Stability Clock
To achieve commercial General Availability (GA), institutional quantitative hedge funds mandate formal verification of platform stability: **a continuous 30-day evidence ledger demonstrating 99.5% uptime, P95 response latency strictly under 500ms, and zero monotonic memory leakage**.

A 30-day evidence window cannot be compressed or retroactively synthesized. It requires an append-only, audited evidence trail beginning during Private Beta. This runbook documents the automated tooling, mathematical regression algorithms, nightly schedulers, and incident triage procedures that power the FinText soak stability harness.

### 1.2 Core Stability Invariants
1. **Zero Monotonic Memory Leak:** Process Resident Set Size (RSS) memory must not exhibit a persistent upward climb. Normal allocator fragmentation and sawtooth patterns from garbage-collected or pooled buffers are permitted, provided the Ordinary Least Squares (OLS) regression slope remains `< 2.0 MiB/hour` or the coefficient of determination $R^2 < 0.5$.
2. **Deterministic Response Latency (P95 <= 500ms):** Over multi-day soak windows, rolling P95 response latency for authenticated alpha signal queries (`/v1/sentiment`) must remain $\le 500\text{ ms}$, with late-window degradation bounded within $\le 25\%$ of the initial baseline.
3. **Audit Ledger Immutability & Truthfulness (P-HONEST Principle):** The stability evidence ledger ([logs/soak_ledger.md](file:///d:/FinText-Alpha-Vectorizer/logs/soak_ledger.md)) is strictly append-only. If a run is aborted or a host reboots, an `INTERRUPTED` or gap line is recorded. Retroactive backfills are formally prohibited.

---

## 2. Memory Leak Detection Mathematics

A common pitfall in containerized systems is mistaking normal allocator behaviors (such as glibc/jemalloc arena allocations) for true memory leaks. FinText combines **Ordinary Least Squares (OLS) Slope** with the **Coefficient of Determination ($R^2$)**:

$$\text{Slope } \beta_1 = \frac{\sum_{i=1}^n (t_i - \bar{t})(M_i - \bar{M})}{\sum_{i=1}^n (t_i - \bar{t})^2} \quad \left[\frac{\text{MiB}}{\text{hour}}\right]$$

$$R^2 = \frac{\left[\sum_{i=1}^n (t_i - \bar{t})(M_i - \bar{M})\right]^2}{\sum_{i=1}^n (t_i - \bar{t})^2 \sum_{i=1}^n (M_i - \bar{M})^2}$$

Where:
- $t_i$ = Elapsed time in hours for sample $i$
- $M_i$ = Container Resident Set Size (RSS) in MiB

### Classification Matrix
```
                       Slope < 2.0 MiB/h          Slope >= 2.0 MiB/h
                   ┌──────────────────────────┬──────────────────────────┐
  R^2 < 0.5        │        CERTIFIED         │        CERTIFIED         │
  (Sawtooth/Noise) │ (Stable flat allocation) │ (Fluctuating batch load) │
                   ├──────────────────────────┼──────────────────────────┤
  R^2 >= 0.8       │        CERTIFIED         │       LEAK_SUSPECT       │
  (Monotonic Trend)│ (Negligible linear drift)│ (Persistent memory leak) │
                   └──────────────────────────┴──────────────────────────┘
```

---

## 3. Tooling & CLI Operation (`scripts/soak_test.py`)

The soak harness (`scripts/soak_test.py`) runs independently across Windows, WSL, and native Linux environments using standard Python 3.10+ libraries with zero third-party dependencies.

### 3.1 Execution Modes
| Mode | Duration | Default Interval | Typical Sample Count | Primary Use Case |
| :--- | :---: | :---: | :---: | :--- |
| **`smoke`** | 10 minutes | 10 seconds | 60 samples | CI/CD smoke test, pre-commit validation |
| **`window`** | 6.0 hours (custom) | 15 seconds | 1,440 samples | Nightly Kubernetes CronJob / Task Scheduler |
| **`standard`** | 24.0 hours | 30 seconds | 2,880 samples | Weekly staging stability certification |

### 3.2 Command Invocations

#### A. Execute Fast 10-Minute Smoke Soak (CI-Safe)
```bash
python scripts/soak_test.py --mode smoke
```
**Expected Terminal Output:**
```text
===============================================================================
 FinText Alpha Vectorizer — Long-Run Soak & Memory Leak Certification
===============================================================================
  Mode:           SMOKE
  Target Hours:   0.17h (600s)
  Sample Rate:    Every 10.0s
  Gateway Target: http://127.0.0.1:8000
  Signal Ticker:  AAPL
  Ledger Path:    D:\FinText-Alpha-Vectorizer\logs\soak_ledger.md
-------------------------------------------------------------------------------

  [Sample 0001 | 0s/600s] Lat:  45.2ms (P95:  45.2ms) | GW RSS:  142.5MB | Ing RSS:  385.1MB | Errs: 0
  [Sample 0010 | 95s/600s] Lat:  38.1ms (P95:  48.3ms) | GW RSS:  142.8MB | Ing RSS:  385.4MB | Errs: 0
  ...
===============================================================================
  SOAK AUDIT VERDICT: CERTIFIED
-------------------------------------------------------------------------------
  Duration:              0.1667 hours (600.2s)
  Total Samples:         60
  P50 / P95 / P99:       41.2ms / 52.4ms / 68.1ms (SLA <= 500ms)
  Error Rate:            0.00% (0 errors)
  Gateway RSS Slope:     +0.120 MiB/h (R^2 = 0.082, delta: +0.30 MiB)
  Ingestion RSS Slope:   +0.045 MiB/h (R^2 = 0.031, delta: +0.10 MiB)
===============================================================================

  [PASS] Structured report saved: logs/soak_report.json
  [PASS] Appended audit entry to GA evidence ledger: logs/soak_ledger.md
```
*(Exit code: `0`)*

#### B. Execute 6-Hour Nightly Window Soak
```bash
python scripts/soak_test.py --mode window --hours 6.0 --interval 15.0
```

#### C. Execute 24-Hour Full Stability Soak
```bash
python scripts/soak_test.py --mode standard
```

---

## 4. Exit Code & Verdict Matrix

| Exit Code | Verdict | Root Cause Criteria | SRE Immediate Action |
| :---: | :--- | :--- | :--- |
| **`0`** | **`CERTIFIED`** | Error rate = 0%, P95 < 500ms, RSS slope < 2.0 MiB/h or $R^2 < 0.5$ | None. Append to ledger and proceed with GA clock. |
| **`1`** | **`ERRORS`** | Any non-2xx/3xx HTTP response received during probe | Inspect container logs via `docker logs fintext-api-gateway`. Check TimescaleDB pool connection starvation. |
| **`2`** | **`LEAK_SUSPECT`** | RSS slope $\ge 2.0\text{ MiB/h}$ AND $R^2 \ge 0.8$ over window | Initiate heap dump inspection via `jemalloc` / `heaptrack`. Check unevicted unbounded cache growth in handlers. |
| **`3`** | **`LATENCY_DRIFT`**| P95 $\ge 500\text{ ms}$ or late window P95 > initial P95 $\times 1.25$ | Check TimescaleDB chunk compression locks, QuestDB ingestion lag, or CPU throttling. |
| **`4`** | **`INTERRUPTED`** | Process caught `SIGINT` (Ctrl+C) or host reboot | Review partial run output in `logs/soak_report.json`. Gap line recorded in ledger. |
| **`5`** | **`USAGE_ERROR`** | Missing arguments, database down, or invalid gateway URL | Verify docker container status (`docker ps`) and gateway reachability. |

---

## 5. Automated Nightly Scheduling

To ensure the 30-day stability clock accumulates continuous, verifiable data points without manual intervention, configure one of the following automated schedulers:

### 5.1 Option A: Kubernetes Nightly CronJob (`k8s/soak/cronjob.yaml`)
Runs automatically at 02:00 UTC every night inside the production/staging cluster:

```bash
kubectl apply -f k8s/soak/cronjob.yaml
```

To manually trigger an immediate test execution of the Kubernetes CronJob:
```bash
kubectl create job --from=cronjob/fintext-nightly-soak-drill manual-soak-drill-01
kubectl logs -f job/manual-soak-drill-01
```

### 5.2 Option B: Windows Host Scheduler (PowerShell One-Liner)
For dedicated Windows development or staging hosts, register a nightly scheduled task via administrative PowerShell:

```powershell
$Action = New-ScheduledTaskAction -Execute "python.exe" `
  -Argument "D:\FinText-Alpha-Vectorizer\scripts\soak_test.py --mode window --hours 6.0" `
  -WorkingDirectory "D:\FinText-Alpha-Vectorizer"

$Trigger = New-ScheduledTaskTrigger -Daily -At "02:00AM"

Register-ScheduledTask -TaskName "FinText-Nightly-Soak-Surveillance" `
  -Action $Action -Trigger $Trigger -Description "Automated 6-Hour Nightly Stability Soak"
```

### 5.3 Option C: Linux / WSL Crontab Setup
```bash
# Add to crontab via 'crontab -e':
0 2 * * * cd /opt/fintext && /usr/bin/python3 scripts/soak_test.py --mode window --hours 6.0 >> /var/log/fintext_soak.log 2>&1
```

---

## 6. SRE Incident Triage & Memory Leak Investigation

When `scripts/soak_test.py` exits with code `2` (`LEAK_SUSPECT`), follow this repair-manual procedure:

### 6.1 Step 1: Confirm Container Memory Growth
Inspect raw memory telemetry over the last 24 hours in Grafana:
- Navigate to: **Dashboards -> FinText Alpha Vectorizer -> Soak Stability & Memory Surveillance**
- Review Panel 1 (Gateway RSS) and Panel 2 (Ingestion RSS).
- Verify whether the climb is linear ($R^2 \ge 0.8$) or stepped/sawtooth.

### 6.2 Step 2: Check Process Memory Distribution inside Container
```bash
docker exec fintext-api-gateway cat /proc/1/status | grep -E "(VmRSS|VmData|VmPeak)"
```

### 6.3 Step 3: Profile Rust Heap Allocations (jemalloc Profiling)
FinText can be booted with `MALLOC_CONF` profiling enabled to dump allocation flamegraphs:
```bash
# 1. Enable jemalloc allocation profiling
export MALLOC_CONF="prof:true,prof_prefix:jeprof.out,lg_prof_interval:30"

# 2. Extract heap profile after 1 hour of soak traffic
jeprof --show_bytes --pdf ./target/release/fintext_api jeprof.out.* > heap_analysis.pdf
```

### 6.4 Step 4: Common Culprits & Rapid Fixes
1. **Unbounded Cache Growth:** Check `AppState` registries (e.g. `monthly_quota_cache`, `provider_health_store`). Ensure TTL eviction threads are actively running.
2. **Channel Backlog:** Check unbounded Tokio `mpsc::unbounded_channel`. Replace with bounded `mpsc::channel(N)` to enforce backpressure.
3. **Connection Leak in sqlx:** Verify that every transaction in `with_tenant` executes either `tx.commit().await` or `tx.rollback().await`.

---

## 7. Automated Ledger Maintenance & Audit Integrity

The stability ledger at `logs/soak_ledger.md` serves as immutable legal and institutional evidence for General Availability (GA) certification and SOC 2 Type II audit examination.

### 7.1 Ledger Integrity Rules
- **Append-Only Principle:** Entries are strictly appended; historical rows must never be overwritten, modified, or reordered.
- **Truthful Failure Recording:** Failed drills (`ERRORS`, `LEAK_SUSPECT`, `LATENCY_DRIFT`) must remain in the ledger alongside the certifying commit hash and corresponding incident ticket reference.
- **Zero Backfill Policy:** Never fabricate or estimate historical soak rows. If an outage or maintenance window interrupts a scheduled soak run, record an `INTERRUPTED` row explicitly.

### 7.2 Ledger Verification Script
Run the following verification snippet to confirm ledger consistency:
```bash
python -c "
from pathlib import Path
ledger = Path('logs/soak_ledger.md')
lines = [l for l in ledger.read_text(encoding='utf-8').splitlines() if l.startswith('| 202')]
print(f'Total certified soak runs recorded: {len(lines)}')
assert len(lines) >= 1, 'Soak ledger is empty!'
"
```

---

## 8. Institutional Client Reporting & SLA Communication

When institutional quantitative funds request proof of platform stability during annual DDQ or pre-trade operational due diligence:

1. **Provide Authoritative Register:** Deliver [`docs/CERTIFIED_METRICS_REGISTER.md`](./CERTIFIED_METRICS_REGISTER.md).
2. **Export Recent Soak Ledger:** Extract the trailing 30 days of entries from `logs/soak_ledger.md`.
3. **Generate Cryptographic Audit Package:**
   ```bash
   sha256sum logs/soak_ledger.md logs/soak_report.json > logs/soak_integrity.sha256
   ```
4. **Link to Public Status Page:** Point compliance teams to `https://status.fintext.ai` (monitored via `GET /v1/status`, detailed in [`docs/STATUS_PAGE_GUIDE.md`](./STATUS_PAGE_GUIDE.md)).

---

## 9. Emergency Node Evacuation & Soak Recovery

If a Kubernetes host node running a multi-hour soak drill requires emergency maintenance or kernel updates:

1. **Graceful Drill Interruption:**
   Send `SIGTERM` or `SIGINT` (Ctrl+C) to `scripts/soak_test.py`. The runner traps the signal, flushes accumulated samples to `logs/soak_history.jsonl`, calculates intermediate linear regression metrics, appends an `INTERRUPTED` row to `logs/soak_ledger.md`, and exits cleanly with code `4`.
2. **Node Cordon & Drain:**
   ```bash
   kubectl cordon node-worker-01
   kubectl drain node-worker-01 --ignore-daemonsets --delete-emptydir-data
   ```
3. **Resume Drill on Target Node:**
   Launch the soak runner on the evacuated node:
   ```bash
   python scripts/soak_test.py --mode window --hours 6.0
   ```

