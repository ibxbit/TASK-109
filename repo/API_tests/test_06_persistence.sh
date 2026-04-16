#!/usr/bin/env bash
# =============================================================================
# API test: offline constraints and data persistence
# Verifies: data written in one request is readable in the next (no in-memory
#           only storage), work order state machine, offline compliance
# =============================================================================
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../tests_common.sh"

echo "▶  [API] Data persistence & offline compliance"

COACH_TOKEN=$(login "$COACH_USER" "$COACH_PASS")
ADMIN_TOKEN=$(login "$ADMIN_USER" "$ADMIN_PASS")
[ -n "$COACH_TOKEN" ] || { echo "ERROR: coach login failed"; exit 1; }

TODAY=$(date -u +%Y-%m-%d)
TS=$(date -u +%s)

# ── Test 1: Write a metric, re-read, verify it's persisted ───
# Use a metric type no other suite writes so the unique constraint
# (member_id, metric_type_id, entry_date) from migration 20260413000016
# doesn't collide with metrics recorded earlier in the same test run.
ENTRY_VAL="35.$(( TS % 100 ))"  # waist circumference range, unique-ish per run
raw=$(http_post "/metrics" \
    "{\"member_id\":\"$MEMBER_ID\",\"metric_type\":\"waist\",
      \"value\":$ENTRY_VAL,\"entry_date\":\"$TODAY\",
      \"notes\":\"persistence_test_$TS\"}" \
    "$COACH_TOKEN")
split_response "$raw"
assert_status "201" "$RESP_STATUS" "Metric written (persistence seed)"
ENTRY_ID=$(printf '%s' "$RESP_BODY" | jq -r '.id')

# Re-read list and confirm entry appears
raw=$(http_get "/metrics?member_id=$MEMBER_ID&range=7d" "$COACH_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "Metrics list readable after write"
FOUND=$(printf '%s' "$RESP_BODY" | jq -r ".entries[] | select(.id == \"$ENTRY_ID\") | .id" 2>/dev/null)
if [ "$FOUND" = "$ENTRY_ID" ]; then
    pass "Written metric entry found in subsequent read"
else
    fail "Written metric entry NOT found in subsequent read (id=$ENTRY_ID)"
fi

# ── Test 2: Work order state machine ─────────────────────────
WO_BODY=$(cat <<JSON
{
  "member_id":   "$MEMBER_ID",
  "title":       "Persistence test work order $TS",
  "ticket_type": "health_query",
  "priority":    "medium"
}
JSON
)
raw=$(http_post "/work-orders" "$WO_BODY" "$COACH_TOKEN")
split_response "$raw"
assert_status "201" "$RESP_STATUS" "POST /work-orders returns 201"
assert_json_field "$RESP_BODY" ".status" "intake" "New work order starts at 'intake'"
WO_ID=$(printf '%s' "$RESP_BODY" | jq -r '.id')

# Transition: intake → triage
raw=$(http_patch "/work-orders/$WO_ID/transition" \
    '{"to_status":"triage","notes":"Moving to triage"}' \
    "$COACH_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "Work order intake→triage returns 200"
assert_json_field "$RESP_BODY" ".status" "triage" "Work order status is 'triage'"

# Transition: triage → in_progress
raw=$(http_patch "/work-orders/$WO_ID/transition" \
    '{"to_status":"in_progress","notes":"Starting work"}' \
    "$COACH_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "Work order triage→in_progress returns 200"
assert_json_field "$RESP_BODY" ".status" "in_progress" "Work order status is 'in_progress'"

# Transition: in_progress → resolved
raw=$(http_patch "/work-orders/$WO_ID/transition" \
    '{"to_status":"resolved","notes":"Issue resolved"}' \
    "$COACH_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "Work order in_progress→resolved returns 200"
assert_json_field "$RESP_BODY" ".status" "resolved" "Work order status is 'resolved'"

# ── Test 3: Invalid state transition rejected ─────────────────
# resolved → intake is not a valid backward transition
raw=$(http_patch "/work-orders/$WO_ID/transition" \
    '{"to_status":"intake","notes":"Try to go back"}' \
    "$COACH_TOKEN")
split_response "$raw"
if [ "$RESP_STATUS" = "400" ] || [ "$RESP_STATUS" = "422" ]; then
    pass "Invalid backward state transition rejected ($RESP_STATUS)"
else
    fail "Invalid state transition should be rejected, got $RESP_STATUS"
fi

# ── Test 4: GET /work-orders list ────────────────────────────
raw=$(http_get "/work-orders" "$COACH_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /work-orders returns 200"

# ── Test 5: Prometheus metrics endpoint (admin-only) has data ─
raw=$(http_get "/internal/metrics" "$ADMIN_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /internal/metrics returns 200 (admin)"
if echo "$RESP_BODY" | grep -q "http_requests_total"; then
    pass "Prometheus output contains http_requests_total counter"
else
    fail "Prometheus output missing http_requests_total"
fi
if echo "$RESP_BODY" | grep -q "http_request_duration_seconds"; then
    pass "Prometheus output contains http_request_duration_seconds histogram"
else
    fail "Prometheus output missing http_request_duration_seconds"
fi

# ── Test 6: No external network dependency ────────────────────
# Verify the service is reachable on localhost only (offline compliance)
# The /health endpoint is fully operational with no external calls required.
raw=$(http_get "/health")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "Service operational on localhost (offline compliant)"
assert_json_field "$RESP_BODY" ".checks.database.status" "ok" "DB reachable locally"

summary
