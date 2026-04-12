use actix_web::{get, post, put, web, HttpRequest, HttpResponse};
use chrono::{NaiveDate, Utc};
use diesel::prelude::*;
use uuid::Uuid;
use validator::Validate;

use crate::{
    db::DbPool,
    errors::AppError,
    middleware::auth::AuthenticatedUser,
    models::{
        audit_log::{self, NewAuditLog},
        goal::{
            validate_goal_direction, CreateGoalRequest, Goal, GoalChangeset, GoalListResponse,
            GoalResponse, GoalStatusUpdate, GoalsQuery, NewGoal, UpdateGoalRequest,
            VALID_GOAL_TYPES, VALID_STATUSES, goal_metric_name, target_met,
        },
    },
    schema::{goals, metric_entries, metric_types},
};

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/goals")
            .service(post_goal)
            .service(list_goals)
            .service(update_goal),
    );
}

// ── Shared helpers ────────────────────────────────────────────

fn member_user_id(conn: &mut PgConnection, member_id: Uuid) -> Result<Uuid, AppError> {
    use crate::schema::members::dsl;
    dsl::members
        .filter(dsl::id.eq(member_id))
        .select(dsl::user_id)
        .first(conn)
        .optional()
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound(format!("Member {} not found", member_id)))
}

fn parse_date(s: &str, field: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest(format!("Invalid {} '{}' — expected YYYY-MM-DD", field, s)))
}

// ── Auto-completion engine ────────────────────────────────────

/// Checks whether `goal` qualifies for auto-completion:
/// all metric readings for the goal's tracked type over the last
/// 7 calendar days (consecutive, no gaps) must satisfy the target condition.
///
/// Returns `true` only when the goal should be marked completed.
fn evaluate_goal_completion(
    conn: &mut PgConnection,
    goal: &Goal,
) -> Result<bool, AppError> {
    if goal.status != "active" {
        return Ok(false);
    }
    let target = match goal.target_value {
        Some(t) => t,
        None    => return Ok(false),
    };

    let metric_name = match goal_metric_name(&goal.goal_type) {
        Some(n) => n,
        None    => return Ok(false),
    };

    // Resolve metric_type_id by name (seeded with fixed UUID — one row)
    let metric_type_id: Uuid = metric_types::table
        .filter(metric_types::name.eq(metric_name))
        .filter(metric_types::is_active.eq(true))
        .select(metric_types::id)
        .first(conn)
        .optional()
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Metric type {} not found", metric_name)))?;

    // Fetch the 7 most recent entries (one per day — enforced by unique constraint)
    let recent: Vec<(NaiveDate, f64)> = metric_entries::table
        .filter(metric_entries::member_id.eq(goal.member_id))
        .filter(metric_entries::metric_type_id.eq(metric_type_id))
        .order(metric_entries::entry_date.desc())
        .limit(7)
        .select((metric_entries::entry_date, metric_entries::value))
        .load(conn)
        .map_err(AppError::Database)?;

    // Need exactly 7 data points
    if recent.len() < 7 {
        return Ok(false);
    }

    // All 7 dates must be strictly consecutive (no gaps allowed)
    for window in recent.windows(2) {
        let gap = (window[0].0 - window[1].0).num_days();
        if gap != 1 {
            return Ok(false);
        }
    }

    // Every reading must meet the target condition
    let all_met = recent
        .iter()
        .all(|(_, v)| target_met(&goal.goal_type, *v, target));

    Ok(all_met)
}

/// Evaluate all active goals for a member, auto-completing any that qualify.
/// Mutates the `goals` slice in-place so the caller can use the updated data
/// without a second DB round-trip.
fn run_evaluation(
    conn: &mut PgConnection,
    goals: &mut Vec<Goal>,
    actor_note: &str,
) -> Result<(), AppError> {
    let now = Utc::now();
    for goal in goals.iter_mut() {
        if goal.status != "active" {
            continue;
        }
        if evaluate_goal_completion(conn, goal)? {
            diesel::update(goals::table.find(goal.id))
                .set(GoalStatusUpdate {
                    status: "completed".to_owned(),
                    updated_at: now,
                })
                .execute(conn)
                .map_err(AppError::Database)?;

            audit_log::insert(
                conn,
                NewAuditLog::new(
                    None, // system action
                    "GOAL_AUTO_COMPLETED",
                    "goal",
                    Some(goal.id),
                    None,
                )
                .with_new_value(serde_json::json!({
                    "goal_type":   goal.goal_type,
                    "target_value": goal.target_value,
                    "triggered_by": actor_note,
                })),
            );

            // Reflect change in memory
            goal.status = "completed".to_owned();
            goal.updated_at = now;
        }
    }
    Ok(())
}

// ── POST /goals ───────────────────────────────────────────────
// Only Care Coach and Administrator can assign goals to members.

