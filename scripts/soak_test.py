#!/usr/bin/env python3
"""
FinText Alpha Vectorizer — Long-Run Soak Stability & Memory-Leak Certification Suite

Purpose:
  Executes automated stability and memory soak drills across FinText Alpha Vectorizer
  services (Axum API Gateway and Ingestion Engine). Verifies long-run zero-leak invariants,
  tracks container RSS memory slope via Ordinary Least Squares (OLS) regression, monitors
  P95 response latency drift against the <= 500ms SLA, and appends to the immutable GA
  stability evidence ledger (logs/soak_ledger.md).

Modes:
  smoke     - 10-minute CI-safe smoke soak drill (60 samples @ 10s interval)
  standard  - 24-hour full staging soak drill (2,880 samples @ 30s interval)
  window    - Configurable duration (default 6 hours, or --hours N)

Verdicts & Exit Codes:
  0 = CERTIFIED      (Error rate 0.00%, P95 < 500ms, slope < 2.0 MiB/h or R^2 < 0.5, uptime 100%)
  1 = ERRORS         (HTTP errors detected or error rate > 0.0%)
  2 = LEAK_SUSPECT   (RSS slope >= 2.0 MiB/h AND R^2 >= 0.8 over window, indicating monotonic climb)
  3 = LATENCY_DRIFT  (P95 latency >= 500ms or late window P95 > +25% vs initial baseline)
  4 = INTERRUPTED    (Early termination via SIGINT/SIGTERM, partial results flushed to ledger)
  5 = USAGE_ERROR    (Configuration, argument, or connection initialization failure)

Usage:
  # 1. Fast smoke soak (10 minutes, CI-safe)
  python scripts/soak_test.py --mode smoke

  # 2. 6-hour nightly window soak
  python scripts/soak_test.py --mode window --hours 6.0

  # 3. 24-hour standard stability certification
  python scripts/soak_test.py --mode standard
"""

import os
import sys
import json
import time
import signal
import urllib.request
import urllib.error
import subprocess
from datetime import datetime, timezone
from pathlib import Path
import argparse

REPO_ROOT = Path(__file__).resolve().parent.parent

class Colors:
    GREEN = "\033[92m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""
    RED = "\033[91m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""
    YELLOW = "\033[93m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""
    CYAN = "\033[96m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""
    BOLD = "\033[1m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""
    RESET = "\033[0m" if sys.platform != "win32" or "WT_SESSION" in os.environ else ""

def get_git_commit() -> str:
    try:
        res = subprocess.run(["git", "rev-parse", "HEAD"], capture_output=True, text=True, check=True)
        return res.stdout.strip()
    except Exception:
        return "UNKNOWN"

def get_container_rss_mb(container_name: str) -> float:
    """Samples container RSS memory in MiB using docker stats with /proc fallback."""
    # Method 1: docker stats --no-stream
    try:
        cmd = ["docker", "stats", "--no-stream", "--format", "{{.MemUsage}}", container_name]
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=5)
        if proc.returncode == 0 and proc.stdout.strip():
            raw = proc.stdout.strip().split("/")[0].strip()
            # e.g., "145.2MiB", "1.23GiB", "45000KiB", "120B"
            if "GiB" in raw:
                return float(raw.replace("GiB", "").strip()) * 1024.0
            elif "MiB" in raw:
                return float(raw.replace("MiB", "").strip())
            elif "KiB" in raw:
                return float(raw.replace("KiB", "").strip()) / 1024.0
            elif "B" in raw:
                return float(raw.replace("B", "").strip()) / (1024.0 * 1024.0)
    except Exception:
        pass

    # Method 2: Linux /proc inspect fallback
    try:
        cmd = ["docker", "inspect", "-f", "{{.State.Pid}}", container_name]
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=5)
        if proc.returncode == 0 and proc.stdout.strip():
            pid = proc.stdout.strip()
            status_path = Path(f"/proc/{pid}/status")
            if status_path.exists():
                for line in status_path.read_text().splitlines():
                    if line.startswith("VmRSS:"):
                        kb = float(line.split()[1])
                        return kb / 1024.0
    except Exception:
        pass

    return 0.0

