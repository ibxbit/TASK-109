use actix_web::{get, post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Datelike, Duration, Timelike, Utc, Weekday};
use diesel::prelude::*;
use uuid::Uuid;
use validator::Validate;

use crate::{
    auth::role::Role,
    db::DbPool,
    errors::AppError,
    middleware::auth::{AdminAuth, ApproverAuth, AuthenticatedUser},
    models::{
        audit_log::{self, NewAuditLog},
        workflow::{
            AddNodeRequest, Approval, ApprovalResponse, CreateTemplateRequest,
            NewApproval, NewWorkflowInstance, NewWorkflowNode, NewWorkflowTemplate,
            NodeResponse, StartWorkflowRequest, TemplateResponse, VALID_ACTION_TYPES,
            VALID_AMOUNT_TIERS, VALID_RISK_TIERS, VALID_WORKFLOW_ACTIONS,
            WorkflowActionRequest, WorkflowInstance, WorkflowInstanceResponse,
            WorkflowNode, WorkflowTemplate,
        },
    },
    schema::{approvals, notifications, workflow_instances, workflow_nodes, workflow_templates},
};

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/workflows")
            .service(create_template)
            .service(add_node)
            .service(start_workflow)
            .service(take_action)
            .service(get_instance),
    );
}

// ── SLA helpers ───────────────────────────────────────────────

/// Add `business_hours` business hours to `start`.
/// Business hours = Mon–Fri, 09:00–17:00 UTC.
fn add_business_hours(start: DateTime<Utc>, business_hours: i64) -> DateTime<Utc> {
    let mut remaining = business_hours;
    let mut current = start;
    while remaining > 0 {
        current += Duration::hours(1);
        let wd = current.weekday();
        let h  = current.hour();
        if wd != Weekday::Sat && wd != Weekday::Sun && h >= 9 && h < 17 {
            remaining -= 1;
        }
    }
    current
}

/// Mark pending approvals whose SLA deadline has passed and create
/// an in-app notification for each breach.
fn check_sla(conn: &mut PgConnection, instance_id: Uuid) -> Result<(), AppError> {
    let now = Utc::now();

    let breached: Vec<Approval> = approvals::table
        .filter(approvals::workflow_instance_id.eq(instance_id))
        .filter(approvals::status.eq("pending"))
        .filter(approvals::sla_breached.eq(false))
        .filter(approvals::sla_deadline.lt(now))
        .select(Approval::as_select())
        .load(conn)
        .map_err(AppError::Database)?;

    for appr in &breached {
        // Mark the approval as SLA-breached
        diesel::update(approvals::table.find(appr.id))
            .set(approvals::sla_breached.eq(true))
            .execute(conn)
            .map_err(AppError::Database)?;

        // Determine who to notify: assignee if set, else the instance initiator
        let notify_user_id = appr.assignee_id.unwrap_or_else(|| {
            workflow_instances::table
                .find(instance_id)
                .select(workflow_instances::initiated_by)
                .first::<Uuid>(conn)
                .unwrap_or(appr.approver_id)
        });

        // Create in-app notification
        use crate::schema::notifications;
        let _ = diesel::insert_into(notifications::table)
            .values((
                notifications::id.eq(Uuid::new_v4()),
                notifications::user_id.eq(notify_user_id),
                notifications::title.eq("SLA Breach — Approval Required"),
                notifications::body.eq(format!(
                    "Workflow {} approval (node {}) exceeded the 48-business-hour SLA.",
                    instance_id, appr.node_id
                )),
                notifications::is_read.eq(false),
                notifications::created_at.eq(now),
            ))
            .execute(conn);

        audit_log::insert(
            conn,
            NewAuditLog::new(
                None,
                "SLA_BREACHED",
                "approval",
                Some(appr.id),
                None,
            )
            .with_new_value(serde_json::json!({
                "workflow_instance_id": instance_id,
                "node_id": appr.node_id,
                "sla_deadline": appr.sla_deadline,
            })),
        );
    }
    Ok(())
}

// ── Stage management ──────────────────────────────────────────