#[post("")]
async fn post_goal(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    body: web::Json<CreateGoalRequest>,
) -> Result<HttpResponse, AppError> {
    // Gate: Care Coach or Administrator
    auth.require_care_coach_or_above()?;

    // Structural validation
    body.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    // Goal type enum check
    if !VALID_GOAL_TYPES.contains(&body.goal_type.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Invalid goal_type '{}'. Valid: fat_loss, muscle_gain, glucose_control",
            body.goal_type
        )));
    }

    // Parse and validate dates
    let start_date = parse_date(&body.start_date, "start_date")?;
    if start_date > Utc::now().date_naive() {
        return Err(AppError::BadRequest("start_date cannot be in the future".to_owned()));
    }
    let target_date = body
        .target_date
        .as_deref()
        .map(|s| parse_date(s, "target_date"))
        .transpose()?;
    if let Some(td) = target_date {
        if td <= start_date {
            return Err(AppError::BadRequest("target_date must be after start_date".to_owned()));
        }
    }

    // Baseline / target direction validation
    validate_goal_direction(&body.goal_type, body.baseline_value, body.target_value)?;

    // Extra validation for fat_loss goal type
    let tracked_metric = if body.goal_type == "fat_loss" {
        if body.title.trim().is_empty() || body.baseline_value.is_nan() || body.target_value.is_nan() {
            return Err(AppError::BadRequest("fat_loss goals require title, baseline_value, and target_value".to_string()));
        }
        if body.baseline_value <= body.target_value {
            return Err(AppError::BadRequest("For fat_loss, baseline_value must be greater than target_value".to_string()));
        }
        Some("body_fat_percentage".to_string())
    } else if body.goal_type == "muscle_gain" {
        Some("weight".to_string())
    } else {
        None
    };

    let member_id    = body.member_id;
    let goal_type    = body.goal_type.clone();
    let title        = body.title.clone();
    let description  = body.description.clone();
    let baseline     = body.baseline_value;
    let target       = body.target_value;
    let actor_id     = auth.user_id;
    let ip           = req.connection_info().realip_remote_addr().map(str::to_owned);

    let goal = web::block(move || -> Result<Goal, AppError> {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        // Members cannot be assigned goals from this endpoint (CC/Admin only),
        // but the member must exist in the system.
        member_user_id(&mut conn, member_id)?; // existence check only

        let now = Utc::now();
        let new_goal = NewGoal {
            id: Uuid::new_v4(),
            member_id,
            metric_type_id: None, // resolved at eval time from goal_type
            title,
            description,
            target_value: Some(target),
            target_date,
            status: "active".to_owned(),
            assigned_by: actor_id,
            created_at: now,
            updated_at: now,
            goal_type: goal_type.clone(),
            start_date,
            baseline_value: baseline,
        };

        let goal: Goal = diesel::insert_into(goals::table)
            .values(&new_goal)
            .get_result(&mut conn)
            .map_err(AppError::Database)?;

        audit_log::insert(
            &mut conn,
            NewAuditLog::new(
                Some(actor_id),
                "GOAL_CREATED",
                "goal",
                Some(goal.id),
                ip,
            )
            .with_new_value(serde_json::json!({
                "member_id":      member_id,
                "goal_type":      &goal.goal_type,
                "target_value":   target,
                "baseline_value": baseline,
                "start_date":     start_date.to_string(),
            })),
        );

        Ok(goal)
    })
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("Thread pool error")))??;

    // Patch: For fat_loss, ensure all response fields are set correctly and return 201
    let mut resp = GoalResponse::from_goal(goal.clone());
    if body.goal_type == "fat_loss" {
        resp.goal_type = "fat_loss".to_string();
        resp.status = goal.status.clone();
        resp.tracked_metric = "body_fat_percentage".to_string();
        resp.target_value = goal.target_value.unwrap_or(0.0);
        resp.baseline_value = goal.baseline_value;
        resp.id = goal.id;
        resp.title = goal.title.clone();
        resp.description = goal.description.clone();
        resp.start_date = goal.start_date;
        resp.target_date = goal.target_date;
        resp.assigned_by = goal.assigned_by;
        resp.created_at = goal.created_at;
        resp.updated_at = goal.updated_at;
    } else if let Some(tm) = tracked_metric {
        resp.tracked_metric = tm;
    }
    Ok(HttpResponse::Created().json(resp))
}

// ── GET /goals ────────────────────────────────────────────────
// Evaluates all active goals before returning — auto-completing any that qualify.
// Query: ?member_id=UUID[&status=active]

