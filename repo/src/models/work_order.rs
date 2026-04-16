use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::{errors::AppError, schema::work_orders};

// ── Allowed values ────────────────────────────────────────────

pub const VALID_STATUSES: &[&str] = &[
    "intake", "triage", "in_progress", "waiting_on_member", "resolved", "closed",
];
pub const VALID_PRIORITIES: &[&str] = &["low", "medium", "high", "urgent"];
pub const VALID_TICKET_TYPES: &[&str] =
    &["health_query", "equipment", "scheduling", "nutrition", "emergency"];

// ── DB model ──────────────────────────────────────────────────

#[derive(Debug, Clone, Queryable, Selectable, Identifiable)]
#[diesel(table_name = work_orders)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkOrder {
    pub id: Uuid,
    pub member_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub priority: String,
    pub status: String,
    pub assigned_to: Option<Uuid>,
    pub created_by: Uuid,
    pub due_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // migration 00008
    pub ticket_type: Option<String>,
    pub processing_notes: Option<String>,
    pub routed_to_org_unit_id: Option<Uuid>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = work_orders)]
pub struct NewWorkOrder {
    pub id: Uuid,
    pub member_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub priority: String,
    pub status: String,
    pub assigned_to: Option<Uuid>,
    pub created_by: Uuid,
    pub due_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub ticket_type: Option<String>,
    pub processing_notes: Option<String>,
    pub routed_to_org_unit_id: Option<Uuid>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
}

