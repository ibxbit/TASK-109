#!/usr/bin/env bash
# =============================================================================
# Unit test: successful authentication
# Verifies: login response shape, token issued, GET /auth/me, logout
# =============================================================================
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../tests_common.sh"

echo "▶  [unit] Auth — successful login"

# ── Test 1: admin login returns 200 ──────────────────────────
raw=$(http_post "/auth/login" '{"username":"admin","password":"Admin1234!"}')
split_response "$raw"
assert_status "200" "$RESP_STATUS" "POST /auth/login (admin) returns 200"

# ── Test 2: response contains token ──────────────────────────
assert_json_present "$RESP_BODY" ".token" "response includes token"

# ── Test 3: response contains expires_at ─────────────────────
assert_json_present "$RESP_BODY" ".expires_at" "response includes expires_at"

# ── Test 4: user object returned ─────────────────────────────
assert_json_field "$RESP_BODY" ".user.username" "admin" "user.username = 'admin'"
assert_json_present "$RESP_BODY" ".user.id" "user.id present"

# Extract token for subsequent tests
ADMIN_TOKEN=$(printf '%s' "$RESP_BODY" | jq -r '.token')

# ── Test 5: GET /auth/me with valid token returns 200 ────────
raw=$(http_get "/auth/me" "$ADMIN_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /auth/me with valid token returns 200"
assert_json_field "$RESP_BODY" ".user.username" "admin" "/auth/me returns correct username"

# ── Test 6: care-coach login ──────────────────────────────────
raw=$(http_post "/auth/login" '{"username":"coach","password":"Coach1234!"}')
split_response "$raw"
assert_status "200" "$RESP_STATUS" "POST /auth/login (coach) returns 200"
COACH_TOKEN=$(printf '%s' "$RESP_BODY" | jq -r '.token')
assert_json_field "$RESP_BODY" ".user.username" "coach" "coach login returns correct username"

# ── Test 7: member login ──────────────────────────────────────
raw=$(http_post "/auth/login" '{"username":"member","password":"Member1234!"}')
split_response "$raw"
assert_status "200" "$RESP_STATUS" "POST /auth/login (member) returns 200"
MEMBER_TOKEN=$(printf '%s' "$RESP_BODY" | jq -r '.token')

# ── Test 8: POST /auth/logout returns 200 ────────────────────
raw=$(http_post "/auth/logout" '{}' "$ADMIN_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "POST /auth/logout returns 200"
assert_json_field "$RESP_BODY" ".message" "Logged out" "logout message correct"

# ── Test 9: token invalidated after logout ────────────────────
raw=$(http_get "/auth/me" "$ADMIN_TOKEN")
split_response "$raw"
assert_status "401" "$RESP_STATUS" "Invalidated token is rejected with 401"

summary