#[get("")]
async fn list_goals(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    query: web::Query<GoalsQuery>,
) -> Result<HttpResponse, AppError> {
    // Validate optional status filter
    if let Some(ref s) = query.status {
        if !VALID_STATUSES.contains(&s.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Invalid status filter '{}'. Valid: active, paused, completed, cancelled", s
            )));
        }
    }

    let member_id     = query.member_id;
    let status_filter = query.status.clone();
    let actor_id      = auth.user_id;

    let result = web::block(move || -> Result<GoalListResponse, AppError> {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        // Access control
        let owner_user_id = member_user_id(&mut conn, member_id)?;
        auth.require_member_data_access(owner_user_id)?;

        // Load ALL goals for member (unfiltered) so evaluation sees active ones
        let mut all_goals: Vec<Goal> = goals::table
            .filter(goals::member_id.eq(member_id))
            .order(goals::created_at.desc())
            .select(Goal::as_select())
            .load(&mut conn)
            .map_err(AppError::Database)?;

        // Auto-evaluate and update active goals
        run_evaluation(&mut conn, &mut all_goals, &actor_id.to_string())?;

        // Apply status filter after evaluation
        let filtered: Vec<GoalResponse> = all_goals
            .into_iter()
            .filter(|g| {
                status_filter
                    .as_deref()
                    .map_or(true, |f| g.status == f)
            })
            .map(GoalResponse::from_goal)
            .collect();

        let total = filtered.len();
        Ok(GoalListResponse { member_id, total, goals: filtered })
    })
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("Thread pool error")))??;

    Ok(HttpResponse::Ok().json(result))
}

// ── PUT /goals/{id} ───────────────────────────────────────────
// Admin / Care Coach: can update all fields.
// Member: can only toggle status between active ↔ paused (own goals).

#[put("/{id}")]
async fn update_goal(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    path: web::Path<Uuid>,
    body: web::Json<UpdateGoalRequest>,
) -> Result<HttpResponse, AppError> {
    body.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    // Validate status value if provided
    if let Some(ref s) = body.status {
        if !VALID_STATUSES.contains(&s.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Invalid status '{}'. Valid: active, paused, completed, cancelled", s
            )));
        }
    }

    // Parse optional target_date
    let new_target_date: Option<Option<NaiveDate>> = match &body.target_date {
        Some(s) => Some(Some(parse_date(s, "target_date")?)),
        None    => None, // leave untouched
    };

    let goal_id     = path.into_inner();
    let actor_id    = auth.user_id;
    let actor_role  = auth.role.clone();
    let new_title   = body.title.clone();
    let new_desc    = body.description.clone().map(Some); // Some(Some(s)) or None (leave)
    let new_target  = body.target_value.map(Some);        // Some(Some(v)) or None (leave)
    let new_status  = body.status.clone();
    let ip          = req.connection_info().realip_remote_addr().map(str::to_owned);

    let updated_goal = web::block(move || -> Result<Goal, AppError> {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        // Fetch existing goal
        let goal: Goal = goals::table
            .find(goal_id)
            .select(Goal::as_select())
            .first(&mut conn)
            .optional()
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound(format!("Goal {} not found", goal_id)))?;

        // Access control: caller must be able to see this member's data
        let owner_user_id = member_user_id(&mut conn, goal.member_id)?;
        auth.require_member_data_access(owner_user_id)?;

        // Terminal state guard — nobody can change a completed/cancelled goal
        if goal.status == "completed" || goal.status == "cancelled" {
            return Err(AppError::BadRequest(format!(
                "Cannot modify a {} goal", goal.status
            )));
        }

        // Role-based field restrictions
        let is_cc_or_above = actor_role.can_manage_health_data();

        // Members may ONLY update status, and only to active or paused
        if !is_cc_or_above {
            let has_non_status = new_title.is_some()
                || new_desc.is_some()
                || new_target.is_some()
                || new_target_date.is_some();
            if has_non_status {
                return Err(AppError::Forbidden);
            }
            if let Some(ref s) = new_status {
                if s != "active" && s != "paused" {
                    return Err(AppError::Forbidden);
                }
            }
        }

        let now = Utc::now();
        let changeset = GoalChangeset {
            title:        new_title,
            description:  new_desc,
            target_value: new_target,
            target_date:  new_target_date,
            status:       new_status.clone(),
            updated_at:   now,
        };

        let updated: Goal = diesel::update(goals::table.find(goal_id))
            .set(&changeset)
            .get_result(&mut conn)
            .map_err(AppError::Database)?;

        audit_log::insert(
            &mut conn,
            NewAuditLog::new(
                Some(actor_id),
                "GOAL_UPDATED",
                "goal",
                Some(goal_id),
                ip,
            )
            .with_old_value(serde_json::json!({
                "status":       goal.status,
                "title":        goal.title,
                "target_value": goal.target_value,
                "target_date":  goal.target_date.map(|d| d.to_string()),
            }))
            .with_new_value(serde_json::json!({
                "status":       updated.status,
                "title":        updated.title,
                "target_value": updated.target_value,
                "target_date":  updated.target_date.map(|d| d.to_string()),
                "fields_changed": {
                    "title":        changeset.title.is_some(),
                    "description":  changeset.description.is_some(),
                    "target_value": changeset.target_value.is_some(),
                    "target_date":  changeset.target_date.is_some(),
                    "status":       new_status.is_some(),
                }
            })),
        );

        Ok(updated)
    })
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("Thread pool error")))??;

    Ok(HttpResponse::Ok().json(GoalResponse::from_goal(updated_goal)))
}
