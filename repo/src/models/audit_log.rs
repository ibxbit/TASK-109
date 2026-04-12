use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::schema::audit_logs;

// ── Reason codes ─────────────────────────────────────────────
// Standardised constant strings for the `reason_code` column.
// Using constants keeps callers consistent and makes the column
// filterable without full-text search.

#[allow(dead_code)]
// Auth
pub const RC_LOGIN_SUCCESS:        &str = "LOGIN_SUCCESS";
#[allow(dead_code)]
pub const RC_LOGIN_FAILED:         &str = "LOGIN_FAILED";
#[allow(dead_code)]
pub const RC_LOGIN_BLOCKED:        &str = "LOGIN_BLOCKED_LOCKED";
#[allow(dead_code)]
pub const RC_LOGOUT:               &str = "LOGOUT";
#[allow(dead_code)]
pub const RC_ACCOUNT_LOCKED:       &str = "ACCOUNT_LOCKED";

// Data access (reads of sensitive data)
#[allow(dead_code)]
pub const RC_HEALTH_PROFILE_READ:  &str = "HEALTH_PROFILE_READ";
#[allow(dead_code)]
pub const RC_ANALYTICS_READ:       &str = "ANALYTICS_READ";
#[allow(dead_code)]
pub const RC_ANALYTICS_DOWNLOAD:   &str = "ANALYTICS_DOWNLOAD";
#[allow(dead_code)]
pub const RC_ANALYTICS_EXPORT:     &str = "ANALYTICS_EXPORT";

// Data mutations
#[allow(dead_code)]
pub const RC_HEALTH_PROFILE_CREATED: &str = "HEALTH_PROFILE_CREATED";
#[allow(dead_code)]
pub const RC_HEALTH_PROFILE_UPDATED: &str = "HEALTH_PROFILE_UPDATED";
#[allow(dead_code)]
pub const RC_GOAL_CREATED:           &str = "GOAL_CREATED";
#[allow(dead_code)]
pub const RC_GOAL_UPDATED:           &str = "GOAL_UPDATED";
#[allow(dead_code)]
pub const RC_GOAL_AUTO_COMPLETED:    &str = "GOAL_AUTO_COMPLETED";
#[allow(dead_code)]
pub const RC_METRIC_ENTRY_CREATED:   &str = "METRIC_ENTRY_CREATED";

// Work orders
#[allow(dead_code)]
pub const RC_WORK_ORDER_CREATED:     &str = "WORK_ORDER_CREATED";
#[allow(dead_code)]
pub const RC_WORK_ORDER_TRANSITION:  &str = "WORK_ORDER_TRANSITION";

// Workflows
#[allow(dead_code)]
pub const RC_WORKFLOW_TEMPLATE_CREATED:   &str = "WORKFLOW_TEMPLATE_CREATED";
#[allow(dead_code)]
pub const RC_WORKFLOW_NODE_ADDED:         &str = "WORKFLOW_NODE_ADDED";
#[allow(dead_code)]
pub const RC_WORKFLOW_STARTED:            &str = "WORKFLOW_STARTED";
#[allow(dead_code)]
pub const RC_WORKFLOW_RESUBMITTED:        &str = "WORKFLOW_RESUBMITTED";
#[allow(dead_code)]
pub const RC_WORKFLOW_WITHDRAWN:          &str = "WORKFLOW_WITHDRAWN";
#[allow(dead_code)]
pub const RC_APPROVAL_APPROVED:           &str = "APPROVAL_APPROVED";
#[allow(dead_code)]
pub const RC_APPROVAL_REJECTED:           &str = "APPROVAL_REJECTED";
#[allow(dead_code)]
pub const RC_APPROVAL_RETURNED_FOR_EDIT:  &str = "APPROVAL_RETURNED_FOR_EDIT";
#[allow(dead_code)]
pub const RC_APPROVAL_REASSIGNED:         &str = "APPROVAL_REASSIGNED";
#[allow(dead_code)]
pub const RC_ADDITIONAL_SIGN_OFF:         &str = "ADDITIONAL_SIGN_OFF_REQUESTED";
#[allow(dead_code)]
pub const RC_SLA_BREACHED:                &str = "SLA_BREACHED";