/// Create one approval row per node at `stage` for `instance_id`.
/// The SLA clock starts immediately.
fn create_stage_approvals(
    conn: &mut PgConnection,
    instance_id: Uuid,
    instance_initiator: Uuid,
    template_id: Uuid,
    stage: i32,
) -> Result<(), AppError> {
    let now  = Utc::now();
    let deadline = add_business_hours(now, 48);

    let nodes: Vec<WorkflowNode> = workflow_nodes::table
        .filter(workflow_nodes::template_id.eq(template_id))
        .filter(workflow_nodes::node_order.eq(stage))
        .select(WorkflowNode::as_select())
        .load(conn)
        .map_err(AppError::Database)?;

    for node in nodes {
        diesel::insert_into(approvals::table)
            .values(NewApproval {
                id: Uuid::new_v4(),
                workflow_instance_id: instance_id,
                node_id: node.id,
                approver_id: instance_initiator,
                status: "pending".to_owned(),
                comments: None,
                decided_at: None,
                created_at: now,
                sla_deadline: Some(deadline),
                sla_breached: false,
                assignee_id: None,
                note: None,
            })
            .execute(conn)
            .map_err(AppError::Database)?;
    }
    Ok(())
}

/// After an approval is recorded: check if the current stage is fully
/// satisfied, then advance the instance or mark it completed.
fn try_advance_stage(
    conn: &mut PgConnection,
    instance: &WorkflowInstance,
) -> Result<(), AppError> {
    let stage = match instance.current_stage {
        Some(s) => s,
        None    => return Ok(()),
    };

    // Collect node IDs for this stage
    let node_ids: Vec<Uuid> = workflow_nodes::table
        .filter(workflow_nodes::template_id.eq(instance.template_id))
        .filter(workflow_nodes::node_order.eq(stage))
        .select(workflow_nodes::id)
        .load(conn)
        .map_err(AppError::Database)?;

    // Count still-pending approvals for this stage
    let pending: i64 = approvals::table
        .filter(approvals::workflow_instance_id.eq(instance.id))
        .filter(approvals::node_id.eq_any(&node_ids))
        .filter(approvals::status.eq("pending"))
        .count()
        .get_result(conn)
        .map_err(AppError::Database)?;

    if pending > 0 {
        return Ok(()); // Stage not yet complete
    }

    let now = Utc::now();

    // Find the next stage (smallest node_order greater than current)
    let next: Option<i32> = workflow_nodes::table
        .filter(workflow_nodes::template_id.eq(instance.template_id))
        .filter(workflow_nodes::node_order.gt(stage))
        .order(workflow_nodes::node_order.asc())
        .select(workflow_nodes::node_order)
        .first(conn)
        .optional()
        .map_err(AppError::Database)?;

    match next {
        None => {
            // All stages done — complete
            diesel::update(workflow_instances::table.find(instance.id))
                .set((
                    workflow_instances::status.eq("completed"),
                    workflow_instances::completed_at.eq(Some(now)),
                    workflow_instances::updated_at.eq(now),
                ))
                .execute(conn)
                .map_err(AppError::Database)?;
        }
        Some(next_stage) => {
            diesel::update(workflow_instances::table.find(instance.id))
                .set((
                    workflow_instances::current_stage.eq(Some(next_stage)),
                    workflow_instances::updated_at.eq(now),
                ))
                .execute(conn)
                .map_err(AppError::Database)?;

            create_stage_approvals(
                conn,
                instance.id,
                instance.initiated_by,
                instance.template_id,
                next_stage,
            )?;
        }
    }
    Ok(())
}

// ── Actor → approval matching ─────────────────────────────────

