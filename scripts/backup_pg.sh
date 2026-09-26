#!/usr/bin/env bash
# ══════════════════════════════════════════════════════════════════════════════
# FinText-Alpha-Vectorizer — Production Database Backup Automation
# ══════════════════════════════════════════════════════════════════════════════
# Executes scheduled PostgreSQL backups with gzip compression, SHA-256 sidecars,
# local staging retention, optional AWS S3 cloud synchronization, and CloudWatch
# failure alerting. Conforms to SEC Rule 17a-4 and SOC 2 CC8.1 / A1.2 criteria.
#
# Citations:
# - DB Container:   docker-compose.yml line 176 (fintext-postgres)
# - DB User:        docker-compose.yml line 179 (fintext)
# - DB Name:        docker-compose.yml line 181 (fintext_metadata)
# - S3 Bucket:      infra/terraform/s3.tf line 25 (fintext-backups-${var.environment})
# - S3 Prefix:      infra/terraform/s3.tf line 77 (postgres/)
# - AWS Region:     infra/terraform/variables.tf line 8 (us-east-1)
# ══════════════════════════════════════════════════════════════════════════════

set -euo pipefail

# ── Architecture Constants ───────────────────────────────────────────────────
readonly DB_CONTAINER="fintext-postgres"          # docker-compose.yml line 176
readonly DB_USER="fintext"                       # docker-compose.yml line 179
readonly DB_NAME="fintext_metadata"              # docker-compose.yml line 181
readonly S3_BUCKET="fintext-backups-production"  # infra/terraform/s3.tf line 25
readonly S3_PREFIX="postgres"                    # infra/terraform/s3.tf line 77
readonly AWS_REGION="us-east-1"                  # infra/terraform/variables.tf line 8
readonly MAX_LOCAL_KEEP=3                        # Retain last 3 local backups

# ── Environment & Directory Discovery ────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
LOGS_DIR="${REPO_ROOT}/logs"
LOCAL_BACKUP_DIR="${LOGS_DIR}/backups_local"
LEDGER_FILE="${LOGS_DIR}/backup_ledger.md"
LOCK_FILE="${LOGS_DIR}/.backup_ledger.lock"

# ── Parse Arguments & Operational Mode ───────────────────────────────────────
# Usage: MODE={local|s3} [CADENCE={hourly|nightly}] bash backup_pg.sh [mode] [cadence]
MODE="${1:-${MODE:-local}}"
CADENCE="${2:-${CADENCE:-hourly}}"

if [[ "${MODE}" != "local" && "${MODE}" != "s3" ]]; then
    echo "ERROR: Invalid mode '${MODE}'. Allowed values: local, s3" >&2
    exit 1
fi

if [[ "${CADENCE}" != "hourly" && "${CADENCE}" != "nightly" ]]; then
    echo "ERROR: Invalid cadence '${CADENCE}'. Allowed values: hourly, nightly" >&2
    exit 1
fi

mkdir -p "${LOCAL_BACKUP_DIR}" "${LOGS_DIR}"

# ── Ledger Management (Flock Synchronized) ───────────────────────────────────
init_ledger() {
    if [[ ! -f "${LEDGER_FILE}" ]]; then
        cat << 'EOF' > "${LEDGER_FILE}"
# FinText Alpha Vectorizer — Production Backup Ledger
════════════════════════════════════════════════════════════════════════════════

| Backup ID | UTC Timestamp | Mode | Bytes | SHA-256 Digest | Duration (s) | Status |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
EOF
    fi
}

append_ledger() {
    local row="$1"
    init_ledger
    (
        if command -v flock >/dev/null 2>&1; then
            flock -x 200
        fi
        echo "${row}" >> "${LEDGER_FILE}"
    ) 200>"${LOCK_FILE}"
}

# ── Failure & Sentinel Handler ───────────────────────────────────────────────
START_EPOCH=$(date +%s)
UTC_STAMP="$(date -u +"%Y%m%d_%H%M%SZ")"
ISO_TIMESTAMP="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
BACKUP_ID="fintext_${CADENCE}_${UTC_STAMP}"
DUMP_FILE="${LOCAL_BACKUP_DIR}/${BACKUP_ID}.dump.gz"
SHA_FILE="${DUMP_FILE}.sha256"

on_failure() {
    local exit_code=$?
    local end_epoch=$(date +%s)
    local duration=$((end_epoch - START_EPOCH))
    echo "[ERROR] Backup job failed with exit code ${exit_code} (Duration: ${duration}s)" >&2

    # Append FAIL line to ledger as an institutional sentinel
    local fail_row="| ${BACKUP_ID} | ${ISO_TIMESTAMP} | ${MODE} | 0 | 0000000000000000000000000000000000000000000000000000000000000000 | ${duration} | FAIL |"
    append_ledger "${fail_row}"

    # Report failure to CloudWatch if in s3 mode
    if [[ "${MODE}" == "s3" ]]; then
        if command -v aws >/dev/null 2>&1; then
            aws cloudwatch put-metric-data \
                --namespace fintext \
                --metric-name BackupJobFailed \
                --value 1 \
                --region "${AWS_REGION}" 2>/dev/null || true
        fi
    fi

    exit "${exit_code}"
}

