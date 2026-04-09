#!/usr/bin/env bash
# =============================================================================
# VitalPath restore from encrypted backup
# =============================================================================
# Usage:
#   restore.sh [BACKUP_FILE] [TARGET_DB_URL]
#
# BACKUP_FILE    — path to a .sql.gz.enc file.
#                  Default: the most recent backup in BACKUP_DIR.
# TARGET_DB_URL  — PostgreSQL DSN.
#                  Default: $DATABASE_URL (production database).
#
# Set RESTORE_NO_CONFIRM=1 to skip the interactive confirmation prompt
# (required for automated restore drills).
# =============================================================================

set -euo pipefail

BACKUP_DIR="${BACKUP_DIR:-/backups}"
BACKUP_ENCRYPTION_KEY="${BACKUP_ENCRYPTION_KEY:?BACKUP_ENCRYPTION_KEY must be set}"
LOG_FILE="${BACKUP_DIR}/restore.log"

# ── Helpers ───────────────────────────────────────────────────────────────────
log() {
    local level="$1"; shift
    echo "[$(date -u '+%Y-%m-%dT%H:%M:%SZ')] ${level} $*" | tee -a "$LOG_FILE"
}

die() {
    log "ERROR" "$*"
    exit 1
}

# ── Resolve backup file ───────────────────────────────────────────────────────
if [ -n "${1:-}" ]; then
    BACKUP_FILE="$1"
else
    BACKUP_FILE=$(find "$BACKUP_DIR" -maxdepth 1 \
                       -name "vitalpath_backup_*.sql.gz.enc" \
                  | sort | tail -1)
    [ -n "$BACKUP_FILE" ] || die "No backup files found in ${BACKUP_DIR}"
fi

[ -f "$BACKUP_FILE" ] || die "Backup file not found: ${BACKUP_FILE}"

# ── Resolve target database ───────────────────────────────────────────────────
TARGET_URL="${2:-${DATABASE_URL:?DATABASE_URL must be set}}"

log "INFO" "━━━ Restore started ━━━"
log "INFO" "  Backup : ${BACKUP_FILE}"
log "INFO" "  Target : ${TARGET_URL%%@*}@…  (credentials redacted)"

# ── Checksum verification ─────────────────────────────────────────────────────
CHECKSUM_FILE="${BACKUP_FILE%.sql.gz.enc}.sha256"
if [ -f "$CHECKSUM_FILE" ]; then
    log "INFO" "Verifying SHA-256 checksum..."
    sha256sum --check "$CHECKSUM_FILE" --quiet \
      || die "Checksum mismatch — backup may be corrupted or tampered with"
    log "INFO" "Checksum OK: $(awk '{print $1}' "$CHECKSUM_FILE")"
else
    log "WARN" "No checksum file found — skipping integrity pre-check"
fi

# ── Confirmation ─────────────────────────────────────────────────────────────
if [ "${RESTORE_NO_CONFIRM:-0}" != "1" ]; then
    echo ""
    echo "  ╔═══════════════════════════════════════════════════╗"
    echo "  ║  ⚠  WARNING: DESTRUCTIVE OPERATION               ║"
    echo "  ║                                                   ║"
    echo "  ║  This will DROP and recreate all tables in the   ║"
    echo "  ║  target database. All existing data will be lost. ║"
    echo "  ║                                                   ║"
    echo "  ║  Type  yes  to proceed, anything else to abort.  ║"
    echo "  ╚═══════════════════════════════════════════════════╝"
    echo -n "  > "
    read -r ANSWER
    if [ "$ANSWER" != "yes" ]; then
        log "INFO" "Restore aborted by user"
        exit 0
    fi
fi

# ── Decrypt → decompress → restore ───────────────────────────────────────────
log "INFO" "Decrypting and restoring (this may take a while)..."
openssl enc -aes-256-cbc -pbkdf2 -iter 100000 -d \
    -pass "pass:${BACKUP_ENCRYPTION_KEY}" \
    -in "$BACKUP_FILE" \
  | gzip -d \
  | psql "$TARGET_URL" \
      --single-transaction \
      --set ON_ERROR_STOP=on \
  || die "Restore pipeline failed"

log "INFO" "━━━ Restore complete ━━━"