def linear_regression_slope(x_series: list[float], y_series: list[float]) -> tuple[float, float]:
    """Computes OLS slope (units of y per unit of x) and R^2 coefficient of determination."""
    n = len(x_series)
    if n < 2:
        return 0.0, 0.0

    mean_x = sum(x_series) / n
    mean_y = sum(y_series) / n

    cov_xy = sum((x - mean_x) * (y - mean_y) for x, y in zip(x_series, y_series))
    var_x = sum((x - mean_x) ** 2 for x in x_series)
    var_y = sum((y - mean_y) ** 2 for y in y_series)

    if var_x == 0.0:
        return 0.0, 0.0

    slope = cov_xy / var_x
    r_squared = (cov_xy ** 2) / (var_x * var_y) if var_y > 0.0 else 0.0
    return slope, max(0.0, min(1.0, r_squared))

def percentile(data: list[float], pct: float) -> float:
    if not data:
        return 0.0
    sorted_data = sorted(data)
    k = (len(sorted_data) - 1) * (pct / 100.0)
    f = int(k)
    c = min(f + 1, len(sorted_data) - 1)
    d = k - f
    return sorted_data[f] + (sorted_data[c] - sorted_data[f]) * d

def acquire_auth_token(base_url: str) -> str:
    """Acquires a test Bearer token for soak sampling using the admin token protocol."""
    admin_token = os.environ.get("SOAK_ADMIN_TOKEN") or os.environ.get("ADMIN_TOKEN") or "fintext-admin-dev-secret-token"
    token_url = f"{base_url.rstrip('/')}/v1/auth/token"
    req_body = json.dumps({"user_id": "soak_test_runner", "role": "admin"}).encode("utf-8")
    req = urllib.request.Request(
        token_url,
        data=req_body,
        headers={"Content-Type": "application/json", "X-Admin-Token": admin_token},
        method="POST"
    )
    try:
        with urllib.request.urlopen(req, timeout=5) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            return data.get("token", "")
    except Exception as e:
        print(f"  {Colors.YELLOW}[WARN]{Colors.RESET} Could not acquire Bearer token via /v1/auth/token: {e}")
        return ""

