#!/usr/bin/env bash
# =============================================================================
# Unit test: /health endpoint
# Verifies: response shape, DB status, pool stats, HTTP 200
# =============================================================================
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../tests_common.sh"

echo "▶  [unit] Health endpoint"

# ── Test 1: HTTP 200 ──────────────────────────────────────────
raw=$(http_get "/health")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /health returns 200"

# ── Test 2: status field = "ok" ───────────────────────────────
assert_json_field "$RESP_BODY" ".status" "ok" "status is 'ok'"

# ── Test 3: timestamp present ─────────────────────────────────
assert_json_present "$RESP_BODY" ".timestamp" "timestamp is present"

# ── Test 4: database check present ───────────────────────────
assert_json_present "$RESP_BODY" ".checks.database.status" "database check present"
assert_json_field "$RESP_BODY" ".checks.database.status" "ok" "database status is 'ok'"

# ── Test 5: DB ping_ms is non-negative number ─────────────────
ping_ms=$(printf '%s' "$RESP_BODY" | jq -r '.checks.database.ping_ms')
if [[ "$ping_ms" =~ ^[0-9]+$ ]]; then
    pass "ping_ms is a non-negative integer ($ping_ms ms)"
else
    fail "ping_ms is not a valid integer: '$ping_ms'"
fi

# ── Test 6: pool stats present ────────────────────────────────
assert_json_present "$RESP_BODY" ".pool.connections" "pool.connections present"
assert_json_present "$RESP_BODY" ".pool.idle" "pool.idle present"

# ── Test 7: Content-Type is JSON ─────────────────────────────
ct=$(curl -s -o /dev/null -w "%{content_type}" "$BASE_URL/health")
if [[ "$ct" == *"application/json"* ]]; then
    pass "Content-Type contains application/json"
else
    fail "Content-Type not JSON: '$ct'"
fi

# ── Test 8: Security headers present ─────────────────────────
headers=$(curl -s -I "$BASE_URL/health")
if echo "$headers" | grep -qi "x-content-type-options"; then
    pass "X-Content-Type-Options header present"
else
    fail "X-Content-Type-Options header missing"
fi
if echo "$headers" | grep -qi "x-frame-options"; then
    pass "X-Frame-Options header present"
else
    fail "X-Frame-Options header missing"
fi

summary
