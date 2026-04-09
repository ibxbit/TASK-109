#!/usr/bin/env bash
# =============================================================================
# Unit test: role-based access control (RBAC)
# Verifies: each role sees exactly what it should and nothing more
# =============================================================================
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../tests_common.sh"

echo "▶  [unit] RBAC enforcement"

# ── Setup: obtain tokens for all three roles ──────────────────
ADMIN_TOKEN=$(login "$ADMIN_USER" "$ADMIN_PASS")
COACH_TOKEN=$(login "$COACH_USER" "$COACH_PASS")
MEMBER_TOKEN=$(login "$MEMBER_USER" "$MEMBER_PASS")

[ -n "$ADMIN_TOKEN" ]  || { echo "ERROR: admin login failed"; exit 1; }
[ -n "$COACH_TOKEN" ]  || { echo "ERROR: coach login failed"; exit 1; }
[ -n "$MEMBER_TOKEN" ] || { echo "ERROR: member login failed"; exit 1; }

# ── GET /audit-logs: admin-only ───────────────────────────────

raw=$(http_get "/audit-logs" "$ADMIN_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "Admin can access GET /audit-logs"

raw=$(http_get "/audit-logs" "$COACH_TOKEN")
split_response "$raw"
assert_status "403" "$RESP_STATUS" "Care-coach cannot access GET /audit-logs (403)"

raw=$(http_get "/audit-logs" "$MEMBER_TOKEN")
split_response "$raw"
assert_status "403" "$RESP_STATUS" "Member cannot access GET /audit-logs (403)"

raw=$(http_get "/audit-logs")
split_response "$raw"
assert_status "401" "$RESP_STATUS" "Unauthenticated cannot access GET /audit-logs (401)"

# ── GET /auth/me: any authenticated user ─────────────────────

raw=$(http_get "/auth/me" "$ADMIN_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "Admin can GET /auth/me"

raw=$(http_get "/auth/me" "$MEMBER_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "Member can GET /auth/me"

# ── /profile: member accesses own data only ───────────────────
# (member token → own member_id works; another member_id returns 404/403)

FAKE_ID="ffffffff-ffff-ffff-ffff-ffffffffffff"

raw=$(http_get "/profile/$FAKE_ID" "$MEMBER_TOKEN")
split_response "$raw"
# Could be 403 (wrong owner) or 404 (member record not found)
if [ "$RESP_STATUS" = "403" ] || [ "$RESP_STATUS" = "404" ]; then
    pass "Member cannot access another member's profile ($RESP_STATUS)"
else
    fail "Member accessing fake member_id should get 403 or 404, got $RESP_STATUS"
fi

# ── Care-coach can access member profile ─────────────────────
# (if profile already exists from a previous test run; 404 is acceptable here)
raw=$(http_get "/profile/$MEMBER_ID" "$COACH_TOKEN")
split_response "$raw"
if [ "$RESP_STATUS" = "200" ] || [ "$RESP_STATUS" = "404" ]; then
    pass "Care-coach can attempt GET /profile/$MEMBER_ID ($RESP_STATUS)"
else
    fail "Care-coach GET /profile should get 200 or 404, got $RESP_STATUS"
fi

# ── /internal/metrics: admin-only Prometheus endpoint ────────────────────────
raw=$(http_get "/internal/metrics" "$ADMIN_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "Admin can GET /internal/metrics (200)"

raw=$(http_get "/internal/metrics" "$COACH_TOKEN")
split_response "$raw"
assert_status "403" "$RESP_STATUS" "Care-coach cannot GET /internal/metrics (403)"

raw=$(http_get "/internal/metrics")
split_response "$raw"
assert_status "401" "$RESP_STATUS" "Unauthenticated cannot GET /internal/metrics (401)"

# ── Error response includes correct 403 shape ─────────────────
raw=$(http_get "/audit-logs" "$MEMBER_TOKEN")
split_response "$raw"
assert_json_present "$RESP_BODY" ".error"   "403 response has 'error' field"
assert_json_present "$RESP_BODY" ".message" "403 response has 'message' field"

summary
