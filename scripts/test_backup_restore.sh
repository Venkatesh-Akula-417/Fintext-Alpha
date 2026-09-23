#!/usr/bin/env bash
# ══════════════════════════════════════════════════════════════════════════════
# FinText-Alpha-Vectorizer — Automated Disaster Recovery & Restore Test Runner
# ══════════════════════════════════════════════════════════════════════════════
# Executes end-to-end non-destructive restoration drill for Linux, K8s, and CI/CD:
# 1. Inspects latest backup archive from local staging or S3
# 2. Runs verification test suite via Python runner
# 3. Validates row counts for 4 PIT tables + sentiment_records
# 4. Asserts SCD Type 2 bi-temporal validity & org_id tenant isolation
# 5. Benchmarks RTO against < 4h SLA and RPO against <= 1h target
# 6. Emits structured JSON report and returns exit code 0
# ══════════════════════════════════════════════════════════════════════════════

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
LOG_DIR="${REPO_ROOT}/logs"
REPORT_PATH="${LOG_DIR}/backup_restore_test_report.json"

mkdir -p "${LOG_DIR}"

echo "================================================================="
echo " FinText Alpha Vectorizer — Disaster Recovery Restoration Drill"
echo "================================================================="
echo "Execution Environment: $(uname -s) $(uname -m)"
echo "Timestamp: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
echo "Report Destination: ${REPORT_PATH}"

# Check for Python 3
PYTHON_BIN=""
if command -v python3 >/dev/null 2>&1; then
  PYTHON_BIN="python3"
elif command -v python >/dev/null 2>&1; then
  PYTHON_BIN="python"
else
  echo "[ERROR] Python 3 runtime is required to execute restoration drill." >&2
  exit 1
fi

echo "Using Python runtime: $(${PYTHON_BIN} --version)"

# Execute restore test suite
START_TIME=$(date +%s)
${PYTHON_BIN} "${SCRIPT_DIR}/test_restore.py" --json-report "${REPORT_PATH}" --isolated-mode
EXIT_CODE=$?
END_TIME=$(date +%s)
TOTAL_DURATION=$((END_TIME - START_TIME))

if [ ${EXIT_CODE} -eq 0 ]; then
  echo "================================================================="
  echo " Disaster Recovery Drill Succeeded in ${TOTAL_DURATION}s."
  echo " Certified: RPO <= 1h, RTO <= 4h. Report: ${REPORT_PATH}"
  echo "================================================================="
  exit 0
else
  echo "[ERROR] Disaster Recovery Drill Failed with Exit Code ${EXIT_CODE}" >&2
  exit ${EXIT_CODE}
fi
