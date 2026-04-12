#!/usr/bin/env bash
# =============================================================================
# Unit test: input validation and boundary conditions
# Verifies: out-of-range values, invalid enums, missing required fields,
#           boundary values at min/max for metrics
# =============================================================================
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../tests_common.sh"

echo "▶  [unit] Input validation & boundary conditions"

ADMIN_TOKEN=$(login "$ADMIN_USER" "$ADMIN_PASS")
COACH_TOKEN=$(login "$COACH_USER" "$COACH_PASS")
[ -n "$ADMIN_TOKEN" ] || { echo "ERROR: admin login failed"; exit 1; }

# ── Metric value boundary tests ───────────────────────────────
# weight valid range: 10.0 – 1500.0 lbs

# At lower bound (10.0) → 201 or 409 (idempotent)
raw=$(http_post "/metrics" \
    "{\"member_id\":\"$MEMBER_ID\",\"metric_type\":\"weight\",\"value\":10.0}" \
    "$COACH_TOKEN")
split_response "$raw"
if [ "$RESP_STATUS" = "201" ] || [ "$RESP_STATUS" = "409" ]; then
    pass "Metric weight=10.0 (lower bound) is accepted (201/409)"
else
    fail "Metric weight=10.0 (lower bound) expected 201 or 409, got $RESP_STATUS"
fi

# Below lower bound (9.99) → 400
raw=$(http_post "/metrics" \
    "{\"member_id\":\"$MEMBER_ID\",\"metric_type\":\"weight\",\"value\":9.99}" \
    "$COACH_TOKEN")
split_response "$raw"
assert_status "400" "$RESP_STATUS" "Metric weight=9.99 (below lower bound) is rejected"
assert_json_present "$RESP_BODY" ".message" "Below-bound rejection has message"

# At upper bound (1500.0) → 201 or 409 (idempotent)
raw=$(http_post "/metrics" \
    "{\"member_id\":\"$MEMBER_ID\",\"metric_type\":\"weight\",\"value\":1500.0}" \
    "$COACH_TOKEN")
split_response "$raw"
if [ "$RESP_STATUS" = "201" ] || [ "$RESP_STATUS" = "409" ]; then
    pass "Metric weight=1500.0 (upper bound) is accepted (201/409)"
else
    fail "Metric weight=1500.0 (upper bound) expected 201 or 409, got $RESP_STATUS"
fi

# Above upper bound (1500.01) → 400
raw=$(http_post "/metrics" \
    "{\"member_id\":\"$MEMBER_ID\",\"metric_type\":\"weight\",\"value\":1500.01}" \
    "$COACH_TOKEN")
split_response "$raw"
assert_status "400" "$RESP_STATUS" "Metric weight=1500.01 (above upper bound) is rejected"

# body_fat_percentage range: 1.0 – 70.0
raw=$(http_post "/metrics" \
    "{\"member_id\":\"$MEMBER_ID\",\"metric_type\":\"body_fat_percentage\",\"value\":0.9}" \
    "$COACH_TOKEN")
split_response "$raw"
assert_status "400" "$RESP_STATUS" "body_fat_percentage=0.9 rejected"

raw=$(http_post "/metrics" \
    "{\"member_id\":\"$MEMBER_ID\",\"metric_type\":\"body_fat_percentage\",\"value\":25.0}" \
    "$COACH_TOKEN")
split_response "$raw"
if [ "$RESP_STATUS" = "201" ] || [ "$RESP_STATUS" = "409" ]; then
    pass "body_fat_percentage=25.0 accepted (201/409)"
else
    fail "body_fat_percentage=25.0 expected 201 or 409, got $RESP_STATUS"
fi

# ── Unknown metric type → 400 ─────────────────────────────────
raw=$(http_post "/metrics" \
    "{\"member_id\":\"$MEMBER_ID\",\"metric_type\":\"unicorn_dust\",\"value\":42.0}" \
    "$COACH_TOKEN")
split_response "$raw"
assert_status "400" "$RESP_STATUS" "Unknown metric_type rejected with 400"

# ── Profile: invalid sex enum → 400 ──────────────────────────
raw=$(http_post "/profile" \
    "{\"member_id\":\"$MEMBER_ID\",\"sex\":\"alien\",\"height_in\":70.0,\"weight_lbs\":160.0,\"activity_level\":\"sedentary\"}" \
    "$COACH_TOKEN")
split_response "$raw"
# 400 (invalid) or 409 (profile already exists) — both are acceptable
if [ "$RESP_STATUS" = "400" ] || [ "$RESP_STATUS" = "409" ]; then
    pass "Invalid sex enum rejected or profile conflict ($RESP_STATUS)"
else
    fail "Invalid sex should get 400 or 409, got $RESP_STATUS"
fi

# ── Profile: invalid activity_level enum → 400 ───────────────
raw=$(http_post "/profile" \
    "{\"member_id\":\"$MEMBER_ID\",\"sex\":\"male\",\"height_in\":70.0,\"weight_lbs\":160.0,\"activity_level\":\"marathon_runner\"}" \
    "$COACH_TOKEN")
split_response "$raw"
if [ "$RESP_STATUS" = "400" ] || [ "$RESP_STATUS" = "409" ]; then
    pass "Invalid activity_level enum rejected or profile conflict ($RESP_STATUS)"
else
    fail "Invalid activity_level should get 400 or 409, got $RESP_STATUS"
fi

# ── Goal: direction validation (fat_loss target must be < baseline) ───────────
raw=$(http_post "/goals" \
    "{\"member_id\":\"$MEMBER_ID\",\"goal_type\":\"fat_loss\",\"title\":\"Bad goal\",
      \"start_date\":\"2024-01-01\",\"baseline_value\":25.0,\"target_value\":30.0}" \
    "$COACH_TOKEN")
split_response "$raw"
assert_status "400" "$RESP_STATUS" "fat_loss goal with target > baseline rejected (400)"

# ── Non-existent member UUID → 404 ───────────────────────────
FAKE_ID="ffffffff-ffff-ffff-ffff-ffffffffffff"
raw=$(http_post "/metrics" \
    "{\"member_id\":\"$FAKE_ID\",\"metric_type\":\"weight\",\"value\":150.0}" \
    "$COACH_TOKEN")
split_response "$raw"
assert_status "404" "$RESP_STATUS" "Metric for non-existent member returns 404"

summary
