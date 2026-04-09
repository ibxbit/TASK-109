# VitalPath Audit Event Catalog

Complete reference for all 33 audit events recorded in the `audit_logs` table.

Each event is emitted via `audit_log::insert()` defined in `src/models/audit_log.rs`.
The `reason_code` column equals the `action` string (constants defined at
`src/models/audit_log.rs` lines 14–58).

**Schema fields present on every entry:**

| Field | Type | Description |
|-------|------|-------------|
| `id` | UUID | Primary key |
| `actor_id` | UUID\|null | User who performed the action (null for system events) |
| `action` | TEXT | Reason code constant |
| `reason_code` | TEXT | Same as `action` — filterable structured code |
| `entity_type` | TEXT | Resource type affected |
| `entity_id` | UUID\|null | Primary key of the affected record |
| `old_value` | JSONB\|null | State before mutation (reads: null) |
| `new_value` | JSONB\|null | State after mutation (reads: summary) |
| `old_hash` | TEXT\|null | SHA-256 hex of `old_value` JSON |
| `new_hash` | TEXT\|null | SHA-256 hex of `new_value` JSON |
| `ip_address` | TEXT\|null | Caller IP via `realip_remote_addr()` |
| `created_at` | TIMESTAMPTZ | Wall-clock timestamp at insert |

---

## Authentication Events

### `LOGIN_SUCCESS`

- **Trigger:** `POST /auth/login` with valid credentials
- **Source:** `src/auth/service.rs` line 237
- **Actor:** Authenticated user (the logging-in user)
- **Entity type:** `user`
- **Entity id:** User UUID
- **old_value:** null
- **new_value:** `{"username": "admin", "role": "administrator"}`

**Sample entry:**
```json
{
  "action": "LOGIN_SUCCESS",
  "reason_code": "LOGIN_SUCCESS",
  "actor_id": "20000000-0000-0000-0000-000000000001",
  "entity_type": "user",
  "entity_id": "20000000-0000-0000-0000-000000000001",
  "old_value": null,
  "new_value": {"username": "admin", "role": "administrator"},
  "new_hash": "a3f5b2c1d9e4...",
  "ip_address": "172.18.0.1",
  "created_at": "2025-06-15T14:23:01.123456Z"
}
```

---

### `LOGIN_FAILED`

- **Trigger:** `POST /auth/login` with wrong password or unknown username
- **Source:** `src/auth/service.rs` line 76
- **Actor:** null (unauthenticated)
- **Entity type:** `user`
- **Entity id:** User UUID if username exists, null otherwise
- **new_value:** `{"username": "admin", "attempt": 1, "reason": "wrong_password"}`

---

### `LOGIN_BLOCKED_LOCKED`

- **Trigger:** `POST /auth/login` when account `locked_until > NOW()`
- **Source:** `src/auth/service.rs` line 105
- **Actor:** null
- **Entity type:** `user`
- **new_value:** `{"username": "admin", "locked_until": "2025-06-15T15:23:01Z"}`

---

### `ACCOUNT_LOCKED`

- **Trigger:** 10th consecutive failed login within the 15-minute window
- **Source:** `src/auth/service.rs` line 189
- **Actor:** null
- **Entity type:** `user`
- **new_value:** `{"username": "admin", "locked_until": "2025-06-15T14:38:01Z", "failed_attempts": 10}`

---

### `LOGOUT`

- **Trigger:** `POST /auth/logout`
- **Source:** `src/api/auth.rs` line 169
- **Actor:** Authenticated user
- **Entity type:** `session`
- **Entity id:** Session UUID (the invalidated JWT session)
- **new_value:** `{"session_id": "<uuid>"}`

---

## Health Profile Events

### `HEALTH_PROFILE_CREATED`

- **Trigger:** `POST /profile`
- **Source:** `src/api/health_profile.rs` line 185
- **Roles allowed:** `administrator`, `care_coach`
- **Entity type:** `health_profile`
- **Entity id:** Profile UUID
- **old_value:** null
- **new_value:** `{"member_id": "...", "sex": "male", "height_in": 70.0, "weight_lbs": 175.0, "activity_level": "active", "encryption_key_id": "v1"}`
- **new_hash:** SHA-256 of new_value JSON

**Sample entry:**
```json
{
  "action": "HEALTH_PROFILE_CREATED",
  "reason_code": "HEALTH_PROFILE_CREATED",
  "actor_id": "20000000-0000-0000-0000-000000000002",
  "entity_type": "health_profile",
  "entity_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "old_value": null,
  "new_value": {
    "member_id": "30000000-0000-0000-0000-000000000001",
    "sex": "male", "height_in": 70.0, "weight_lbs": 175.0,
    "activity_level": "active", "encryption_key_id": "v1"
  },
  "new_hash": "b3d4e5f6a7c8..."
}
```

---

### `HEALTH_PROFILE_UPDATED`

