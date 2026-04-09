#!/usr/bin/env bash
# =============================================================================
# Unit test: authentication failure scenarios
# Verifies: wrong credentials, missing fields, unauthenticated access,
#           standard error response format (JSON with "error" + "message")
# =============================================================================
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../tests_common.sh"

echo "▶  [unit] Auth — failure scenarios"

# ── Error response shape helper ───────────────────────────────
# Every 4xx/5xx must return: { "error": "<reason>", "message": "<detail>" }
assert_error_shape() {
    local body="$1" label="$2"
    assert_json_present "$body" ".error"   "$label — has 'error' field"
    assert_json_present "$body" ".message" "$label — has 'message' field"
}

# ── Test 1: wrong password → 401 ─────────────────────────────
raw=$(http_post "/auth/login" '{"username":"admin","password":"WrongPass!"}')
split_response "$raw"
assert_status "401" "$RESP_STATUS" "Wrong password returns 401"
assert_error_shape "$RESP_BODY" "Wrong password"

# ── Test 2: unknown user → 401 ───────────────────────────────
raw=$(http_post "/auth/login" '{"username":"nobody","password":"anything"}')
split_response "$raw"
assert_status "401" "$RESP_STATUS" "Unknown user returns 401"
assert_error_shape "$RESP_BODY" "Unknown user"

# ── Test 3: empty username → 400 (validation) ────────────────
raw=$(http_post "/auth/login" '{"username":"","password":"Admin1234!"}')
split_response "$raw"
assert_status "400" "$RESP_STATUS" "Empty username returns 400"
assert_error_shape "$RESP_BODY" "Empty username"

# ── Test 4: empty password → 400 (validation) ────────────────
raw=$(http_post "/auth/login" '{"username":"admin","password":""}')
split_response "$raw"
assert_status "400" "$RESP_STATUS" "Empty password returns 400"
assert_error_shape "$RESP_BODY" "Empty password"

# ── Test 5: missing Authorization header → 401 ───────────────
raw=$(curl -s -w "\n%{http_code}" "$BASE_URL/auth/me")
split_response "$raw"
assert_status "401" "$RESP_STATUS" "Missing Bearer token returns 401"
assert_error_shape "$RESP_BODY" "No auth header"

# ── Test 6: malformed Bearer token → 401 ─────────────────────
raw=$(curl -s -w "\n%{http_code}" \
    -H "Authorization: Bearer not.a.valid.jwt" \
    "$BASE_URL/auth/me")
split_response "$raw"
assert_status "401" "$RESP_STATUS" "Malformed JWT returns 401"

# ── Test 7: POST /auth/logout without auth → 401 ─────────────
raw=$(http_post "/auth/logout" '{}')
split_response "$raw"
assert_status "401" "$RESP_STATUS" "Logout without token returns 401"

# ── Test 8: username too long → 400 ──────────────────────────
LONG_USERNAME=$(python3 -c "print('a'*51)" 2>/dev/null || printf '%0.s a' {1..51} | tr -d ' ')
raw=$(http_post "/auth/login" "{\"username\":\"$LONG_USERNAME\",\"password\":\"pw\"}")
split_response "$raw"
assert_status "400" "$RESP_STATUS" "Username > 50 chars returns 400"

summary
