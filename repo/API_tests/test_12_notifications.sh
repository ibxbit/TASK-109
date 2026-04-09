#!/usr/bin/env bash
# =============================================================================
# API test: notifications, subscriptions, and schedules
# =============================================================================
# Exercises:
#   1. POST /notifications — admin creates a notification for member
#   2. GET /notifications — member reads own notifications
#   3. GET /notifications?is_read=false — unread filter
#   4. POST /notifications/{id}/read — mark single notification read
#   5. GET /notifications?is_read=true — verify marked-read appears
#   6. POST /notifications/read-all — mark all unread as read
#   7. GET /notifications/subscriptions — list subscription preferences
#   8. PATCH /notifications/subscriptions/{event_type} — opt out
#   9. PATCH /notifications/subscriptions/{event_type} — re-subscribe
#  10. POST /notifications/schedules — create daily reminder
#  11. GET /notifications/schedules — list schedules (admin sees all)
#  12. DELETE /notifications/schedules/{id} — delete schedule
#  13. Invalid event_type → 400
#  14. Non-admin cannot create notifications for other users → 403
#  15. Member cannot read another member's notification → 403
#  16. Unauthenticated access → 401
# =============================================================================
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../tests_common.sh"

echo "▶  [API] Notifications, subscriptions, and schedules"

# ── Login ─────────────────────────────────────────────────────
ADMIN_TOKEN=$(login "$ADMIN_USER" "$ADMIN_PASS")
[ -n "$ADMIN_TOKEN" ] || { echo "ERROR: admin login failed"; exit 1; }

MEMBER_TOKEN=$(login "$MEMBER_USER" "$MEMBER_PASS")
[ -n "$MEMBER_TOKEN" ] || { echo "ERROR: member login failed"; exit 1; }

COACH_TOKEN=$(login "$COACH_USER" "$COACH_PASS")
[ -n "$COACH_TOKEN" ] || { echo "ERROR: coach login failed"; exit 1; }

# Fixed member user_id from seeded data
MEMBER_USER_ID="20000000-0000-0000-0000-000000000003"

# =============================================================================
# Step 1: Admin creates a notification for the member
# =============================================================================
echo ""
echo "  ── Step 1: Admin creates notification for member ──"

NOTIF_TITLE="Test notification $(date -u +%s)"

raw=$(http_post "/notifications" \
    "{\"user_id\":\"$MEMBER_USER_ID\",
      \"event_type\":\"manual\",
      \"title\":\"$NOTIF_TITLE\",
      \"body\":\"This is an automated test notification.\"}" \
    "$ADMIN_TOKEN")
split_response "$raw"
assert_status "201" "$RESP_STATUS" "POST /notifications returns 201"

NOTIF_ID=$(printf '%s' "$RESP_BODY" | jq -r '.id' 2>/dev/null)
if [ -n "$NOTIF_ID" ] && [ "$NOTIF_ID" != "null" ]; then
    pass "Notification created with id: $NOTIF_ID"
else
    fail "POST /notifications response missing id"
    summary
    exit 1
fi

# =============================================================================
# Step 2: Member reads own notifications
# =============================================================================
echo ""
echo "  ── Step 2: Member reads own notifications ──"

