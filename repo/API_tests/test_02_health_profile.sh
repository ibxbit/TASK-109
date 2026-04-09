#!/usr/bin/env bash
# =============================================================================
# API test: health profile CRUD workflow
# Verifies: create, read, update; conflict on duplicate; access control
# =============================================================================
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../tests_common.sh"

echo "▶  [API] Health profile CRUD"

COACH_TOKEN=$(login "$COACH_USER" "$COACH_PASS")
MEMBER_TOKEN=$(login "$MEMBER_USER" "$MEMBER_PASS")
[ -n "$COACH_TOKEN" ]  || { echo "ERROR: coach login failed";  exit 1; }
[ -n "$MEMBER_TOKEN" ] || { echo "ERROR: member login failed"; exit 1; }

# ── Step 1: Create profile (care-coach creates for member) ────
CREATE_BODY=$(cat <<JSON
{
  "member_id":      "$MEMBER_ID",
  "sex":            "male",
  "height_in":      70.0,
  "weight_lbs":     175.0,
  "activity_level": "moderately_active",
  "dietary_notes":  "No gluten"
}
JSON
)

raw=$(http_post "/profile" "$CREATE_BODY" "$COACH_TOKEN")
split_response "$raw"

# Profile might already exist from a previous test run (409) or be newly created (201)
if [ "$RESP_STATUS" = "201" ]; then
    pass "POST /profile created (201)"
    assert_json_field "$RESP_BODY" ".sex"            "male"               "sex field correct"
    assert_json_field "$RESP_BODY" ".activity_level" "moderately_active"  "activity_level correct"
    assert_json_present "$RESP_BODY" ".id" "profile id present"
elif [ "$RESP_STATUS" = "409" ]; then
    pass "POST /profile conflict — profile already exists (idempotent re-run)"
else
    fail "POST /profile expected 201 or 409, got $RESP_STATUS"
fi

# ── Step 2: GET profile ────────────────────────────────────────
raw=$(http_get "/profile/$MEMBER_ID" "$COACH_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /profile/$MEMBER_ID (coach) returns 200"
assert_json_field "$RESP_BODY" ".member_id" "$MEMBER_ID" "GET profile has correct member_id"
assert_json_present "$RESP_BODY" ".sex"            "GET profile has sex field"
assert_json_present "$RESP_BODY" ".height_in"      "GET profile has height_in"
assert_json_present "$RESP_BODY" ".weight_lbs"     "GET profile has weight_lbs"
assert_json_present "$RESP_BODY" ".activity_level" "GET profile has activity_level"

# ── Step 3: Member can read own profile ───────────────────────
raw=$(http_get "/profile/$MEMBER_ID" "$MEMBER_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /profile/$MEMBER_ID (member, own record) returns 200"

# ── Step 4: Update profile ────────────────────────────────────
UPDATE_BODY='{"weight_lbs": 170.0, "activity_level": "very_active"}'
raw=$(http_put "/profile/$MEMBER_ID" "$UPDATE_BODY" "$COACH_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "PUT /profile/$MEMBER_ID returns 200"
assert_json_field "$RESP_BODY" ".weight_lbs"     "170.0"       "weight_lbs updated"
assert_json_field "$RESP_BODY" ".activity_level" "very_active" "activity_level updated"

# ── Step 5: Verify update persisted ──────────────────────────
raw=$(http_get "/profile/$MEMBER_ID" "$COACH_TOKEN")
split_response "$raw"
assert_json_field "$RESP_BODY" ".weight_lbs"     "170"         "Updated weight persisted"
assert_json_field "$RESP_BODY" ".activity_level" "very_active" "Updated activity_level persisted"

# ── Step 6: Duplicate create → 409 ───────────────────────────
raw=$(http_post "/profile" "$CREATE_BODY" "$COACH_TOKEN")
split_response "$raw"
assert_status "409" "$RESP_STATUS" "Duplicate POST /profile returns 409"

# ── Step 7: Unknown member_id → 404 ──────────────────────────
FAKE="ffffffff-ffff-ffff-ffff-ffffffffffff"
raw=$(http_get "/profile/$FAKE" "$COACH_TOKEN")
split_response "$raw"
assert_status "404" "$RESP_STATUS" "GET /profile with unknown member_id returns 404"

# ── Step 8: Unauthenticated GET → 401 ────────────────────────
raw=$(http_get "/profile/$MEMBER_ID")
split_response "$raw"
assert_status "401" "$RESP_STATUS" "Unauthenticated GET /profile returns 401"

summary
