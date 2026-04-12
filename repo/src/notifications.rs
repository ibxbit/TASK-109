//! Notification business-logic layer.
//!
//! Exposes `send_notification` for use by any subsystem, plus two
//! long-running background workers that are spawned once at startup:
//!
//!  - `start_delivery_worker` — picks up pending in-app deliveries,
//!    retries failures with exponential back-off (max 5 retries).
//!  - `start_schedule_worker` — fires scheduled daily reminders when
//!    `next_fire_at` falls due.

use chrono::{DateTime, Duration, FixedOffset, TimeZone, Utc};
use diesel::prelude::*;
use tokio::time;
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    db::DbPool,
    errors::AppError,
    models::notification::{
        Delivery, NewDelivery, NewNotification, NotificationSchedule,
        MAX_DAILY_SENDS_PER_TEMPLATE, MAX_DELIVERY_ATTEMPTS,
    },
    schema::{deliveries, notification_schedules, notifications},
};

// ── Public send API ───────────────────────────────────────────

/// Create and enqueue an in-app notification for `user_id`.
///
/// Returns `Ok(Some(id))` when sent, `Ok(None)` when suppressed:
/// - Caller is unsubscribed from `event_type`, **or**
/// - Daily rate limit (3 per template per user) reached.
///
/// The notification row is inserted immediately; a `delivery` row
/// with `status = 'pending'` is inserted alongside it for the
/// delivery worker to process.
pub fn send_notification(
    conn: &mut PgConnection,
    user_id: Uuid,
    template_id: Option<Uuid>,
    event_type: &str,
    title: &str,
    body: &str,
    entity_type: Option<&str>,
    entity_id: Option<Uuid>,
) -> Result<Option<Uuid>, AppError> {
    // ── 1. Subscription check (opt-out model) ─────────────────
    // A missing row means subscribed; only an explicit false row suppresses.
    use crate::schema::notification_subscriptions::dsl as ns;
    let opt_out: Option<bool> = ns::notification_subscriptions
        .filter(ns::user_id.eq(user_id))
        .filter(ns::event_type.eq(event_type))
        .select(ns::is_subscribed)
        .first::<bool>(conn)
        .optional()
        .map_err(AppError::Database)?;

    if opt_out == Some(false) {
        return Ok(None); // user has opted out of this event type
    }

    // ── 2. Rate-limit check (template-scoped, today UTC) ──────
    if let Some(tmpl_id) = template_id {
        let start_of_day = Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .expect("valid hms")
            .and_utc();

        let today_count: i64 = notifications::table
            .filter(notifications::user_id.eq(user_id))
            .filter(notifications::template_id.eq(tmpl_id))
            .filter(notifications::created_at.ge(start_of_day))
            .count()
            .get_result(conn)
            .map_err(AppError::Database)?;

        if today_count >= MAX_DAILY_SENDS_PER_TEMPLATE {
            return Ok(None); // rate-limited
        }
    }

    // ── 3. Insert notification ────────────────────────────────
    let now  = Utc::now();
    let n_id = Uuid::new_v4();

    let new_n = NewNotification {
        id:          n_id,
        user_id,
        template_id,
        title:       title.to_owned(),
        body:        body.to_owned(),
        is_read:     false,
        created_at:  now,
        read_at:     None,
        event_type:  Some(event_type.to_owned()),
        entity_type: entity_type.map(str::to_owned),
        entity_id,
    };

    diesel::insert_into(notifications::table)
        .values(&new_n)
        .execute(conn)
        .map_err(AppError::Database)?;

    // ── 4. Insert pending delivery (in_app channel) ───────────
    let new_d = NewDelivery {
        id:              Uuid::new_v4(),
        notification_id: n_id,
        channel:         "in_app".to_owned(),
        status:          "pending".to_owned(),
        delivered_at:    None,
        created_at:      now,
        attempt_count:   0,
        next_attempt_at: None, // eligible immediately
        last_error:      None,
    };

    diesel::insert_into(deliveries::table)
        .values(&new_d)
        .execute(conn)
        .map_err(AppError::Database)?;

    Ok(Some(n_id))
}

// ── Background: delivery worker ───────────────────────────────

/// Spawns a Tokio task that processes pending deliveries every 30 seconds.
///
/// Delivery for in-app channel is a no-op acknowledgement (the
/// notification row is already visible). On success the delivery
/// is marked `delivered`. On an unexpected DB error it is retried
/// with exponential back-off up to `MAX_DELIVERY_ATTEMPTS` total
/// attempts; after that it is permanently marked `failed`.
///
/// Backoff schedule (minutes): 1, 2, 4, 8, 16.
pub fn start_delivery_worker(pool: DbPool) {
    tokio::spawn(async move {
        let mut interval = time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            let pool = pool.clone();
            let result = tokio::task::spawn_blocking(move || {
                let mut conn = pool.get().map_err(|e| anyhow::anyhow!(e))?;
                run_delivery_pass(&mut conn)
            })
            .await;

            match result {
                Ok(Ok(())) => {}
                Ok(Err(e)) => error!(error = %e, "delivery pass error"),
                Err(e)     => error!(error = %e, "delivery worker panic"),
            }
        }
    });
}

