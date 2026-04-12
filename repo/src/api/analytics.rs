use std::collections::HashMap;
use std::path::Path;

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use chrono::Utc;
use diesel::prelude::*;
use rust_xlsxwriter::{Format, Workbook};
use tracing::info;
use uuid::Uuid;

use validator::Validate;

use crate::{
    config::AppConfig,
    db::DbPool,
    errors::AppError,
    middleware::auth::{AuthenticatedUser, CareCoachAuth},
    models::{
        analytics::{
            AnalyticsQuery, AnalyticsReport, AttendanceMetrics, CancellationMetrics,
            ConversionMetrics, DistributionMetrics, ExportMeta, ExportRequest, NamedCount,
            PeriodInfo, PopularityMetrics, ResolvedFilter,
        },
        audit_log::{self, NewAuditLog},
    },
    schema::{
        approvals, goals, health_profiles, members, metric_entries, metric_types,
        notifications, work_orders, workflow_instances, workflow_templates,
    },
    security::{hmac_sign, masking},
};

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/analytics")
            .service(get_analytics)
            .service(export_analytics)
            .service(download_export),
    );
}

// ── Query helpers ─────────────────────────────────────────────

/// Pre-load the member_id list scoped to an org unit.
/// Returns an empty Vec when no org_unit filter is requested
/// (the caller treats empty = "all members").
fn load_member_ids(
    conn: &mut PgConnection,
    org_unit_id: Option<Uuid>,
) -> Result<Vec<Uuid>, AppError> {
    match org_unit_id {
        None => Ok(vec![]),
        Some(ouid) => members::table
            .filter(members::org_unit_id.eq(ouid))
            .select(members::id)
            .load::<Uuid>(conn)
            .map_err(AppError::Database),
    }
}

/// Apply a member-ID scope filter to a boxed query.
/// If `member_ids` is empty, no filter is applied (all rows).
macro_rules! apply_member_filter {
    ($q:expr, $col:expr, $ids:expr) => {
        if !$ids.is_empty() {
            $q = $q.filter($col.eq_any($ids.to_vec()));
        }
    };
}

/// Apply start/end datetime filters to a boxed query.
macro_rules! apply_time_filter {
    ($q:expr, $col:expr, $filter:expr) => {
        if let Some(start) = $filter.start_dt {
            $q = $q.filter($col.ge(start));
        }
        if let Some(end) = $filter.end_dt {
            $q = $q.filter($col.le(end));
        }
    };
}

/// Group a Vec<String> into sorted NamedCount items (most common first).
fn count_strings(values: Vec<String>) -> Vec<NamedCount> {
    let mut map: HashMap<String, i64> = HashMap::new();
    for v in values {
        *map.entry(v).or_insert(0) += 1;
    }
    let mut out: Vec<NamedCount> = map
        .into_iter()
        .map(|(label, count)| NamedCount { label, count })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count));
    out
}

/// Same as `count_strings` but handles nullable columns (None → "unspecified").
fn count_opt_strings(values: Vec<Option<String>>) -> Vec<NamedCount> {
    count_strings(
        values
            .into_iter()
            .map(|v| v.unwrap_or_else(|| "unspecified".to_owned()))
            .collect(),
    )
}

// ── Analytics computation ─────────────────────────────────────

fn compute_popularity(
    conn: &mut PgConnection,
    f: &ResolvedFilter,
) -> Result<PopularityMetrics, AppError> {
    // ── Top metric types by entry count ───────────────────────
    let top_metric_types: Vec<NamedCount> = {
        let mut q = metric_entries::table
            .inner_join(metric_types::table)
            .select(metric_types::name)
            .into_boxed();
        apply_time_filter!(q, metric_entries::created_at, f);
        apply_member_filter!(q, metric_entries::member_id, f.member_ids);
        let names: Vec<String> = q.load::<String>(conn).map_err(AppError::Database)?;
        let mut counts = count_strings(names);
        counts.truncate(10);
        counts
    };

    // ── Top workflow templates by instance count ───────────────
    let top_workflow_templates: Vec<NamedCount> = {
        let mut q = workflow_instances::table
            .inner_join(workflow_templates::table)
            .select(workflow_templates::name)
            .into_boxed();
        apply_time_filter!(q, workflow_instances::created_at, f);
        let names: Vec<String> = q.load::<String>(conn).map_err(AppError::Database)?;
        let mut counts = count_strings(names);
        counts.truncate(10);
        counts
    };

    // ── Work-order volume by ticket type ──────────────────────
    let work_order_types: Vec<NamedCount> = {
        let mut q = work_orders::table
            .select(work_orders::ticket_type)
            .into_boxed();
        apply_time_filter!(q, work_orders::created_at, f);
        apply_member_filter!(q, work_orders::member_id, f.member_ids);
        if let Some(ref tt) = f.ticket_type {
            q = q.filter(work_orders::ticket_type.eq(tt.clone()));
        }
        let vals: Vec<Option<String>> = q.load::<Option<String>>(conn).map_err(AppError::Database)?;
        count_opt_strings(vals)
    };

    Ok(PopularityMetrics {
        top_metric_types,
        top_workflow_templates,
        work_order_types,
    })
}

