#!/usr/bin/env bash
# =============================================================================
# API test: HMAC-SHA256 request signing on privileged endpoints
# =============================================================================
# Endpoint: POST /analytics/export
# Protocol:
#   X-Timestamp: <unix epoch seconds>
#   X-Signature: hex(HMAC-SHA256(HMAC_SECRET, "{ts}:POST:/analytics/export"))
#
# Also requires: Bearer token with care_coach or administrator role.
# =============================================================================
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../tests_common.sh"

echo "▶  [API] HMAC request signing"

# ── Verify openssl is available (needed for HMAC computation) ─
if ! command -v openssl &>/dev/null; then
    echo "  SKIP: openssl not available on host — HMAC tests require openssl"
    exit 0
fi

# ── Retrieve the configured HMAC_SECRET from the running container ────────────
# This uses the actual secret so tests remain valid regardless of the value.
if command -v docker &>/dev/null; then
    HMAC_SECRET=$(docker compose exec -T app \
        sh -c 'echo "$HMAC_SECRET"' 2>/dev/null | tr -d '\r\n') || true
fi
# Fall back to the docker-compose default if exec failed
HMAC_SECRET="${HMAC_SECRET:-change_me_in_production_use_openssl_rand_hex_32}"

# ── Helper: compute valid HMAC signature ──────────────────────
# Usage: make_sig <timestamp>
make_sig() {
    local ts="$1"
    local message="${ts}:POST:/analytics/export"
    printf '%s' "$message" \
        | openssl dgst -sha256 -hmac "$HMAC_SECRET" \
        | awk '{print $NF}'
}

# ── Setup: obtain a care-coach token (required by endpoint) ───
COACH_TOKEN=$(login "$COACH_USER" "$COACH_PASS")
[ -n "$COACH_TOKEN" ] || { echo "ERROR: coach login failed"; exit 1; }

EXPORT_BODY='{"format":"csv"}'
ENDPOINT="/analytics/export"

# ── Test 1: Valid HMAC → accepted ─────────────────────────────
TS=$(date -u +%s)
SIG=$(make_sig "$TS")

raw=$(curl -s -w "\n%{http_code}" \
    -X POST "$BASE_URL$ENDPOINT" \
    -H "Authorization: Bearer $COACH_TOKEN" \
    -H "Content-Type: application/json" \
    -H "X-Timestamp: $TS" \
    -H "X-Signature: $SIG" \
    -d "$EXPORT_BODY")
split_response "$raw"
# Handler returns 201 Created (export generated).
assert_status "201" "$RESP_STATUS" "Valid HMAC accepted → 201 Created"
assert_json_present "$RESP_BODY" ".export_id"    "Export response has export_id"
assert_json_present "$RESP_BODY" ".filename"     "Export response has filename"
assert_json_present "$RESP_BODY" ".download_url" "Export response has download_url"
assert_json_field   "$RESP_BODY" ".format" "csv" "Export format echoed as csv"
assert_json_present "$RESP_BODY" ".size_bytes"   "Export response has size_bytes"

# ── Test 2: Missing X-Timestamp → 400 ────────────────────────
raw=$(curl -s -w "\n%{http_code}" \
    -X POST "$BASE_URL$ENDPOINT" \
    -H "Authorization: Bearer $COACH_TOKEN" \
    -H "Content-Type: application/json" \
    -H "X-Signature: $SIG" \
    -d "$EXPORT_BODY")
split_response "$raw"
assert_status "400" "$RESP_STATUS" "Missing X-Timestamp returns 400"
assert_json_present "$RESP_BODY" ".error"   "400 body has error field"
assert_json_present "$RESP_BODY" ".message" "400 body has message field"

# ── Test 3: Missing X-Signature → 400 ────────────────────────
TS2=$(date -u +%s)
raw=$(curl -s -w "\n%{http_code}" \
    -X POST "$BASE_URL$ENDPOINT" \
    -H "Authorization: Bearer $COACH_TOKEN" \
    -H "Content-Type: application/json" \
    -H "X-Timestamp: $TS2" \
    -d "$EXPORT_BODY")
split_response "$raw"
assert_status "400" "$RESP_STATUS" "Missing X-Signature returns 400"
assert_json_present "$RESP_BODY" ".error"   "Missing-sig 400 has error field"
assert_json_present "$RESP_BODY" ".message" "Missing-sig 400 has message field"

# ── Test 4: Wrong signature → 403 ────────────────────────────
TS3=$(date -u +%s)
raw=$(curl -s -w "\n%{http_code}" \
    -X POST "$BASE_URL$ENDPOINT" \
    -H "Authorization: Bearer $COACH_TOKEN" \
    -H "Content-Type: application/json" \
    -H "X-Timestamp: $TS3" \
    -H "X-Signature: deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef" \
    -d "$EXPORT_BODY")
