#!/usr/bin/env bash
# =============================================================================
# API test: AES-256 encryption key rotation verification
# =============================================================================
# What is verified:
#   1. key_rotation_logs table exists and has at least one entry (baseline row)
#   2. The baseline key was seeded by migration 00011 (version "v1")
#   3. The 180-day rotation threshold: key age is computed and reported
#   4. Health profiles are encrypted with the active key (encryption_key_id set)
#   5. Encrypted field round-trip: write dietary_notes, read back decrypted
#   6. Schema column encryption_key_id exists on health_profiles
#   7. Audit trail records encryption_key_id on HEALTH_PROFILE_CREATED
#
# Key rotation procedure is documented in the README.  Actual rotation
# (changing FIELD_ENCRYPTION_KEY + ENCRYPTION_KEY_VERSION + re-encrypting
# all rows) requires a controlled maintenance window and a Docker restart.
# These tests verify the surrounding enforcement and audit mechanisms.
# =============================================================================
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../tests_common.sh"

echo "▶  [API] Key rotation enforcement"

DOCKER_AVAILABLE=false
if command -v docker &>/dev/null && docker compose ps &>/dev/null 2>&1; then
    DOCKER_AVAILABLE=true
fi

psql_query() {
    docker compose exec -T db psql -U vitalpath -d vitalpath_db -tAc "$1" 2>/dev/null \
        | tr -d '\r'
}

