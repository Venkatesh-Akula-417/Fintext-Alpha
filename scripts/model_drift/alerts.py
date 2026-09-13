"""Alert dispatch for FinText model drift monitoring."""

from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from enum import Enum
import json
from pathlib import Path
from typing import Any, Dict, Optional, Tuple
from urllib import error as urlerror
from urllib import request as urlrequest


class AlertSeverity(Enum):
    OK = "OK"
    WARN = "WARN"
    CRITICAL = "CRITICAL"


@dataclass
class DriftAlert:
    timestamp_utc: str          # ISO 8601 with Z suffix
    severity: str               # "OK" | "WARN" | "CRITICAL"
    overall_status: str
    breaches: list              # from drift result
    f1_macro_delta: float
    ece_delta: float
    latency_p95_rel_change_pct: float
    git_commit_sha: str
    webhook_status: Optional[str] = None      # "sent" | "failed" | "skipped" | None
    webhook_error: Optional[str] = None


def format_alert(drift_result: Dict[str, Any], run_metadata: Dict[str, Any]) -> DriftAlert:
    """Build a DriftAlert from compute_drift output and run metadata."""
    timestamp_utc = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    overall_status = str(drift_result.get("overall_status", "OK"))
    severity = overall_status

    breaches = list(drift_result.get("breaches") or [])

    f1_val = drift_result.get("f1_macro_delta")
    f1_macro_delta = float(f1_val) if f1_val is not None else 0.0

    ece_val = drift_result.get("ece_delta")
    ece_delta = float(ece_val) if ece_val is not None else 0.0

    lat_val = drift_result.get("latency_p95_rel_change_pct")
    latency_p95_rel_change_pct = float(lat_val) if lat_val is not None else 0.0

    git_commit_sha = str(run_metadata.get("git_commit_sha") or "UNKNOWN")

    return DriftAlert(
        timestamp_utc=timestamp_utc,
        severity=severity,
        overall_status=overall_status,
        breaches=breaches,
        f1_macro_delta=f1_macro_delta,
        ece_delta=ece_delta,
        latency_p95_rel_change_pct=latency_p95_rel_change_pct,
        git_commit_sha=git_commit_sha,
        webhook_status=None,
        webhook_error=None,
    )


def append_alert_log(alert: DriftAlert, path: Path) -> None:
    """
    Ensure parent dir exists and append a single JSON line to path.
    Uses json.dumps with separators=(',', ':') for compactness.
    """
    target_path = Path(path)
    target_path.parent.mkdir(parents=True, exist_ok=True)
    alert_dict = asdict(alert)
    line = json.dumps(alert_dict, separators=(",", ":"), sort_keys=False) + "\n"
    with open(target_path, "a", encoding="utf-8") as f:
        f.write(line)


def post_webhook(url: str, alert: DriftAlert, timeout_s: float = 5.0) -> Tuple[bool, Optional[str]]:
    """
    POST alert JSON payload to url with Content-Type: application/json.
    Returns (True, None) on 2xx response.
    Returns (False, error_message) on any exception or non-2xx response.
    Must not raise or print to stderr.
    """
    try:
        alert_dict = asdict(alert)
        payload = json.dumps(alert_dict, separators=(",", ":"), sort_keys=False).encode("utf-8")
        req = urlrequest.Request(
            url,
            data=payload,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urlrequest.urlopen(req, timeout=timeout_s) as response:
            code = getattr(response, "status", getattr(response, "code", 200))
            if 200 <= code < 300:
                return True, None
            return False, f"HTTP {code}"
    except urlerror.HTTPError as exc:
        return False, f"HTTP {exc.code}: {exc.reason}"
    except urlerror.URLError as exc:
        return False, f"Network error: {exc.reason}"
    except Exception as exc:
        return False, f"Webhook error: {exc}"


def dispatch_alerts(
    alert: DriftAlert,
    alert_log_path: Optional[Path],
    webhook_url: Optional[str],
    webhook_on_warn: bool = False,
) -> DriftAlert:
    """
    Dispatch alert to optional webhook and append to alert log.
    - severity == 'CRITICAL': always fire (if url set)
    - severity == 'WARN': fire only if webhook_on_warn
    - severity == 'OK': never fire
    Returns updated alert.
    """
    should_fire = False
    if webhook_url:
        if alert.severity == AlertSeverity.CRITICAL.value:
            should_fire = True
        elif alert.severity == AlertSeverity.WARN.value and webhook_on_warn:
            should_fire = True

    if should_fire and webhook_url:
        ok, err = post_webhook(webhook_url, alert)
        if ok:
            alert.webhook_status = "sent"
            alert.webhook_error = None
        else:
            alert.webhook_status = "failed"
            alert.webhook_error = err
    else:
        alert.webhook_status = "skipped"
        alert.webhook_error = None

    if alert_log_path is not None:
        append_alert_log(alert, Path(alert_log_path))

    return alert
