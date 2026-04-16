use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::schema::{approvals, workflow_instances, workflow_nodes, workflow_templates};

// ── Allowed values ────────────────────────────────────────────

pub const VALID_ACTION_TYPES: &[&str]   = &["review", "approve", "notify", "complete"];
pub const VALID_RISK_TIERS: &[&str]     = &["low", "medium", "high", "critical"];
pub const VALID_AMOUNT_TIERS: &[&str]   = &["under_1k", "1k_10k", "10k_100k", "over_100k"];
pub const VALID_WORKFLOW_ACTIONS: &[&str] = &[
    "submit", "approve", "reject",
    "return_for_edit", "withdraw", "reassign", "additional_sign_off",
];

// ── DB models ─────────────────────────────────────────────────

#[derive(Debug, Clone, Queryable, Selectable, Identifiable)]
#[diesel(table_name = workflow_templates)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkflowTemplate {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub business_type: Option<String>,
    pub org_unit_id: Option<Uuid>,
    pub risk_tier: Option<String>,
    /// Financial approval threshold tier (e.g. "under_1k", "1k_10k", "10k_100k", "over_100k").
    pub amount_tier: Option<String>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = workflow_templates)]
pub struct NewWorkflowTemplate {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub business_type: Option<String>,
    pub org_unit_id: Option<Uuid>,
    pub risk_tier: Option<String>,
    pub amount_tier: Option<String>,
}

#[derive(Debug, Clone, Queryable, Selectable, Identifiable)]
#[diesel(table_name = workflow_nodes)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkflowNode {
    pub id: Uuid,
    pub template_id: Uuid,
    pub name: String,
    pub node_order: i32,
    pub role_required: Option<Uuid>,
    pub action_type: String,
    pub created_at: DateTime<Utc>,
    pub is_parallel: bool,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = workflow_nodes)]
pub struct NewWorkflowNode {
    pub id: Uuid,
    pub template_id: Uuid,
    pub name: String,
    pub node_order: i32,
    pub role_required: Option<Uuid>,
    pub action_type: String,
    pub created_at: DateTime<Utc>,
    pub is_parallel: bool,
}

#[derive(Debug, Clone, Queryable, Selectable, Identifiable)]
#[diesel(table_name = workflow_instances)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkflowInstance {
    pub id: Uuid,
    pub template_id: Uuid,
    pub work_order_id: Option<Uuid>,
    pub current_node_id: Option<Uuid>,
    pub status: String,
    pub initiated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub current_stage: Option<i32>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = workflow_instances)]
pub struct NewWorkflowInstance {
    pub id: Uuid,
    pub template_id: Uuid,
    pub work_order_id: Option<Uuid>,
    pub current_node_id: Option<Uuid>,
    pub status: String,
    pub initiated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub current_stage: Option<i32>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Queryable, Selectable, Identifiable)]
#[diesel(table_name = approvals)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Approval {
    pub id: Uuid,
    pub workflow_instance_id: Uuid,
    pub node_id: Uuid,
    pub approver_id: Uuid,
    pub status: String,
    pub comments: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub sla_deadline: Option<DateTime<Utc>>,
    pub sla_breached: bool,
    pub assignee_id: Option<Uuid>,
    pub note: Option<String>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = approvals)]
pub struct NewApproval {
    pub id: Uuid,
    pub workflow_instance_id: Uuid,
    pub node_id: Uuid,
    pub approver_id: Uuid,  // the user who created/initiated this approval slot
    pub status: String,
    pub comments: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub sla_deadline: Option<DateTime<Utc>>,
    pub sla_breached: bool,
    pub assignee_id: Option<Uuid>,
    pub note: Option<String>,
}

// ── API requests ──────────────────────────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct CreateTemplateRequest {
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    #[validate(length(max = 1000))]
    pub description: Option<String>,
    pub business_type: Option<String>,
    pub org_unit_id: Option<Uuid>,
    /// low | medium | high | critical
    pub risk_tier: Option<String>,
    /// under_1k | 1k_10k | 10k_100k | over_100k
    pub amount_tier: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AddNodeRequest {
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    /// Position in execution order; same value = parallel stage.
    pub node_order: i32,
    /// When true, this node shares its stage with other same-order nodes (parallel gate).
    pub is_parallel: bool,
    /// UUID of a role whose holders can act on this node.
    pub role_required: Option<Uuid>,
    /// review | approve | notify | complete
    pub action_type: String,
}

