use actix_web::{get, post, web, HttpRequest, HttpResponse};
use chrono::{Duration, NaiveDate, Utc};
use diesel::prelude::*;
use std::collections::BTreeMap;
use uuid::Uuid;
use validator::Validate;

use crate::{
    db::DbPool,
    errors::AppError,
    middleware::auth::AuthenticatedUser,
    models::{
        audit_log::{self, NewAuditLog},
        metric::{
            is_valid_metric_type, validate_metric_value, CreateMetricEntryRequest, EntryWithType,
            MetricEntryResponse, MetricListResponse, MetricSummaryItem, MetricSummaryResponse,
            MetricTypeRecord, MetricsQuery, NewMetricEntry, SummaryQuery,
        },
    },
    schema::{metric_entries, metric_types},
};

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/metrics")
            .service(post_metric)
            .service(list_metrics)
            .service(get_summary),
    );
}

// ── Shared helpers ────────────────────────────────────────────

/// Resolve the `user_id` that owns a given member_id.
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

/// Parse range shorthand or explicit start/end into a (start, end) NaiveDate pair.
fn resolve_date_range(
    range: Option<&str>,
    start: Option<&str>,
    end: Option<&str>,
) -> Result<(NaiveDate, NaiveDate), AppError> {
    let today = Utc::now().date_naive();

    if let Some(r) = range {
        return match r {
            "7d"  => Ok((today - Duration::days(7),  today)),
            "30d" => Ok((today - Duration::days(30), today)),
            "90d" => Ok((today - Duration::days(90), today)),
            "all" => Ok((NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(), today)),
            other => Err(AppError::BadRequest(format!(
                "Invalid range '{}'. Accepted: 7d | 30d | 90d | all", other
            ))),
        };
    }

    match (start, end) {
        (Some(s), Some(e)) => {
            let s = NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|_| AppError::BadRequest(format!("Invalid start date '{}'", s)))?;
            let e = NaiveDate::parse_from_str(e, "%Y-%m-%d")
                .map_err(|_| AppError::BadRequest(format!("Invalid end date '{}'", e)))?;
            if s > e {
                return Err(AppError::BadRequest("start must be <= end".to_owned()));
            }
            Ok((s, e))
        }
        (Some(_), None) => Err(AppError::BadRequest("end is required when start is provided".to_owned())),
        (None, Some(_)) => Err(AppError::BadRequest("start is required when end is provided".to_owned())),
        // Default: last 30 days
        (None, None) => Ok((today - Duration::days(30), today)),
    }
}

/// Detect a unique-constraint violation from Diesel.
fn is_unique_violation(e: &diesel::result::Error) -> bool {
    matches!(
        e,
        diesel::result::Error::DatabaseError(
            diesel::result::DatabaseErrorKind::UniqueViolation,
            _
        )
    )
}

/// Load metric_entries joined with metric_types for a given member + date range.
/// Optional `type_filter` restricts to a single metric type name.
fn load_entries(
    conn: &mut PgConnection,
    member_id: Uuid,
    start: NaiveDate,
    end: NaiveDate,
    type_filter: Option<&str>,
) -> Result<Vec<EntryWithType>, diesel::result::Error> {
    // Build the base result set and apply the optional metric-type filter in Rust.
    // This avoids complex boxed-query generics while keeping DB work minimal —
    // max 5 types × ~90 rows = ~450 rows at most, well within in-memory comfort.
    let all: Vec<(Uuid, Uuid, Uuid, String, String, f64, NaiveDate, Uuid, Option<String>, chrono::DateTime<Utc>)> =
        metric_entries::table
            .inner_join(metric_types::table)
            .filter(metric_entries::member_id.eq(member_id))
            .filter(metric_entries::entry_date.ge(start))
            .filter(metric_entries::entry_date.le(end))
            .order(metric_entries::entry_date.asc())
            .select((
                metric_entries::id,
                metric_entries::member_id,
                metric_entries::metric_type_id,
                metric_types::name,
                metric_types::unit,
                metric_entries::value,
                metric_entries::entry_date,
                metric_entries::recorded_by,
                metric_entries::notes,
                metric_entries::created_at,
            ))
            .load(conn)?;

    let rows: Vec<EntryWithType> = all
        .into_iter()
        .filter(|(_, _, _, name, _, _, _, _, _, _)| {
            type_filter.map_or(true, |f| name == f)
        })
        .map(|(id, member_id, metric_type_id, metric_type_name, unit, value, entry_date, recorded_by, notes, created_at)| {
            EntryWithType {
                id,
                member_id,
                metric_type_id,
                metric_type_name,
                unit,
                value,
                entry_date,
                recorded_by,
                notes,
                created_at,
            }
        })
        .collect();

    Ok(rows)
}