// Notifications
#[allow(dead_code)]
pub const RC_NOTIFICATION_CREATED:             &str = "NOTIFICATION_CREATED";
#[allow(dead_code)]
pub const RC_NOTIFICATION_READ:                &str = "NOTIFICATION_READ";
#[allow(dead_code)]
pub const RC_NOTIFICATION_ALL_READ:            &str = "NOTIFICATION_ALL_READ";
#[allow(dead_code)]
pub const RC_NOTIFICATION_SUBSCRIPTION_UPDATED: &str = "NOTIFICATION_SUBSCRIPTION_UPDATED";
#[allow(dead_code)]
pub const RC_NOTIFICATION_SCHEDULE_CREATED:    &str = "NOTIFICATION_SCHEDULE_CREATED";
#[allow(dead_code)]
pub const RC_NOTIFICATION_SCHEDULE_DELETED:    &str = "NOTIFICATION_SCHEDULE_DELETED";

// ── DB row ───────────────────────────────────────────────────

#[derive(Debug, Queryable, Selectable, Identifiable)]
#[diesel(table_name = audit_logs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AuditLog {
    pub id:          Uuid,
    pub actor_id:    Option<Uuid>,
    pub action:      String,
    pub entity_type: String,
    pub entity_id:   Option<Uuid>,
    pub old_value:   Option<Value>,
    pub new_value:   Option<Value>,
    pub ip_address:  Option<String>,
    pub created_at:  DateTime<Utc>,
    pub reason_code: Option<String>,
    pub old_hash:    Option<String>,
    pub new_hash:    Option<String>,
}

// ── Insert struct ─────────────────────────────────────────────

#[derive(Debug, Insertable)]
#[diesel(table_name = audit_logs)]
pub struct NewAuditLog {
    pub id:          Uuid,
    pub actor_id:    Option<Uuid>,
    pub action:      String,
    pub entity_type: String,
    pub entity_id:   Option<Uuid>,
    pub old_value:   Option<Value>,
    pub new_value:   Option<Value>,
    pub ip_address:  Option<String>,
    pub created_at:  DateTime<Utc>,
    pub reason_code: Option<String>,
    /// SHA-256 hex of the serialised `old_value` JSON (tamper evidence).
    pub old_hash:    Option<String>,
    /// SHA-256 hex of the serialised `new_value` JSON (tamper evidence).
    pub new_hash:    Option<String>,
}

impl NewAuditLog {
    /// Create a new audit log entry.
    ///
    /// `action` should use the `RC_*` constants defined in this module.
    /// Call builder methods to attach old/new values and a reason code.
    pub fn new(
        actor_id:    Option<Uuid>,
        action:      impl Into<String>,
        entity_type: impl Into<String>,
        entity_id:   Option<Uuid>,
        ip_address:  Option<String>,
    ) -> Self {
        let action_str = action.into();
        // Default reason_code equals the action string — callers can
        // override via `.with_reason_code()` for finer granularity.
        let reason_code = Some(action_str.clone());
        Self {
            id:          Uuid::new_v4(),
            actor_id,
            action:      action_str,
            entity_type: entity_type.into(),
            entity_id,
            old_value:   None,
            new_value:   None,
            ip_address,
            created_at:  Utc::now(),
            reason_code,
            old_hash:    None,
            new_hash:    None,
        }
    }

    /// Attach the new (after) state and compute its SHA-256 hash.
    pub fn with_new_value(mut self, value: Value) -> Self {
        self.new_hash  = Some(hash_json(&value));
        self.new_value = Some(value);
        self
    }

    /// Attach the old (before) state and compute its SHA-256 hash.
    pub fn with_old_value(mut self, value: Value) -> Self {
        self.old_hash  = Some(hash_json(&value));
        self.old_value = Some(value);
        self
    }

