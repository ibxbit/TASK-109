#!/usr/bin/env bash
# =============================================================================
# API test: full authentication lifecycle
# Verifies: login → access protected resource → logout → access denied
#           All three roles go through the full flow.
# =============================================================================
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../tests_common.sh"

echo "▶  [API] Auth lifecycle (login → me → logout → reject)"

# ── Helper: run lifecycle for one user ───────────────────────
lifecycle_test() {
    local username="$1" password="$2" role_label="$3"

    # Step 1: login
    raw=$(http_post "/auth/login" "{\"username\":\"$username\",\"password\":\"$password\"}")
    split_response "$raw"
    assert_status "200" "$RESP_STATUS" "$role_label: login returns 200"
    local token
    token=$(printf '%s' "$RESP_BODY" | jq -r '.token')
    assert_json_present "$RESP_BODY" ".token"      "$role_label: token present"
    assert_json_present "$RESP_BODY" ".expires_at" "$role_label: expires_at present"
    assert_json_field "$RESP_BODY" ".user.username" "$username" "$role_label: username in response"

    # Step 2: GET /auth/me with token
    raw=$(http_get "/auth/me" "$token")
    split_response "$raw"
    assert_status "200" "$RESP_STATUS" "$role_label: /auth/me returns 200"
    assert_json_field "$RESP_BODY" ".user.username" "$username" "$role_label: /auth/me returns correct user"

    # Step 3: logout
    raw=$(http_post "/auth/logout" '{}' "$token")
    split_response "$raw"
    assert_status "200" "$RESP_STATUS" "$role_label: logout returns 200"
    assert_json_field "$RESP_BODY" ".message" "Logged out" "$role_label: logout message correct"

    # Step 4: token rejected after logout
    raw=$(http_get "/auth/me" "$token")
    split_response "$raw"
    assert_status "401" "$RESP_STATUS" "$role_label: invalidated token rejected"
}

lifecycle_test "$ADMIN_USER"  "$ADMIN_PASS"  "Admin"
lifecycle_test "$COACH_USER"  "$COACH_PASS"  "CareCoach"
lifecycle_test "$MEMBER_USER" "$MEMBER_PASS" "Member"

# ── Double-login: two independent sessions ────────────────────
echo "  Testing concurrent sessions…"
raw1=$(http_post "/auth/login" "{\"username\":\"$ADMIN_USER\",\"password\":\"$ADMIN_PASS\"}")
raw2=$(http_post "/auth/login" "{\"username\":\"$ADMIN_USER\",\"password\":\"$ADMIN_PASS\"}")
split_response "$raw1"; tok1=$(printf '%s' "$RESP_BODY" | jq -r '.token')
split_response "$raw2"; tok2=$(printf '%s' "$RESP_BODY" | jq -r '.token')

if [ "$tok1" != "$tok2" ]; then
    pass "Concurrent logins produce independent tokens"
else
    fail "Concurrent logins should produce different tokens"
fi

# Both tokens work
raw=$(http_get "/auth/me" "$tok1"); split_response "$raw"
assert_status "200" "$RESP_STATUS" "Session 1 token is valid"
raw=$(http_get "/auth/me" "$tok2"); split_response "$raw"
assert_status "200" "$RESP_STATUS" "Session 2 token is valid"

# Logout session 1; session 2 still works
http_post "/auth/logout" '{}' "$tok1" > /dev/null
raw=$(http_get "/auth/me" "$tok2"); split_response "$raw"
assert_status "200" "$RESP_STATUS" "Session 2 unaffected by session 1 logout"
http_post "/auth/logout" '{}' "$tok2" > /dev/null

summary