/// Find the pending approval at the instance's current stage that
/// the calling actor is entitled to act on.
///
/// Priority: explicitly assigned (`assignee_id == actor`) > role match.
/// Admin can act on any pending approval.
fn find_actor_approval(
    conn: &mut PgConnection,
    instance: &WorkflowInstance,
    actor_id: Uuid,
    actor_role_id: Uuid,
    is_admin: bool,
) -> Result<Approval, AppError> {
    let stage = instance
        .current_stage
        .ok_or_else(|| AppError::BadRequest("Instance has no active stage".to_owned()))?;

    let node_ids: Vec<Uuid> = workflow_nodes::table
        .filter(workflow_nodes::template_id.eq(instance.template_id))
        .filter(workflow_nodes::node_order.eq(stage))
        .select(workflow_nodes::id)
        .load(conn)
        .map_err(AppError::Database)?;

    let pending: Vec<Approval> = approvals::table
        .filter(approvals::workflow_instance_id.eq(instance.id))
        .filter(approvals::node_id.eq_any(&node_ids))
        .filter(approvals::status.eq("pending"))
        .select(Approval::as_select())
        .load(conn)
        .map_err(AppError::Database)?;

    if pending.is_empty() {
        return Err(AppError::BadRequest(
            "No pending approvals at the current stage".to_owned(),
        ));
    }

    if is_admin {
        return Ok(pending.into_iter().next().unwrap());
    }

    // Explicit assignment takes priority
    if let Some(mine) = pending.iter().find(|a| a.assignee_id == Some(actor_id)) {
        return Ok(mine.clone());
    }

    // Role match (unassigned node whose required role matches actor's role)
    let role_match = pending.iter().find(|a| {
        if a.assignee_id.is_some() {
            return false;
        }
        let required: Option<Option<Uuid>> = workflow_nodes::table
            .find(a.node_id)
            .select(workflow_nodes::role_required)
            .first(conn)
            .optional()
            .ok()
            .flatten();
        match required {
            Some(Some(req_role)) => req_role == actor_role_id,
            Some(None) => true, // no role restriction
            None => false,
        }
    });

    role_match
        .cloned()
        .ok_or(AppError::Forbidden)
}

// ── State-machine guard ───────────────────────────────────────

fn guard_transition(
    instance_status: &str,
    action: &str,
    has_prior_approvals: bool,
) -> Result<(), AppError> {
    let ok = match (instance_status, action) {
        ("in_progress", "approve")           => true,
        ("in_progress", "reject")            => true,
        ("in_progress", "return_for_edit")   => true,
        ("in_progress", "reassign")          => true,
        ("in_progress", "additional_sign_off") => true,
        ("in_progress", "withdraw")          => !has_prior_approvals,
        ("pending",     "withdraw")          => true,
        ("returned",    "submit")            => true,
        _ => false,
    };
    if !ok {
        Err(AppError::BadRequest(format!(
            "Action '{}' is not permitted when instance is '{}'", action, instance_status
        )))
    } else {
        Ok(())
    }
}

// ── Build response ────────────────────────────────────────────

fn build_instance_response(
    conn: &mut PgConnection,
    instance: WorkflowInstance,
) -> Result<WorkflowInstanceResponse, AppError> {
    let template: WorkflowTemplate = workflow_templates::table
        .find(instance.template_id)
        .select(WorkflowTemplate::as_select())
        .first(conn)
        .map_err(AppError::Database)?;

    // All approvals for this instance, joined with node info
    let appr_rows: Vec<(Approval, WorkflowNode)> = approvals::table
        .inner_join(workflow_nodes::table)
        .filter(approvals::workflow_instance_id.eq(instance.id))
        .order(approvals::created_at.asc())
        .select((Approval::as_select(), WorkflowNode::as_select()))
        .load(conn)
        .map_err(AppError::Database)?;

    let appr_responses: Vec<ApprovalResponse> = appr_rows
        .into_iter()
        .map(|(a, n)| ApprovalResponse {
            id: a.id,
            node_id: a.node_id,
            node_name: n.name,
            node_order: n.node_order,
            status: a.status,
            assignee_id: a.assignee_id,
            comments: a.comments,
            note: a.note,
            sla_deadline: a.sla_deadline,
            sla_breached: a.sla_breached,
            decided_at: a.decided_at,
            created_at: a.created_at,
        })
        .collect();

    Ok(WorkflowInstanceResponse {
        id: instance.id,
        template_id: instance.template_id,
        template_name: template.name,
        work_order_id: instance.work_order_id,
        status: instance.status,
        current_stage: instance.current_stage,
        initiated_by: instance.initiated_by,
        submitted_at: instance.submitted_at,
        completed_at: instance.completed_at,
        created_at: instance.created_at,
        updated_at: instance.updated_at,
        approvals: appr_responses,
    })
}

