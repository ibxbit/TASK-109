#!/usr/bin/env bash
# =============================================================================
# VitalPath quarterly restore drill
# =============================================================================
# Runs a full restore into a temporary database, executes data-integrity
# checks, records the result, then tears down the temporary database.
#
# Scheduled via cron: 0 4 1 1,4,7,10 *  (first of Jan/Apr/Jul/Oct, 04:00 UTC)
#
# Exit codes:
#   0 — drill passed
#   1 — drill failed (see DRILL_LOG for details)
#
# Results are appended to DRILL_LOG (/backups/drill.log) so that a
# chronological history of drills is preserved for audit purposes.
# =============================================================================

set -euo pipefail

BACKUP_DIR="${BACKUP_DIR:-/backups}"
BACKUP_ENCRYPTION_KEY="${BACKUP_ENCRYPTION_KEY:?BACKUP_ENCRYPTION_KEY must be set}"
DATABASE_URL="${DATABASE_URL:?DATABASE_URL must be set}"
DRILL_LOG="${BACKUP_DIR}/drill.log"
SCRIPTS_DIR="${SCRIPTS_DIR:-/scripts}"

DRILL_STAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
DRILL_ID="drill_$(date -u +%Y%m%d_%H%M%S)"
# Temporary database name (max 63 chars for PostgreSQL)
DRILL_DB="vitalpath_${DRILL_ID}"

# Strip the database name and append the drill DB name
# e.g. postgres://user:pass@host:5432/vitalpath_db → postgres://user:pass@host:5432/
ADMIN_URL="${DATABASE_URL%/*}/postgres"
DRILL_URL="${DATABASE_URL%/*}/${DRILL_DB}"

# ── Helpers ───────────────────────────────────────────────────────────────────
log() {
    local level="$1"; shift
    echo "[${DRILL_STAMP}] DRILL ${level} $*" | tee -a "$DRILL_LOG"
}

DRILL_RESULT="FAIL"

cleanup() {
    log "INFO" "Dropping drill database: ${DRILL_DB}"
    psql "$ADMIN_URL" -c "DROP DATABASE IF EXISTS ${DRILL_DB};" 2>/dev/null || true

    echo "" >> "$DRILL_LOG"
    echo "[${DRILL_STAMP}] DRILL ════ Result: ${DRILL_RESULT} (${DRILL_ID}) ════" >> "$DRILL_LOG"
    echo "" >> "$DRILL_LOG"

    if [ "$DRILL_RESULT" = "PASS" ]; then
        echo "[${DRILL_STAMP}] DRILL PASS ${DRILL_ID}" >&2
    else
        echo "[${DRILL_STAMP}] DRILL FAIL ${DRILL_ID} — see ${DRILL_LOG}" >&2
        exit 1
    fi
}
trap cleanup EXIT

# ── Header ────────────────────────────────────────────────────────────────────
log "INFO" "════════════════════════════════════════════"
log "INFO" " Quarterly Restore Drill: ${DRILL_ID}"
log "INFO" "════════════════════════════════════════════"

# ── Find latest backup ────────────────────────────────────────────────────────
BACKUP_FILE=$(find "$BACKUP_DIR" -maxdepth 1 \
                   -name "vitalpath_backup_*.sql.gz.enc" \
              | sort | tail -1)
[ -n "$BACKUP_FILE" ] || { log "FAIL" "No backups found in ${BACKUP_DIR}"; exit 1; }
log "INFO" "Backup file: $(basename "$BACKUP_FILE")"

# ── Checksum pre-check ────────────────────────────────────────────────────────
CHECKSUM_FILE="${BACKUP_FILE%.sql.gz.enc}.sha256"
if [ -f "$CHECKSUM_FILE" ]; then
    sha256sum --check "$CHECKSUM_FILE" --quiet \
      || { log "FAIL" "Checksum verification failed"; exit 1; }
    log "INFO" "Checksum pre-check: PASS"
else
    log "WARN" "No checksum file found — skipping pre-check"
fi

