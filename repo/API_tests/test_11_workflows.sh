#!/usr/bin/env bash
# =============================================================================
# API test: workflow templates, instances, and actions
# =============================================================================
# Exercises:
#   1. POST /workflows/templates — admin creates a template (name, risk_tier)
#   2. Duplicate template name → 409
#   3. Non-admin cannot create templates → 403
#   4. POST /workflows/templates/{id}/nodes — admin adds sequential approval nodes
#   5. POST /workflows/instances — admin starts a workflow instance
#   6. GET /workflows/instances/{id} — inspect status, approvals, SLA fields
#   7. POST /workflows/instances/{id}/actions — approve advances the stage
#   8. Reject action marks instance "rejected"
#   9. Invalid action value → 400
#  10. Member cannot take action on a workflow → 403
# =============================================================================
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../tests_common.sh"

echo "▶  [API] Workflow templates, instances, and actions"

# ── Login ─────────────────────────────────────────────────────
ADMIN_TOKEN=$(login "$ADMIN_USER" "$ADMIN_PASS")
[ -n "$ADMIN_TOKEN" ] || { echo "ERROR: admin login failed"; exit 1; }

COACH_TOKEN=$(login "$COACH_USER" "$COACH_PASS")
[ -n "$COACH_TOKEN" ] || { echo "ERROR: coach login failed"; exit 1; }

MEMBER_TOKEN=$(login "$MEMBER_USER" "$MEMBER_PASS")
[ -n "$MEMBER_TOKEN" ] || { echo "ERROR: member login failed"; exit 1; }

# Use a unique name per run to avoid conflicts across test runs
RUN_ID=$(date -u +%s)
TEMPLATE_NAME="Test Approval Workflow $RUN_ID"

# =============================================================================
# Step 1: Admin creates a workflow template
# =============================================================================
echo ""
echo "  ── Step 1: Create workflow template (admin) ──"

raw=$(http_post "/workflows/templates" \
    "{\"name\":\"$TEMPLATE_NAME\",
      \"description\":\"Automated test template\",
      \"business_type\":\"care_coordination\",
      \"org_unit_id\":\"$ORG_UNIT_ID\",
      \"risk_tier\":\"low\"}" \
    "$ADMIN_TOKEN")
split_response "$raw"
assert_status "201" "$RESP_STATUS" "POST /workflows/templates returns 201"

TEMPLATE_ID=$(printf '%s' "$RESP_BODY" | jq -r '.id' 2>/dev/null)
if [ -n "$TEMPLATE_ID" ] && [ "$TEMPLATE_ID" != "null" ]; then
    pass "Template created with id: $TEMPLATE_ID"
else
    fail "Template response missing id"
    summary
    exit 1
fi

# Validate response fields
assert_json_field "$RESP_BODY" ".name"      "$TEMPLATE_NAME" "Template name matches"
assert_json_field "$RESP_BODY" ".risk_tier" "low"            "Template risk_tier matches"
assert_json_field "$RESP_BODY" ".is_active" "true"           "Template is active"

# =============================================================================
# Step 2: Duplicate template name → 409
# =============================================================================
echo ""
echo "  ── Step 2: Duplicate template name → 409 ──"

raw=$(http_post "/workflows/templates" \
    "{\"name\":\"$TEMPLATE_NAME\",
      \"description\":\"Duplicate\",
      \"business_type\":\"care_coordination\",
      \"org_unit_id\":\"$ORG_UNIT_ID\"}" \
    "$ADMIN_TOKEN")
split_response "$raw"
assert_status "409" "$RESP_STATUS" "Duplicate template name returns 409"

# =============================================================================
# Step 3: Non-admin cannot create templates
# =============================================================================
echo ""
echo "  ── Step 3: Non-admin template creation → 403 ──"

raw=$(http_post "/workflows/templates" \
    "{\"name\":\"Coach Template $RUN_ID\",
      \"description\":\"Should fail\",
      \"business_type\":\"care_coordination\",
      \"org_unit_id\":\"$ORG_UNIT_ID\"}" \
    "$COACH_TOKEN")