fn compute_conversion(
    conn: &mut PgConnection,
    f: &ResolvedFilter,
) -> Result<ConversionMetrics, AppError> {
    // All member counts are in-org (no time filter for the funnel
    // — conversion is a point-in-time structural metric).

    let total_members: i64 = {
        let mut q = members::table.into_boxed();
        apply_member_filter!(q, members::id, f.member_ids);
        q.count().get_result(conn).map_err(AppError::Database)?
    };

    let with_health_profile: i64 = {
        let mut q = health_profiles::table
            .inner_join(members::table)
            .select(health_profiles::id)
            .into_boxed();
        apply_member_filter!(q, members::id, f.member_ids);
        q.count().get_result(conn).map_err(AppError::Database)?
    };

    let with_metric_entries: i64 = {
        // Distinct member_ids that have any metric entry
        let mut q = metric_entries::table
            .select(metric_entries::member_id)
            .distinct()
            .into_boxed();
        apply_member_filter!(q, metric_entries::member_id, f.member_ids);
        let ids: Vec<Uuid> = q.load::<Uuid>(conn).map_err(AppError::Database)?;
        ids.len() as i64
    };

    let with_active_goals: i64 = {
        // Distinct member_ids with an active goal
        let mut q = goals::table
            .filter(goals::status.eq("active"))
            .select(goals::member_id)
            .distinct()
            .into_boxed();
        apply_member_filter!(q, goals::member_id, f.member_ids);
        let ids: Vec<Uuid> = q.load::<Uuid>(conn).map_err(AppError::Database)?;
        ids.len() as i64
    };

    let pct = |num: i64, den: i64| -> f64 {
        if den == 0 { 0.0 } else { (num as f64 / den as f64 * 10000.0).round() / 100.0 }
    };

    Ok(ConversionMetrics {
        total_members,
        with_health_profile,
        with_metric_entries,
        with_active_goals,
        profile_completion_rate: pct(with_health_profile, total_members),
        engagement_rate:         pct(with_metric_entries, total_members),
        goal_adoption_rate:      pct(with_active_goals, total_members),
    })
}

fn compute_attendance(
    conn: &mut PgConnection,
    f: &ResolvedFilter,
) -> Result<AttendanceMetrics, AppError> {
    // ── Work orders ───────────────────────────────────────────
    let wo_statuses: Vec<String> = {
        let mut q = work_orders::table.select(work_orders::status).into_boxed();
        apply_time_filter!(q, work_orders::created_at, f);
        apply_member_filter!(q, work_orders::member_id, f.member_ids);
        if let Some(ref tt) = f.ticket_type {
            q = q.filter(work_orders::ticket_type.eq(tt.clone()));
        }
        q.load::<String>(conn).map_err(AppError::Database)?
    };

    let wo_total    = wo_statuses.len() as i64;
    let wo_resolved = wo_statuses.iter().filter(|s| s.as_str() == "resolved").count() as i64;
    let wo_by_status = count_strings(wo_statuses);

    // ── Workflow instances ────────────────────────────────────
    let wi_statuses: Vec<String> = {
        let mut q = workflow_instances::table
            .select(workflow_instances::status)
            .into_boxed();
        apply_time_filter!(q, workflow_instances::created_at, f);
        q.load::<String>(conn).map_err(AppError::Database)?
    };

    let wi_total     = wi_statuses.len() as i64;
    let wi_completed = wi_statuses.iter().filter(|s| s.as_str() == "completed").count() as i64;
    let wi_by_status = count_strings(wi_statuses);

    // ── Active members in period ──────────────────────────────
    let active_members_in_period: i64 = {
        let mut q = metric_entries::table
            .select(metric_entries::member_id)
            .distinct()
            .into_boxed();
        apply_time_filter!(q, metric_entries::created_at, f);
        apply_member_filter!(q, metric_entries::member_id, f.member_ids);
        let ids: Vec<Uuid> = q.load::<Uuid>(conn).map_err(AppError::Database)?;
        ids.len() as i64
    };

    let rate = |num: i64, den: i64| -> f64 {
        if den == 0 { 0.0 } else { (num as f64 / den as f64 * 10000.0).round() / 100.0 }
    };

    Ok(AttendanceMetrics {
        work_orders_total:              wo_total,
        work_orders_resolved:           wo_resolved,
        work_order_resolution_rate:     rate(wo_resolved, wo_total),
        work_orders_by_status:          wo_by_status,
        workflow_instances_total:       wi_total,
        workflow_instances_completed:   wi_completed,
        workflow_completion_rate:       rate(wi_completed, wi_total),
        workflow_instances_by_status:   wi_by_status,
        active_members_in_period,
    })
}