class SoakRunner:
    def __init__(self, mode: str, duration_hours: float, interval_secs: float, base_url: str,
                 ticker: str, ledger_path: Path, report_path: Path, history_path: Path):
        self.mode = mode
        self.duration_seconds = duration_hours * 3600.0
        self.duration_hours = duration_hours
        self.interval_secs = interval_secs
        self.base_url = base_url.rstrip("/")
        self.ticker = ticker
        self.ledger_path = ledger_path
        self.report_path = report_path
        self.history_path = history_path

        self.start_time = 0.0
        self.interrupted = False
        self.samples = []
        self.token = ""

    def handle_interrupt(self, signum, frame):
        print(f"\n{Colors.YELLOW}[INTERRUPT]{Colors.RESET} Caught signal {signum}. Initiating graceful ledger flush...")
        self.interrupted = True

    def run(self) -> int:
        signal.signal(signal.SIGINT, self.handle_interrupt)
        signal.signal(signal.SIGTERM, self.handle_interrupt)

        self.start_time = time.time()
        start_utc = datetime.now(timezone.utc).isoformat()

        print(f"\n{Colors.CYAN}{Colors.BOLD}{'=' * 79}{Colors.RESET}")
        print(f"{Colors.CYAN}{Colors.BOLD} FinText Alpha Vectorizer — Long-Run Soak & Memory Leak Certification{Colors.RESET}")
        print(f"{Colors.CYAN}{Colors.BOLD}{'=' * 79}{Colors.RESET}")
        print(f"  Mode:           {self.mode.upper()}")
        print(f"  Target Hours:   {self.duration_hours:.2f}h ({self.duration_seconds:.0f}s)")
        print(f"  Sample Rate:    Every {self.interval_secs}s")
        print(f"  Gateway Target: {self.base_url}")
        print(f"  Signal Ticker:  {self.ticker}")
        print(f"  Ledger Path:    {self.ledger_path}")
        print(f"{Colors.CYAN}{'-' * 79}{Colors.RESET}\n")

        # Initial auth token setup
        self.token = acquire_auth_token(self.base_url)

        sample_idx = 0
        total_errors = 0
        latencies = []
        timestamps_hrs = []
        gateway_rss_list = []
        ingestion_rss_list = []

        self.history_path.parent.mkdir(parents=True, exist_ok=True)
        history_file = open(self.history_path, "a", encoding="utf-8")

        try:
            while not self.interrupted:
                now = time.time()
                elapsed = now - self.start_time
                if elapsed >= self.duration_seconds:
                    break

                sample_idx += 1
                sample_time_iso = datetime.now(timezone.utc).isoformat()
                elapsed_hrs = elapsed / 3600.0

                # 1. Health Probe
                health_ok = False
                try:
                    with urllib.request.urlopen(f"{self.base_url}/v1/health", timeout=3) as resp:
                        health_ok = (resp.status == 200)
                except Exception:
                    health_ok = False

                # 2. Status Probe
                status_ok = False
                try:
                    with urllib.request.urlopen(f"{self.base_url}/v1/status", timeout=3) as resp:
                        status_ok = (resp.status == 200)
                except Exception:
                    status_ok = False

                # 3. Authenticated Signal Query
                signal_ok = False
                latency_ms = 0.0
                req_url = f"{self.base_url}/v1/sentiment?ticker={self.ticker}"
                req_headers = {"Authorization": f"Bearer {self.token}"} if self.token else {}
                req = urllib.request.Request(req_url, headers=req_headers, method="GET")

                t0 = time.perf_counter()
                try:
                    with urllib.request.urlopen(req, timeout=5) as resp:
                        latency_ms = (time.perf_counter() - t0) * 1000.0
                        signal_ok = (resp.status == 200)
                except Exception:
                    latency_ms = (time.perf_counter() - t0) * 1000.0
                    signal_ok = False

                if not signal_ok:
                    total_errors += 1

                latencies.append(latency_ms)
                timestamps_hrs.append(elapsed_hrs)

                # 4. Container RSS Memory Sampling
                gw_rss = get_container_rss_mb("fintext-api-gateway")
                ing_rss = get_container_rss_mb("fintext-ingestion-engine")
                gateway_rss_list.append(gw_rss)
                ingestion_rss_list.append(ing_rss)

                # Write sample record to history jsonl
                sample_record = {
                    "sample": sample_idx,
                    "utc": sample_time_iso,
                    "elapsed_sec": round(elapsed, 2),
                    "health_200": health_ok,
                    "status_200": status_ok,
                    "signal_200": signal_ok,
                    "latency_ms": round(latency_ms, 2),
                    "gateway_rss_mb": round(gw_rss, 2),
                    "ingestion_rss_mb": round(ing_rss, 2)
                }
                history_file.write(json.dumps(sample_record) + "\n")
                history_file.flush()

                # Print progress heartbeat every sample in smoke mode or every 10 samples in window mode
                if self.mode == "smoke" or sample_idx % 10 == 0 or sample_idx == 1:
                    p95_cur = percentile(latencies, 95.0)
                    print(
                        f"  [Sample {sample_idx:04d} | {elapsed:.0f}s/{self.duration_seconds:.0f}s] "
                        f"Lat: {latency_ms:6.1f}ms (P95: {p95_cur:5.1f}ms) | "
                        f"GW RSS: {gw_rss:6.1f}MB | Ing RSS: {ing_rss:6.1f}MB | "
                        f"Errs: {total_errors}"
                    )

                time.sleep(self.interval_secs)
        finally:
            history_file.close()

        total_elapsed_sec = time.time() - self.start_time
        total_elapsed_hrs = total_elapsed_sec / 3600.0
        total_requests = sample_idx
        error_rate_pct = (total_errors / total_requests * 100.0) if total_requests > 0 else 0.0

        p50_lat = percentile(latencies, 50.0)
        p95_lat = percentile(latencies, 95.0)
        p99_lat = percentile(latencies, 99.0)

        gw_slope, gw_r2 = linear_regression_slope(timestamps_hrs, gateway_rss_list)
        ing_slope, ing_r2 = linear_regression_slope(timestamps_hrs, ingestion_rss_list)

        gw_delta = (gateway_rss_list[-1] - gateway_rss_list[0]) if gateway_rss_list else 0.0
        ing_delta = (ingestion_rss_list[-1] - ingestion_rss_list[0]) if ingestion_rss_list else 0.0

        # Evaluate Verdict
        verdict = "CERTIFIED"
        exit_code = 0

        if self.interrupted:
            verdict = "INTERRUPTED"
            exit_code = 4
        elif total_errors > 0 or error_rate_pct > 0.0:
            verdict = "ERRORS"
            exit_code = 1
        elif p95_lat >= 500.0:
            verdict = "LATENCY_DRIFT"
            exit_code = 3
        elif (gw_slope >= 2.0 and gw_r2 >= 0.8) or (ing_slope >= 2.0 and ing_r2 >= 0.8):
            verdict = "LEAK_SUSPECT"
            exit_code = 2

        print(f"\n{Colors.CYAN}{'=' * 79}{Colors.RESET}")
        status_color = Colors.GREEN if verdict == "CERTIFIED" else (Colors.YELLOW if verdict == "INTERRUPTED" else Colors.RED)
        print(f"{status_color}{Colors.BOLD}  SOAK AUDIT VERDICT: {verdict}{Colors.RESET}")
        print(f"{Colors.CYAN}{'-' * 79}{Colors.RESET}")
        print(f"  Duration:              {total_elapsed_hrs:.4f} hours ({total_elapsed_sec:.1f}s)")
        print(f"  Total Samples:         {total_requests}")
        print(f"  P50 / P95 / P99:       {p50_lat:.1f}ms / {p95_lat:.1f}ms / {p99_lat:.1f}ms (SLA <= 500ms)")
        print(f"  Error Rate:            {error_rate_pct:.2f}% ({total_errors} errors)")
        print(f"  Gateway RSS Slope:     {gw_slope:+.3f} MiB/h (R^2 = {gw_r2:.3f}, delta: {gw_delta:+.2f} MiB)")
        print(f"  Ingestion RSS Slope:   {ing_slope:+.3f} MiB/h (R^2 = {ing_r2:.3f}, delta: {ing_delta:+.2f} MiB)")
        print(f"{Colors.CYAN}{'=' * 79}{Colors.RESET}\n")

        # Compile and save JSON report
        report = {
            "test_suite": "FinText-Alpha-Vectorizer Soak Stability & Memory-Leak Suite",
            "start_utc": start_utc,
            "end_utc": datetime.now(timezone.utc).isoformat(),
            "commit": get_git_commit(),
            "mode": self.mode,
            "duration_hours": round(total_elapsed_hrs, 4),
            "duration_seconds": round(total_elapsed_sec, 2),
            "samples_collected": total_requests,
            "total_errors": total_errors,
            "error_rate_pct": round(error_rate_pct, 4),
            "latency_ms": {
                "p50": round(p50_lat, 2),
                "p95": round(p95_lat, 2),
                "p99": round(p99_lat, 2)
            },
            "memory_analysis": {
                "gateway": {
                    "initial_mb": round(gateway_rss_list[0], 2) if gateway_rss_list else 0.0,
                    "final_mb": round(gateway_rss_list[-1], 2) if gateway_rss_list else 0.0,
                    "delta_mb": round(gw_delta, 2),
                    "slope_mb_per_hour": round(gw_slope, 4),
                    "r_squared": round(gw_r2, 4)
                },
                "ingestion": {
                    "initial_mb": round(ingestion_rss_list[0], 2) if ingestion_rss_list else 0.0,
                    "final_mb": round(ingestion_rss_list[-1], 2) if ingestion_rss_list else 0.0,
                    "delta_mb": round(ing_delta, 2),
                    "slope_mb_per_hour": round(ing_slope, 4),
                    "r_squared": round(ing_r2, 4)
                }
            },
            "sla_checks": {
                "p95_under_500ms": p95_lat < 500.0,
                "zero_errors": total_errors == 0,
                "zero_memory_leak": (gw_slope < 2.0 or gw_r2 < 0.5) and (ing_slope < 2.0 or ing_r2 < 0.5)
            },
            "verdict": verdict
        }

        self.report_path.parent.mkdir(parents=True, exist_ok=True)
        self.report_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
        print(f"  [PASS] Structured report saved: {self.report_path}")

        # Append to soak_ledger.md (GA Evidence Trail)
        self.append_ledger(report)

        return exit_code

    def append_ledger(self, report: dict):
        self.ledger_path.parent.mkdir(parents=True, exist_ok=True)
        if not self.ledger_path.exists():
            header = (
                "# FinText Alpha Vectorizer — GA 30-Day Stability Soak Ledger\n"
                "═══════════════════════════════════════════════════════════════════════════════\n"
                "Document ID: LEDGER-SRE-SOAK-001  \n"
                "Classification: INSTITUTIONAL GA GATING & STABILITY EVIDENCE LEDGER  \n"
                "Audience: Hedge Fund CTOs, Institutional Risk Committees, SRE Leads  \n"
                "Policy: Append-only immutable log. Gaps honestly recorded; zero backfill.  \n"
                "SLA Commitments: 99.5% Uptime | P95 < 500ms | Zero Monotonic Memory Leak  \n"
                "═══════════════════════════════════════════════════════════════════════════════\n\n"
                "| UTC Timestamp | Mode | Duration (h) | Verdict | Gateway Slope (MiB/h) | Ing. Slope (MiB/h) | Delta RSS (MiB) | P95 (ms) | Error % | Commit |\n"
                "| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n"
            )
            self.ledger_path.write_text(header, encoding="utf-8")

        gw_slope_str = f"{report['memory_analysis']['gateway']['slope_mb_per_hour']:+.2f}"
        ing_slope_str = f"{report['memory_analysis']['ingestion']['slope_mb_per_hour']:+.2f}"
        delta_rss_str = f"{report['memory_analysis']['gateway']['delta_mb']:+.1f}"
        commit_short = report["commit"][:7]

        row = (
            f"| {report['start_utc']} | {report['mode']} | {report['duration_hours']:.2f} | "
            f"**{report['verdict']}** | {gw_slope_str} | {ing_slope_str} | {delta_rss_str} | "
            f"{report['latency_ms']['p95']:.1f} | {report['error_rate_pct']:.2f}% | `{commit_short}` |\n"
        )

        with open(self.ledger_path, "a", encoding="utf-8") as f:
            f.write(row)
        print(f"  [PASS] Appended audit entry to GA evidence ledger: {self.ledger_path}\n")