#[derive(Debug, Deserialize)]
pub struct StartWorkflowRequest {
    pub template_id: Uuid,
    pub work_order_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct WorkflowActionRequest {
    /// submit | approve | reject | return_for_edit | withdraw | reassign | additional_sign_off
    pub action: String,
    #[validate(length(max = 2000))]
    pub comment: Option<String>,
    /// Required for: reassign
    pub new_assignee_id: Option<Uuid>,
    /// Required for: additional_sign_off
    pub additional_approver_id: Option<Uuid>,
}

// ── API responses ─────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct TemplateResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub business_type: Option<String>,
    pub org_unit_id: Option<Uuid>,
    pub risk_tier: Option<String>,
    pub amount_tier: Option<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub nodes: Vec<NodeResponse>,
}

#[derive(Debug, Serialize)]
pub struct NodeResponse {
    pub id: Uuid,
    pub name: String,
    pub node_order: i32,
    pub is_parallel: bool,
    pub role_required: Option<Uuid>,
    pub action_type: String,
}

impl From<WorkflowNode> for NodeResponse {
    fn from(n: WorkflowNode) -> Self {
        Self {
            id: n.id,
            name: n.name,
            node_order: n.node_order,
            is_parallel: n.is_parallel,
            role_required: n.role_required,
            action_type: n.action_type,
        }
    }
}

// ─────────────────────────────────────────────────────────────────
// Unit tests — pinned constants and DTO validation.
// ─────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    #[test]
    fn action_types_pinned() {
        for a in ["review", "approve", "notify", "complete"] {
            assert!(VALID_ACTION_TYPES.contains(&a), "missing {a}");
        }
        assert_eq!(VALID_ACTION_TYPES.len(), 4);
    }

    #[test]
    fn risk_tiers_pinned() {
        for t in ["low", "medium", "high", "critical"] {
            assert!(VALID_RISK_TIERS.contains(&t), "missing risk tier {t}");
        }
    }

    #[test]
    fn amount_tiers_pinned() {
        for t in ["under_1k", "1k_10k", "10k_100k", "over_100k"] {
            assert!(VALID_AMOUNT_TIERS.contains(&t), "missing amount tier {t}");
        }
    }

    #[test]
    fn workflow_actions_pinned() {
        for a in [
            "submit", "approve", "reject",
            "return_for_edit", "withdraw", "reassign", "additional_sign_off",
        ] {
            assert!(VALID_WORKFLOW_ACTIONS.contains(&a), "missing action {a}");
        }
    }

    fn template_req(name: &str) -> CreateTemplateRequest {
        CreateTemplateRequest {
            name: name.into(),
            description: None,
            business_type: None,
            org_unit_id: None,
            risk_tier: None,
            amount_tier: None,
        }
    }

    #[test]
    fn create_template_request_accepts_valid_name() {
        assert!(template_req("My Template").validate().is_ok());
    }

    #[test]
    fn create_template_request_rejects_empty_name() {
        assert!(template_req("").validate().is_err());
    }

    #[test]
    fn create_template_request_rejects_oversized_name() {
        assert!(template_req(&"x".repeat(201)).validate().is_err());
    }

    #[test]
    fn add_node_request_accepts_valid_action_type_field() {
        let req = AddNodeRequest {
            name: "Node 1".into(),
            node_order: 1,
            is_parallel: false,
            role_required: None,
            action_type: "approve".into(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn workflow_action_request_validates_comment_length() {
        let mk = |comment: Option<String>| WorkflowActionRequest {
            action: "approve".into(),
            comment,
            new_assignee_id: None,
            additional_approver_id: None,
        };
        assert!(mk(None).validate().is_ok());
        assert!(mk(Some("ok".into())).validate().is_ok());
        assert!(mk(Some("x".repeat(2001))).validate().is_err());
    }
}

#[derive(Debug, Serialize)]
pub struct ApprovalResponse {
    pub id: Uuid,
    pub node_id: Uuid,
    pub node_name: String,
    pub node_order: i32,
    pub status: String,
    pub assignee_id: Option<Uuid>,
    pub comments: Option<String>,
    pub note: Option<String>,
    pub sla_deadline: Option<DateTime<Utc>>,
    pub sla_breached: bool,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct WorkflowInstanceResponse {
    pub id: Uuid,
    pub template_id: Uuid,
    pub template_name: String,
    pub work_order_id: Option<Uuid>,
    pub status: String,
    pub current_stage: Option<i32>,
    pub initiated_by: Uuid,
    pub submitted_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub approvals: Vec<ApprovalResponse>,
}
