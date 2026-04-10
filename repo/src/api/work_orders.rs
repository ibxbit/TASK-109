use actix_web::{get, patch, post, web, HttpRequest, HttpResponse};
use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;
use validator::Validate;

use crate::{
    auth::role::{Role, CARE_COACH_ID},
    db::DbPool,
    errors::AppError,
    middleware::auth::{AuthenticatedUser, CareCoachAuth},
    models::{
        audit_log::{self, NewAuditLog},
        work_order::{
            guard_transition, CreateWorkOrderRequest, ListQuery, NewWorkOrder,
            TransitionRequest, WorkOrder, WorkOrderChangeset, WorkOrderResponse,
            VALID_PRIORITIES, VALID_STATUSES, VALID_TICKET_TYPES,
        },
    },
    schema::{members, users, work_orders},
};

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/work-orders")
            .service(create_work_order)
            .service(transition_work_order)
            .service(list_work_orders),
    );
}

// ── Auto-routing ──────────────────────────────────────────────

/// Derives `(routed_to_org_unit_id, assigned_to)` for an intake→triage
/// transition.
///
/// Routing rules (designed for extension):
/// - All ticket types resolve to the member's own org_unit.
///   Future versions can pattern-match on `ticket_type` to direct
///   specialised types (e.g. `emergency`) to a different sub-unit.
/// - Within that org_unit, the first active care_coach is auto-assigned.
///   Returns `None` for `assigned_to` if none exists.
fn auto_route(
    conn: &mut PgConnection,
    member_id: Uuid,
    _ticket_type: Option<&str>,
) -> Result<(Option<Uuid>, Option<Uuid>), AppError> {
    // Resolve member → their org_unit (NOT NULL in schema)
    let org_unit_id: Uuid = members::table
        .find(member_id)
        .select(members::org_unit_id)
        .first(conn)
        .map_err(|_| AppError::NotFound("member record not found".into()))?;

    // Find first active care_coach in that org_unit
    let assigned_to: Option<Uuid> = users::table
        .filter(users::org_unit_id.eq(org_unit_id))
        .filter(users::role_id.eq(CARE_COACH_ID))
        .filter(users::is_active.eq(true))
        .select(users::id)
        .first::<Uuid>(conn)
        .optional()
        .map_err(AppError::Database)?;

    Ok((Some(org_unit_id), assigned_to))
}

// ── Note accumulation ─────────────────────────────────────────

/// Prepends a timestamped entry to the existing processing notes.
/// Entries are separated by newlines, newest at the bottom.
fn append_note(existing: Option<String>, note: &str, actor: &str, from: &str, to: &str) -> String {
    let entry = format!(
        "[{}] {} ({} → {}): {}",
        Utc::now().format("%Y-%m-%dT%H:%M:%SZ"),
        actor,
        from,
        to,
        note
    );
    match existing {
        None       => entry,
        Some(prev) => format!("{}\n{}", prev, entry),
    }
}

// ── Handlers ──────────────────────────────────────────────────

/// POST /work-orders
/// Creates a new work order in `intake` status.
/// Requires care_coach or administrator.
#[post("")]
async fn create_work_order(
    req: HttpRequest,
    user: CareCoachAuth,
    pool: web::Data<DbPool>,
    body: web::Json<CreateWorkOrderRequest>,
) -> Result<HttpResponse, AppError> {
    body.validate().map_err(|e| AppError::BadRequest(e.to_string()))?;

    if let Some(ref p) = body.priority {
        if !VALID_PRIORITIES.contains(&p.as_str()) {
            return Err(AppError::BadRequest(format!(
                "invalid priority '{}'; valid: {}",
                p,
                VALID_PRIORITIES.join(", ")
            )));
        }
    }
    if let Some(ref t) = body.ticket_type {
        if !VALID_TICKET_TYPES.contains(&t.as_str()) {
            return Err(AppError::BadRequest(format!(
                "invalid ticket_type '{}'; valid: {}",
                t,
                VALID_TICKET_TYPES.join(", ")
            )));
        }
    }

    let ip       = req.connection_info().realip_remote_addr().map(str::to_owned);
    let actor_id = user.user_id;
    let body     = body.into_inner();
    let now      = Utc::now();
    let id       = Uuid::new_v4();

    let wo = web::block(move || {
        let mut conn = pool.get().map_err(|_| AppError::Internal(anyhow::anyhow!("pool error")))?;

        // Verify the member exists
        members::table
            .find(body.member_id)
            .select(members::id)
            .first::<Uuid>(&mut conn)
            .map_err(|_| AppError::NotFound("member not found".into()))?;

        let new_wo = NewWorkOrder {
            id,
            member_id:             body.member_id,
            title:                 body.title.clone(),
            description:           body.description.clone(),
            priority:              body.priority.clone().unwrap_or_else(|| "medium".into()),
            status:                "intake".into(),
            assigned_to:           None,
            created_by:            actor_id,
            due_date:              body.due_date,
            created_at:            now,
            updated_at:            now,
            ticket_type:           body.ticket_type.clone(),
            processing_notes:      None,
            routed_to_org_unit_id: None,
            resolved_at:           None,
            closed_at:             None,
        };

        let wo: WorkOrder = diesel::insert_into(work_orders::table)
            .values(&new_wo)
            .returning(WorkOrder::as_returning())
            .get_result(&mut conn)
            .map_err(AppError::Database)?;

        audit_log::insert(
            &mut conn,
            NewAuditLog::new(Some(actor_id), "WORK_ORDER_CREATED", "work_order", Some(wo.id), ip)
                .with_new_value(serde_json::json!({
                    "title":       wo.title,
                    "member_id":   wo.member_id,
                    "status":      wo.status,
                    "priority":    wo.priority,
                    "ticket_type": wo.ticket_type,
                })),
        );

        Ok(wo)
    })
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("task join error")))??;

    Ok(HttpResponse::Created().json(WorkOrderResponse::from(wo)))
}

