#!/usr/bin/env bash
# =============================================================================
# API test: audit log access control and content
# Verifies: admin-only access, log generation from other actions,
#           pagination, date filtering, JSON shape
# =============================================================================
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../tests_common.sh"

echo "▶  [API] Audit logs"

ADMIN_TOKEN=$(login "$ADMIN_USER" "$ADMIN_PASS")
COACH_TOKEN=$(login "$COACH_USER" "$COACH_PASS")
MEMBER_TOKEN=$(login "$MEMBER_USER" "$MEMBER_PASS")
[ -n "$ADMIN_TOKEN" ] || { echo "ERROR: admin login failed"; exit 1; }

# ── Step 1: Admin can list audit logs ─────────────────────────
raw=$(http_get "/audit-logs" "$ADMIN_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "Admin GET /audit-logs returns 200"

# ── Step 2: Response shape ────────────────────────────────────
assert_json_present "$RESP_BODY" ".data"     "Response has 'data' array"
assert_json_present "$RESP_BODY" ".page"     "Response has 'page'"
assert_json_present "$RESP_BODY" ".per_page" "Response has 'per_page'"
assert_json_present "$RESP_BODY" ".total"    "Response has 'total'"
assert_json_ge "$RESP_BODY" ".total" "1"     "At least 1 audit log entry exists"

# ── Step 3: Individual log entry shape ────────────────────────
FIRST=$(printf '%s' "$RESP_BODY" | jq -r '.data[0]' 2>/dev/null)
if [ -n "$FIRST" ] && [ "$FIRST" != "null" ]; then
    assert_json_present "$FIRST" ".id"          "Log entry has id"
    assert_json_present "$FIRST" ".action"      "Log entry has action"
    assert_json_present "$FIRST" ".entity_type" "Log entry has entity_type"
    assert_json_present "$FIRST" ".created_at"  "Log entry has created_at"
else
    fail "Could not extract first log entry from response"
fi

# ── Step 4: RBAC — care-coach cannot access ───────────────────
raw=$(http_get "/audit-logs" "$COACH_TOKEN")
split_response "$raw"
assert_status "403" "$RESP_STATUS" "Care-coach GET /audit-logs returns 403"
assert_json_present "$RESP_BODY" ".error"   "403 response has error field"
assert_json_present "$RESP_BODY" ".message" "403 response has message field"

# ── Step 5: RBAC — member cannot access ──────────────────────
raw=$(http_get "/audit-logs" "$MEMBER_TOKEN")
split_response "$raw"
assert_status "403" "$RESP_STATUS" "Member GET /audit-logs returns 403"

# ── Step 6: Unauthenticated → 401 ────────────────────────────
raw=$(http_get "/audit-logs")
split_response "$raw"
assert_status "401" "$RESP_STATUS" "Unauthenticated GET /audit-logs returns 401"

# ── Step 7: Trigger a new audit log by logging in ────────────
# Login/logout generates LOGIN and LOGOUT entries
http_post "/auth/login" \
    "{\"username\":\"$MEMBER_USER\",\"password\":\"$MEMBER_PASS\"}" > /dev/null

raw=$(http_get "/audit-logs?action=LOGIN" "$ADMIN_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /audit-logs?action=LOGIN returns 200"

# ── Step 8: Pagination ────────────────────────────────────────
raw=$(http_get "/audit-logs?per_page=2&page=1" "$ADMIN_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "Paginated GET /audit-logs returns 200"
assert_json_field "$RESP_BODY" ".per_page" "2" "per_page honoured"
assert_json_field "$RESP_BODY" ".page"     "1" "page is 1"

# ── Step 9: Date range filter ─────────────────────────────────
TODAY=$(date -u +%Y-%m-%d)
raw=$(http_get "/audit-logs?start_date=$TODAY&end_date=$TODAY" "$ADMIN_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "Date-filtered GET /audit-logs returns 200"

# ── Step 10: Invalid date format → 400 ───────────────────────
raw=$(http_get "/audit-logs?start_date=not-a-date" "$ADMIN_TOKEN")
split_response "$raw"
assert_status "400" "$RESP_STATUS" "Invalid start_date returns 400"

# ── Step 11: Entity type filter ───────────────────────────────
raw=$(http_get "/audit-logs?entity_type=session" "$ADMIN_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "Entity type filter returns 200"

summary