// ── POST /metrics ─────────────────────────────────────────────

#[post("")]
async fn post_metric(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    body: web::Json<CreateMetricEntryRequest>,
) -> Result<HttpResponse, AppError> {
    // 1. Structural validation
    body.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    // 2. Metric type enum check
    if !is_valid_metric_type(&body.metric_type) {
        return Err(AppError::BadRequest(format!(
            "Unknown metric type '{}'. Valid types: weight, body_fat_percentage, waist, hip, chest",
            body.metric_type
        )));
    }

    // 3. Per-type value range check
    validate_metric_value(&body.metric_type, body.value)?;

    // 4. Parse entry_date; default to today UTC
    let entry_date = match &body.entry_date {
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|_| AppError::BadRequest(format!("Invalid entry_date '{}'", s)))?,
        None => Utc::now().date_naive(),
    };

    // Entry dates in the future are not allowed
    if entry_date > Utc::now().date_naive() {
        return Err(AppError::BadRequest("entry_date cannot be in the future".to_owned()));
    }

    let member_id   = body.member_id;
    let metric_type = body.metric_type.clone();
    let value       = body.value;
    let notes       = body.notes.clone();
    let actor_id    = auth.user_id;
    let ip          = req.connection_info().realip_remote_addr().map(str::to_owned);

    let response = web::block(move || -> Result<MetricEntryResponse, AppError> {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        // 5. Access control
        let owner_user_id = member_user_id(&mut conn, member_id)?;
        auth.require_member_data_access(owner_user_id)?;

        // 6. Resolve metric_type_id from name
        let mt: MetricTypeRecord = metric_types::table
            .filter(metric_types::name.eq(&metric_type))
            .filter(metric_types::is_active.eq(true))
            .select(MetricTypeRecord::as_select())
            .first(&mut conn)
            .optional()
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::BadRequest(format!("Metric type '{}' is not active", metric_type)))?;

        // 7. Insert — DB unique constraint enforces one-per-member-per-type-per-day
        let new_entry = NewMetricEntry {
            id: Uuid::new_v4(),
            member_id,
            metric_type_id: mt.id,
            value,
            entry_date,
            recorded_by: actor_id,
            notes: notes.clone(),
            created_at: Utc::now(),
        };

        match diesel::insert_into(metric_entries::table)
            .values(&new_entry)
            .execute(&mut conn)
        {
            Ok(_) => {}
            Err(ref e) if is_unique_violation(e) => {
                return Err(AppError::Conflict(format!(
                    "A '{}' entry already exists for member {} on {}",
                    metric_type, member_id, entry_date
                )));
            }
            Err(e) => return Err(AppError::Database(e)),
        }

        // 8. Audit
        audit_log::insert(
            &mut conn,
            NewAuditLog::new(
                Some(actor_id),
                "METRIC_ENTRY_CREATED",
                "metric_entry",
                Some(new_entry.id),
                ip,
            )
            .with_new_value(serde_json::json!({
                "member_id":   member_id,
                "metric_type": metric_type,
                "value":       value,
                "entry_date":  entry_date.to_string(),
            })),
        );

        Ok(MetricEntryResponse {
            id:          new_entry.id,
            member_id:   new_entry.member_id,
            metric_type: mt.name,
            unit:        mt.unit,
            value:       new_entry.value,
            entry_date:  new_entry.entry_date,
            recorded_by: new_entry.recorded_by,
            notes:       new_entry.notes,
            created_at:  new_entry.created_at,
        })
    })
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("Thread pool error")))??;

    Ok(HttpResponse::Created().json(response))
}

// ── GET /metrics ──────────────────────────────────────────────
//
// Query params: member_id, range?, start?, end?, metric_type?