split_response "$raw"
assert_status "403" "$RESP_STATUS" "Wrong X-Signature returns 403"
assert_json_present "$RESP_BODY" ".error" "Wrong-sig 403 has error field"

# ── Test 5: Signature for wrong method → 403 ─────────────────
TS4=$(date -u +%s)
# Sign as if it were a GET, but send as POST
WRONG_METHOD_SIG=$(printf '%s' "${TS4}:GET:/analytics/export" \
    | openssl dgst -sha256 -hmac "$HMAC_SECRET" | awk '{print $NF}')
raw=$(curl -s -w "\n%{http_code}" \
    -X POST "$BASE_URL$ENDPOINT" \
    -H "Authorization: Bearer $COACH_TOKEN" \
    -H "Content-Type: application/json" \
    -H "X-Timestamp: $TS4" \
    -H "X-Signature: $WRONG_METHOD_SIG" \
    -d "$EXPORT_BODY")
split_response "$raw"
assert_status "403" "$RESP_STATUS" "Signature for wrong method is rejected (403)"

# ── Test 6: Stale timestamp (> 5 min old) → 400 ──────────────
STALE_TS=$(( $(date -u +%s) - 400 ))  # 400 seconds ago — beyond 300s tolerance
STALE_SIG=$(make_sig "$STALE_TS")
raw=$(curl -s -w "\n%{http_code}" \
    -X POST "$BASE_URL$ENDPOINT" \
    -H "Authorization: Bearer $COACH_TOKEN" \
    -H "Content-Type: application/json" \
    -H "X-Timestamp: $STALE_TS" \
    -H "X-Signature: $STALE_SIG" \
    -d "$EXPORT_BODY")
split_response "$raw"
assert_status "400" "$RESP_STATUS" "Stale timestamp (>300s) returns 400"
# Message must mention the skew
echo "$RESP_BODY" | grep -qi "timestamp\|skew\|window\|seconds" \
    && pass "Stale-timestamp error mentions time skew" \
    || fail "Stale-timestamp error should mention time skew: $RESP_BODY"

# ── Test 7: Future timestamp far ahead → 400 ─────────────────
FUTURE_TS=$(( $(date -u +%s) + 400 ))
FUTURE_SIG=$(make_sig "$FUTURE_TS")
raw=$(curl -s -w "\n%{http_code}" \
    -X POST "$BASE_URL$ENDPOINT" \
    -H "Authorization: Bearer $COACH_TOKEN" \
    -H "Content-Type: application/json" \
    -H "X-Timestamp: $FUTURE_TS" \
    -H "X-Signature: $FUTURE_SIG" \
    -d "$EXPORT_BODY")
split_response "$raw"
assert_status "400" "$RESP_STATUS" "Far-future timestamp (>300s ahead) returns 400"

# ── Test 8: Unauthenticated + valid HMAC → 401 (auth checked first) ──────────
TS5=$(date -u +%s)
SIG5=$(make_sig "$TS5")
raw=$(curl -s -w "\n%{http_code}" \
    -X POST "$BASE_URL$ENDPOINT" \
    -H "Content-Type: application/json" \
    -H "X-Timestamp: $TS5" \
    -H "X-Signature: $SIG5" \
    -d "$EXPORT_BODY")
split_response "$raw"
assert_status "401" "$RESP_STATUS" "Unauthenticated request rejected before HMAC check (401)"
assert_json_present "$RESP_BODY" ".error"   "Unauth 401 has error field"
assert_json_present "$RESP_BODY" ".message" "Unauth 401 has message field"

# ── Test 9: Member role + valid HMAC → 403 (role checked before HMAC) ────────
MEMBER_TOKEN=$(login "$MEMBER_USER" "$MEMBER_PASS")
TS6=$(date -u +%s)
SIG6=$(make_sig "$TS6")
raw=$(curl -s -w "\n%{http_code}" \
    -X POST "$BASE_URL$ENDPOINT" \
    -H "Authorization: Bearer $MEMBER_TOKEN" \
    -H "Content-Type: application/json" \
    -H "X-Timestamp: $TS6" \
    -H "X-Signature: $SIG6" \
    -d "$EXPORT_BODY")
split_response "$raw"
assert_status "403" "$RESP_STATUS" "Member role rejected regardless of valid HMAC (403)"
assert_json_present "$RESP_BODY" ".error" "Member 403 has error field"

summary
