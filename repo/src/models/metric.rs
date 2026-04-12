use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::{errors::AppError, schema::{metric_entries, metric_types}};

// ── Metric type catalogue ────────────────────────────────────

/// (name, unit, value_min, value_max)
pub const METRIC_CATALOGUE: &[(&str, &str, f64, f64)] = &[
    ("weight",              "lbs",    10.0, 1500.0),
    ("body_fat_percentage", "%",       1.0,   70.0),
    ("waist",               "inches", 10.0,  120.0),
    ("hip",                 "inches", 10.0,  120.0),
    ("chest",               "inches", 10.0,  120.0),
    ("blood_glucose",       "mg/dL",  50.0,  600.0),
];

pub fn is_valid_metric_type(name: &str) -> bool {
    METRIC_CATALOGUE.iter().any(|(n, _, _, _)| *n == name)
}

/// Validates that `value` falls within the allowed range for `metric_type`.
#[allow(dead_code)]
pub fn validate_metric_value(metric_type: &str, value: f64) -> Result<(), AppError> {
    let entry = METRIC_CATALOGUE
        .iter()
        .find(|(n, _, _, _)| *n == metric_type)
        .ok_or_else(|| AppError::BadRequest(format!("Unknown metric type: {}", metric_type)))?;

    let (_, unit, min, max) = entry;
    if value < *min || value > *max {
        return Err(AppError::BadRequest(format!(
            "'{}' value must be between {:.1} and {:.1} {} (got {:.2})",
            metric_type, min, max, unit, value
        )));
    }
    Ok(())
}

// ── DB models ────────────────────────────────────────────────

#[derive(Debug, Queryable, Selectable, Identifiable)]
#[diesel(table_name = metric_types)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct MetricTypeRecord {
    pub id: Uuid,
    pub name: String,
    pub unit: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Queryable, Selectable, Identifiable)]
#[diesel(table_name = metric_entries)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct MetricEntry {
    pub id: Uuid,
    pub member_id: Uuid,
    pub metric_type_id: Uuid,
    pub value: f64,
    pub entry_date: NaiveDate,
    pub recorded_by: Uuid,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = metric_entries)]
pub struct NewMetricEntry {
    pub id: Uuid,
    pub member_id: Uuid,
    pub metric_type_id: Uuid,
    pub value: f64,
    pub entry_date: NaiveDate,
    pub recorded_by: Uuid,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ── Joined query row (metric_entries ⨝ metric_types) ────────

/// Flat row returned by list/summary queries that join both tables.
#[derive(Debug, Queryable)]
pub struct EntryWithType {
    pub id: Uuid,
    pub member_id: Uuid,
    pub metric_type_id: Uuid,
    pub metric_type_name: String,
    pub unit: String,
    pub value: f64,
    pub entry_date: NaiveDate,
    pub recorded_by: Uuid,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ── API request / response shapes ────────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct CreateMetricEntryRequest {
    pub member_id: Uuid,

    /// One of: weight, body_fat_percentage, waist, hip, chest
    #[validate(length(min = 1))]
    pub metric_type: String,

    /// Validated per-type range in the handler.
    pub value: f64,

    /// Defaults to today (UTC) if omitted.
    pub entry_date: Option<String>,

    #[validate(length(max = 500))]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MetricsQuery {
    pub member_id: Uuid,
    /// Shorthand: "7d" | "30d" | "90d" | "all"
    pub range: Option<String>,
    /// Explicit start date (YYYY-MM-DD). Used when range is absent.
    pub start: Option<String>,
    /// Explicit end date (YYYY-MM-DD). Used when range is absent.
    pub end: Option<String>,
    /// Optional filter to a single metric type name.
    pub metric_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SummaryQuery {
    pub member_id: Uuid,
    pub range: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MetricEntryResponse {
    pub id: Uuid,
    pub member_id: Uuid,
    pub metric_type: String,
    pub unit: String,
    pub value: f64,
    pub entry_date: NaiveDate,
    pub recorded_by: Uuid,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<EntryWithType> for MetricEntryResponse {
    fn from(r: EntryWithType) -> Self {
        Self {
            id: r.id,
            member_id: r.member_id,
            metric_type: r.metric_type_name,
            unit: r.unit,
            value: r.value,
            entry_date: r.entry_date,
            recorded_by: r.recorded_by,
            notes: r.notes,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MetricListResponse {
    pub member_id: Uuid,
    pub range_start: NaiveDate,
    pub range_end: NaiveDate,
    pub total: usize,
    pub entries: Vec<MetricEntryResponse>,
}

#[derive(Debug, Serialize)]
pub struct MetricSummaryItem {
    pub metric_type: String,
    pub unit: String,
    pub count: usize,
    pub first_date: Option<NaiveDate>,
    pub latest_date: Option<NaiveDate>,
    pub first_value: Option<f64>,
    pub latest_value: Option<f64>,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub avg_value: Option<f64>,
    /// Absolute change: latest − first
    pub change: Option<f64>,
    /// Relative change: (latest − first) / first × 100
    pub change_pct: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct MetricSummaryResponse {
    pub member_id: Uuid,
    pub range_start: NaiveDate,
    pub range_end: NaiveDate,
    pub summaries: Vec<MetricSummaryItem>,
}