    /// Override the reason code with a more specific value.
    pub fn with_reason_code(mut self, code: impl Into<String>) -> Self {
        self.reason_code = Some(code.into());
        self
    }
}

// ── Hash helper ───────────────────────────────────────────────

/// Compute SHA-256 of the canonical JSON serialisation.
///
/// The hash is stored alongside the value so that any later tampering
/// of the stored JSON is detectable by recomputing and comparing.
fn hash_json(value: &Value) -> String {
    let json_str = value.to_string();
    let mut hasher = Sha256::new();
    hasher.update(json_str.as_bytes());
    hex::encode(hasher.finalize())
}

// ── Insert helper ─────────────────────────────────────────────

/// Insert an audit log row.
///
/// Errors are silently ignored so that a logging failure never
/// blocks the primary operation.  The function logs a warning if
/// the insert fails so the issue surfaces in structured logs.
pub fn insert(conn: &mut diesel::PgConnection, log: NewAuditLog) {
    use diesel::RunQueryDsl;
    use tracing::warn;

    if let Err(e) = diesel::insert_into(audit_logs::table)
        .values(&log)
        .execute(conn)
    {
        warn!(
            action      = %log.action,
            entity_type = %log.entity_type,
            error       = %e,
            "AUDIT_LOG_INSERT_FAILED"
        );
    }
}

/// Insert an audit log row for a **critical** security event.
///
/// Unlike [`insert`], this variant propagates the error so that the
/// caller can abort the operation if the audit trail cannot be
/// maintained.  Use this for events where a missing log entry would
/// be a compliance violation (e.g. LOGIN_SUCCESS, LOGIN_FAILED).
pub fn insert_critical(
    conn: &mut diesel::PgConnection,
    log: NewAuditLog,
) -> Result<(), crate::errors::AppError> {
    use diesel::RunQueryDsl;

    diesel::insert_into(audit_logs::table)
        .values(&log)
        .execute(conn)
        .map_err(crate::errors::AppError::Database)?;

    Ok(())
}

// ── Query types ───────────────────────────────────────────────

/// Filters accepted by the `GET /audit-logs` admin endpoint.
#[derive(Debug, serde::Deserialize)]
pub struct AuditLogQuery {
    pub actor_id:    Option<Uuid>,
    pub action:      Option<String>,
    pub reason_code: Option<String>,
    pub entity_type: Option<String>,
    pub entity_id:   Option<Uuid>,
    pub start_date:  Option<String>,
    pub end_date:    Option<String>,
    /// 1-based page number (default 1).
    pub page:        Option<i64>,
    /// Rows per page (default 50, capped at 200).
    pub per_page:    Option<i64>,
}

/// Public representation returned by `GET /audit-logs`.
#[derive(Debug, serde::Serialize)]
pub struct AuditLogResponse {
    pub id:          Uuid,
    pub actor_id:    Option<Uuid>,
    pub action:      String,
    pub reason_code: Option<String>,
    pub entity_type: String,
    pub entity_id:   Option<Uuid>,
    pub old_value:   Option<Value>,
    pub new_value:   Option<Value>,
    pub old_hash:    Option<String>,
    pub new_hash:    Option<String>,
    pub ip_address:  Option<String>,
    pub created_at:  DateTime<Utc>,
}

impl From<AuditLog> for AuditLogResponse {
    fn from(a: AuditLog) -> Self {
        Self {
            id:          a.id,
            actor_id:    a.actor_id,
            action:      a.action,
            reason_code: a.reason_code,
            entity_type: a.entity_type,
            entity_id:   a.entity_id,
            old_value:   a.old_value,
            new_value:   a.new_value,
            old_hash:    a.old_hash,
            new_hash:    a.new_hash,
            ip_address:  a.ip_address,
            created_at:  a.created_at,
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct AuditLogPage {
    pub data:     Vec<AuditLogResponse>,
    pub page:     i64,
    pub per_page: i64,
    pub total:    i64,
}