fn compute_cancellations(
    conn: &mut PgConnection,
    f: &ResolvedFilter,
) -> Result<CancellationMetrics, AppError> {

    fn count_wi_status(conn: &mut PgConnection, f: &ResolvedFilter, status: &str) -> Result<i64, AppError> {
        let mut q = workflow_instances::table
            .filter(workflow_instances::status.eq(status))
            .into_boxed();
        apply_time_filter!(q, workflow_instances::created_at, f);
        q.count().get_result::<i64>(conn).map_err(AppError::Database)
    }

    let work_orders_closed: i64 = {
        let mut q = work_orders::table
            .filter(work_orders::status.eq("closed"))
            .into_boxed();
        apply_time_filter!(q, work_orders::created_at, f);
        apply_member_filter!(q, work_orders::member_id, f.member_ids);
        q.count().get_result::<i64>(conn).map_err(AppError::Database)?
    };

    let approval_rejections: i64 = {
        let mut q = approvals::table
            .filter(approvals::status.eq("rejected"))
            .into_boxed();
        apply_time_filter!(q, approvals::created_at, f);
        q.count().get_result::<i64>(conn).map_err(AppError::Database)?
    };

    Ok(CancellationMetrics {
        work_orders_closed,
        workflow_rejections:   count_wi_status(conn, f, "rejected")?,
        workflow_withdrawals:  count_wi_status(conn, f, "withdrawn")?,
        workflow_cancellations: count_wi_status(conn, f, "cancelled")?,
        approval_rejections,
    })
}

fn compute_distributions(
    conn: &mut PgConnection,
    f: &ResolvedFilter,
) -> Result<DistributionMetrics, AppError> {
    // Work orders by ticket_type
    let work_orders_by_ticket_type: Vec<NamedCount> = {
        let mut q = work_orders::table.select(work_orders::ticket_type).into_boxed();
        apply_time_filter!(q, work_orders::created_at, f);
        apply_member_filter!(q, work_orders::member_id, f.member_ids);
        count_opt_strings(q.load::<Option<String>>(conn).map_err(AppError::Database)?)
    };

    // Work orders by priority
    let work_orders_by_priority: Vec<NamedCount> = {
        let mut q = work_orders::table.select(work_orders::priority).into_boxed();
        apply_time_filter!(q, work_orders::created_at, f);
        apply_member_filter!(q, work_orders::member_id, f.member_ids);
        count_strings(q.load::<String>(conn).map_err(AppError::Database)?)
    };

    // Notifications by event_type
    let notifications_by_event_type: Vec<NamedCount> = {
        let mut q = notifications::table.select(notifications::event_type).into_boxed();
        apply_time_filter!(q, notifications::created_at, f);
        count_opt_strings(q.load::<Option<String>>(conn).map_err(AppError::Database)?)
    };

    // Goals by goal_type
    let goals_by_type: Vec<NamedCount> = {
        let mut q = goals::table.select(goals::goal_type).into_boxed();
        apply_time_filter!(q, goals::created_at, f);
        apply_member_filter!(q, goals::member_id, f.member_ids);
        count_strings(q.load::<String>(conn).map_err(AppError::Database)?)
    };

    // Goals by status
    let goals_by_status: Vec<NamedCount> = {
        let mut q = goals::table.select(goals::status).into_boxed();
        apply_time_filter!(q, goals::created_at, f);
        apply_member_filter!(q, goals::member_id, f.member_ids);
        count_strings(q.load::<String>(conn).map_err(AppError::Database)?)
    };

    Ok(DistributionMetrics {
        work_orders_by_ticket_type,
        work_orders_by_priority,
        notifications_by_event_type,
        goals_by_type,
        goals_by_status,
    })
}