/// PATCH /work-orders/{id}/transition
/// Moves the work order through its lifecycle state machine.
/// Invalid transitions are rejected with 400.
/// Auto-routing fires on intake → triage.
/// Requires care_coach or administrator.
#[patch("/{id}/transition")]
async fn transition_work_order(
    req: HttpRequest,
    user: CareCoachAuth,
    pool: web::Data<DbPool>,
    id: web::Path<Uuid>,
    body: web::Json<TransitionRequest>,
) -> Result<HttpResponse, AppError> {
    body.validate().map_err(|e| AppError::BadRequest(e.to_string()))?;

    let to_status = body.to_status.trim().to_owned();
    if !VALID_STATUSES.contains(&to_status.as_str()) {
        return Err(AppError::BadRequest(format!(
            "invalid status '{}'; valid: {}",
            to_status,
            VALID_STATUSES.join(", ")
        )));
    }

    let ip         = req.connection_info().realip_remote_addr().map(str::to_owned);
    let actor_id   = user.user_id;
    let actor_name = user.username.clone();
    let is_admin   = matches!(user.role, Role::Administrator);
    let body       = body.into_inner();
    let wo_id      = *id;

    let wo = web::block(move || {
        let mut conn = pool.get().map_err(|_| AppError::Internal(anyhow::anyhow!("pool error")))?;

        // Load current work order
        let current: WorkOrder = work_orders::table
            .find(wo_id)
            .select(WorkOrder::as_select())
            .first(&mut conn)
            .map_err(|_| AppError::NotFound("work order not found".into()))?;

        // ── Object-level authorization ─────────────────────────
        // Admins may transition any work order.
        // Care coaches may transition work orders that are:
        //   - assigned to them, OR
        //   - routed to their org unit, OR
        //   - created by them (so the coach who filed a ticket can manage it)
        if !is_admin {
            let coach_org: Option<Uuid> = users::table
                .find(actor_id)
                .select(users::org_unit_id)
                .first::<Option<Uuid>>(&mut conn)
                .map_err(AppError::Database)?;

            let assigned_to_caller  = current.assigned_to == Some(actor_id);
            let routed_to_coach_org = coach_org
                .is_some_and(|oid| current.routed_to_org_unit_id == Some(oid));
            let created_by_caller   = current.created_by == actor_id;

            if !assigned_to_caller && !routed_to_coach_org && !created_by_caller {
                return Err(AppError::Forbidden);
            }
        }

        let from_status = current.status.clone();

        // ── State-machine guard ────────────────────────────────
        guard_transition(&from_status, &to_status)?;

        let now = Utc::now();

        // ── Accumulate processing notes ────────────────────────
        let new_notes = body.processing_notes.as_deref().map(|note| {
            append_note(
                current.processing_notes.clone(),
                note,
                &actor_name,
                &from_status,
                &to_status,
            )
        });

        // ── Auto-routing (intake → triage only) ───────────────
        let (routed_org_unit, auto_assigned) =
            if from_status == "intake" && to_status == "triage" {
                auto_route(&mut conn, current.member_id, current.ticket_type.as_deref())?
            } else {
                (None, None)
            };

        // ── Assignment resolution ──────────────────────────────
        // Priority: explicit override (admin only) > auto-route result > no change
        let assigned_to_update: Option<Option<Uuid>> =
            if let Some(explicit) = body.assigned_to {
                if !is_admin {
                    return Err(AppError::Forbidden);
                }
                Some(Some(explicit))
            } else {
                // `Option<Uuid>::map(Some)` produces `Option<Option<Uuid>>`
                // None → no change; Some(id) → set to id
                auto_assigned.map(Some)
            };

        // ── Lifecycle timestamps ───────────────────────────────
        let resolved_at = if to_status == "resolved" { Some(now) } else { None };
        let closed_at   = if to_status == "closed"   { Some(now) } else { None };

        let changeset = WorkOrderChangeset {
            status:                to_status.clone(),
            assigned_to:           assigned_to_update,
            processing_notes:      new_notes,
            routed_to_org_unit_id: routed_org_unit,
            resolved_at,
            closed_at,
            updated_at:            now,
        };

        let updated: WorkOrder = diesel::update(work_orders::table.find(wo_id))
            .set(&changeset)
            .returning(WorkOrder::as_returning())
            .get_result(&mut conn)
            .map_err(AppError::Database)?;

        audit_log::insert(
            &mut conn,
            NewAuditLog::new(Some(actor_id), "WORK_ORDER_TRANSITION", "work_order", Some(wo_id), ip)
                .with_new_value(serde_json::json!({
                    "from":                  from_status,
                    "to":                    to_status,
                    "auto_routed_org_unit":  routed_org_unit,
                    "auto_assigned_user":    auto_assigned,
                })),
        );

        Ok(updated)
    })
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("task join error")))??;

    Ok(HttpResponse::Ok().json(WorkOrderResponse::from(wo)))
}

