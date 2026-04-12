use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::schema::{
    deliveries, notification_schedules, notification_subscriptions, notification_templates,
    notifications,
};

// ── Allowed values ────────────────────────────────────────────

pub const VALID_EVENT_TYPES: &[&str] = &[
    "sla_breach",
    "return_for_edit",
    "scheduled_reminder",
    "work_order_assigned",
    "workflow_action",
    "manual",
];

pub const MAX_DAILY_SENDS_PER_TEMPLATE: i64 = 3;
pub const MAX_DELIVERY_ATTEMPTS: i32 = 6; // 1 initial + 5 retries

// ── DB models ─────────────────────────────────────────────────

#[derive(Debug, Clone, Queryable, Selectable, Identifiable)]
#[diesel(table_name = notification_templates)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NotificationTemplate {
    pub id: Uuid,
    pub name: String,
    pub subject: String,
    pub body_template: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Queryable, Selectable, Identifiable)]
#[diesel(table_name = notifications)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Notification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub template_id: Option<Uuid>,
    pub title: String,
    pub body: String,
    pub is_read: bool,
    pub created_at: DateTime<Utc>,
    // migration 00009
    pub read_at: Option<DateTime<Utc>>,
    pub event_type: Option<String>,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = notifications)]
pub struct NewNotification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub template_id: Option<Uuid>,
    pub title: String,
    pub body: String,
    pub is_read: bool,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
    pub event_type: Option<String>,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
}

#[derive(Debug, Clone, Queryable, Selectable, Identifiable)]
#[diesel(table_name = deliveries)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Delivery {
    pub id: Uuid,
    pub notification_id: Uuid,
    pub channel: String,
    pub status: String,
    pub delivered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    // migration 00009
    pub attempt_count: i32,
    pub next_attempt_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = deliveries)]
pub struct NewDelivery {
    pub id: Uuid,
    pub notification_id: Uuid,
    pub channel: String,
    pub status: String,
    pub delivered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub attempt_count: i32,
    pub next_attempt_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Queryable, Selectable, Identifiable)]
#[diesel(table_name = notification_subscriptions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NotificationSubscription {
    pub id: Uuid,
    pub user_id: Uuid,
    pub event_type: String,
    pub is_subscribed: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = notification_subscriptions)]
pub struct NewNotificationSubscription {
    pub id: Uuid,
    pub user_id: Uuid,
    pub event_type: String,
    pub is_subscribed: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Queryable, Selectable, Identifiable)]
#[diesel(table_name = notification_schedules)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NotificationSchedule {
    pub id: Uuid,
    pub user_id: Uuid,
    pub template_id: Option<Uuid>,
    pub label: String,
    pub fire_hour: i32,
    pub tz_offset_minutes: i32,
    pub is_active: bool,
    pub last_fired_at: Option<DateTime<Utc>>,
    pub next_fire_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // migration 00015
    pub created_by: Option<Uuid>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = notification_schedules)]
pub struct NewNotificationSchedule {
    pub id: Uuid,
    pub user_id: Uuid,
    pub template_id: Option<Uuid>,
    pub label: String,
    pub fire_hour: i32,
    pub tz_offset_minutes: i32,
    pub is_active: bool,
    pub last_fired_at: Option<DateTime<Utc>>,
    pub next_fire_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
}

// ── Requests ──────────────────────────────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct CreateNotificationRequest {
    pub user_id: Uuid,
    pub template_id: Option<Uuid>,
    /// sla_breach | return_for_edit | scheduled_reminder |
    /// work_order_assigned | workflow_action | manual
    pub event_type: Option<String>,
    #[validate(length(min = 1, max = 300))]
    pub title: String,
    #[validate(length(min = 1, max = 5000))]
    pub body: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSubscriptionRequest {
    pub is_subscribed: bool,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateScheduleRequest {
    /// Admin may target a different user; others default to themselves.
    pub user_id: Option<Uuid>,
    pub template_id: Option<Uuid>,
    #[validate(length(min = 1, max = 200))]
    pub label: String,
    /// Local hour to fire (0–23).
    pub fire_hour: i32,
    /// UTC offset in minutes (e.g. -300 = UTC-5, 330 = UTC+5:30).
    pub tz_offset_minutes: i32,
}

#[derive(Debug, Deserialize)]
pub struct NotificationListQuery {
    pub is_read: Option<bool>,
    pub event_type: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ── Responses ─────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct NotificationResponse {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub is_read: bool,
    pub read_at: Option<DateTime<Utc>>,
    pub event_type: Option<String>,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub template_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl From<Notification> for NotificationResponse {
    fn from(n: Notification) -> Self {
        Self {
            id:          n.id,
            title:       n.title,
            body:        n.body,
            is_read:     n.is_read,
            read_at:     n.read_at,
            event_type:  n.event_type,
            entity_type: n.entity_type,
            entity_id:   n.entity_id,
            template_id: n.template_id,
            created_at:  n.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SubscriptionResponse {
    pub id: Uuid,
    pub event_type: String,
    pub is_subscribed: bool,
    pub updated_at: DateTime<Utc>,
}

impl From<NotificationSubscription> for SubscriptionResponse {
    fn from(s: NotificationSubscription) -> Self {
        Self {
            id:            s.id,
            event_type:    s.event_type,
            is_subscribed: s.is_subscribed,
            updated_at:    s.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ScheduleResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub template_id: Option<Uuid>,
    pub label: String,
    pub fire_hour: i32,
    pub tz_offset_minutes: i32,
    pub is_active: bool,
    pub last_fired_at: Option<DateTime<Utc>>,
    pub next_fire_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

impl From<NotificationSchedule> for ScheduleResponse {
    fn from(s: NotificationSchedule) -> Self {
        Self {
            id:                s.id,
            user_id:           s.user_id,
            template_id:       s.template_id,
            label:             s.label,
            fire_hour:         s.fire_hour,
            tz_offset_minutes: s.tz_offset_minutes,
            is_active:         s.is_active,
            last_fired_at:     s.last_fired_at,
            next_fire_at:      s.next_fire_at,
            created_at:        s.created_at,
        }
    }
}