trap on_failure ERR

# ── Backup Execution Core ────────────────────────────────────────────────────
echo "================================================================="
echo " FinText Alpha Vectorizer — Scheduled Database Backup Job"
echo "================================================================="
echo "Mode:      ${MODE}"
echo "Cadence:   ${CADENCE}"
echo "Backup ID: ${BACKUP_ID}"
echo "Timestamp: ${ISO_TIMESTAMP}"
echo "Target:    ${DUMP_FILE}"

# Verify Docker container is reachable
if ! docker inspect --format="{{.State.Running}}" "${DB_CONTAINER}" 2>/dev/null | grep -q "true"; then
    echo "[ERROR] Target PostgreSQL container '${DB_CONTAINER}' is not running." >&2
    exit 1
fi

echo "Streaming PostgreSQL custom-format compressed dump from container '${DB_CONTAINER}'..."
# Execute pg_dump inside container with custom archive format and level 6 compression
docker exec -i "${DB_CONTAINER}" pg_dump \
    -U "${DB_USER}" \
    -d "${DB_NAME}" \
    --format=custom \
    --compress=6 > "${DUMP_FILE}"

if [[ ! -s "${DUMP_FILE}" ]]; then
    echo "[ERROR] Backup dump file was created but is empty." >&2
    exit 1
fi

FILE_BYTES=$(wc -c < "${DUMP_FILE}" | tr -d ' ')

# Compute cryptographic SHA-256 sidecar
echo "Generating SHA-256 cryptographic digest sidecar..."
if command -v sha256sum >/dev/null 2>&1; then
    SHA256_HASH=$(sha256sum "${DUMP_FILE}" | awk '{print $1}')
elif command -v shasum >/dev/null 2>&1; then
    SHA256_HASH=$(shasum -a 256 "${DUMP_FILE}" | awk '{print $1}')
else
    # Fallback to python
    SHA256_HASH=$(python -c "import hashlib; print(hashlib.sha256(open('${DUMP_FILE}','rb').read()).hexdigest())")
fi

echo "${SHA256_HASH}  $(basename "${DUMP_FILE}")" > "${SHA_FILE}"

# ── S3 Cloud Shipping (Mode = s3) ────────────────────────────────────────────
if [[ "${MODE}" == "s3" ]]; then
    echo "Shipping backup and SHA-256 sidecar to AWS S3: s3://${S3_BUCKET}/${S3_PREFIX}/${CADENCE}/"
    aws s3 cp "${DUMP_FILE}" "s3://${S3_BUCKET}/${S3_PREFIX}/${CADENCE}/$(basename "${DUMP_FILE}")" --only-show-errors
    aws s3 cp "${SHA_FILE}" "s3://${S3_BUCKET}/${S3_PREFIX}/${CADENCE}/$(basename "${SHA_FILE}")" --only-show-errors
    echo "S3 shipping complete."

    # Publish CloudWatch 0 (Success) metric if reachable
    if command -v aws >/dev/null 2>&1; then
        aws cloudwatch put-metric-data \
            --namespace fintext \
            --metric-name BackupJobFailed \
            --value 0 \
            --region "${AWS_REGION}" 2>/dev/null || true
    fi
fi

# ── Local Retention Pruning ──────────────────────────────────────────────────
# Retain last 3 local dumps in logs/backups_local/
echo "Applying local staging retention (keeping last ${MAX_LOCAL_KEEP} dumps)..."
find "${LOCAL_BACKUP_DIR}" -name "fintext_*.dump.gz" -type f | sort -r | tail -n +$((MAX_LOCAL_KEEP + 1)) | while read -r old_dump; do
    echo "  Pruning stale local backup: $(basename "${old_dump}")"
    rm -f "${old_dump}" "${old_dump}.sha256"
done

# ── Ledger Certification Append ──────────────────────────────────────────────
END_EPOCH=$(date +%s)
DURATION=$((END_EPOCH - START_EPOCH))
SUCCESS_ROW="| ${BACKUP_ID} | ${ISO_TIMESTAMP} | ${MODE} | ${FILE_BYTES} | ${SHA256_HASH} | ${DURATION} | SUCCESS |"
append_ledger "${SUCCESS_ROW}"

echo "================================================================="
echo " Backup Completed Successfully in ${DURATION}s."
echo " Size:      ${FILE_BYTES} bytes"
echo " SHA-256:   ${SHA256_HASH}"
echo " Ledger:    ${LEDGER_FILE}"
echo "================================================================="
exit 0
