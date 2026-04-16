use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

// ── Filter query params ───────────────────────────────────────

/// Query parameters for GET /analytics and POST /analytics/export.
/// Dates are "YYYY-MM-DD" strings; parsed manually for reliable
/// query-string deserialization.
#[derive(Debug, Deserialize)]
pub struct AnalyticsQuery {
    /// Inclusive start date (YYYY-MM-DD). Defaults to the epoch.
    pub start_date: Option<String>,
    /// Inclusive end date (YYYY-MM-DD). Defaults to now.
    pub end_date: Option<String>,
    /// Restrict all metrics to one org unit's members.
    pub org_unit_id: Option<Uuid>,
    /// Additional work-order attribute filter.
    pub ticket_type: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ExportRequest {
    /// "csv" or "excel"
    #[validate(length(min = 1, max = 10))]
    pub format: String,
    /// Inclusive start date (YYYY-MM-DD).
    #[validate(length(min = 10, max = 10))]
    pub start_date: Option<String>,
    /// Inclusive end date (YYYY-MM-DD).
    #[validate(length(min = 10, max = 10))]
    pub end_date: Option<String>,
    pub org_unit_id: Option<Uuid>,
    #[validate(length(min = 1, max = 50))]
    pub ticket_type: Option<String>,
}

// ── Internal filter, built from the query ────────────────────

#[derive(Debug, Clone)]
pub struct ResolvedFilter {
    pub start_dt: Option<DateTime<Utc>>,
    pub end_dt: Option<DateTime<Utc>>,
    pub org_unit_id: Option<Uuid>,
    pub ticket_type: Option<String>,
    /// Pre-loaded member IDs for the org_unit (empty = no org filter).
    pub member_ids: Vec<Uuid>,
}

impl ResolvedFilter {
    pub fn parse(q: &AnalyticsQuery, member_ids: Vec<Uuid>) -> Result<Self, String> {
        let start_dt = q
            .start_date
            .as_deref()
            .map(|s| {
                NaiveDate::parse_from_str(s, "%Y-%m-%d")
                    .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
                    .map_err(|_| format!("invalid start_date '{}', expected YYYY-MM-DD", s))
            })
            .transpose()?;

        let end_dt = q
            .end_date
            .as_deref()
            .map(|s| {
                NaiveDate::parse_from_str(s, "%Y-%m-%d")
                    .map(|d| d.and_hms_opt(23, 59, 59).unwrap().and_utc())
                    .map_err(|_| format!("invalid end_date '{}', expected YYYY-MM-DD", s))
            })
            .transpose()?;

        Ok(Self {
            start_dt,
            end_dt,
            org_unit_id: q.org_unit_id,
            ticket_type: q.ticket_type.clone(),
            member_ids,
        })
    }
}

// ── Response types ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct NamedCount {
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct PeriodInfo {
    pub start: String,
    pub end: String,
}

/// Popularity: which resources are used most.
#[derive(Debug, Serialize)]
pub struct PopularityMetrics {
    /// Top 10 metric types by number of recorded entries.
    pub top_metric_types: Vec<NamedCount>,
    /// Top 10 workflow templates by number of instances started.
    pub top_workflow_templates: Vec<NamedCount>,
    /// Work-order volume by ticket type.
    pub work_order_types: Vec<NamedCount>,
}

/// Registration funnel: member onboarding depth.
#[derive(Debug, Serialize)]
pub struct ConversionMetrics {
    pub total_members: i64,
    pub with_health_profile: i64,
    pub with_metric_entries: i64,
    pub with_active_goals: i64,
    /// with_health_profile / total_members
    pub profile_completion_rate: f64,
    /// with_metric_entries / total_members
    pub engagement_rate: f64,
    /// with_active_goals / total_members
    pub goal_adoption_rate: f64,
}

/// Attendance / service completion rates.
#[derive(Debug, Serialize)]
pub struct AttendanceMetrics {
    pub work_orders_total: i64,
    pub work_orders_resolved: i64,
    pub work_order_resolution_rate: f64,
    pub work_orders_by_status: Vec<NamedCount>,
    pub workflow_instances_total: i64,
    pub workflow_instances_completed: i64,
    pub workflow_completion_rate: f64,
    pub workflow_instances_by_status: Vec<NamedCount>,
    /// Members with at least one metric entry in the period.
    pub active_members_in_period: i64,
}

/// Drop-off and failure counts.
#[derive(Debug, Serialize)]
pub struct CancellationMetrics {
    /// Work orders that reached `closed` status.
    pub work_orders_closed: i64,
    /// Workflow instances with status `rejected`.
    pub workflow_rejections: i64,
    /// Workflow instances with status `withdrawn`.
    pub workflow_withdrawals: i64,
    /// Workflow instances with status `cancelled`.
    pub workflow_cancellations: i64,
    /// Individual approval records with status `rejected`.
    pub approval_rejections: i64,
}

/// Attribute-level breakdowns (tag / channel distributions).
#[derive(Debug, Serialize)]
pub struct DistributionMetrics {
    pub work_orders_by_ticket_type: Vec<NamedCount>,
    pub work_orders_by_priority: Vec<NamedCount>,
    pub notifications_by_event_type: Vec<NamedCount>,
    pub goals_by_type: Vec<NamedCount>,
    pub goals_by_status: Vec<NamedCount>,
}

/// Top-level analytics response.
#[derive(Debug, Serialize)]
pub struct AnalyticsReport {
    pub period: PeriodInfo,
    pub org_unit_id: Option<Uuid>,
    pub ticket_type_filter: Option<String>,
    pub popularity: PopularityMetrics,
    pub registration_conversion: ConversionMetrics,
    pub attendance: AttendanceMetrics,
    pub cancellations: CancellationMetrics,
    pub distributions: DistributionMetrics,
}

/// Metadata returned after a successful export.
#[derive(Debug, Serialize)]
pub struct ExportMeta {
    pub export_id: Uuid,
    pub format: String,
    pub filename: String,
    pub file_path: String,
    pub size_bytes: u64,
    pub created_at: DateTime<Utc>,
    /// Relative URL to fetch the file.
    pub download_url: String,
}

// ─────────────────────────────────────────────────────────────────
// Unit tests — date-string parsing for the analytics filter.
// ─────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    fn empty_query() -> AnalyticsQuery {
        AnalyticsQuery {
            start_date: None,
            end_date: None,
            org_unit_id: None,
            ticket_type: None,
        }
    }

    #[test]
    fn resolved_filter_with_no_dates_keeps_them_none() {
        let f = ResolvedFilter::parse(&empty_query(), vec![]).unwrap();
        assert!(f.start_dt.is_none());
        assert!(f.end_dt.is_none());
        assert!(f.member_ids.is_empty());
    }

    #[test]
    fn resolved_filter_parses_valid_iso_date() {
        let mut q = empty_query();
        q.start_date = Some("2024-01-01".into());
        q.end_date = Some("2024-12-31".into());
        let f = ResolvedFilter::parse(&q, vec![]).unwrap();
        let s = f.start_dt.unwrap();
        let e = f.end_dt.unwrap();
        assert_eq!(s.format("%Y-%m-%d").to_string(), "2024-01-01");
        // End date is anchored at 23:59:59 to make the range inclusive.
        assert_eq!(e.format("%H:%M:%S").to_string(), "23:59:59");
    }

    #[test]
    fn resolved_filter_rejects_malformed_start_date() {
        let mut q = empty_query();
        q.start_date = Some("01/01/2024".into());
        let err = ResolvedFilter::parse(&q, vec![]).unwrap_err();
        assert!(err.contains("start_date"));
    }

    #[test]
    fn resolved_filter_rejects_malformed_end_date() {
        let mut q = empty_query();
        q.end_date = Some("not-a-date".into());
        let err = ResolvedFilter::parse(&q, vec![]).unwrap_err();
        assert!(err.contains("end_date"));
    }

    #[test]
    fn resolved_filter_carries_org_unit_and_ticket_type() {
        let org = Uuid::new_v4();
        let mut q = empty_query();
        q.org_unit_id = Some(org);
        q.ticket_type = Some("equipment".into());
        let mids = vec![Uuid::new_v4(), Uuid::new_v4()];
        let f = ResolvedFilter::parse(&q, mids.clone()).unwrap();
        assert_eq!(f.org_unit_id, Some(org));
        assert_eq!(f.ticket_type.as_deref(), Some("equipment"));
        assert_eq!(f.member_ids, mids);
    }

    #[test]
    fn export_request_validates_format_length() {
        let req = ExportRequest {
            format:      "csv".into(),
            start_date:  Some("2024-01-01".into()),
            end_date:    Some("2024-12-31".into()),
            org_unit_id: None,
            ticket_type: None,
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn export_request_rejects_empty_format() {
        let req = ExportRequest {
            format:      "".into(),
            start_date:  None,
            end_date:    None,
            org_unit_id: None,
            ticket_type: None,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn export_request_rejects_oversized_format() {
        let req = ExportRequest {
            format:      "x".repeat(11),
            start_date:  None,
            end_date:    None,
            org_unit_id: None,
            ticket_type: None,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn export_request_rejects_short_dates() {
        let req = ExportRequest {
            format:      "csv".into(),
            start_date:  Some("2024-1-1".into()), // 8 chars not 10
            end_date:    None,
            org_unit_id: None,
            ticket_type: None,
        };
        assert!(req.validate().is_err());
    }
}
