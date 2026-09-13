"""
Tests for FinText Model Drift Alert Dispatcher.
No external network dependencies — all network operations mocked.
"""

import json
from pathlib import Path
import sys
from urllib.error import URLError
import pytest

PROJECT_ROOT = Path(__file__).resolve().parent.parent
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.model_drift.alerts import (
    AlertSeverity,
    DriftAlert,
    append_alert_log,
    dispatch_alerts,
    format_alert,
    post_webhook,
)


def test_format_alert_critical():
    """Test format_alert constructs a valid DriftAlert with correct fields on CRITICAL status."""
    drift_result = {
        "overall_status": "CRITICAL",
        "breaches": ["Macro F1 drop 0.0800 breaches CRITICAL threshold"],
        "f1_macro_delta": -0.08,
        "ece_delta": 0.06,
        "latency_p95_rel_change_pct": 65.0,
    }
    run_metadata = {
        "git_commit_sha": "a1b2c3d4e5f6",
    }
    alert = format_alert(drift_result, run_metadata)

    assert isinstance(alert, DriftAlert)
    assert alert.severity == "CRITICAL"
    assert alert.overall_status == "CRITICAL"
    assert alert.breaches == ["Macro F1 drop 0.0800 breaches CRITICAL threshold"]
    assert alert.f1_macro_delta == -0.08
    assert alert.ece_delta == 0.06
    assert alert.latency_p95_rel_change_pct == 65.0
    assert alert.git_commit_sha == "a1b2c3d4e5f6"
    assert alert.webhook_status is None
    assert alert.webhook_error is None
    assert alert.timestamp_utc.endswith("Z")


def test_append_alert_log_creates_file(tmp_path: Path):
    """Test append_alert_log creates parent directories and writes parseable JSONL line."""
    log_path = tmp_path / "nested" / "dir" / "alerts.jsonl"
    alert = DriftAlert(
        timestamp_utc="2026-09-13T10:00:00Z",
        severity="CRITICAL",
        overall_status="CRITICAL",
        breaches=["Critical drift breach"],
        f1_macro_delta=-0.085,
        ece_delta=0.055,
        latency_p95_rel_change_pct=62.0,
        git_commit_sha="commit12345",
        webhook_status="sent",
        webhook_error=None,
    )

    append_alert_log(alert, log_path)

    assert log_path.is_file()
    lines = log_path.read_text(encoding="utf-8").strip().splitlines()
    assert len(lines) == 1

    parsed = json.loads(lines[0])
    assert parsed["severity"] == "CRITICAL"
    assert parsed["git_commit_sha"] == "commit12345"
    assert parsed["webhook_status"] == "sent"
    assert parsed["webhook_error"] is None
    assert parsed["breaches"] == ["Critical drift breach"]


def test_append_alert_log_appends(tmp_path: Path):
    """Test append_alert_log appends multiple records on consecutive calls."""
    log_path = tmp_path / "alerts.jsonl"
    alert1 = DriftAlert(
        timestamp_utc="2026-09-13T10:00:00Z",
        severity="WARN",
        overall_status="WARN",
        breaches=["Warning breach"],
        f1_macro_delta=-0.035,
        ece_delta=0.025,
        latency_p95_rel_change_pct=32.0,
        git_commit_sha="sha_first",
        webhook_status="skipped",
        webhook_error=None,
    )
    alert2 = DriftAlert(
        timestamp_utc="2026-09-13T10:05:00Z",
        severity="CRITICAL",
        overall_status="CRITICAL",
        breaches=["Critical breach"],
        f1_macro_delta=-0.075,
        ece_delta=0.055,
        latency_p95_rel_change_pct=65.0,
        git_commit_sha="sha_second",
        webhook_status="sent",
        webhook_error=None,
    )

    append_alert_log(alert1, log_path)
    append_alert_log(alert2, log_path)

    lines = log_path.read_text(encoding="utf-8").strip().splitlines()
    assert len(lines) == 2

    p1 = json.loads(lines[0])
    p2 = json.loads(lines[1])
    assert p1["git_commit_sha"] == "sha_first"
    assert p1["severity"] == "WARN"
    assert p2["git_commit_sha"] == "sha_second"
    assert p2["severity"] == "CRITICAL"


