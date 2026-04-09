#!/usr/bin/env bash
# =============================================================================
# VitalPath daily encrypted backup
# =============================================================================
# Produces: <BACKUP_DIR>/vitalpath_backup_YYYY-MM-DD_HHMMSS.sql.gz.enc
#           <BACKUP_DIR>/vitalpath_backup_YYYY-MM-DD_HHMMSS.sha256
#
# Encryption: AES-256-CBC, PBKDF2 (100k iterations), random salt per backup.
# Compression: gzip -9 applied before encryption to minimise ciphertext size.
# Retention:   Backups older than RETAIN_DAYS are deleted after each run.
# =============================================================================

set -euo pipefail

# ── Configuration ─────────────────────────────────────────────────────────────
BACKUP_DIR="${BACKUP_DIR:-/backups}"
RETAIN_DAYS="${RETAIN_DAYS:-30}"
DATABASE_URL="${DATABASE_URL:?DATABASE_URL must be set}"
BACKUP_ENCRYPTION_KEY="${BACKUP_ENCRYPTION_KEY:?BACKUP_ENCRYPTION_KEY must be set}"

TIMESTAMP=$(date -u +"%Y-%m-%d_%H%M%S")
BACKUP_BASE="vitalpath_backup_${TIMESTAMP}"
BACKUP_FILE="${BACKUP_DIR}/${BACKUP_BASE}.sql.gz.enc"
CHECKSUM_FILE="${BACKUP_DIR}/${BACKUP_BASE}.sha256"
LOG_FILE="${BACKUP_DIR}/backup.log"

# ── Helpers ───────────────────────────────────────────────────────────────────
log() {
    local level="$1"; shift
    echo "[$(date -u '+%Y-%m-%dT%H:%M:%SZ')] ${level} $*" | tee -a "$LOG_FILE"
}

die() {
    log "ERROR" "$*"
    exit 1
}

# ── Setup ─────────────────────────────────────────────────────────────────────
mkdir -p "$BACKUP_DIR"
log "INFO" "━━━ Backup started: ${BACKUP_BASE} ━━━"

# ── Dump → compress → encrypt ─────────────────────────────────────────────────
log "INFO" "Running pg_dump..."
pg_dump "$DATABASE_URL" \
    --format=plain \
    --no-password \
  | gzip -9 \
  | openssl enc -aes-256-cbc -pbkdf2 -iter 100000 -salt \
      -pass "pass:${BACKUP_ENCRYPTION_KEY}" \
      -out "$BACKUP_FILE" \
  || die "Dump/encrypt pipeline failed"

BACKUP_SIZE=$(du -sh "$BACKUP_FILE" | cut -f1)
log "INFO" "Encrypted backup written: ${BACKUP_FILE} (${BACKUP_SIZE})"

# ── Checksum ──────────────────────────────────────────────────────────────────
sha256sum "$BACKUP_FILE" > "$CHECKSUM_FILE"
log "INFO" "SHA-256: $(awk '{print $1}' "$CHECKSUM_FILE")"

# Verify checksum is self-consistent
sha256sum --check "$CHECKSUM_FILE" --quiet \
  || die "Checksum self-check failed immediately after writing"
log "INFO" "Checksum verified"

# ── Quick decrypt test (detect key/corruption issues early) ───────────────────
log "INFO" "Running decryption smoke test..."
openssl enc -aes-256-cbc -pbkdf2 -iter 100000 -d \
    -pass "pass:${BACKUP_ENCRYPTION_KEY}" \
    -in "$BACKUP_FILE" \
  | gzip -d \
  | head -c 512 > /dev/null \
  || die "Decryption smoke test failed — backup may be unusable"
log "INFO" "Decryption smoke test passed"

# ── Write manifest entry ──────────────────────────────────────────────────────
MANIFEST="${BACKUP_DIR}/manifest.csv"
if [ ! -f "$MANIFEST" ]; then
    echo "timestamp,filename,size_bytes,sha256" > "$MANIFEST"
fi
SIZE_BYTES=$(stat -c%s "$BACKUP_FILE")
SHA256=$(awk '{print $1}' "$CHECKSUM_FILE")
echo "${TIMESTAMP},${BACKUP_BASE}.sql.gz.enc,${SIZE_BYTES},${SHA256}" >> "$MANIFEST"

# ── Prune backups older than RETAIN_DAYS ─────────────────────────────────────
log "INFO" "Pruning backups older than ${RETAIN_DAYS} days..."
while IFS= read -r old_enc; do
    old_base="${old_enc%.sql.gz.enc}"
    rm -f "$old_enc" "${old_base}.sha256"
    log "INFO" "Pruned: $(basename "$old_enc")"
done < <(find "$BACKUP_DIR" -maxdepth 1 \
              -name "vitalpath_backup_*.sql.gz.enc" \
              -mtime "+${RETAIN_DAYS}")

RETAINED=$(find "$BACKUP_DIR" -maxdepth 1 -name "vitalpath_backup_*.sql.gz.enc" | wc -l)
log "INFO" "━━━ Backup complete. Retained: ${RETAINED} backup(s) ━━━"