/// Applied on every state transition.
///
/// `None` fields are skipped (not written to the DB).
/// For `assigned_to`: `Some(None)` → SET NULL, `Some(Some(id))` → SET id.
#[derive(Debug, AsChangeset)]
#[diesel(table_name = work_orders)]
pub struct WorkOrderChangeset {
    pub status: String,
    /// `Some(None)` clears the assignment; `Some(Some(id))` sets it.
    pub assigned_to: Option<Option<Uuid>>,
    pub processing_notes: Option<String>,
    pub routed_to_org_unit_id: Option<Uuid>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

// ── Requests ──────────────────────────────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct CreateWorkOrderRequest {
    pub member_id: Uuid,
    #[validate(length(min = 1, max = 300))]
    pub title: String,
    #[validate(length(max = 2000))]
    pub description: Option<String>,
    /// low | medium | high | urgent  (default: medium)
    pub priority: Option<String>,
    /// health_query | equipment | scheduling | nutrition | emergency
    pub ticket_type: Option<String>,
    pub due_date: Option<NaiveDate>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct TransitionRequest {
    /// Target status (see VALID_STATUSES).
    /// Accepts both "to_status" and "new_status" as field names.
    #[serde(alias = "new_status")]
    pub to_status: String,
    /// Appended to processing history with a timestamp header.
    #[validate(length(max = 2000))]
    pub processing_notes: Option<String>,
    /// Override auto-assigned user. Admin only.
    pub assigned_to: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub status: Option<String>,
    pub priority: Option<String>,
    pub ticket_type: Option<String>,
    /// Admin-only filter.
    pub member_id: Option<Uuid>,
}

// ── Response ──────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct WorkOrderResponse {
    pub id: Uuid,
    pub member_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub priority: String,
    pub status: String,
    pub ticket_type: Option<String>,
    pub processing_notes: Option<String>,
    pub assigned_to: Option<Uuid>,
    pub routed_to_org_unit_id: Option<Uuid>,
    pub created_by: Uuid,
    pub due_date: Option<NaiveDate>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<WorkOrder> for WorkOrderResponse {
    fn from(w: WorkOrder) -> Self {
        Self {
            id:                    w.id,
            member_id:             w.member_id,
            title:                 w.title,
            description:           w.description,
            priority:              w.priority,
            status:                w.status,
            ticket_type:           w.ticket_type,
            processing_notes:      w.processing_notes,
            assigned_to:           w.assigned_to,
            routed_to_org_unit_id: w.routed_to_org_unit_id,
            created_by:            w.created_by,
            due_date:              w.due_date,
            resolved_at:           w.resolved_at,
            closed_at:             w.closed_at,
            created_at:            w.created_at,
            updated_at:            w.updated_at,
        }
    }
}

// ── State machine ─────────────────────────────────────────────

/// Allowed transitions:
/// ```text
/// intake            → triage, closed
/// triage            → in_progress, closed
/// in_progress       → waiting_on_member, resolved
/// waiting_on_member → in_progress, closed
/// resolved          → closed
/// closed            → (terminal)
/// ```
pub fn guard_transition(from: &str, to: &str) -> Result<(), AppError> {
    let allowed: &[&str] = match from {
        "intake"            => &["triage", "closed"],
        "triage"            => &["in_progress", "closed"],
        "in_progress"       => &["waiting_on_member", "resolved"],
        "waiting_on_member" => &["in_progress", "closed"],
        "resolved"          => &["closed"],
        "closed"            => &[],
        _                   => {
            return Err(AppError::BadRequest(format!("unknown status '{}'", from)))
        }
    };

    if allowed.contains(&to) {
        Ok(())
    } else {
        Err(AppError::BadRequest(format!(
            "transition '{}' → '{}' is not allowed; valid targets from '{}': [{}]",
            from,
            to,
            from,
            if allowed.is_empty() { "none — terminal state".to_owned() } else { allowed.join(", ") },
        )))
    }
}

// ─────────────────────────────────────────────────────────────────
// Unit tests — exhaustive coverage of the work-order state machine.
//
// Every (from, to) pair documented in `guard_transition`'s docs is
// pinned so an accidental edit to the allow-list is loud.
// ─────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_transitions_succeed() {
        let allowed: &[(&str, &str)] = &[
            ("intake", "triage"),
            ("intake", "closed"),
            ("triage", "in_progress"),
            ("triage", "closed"),
            ("in_progress", "waiting_on_member"),
            ("in_progress", "resolved"),
            ("waiting_on_member", "in_progress"),
            ("waiting_on_member", "closed"),
            ("resolved", "closed"),
        ];
        for (from, to) in allowed {
            assert!(
                guard_transition(from, to).is_ok(),
                "expected {from} → {to} to be allowed"
            );
        }
    }

    #[test]
    fn disallowed_transitions_are_rejected() {
        let disallowed: &[(&str, &str)] = &[
            ("intake", "in_progress"),       // must go through triage
            ("intake", "resolved"),
            ("triage", "waiting_on_member"), // skipping in_progress
            ("triage", "resolved"),
            ("in_progress", "intake"),       // no going backwards
            ("in_progress", "closed"),       // must resolve first
            ("waiting_on_member", "resolved"),
            ("resolved", "in_progress"),     // resolved is one-way
            ("resolved", "intake"),
        ];
        for (from, to) in disallowed {
            let err = guard_transition(from, to).unwrap_err();
            assert!(matches!(err, AppError::BadRequest(_)), "for {from}→{to}");
            let msg = err.to_string();
            assert!(msg.contains("not allowed"));
        }
    }

    #[test]
    fn closed_is_terminal() {
        // closed has zero allowed targets — every attempt fails.
        for to in &["intake", "triage", "in_progress", "waiting_on_member", "resolved", "closed"] {
            let err = guard_transition("closed", to).unwrap_err();
            assert!(err.to_string().contains("terminal"));
        }
    }

    #[test]
    fn unknown_from_status_is_bad_request() {
        let err = guard_transition("zombie", "closed").unwrap_err();
        match err {
            AppError::BadRequest(msg) => assert!(msg.contains("unknown status")),
            other => panic!("expected BadRequest, got {:?}", other),
        }
    }

    #[test]
    fn invalid_to_status_is_bad_request_with_targets_listed() {
        let err = guard_transition("intake", "invented").unwrap_err();
        match err {
            AppError::BadRequest(msg) => {
                assert!(msg.contains("intake"));
                assert!(msg.contains("invented"));
                assert!(msg.contains("triage"));   // valid target
                assert!(msg.contains("closed"));   // valid target
            }
            other => panic!("expected BadRequest, got {:?}", other),
        }
    }

    #[test]
    fn no_self_transitions_allowed() {
        for status in VALID_STATUSES {
            assert!(
                guard_transition(status, status).is_err(),
                "self-transition {status}→{status} should be rejected"
            );
        }
    }

    #[test]
    fn constants_match_state_machine_universe() {
        // VALID_STATUSES must be the same set as the from-states the machine knows.
        for status in &["intake", "triage", "in_progress", "waiting_on_member", "resolved", "closed"] {
            assert!(VALID_STATUSES.contains(status), "missing status {status}");
        }
    }
}