- **Trigger:** `PUT /profile/{member_id}`
- **Source:** `src/api/health_profile.rs` line 349
- **Roles allowed:** `administrator`, `care_coach`
- **Entity type:** `health_profile`
- **old_value:** State before update (plaintext dietary_notes excluded from audit — only encryption metadata recorded)
- **new_value:** Updated fields

---

### `HEALTH_PROFILE_READ`

- **Trigger:** `GET /profile/{member_id}`
- **Source:** `src/api/health_profile.rs`
- **Actor:** Authenticated user
- **Entity type:** `health_profile`
- **new_value:** `{"member_id": "...", "actor_role": "care_coach"}`

---

## Analytics Events

### `ANALYTICS_READ`

- **Trigger:** `GET /analytics/export` or internal analytics query
- **Source:** `src/api/analytics.rs` line 763
- **Roles allowed:** `administrator`, `care_coach`
- **new_value:** `{"query_params": {...}, "result_count": 42}`

---

### `ANALYTICS_DOWNLOAD`

- **Trigger:** Download of a previously generated export file
- **Source:** `src/api/analytics.rs` line 868
- **new_value:** `{"filename": "export_2025-06-15.csv", "format": "csv"}`

---

### `ANALYTICS_EXPORT`

- **Trigger:** `POST /analytics/export` (HMAC-signed endpoint)
- **Source:** `src/api/analytics.rs` line 962
- **Roles allowed:** `administrator`, `care_coach` (+ valid HMAC signature required)
- **new_value:** `{"format": "csv", "filename": "vitalpath_export_2025-06-15_143000.csv", "download_url": "/exports/..."}`

---

## Goal Events

### `GOAL_CREATED`

- **Trigger:** `POST /goals`
- **Source:** `src/api/goals.rs` line 137
- **new_value:** `{"member_id": "...", "goal_type": "fat_loss", "title": "Reduce body fat", "target_value": 20.0}`

---

### `GOAL_UPDATED`

- **Trigger:** `PUT /goals/{id}`
- **Source:** `src/api/goals.rs` line 244
- **old_value:** Previous goal state
- **new_value:** Updated goal state
- **old_hash / new_hash:** SHA-256 of respective JSON

---

### `GOAL_AUTO_COMPLETED`

- **Trigger:** Metric entry write where latest value satisfies goal target
- **Source:** `src/api/goals.rs` line 427
- **Actor:** User who recorded the metric (indirectly triggers goal completion)
- **new_value:** `{"goal_id": "...", "goal_type": "fat_loss", "final_value": 19.8}`

---

## Metric Entry Events

### `METRIC_ENTRY_CREATED`

- **Trigger:** `POST /metrics`
- **Source:** `src/api/metric_entries.rs` line 239
- **new_value:** `{"member_id": "...", "metric_type": "weight", "value": 175.0, "recorded_at": "2025-06-15T14:23:01Z"}`

---

## Work Order Events

### `WORK_ORDER_CREATED`

- **Trigger:** `POST /work-orders`
- **Source:** `src/api/work_orders.rs` line 161
- **new_value:** `{"work_order_id": "...", "title": "...", "status": "intake", "org_unit_id": "..."}`

---

### `WORK_ORDER_TRANSITION`

- **Trigger:** `PATCH /work-orders/{id}/transition`
- **Source:** `src/api/work_orders.rs` line 282
- **old_value:** `{"status": "intake"}`
- **new_value:** `{"status": "triage"}`
- **old_hash / new_hash:** SHA-256 of respective JSON

---

## Workflow Engine Events

### `WORKFLOW_TEMPLATE_CREATED`

- **Trigger:** `POST /workflows/templates` (admin only)
- **Source:** `src/api/workflows.rs` line 452
- **new_value:** `{"template_id": "...", "name": "...", "risk_tier": "low", "business_type": "care_coordination"}`

---

### `WORKFLOW_NODE_ADDED`

- **Trigger:** `POST /workflows/templates/{id}/nodes` (admin only)
- **Source:** `src/api/workflows.rs` line 528
- **new_value:** `{"template_id": "...", "node_id": "...", "name": "Initial Review", "node_order": 1}`

---

### `WORKFLOW_STARTED`

- **Trigger:** `POST /workflows/instances`
- **Source:** `src/api/workflows.rs` line 606
- **new_value:** `{"instance_id": "...", "template_id": "...", "status": "in_progress", "current_stage": 1}`

---

### `WORKFLOW_RESUBMITTED`

- **Trigger:** `POST /workflows/instances/{id}/actions` with action `"submit"` (after `returned` status)
- **Source:** `src/api/workflows.rs` line 707
- **new_value:** `{"instance_id": "...", "status": "in_progress"}`

---

### `WORKFLOW_WITHDRAWN`

- **Trigger:** `POST /workflows/instances/{id}/actions` with action `"withdraw"`
- **Source:** `src/api/workflows.rs` line 737
- **new_value:** `{"instance_id": "...", "status": "withdrawn"}`

