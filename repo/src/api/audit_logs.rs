//! GET /audit-logs — admin-only audit log query endpoint.
//!
//! Supports filtering by actor, action, reason_code, entity type/id, and
//! date range.  Results are paginated (default 50 / max 200 per page)
//! and always ordered by `created_at DESC` (newest first).
//!
//! The endpoint is read-only; the DB trigger added in migration 00012
//! makes audit_logs physically immutable — no UPDATE or DELETE is possible.

use actix_web::{get, web, HttpResponse};
use chrono::{NaiveDate, Utc};
use diesel::prelude::*;

use crate::{
    db::DbPool,
    errors::AppError,
    middleware::auth::AdminAuth,
    models::audit_log::{AuditLog, AuditLogPage, AuditLogQuery, AuditLogResponse},
    schema::audit_logs,
};

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/audit-logs").service(list_audit_logs));
}

const DEFAULT_PAGE:     i64 = 1;
const DEFAULT_PER_PAGE: i64 = 50;
const MAX_PER_PAGE:     i64 = 200;

/// GET /audit-logs
///
/// Query parameters (all optional):
/// - `actor_id`    — filter by who performed the action
/// - `action`      — exact action string match (e.g. "LOGIN_FAILED")
/// - `reason_code` — exact reason_code match
/// - `entity_type` — e.g. "work_order", "health_profile"
/// - `entity_id`   — specific entity UUID
/// - `start_date`  — inclusive lower bound (YYYY-MM-DD)
/// - `end_date`    — inclusive upper bound (YYYY-MM-DD)
/// - `page`        — 1-based page number (default 1)
/// - `per_page`    — rows per page (default 50, max 200)
#[get("")]
async fn list_audit_logs(
    _auth: AdminAuth,
    pool: web::Data<DbPool>,
    query: web::Query<AuditLogQuery>,
) -> Result<HttpResponse, AppError> {
    let q = query.into_inner();

    let page     = q.page.unwrap_or(DEFAULT_PAGE).max(1);
    let per_page = q.per_page.unwrap_or(DEFAULT_PER_PAGE).clamp(1, MAX_PER_PAGE);
    let offset   = (page - 1) * per_page;

    // Parse optional date bounds
    let start_dt = q.start_date.as_deref().map(|s| {
        NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
            .map_err(|_| AppError::BadRequest(format!("invalid start_date '{}'", s)))
    }).transpose()?;

    let end_dt = q.end_date.as_deref().map(|s| {
        NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map(|d| d.and_hms_opt(23, 59, 59).unwrap().and_utc())
            .map_err(|_| AppError::BadRequest(format!("invalid end_date '{}'", s)))
    }).transpose()?;

    let actor_id    = q.actor_id;
    let action      = q.action;
    let reason_code = q.reason_code;
    let entity_type = q.entity_type;
    let entity_id   = q.entity_id;

    let (rows, total) = web::block(move || {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        // ── Build the base boxed query ────────────────────────
        let mut base = audit_logs::table
            .into_boxed::<diesel::pg::Pg>();

        if let Some(aid) = actor_id {
            base = base.filter(audit_logs::actor_id.eq(aid));
        }
        if let Some(ref act) = action {
            base = base.filter(audit_logs::action.eq(act));
        }
        if let Some(ref rc) = reason_code {
            base = base.filter(audit_logs::reason_code.eq(rc));
        }
        if let Some(ref et) = entity_type {
            base = base.filter(audit_logs::entity_type.eq(et));
        }
        if let Some(eid) = entity_id {
            base = base.filter(audit_logs::entity_id.eq(eid));
        }
        if let Some(start) = start_dt {
            base = base.filter(audit_logs::created_at.ge(start));
        }
        if let Some(end) = end_dt {
            base = base.filter(audit_logs::created_at.le(end));
        }

        // ── Count total matching rows ─────────────────────────
        // Rebuild the filter on a fresh boxed query for the COUNT.
        let mut count_q = audit_logs::table.into_boxed::<diesel::pg::Pg>();
        if let Some(aid) = actor_id {
            count_q = count_q.filter(audit_logs::actor_id.eq(aid));
        }
        if let Some(ref act) = action {
            count_q = count_q.filter(audit_logs::action.eq(act));
        }
        if let Some(ref rc) = reason_code {
            count_q = count_q.filter(audit_logs::reason_code.eq(rc));
        }
        if let Some(ref et) = entity_type {
            count_q = count_q.filter(audit_logs::entity_type.eq(et));
        }
        if let Some(eid) = entity_id {
            count_q = count_q.filter(audit_logs::entity_id.eq(eid));
        }
        if let Some(start) = start_dt {
            count_q = count_q.filter(audit_logs::created_at.ge(start));
        }
        if let Some(end) = end_dt {
            count_q = count_q.filter(audit_logs::created_at.le(end));
        }

        let total: i64 = count_q
            .count()
            .get_result(&mut conn)
            .map_err(AppError::Database)?;

        // ── Paginated rows ────────────────────────────────────
        let rows: Vec<AuditLog> = base
            .select(AuditLog::as_select())
            .order(audit_logs::created_at.desc())
            .limit(per_page)
            .offset(offset)
            .load(&mut conn)
            .map_err(AppError::Database)?;

        Ok::<_, AppError>((rows, total))
    })
    .await
    .map_err(|e: actix_web::error::BlockingError| AppError::Internal(anyhow::anyhow!(e)))??;

    let data: Vec<AuditLogResponse> = rows.into_iter().map(Into::into).collect();

    Ok(HttpResponse::Ok().json(AuditLogPage {
        data,
        page,
        per_page,
        total,
    }))
}