/// GET /work-orders
/// Returns work orders visible to the caller, scoped by role:
/// - Administrator: all work orders (optional member_id filter).
/// - Approver:      all work orders (visibility only; no transitions).
/// - CareCoach:     work orders assigned to them or routed to their org_unit.
/// - Member:        their own work orders only.
///
/// Optional query params: status, priority, ticket_type, member_id (admin only).
#[get("")]
async fn list_work_orders(
    user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    query: web::Query<ListQuery>,
) -> Result<HttpResponse, AppError> {
    let actor_id  = user.user_id;
    let actor_role = user.role;
    let query     = query.into_inner();

    let results = web::block(move || {
        let mut conn = pool.get().map_err(|_| AppError::Internal(anyhow::anyhow!("pool error")))?;

        let is_admin  = matches!(actor_role, Role::Administrator);
        let is_coach  = matches!(actor_role, Role::CareCoach);
        let is_member = matches!(actor_role, Role::Member);

        // Pre-fetch the caller's member_id (members only)
        let caller_member_id: Option<Uuid> = if is_member {
            let mid: Uuid = members::table
                .filter(members::user_id.eq(actor_id))
                .select(members::id)
                .first(&mut conn)
                .map_err(|_| AppError::NotFound("member profile not found".into()))?;
            Some(mid)
        } else {
            None
        };

        // Pre-fetch the care_coach's org_unit_id
        let coach_org_unit: Option<Uuid> = if is_coach {
            users::table
                .find(actor_id)
                .select(users::org_unit_id)
                .first::<Option<Uuid>>(&mut conn)
                .map_err(AppError::Database)?
        } else {
            None
        };

        let mut q = work_orders::table.into_boxed();

        // ── Role-based access scope ────────────────────────────
        if is_member {
            if let Some(mid) = caller_member_id {
                q = q.filter(work_orders::member_id.eq(mid));
            }
        } else if is_coach {
            // Work orders assigned to them OR routed to their org_unit
            if let Some(ouid) = coach_org_unit {
                q = q.filter(
                    work_orders::assigned_to
                        .eq(actor_id)
                        .or(work_orders::routed_to_org_unit_id.eq(ouid)),
                );
            } else {
                q = q.filter(work_orders::assigned_to.eq(actor_id));
            }
        }
        // Administrator / Approver: no row-level restriction

        // ── Optional filters ───────────────────────────────────
        if let Some(s) = query.status {
            if VALID_STATUSES.contains(&s.as_str()) {
                q = q.filter(work_orders::status.eq(s));
            }
        }
        if let Some(p) = query.priority {
            if VALID_PRIORITIES.contains(&p.as_str()) {
                q = q.filter(work_orders::priority.eq(p));
            }
        }
        if let Some(t) = query.ticket_type {
            if VALID_TICKET_TYPES.contains(&t.as_str()) {
                q = q.filter(work_orders::ticket_type.eq(t));
            }
        }
        // member_id override — admin only
        if is_admin {
            if let Some(mid) = query.member_id {
                q = q.filter(work_orders::member_id.eq(mid));
            }
        }

        q.select(WorkOrder::as_select())
            .order(work_orders::created_at.desc())
            .load::<WorkOrder>(&mut conn)
            .map_err(AppError::Database)
    })
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("task join error")))??;

    let body: Vec<WorkOrderResponse> = results.into_iter().map(WorkOrderResponse::from).collect();
    Ok(HttpResponse::Ok().json(body))
}