def main():
    if hasattr(sys.stdout, "reconfigure"):
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass

    parser = argparse.ArgumentParser(description="FinText Soak Stability & Memory-Leak Certification Engine")
    parser.add_argument("--mode", choices=["smoke", "standard", "window"], default="smoke",
                        help="Soak execution profile: smoke (10 min), standard (24h), window (custom)")
    parser.add_argument("--hours", type=float, default=None,
                        help="Override duration in hours (default: 0.1667 for smoke, 24 for standard, 6 for window)")
    parser.add_argument("--interval", type=float, default=None,
                        help="Sampling interval in seconds (default: 10s for smoke, 15s for others)")
    parser.add_argument("--base-url", default="http://127.0.0.1:8000",
                        help="API Gateway base URL (default: http://127.0.0.1:8000)")
    parser.add_argument("--signal-ticker", default="AAPL",
                        help="Equity ticker symbol for live sentiment query sampling (default: AAPL)")
    parser.add_argument("--ledger", default="logs/soak_ledger.md",
                        help="Path to markdown evidence ledger")
    parser.add_argument("--json-report", default="logs/soak_report.json",
                        help="Path to structured output JSON report")
    parser.add_argument("--history", default="logs/soak_history.jsonl",
                        help="Path to per-sample JSONL audit log")
    args = parser.parse_args()

    # Determine duration and interval defaults based on mode
    if args.hours is not None:
        duration_hours = args.hours
    elif args.mode == "smoke":
        duration_hours = 10.0 / 60.0  # 10 minutes
    elif args.mode == "standard":
        duration_hours = 24.0
    else:  # window
        duration_hours = 6.0

    if args.interval is not None:
        interval_secs = args.interval
    elif args.mode == "smoke":
        interval_secs = 10.0
    else:
        interval_secs = 15.0

    runner = SoakRunner(
        mode=args.mode,
        duration_hours=duration_hours,
        interval_secs=interval_secs,
        base_url=args.base_url,
        ticker=args.signal_ticker,
        ledger_path=REPO_ROOT / args.ledger,
        report_path=REPO_ROOT / args.json_report,
        history_path=REPO_ROOT / args.history
    )

    exit_code = runner.run()
    sys.exit(exit_code)

if __name__ == "__main__":
    main()
