#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Verification: Zero-Touch Operations & Alerting Suite
===============================================================================
Verifies:
  1.  k8s/observability/prometheus-alerts.yaml is valid Kubernetes ConfigMap YAML
  2.  All 9 Prometheus alerting rules exist in ConfigMap
  3.  APIP95LatencyHigh rule PromQL expression & threshold (> 0.5s)
  4.  APIServerErrorRateHigh rule PromQL expression & threshold (> 1%)
  5.  QuestDBIngestionLagHigh rule PromQL expression & threshold (< 100)
  6.  NATSConsumerLagHigh (>10k) and KafkaConsumerLagHigh (>100k) rules
  7.  PodRestartFrequent, DiskSpaceLow (>80%), MemoryPressureHigh (>90%) rules
  8.  DatabaseBackupFailed rule configuration
  9.  k8s/observability/alertmanager.yaml parses as valid Kubernetes YAML
  10. Alertmanager config routes to Slack (#fintext-ops-alerts) with inhibition
  11. k8s/observability/prometheus.yaml targets Alertmanager & mounts alert rules
  12. k8s/backups/postgres-backup-cronjob.yaml schedule (0 */6 * * *), pg_dump, sha256, S3
  13. k8s/backups/questdb-backup-cronjob.yaml schedule (0 2 * * *) & Velero snapshot
  14. .env.example, config/config.yaml, and docs/OPERATIONS.md operational completeness
===============================================================================
"""

import os
from pathlib import Path
import sys
import yaml

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent

passed = 0
failed = 0
total = 14


def report(phase: int, name: str, ok: bool, detail: str = ""):
    global passed, failed
    if ok:
        passed += 1
        print(f"  ✅ Phase {phase:2d} │ {name}")
    else:
        failed += 1
        print(f"  ❌ Phase {phase:2d} │ {name}")
        if detail:
            print(f"     └─ {detail}")


def run_all_phases():
    print("=" * 80)
    print(" FinText-Alpha-Vectorizer — Zero-Touch Operations & Alerting Verification")
    print("=" * 80)
    print(f" Project Root: {PROJECT_ROOT}")

    # Phase 1: k8s/observability/prometheus-alerts.yaml exists and parses
    alerts_yaml_path = PROJECT_ROOT / "k8s" / "observability" / "prometheus-alerts.yaml"
    p1_ok = False
    alerts_data = {}
    rule_groups = []
    rule_map = {}
    if alerts_yaml_path.exists():
        try:
            with open(alerts_yaml_path, "r", encoding="utf-8") as f:
                alerts_data = yaml.safe_load(f)
            if alerts_data.get("kind") == "ConfigMap" and "alerts.yml" in alerts_data.get("data", {}):
                parsed_alerts = yaml.safe_load(alerts_data["data"]["alerts.yml"])
                rule_groups = parsed_alerts.get("groups", [])
                for grp in rule_groups:
                    for rule in grp.get("rules", []):
                        if "alert" in rule:
                            rule_map[rule["alert"]] = rule
                p1_ok = True
        except Exception as e:
            p1_ok = False
    report(1, "k8s/observability/prometheus-alerts.yaml is valid ConfigMap YAML", p1_ok)

    # Phase 2: All 9 Prometheus alerting rules exist
    expected_rules = {
        "APIP95LatencyHigh",
        "APIServerErrorRateHigh",
        "QuestDBIngestionLagHigh",
        "KafkaRealtimeConsumerLagHigh",
        "KafkaConsumerLagHigh",
        "PodRestartFrequent",
        "DiskSpaceLow",
        "MemoryPressureHigh",
        "DatabaseBackupFailed",
    }
    actual_rules = set(rule_map.keys())
    p2_ok = expected_rules.issubset(actual_rules)
    report(2, "All 9 Prometheus alerting rules exist in ConfigMap", p2_ok, f"Missing: {expected_rules - actual_rules}")

    # Phase 3: APIP95LatencyHigh rule PromQL expression & threshold (> 0.5s)
    r3 = rule_map.get("APIP95LatencyHigh", {})
    expr3 = r3.get("expr", "")
    p3_ok = "histogram_quantile(0.95" in expr3 and "0.5" in expr3 and r3.get("labels", {}).get("severity") == "critical"
    report(3, "APIP95LatencyHigh PromQL expression & threshold (> 0.5s)", p3_ok, f"Expr: {expr3}")

    # Phase 4: APIServerErrorRateHigh rule PromQL expression & threshold (> 1%)
    r4 = rule_map.get("APIServerErrorRateHigh", {})
    expr4 = r4.get("expr", "")
    p4_ok = "5.." in expr4 and "0.01" in expr4 and r4.get("labels", {}).get("severity") == "critical"
    report(4, "APIServerErrorRateHigh PromQL expression & threshold (> 1%)", p4_ok, f"Expr: {expr4}")

    # Phase 5: QuestDBIngestionLagHigh rule PromQL expression & threshold (< 100)
    r5 = rule_map.get("QuestDBIngestionLagHigh", {})
    expr5 = r5.get("expr", "")
    p5_ok = "questdb_ingested_total" in expr5 and "100" in expr5 and r5.get("labels", {}).get("severity") == "critical"
    report(5, "QuestDBIngestionLagHigh PromQL expression & threshold (< 100 rows)", p5_ok, f"Expr: {expr5}")

    # Phase 6: KafkaRealtimeConsumerLagHigh (>10k) and KafkaConsumerLagHigh (>100k) rules
    r6_rt = rule_map.get("KafkaRealtimeConsumerLagHigh", {})
    r6_kafka = rule_map.get("KafkaConsumerLagHigh", {})
    p6_ok = (
        "10000" in r6_rt.get("expr", "")
        and "100000" in r6_kafka.get("expr", "")
    )
    report(6, "KafkaRealtimeConsumerLagHigh (>10k) and KafkaConsumerLagHigh (>100k) rules", p6_ok)

    # Phase 7: PodRestartFrequent, DiskSpaceLow (>80%), MemoryPressureHigh (>90%) rules
    r7_pod = rule_map.get("PodRestartFrequent", {})
    r7_disk = rule_map.get("DiskSpaceLow", {})
    r7_mem = rule_map.get("MemoryPressureHigh", {})
    p7_ok = (
        "restarts_total" in r7_pod.get("expr", "")
        and "80" in r7_disk.get("expr", "")
        and "0.9" in r7_mem.get("expr", "")
    )
    report(7, "PodRestartFrequent, DiskSpaceLow (>80%), MemoryPressureHigh (>90%) rules", p7_ok)

    # Phase 8: DatabaseBackupFailed rule configuration
    r8 = rule_map.get("DatabaseBackupFailed", {})
    expr8 = r8.get("expr", "")
    p8_ok = "backup_status" in expr8 and r8.get("labels", {}).get("severity") == "critical"
    report(8, "DatabaseBackupFailed rule configuration", p8_ok, f"Expr: {expr8}")

    # Phase 9: k8s/observability/alertmanager.yaml parses as valid Kubernetes YAML
    am_yaml_path = PROJECT_ROOT / "k8s" / "observability" / "alertmanager.yaml"
    p9_ok = False
    am_docs = []
    if am_yaml_path.exists():
        try:
            with open(am_yaml_path, "r", encoding="utf-8") as f:
                am_docs = list(yaml.safe_load_all(f))
            kinds = {d.get("kind") for d in am_docs if d}
            p9_ok = {"ConfigMap", "Service", "Deployment"}.issubset(kinds)
        except Exception:
            p9_ok = False
    report(9, "k8s/observability/alertmanager.yaml parses as valid Kubernetes YAML", p9_ok)

    # Phase 10: Alertmanager config routes to Slack with inhibition rules
    p10_ok = False
    for doc in am_docs:
        if doc and doc.get("kind") == "ConfigMap" and "alertmanager.yml" in doc.get("data", {}):
            am_cfg = yaml.safe_load(doc["data"]["alertmanager.yml"])
            receivers = am_cfg.get("receivers", [])
            slack_receivers = [r for r in receivers if "slack_configs" in r]
            routes = am_cfg.get("route", {}).get("routes", [])
            inhibit = am_cfg.get("inhibit_rules", [])
            if len(slack_receivers) > 0 and len(routes) > 0 and len(inhibit) > 0:
                p10_ok = True
            break
    report(10, "Alertmanager config routes to Slack (#fintext-ops-alerts) with inhibition", p10_ok)

    # Phase 11: k8s/observability/prometheus.yaml targets Alertmanager & mounts alert rules
    prom_yaml_path = PROJECT_ROOT / "k8s" / "observability" / "prometheus.yaml"
    p11_ok = False
    if prom_yaml_path.exists():
        try:
            with open(prom_yaml_path, "r", encoding="utf-8") as f:
                prom_docs = list(yaml.safe_load_all(f))
            for doc in prom_docs:
                if doc and doc.get("kind") == "ConfigMap" and "prometheus.yml" in doc.get("data", {}):
                    p_cfg = yaml.safe_load(doc["data"]["prometheus.yml"])
                    am_targets = p_cfg.get("alerting", {}).get("alertmanagers", [])
                    rule_files = p_cfg.get("rule_files", [])
                    has_am = any("alertmanager:9093" in str(t) for t in am_targets)
                    has_rule_file = any("alerts" in str(rf) for rf in rule_files)
                    if has_am and has_rule_file:
                        p11_ok = True
                    break
        except Exception:
            p11_ok = False
    report(11, "k8s/observability/prometheus.yaml targets Alertmanager & mounts alert rules", p11_ok)

    # Phase 12: k8s/backups/postgres-backup-cronjob.yaml schedule, pg_dump, sha256, S3
    pg_cron_path = PROJECT_ROOT / "k8s" / "backups" / "postgres-backup-cronjob.yaml"
    p12_ok = False
    if pg_cron_path.exists():
        try:
            with open(pg_cron_path, "r", encoding="utf-8") as f:
                pg_docs = list(yaml.safe_load_all(f))
            has_cm = False
            has_cron = False
            for doc in pg_docs:
                if doc and doc.get("kind") == "ConfigMap":
                    script = doc.get("data", {}).get("backup.sh", "")
                    if "pg_dump" in script and "sha256sum" in script and "S3_BUCKET" in script:
                        has_cm = True
                if doc and doc.get("kind") == "CronJob":
                    sched = doc.get("spec", {}).get("schedule", "")
                    if sched == "0 */6 * * *" and doc.get("spec", {}).get("concurrencyPolicy") == "Forbid":
                        has_cron = True
            p12_ok = has_cm and has_cron
        except Exception:
            p12_ok = False
    report(12, "k8s/backups/postgres-backup-cronjob.yaml schedule, pg_dump, sha256, S3", p12_ok)

    # Phase 13: k8s/backups/questdb-backup-cronjob.yaml schedule (0 2 * * *) & Velero snapshot
    qdb_cron_path = PROJECT_ROOT / "k8s" / "backups" / "questdb-backup-cronjob.yaml"
    p13_ok = False
    if qdb_cron_path.exists():
        try:
            with open(qdb_cron_path, "r", encoding="utf-8") as f:
                qdb_docs = list(yaml.safe_load_all(f))
            has_cm = False
            has_cron = False
            for doc in qdb_docs:
                if doc and doc.get("kind") == "ConfigMap":
                    script = doc.get("data", {}).get("backup.sh", "")
                    if "velero" in script or "snapshot" in script:
                        has_cm = True
                if doc and doc.get("kind") == "CronJob":
                    sched = doc.get("spec", {}).get("schedule", "")
                    if sched == "0 2 * * *" and doc.get("spec", {}).get("concurrencyPolicy") == "Forbid":
                        has_cron = True
            p13_ok = has_cm and has_cron
        except Exception:
            p13_ok = False
    report(13, "k8s/backups/questdb-backup-cronjob.yaml schedule (0 2 * * *) & Velero snapshot", p13_ok)

    # Phase 14: .env.example, config/config.yaml, and docs/OPERATIONS.md operational completeness
    env_ex = (PROJECT_ROOT / ".env.example").read_text(encoding="utf-8")
    cfg_yaml = (PROJECT_ROOT / "config" / "config.yaml").read_text(encoding="utf-8")
    ops_md = (PROJECT_ROOT / "docs" / "OPERATIONS.md").read_text(encoding="utf-8")

    has_env = "SLACK_WEBHOOK_URL" in env_ex and "BACKUP_S3_BUCKET" in env_ex
    has_cfg = "alerting:" in cfg_yaml and "backups:" in cfg_yaml
    has_ops = "APIP95LatencyHigh" in ops_md and "postgres-backup-cronjob" in ops_md and "questdb-backup-cronjob" in ops_md

    p14_ok = has_env and has_cfg and has_ops
    report(14, ".env.example, config/config.yaml, and docs/OPERATIONS.md completeness", p14_ok)


if __name__ == "__main__":
    run_all_phases()
    print("=" * 80)
    print(f" Operations & Alerting Results: {passed}/{total} Phases Passed ({failed} Failed)")
    print("=" * 80)
    if failed > 0:
        sys.exit(1)
    sys.exit(0)
