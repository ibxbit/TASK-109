use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::{errors::AppError, schema::goals};

// ── Goal-type catalogue ──────────────────────────────────────

pub const VALID_GOAL_TYPES: &[&str] = &["fat_loss", "muscle_gain", "glucose_control"];
pub const VALID_STATUSES: &[&str]   = &["active", "paused", "completed", "cancelled"];

/// Maps a goal type to the metric type name used for auto-evaluation.
pub fn goal_metric_name(goal_type: &str) -> Option<&'static str> {
    match goal_type {
        "fat_loss"        => Some("body_fat_percentage"),
        "muscle_gain"     => Some("weight"),
        "glucose_control" => Some("blood_glucose"),
        _                 => None,
    }
}

/// Returns true when `value` satisfies the completion condition.
///
/// - fat_loss / glucose_control → want to *decrease* → value ≤ target
/// - muscle_gain               → want to *increase* → value ≥ target
pub fn target_met(goal_type: &str, value: f64, target: f64) -> bool {
    match goal_type {
        "fat_loss" | "glucose_control" => value <= target,
        "muscle_gain"                  => value >= target,
        _                              => false,
    }
}

/// Validates that target moves in the right direction relative to baseline.
pub fn validate_goal_direction(
    goal_type: &str,
    baseline: f64,
    target: f64,
) -> Result<(), AppError> {
    let ok = match goal_type {
        "fat_loss" | "glucose_control" => target < baseline,
        "muscle_gain"                  => target > baseline,
        _                              => true,
    };
    if !ok {
        let direction = if goal_type == "muscle_gain" { "greater than" } else { "less than" };
        return Err(AppError::BadRequest(format!(
            "For '{}' goals, target_value ({:.2}) must be {} baseline_value ({:.2})",
            goal_type, target, direction, baseline
        )));
    }
    Ok(())
}

// ── DB model ─────────────────────────────────────────────────
// Column order must match schema.rs exactly.

#[derive(Debug, Clone, Queryable, Selectable, Identifiable)]
#[diesel(table_name = goals)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Goal {
    pub id: Uuid,
    pub member_id: Uuid,
    pub metric_type_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub target_value: Option<f64>,
    pub target_date: Option<NaiveDate>,
    pub status: String,
    pub assigned_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // Added by migration 00006
    pub goal_type: String,
    pub start_date: NaiveDate,
    pub baseline_value: f64,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = goals)]
pub struct NewGoal {
    pub id: Uuid,
    pub member_id: Uuid,
    pub metric_type_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub target_value: Option<f64>,
    pub target_date: Option<NaiveDate>,
    pub status: String,
    pub assigned_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub goal_type: String,
    pub start_date: NaiveDate,
    pub baseline_value: f64,
}

/// Full update changeset (Admin / Care Coach only).
#[derive(Debug, AsChangeset)]
#[diesel(table_name = goals)]
pub struct GoalChangeset {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub target_value: Option<Option<f64>>,
    pub target_date: Option<Option<NaiveDate>>,
    pub status: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// Status-only changeset (used by auto-evaluation and Member self-service).
#[derive(Debug, AsChangeset)]
#[diesel(table_name = goals)]
pub struct GoalStatusUpdate {
    pub status: String,
    pub updated_at: DateTime<Utc>,
}

// ── API request shapes ────────────────────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct CreateGoalRequest {
    pub member_id: Uuid,

    /// fat_loss | muscle_gain | glucose_control
    pub goal_type: String,

    #[validate(length(min = 1, max = 200))]
    pub title: String,

    #[validate(length(max = 1000))]
    pub description: Option<String>,

    /// YYYY-MM-DD — must not be in the future.
    pub start_date: String,

    /// YYYY-MM-DD — must be after start_date.
    pub target_date: Option<String>,

    /// Member's current metric value (starting point).
    pub baseline_value: f64,

    /// The value the member is aiming to reach.
    pub target_value: f64,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateGoalRequest {
    #[validate(length(min = 1, max = 200))]
    pub title: Option<String>,

    #[validate(length(max = 1000))]
    pub description: Option<String>,

    /// YYYY-MM-DD
    pub target_date: Option<String>,

    pub target_value: Option<f64>,

    /// active | paused | completed | cancelled
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GoalsQuery {
    pub member_id: Uuid,
    /// Optional filter: active | paused | completed | cancelled
    pub status: Option<String>,
}

// ── API response shapes ───────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct GoalResponse {
    pub id: Uuid,
    pub member_id: Uuid,
    pub goal_type: String,
    /// Metric type consumed during auto-evaluation.
    pub tracked_metric: String,
    pub title: String,
    pub description: Option<String>,
    pub start_date: NaiveDate,
    pub target_date: Option<NaiveDate>,
    pub baseline_value: f64,
    pub target_value: f64,
    pub status: String,
    pub assigned_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl GoalResponse {
    pub fn from_goal(g: Goal) -> Self {
        let tracked_metric = goal_metric_name(&g.goal_type)
            .unwrap_or("unknown")
            .to_owned();
        Self {
            id: g.id,
            member_id: g.member_id,
            goal_type: g.goal_type,
            tracked_metric,
            title: g.title,
            description: g.description,
            start_date: g.start_date,
            target_date: g.target_date,
            baseline_value: g.baseline_value,
            target_value: g.target_value.unwrap_or(0.0),
            status: g.status,
            assigned_by: g.assigned_by,
            created_at: g.created_at,
            updated_at: g.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GoalListResponse {
    pub member_id: Uuid,
    pub total: usize,
    pub goals: Vec<GoalResponse>,
}