split_response "$raw"
assert_status "403" "$RESP_STATUS" "Care coach cannot create templates (403)"

raw=$(http_post "/workflows/templates" \
    "{\"name\":\"Member Template $RUN_ID\",
      \"description\":\"Should fail\",
      \"business_type\":\"care_coordination\",
      \"org_unit_id\":\"$ORG_UNIT_ID\"}" \
    "$MEMBER_TOKEN")
split_response "$raw"
assert_status "403" "$RESP_STATUS" "Member cannot create templates (403)"

# =============================================================================
# Step 4: Add nodes to the template
# =============================================================================
echo ""
echo "  ── Step 4: Add approval nodes to template ──"

# Node 1 (stage 1)
raw=$(http_post "/workflows/templates/$TEMPLATE_ID/nodes" \
    "{\"name\":\"Initial Review\",
      \"node_order\":1,
      \"is_parallel\":false,
      \"action_type\":\"approve\"}" \
    "$ADMIN_TOKEN")
split_response "$raw"
assert_status "201" "$RESP_STATUS" "POST /workflows/templates/{id}/nodes returns 201 (node 1)"

NODE_ID=$(printf '%s' "$RESP_BODY" | jq -r '.id' 2>/dev/null)
if [ -n "$NODE_ID" ] && [ "$NODE_ID" != "null" ]; then
    pass "Node 1 created with id: $NODE_ID"
else
    fail "Node 1 response missing id"
fi

assert_json_field "$RESP_BODY" ".name"       "Initial Review" "Node name matches"
assert_json_field "$RESP_BODY" ".node_order" "1"              "Node order is 1"

# Node 2 (stage 2)
raw=$(http_post "/workflows/templates/$TEMPLATE_ID/nodes" \
    "{\"name\":\"Final Approval\",
      \"node_order\":2,
      \"is_parallel\":false,
      \"action_type\":\"approve\"}" \
    "$ADMIN_TOKEN")
split_response "$raw"
assert_status "201" "$RESP_STATUS" "POST /workflows/templates/{id}/nodes returns 201 (node 2)"

# =============================================================================
# Step 5: Start a workflow instance
# =============================================================================
echo ""
echo "  ── Step 5: Start workflow instance ──"

raw=$(http_post "/workflows/instances" \
    "{\"template_id\":\"$TEMPLATE_ID\"}" \
    "$ADMIN_TOKEN")
split_response "$raw"
assert_status "201" "$RESP_STATUS" "POST /workflows/instances returns 201"

INSTANCE_ID=$(printf '%s' "$RESP_BODY" | jq -r '.id' 2>/dev/null)
if [ -n "$INSTANCE_ID" ] && [ "$INSTANCE_ID" != "null" ]; then
    pass "Workflow instance created with id: $INSTANCE_ID"
else
    fail "Workflow instance response missing id"
    summary
    exit 1
fi

assert_json_field "$RESP_BODY" ".template_id" "$TEMPLATE_ID" "Instance template_id matches"
assert_json_field "$RESP_BODY" ".status"      "in_progress"  "New instance is in_progress"

# =============================================================================
# Step 6: GET /workflows/instances/{id} — inspect state
# =============================================================================
echo ""
echo "  ── Step 6: Inspect workflow instance ──"