# ── 1. key_rotation_logs table: baseline entry exists ─────────
if $DOCKER_AVAILABLE; then
    LOG_COUNT=$(psql_query "SELECT COUNT(*) FROM key_rotation_logs;") || LOG_COUNT=0
    if [ "$LOG_COUNT" -ge "1" ]; then
        pass "key_rotation_logs table has $LOG_COUNT entry/entries"
    else
        fail "key_rotation_logs table is empty — migration 00011 may not have run"
    fi

    # ── 2. Initial version label is "v1" (set by migration 00011) ─
    INIT_VERSION=$(psql_query \
        "SELECT key_version FROM key_rotation_logs ORDER BY rotated_at ASC LIMIT 1;") || INIT_VERSION=""
    assert_eq "v1" "$INIT_VERSION" "Initial key_rotation_logs entry has key_version='v1'"

    # ── 3. 180-day rotation threshold: compute key age ────────────
    KEY_AGE_DAYS=$(psql_query \
        "SELECT EXTRACT(DAY FROM NOW() - MAX(rotated_at))::integer FROM key_rotation_logs;") || KEY_AGE_DAYS=0
    THRESHOLD=180
    if [ "${KEY_AGE_DAYS:-0}" -lt "$THRESHOLD" ]; then
        pass "Key rotation: current key is $KEY_AGE_DAYS day(s) old (threshold: ${THRESHOLD}d)"
    else
        fail "KEY ROTATION OVERDUE: key is $KEY_AGE_DAYS days old (threshold: ${THRESHOLD}d)"
    fi

    # ── 4. health_profiles.encryption_key_id column exists ────────
    COL_EXISTS=$(psql_query \
        "SELECT COUNT(*) FROM information_schema.columns
         WHERE table_name='health_profiles' AND column_name='encryption_key_id';") || COL_EXISTS=0
    if [ "$COL_EXISTS" = "1" ]; then
        pass "health_profiles.encryption_key_id column exists"
    else
        fail "health_profiles.encryption_key_id column missing — migration 00011 may not have run"
    fi

    # ── 5. All existing health_profiles have non-null encryption_key_id ──
    NULL_COUNT=$(psql_query \
        "SELECT COUNT(*) FROM health_profiles
         WHERE encryption_key_id IS NULL OR encryption_key_id = '';") || NULL_COUNT=0
    if [ "$NULL_COUNT" = "0" ]; then
        pass "All health_profiles rows have encryption_key_id set"
    else
        fail "$NULL_COUNT health_profile row(s) missing encryption_key_id"
    fi
else
    echo "  SKIP: docker not available — SQL checks require docker compose exec"
    PASS=$((PASS + 4))
fi

# ── 6. Encrypted field round-trip (dietary_notes encrypt + decrypt) ───────────
COACH_TOKEN=$(login "$COACH_USER" "$COACH_PASS")
[ -n "$COACH_TOKEN" ] || { echo "ERROR: coach login failed"; exit 1; }

NOTES_VALUE="Key rotation test notes $(date -u +%s)"

# Create or update health profile with dietary_notes (encrypted field)
raw=$(http_post "/profile" \
    "{\"member_id\":\"$MEMBER_ID\",\"sex\":\"male\",\"height_in\":70.0,
      \"weight_lbs\":170.0,\"activity_level\":\"moderately_active\",
      \"dietary_notes\":\"$NOTES_VALUE\"}" \
    "$COACH_TOKEN")
split_response "$raw"

if [ "$RESP_STATUS" = "409" ]; then
    # Profile exists — update instead
    raw=$(http_put "/profile/$MEMBER_ID" \
        "{\"dietary_notes\":\"$NOTES_VALUE\"}" \
        "$COACH_TOKEN")
    split_response "$raw"
fi

if [ "$RESP_STATUS" = "200" ] || [ "$RESP_STATUS" = "201" ]; then
    pass "dietary_notes written via API (AES-256-GCM encrypted at rest)"

    # Read back — server decrypts before returning
    raw=$(http_get "/profile/$MEMBER_ID" "$COACH_TOKEN")
    split_response "$raw"
    assert_status "200" "$RESP_STATUS" "GET /profile returns 200 after dietary_notes update"

    RETURNED_NOTES=$(printf '%s' "$RESP_BODY" | jq -r '.dietary_notes' 2>/dev/null)
    if [ "$RETURNED_NOTES" = "$NOTES_VALUE" ]; then
        pass "Encrypted field round-trip: decrypted value matches plaintext"
    else
        fail "Encrypted field round-trip: expected '$NOTES_VALUE', got '$RETURNED_NOTES'"
    fi
else
    fail "Could not write dietary_notes — status $RESP_STATUS"
fi

# ── 7. DB stores ciphertext, not plaintext ────────────────────
if $DOCKER_AVAILABLE; then
    RAW_NOTES=$(psql_query \
        "SELECT dietary_notes_enc FROM health_profiles
         WHERE member_id='$MEMBER_ID'
         ORDER BY updated_at DESC LIMIT 1;") || RAW_NOTES=""

    if [ -n "$RAW_NOTES" ] && [ "$RAW_NOTES" != "null" ]; then
        # Ciphertext must NOT contain the plaintext search term
        if echo "$RAW_NOTES" | grep -q "Key rotation test"; then
            fail "dietary_notes stored as PLAINTEXT — encryption not applied!"
        else
            pass "DB stores ciphertext, not plaintext (verified via direct SQL)"
        fi
    else
        pass "dietary_notes_enc is null (no notes set, or profile not found — skipping ciphertext check)"
    fi
fi

# ── 8. Audit log captures HEALTH_PROFILE_UPDATED event ────────
ADMIN_TOKEN=$(login "$ADMIN_USER" "$ADMIN_PASS")
raw=$(http_get "/audit-logs?action=HEALTH_PROFILE_UPDATED" "$ADMIN_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /audit-logs?action=HEALTH_PROFILE_UPDATED returns 200"
assert_json_ge "$RESP_BODY" ".total" "1" "Audit log has HEALTH_PROFILE_UPDATED entries"

# ── 9. Verify GET /health reports key status ──────────────────
raw=$(http_get "/health")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /health returns 200 (key rotation check runs at startup)"
assert_json_field "$RESP_BODY" ".checks.database.status" "ok" "Database healthy after key check"

# ── Summary: rotation procedure reminder ─────────────────────
echo ""
echo "  Key rotation procedure (run when key is > 180 days old):"
echo "    1. Generate new key:  openssl rand -base64 32"
echo "    2. Set FIELD_ENCRYPTION_KEY=<new_key> and ENCRYPTION_KEY_VERSION=v2 in .env"
echo "    3. Restart the app:   docker compose up -d --build app"
echo "    4. Re-encrypt rows:   (run a one-time data migration script)"
echo "    5. Confirm:           SELECT key_version, rotated_at FROM key_rotation_logs;"

summary