---

## Approval Events

### `APPROVAL_APPROVED`

- **Trigger:** `POST /workflows/instances/{id}/actions` with action `"approve"`
- **Source:** `src/api/workflows.rs` line 769
- **old_value:** `{"approval_id": "...", "status": "pending"}`
- **new_value:** `{"approval_id": "...", "status": "approved", "comments": "Looks good"}`

---

### `APPROVAL_REJECTED`

- **Trigger:** `POST /workflows/instances/{id}/actions` with action `"reject"`
- **Source:** `src/api/workflows.rs` line 801
- **old_value / new_value / hashes:** As above with `"status": "rejected"`

---

### `APPROVAL_RETURNED_FOR_EDIT`

- **Trigger:** `POST /workflows/instances/{id}/actions` with action `"return_for_edit"`
- **Source:** `src/api/workflows.rs` line 831

---

### `APPROVAL_REASSIGNED`

- **Trigger:** `POST /workflows/instances/{id}/actions` with action `"reassign"`
- **Source:** `src/api/workflows.rs` line 877
- **new_value:** `{"approval_id": "...", "old_assignee_id": null, "new_assignee_id": "..."}`

---

### `ADDITIONAL_SIGN_OFF_REQUESTED`

- **Trigger:** `POST /workflows/instances/{id}/actions` with action `"additional_sign_off"`
- **Source:** `src/api/workflows.rs` line 945

---

### `SLA_BREACHED`

- **Trigger:** System check — approval `sla_deadline < NOW()` and `sla_breached = false`
- **Source:** `src/api/workflows.rs` line 100 (`check_sla()` helper)
- **Actor:** null (system-generated)
- **new_value:** `{"workflow_instance_id": "...", "node_id": "...", "sla_deadline": "2025-06-17T14:23:01Z"}`

---

## Notification Events

### `NOTIFICATION_CREATED`

- **Trigger:** `POST /notifications` (admin only)
- **Source:** `src/api/notifications.rs` line 95
- **new_value:** `{"user_id": "...", "event_type": "manual", "template_id": null}`

---

### `NOTIFICATION_READ`

- **Trigger:** `POST /notifications/{id}/read`
- **Source:** `src/api/notifications.rs` line 205

---

### `NOTIFICATION_ALL_READ`

- **Trigger:** `POST /notifications/read-all`
- **Source:** `src/api/notifications.rs` line 245
- **new_value:** `{"marked_count": 5}`

---

### `NOTIFICATION_SUBSCRIPTION_UPDATED`

- **Trigger:** `PATCH /notifications/subscriptions/{event_type}`
- **Source:** `src/api/notifications.rs` line 345
- **new_value:** `{"event_type": "sla_breach", "is_subscribed": false}`

---

### `NOTIFICATION_SCHEDULE_CREATED`

- **Trigger:** `POST /notifications/schedules`
- **Source:** `src/api/notifications.rs` line 426
- **new_value:** `{"user_id": "...", "label": "Daily Health Reminder", "fire_hour": 9, "next_fire": "2025-06-16T09:00:00Z"}`

---

### `NOTIFICATION_SCHEDULE_DELETED`

- **Trigger:** `DELETE /notifications/schedules/{id}`
- **Source:** `src/api/notifications.rs` line 509

---

## Coverage Summary

| Requirement | Status | Events |
|-------------|--------|--------|
| Login attempts (success + failure) | **Covered** | `LOGIN_SUCCESS`, `LOGIN_FAILED`, `LOGIN_BLOCKED_LOCKED`, `ACCOUNT_LOCKED` |
| Logout | **Covered** | `LOGOUT` |
| Data reads of sensitive data | **Covered** | `HEALTH_PROFILE_READ`, `ANALYTICS_READ`, `ANALYTICS_DOWNLOAD` |
| Data mutations with before/after | **Covered** | `HEALTH_PROFILE_CREATED`, `HEALTH_PROFILE_UPDATED`, `GOAL_CREATED`, `GOAL_UPDATED`, `METRIC_ENTRY_CREATED` |
| Work order state changes | **Covered** | `WORK_ORDER_CREATED`, `WORK_ORDER_TRANSITION` |
| Exports (privileged + HMAC-signed) | **Covered** | `ANALYTICS_EXPORT` |
| Workflow state machine | **Covered** | 8 workflow/approval events + `SLA_BREACHED` |
| Notification management | **Covered** | 6 notification events |
| Config edits | **N/A** | Config is environment-variable only; no runtime config API exists |
| Before/after hashes | **Covered** | `old_hash` + `new_hash` on all mutation events |
| Actor identity | **Covered** | `actor_id` (UUID) + `ip_address` on all events |
| Timestamp | **Covered** | `created_at` TIMESTAMPTZ on all events |
| Reason codes | **Covered** | `reason_code` = structured constant on all events |
| Immutability | **Covered** | Database trigger blocks UPDATE/DELETE |