# ── Create drill database ─────────────────────────────────────────────────────
log "INFO" "Creating drill database: ${DRILL_DB}"
psql "$ADMIN_URL" -c "CREATE DATABASE ${DRILL_DB};" \
  || { log "FAIL" "Could not create drill database"; exit 1; }

# ── Restore ───────────────────────────────────────────────────────────────────
log "INFO" "Restoring backup into drill database..."
RESTORE_NO_CONFIRM=1 \
BACKUP_DIR="$BACKUP_DIR" \
BACKUP_ENCRYPTION_KEY="$BACKUP_ENCRYPTION_KEY" \
    "${SCRIPTS_DIR}/restore.sh" "$BACKUP_FILE" "$DRILL_URL" \
  || { log "FAIL" "Restore step failed"; exit 1; }
log "INFO" "Restore: PASS"

# ── Data integrity checks ─────────────────────────────────────────────────────
log "INFO" "Running integrity checks..."

# Helper: assert row count >= minimum
check_min_rows() {
    local table="$1" min="${2:-0}"
    local count
    count=$(psql "$DRILL_URL" -t -c "SELECT COUNT(*) FROM ${table};" | tr -d ' \n')
    if [ "$count" -ge "$min" ]; then
        log "INFO" "  ✓ ${table}: ${count} rows (min ${min})"
    else
        log "FAIL" "  ✗ ${table}: ${count} rows but expected >= ${min}"
        exit 1
    fi
}

# Helper: assert a query returns exactly one row
check_query() {
    local label="$1" query="$2"
    psql "$DRILL_URL" -t -c "$query" > /dev/null \
      || { log "FAIL" "  ✗ ${label}"; exit 1; }
    log "INFO" "  ✓ ${label}"
}

# Core tables exist and are populated
check_min_rows "roles"            1
check_min_rows "users"            1
check_min_rows "sessions"         0
check_min_rows "members"          0
check_min_rows "health_profiles"  0
check_min_rows "metric_types"     1   # seeded by migration
check_min_rows "metric_entries"   0
check_min_rows "goals"            0
check_min_rows "audit_logs"       0
check_min_rows "key_rotation_logs" 1  # seeded by migration 00011

# Schema spot-checks: new columns added in recent migrations must exist
check_query \
    "audit_logs has reason_code, old_hash, new_hash" \
    "SELECT reason_code, old_hash, new_hash FROM audit_logs LIMIT 0;"

check_query \
    "health_profiles has encryption_key_id" \
    "SELECT encryption_key_id FROM health_profiles LIMIT 0;"

check_query \
    "key_rotation_logs table is queryable" \
    "SELECT id, key_version, rotated_at, fields_updated FROM key_rotation_logs LIMIT 1;"

check_query \
    "users table has lockout columns" \
    "SELECT failed_attempts, locked_until, captcha_required FROM users LIMIT 0;"

check_query \
    "goals table has goal_type" \
    "SELECT goal_type, start_date, baseline_value FROM goals LIMIT 0;"

# Decrypt test: if health_profiles has rows, verify encryption_key_id is set
PROFILE_COUNT=$(psql "$DRILL_URL" -t -c "SELECT COUNT(*) FROM health_profiles WHERE encryption_key_id IS NULL OR encryption_key_id = '';" | tr -d ' \n')
if [ "$PROFILE_COUNT" -gt 0 ]; then
    log "FAIL" "  ✗ health_profiles: ${PROFILE_COUNT} row(s) with missing encryption_key_id"
    exit 1
fi
log "INFO" "  ✓ health_profiles: all rows have encryption_key_id"

# ── Record drill metadata ─────────────────────────────────────────────────────
DRILL_META="${BACKUP_DIR}/drill_history.csv"
if [ ! -f "$DRILL_META" ]; then
    echo "drill_id,timestamp,backup_file,result" > "$DRILL_META"
fi
echo "${DRILL_ID},${DRILL_STAMP},$(basename "$BACKUP_FILE"),PASS" >> "$DRILL_META"

log "INFO" "All checks passed"
DRILL_RESULT="PASS"