// ══════════════════════════════════════════════════════════════
// Handlers
// ══════════════════════════════════════════════════════════════

// ── POST /workflows/templates  (Admin only) ───────────────────

#[post("/templates")]
async fn create_template(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    auth: AdminAuth,
    body: web::Json<CreateTemplateRequest>,
) -> Result<HttpResponse, AppError> {
    body.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    if let Some(ref rt) = body.risk_tier {
        if !VALID_RISK_TIERS.contains(&rt.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Invalid risk_tier '{}'. Valid: low, medium, high, critical", rt
            )));
        }
    }
    if let Some(ref at) = body.amount_tier {
        if !VALID_AMOUNT_TIERS.contains(&at.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Invalid amount_tier '{}'. Valid: under_1k, 1k_10k, 10k_100k, over_100k", at
            )));
        }
    }

    let actor_id    = auth.user_id;
    let ip          = req.connection_info().realip_remote_addr().map(str::to_owned);
    let name        = body.name.clone();
    let description = body.description.clone();
    let business    = body.business_type.clone();
    let org_id      = body.org_unit_id;
    let risk        = body.risk_tier.clone();
    let amount      = body.amount_tier.clone();

    let tpl = web::block(move || -> Result<WorkflowTemplate, AppError> {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
        let now = Utc::now();

        // Unique name check
        let exists: bool = diesel::select(diesel::dsl::exists(
            workflow_templates::table.filter(workflow_templates::name.eq(&name)),
        ))
        .get_result(&mut conn)
        .map_err(AppError::Database)?;
        if exists {
            return Err(AppError::Conflict(format!("Template '{}' already exists", name)));
        }

        let row: WorkflowTemplate = diesel::insert_into(workflow_templates::table)
            .values(NewWorkflowTemplate {
                id: Uuid::new_v4(), name: name.clone(),
                description, is_active: true,
                created_by: actor_id,
                created_at: now, updated_at: now,
                business_type: business,
                org_unit_id: org_id,
                risk_tier: risk,
                amount_tier: amount,
            })
            .get_result(&mut conn)
            .map_err(AppError::Database)?;

        audit_log::insert(&mut conn, NewAuditLog::new(
            Some(actor_id), "WORKFLOW_TEMPLATE_CREATED", "workflow_template",
            Some(row.id), ip,
        ).with_new_value(serde_json::json!({ "name": &name })));

        Ok(row)
    })
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("Thread pool error")))??;

    let response = TemplateResponse {
        id: tpl.id, name: tpl.name, description: tpl.description,
        is_active: tpl.is_active, business_type: tpl.business_type,
        org_unit_id: tpl.org_unit_id, risk_tier: tpl.risk_tier,
        amount_tier: tpl.amount_tier,
        created_by: tpl.created_by, created_at: tpl.created_at, nodes: vec![],
    };
    Ok(HttpResponse::Created().json(response))
}

// ── POST /workflows/templates/{id}/nodes  (Admin only) ────────

