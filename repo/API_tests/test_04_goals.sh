#!/usr/bin/env bash
# =============================================================================
# API test: goals workflow
# Verifies: create, list, update status; direction validation; RBAC
# =============================================================================
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../tests_common.sh"

echo "▶  [API] Goals workflow"

COACH_TOKEN=$(login "$COACH_USER" "$COACH_PASS")
MEMBER_TOKEN=$(login "$MEMBER_USER" "$MEMBER_PASS")
[ -n "$COACH_TOKEN" ] || { echo "ERROR: coach login failed"; exit 1; }

TODAY=$(date -u +%Y-%m-%d)
TARGET_DATE=$(date -u -d "+90 days" +%Y-%m-%d 2>/dev/null \
    || date -u -v +90d +%Y-%m-%d 2>/dev/null \
    || echo "2025-12-31")

# ── Step 1: Create a fat_loss goal ────────────────────────────
GOAL_BODY=$(cat <<JSON
{
  "member_id":      "$MEMBER_ID",
  "goal_type":      "fat_loss",
  "title":          "Reduce body fat to 18%",
  "description":    "Steady fat loss over 90 days",
  "start_date":     "$TODAY",
  "target_date":    "$TARGET_DATE",
  "baseline_value": 22.5,
  "target_value":   18.0
}
JSON
)

raw=$(http_post "/goals" "$GOAL_BODY" "$COACH_TOKEN")
split_response "$raw"
assert_status "201" "$RESP_STATUS" "POST /goals (fat_loss) returns 201"
assert_json_field "$RESP_BODY" ".goal_type"       "fat_loss" "goal_type is 'fat_loss'"
assert_json_field "$RESP_BODY" ".status"          "active"   "initial status is 'active'"
assert_json_present "$RESP_BODY" ".id"                       "goal id present"
assert_json_present "$RESP_BODY" ".tracked_metric"           "tracked_metric present"
assert_json_field "$RESP_BODY" ".tracked_metric" "body_fat_percentage" \
    "fat_loss goal tracks body_fat_percentage"

GOAL_ID=$(printf '%s' "$RESP_BODY" | jq -r '.id')

# ── Step 2: Create a muscle_gain goal ────────────────────────
GAIN_BODY=$(cat <<JSON
{
  "member_id":      "$MEMBER_ID",
  "goal_type":      "muscle_gain",
  "title":          "Reach 180 lbs",
  "start_date":     "$TODAY",
  "baseline_value": 170.5,
  "target_value":   180.0
}
JSON
)
raw=$(http_post "/goals" "$GAIN_BODY" "$COACH_TOKEN")
split_response "$raw"
assert_status "201" "$RESP_STATUS" "POST /goals (muscle_gain) returns 201"
assert_json_field "$RESP_BODY" ".tracked_metric" "weight" "muscle_gain tracks weight"

# ── Step 3: List goals ────────────────────────────────────────
raw=$(http_get "/goals?member_id=$MEMBER_ID" "$COACH_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /goals returns 200"
assert_json_present "$RESP_BODY" ".member_id" "List has member_id"
assert_json_ge "$RESP_BODY" ".total" "1"      "At least 1 goal returned"
assert_json_present "$RESP_BODY" ".goals"     "List has goals array"

# ── Step 4: Filter by status ──────────────────────────────────
raw=$(http_get "/goals?member_id=$MEMBER_ID&status=active" "$COACH_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /goals?status=active returns 200"

# ── Step 5: Update goal — pause it ───────────────────────────
if [ -n "$GOAL_ID" ] && [ "$GOAL_ID" != "null" ]; then
    raw=$(http_put "/goals/$GOAL_ID" '{"status":"paused"}' "$COACH_TOKEN")
    split_response "$raw"
    assert_status "200" "$RESP_STATUS" "PUT /goals/$GOAL_ID (pause) returns 200"
    assert_json_field "$RESP_BODY" ".status" "paused" "status updated to 'paused'"

    # Update back to active
    raw=$(http_put "/goals/$GOAL_ID" '{"status":"active"}' "$COACH_TOKEN")
    split_response "$raw"
    assert_status "200" "$RESP_STATUS" "PUT /goals/$GOAL_ID (re-activate) returns 200"
    assert_json_field "$RESP_BODY" ".status" "active" "status restored to 'active'"
else
    fail "Goal ID not captured — skipping update tests"
fi

# ── Step 6: Direction validation (target wrong way) ──────────
BAD_GOAL=$(cat <<JSON
{
  "member_id":      "$MEMBER_ID",
  "goal_type":      "fat_loss",
  "title":          "Wrong direction",
  "start_date":     "$TODAY",
  "baseline_value": 20.0,
  "target_value":   25.0
}
JSON
)
raw=$(http_post "/goals" "$BAD_GOAL" "$COACH_TOKEN")
split_response "$raw"
assert_status "400" "$RESP_STATUS" "fat_loss target > baseline rejected (400)"

# ── Step 7: Invalid goal_type → 400 ──────────────────────────
BAD_TYPE=$(cat <<JSON
{
  "member_id":      "$MEMBER_ID",
  "goal_type":      "world_domination",
  "title":          "Bad type",
  "start_date":     "$TODAY",
  "baseline_value": 10.0,
  "target_value":   5.0
}
JSON
)
raw=$(http_post "/goals" "$BAD_TYPE" "$COACH_TOKEN")
split_response "$raw"
assert_status "400" "$RESP_STATUS" "Invalid goal_type returns 400"

# ── Step 8: Member can read own goals ─────────────────────────
raw=$(http_get "/goals?member_id=$MEMBER_ID" "$MEMBER_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "Member can GET own goals"

# ── Step 9: Unauthenticated → 401 ────────────────────────────
raw=$(http_get "/goals?member_id=$MEMBER_ID")
split_response "$raw"
assert_status "401" "$RESP_STATUS" "Unauthenticated GET /goals returns 401"

summary