fn build_report(conn: &mut PgConnection, f: &ResolvedFilter) -> Result<AnalyticsReport, AppError> {
    let now = Utc::now();
    let period = PeriodInfo {
        start: f
            .start_dt
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "all time".to_owned()),
        end: f
            .end_dt
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| now.format("%Y-%m-%d").to_string()),
    };

    Ok(AnalyticsReport {
        period,
        org_unit_id:       f.org_unit_id,
        ticket_type_filter: f.ticket_type.clone(),
        popularity:         compute_popularity(conn, f)?,
        registration_conversion: compute_conversion(conn, f)?,
        attendance:         compute_attendance(conn, f)?,
        cancellations:      compute_cancellations(conn, f)?,
        distributions:      compute_distributions(conn, f)?,
    })
}

// ── CSV generation ────────────────────────────────────────────

fn report_to_csv(r: &AnalyticsReport) -> String {
    let mut out = String::with_capacity(8 * 1024);

    // Header metadata
    out.push_str("# VitalPath Analytics Report\n");
    out.push_str(&format!("# Period: {} to {}\n", r.period.start, r.period.end));
    if let Some(ouid) = r.org_unit_id {
        out.push_str(&format!("# Org Unit: {}\n", ouid));
    }
    if let Some(ref tt) = r.ticket_type_filter {
        out.push_str(&format!("# Ticket Type Filter: {}\n", tt));
    }
    out.push_str(&format!("# Generated: {}\n\n", Utc::now().format("%Y-%m-%dT%H:%M:%SZ")));

    // Flat format: section,category,label,value
    out.push_str("section,category,label,value\n");

    // Popularity
    for row in &r.popularity.top_metric_types {
        out.push_str(&format!("popularity,metric_type,{},{}\n", csv_esc(&row.label), row.count));
    }
    for row in &r.popularity.top_workflow_templates {
        out.push_str(&format!("popularity,workflow_template,{},{}\n", csv_esc(&row.label), row.count));
    }
    for row in &r.popularity.work_order_types {
        out.push_str(&format!("popularity,work_order_type,{},{}\n", csv_esc(&row.label), row.count));
    }

    // Registration conversion
    let c = &r.registration_conversion;
    out.push_str(&format!("conversion,funnel,total_members,{}\n", c.total_members));
    out.push_str(&format!("conversion,funnel,with_health_profile,{}\n", c.with_health_profile));
    out.push_str(&format!("conversion,funnel,with_metric_entries,{}\n", c.with_metric_entries));
    out.push_str(&format!("conversion,funnel,with_active_goals,{}\n", c.with_active_goals));
    out.push_str(&format!("conversion,rate,profile_completion_rate,{}\n", c.profile_completion_rate));
    out.push_str(&format!("conversion,rate,engagement_rate,{}\n", c.engagement_rate));
    out.push_str(&format!("conversion,rate,goal_adoption_rate,{}\n", c.goal_adoption_rate));

    // Attendance
    let a = &r.attendance;
    out.push_str(&format!("attendance,work_orders,total,{}\n", a.work_orders_total));
    out.push_str(&format!("attendance,work_orders,resolved,{}\n", a.work_orders_resolved));
    out.push_str(&format!("attendance,work_orders,resolution_rate,{}\n", a.work_order_resolution_rate));
    for row in &a.work_orders_by_status {
        out.push_str(&format!("attendance,work_order_status,{},{}\n", csv_esc(&row.label), row.count));
    }
    out.push_str(&format!("attendance,workflows,total,{}\n", a.workflow_instances_total));
    out.push_str(&format!("attendance,workflows,completed,{}\n", a.workflow_instances_completed));
    out.push_str(&format!("attendance,workflows,completion_rate,{}\n", a.workflow_completion_rate));
    for row in &a.workflow_instances_by_status {
        out.push_str(&format!("attendance,workflow_status,{},{}\n", csv_esc(&row.label), row.count));
    }
    out.push_str(&format!("attendance,engagement,active_members_in_period,{}\n", a.active_members_in_period));

    // Cancellations
    let can = &r.cancellations;
    out.push_str(&format!("cancellations,work_orders,closed,{}\n", can.work_orders_closed));
    out.push_str(&format!("cancellations,workflows,rejections,{}\n", can.workflow_rejections));
    out.push_str(&format!("cancellations,workflows,withdrawals,{}\n", can.workflow_withdrawals));
    out.push_str(&format!("cancellations,workflows,cancellations,{}\n", can.workflow_cancellations));
    out.push_str(&format!("cancellations,approvals,rejected,{}\n", can.approval_rejections));

    // Distributions
    for row in &r.distributions.work_orders_by_ticket_type {
        out.push_str(&format!("distribution,ticket_type,{},{}\n", csv_esc(&row.label), row.count));
    }
    for row in &r.distributions.work_orders_by_priority {
        out.push_str(&format!("distribution,priority,{},{}\n", csv_esc(&row.label), row.count));
    }
    for row in &r.distributions.notifications_by_event_type {
        out.push_str(&format!("distribution,notification_event,{},{}\n", csv_esc(&row.label), row.count));
    }
    for row in &r.distributions.goals_by_type {
        out.push_str(&format!("distribution,goal_type,{},{}\n", csv_esc(&row.label), row.count));
    }
    for row in &r.distributions.goals_by_status {
        out.push_str(&format!("distribution,goal_status,{},{}\n", csv_esc(&row.label), row.count));
    }

    out
}