#[post("/templates/{id}/nodes")]
async fn add_node(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    auth: AdminAuth,
    path: web::Path<Uuid>,
    body: web::Json<AddNodeRequest>,
) -> Result<HttpResponse, AppError> {
    body.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    if !VALID_ACTION_TYPES.contains(&body.action_type.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Invalid action_type '{}'. Valid: review, approve, notify, complete",
            body.action_type
        )));
    }
    if body.node_order < 1 {
        return Err(AppError::BadRequest("node_order must be >= 1".to_owned()));
    }

    let template_id  = path.into_inner();
    let actor_id     = auth.user_id;
    let ip           = req.connection_info().realip_remote_addr().map(str::to_owned);
    let name         = body.name.clone();
    let node_order   = body.node_order;
    let is_parallel  = body.is_parallel;
    let role_req     = body.role_required;
    let action_type  = body.action_type.clone();

    let node = web::block(move || -> Result<WorkflowNode, AppError> {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        // Template must exist and be active
        let tpl_active: bool = workflow_templates::table
            .filter(workflow_templates::id.eq(template_id))
            .filter(workflow_templates::is_active.eq(true))
            .count()
            .get_result::<i64>(&mut conn)
            .map_err(AppError::Database)? > 0;
        if !tpl_active {
            return Err(AppError::NotFound(format!("Template {} not found", template_id)));
        }

        let now = Utc::now();
        let row: WorkflowNode = diesel::insert_into(workflow_nodes::table)
            .values(NewWorkflowNode {
                id: Uuid::new_v4(),
                template_id, name: name.clone(),
                node_order, role_required: role_req,
                action_type: action_type.clone(),
                created_at: now, is_parallel,
            })
            .get_result(&mut conn)
            .map_err(AppError::Database)?;

        audit_log::insert(&mut conn, NewAuditLog::new(
            Some(actor_id), "WORKFLOW_NODE_ADDED", "workflow_node",
            Some(row.id), ip,
        ).with_new_value(serde_json::json!({
            "template_id": template_id,
            "node_order": node_order,
            "is_parallel": is_parallel,
        })));

        Ok(row)
    })
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("Thread pool error")))??;

    Ok(HttpResponse::Created().json(NodeResponse::from(node)))
}

// ── POST /workflows/instances  (Approver or above) ───────────

#[post("/instances")]
async fn start_workflow(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    auth: ApproverAuth,
    body: web::Json<StartWorkflowRequest>,
) -> Result<HttpResponse, AppError> {
    let actor_id    = auth.user_id;
    let ip          = req.connection_info().realip_remote_addr().map(str::to_owned);
    let template_id = body.template_id;
    let work_order  = body.work_order_id;

    let instance = web::block(move || -> Result<WorkflowInstance, AppError> {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        // Template must be active
        let _tpl: WorkflowTemplate = workflow_templates::table
            .filter(workflow_templates::id.eq(template_id))
            .filter(workflow_templates::is_active.eq(true))
            .select(WorkflowTemplate::as_select())
            .first(&mut conn)
            .optional()
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound(format!("Template {} not found", template_id)))?;

        // Template must have at least one node
        let first_stage: Option<i32> = workflow_nodes::table
            .filter(workflow_nodes::template_id.eq(template_id))
            .order(workflow_nodes::node_order.asc())
            .select(workflow_nodes::node_order)
            .first(&mut conn)
            .optional()
            .map_err(AppError::Database)?;

        let first_stage = first_stage.ok_or_else(|| {
            AppError::BadRequest("Template has no nodes — add nodes before starting".to_owned())
        })?;

        let now = Utc::now();
        let instance: WorkflowInstance = diesel::insert_into(workflow_instances::table)
            .values(NewWorkflowInstance {
                id: Uuid::new_v4(),
                template_id,
                work_order_id: work_order,
                current_node_id: None,
                status: "in_progress".to_owned(),
                initiated_by: actor_id,
                created_at: now,
                updated_at: now,
                current_stage: Some(first_stage),
                submitted_at: Some(now),
                completed_at: None,
            })
            .get_result(&mut conn)
            .map_err(AppError::Database)?;

        // Create approvals for the first stage (SLA clock starts now)
        create_stage_approvals(&mut conn, instance.id, actor_id, template_id, first_stage)?;

        audit_log::insert(&mut conn, NewAuditLog::new(
            Some(actor_id), "WORKFLOW_STARTED", "workflow_instance",
            Some(instance.id), ip,
        ).with_new_value(serde_json::json!({
            "template_id": template_id,
            "first_stage": first_stage,
        })));

        Ok(instance)
    })
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("Thread pool error")))??;

    Ok(HttpResponse::Created().json(serde_json::json!({
        "id":            instance.id,
        "template_id":   instance.template_id,
        "status":        instance.status,
        "current_stage": instance.current_stage,
        "submitted_at":  instance.submitted_at,
    })))
}

// ── POST /workflows/instances/{id}/actions ────────────────────