fn run_delivery_pass(conn: &mut PgConnection) -> Result<(), anyhow::Error> {
    let now = Utc::now();

    // Pending deliveries that are due (next_attempt_at is NULL or <= now)
    let due: Vec<Delivery> = deliveries::table
        .filter(deliveries::status.eq("pending"))
        .filter(
            deliveries::next_attempt_at
                .is_null()
                .or(deliveries::next_attempt_at.le(now)),
        )
        .select(Delivery::as_select())
        .load(conn)?;

    for d in due {
        // For in-app channel the "delivery" is purely a DB acknowledgement.
        // Simulate a fallible operation so the retry path is exercised if
        // a DB error occurs during the update itself.
        let update_result = diesel::update(deliveries::table.find(d.id))
            .set((
                deliveries::status.eq("delivered"),
                deliveries::delivered_at.eq(now),
                deliveries::attempt_count.eq(d.attempt_count + 1),
            ))
            .execute(conn);

        if let Err(e) = update_result {
            // Failed to acknowledge — schedule a retry with exponential back-off.
            let new_attempt = d.attempt_count + 1;
            let (new_status, next_at) = if new_attempt >= MAX_DELIVERY_ATTEMPTS {
                ("failed", None)
            } else {
                let delay_minutes = 1i64 << d.attempt_count; // 1, 2, 4, 8, 16
                ("pending", Some(now + Duration::minutes(delay_minutes)))
            };

            let _ = diesel::update(deliveries::table.find(d.id))
                .set((
                    deliveries::status.eq(new_status),
                    deliveries::attempt_count.eq(new_attempt),
                    deliveries::next_attempt_at.eq(next_at),
                    deliveries::last_error.eq(e.to_string()),
                ))
                .execute(conn);
        }
    }

    Ok(())
}

// ── Background: schedule worker ───────────────────────────────

/// Spawns a Tokio task that fires due scheduled reminders every 60 seconds.
///
/// For each due schedule:
/// 1. Fetches the linked template (if set) for title / body.
/// 2. Calls `send_notification` (applies rate-limit + subscription checks).
/// 3. Updates `last_fired_at` and advances `next_fire_at` by one day
///    (computed in the schedule's local timezone).
pub fn start_schedule_worker(pool: DbPool) {
    tokio::spawn(async move {
        let mut interval = time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            let pool = pool.clone();
            let result = tokio::task::spawn_blocking(move || {
                let mut conn = pool.get().map_err(|e| anyhow::anyhow!(e))?;
                run_schedule_pass(&mut conn)
            })
            .await;

            match result {
                Ok(Ok(())) => {}
                Ok(Err(e)) => error!(error = %e, "schedule pass error"),
                Err(e)     => error!(error = %e, "schedule worker panic"),
            }
        }
    });
}

fn run_schedule_pass(conn: &mut PgConnection) -> Result<(), anyhow::Error> {
    let now = Utc::now();

    let due: Vec<NotificationSchedule> = notification_schedules::table
        .filter(notification_schedules::is_active.eq(true))
        .filter(notification_schedules::next_fire_at.le(now))
        .select(NotificationSchedule::as_select())
        .load(conn)?;

    for sched in due {
        // Resolve title/body from template if linked
        let (title, body) = resolve_template(conn, sched.template_id, &sched.label);

        // send_notification checks subscription + rate-limit
        match send_notification(
            conn,
            sched.user_id,
            sched.template_id,
            "scheduled_reminder",
            &title,
            &body,
            None,
            None,
        ) {
            Ok(_) => {}
            Err(e) => {
                error!(schedule_id = %sched.id, error = %e, "schedule fire error");
                continue;
            }
        }

        // Advance next_fire_at by one day in local timezone
        let next = compute_next_fire_at(sched.fire_hour, sched.tz_offset_minutes);
        let _ = diesel::update(notification_schedules::table.find(sched.id))
            .set((
                notification_schedules::last_fired_at.eq(now),
                notification_schedules::next_fire_at.eq(next),
                notification_schedules::updated_at.eq(now),
            ))
            .execute(conn);

        info!(schedule_id = %sched.id, user_id = %sched.user_id, "scheduled reminder fired");
    }

    Ok(())
}

fn resolve_template(
    conn: &mut PgConnection,
    template_id: Option<Uuid>,
    fallback_label: &str,
) -> (String, String) {
    use crate::schema::notification_templates;
    if let Some(tmpl_id) = template_id {
        let result = notification_templates::table
            .find(tmpl_id)
            .select((
                notification_templates::subject,
                notification_templates::body_template,
            ))
            .first::<(String, String)>(conn)
            .optional();
        if let Ok(Some((subject, body))) = result {
            return (subject, body);
        }
    }
    (
        fallback_label.to_owned(),
        "You have a scheduled reminder.".to_owned(),
    )
}

// ── Scheduling helpers ────────────────────────────────────────

/// Compute the next UTC timestamp at which `fire_hour` local time
/// occurs (tomorrow if today's occurrence has already passed).
///
/// `tz_offset_minutes` is the local timezone's offset from UTC in minutes
/// (positive = east, negative = west).
pub fn compute_next_fire_at(fire_hour: i32, tz_offset_minutes: i32) -> DateTime<Utc> {
    let now = Utc::now();
    let offset_secs = tz_offset_minutes.clamp(-18 * 3600, 18 * 3600) * 60;
    let tz = FixedOffset::east_opt(offset_secs).unwrap_or_else(|| FixedOffset::east_opt(0).unwrap());
    let local_now   = now.with_timezone(&tz);
    let local_date  = local_now.date_naive();
    let clamped_h   = (fire_hour.clamp(0, 23)) as u32;

    let utc_fire = local_date
        .and_hms_opt(clamped_h, 0, 0)
        .and_then(|naive| tz.from_local_datetime(&naive).single())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|| now + Duration::days(1));

    if utc_fire > now {
        utc_fire
    } else {
        utc_fire + Duration::days(1)
    }
}