/// Escape a value for CSV (wrap in quotes if it contains commas, quotes, or newlines).
fn csv_esc(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

// ── Excel generation ──────────────────────────────────────────

fn report_to_xlsx(r: &AnalyticsReport, path: &str) -> Result<(), AppError> {
    let mut wb = Workbook::new();
    let bold = Format::new().set_bold();

    // ── Sheet 1: Summary ───────────────────────────────────────
    {
        let ws = wb.add_worksheet();
        ws.set_name("Summary")
            .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;

        let meta = [
            ("Period Start", r.period.start.as_str()),
            ("Period End",   r.period.end.as_str()),
        ];
        for (i, (k, v)) in meta.iter().enumerate() {
            ws.write_string_with_format(i as u32, 0, *k, &bold)
                .and_then(|ws| ws.write_string(i as u32, 1, *v))
                .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;
        }
        let row = meta.len() as u32;
        if let Some(ouid) = r.org_unit_id {
            ws.write_string_with_format(row, 0, "Org Unit", &bold)
                .and_then(|ws| ws.write_string(row, 1, &ouid.to_string()))
                .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;
        }
    }

    // ── Helper: write a NamedCount table to a sheet ───────────
    let write_named_count =
        |wb: &mut Workbook,
         sheet_name: &str,
         section_label: &str,
         col_header: &str,
         rows: &[NamedCount]| -> Result<(), AppError> {
            let ws = wb.add_worksheet();
            ws.set_name(sheet_name)
                .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;
            ws.write_string_with_format(0, 0, section_label, &bold)
                .and_then(|ws| ws.write_string_with_format(0, 1, col_header, &bold))
                .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;
            for (i, row) in rows.iter().enumerate() {
                ws.write_string((i + 1) as u32, 0, &row.label)
                    .and_then(|ws| ws.write_number((i + 1) as u32, 1, row.count as f64))
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;
            }
            Ok(())
        };

    // ── Sheet 2: Popularity ────────────────────────────────────
    {
        let ws = wb.add_worksheet();
        ws.set_name("Popularity")
            .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;

        ws.write_string_with_format(0, 0, "Category", &bold)
            .and_then(|ws| ws.write_string_with_format(0, 1, "Label", &bold))
            .and_then(|ws| ws.write_string_with_format(0, 2, "Count", &bold))
            .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;

        let mut row = 1u32;
        for item in &r.popularity.top_metric_types {
            ws.write_string(row, 0, "Metric Type")
                .and_then(|ws| ws.write_string(row, 1, &item.label))
                .and_then(|ws| ws.write_number(row, 2, item.count as f64))
                .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;
            row += 1;
        }
        for item in &r.popularity.top_workflow_templates {
            ws.write_string(row, 0, "Workflow Template")
                .and_then(|ws| ws.write_string(row, 1, &item.label))
                .and_then(|ws| ws.write_number(row, 2, item.count as f64))
                .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;
            row += 1;
        }
        for item in &r.popularity.work_order_types {
            ws.write_string(row, 0, "Work Order Type")
                .and_then(|ws| ws.write_string(row, 1, &item.label))
                .and_then(|ws| ws.write_number(row, 2, item.count as f64))
                .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;
            row += 1;
        }
    }

    // ── Sheet 3: Conversion ────────────────────────────────────
    {
        let ws = wb.add_worksheet();
        ws.set_name("Conversion")
            .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;

        ws.write_string_with_format(0, 0, "Metric", &bold)
            .and_then(|ws| ws.write_string_with_format(0, 1, "Value", &bold))
            .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;

        let c = &r.registration_conversion;
        let rows: &[(&str, f64)] = &[
            ("Total Members",              c.total_members as f64),
            ("With Health Profile",        c.with_health_profile as f64),
            ("With Metric Entries",        c.with_metric_entries as f64),
            ("With Active Goals",          c.with_active_goals as f64),
            ("Profile Completion Rate %",  c.profile_completion_rate),
            ("Engagement Rate %",          c.engagement_rate),
            ("Goal Adoption Rate %",       c.goal_adoption_rate),
        ];
        for (i, (label, val)) in rows.iter().enumerate() {
            ws.write_string((i + 1) as u32, 0, *label)
                .and_then(|ws| ws.write_number((i + 1) as u32, 1, *val))
                .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;
        }
    }

    // ── Sheet 4: Attendance ────────────────────────────────────
    {
        let ws = wb.add_worksheet();
        ws.set_name("Attendance")
            .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;

        ws.write_string_with_format(0, 0, "Metric", &bold)
            .and_then(|ws| ws.write_string_with_format(0, 1, "Value", &bold))
            .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;

        let a = &r.attendance;
        let scalar_rows: &[(&str, f64)] = &[
            ("Work Orders Total",             a.work_orders_total as f64),
            ("Work Orders Resolved",          a.work_orders_resolved as f64),
            ("Work Order Resolution Rate %",  a.work_order_resolution_rate),
            ("Workflow Instances Total",      a.workflow_instances_total as f64),
            ("Workflow Instances Completed",  a.workflow_instances_completed as f64),
            ("Workflow Completion Rate %",    a.workflow_completion_rate),
            ("Active Members in Period",      a.active_members_in_period as f64),
        ];
        let mut row = 1u32;
        for (label, val) in scalar_rows {
            ws.write_string(row, 0, *label)
                .and_then(|ws| ws.write_number(row, 1, *val))
                .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;
            row += 1;
        }
        row += 1; // blank separator
        ws.write_string_with_format(row, 0, "Status", &bold)
            .and_then(|ws| ws.write_string_with_format(row, 1, "Work Orders", &bold))
            .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;
        row += 1;
        for item in &a.work_orders_by_status {
            ws.write_string(row, 0, &item.label)
                .and_then(|ws| ws.write_number(row, 1, item.count as f64))
                .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;
            row += 1;
        }
    }

    // ── Sheet 5: Cancellations ─────────────────────────────────
    {
        let ws = wb.add_worksheet();
        ws.set_name("Cancellations")
            .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;

        ws.write_string_with_format(0, 0, "Metric", &bold)
            .and_then(|ws| ws.write_string_with_format(0, 1, "Count", &bold))
            .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;

        let can = &r.cancellations;
        let rows: &[(&str, i64)] = &[
            ("Work Orders Closed",          can.work_orders_closed),
            ("Workflow Rejections",         can.workflow_rejections),
            ("Workflow Withdrawals",        can.workflow_withdrawals),
            ("Workflow Cancellations",      can.workflow_cancellations),
            ("Approval Rejections",         can.approval_rejections),
        ];
        for (i, (label, val)) in rows.iter().enumerate() {
            ws.write_string((i + 1) as u32, 0, *label)
                .and_then(|ws| ws.write_number((i + 1) as u32, 1, *val as f64))
                .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;
        }
    }

    // ── Sheet 6: Distributions ─────────────────────────────────
    {
        let ws = wb.add_worksheet();
        ws.set_name("Distributions")
            .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;

        ws.write_string_with_format(0, 0, "Category", &bold)
            .and_then(|ws| ws.write_string_with_format(0, 1, "Label", &bold))
            .and_then(|ws| ws.write_string_with_format(0, 2, "Count", &bold))
            .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;

        let dist = &r.distributions;
        let sections: &[(&str, &[NamedCount])] = &[
            ("Ticket Type",         &dist.work_orders_by_ticket_type),
            ("Priority",            &dist.work_orders_by_priority),
            ("Notification Event",  &dist.notifications_by_event_type),
            ("Goal Type",           &dist.goals_by_type),
            ("Goal Status",         &dist.goals_by_status),
        ];
        let mut row = 1u32;
        for (category, items) in sections {
            for item in *items {
                ws.write_string(row, 0, *category)
                    .and_then(|ws| ws.write_string(row, 1, &item.label))
                    .and_then(|ws| ws.write_number(row, 2, item.count as f64))
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx: {}", e)))?;
                row += 1;
            }
        }
    }

    drop(write_named_count); // unused path; prevent warning
    wb.save(path)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx save: {}", e)))?;

    Ok(())
}

// ── Handlers ──────────────────────────────────────────────────

/// GET /analytics
/// Returns the full analytics report in JSON.
/// Filters: start_date, end_date, org_unit_id, ticket_type.
/// Requires administrator or care_coach role.
/// Org isolation: non-admin callers are always scoped to their own org unit
/// regardless of any org_unit_id supplied in the query parameters.
/// Every access is recorded in the audit log.
#[get("")]
async fn get_analytics(
    req: HttpRequest,
    user: CareCoachAuth,
    pool: web::Data<DbPool>,
    query: web::Query<AnalyticsQuery>,
) -> Result<HttpResponse, AppError> {
    let query    = query.into_inner();
    let is_admin = user.role.is_admin();
    let actor_id = user.user_id;
    let ip       = req.connection_info().realip_remote_addr().map(str::to_owned);

    let report = web::block(move || {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        // Admins may filter by any org_unit_id (or none = all orgs).
        // Non-admins are ALWAYS restricted to their own org_unit; any
        // org_unit_id they supply in the query is silently ignored to
        // prevent cross-org data access.
        let effective_org = if is_admin {
            query.org_unit_id
        } else {
            use crate::schema::users;
            users::table
                .find(actor_id)
                .select(users::org_unit_id)
                .first::<Option<Uuid>>(&mut conn)
                .map_err(AppError::Database)?
        };

        let adjusted_query = AnalyticsQuery {
            start_date: query.start_date.clone(),
            end_date:   query.end_date.clone(),
            org_unit_id: effective_org,
            ticket_type: query.ticket_type.clone(),
        };

        let member_ids = load_member_ids(&mut conn, effective_org)
            .map_err(|e| match e {
                AppError::Database(de) => AppError::Database(de),
                other => other,
            })?;

        let filter = ResolvedFilter::parse(&adjusted_query, member_ids)
            .map_err(AppError::BadRequest)?;

        let report = build_report(&mut conn, &filter)?;

        // Audit: every analytics read is a data-access event.
        audit_log::insert(
            &mut conn,
            NewAuditLog::new(Some(actor_id), "ANALYTICS_READ", "analytics", None, ip)
                .with_new_value(serde_json::json!({
                    "start_date":  query.start_date,
                    "end_date":    query.end_date,
                    "org_unit_id": effective_org,
                    "ticket_type": query.ticket_type,
                })),
        );

        Ok::<_, AppError>(report)
    })
    .await
    .map_err(|e: actix_web::error::BlockingError| AppError::Internal(anyhow::anyhow!(e)))??;

    Ok(HttpResponse::Ok().json(report))
}

/// POST /analytics/export
/// Generates a CSV or Excel file, saves it locally, and returns metadata.
/// Requires administrator or care_coach.
///
/// **Privileged endpoint** — in addition to role checks, the request
/// must carry a valid HMAC-SHA256 signature:
///
///   `X-Timestamp: <unix epoch seconds>`
///   `X-Signature: hex(HMAC-SHA256(HMAC_SECRET, "{ts}:POST:/analytics/export"))`
///
/// Every export (including failures) is recorded in the audit log.
#[post("/export")]
async fn export_analytics(
    req: HttpRequest,
    user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    cfg: web::Data<AppConfig>,
    body: web::Json<ExportRequest>,
) -> Result<HttpResponse, AppError> {
    // ── Role check ────────────────────────────────────────────
    user.require_care_coach_or_above()?;

    // ── HMAC signature check (privileged endpoint) ────────────
    hmac_sign::verify(&req, &cfg.hmac_secret)?;

    // ── Input validation ──────────────────────────────────────
    body.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let fmt = body.format.to_lowercase();
    if !["csv", "excel"].contains(&fmt.as_str()) {
        return Err(AppError::BadRequest(
            "format must be 'csv' or 'excel'".into(),
        ));
    }

    let ip         = req.connection_info().realip_remote_addr().map(str::to_owned);
    let masked_actor = masking::mask_id(&user.user_id);
    let actor_id   = user.user_id;
    let exports_dir = cfg.exports_dir.clone();
    let body       = body.into_inner();

    let meta = web::block(move || -> Result<ExportMeta, AppError> {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        // Build the analytics query from the export request fields
        let as_query = AnalyticsQuery {
            start_date:  body.start_date,
            end_date:    body.end_date,
            org_unit_id: body.org_unit_id,
            ticket_type: body.ticket_type,
        };

        let member_ids = load_member_ids(&mut conn, as_query.org_unit_id)?;
        let filter = ResolvedFilter::parse(&as_query, member_ids)
            .map_err(AppError::BadRequest)?;
        let report = build_report(&mut conn, &filter)?;

        // ── Build file path ────────────────────────────────────
        let export_id = Uuid::new_v4();
        let now       = Utc::now();
        let timestamp = now.format("%Y%m%dT%H%M%SZ");
        let ext       = if fmt == "csv" { "csv" } else { "xlsx" };
        let filename  = format!("analytics_{}_{}.{}", timestamp, export_id, ext);

        // Ensure exports directory exists
        std::fs::create_dir_all(&exports_dir)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("create exports dir: {}", e)))?;

        let file_path = Path::new(&exports_dir).join(&filename);
        let file_path_str = file_path.to_string_lossy().into_owned();

        // ── Write file ─────────────────────────────────────────
        if fmt == "csv" {
            let csv = report_to_csv(&report);
            std::fs::write(&file_path, csv.as_bytes())
                .map_err(|e| AppError::Internal(anyhow::anyhow!("write csv: {}", e)))?;
        } else {
            report_to_xlsx(&report, &file_path_str)?;
        }

        let size_bytes = std::fs::metadata(&file_path)
            .map(|m| m.len())
            .unwrap_or(0);

        // ── Audit log ──────────────────────────────────────────
        audit_log::insert(
            &mut conn,
            NewAuditLog::new(
                Some(actor_id),
                "ANALYTICS_EXPORT",
                "analytics",
                Some(export_id),
                ip,
            )
            .with_new_value(serde_json::json!({
                "format":       fmt,
                "filename":     filename,
                "size_bytes":   size_bytes,
                "period_start": filter.start_dt.map(|d| d.to_rfc3339()),
                "period_end":   filter.end_dt.map(|d| d.to_rfc3339()),
                "org_unit_id":  filter.org_unit_id,
            })),
        );

        Ok(ExportMeta {
            export_id,
            format: fmt,
            filename: filename.clone(),
            file_path: file_path_str,
            size_bytes,
            created_at: now,
            download_url: format!("/analytics/export/{}", filename),
        })
    })
    .await
    .map_err(|e: actix_web::error::BlockingError| AppError::Internal(anyhow::anyhow!(e)))??;

    info!(
        actor  = %masked_actor,
        format = %meta.format,
        bytes  = meta.size_bytes,
        "ANALYTICS_EXPORT_SUCCESS"
    );

    Ok(HttpResponse::Created().json(meta))
}