#[post("/instances/{id}/actions")]
async fn take_action(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    path: web::Path<Uuid>,
    body: web::Json<WorkflowActionRequest>,
) -> Result<HttpResponse, AppError> {
    // Only Approvers and above (+ Admins) may take workflow actions
    auth.require_approver_or_above()?;

    body.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    if !VALID_WORKFLOW_ACTIONS.contains(&body.action.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Invalid action '{}'. Valid: {}",
            body.action,
            VALID_WORKFLOW_ACTIONS.join(", ")
        )));
    }

    let instance_id    = path.into_inner();
    let actor_id       = auth.user_id;
    let ip             = req.connection_info().realip_remote_addr().map(str::to_owned);
    let actor_role_id  = auth.role_id;
    let is_admin       = auth.role.is_admin();
    let action         = body.action.clone();
    let comment        = body.comment.clone();
    let new_assignee   = body.new_assignee_id;
    let extra_approver = body.additional_approver_id;

    let updated = web::block(move || -> Result<WorkflowInstance, AppError> {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        // Load instance
        let instance: WorkflowInstance = workflow_instances::table
            .find(instance_id)
            .select(WorkflowInstance::as_select())
            .first(&mut conn)
            .optional()
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound(format!("Workflow instance {} not found", instance_id)))?;

        // Check SLA on pending approvals before processing action
        check_sla(&mut conn, instance_id)?;

        // Has any prior approval been recorded?
        let prior_approvals: i64 = approvals::table
            .filter(approvals::workflow_instance_id.eq(instance_id))
            .filter(approvals::status.eq("approved"))
            .count()
            .get_result(&mut conn)
            .map_err(AppError::Database)?;

        // Guard state-machine transition
        guard_transition(&instance.status, &action, prior_approvals > 0)?;

        let now = Utc::now();

        match action.as_str() {
            // ── submit (re-submit a returned workflow) ────────
            "submit" => {
                let stage = instance.current_stage.unwrap_or(1);
                diesel::update(workflow_instances::table.find(instance_id))
                    .set((
                        workflow_instances::status.eq("in_progress"),
                        workflow_instances::submitted_at.eq(Some(now)),
                        workflow_instances::updated_at.eq(now),
                    ))
                    .execute(&mut conn)
                    .map_err(AppError::Database)?;

                create_stage_approvals(
                    &mut conn, instance_id, actor_id,
                    instance.template_id, stage,
                )?;

                audit_log::insert(&mut conn, NewAuditLog::new(
                    Some(actor_id), "WORKFLOW_RESUBMITTED", "workflow_instance",
                    Some(instance_id), ip.clone(),
                ));
            }

            // ── approve ───────────────────────────────────────
            "approve" => {
                let appr = find_actor_approval(
                    &mut conn, &instance, actor_id, actor_role_id, is_admin,
                )?;

                diesel::update(approvals::table.find(appr.id))
                    .set((
                        approvals::status.eq("approved"),
                        approvals::comments.eq(comment.as_deref()),
                        approvals::decided_at.eq(Some(now)),
                    ))
                    .execute(&mut conn)
                    .map_err(AppError::Database)?;

                // Reload fresh instance for stage-advance check
                let fresh: WorkflowInstance = workflow_instances::table
                    .find(instance_id)
                    .select(WorkflowInstance::as_select())
                    .first(&mut conn)
                    .map_err(AppError::Database)?;

                try_advance_stage(&mut conn, &fresh)?;

                audit_log::insert(&mut conn, NewAuditLog::new(
                    Some(actor_id), "APPROVAL_APPROVED", "approval",
                    Some(appr.id), ip.clone(),
                ).with_new_value(serde_json::json!({
                    "workflow_instance_id": instance_id,
                    "node_id": appr.node_id,
                })));
            }

            // ── reject ────────────────────────────────────────
            "reject" => {
                let appr = find_actor_approval(
                    &mut conn, &instance, actor_id, actor_role_id, is_admin,
                )?;

                diesel::update(approvals::table.find(appr.id))
                    .set((
                        approvals::status.eq("rejected"),
                        approvals::comments.eq(comment.as_deref()),
                        approvals::decided_at.eq(Some(now)),
                    ))
                    .execute(&mut conn)
                    .map_err(AppError::Database)?;

                diesel::update(workflow_instances::table.find(instance_id))
                    .set((
                        workflow_instances::status.eq("rejected"),
                        workflow_instances::updated_at.eq(now),
                    ))
                    .execute(&mut conn)
                    .map_err(AppError::Database)?;

                audit_log::insert(&mut conn, NewAuditLog::new(
                    Some(actor_id), "APPROVAL_REJECTED", "approval",
                    Some(appr.id), ip.clone(),
                ).with_new_value(serde_json::json!({
                    "workflow_instance_id": instance_id,
                    "reason": comment,
                })));
            }

            // ── return_for_edit ───────────────────────────────
            "return_for_edit" => {
                let appr = find_actor_approval(
                    &mut conn, &instance, actor_id, actor_role_id, is_admin,
                )?;

                diesel::update(approvals::table.find(appr.id))
                    .set((
                        approvals::status.eq("returned"),
                        approvals::note.eq(comment.as_deref()),
                        approvals::decided_at.eq(Some(now)),
                    ))
                    .execute(&mut conn)
                    .map_err(AppError::Database)?;

                diesel::update(workflow_instances::table.find(instance_id))
                    .set((
                        workflow_instances::status.eq("returned"),
                        workflow_instances::updated_at.eq(now),
                    ))
                    .execute(&mut conn)
                    .map_err(AppError::Database)?;

                audit_log::insert(&mut conn, NewAuditLog::new(
                    Some(actor_id), "APPROVAL_RETURNED_FOR_EDIT", "approval",
                    Some(appr.id), ip.clone(),
                ).with_new_value(serde_json::json!({ "reason": comment })));
            }

            // ── withdraw ──────────────────────────────────────
            "withdraw" => {
                // Only the initiator (or an admin) may withdraw their own workflow.
                if !is_admin && actor_id != instance.initiated_by {
                    return Err(AppError::Forbidden);
                }

                // Mark all pending approvals cancelled
                diesel::update(
                    approvals::table
                        .filter(approvals::workflow_instance_id.eq(instance_id))
                        .filter(approvals::status.eq("pending")),
                )
                .set((
                    approvals::status.eq("reassigned"), // reuse a terminal-ish status
                    approvals::note.eq("Withdrawn by initiator"),
                    approvals::decided_at.eq(Some(now)),
                ))
                .execute(&mut conn)
                .map_err(AppError::Database)?;

                diesel::update(workflow_instances::table.find(instance_id))
                    .set((
                        workflow_instances::status.eq("withdrawn"),
                        workflow_instances::updated_at.eq(now),
                    ))
                    .execute(&mut conn)
                    .map_err(AppError::Database)?;

                audit_log::insert(&mut conn, NewAuditLog::new(
                    Some(actor_id), "WORKFLOW_WITHDRAWN", "workflow_instance",
                    Some(instance_id), ip.clone(),
                ));
            }

            // ── reassign ──────────────────────────────────────
            "reassign" => {
                let new_assignee = new_assignee.ok_or_else(|| {
                    AppError::BadRequest("new_assignee_id is required for reassign".to_owned())
                })?;

                let appr = find_actor_approval(
                    &mut conn, &instance, actor_id, actor_role_id, is_admin,
                )?;

                // Mark old approval as reassigned
                diesel::update(approvals::table.find(appr.id))
                    .set((
                        approvals::status.eq("reassigned"),
                        approvals::note.eq(comment.as_deref()),
                        approvals::decided_at.eq(Some(now)),
                    ))
                    .execute(&mut conn)
                    .map_err(AppError::Database)?;

                // Create a new pending approval for the new assignee
                let deadline = add_business_hours(now, 48);
                diesel::insert_into(approvals::table)
                    .values(NewApproval {
                        id: Uuid::new_v4(),
                        workflow_instance_id: instance_id,
                        node_id: appr.node_id,
                        approver_id: actor_id,
                        status: "pending".to_owned(),
                        comments: None,
                        decided_at: None,
                        created_at: now,
                        sla_deadline: Some(deadline),
                        sla_breached: false,
                        assignee_id: Some(new_assignee),
                        note: None,
                    })
                    .execute(&mut conn)
                    .map_err(AppError::Database)?;

                audit_log::insert(&mut conn, NewAuditLog::new(
                    Some(actor_id), "APPROVAL_REASSIGNED", "approval",
                    Some(appr.id), ip.clone(),
                ).with_new_value(serde_json::json!({
                    "from_user": actor_id,
                    "to_user": new_assignee,
                    "reason": comment,
                })));
            }

            // ── additional_sign_off ───────────────────────────
            "additional_sign_off" => {
                let extra = extra_approver.ok_or_else(|| {
                    AppError::BadRequest(
                        "additional_approver_id is required for additional_sign_off".to_owned(),
                    )
                })?;

                let stage = instance.current_stage.ok_or_else(|| {
                    AppError::BadRequest("Instance has no active stage".to_owned())
                })?;

                // Find the first node at current stage to attach the extra approval
                let node_id: Uuid = workflow_nodes::table
                    .filter(workflow_nodes::template_id.eq(instance.template_id))
                    .filter(workflow_nodes::node_order.eq(stage))
                    .select(workflow_nodes::id)
                    .first(&mut conn)
                    .map_err(AppError::Database)?;

                let deadline = add_business_hours(now, 48);
                diesel::insert_into(approvals::table)
                    .values(NewApproval {
                        id: Uuid::new_v4(),
                        workflow_instance_id: instance_id,
                        node_id,
                        approver_id: actor_id,
                        status: "additional_sign_off".to_owned(), // marks it as extra
                        comments: comment.clone(),
                        decided_at: None,
                        created_at: now,
                        sla_deadline: Some(deadline),
                        sla_breached: false,
                        assignee_id: Some(extra),
                        note: Some("Additional sign-off requested".to_owned()),
                    })
                    .execute(&mut conn)
                    .map_err(AppError::Database)?;

                // Insert a second pending approval for the extra approver
                diesel::insert_into(approvals::table)
                    .values(NewApproval {
                        id: Uuid::new_v4(),
                        workflow_instance_id: instance_id,
                        node_id,
                        approver_id: actor_id,
                        status: "pending".to_owned(),
                        comments: None,
                        decided_at: None,
                        created_at: now,
                        sla_deadline: Some(deadline),
                        sla_breached: false,
                        assignee_id: Some(extra),
                        note: None,
                    })
                    .execute(&mut conn)
                    .map_err(AppError::Database)?;

                audit_log::insert(&mut conn, NewAuditLog::new(
                    Some(actor_id), "ADDITIONAL_SIGN_OFF_REQUESTED", "workflow_instance",
                    Some(instance_id), ip.clone(),
                ).with_new_value(serde_json::json!({
                    "extra_approver": extra,
                    "reason": comment,
                })));
            }

            _ => unreachable!(), // guard_transition already validated the action
        }

        // Reload and return
        workflow_instances::table
            .find(instance_id)
            .select(WorkflowInstance::as_select())
            .first(&mut conn)
            .map_err(AppError::Database)
    })
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("Thread pool error")))??;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "id":           updated.id,
        "status":       updated.status,
        "current_stage": updated.current_stage,
        "completed_at": updated.completed_at,
    })))
}

// ── GET /workflows/instances/{id} ─────────────────────────────

#[get("/instances/{id}")]
async fn get_instance(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    // Approver/Admin can see any instance; others are excluded
    auth.require_approver_or_above()?;

    let instance_id = path.into_inner();

    let response = web::block(move || -> Result<WorkflowInstanceResponse, AppError> {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        // SLA check on every read
        check_sla(&mut conn, instance_id)?;

        let instance: WorkflowInstance = workflow_instances::table
            .find(instance_id)
            .select(WorkflowInstance::as_select())
            .first(&mut conn)
            .optional()
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound(format!("Instance {} not found", instance_id)))?;

        build_instance_response(&mut conn, instance)
    })
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("Thread pool error")))??;

    Ok(HttpResponse::Ok().json(response))
}