def test_post_webhook_success(monkeypatch: pytest.MonkeyPatch):
    """Test post_webhook returns (True, None) when HTTP 200 response received."""
    class MockSuccessResponse:
        status = 200

        def __enter__(self):
            return self

        def __exit__(self, *args):
            pass

    monkeypatch.setattr(
        "urllib.request.urlopen",
        lambda req, timeout=5.0: MockSuccessResponse(),
    )

    alert = DriftAlert(
        timestamp_utc="2026-09-13T10:00:00Z",
        severity="CRITICAL",
        overall_status="CRITICAL",
        breaches=["Breach"],
        f1_macro_delta=-0.08,
        ece_delta=0.06,
        latency_p95_rel_change_pct=65.0,
        git_commit_sha="test_sha",
    )

    success, err = post_webhook("https://hooks.example.com/drift", alert)
    assert success is True
    assert err is None


def test_post_webhook_failure_network(monkeypatch: pytest.MonkeyPatch):
    """Test post_webhook handles network errors gracefully without raising."""
    def mock_network_error(req, timeout=5.0):
        raise URLError("Connection refused by target host")

    monkeypatch.setattr("urllib.request.urlopen", mock_network_error)

    alert = DriftAlert(
        timestamp_utc="2026-09-13T10:00:00Z",
        severity="CRITICAL",
        overall_status="CRITICAL",
        breaches=["Breach"],
        f1_macro_delta=-0.08,
        ece_delta=0.06,
        latency_p95_rel_change_pct=65.0,
        git_commit_sha="test_sha",
    )

    success, err = post_webhook("https://hooks.example.com/drift", alert)
    assert success is False
    assert err is not None
    assert "Connection refused by target host" in err


def test_post_webhook_failure_http(monkeypatch: pytest.MonkeyPatch):
    """Test post_webhook handles HTTP 500 error code gracefully without raising."""
    class MockServerErrorResponse:
        status = 500

        def __enter__(self):
            return self

        def __exit__(self, *args):
            pass

    monkeypatch.setattr(
        "urllib.request.urlopen",
        lambda req, timeout=5.0: MockServerErrorResponse(),
    )

    alert = DriftAlert(
        timestamp_utc="2026-09-13T10:00:00Z",
        severity="CRITICAL",
        overall_status="CRITICAL",
        breaches=["Breach"],
        f1_macro_delta=-0.08,
        ece_delta=0.06,
        latency_p95_rel_change_pct=65.0,
        git_commit_sha="test_sha",
    )

    success, err = post_webhook("https://hooks.example.com/drift", alert)
    assert success is False
    assert err is not None
    assert "500" in err


def test_dispatch_alerts_ok_no_webhook(tmp_path: Path):
    """Test dispatch_alerts skips webhook when severity is OK even if webhook_url is set."""
    log_path = tmp_path / "alerts.jsonl"
    alert = DriftAlert(
        timestamp_utc="2026-09-13T10:00:00Z",
        severity="OK",
        overall_status="OK",
        breaches=[],
        f1_macro_delta=0.001,
        ece_delta=-0.002,
        latency_p95_rel_change_pct=-5.0,
        git_commit_sha="commit_ok",
    )

    result = dispatch_alerts(
        alert=alert,
        alert_log_path=log_path,
        webhook_url="https://hooks.example.com/drift",
        webhook_on_warn=True,
    )

    assert result.webhook_status == "skipped"
    assert result.webhook_error is None

    # Alert log should still record the entry
    lines = log_path.read_text(encoding="utf-8").strip().splitlines()
    assert len(lines) == 1
    parsed = json.loads(lines[0])
    assert parsed["webhook_status"] == "skipped"
    assert parsed["severity"] == "OK"