raw=$(http_get "/workflows/instances/$INSTANCE_ID" "$ADMIN_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "GET /workflows/instances/{id} returns 200"
assert_json_field "$RESP_BODY" ".id"            "$INSTANCE_ID" "Instance id matches"
assert_json_field "$RESP_BODY" ".status"        "in_progress"  "Instance status is in_progress"
assert_json_field "$RESP_BODY" ".current_stage" "1"            "Instance starts at stage 1"

# Verify approvals array is present
APPROVALS_COUNT=$(printf '%s' "$RESP_BODY" | jq '.approvals | length' 2>/dev/null || echo "0")
if [ "${APPROVALS_COUNT:-0}" -ge "1" ]; then
    pass "Instance has $APPROVALS_COUNT approval(s) for stage 1"
else
    fail "Instance has no approvals (expected at least 1 for stage 1)"
fi

# Verify SLA fields exist
assert_json_present "$RESP_BODY" ".approvals[0].sla_deadline" "Approval has sla_deadline"
assert_json_present "$RESP_BODY" ".approvals[0].sla_breached" "Approval has sla_breached field"

# =============================================================================
# Step 7: Approve action — advances to stage 2
# =============================================================================
echo ""
echo "  ── Step 7: Approve action (stage 1 → stage 2) ──"

raw=$(http_post "/workflows/instances/$INSTANCE_ID/actions" \
    '{"action":"approve","comment":"Stage 1 looks good"}' \
    "$ADMIN_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "POST /workflows/instances/{id}/actions (approve) returns 200"
assert_json_field "$RESP_BODY" ".current_stage" "2" "Instance advances to stage 2 after approval"
assert_json_field "$RESP_BODY" ".status"        "in_progress" "Instance remains in_progress at stage 2"

# =============================================================================
# Step 8: Approve stage 2 — completes the workflow
# =============================================================================
echo ""
echo "  ── Step 8: Approve stage 2 — workflow completes ──"

raw=$(http_post "/workflows/instances/$INSTANCE_ID/actions" \
    '{"action":"approve","comment":"Stage 2 approved — workflow complete"}' \
    "$ADMIN_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "POST /workflows/instances/{id}/actions (approve stage 2) returns 200"
assert_json_field "$RESP_BODY" ".status" "completed" "Instance status is completed after all approvals"

# completed_at should now be set
assert_json_present "$RESP_BODY" ".completed_at" "completed_at is set after workflow completion"

# =============================================================================
# Step 9: Start a second instance for reject test
# =============================================================================
echo ""
echo "  ── Step 9: Reject action ──"

raw=$(http_post "/workflows/instances" \
    "{\"template_id\":\"$TEMPLATE_ID\"}" \
    "$ADMIN_TOKEN")
split_response "$raw"
assert_status "201" "$RESP_STATUS" "Second instance created for reject test"
INSTANCE_ID2=$(printf '%s' "$RESP_BODY" | jq -r '.id' 2>/dev/null)

raw=$(http_post "/workflows/instances/$INSTANCE_ID2/actions" \
    '{"action":"reject","comment":"Does not meet criteria"}' \
    "$ADMIN_TOKEN")
split_response "$raw"
assert_status "200" "$RESP_STATUS" "POST /workflows/instances/{id}/actions (reject) returns 200"
assert_json_field "$RESP_BODY" ".status" "rejected" "Instance status is rejected after reject action"

# =============================================================================
# Step 10: Invalid action value → 400
# =============================================================================
echo ""
echo "  ── Step 10: Invalid action → 400 ──"

raw=$(http_post "/workflows/instances" \
    "{\"template_id\":\"$TEMPLATE_ID\"}" \
    "$ADMIN_TOKEN")
split_response "$raw"
INSTANCE_ID3=$(printf '%s' "$RESP_BODY" | jq -r '.id' 2>/dev/null)

raw=$(http_post "/workflows/instances/$INSTANCE_ID3/actions" \
    '{"action":"invalid_action_xyz"}' \
    "$ADMIN_TOKEN")
split_response "$raw"
assert_status "400" "$RESP_STATUS" "Invalid action returns 400"

# =============================================================================
# Step 11: Member cannot take actions → 403
# =============================================================================
echo ""
echo "  ── Step 11: Member cannot take actions → 403 ──"

raw=$(http_post "/workflows/instances/$INSTANCE_ID3/actions" \
    '{"action":"approve"}' \
    "$MEMBER_TOKEN")
split_response "$raw"
assert_status "403" "$RESP_STATUS" "Member cannot take workflow action (403)"

# =============================================================================
# Step 12: Unauthenticated access → 401
# =============================================================================
echo ""
echo "  ── Step 12: Unauthenticated access → 401 ──"

raw=$(http_get "/workflows/instances/$INSTANCE_ID")
split_response "$raw"
assert_status "401" "$RESP_STATUS" "GET /workflows/instances/{id} without token returns 401"

summary