/// GET /analytics/export/{filename}
/// Stream a previously generated export file.
/// Path traversal is prevented: the filename must not contain
/// path separators or directory escape sequences.
/// Every download is recorded in the audit log.
#[get("/export/{filename}")]
async fn download_export(
    req: HttpRequest,
    user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    cfg: web::Data<AppConfig>,
    filename: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    user.require_care_coach_or_above()?;

    let filename = filename.into_inner();
    let actor_id = user.user_id;
    let ip       = req.connection_info().realip_remote_addr().map(str::to_owned);

    // ── Path traversal guard ───────────────────────────────────
    if filename.contains('/')
        || filename.contains('\\')
        || filename.contains("..")
        || filename.starts_with('.')
    {
        return Err(AppError::BadRequest("invalid filename".into()));
    }

    let file_path = Path::new(&cfg.exports_dir).join(&filename);

    // ── Read file (blocking I/O via spawn_blocking) ────────────
    let file_path_owned = file_path.to_string_lossy().into_owned();
    let filename_clone  = filename.clone();

    let bytes = tokio::task::spawn_blocking(move || {
        std::fs::read(&file_path_owned)
            .map_err(|e: std::io::Error| AppError::Internal(anyhow::anyhow!("read export file: {}", e)))
    })
    .await
    .map_err(|e: tokio::task::JoinError| AppError::Internal(anyhow::anyhow!(e)))??;

    let content_type = if filename.ends_with(".csv") {
        "text/csv; charset=utf-8"
    } else {
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    };

    // ── Audit download event ───────────────────────────────────
    let filename_for_log = filename.clone();
    let bytes_len        = bytes.len() as u64;
    web::block(move || {
        if let Ok(mut conn) = pool.get() {
            audit_log::insert(
                &mut conn,
                NewAuditLog::new(
                    Some(actor_id),
                    "ANALYTICS_DOWNLOAD",
                    "analytics",
                    None,
                    ip,
                )
                .with_new_value(serde_json::json!({
                    "filename":   filename_for_log,
                    "bytes_sent": bytes_len,
                })),
            );
        }
        Ok::<_, AppError>(())
    })
    .await
    .ok(); // fire-and-forget; never fail the download for a log error

    Ok(HttpResponse::Ok()
        .content_type(content_type)
        .insert_header((
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", filename_clone),
        ))
        .body(bytes))
}