def test_dispatch_alerts_critical_webhook_fired(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    """Test dispatch_alerts fires webhook on CRITICAL severity and marks sent."""
    called_urls = []

    def mock_post(url, alert, timeout_s=5.0):
        called_urls.append(url)
        return True, None

    monkeypatch.setattr("scripts.model_drift.alerts.post_webhook", mock_post)

    log_path = tmp_path / "alerts.jsonl"
    alert = DriftAlert(
        timestamp_utc="2026-09-13T10:00:00Z",
        severity="CRITICAL",
        overall_status="CRITICAL",
        breaches=["Severe breach"],
        f1_macro_delta=-0.09,
        ece_delta=0.07,
        latency_p95_rel_change_pct=70.0,
        git_commit_sha="commit_crit",
    )

    result = dispatch_alerts(
        alert=alert,
        alert_log_path=log_path,
        webhook_url="https://hooks.example.com/critical",
    )

    assert len(called_urls) == 1
    assert called_urls[0] == "https://hooks.example.com/critical"
    assert result.webhook_status == "sent"
    assert result.webhook_error is None

    # File log check
    parsed = json.loads(log_path.read_text(encoding="utf-8").strip())
    assert parsed["webhook_status"] == "sent"
    assert parsed["severity"] == "CRITICAL"


def test_dispatch_alerts_warn_respects_flag(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    """Test dispatch_alerts respects webhook_on_warn flag for WARN severity."""
    called = []

    def mock_post(url, alert, timeout_s=5.0):
        called.append(url)
        return True, None

    monkeypatch.setattr("scripts.model_drift.alerts.post_webhook", mock_post)

    # 1. WARN without flag -> skipped
    alert_no_flag = DriftAlert(
        timestamp_utc="2026-09-13T10:00:00Z",
        severity="WARN",
        overall_status="WARN",
        breaches=["Moderate breach"],
        f1_macro_delta=-0.04,
        ece_delta=0.03,
        latency_p95_rel_change_pct=35.0,
        git_commit_sha="warn_commit",
    )
    res_no_flag = dispatch_alerts(
        alert=alert_no_flag,
        alert_log_path=tmp_path / "log_no_flag.jsonl",
        webhook_url="https://hooks.example.com/warn",
        webhook_on_warn=False,
    )
    assert res_no_flag.webhook_status == "skipped"
    assert len(called) == 0

    # 2. WARN with flag -> sent
    alert_with_flag = DriftAlert(
        timestamp_utc="2026-09-13T10:00:00Z",
        severity="WARN",
        overall_status="WARN",
        breaches=["Moderate breach"],
        f1_macro_delta=-0.04,
        ece_delta=0.03,
        latency_p95_rel_change_pct=35.0,
        git_commit_sha="warn_commit",
    )
    res_with_flag = dispatch_alerts(
        alert=alert_with_flag,
        alert_log_path=tmp_path / "log_with_flag.jsonl",
        webhook_url="https://hooks.example.com/warn",
        webhook_on_warn=True,
    )
    assert res_with_flag.webhook_status == "sent"
    assert len(called) == 1


def test_dispatch_alerts_webhook_failure_recorded(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    """Test dispatch_alerts records failed webhook status and error message in alert and log."""
    def mock_failing_post(url, alert, timeout_s=5.0):
        return False, "HTTP 503: Service Unavailable"

    monkeypatch.setattr("scripts.model_drift.alerts.post_webhook", mock_failing_post)

    log_path = tmp_path / "alerts.jsonl"
    alert = DriftAlert(
        timestamp_utc="2026-09-13T10:00:00Z",
        severity="CRITICAL",
        overall_status="CRITICAL",
        breaches=["Major failure breach"],
        f1_macro_delta=-0.12,
        ece_delta=0.09,
        latency_p95_rel_change_pct=85.0,
        git_commit_sha="crit_commit",
    )

    result = dispatch_alerts(
        alert=alert,
        alert_log_path=log_path,
        webhook_url="https://hooks.example.com/fail",
    )

    assert result.webhook_status == "failed"
    assert result.webhook_error == "HTTP 503: Service Unavailable"

    parsed = json.loads(log_path.read_text(encoding="utf-8").strip())
    assert parsed["webhook_status"] == "failed"
    assert parsed["webhook_error"] == "HTTP 503: Service Unavailable"
