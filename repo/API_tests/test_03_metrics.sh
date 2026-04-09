#!/usr/bin/env bash
# =============================================================================
# API test: metric entries workflow
# Verifies: create entries, list with date range, summary aggregation
# =============================================================================
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../tests_common.sh"

echo "▶  [API] Metric entries"

COACH_TOKEN=$(login "$COACH_USER" "$COACH_PASS")
MEMBER_TOKEN=$(login "$MEMBER_USER" "$MEMBER_PASS")
[ -n "$COACH_TOKEN" ] || { echo "ERROR: coach login failed"; exit 1; }

TODAY=$(date -u +%Y-%m-%d)

# ── Step 1: Create weight metric entry ────────────────────────
raw=$(http_post "/metrics" \
    "{\"member_id\":\"$MEMBER_ID\",\"metric_type\":\"weight\",\"value\":170.5,\"entry_date\":\"$TODAY\"}" \
    "$COACH_TOKEN")
split_response "$raw"
assert_status "201" "$RESP_STATUS" "POST /metrics (weight) returns 201"
assert_json_field "$RESP_BODY" ".metric_type" "weight"      "metric_type is 'weight'"
assert_json_field "$RESP_BODY" ".unit"        "lbs"         "unit is 'lbs'"
assert_json_present "$RESP_BODY" ".id"                    "metric entry id present"
assert_json_present "$RESP_BODY" ".entry_date"            "entry_date present"
assert_json_present "$RESP_BODY" ".recorded_by"           "recorded_by present"

# ── Step 2: Create body_fat_percentage metric ─────────────────
raw=$(http_post "/metrics" \
    "{\"member_id\":\"$MEMBER_ID\",\"metric_type\":\"body_fat_percentage\",\"value\":22.5,\"entry_date\":\"$TODAY\"}" \
    "$COACH_TOKEN")
split_response "$raw"
assert_status "201" "$RESP_STATUS" "POST /metrics (body_fat_percentage) returns 201"

# ── Step 3: List metrics (range=7d) ──────────────────────────
raw=$(http_get "/metrics?member_id=$MEMBER_ID&range=7d" "$COACH_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /metrics?range=7d returns 200"
assert_json_present "$RESP_BODY" ".member_id"   "List response has member_id"
assert_json_present "$RESP_BODY" ".total"       "List response has total"
assert_json_present "$RESP_BODY" ".entries"     "List response has entries array"
assert_json_ge "$RESP_BODY" ".total" "1"        "At least 1 metric entry returned"

# ── Step 4: Filter by metric_type ────────────────────────────
raw=$(http_get "/metrics?member_id=$MEMBER_ID&range=7d&metric_type=weight" "$COACH_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /metrics?metric_type=weight returns 200"
entry_type=$(printf '%s' "$RESP_BODY" | jq -r '.entries[0].metric_type' 2>/dev/null)
if [ "$entry_type" = "weight" ] || [ "$entry_type" = "null" ] || [ -z "$entry_type" ]; then
    pass "Filtered list returns only 'weight' entries (or empty)"
else
    fail "Metric type filter returned wrong type: '$entry_type'"
fi

# ── Step 5: Summary endpoint ──────────────────────────────────
raw=$(http_get "/metrics/summary?member_id=$MEMBER_ID&range=30d" "$COACH_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /metrics/summary returns 200"
assert_json_present "$RESP_BODY" ".member_id"  "Summary has member_id"
assert_json_present "$RESP_BODY" ".summaries"  "Summary has summaries array"
assert_json_ge "$RESP_BODY" "(.summaries | length)" "1" "Summary has at least 1 item"

# ── Step 6: Member can read own metrics ──────────────────────
raw=$(http_get "/metrics?member_id=$MEMBER_ID&range=7d" "$MEMBER_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "Member can GET own metrics"

# ── Step 7: range=all retrieves all history ───────────────────
raw=$(http_get "/metrics?member_id=$MEMBER_ID&range=all" "$COACH_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /metrics?range=all returns 200"

# ── Step 8: invalid range shorthand → 400 ────────────────────
raw=$(http_get "/metrics?member_id=$MEMBER_ID&range=banana" "$COACH_TOKEN")
split_response "$raw"
assert_status "400" "$RESP_STATUS" "Invalid range shorthand returns 400"

# ── Step 9: unauthenticated access → 401 ─────────────────────
raw=$(http_get "/metrics?member_id=$MEMBER_ID&range=7d")
split_response "$raw"
assert_status "401" "$RESP_STATUS" "Unauthenticated GET /metrics returns 401"

summary