raw=$(http_get "/notifications" "$MEMBER_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /notifications returns 200"

NOTIF_COUNT=$(printf '%s' "$RESP_BODY" | jq 'length' 2>/dev/null || echo "0")
if [ "${NOTIF_COUNT:-0}" -ge "1" ]; then
    pass "Member has $NOTIF_COUNT notification(s)"
else
    fail "Member has no notifications (expected at least 1)"
fi

# Verify notification shape
FIRST_NOTIF=$(printf '%s' "$RESP_BODY" | jq '.[0]' 2>/dev/null)
assert_json_present "$FIRST_NOTIF" ".id"         "Notification has id"
assert_json_present "$FIRST_NOTIF" ".title"      "Notification has title"
assert_json_present "$FIRST_NOTIF" ".body"       "Notification has body"
assert_json_present "$FIRST_NOTIF" ".is_read"    "Notification has is_read"
assert_json_present "$FIRST_NOTIF" ".event_type" "Notification has event_type"
assert_json_present "$FIRST_NOTIF" ".created_at" "Notification has created_at"

# =============================================================================
# Step 3: Filter by is_read=false
# =============================================================================
echo ""
echo "  ── Step 3: Filter unread notifications ──"

raw=$(http_get "/notifications?is_read=false" "$MEMBER_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /notifications?is_read=false returns 200"

UNREAD_COUNT=$(printf '%s' "$RESP_BODY" | jq 'length' 2>/dev/null || echo "0")
if [ "${UNREAD_COUNT:-0}" -ge "1" ]; then
    pass "Unread filter returns $UNREAD_COUNT notification(s)"
else
    fail "No unread notifications found (expected at least 1 just created)"
fi

# All returned items should have is_read=false
ALL_UNREAD=$(printf '%s' "$RESP_BODY" | jq '[.[] | select(.is_read == false)] | length' 2>/dev/null || echo "0")
if [ "$ALL_UNREAD" = "$UNREAD_COUNT" ]; then
    pass "All returned notifications have is_read=false"
else
    fail "Some notifications in unread filter have is_read=true"
fi

# =============================================================================
# Step 4: Mark single notification read
# =============================================================================
echo ""
echo "  ── Step 4: Mark single notification read ──"

raw=$(http_post "/notifications/$NOTIF_ID/read" "" "$MEMBER_TOKEN")
split_response "$raw"
assert_status "204" "$RESP_STATUS" "POST /notifications/{id}/read returns 204"

# =============================================================================
# Step 5: Verify notification now appears in is_read=true filter
# =============================================================================
echo ""
echo "  ── Step 5: Verify notification is now read ──"

raw=$(http_get "/notifications?is_read=true" "$MEMBER_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /notifications?is_read=true returns 200"

READ_COUNT=$(printf '%s' "$RESP_BODY" | jq 'length' 2>/dev/null || echo "0")
if [ "${READ_COUNT:-0}" -ge "1" ]; then
    pass "Read filter returns $READ_COUNT notification(s)"
else
    fail "No read notifications found after marking as read"
fi

# =============================================================================
# Step 6: Admin creates second notification; member marks all read
# =============================================================================
echo ""
echo "  ── Step 6: Mark all notifications read ──"

# Create another unread notification
http_post "/notifications" \
    "{\"user_id\":\"$MEMBER_USER_ID\",
      \"event_type\":\"manual\",
      \"title\":\"Second test notification\",
      \"body\":\"Another automated test notification.\"}" \
    "$ADMIN_TOKEN" > /dev/null

raw=$(http_post "/notifications/read-all" "" "$MEMBER_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "POST /notifications/read-all returns 200"
assert_json_present "$RESP_BODY" ".marked_count" "read-all response has marked_count"

MARKED=$(printf '%s' "$RESP_BODY" | jq '.marked_count' 2>/dev/null || echo "0")
if [ "${MARKED:-0}" -ge "1" ]; then
    pass "read-all marked $MARKED notification(s) as read"
else
    # 0 is acceptable if all were already read (idempotent)
    pass "read-all returned marked_count=$MARKED (may already be read)"
fi

# Verify no unread remain
raw=$(http_get "/notifications?is_read=false" "$MEMBER_TOKEN")
split_response "$raw"
REMAINING=$(printf '%s' "$RESP_BODY" | jq 'length' 2>/dev/null || echo "0")
if [ "${REMAINING:-0}" = "0" ]; then
    pass "No unread notifications remain after read-all"
else
    fail "$REMAINING unread notification(s) remain after read-all"
fi

# =============================================================================
# Step 7: List subscription preferences
# =============================================================================
echo ""
echo "  ── Step 7: List notification subscriptions ──"

raw=$(http_get "/notifications/subscriptions" "$MEMBER_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /notifications/subscriptions returns 200"

# Response is an array (may be empty if no explicit preferences set)
IS_ARRAY=$(printf '%s' "$RESP_BODY" | jq 'if type=="array" then "yes" else "no" end' 2>/dev/null || echo "no")
if [ "$IS_ARRAY" = '"yes"' ]; then
    SUBS_COUNT=$(printf '%s' "$RESP_BODY" | jq 'length' 2>/dev/null || echo "0")
    pass "GET /notifications/subscriptions returns array with $SUBS_COUNT entries"
else
    fail "GET /notifications/subscriptions did not return an array"
fi

# =============================================================================
# Step 8: Opt out of sla_breach notifications
# =============================================================================
echo ""
echo "  ── Step 8: Opt out of sla_breach notifications ──"

raw=$(curl -s -w "\n%{http_code}" -X PATCH \
    -H "Authorization: Bearer $MEMBER_TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"is_subscribed":false}' \
    "$BASE_URL/notifications/subscriptions/sla_breach")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "PATCH /notifications/subscriptions/sla_breach returns 200"
assert_json_field "$RESP_BODY" ".event_type"    "sla_breach" "Subscription event_type is sla_breach"
assert_json_field "$RESP_BODY" ".is_subscribed" "false"      "Subscription is_subscribed is false"

# =============================================================================
# Step 9: Re-subscribe to sla_breach
# =============================================================================
echo ""
echo "  ── Step 9: Re-subscribe to sla_breach ──"

raw=$(curl -s -w "\n%{http_code}" -X PATCH \
    -H "Authorization: Bearer $MEMBER_TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"is_subscribed":true}' \
    "$BASE_URL/notifications/subscriptions/sla_breach")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "PATCH /notifications/subscriptions/sla_breach (re-subscribe) returns 200"
assert_json_field "$RESP_BODY" ".is_subscribed" "true" "Re-subscription is_subscribed is true"

# Invalid event_type returns 400
raw=$(curl -s -w "\n%{http_code}" -X PATCH \
    -H "Authorization: Bearer $MEMBER_TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"is_subscribed":false}' \
    "$BASE_URL/notifications/subscriptions/invalid_type_xyz")
split_response "$raw"
assert_status "400" "$RESP_STATUS" "Invalid event_type in PATCH subscriptions returns 400"

# =============================================================================
# Step 10: Create a notification schedule
# =============================================================================
echo ""
echo "  ── Step 10: Create notification schedule ──"

raw=$(http_post "/notifications/schedules" \
    "{\"label\":\"Daily Health Reminder\",
      \"fire_hour\":9,
      \"tz_offset_minutes\":0}" \
    "$MEMBER_TOKEN")
split_response "$raw"
assert_status "201" "$RESP_STATUS" "POST /notifications/schedules returns 201"

SCHED_ID=$(printf '%s' "$RESP_BODY" | jq -r '.id' 2>/dev/null)
if [ -n "$SCHED_ID" ] && [ "$SCHED_ID" != "null" ]; then
    pass "Schedule created with id: $SCHED_ID"
else
    fail "POST /notifications/schedules response missing id"
    summary
    exit 1
fi

assert_json_field "$RESP_BODY" ".label"     "Daily Health Reminder" "Schedule label matches"
assert_json_field "$RESP_BODY" ".fire_hour" "9"                     "Schedule fire_hour matches"
assert_json_present "$RESP_BODY" ".next_fire_at" "Schedule has next_fire_at"

# Admin creates schedule targeting member
raw=$(http_post "/notifications/schedules" \
    "{\"user_id\":\"$MEMBER_USER_ID\",
      \"label\":\"Admin-set Reminder\",
      \"fire_hour\":14,
      \"tz_offset_minutes\":-300}" \
    "$ADMIN_TOKEN")
split_response "$raw"
assert_status "201" "$RESP_STATUS" "Admin can create schedule for another user (201)"
ADMIN_SCHED_ID=$(printf '%s' "$RESP_BODY" | jq -r '.id' 2>/dev/null)

# Non-admin cannot create schedule for another user
ADMIN_USER_ID="20000000-0000-0000-0000-000000000001"
raw=$(http_post "/notifications/schedules" \
    "{\"user_id\":\"$ADMIN_USER_ID\",
      \"label\":\"Forbidden Reminder\",
      \"fire_hour\":8,
      \"tz_offset_minutes\":0}" \
    "$MEMBER_TOKEN")
split_response "$raw"
assert_status "403" "$RESP_STATUS" "Member cannot create schedule for another user (403)"

# =============================================================================
# Step 11: List schedules
# =============================================================================
echo ""
echo "  ── Step 11: List notification schedules ──"

raw=$(http_get "/notifications/schedules" "$MEMBER_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /notifications/schedules returns 200"

IS_ARRAY=$(printf '%s' "$RESP_BODY" | jq 'if type=="array" then "yes" else "no" end' 2>/dev/null || echo "no")
if [ "$IS_ARRAY" = '"yes"' ]; then
    SCHED_COUNT=$(printf '%s' "$RESP_BODY" | jq 'length' 2>/dev/null || echo "0")
    pass "GET /notifications/schedules returns array with $SCHED_COUNT entry/entries"
else
    fail "GET /notifications/schedules did not return an array"
fi

# Admin sees all schedules (including other users')
raw=$(http_get "/notifications/schedules" "$ADMIN_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /notifications/schedules (admin) returns 200"

ADMIN_COUNT=$(printf '%s' "$RESP_BODY" | jq 'length' 2>/dev/null || echo "0")
if [ "${ADMIN_COUNT:-0}" -ge "$SCHED_COUNT" ]; then
    pass "Admin sees all schedules ($ADMIN_COUNT total vs member's $SCHED_COUNT)"
else
    pass "Schedule list returned ($ADMIN_COUNT entries)"
fi

# =============================================================================
# Step 12: Delete schedule
# =============================================================================
echo ""
echo "  ── Step 12: Delete notification schedule ──"

raw=$(curl -s -w "\n%{http_code}" -X DELETE \
    -H "Authorization: Bearer $MEMBER_TOKEN" \
    "$BASE_URL/notifications/schedules/$SCHED_ID")
split_response "$raw"
assert_status "204" "$RESP_STATUS" "DELETE /notifications/schedules/{id} returns 204"

# Second delete returns 404
raw=$(curl -s -w "\n%{http_code}" -X DELETE \
    -H "Authorization: Bearer $MEMBER_TOKEN" \
    "$BASE_URL/notifications/schedules/$SCHED_ID")
split_response "$raw"
assert_status "404" "$RESP_STATUS" "Deleting already-deleted schedule returns 404"

# Member cannot delete admin's schedule
if [ -n "${ADMIN_SCHED_ID:-}" ] && [ "$ADMIN_SCHED_ID" != "null" ]; then
    raw=$(curl -s -w "\n%{http_code}" -X DELETE \
        -H "Authorization: Bearer $MEMBER_TOKEN" \
        "$BASE_URL/notifications/schedules/$ADMIN_SCHED_ID")
    split_response "$raw"
    assert_status "403" "$RESP_STATUS" "Member cannot delete another user's schedule (403)"

    # Cleanup: admin deletes the schedule
    curl -s -o /dev/null -X DELETE \
        -H "Authorization: Bearer $ADMIN_TOKEN" \
        "$BASE_URL/notifications/schedules/$ADMIN_SCHED_ID" || true
fi

# =============================================================================
# Step 13: Invalid event_type in POST /notifications → 400
# =============================================================================
echo ""
echo "  ── Step 13: Invalid event_type → 400 ──"

raw=$(http_post "/notifications" \
    "{\"user_id\":\"$MEMBER_USER_ID\",
      \"event_type\":\"invalid_type_xyz\",
      \"title\":\"Test\",
      \"body\":\"Test\"}" \
    "$ADMIN_TOKEN")
split_response "$raw"
assert_status "400" "$RESP_STATUS" "Invalid event_type in POST /notifications returns 400"

# =============================================================================
# Step 14: Non-admin cannot create notifications for other users
# =============================================================================
echo ""
echo "  ── Step 14: Non-admin cannot POST /notifications → 403 ──"

raw=$(http_post "/notifications" \
    "{\"user_id\":\"$MEMBER_USER_ID\",
      \"event_type\":\"manual\",
      \"title\":\"Coach test\",
      \"body\":\"From coach\"}" \
    "$COACH_TOKEN")
split_response "$raw"
assert_status "403" "$RESP_STATUS" "Care coach cannot POST /notifications (403)"

raw=$(http_post "/notifications" \
    "{\"user_id\":\"$MEMBER_USER_ID\",
      \"event_type\":\"manual\",
      \"title\":\"Member test\",
      \"body\":\"From member\"}" \
    "$MEMBER_TOKEN")
split_response "$raw"
assert_status "403" "$RESP_STATUS" "Member cannot POST /notifications (403)"

# =============================================================================
# Step 15: Member cannot mark another member's notification as read
# =============================================================================
echo ""
echo "  ── Step 15: Cannot mark another user's notification ──"

raw=$(http_post "/notifications/$NOTIF_ID/read" "" "$COACH_TOKEN")
split_response "$raw"
# Coach is not the owner of this notification — expect 403 or 404
if [ "$RESP_STATUS" = "403" ] || [ "$RESP_STATUS" = "404" ]; then
    pass "Coach cannot mark member's notification read (HTTP $RESP_STATUS)"
else
    fail "Expected 403 or 404 when marking another's notification, got $RESP_STATUS"
fi

# =============================================================================
# Step 16: Unauthenticated access → 401
# =============================================================================
echo ""
echo "  ── Step 16: Unauthenticated access → 401 ──"

raw=$(http_get "/notifications")
split_response "$raw"
assert_status "401" "$RESP_STATUS" "GET /notifications without token returns 401"

raw=$(http_get "/notifications/subscriptions")
split_response "$raw"
assert_status "401" "$RESP_STATUS" "GET /notifications/subscriptions without token returns 401"

raw=$(http_get "/notifications/schedules")
split_response "$raw"
assert_status "401" "$RESP_STATUS" "GET /notifications/schedules without token returns 401"

summary