#[get("")]
async fn list_metrics(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    query: web::Query<MetricsQuery>,
) -> Result<HttpResponse, AppError> {
    // Validate optional metric_type filter
    if let Some(ref mt) = query.metric_type {
        if !is_valid_metric_type(mt) {
            return Err(AppError::BadRequest(format!(
                "Unknown metric_type filter '{}'. Valid: weight, body_fat_percentage, waist, hip, chest",
                mt
            )));
        }
    }

    let (start, end) = resolve_date_range(
        query.range.as_deref(),
        query.start.as_deref(),
        query.end.as_deref(),
    )?;

    let member_id   = query.member_id;
    let type_filter = query.metric_type.clone();

    let result = web::block(move || -> Result<MetricListResponse, AppError> {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        let owner_user_id = member_user_id(&mut conn, member_id)?;
        auth.require_member_data_access(owner_user_id)?;

        let rows = load_entries(&mut conn, member_id, start, end, type_filter.as_deref())
            .map_err(AppError::Database)?;

        // Return most-recent first for list view
        let mut entries: Vec<MetricEntryResponse> =
            rows.into_iter().map(MetricEntryResponse::from).collect();
        entries.sort_by(|a, b| b.entry_date.cmp(&a.entry_date));

        let total = entries.len();
        Ok(MetricListResponse { member_id, range_start: start, range_end: end, total, entries })
    })
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("Thread pool error")))??;

    Ok(HttpResponse::Ok().json(result))
}

// ── GET /metrics/summary ──────────────────────────────────────
//
// Query params: member_id, range?, start?, end?

#[get("/summary")]
async fn get_summary(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
    query: web::Query<SummaryQuery>,
) -> Result<HttpResponse, AppError> {
    let (start, end) = resolve_date_range(
        query.range.as_deref(),
        query.start.as_deref(),
        query.end.as_deref(),
    )?;

    let member_id = query.member_id;

    let result = web::block(move || -> Result<MetricSummaryResponse, AppError> {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        let owner_user_id = member_user_id(&mut conn, member_id)?;
        auth.require_member_data_access(owner_user_id)?;

        // Load all entries in range (all types), sorted ASC by date
        let rows = load_entries(&mut conn, member_id, start, end, None)
            .map_err(AppError::Database)?;

        // Group by metric_type using BTreeMap for stable ordering
        let mut groups: BTreeMap<String, (String, Vec<(NaiveDate, f64)>)> = BTreeMap::new();
        for row in &rows {
            groups
                .entry(row.metric_type_name.clone())
                .or_insert_with(|| (row.unit.clone(), Vec::new()))
                .1
                .push((row.entry_date, row.value));
        }

        let summaries: Vec<MetricSummaryItem> = groups
            .into_iter()
            .map(|(metric_type, (unit, points))| {
                // points are already sorted ASC by entry_date from load_entries
                let count = points.len();
                let values: Vec<f64> = points.iter().map(|(_, v)| *v).collect();

                let first_date   = points.first().map(|(d, _)| *d);
                let latest_date  = points.last().map(|(d, _)| *d);
                let first_value  = points.first().map(|(_, v)| *v);
                let latest_value = points.last().map(|(_, v)| *v);

                let min_value = values.iter().cloned().reduce(f64::min);
                let max_value = values.iter().cloned().reduce(f64::max);
                let avg_value = if count > 0 {
                    Some(values.iter().sum::<f64>() / count as f64)
                } else {
                    None
                };

                let change = match (first_value, latest_value) {
                    (Some(f), Some(l)) => Some(round2(l - f)),
                    _ => None,
                };
                let change_pct = match (first_value, change) {
                    (Some(f), Some(c)) if f != 0.0 => Some(round2(c / f * 100.0)),
                    _ => None,
                };

                MetricSummaryItem {
                    metric_type,
                    unit,
                    count,
                    first_date,
                    latest_date,
                    first_value:  first_value.map(round2),
                    latest_value: latest_value.map(round2),
                    min_value:    min_value.map(round2),
                    max_value:    max_value.map(round2),
                    avg_value:    avg_value.map(round2),
                    change,
                    change_pct,
                }
            })
            .collect();

        Ok(MetricSummaryResponse { member_id, range_start: start, range_end: end, summaries })
    })
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("Thread pool error")))??;

    Ok(HttpResponse::Ok().json(result))
}

// ── Utility ───────────────────────────────────────────────────

/// Round to 2 decimal places to avoid floating-point noise in responses.
#[inline]
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
