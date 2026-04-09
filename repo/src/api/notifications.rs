use actix_web::{delete, get, patch, post, web, HttpRequest, HttpResponse};
use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;
use validator::Validate;

use crate::{
    db::DbPool,
    errors::AppError,
    middleware::auth::{AdminAuth, AuthenticatedUser},
    models::{
        audit_log::{self, NewAuditLog},
        notification::{
            CreateNotificationRequest, CreateScheduleRequest, NewNotificationSchedule,
            NewNotificationSubscription, NotificationListQuery, NotificationResponse,
            NotificationSchedule, NotificationSubscription, ScheduleResponse,
            SubscriptionResponse, UpdateSubscriptionRequest, VALID_EVENT_TYPES,
        },
    },
    notifications::{compute_next_fire_at, send_notification},
    schema::{notification_schedules, notification_subscriptions, notifications},
};

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/notifications")
            // Sub-scopes for literal path segments — registered before
            // any parameterised handler so the router matches them first.
            .service(
                web::scope("/subscriptions")
                    .service(list_subscriptions)
                    .service(update_subscription),
            )
            .service(
                web::scope("/schedules")
                    .service(create_schedule)
                    .service(list_schedules)
                    .service(delete_schedule),
            )
            // Core notification endpoints
            .service(create_notification)
            .service(list_notifications)
            .service(mark_all_read)
            .service(mark_read),
    );
}

// ── Create ────────────────────────────────────────────────────

/// POST /notifications
/// Manually send an in-app notification to a specific user.
/// Applies rate-limit and subscription checks.
/// Requires care_coach or administrator.
#[post("")]
async fn create_notification(
    req: HttpRequest,
    user: AdminAuth, // admin-only manual creation
    pool: web::Data<DbPool>,
    body: web::Json<CreateNotificationRequest>,
) -> Result<HttpResponse, AppError> {
    body.validate().map_err(|e| AppError::BadRequest(e.to_string()))?;

    let event_type = body
        .event_type
        .clone()
        .unwrap_or_else(|| "manual".to_owned());

    if !VALID_EVENT_TYPES.contains(&event_type.as_str()) {
        return Err(AppError::BadRequest(format!(
            "invalid event_type '{}'; valid: {}",
            event_type,
            VALID_EVENT_TYPES.join(", ")
        )));
    }

    let ip       = req.connection_info().realip_remote_addr().map(str::to_owned);
    let actor_id = user.user_id;
    let body     = body.into_inner();

    let notif_id = web::block(move || {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        let id = send_notification(
            &mut conn,
            body.user_id,
            body.template_id,
            &event_type,
            &body.title,
            &body.body,
            body.entity_type.as_deref(),
            body.entity_id,
        )?;

        if let Some(nid) = id {
            audit_log::insert(
                &mut conn,
                NewAuditLog::new(Some(actor_id), "NOTIFICATION_CREATED", "notification", Some(nid), ip)
                    .with_new_value(serde_json::json!({
                        "user_id":      body.user_id,
                        "event_type":   event_type,
                        "template_id":  body.template_id,
                    })),
            );
        }

        Ok(id)
    })
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))??;

    match notif_id {
        Some(id) => Ok(HttpResponse::Created().json(serde_json::json!({ "id": id }))),
        None     => Err(AppError::TooManyRequests(
            "notification suppressed: rate-limited or user is unsubscribed".into(),
        )),
    }
}

// ── List ──────────────────────────────────────────────────────

/// GET /notifications
/// Returns the caller's own notifications, newest first.
/// Query params: is_read, event_type, limit (default 50, max 100), offset.
#[get("")]
async fn list_notifications(
    user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    query: web::Query<NotificationListQuery>,
) -> Result<HttpResponse, AppError> {
    let actor_id = user.user_id;
    let query    = query.into_inner();

    let items = web::block(move || {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        let limit  = query.limit.unwrap_or(50).clamp(1, 100);
        let offset = query.offset.unwrap_or(0).max(0);

        let mut q = notifications::table
            .filter(notifications::user_id.eq(actor_id))
            .into_boxed();

        if let Some(is_read) = query.is_read {
            q = q.filter(notifications::is_read.eq(is_read));
        }
        if let Some(ref et) = query.event_type {
            if VALID_EVENT_TYPES.contains(&et.as_str()) {
                q = q.filter(notifications::event_type.eq(et.clone()));
            }
        }

        use crate::models::notification::Notification;
        q.select(Notification::as_select())
            .order(notifications::created_at.desc())
            .limit(limit)
            .offset(offset)
            .load::<Notification>(&mut conn)
            .map_err(AppError::Database)
    })
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))??;

    let body: Vec<NotificationResponse> = items.into_iter().map(NotificationResponse::from).collect();
    Ok(HttpResponse::Ok().json(body))
}

// ── Mark read ─────────────────────────────────────────────────

/// POST /notifications/{id}/read
/// Mark a single notification as read (read receipt).
#[post("/{id}/read")]
async fn mark_read(
    req: HttpRequest,
    user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let ip       = req.connection_info().realip_remote_addr().map(str::to_owned);
    let actor_id = user.user_id;
    let notif_id = *id;

    web::block(move || {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        // Load and ownership-check in one query
        let owner: Uuid = notifications::table
            .find(notif_id)
            .select(notifications::user_id)
            .first(&mut conn)
            .map_err(|_| AppError::NotFound("notification not found".into()))?;

        if owner != actor_id {
            return Err(AppError::Forbidden);
        }

        let now = Utc::now();
        diesel::update(notifications::table.find(notif_id))
            .set((
                notifications::is_read.eq(true),
                notifications::read_at.eq(now),
            ))
            .execute(&mut conn)
            .map_err(AppError::Database)?;

        audit_log::insert(
            &mut conn,
            NewAuditLog::new(
                Some(actor_id), "NOTIFICATION_READ", "notification", Some(notif_id), ip,
            ),
        );

        Ok(())
    })
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))??;

    Ok(HttpResponse::NoContent().finish())
}

/// POST /notifications/read-all
/// Mark all of the caller's unread notifications as read.
#[post("/read-all")]
async fn mark_all_read(
    req: HttpRequest,
    user: AuthenticatedUser,
    pool: web::Data<DbPool>,
) -> Result<HttpResponse, AppError> {
    let ip       = req.connection_info().realip_remote_addr().map(str::to_owned);
    let actor_id = user.user_id;

    let count = web::block(move || {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
        let now = Utc::now();

        let n = diesel::update(notifications::table)
            .filter(notifications::user_id.eq(actor_id))
            .filter(notifications::is_read.eq(false))
            .set((
                notifications::is_read.eq(true),
                notifications::read_at.eq(now),
            ))
            .execute(&mut conn)
            .map_err(AppError::Database)?;

        audit_log::insert(
            &mut conn,
            NewAuditLog::new(Some(actor_id), "NOTIFICATION_ALL_READ", "notification", None, ip)
                .with_new_value(serde_json::json!({ "marked_count": n })),
        );

        Ok(n)
    })
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))??;

    Ok(HttpResponse::Ok().json(serde_json::json!({ "marked_count": count })))
}

// ── Subscriptions ─────────────────────────────────────────────

/// GET /notifications/subscriptions
/// Returns explicitly set subscription preferences for the caller.
/// Missing entries mean the user is subscribed (opt-out model).
#[get("")]
async fn list_subscriptions(
    user: AuthenticatedUser,
    pool: web::Data<DbPool>,
) -> Result<HttpResponse, AppError> {
    let actor_id = user.user_id;

    let subs = web::block(move || {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        notification_subscriptions::table
            .filter(notification_subscriptions::user_id.eq(actor_id))
            .select(NotificationSubscription::as_select())
            .order(notification_subscriptions::event_type.asc())
            .load::<NotificationSubscription>(&mut conn)
            .map_err(AppError::Database)
    })
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))??;

    let body: Vec<SubscriptionResponse> = subs.into_iter().map(SubscriptionResponse::from).collect();
    Ok(HttpResponse::Ok().json(body))
}

/// PATCH /notifications/subscriptions/{event_type}
/// Upsert a subscription preference for the caller.
/// `is_subscribed: false` opts the user out of that event type.
#[patch("/{event_type}")]
async fn update_subscription(
    req: HttpRequest,
    user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    event_type: web::Path<String>,
    body: web::Json<UpdateSubscriptionRequest>,
) -> Result<HttpResponse, AppError> {
    let et = event_type.into_inner();
    if !VALID_EVENT_TYPES.contains(&et.as_str()) {
        return Err(AppError::BadRequest(format!(
            "invalid event_type '{}'; valid: {}",
            et,
            VALID_EVENT_TYPES.join(", ")
        )));
    }

    let ip       = req.connection_info().realip_remote_addr().map(str::to_owned);
    let actor_id = user.user_id;
    let is_sub   = body.is_subscribed;

    let sub = web::block(move || {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
        let now = Utc::now();

        // Upsert: insert or update on (user_id, event_type) conflict
        diesel::insert_into(notification_subscriptions::table)
            .values(&NewNotificationSubscription {
                id:            Uuid::new_v4(),
                user_id:       actor_id,
                event_type:    et.clone(),
                is_subscribed: is_sub,
                created_at:    now,
                updated_at:    now,
            })
            .on_conflict((
                notification_subscriptions::user_id,
                notification_subscriptions::event_type,
            ))
            .do_update()
            .set((
                notification_subscriptions::is_subscribed.eq(is_sub),
                notification_subscriptions::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .map_err(AppError::Database)?;

        let sub: NotificationSubscription = notification_subscriptions::table
            .filter(notification_subscriptions::user_id.eq(actor_id))
            .filter(notification_subscriptions::event_type.eq(&et))
            .select(NotificationSubscription::as_select())
            .first(&mut conn)
            .map_err(AppError::Database)?;

        audit_log::insert(
            &mut conn,
            NewAuditLog::new(
                Some(actor_id),
                "NOTIFICATION_SUBSCRIPTION_UPDATED",
                "notification_subscription",
                Some(sub.id),
                ip,
            )
            .with_new_value(serde_json::json!({
                "event_type":    et,
                "is_subscribed": is_sub,
            })),
        );

        Ok(sub)
    })
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))??;

    Ok(HttpResponse::Ok().json(SubscriptionResponse::from(sub)))
}

// ── Schedules ─────────────────────────────────────────────────

/// POST /notifications/schedules
/// Create a daily scheduled reminder for a user.
/// Admin may target any user; others target themselves.
#[post("")]
async fn create_schedule(
    req: HttpRequest,
    user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    body: web::Json<CreateScheduleRequest>,
) -> Result<HttpResponse, AppError> {
    body.validate().map_err(|e| AppError::BadRequest(e.to_string()))?;

    if !(0..=23).contains(&body.fire_hour) {
        return Err(AppError::BadRequest("fire_hour must be 0–23".into()));
    }

    let is_admin  = user.role.is_admin();
    let actor_id  = user.user_id;
    let target_user = match body.user_id {
        Some(uid) if uid != actor_id => {
            if !is_admin {
                return Err(AppError::Forbidden);
            }
            uid
        }
        _ => actor_id,
    };

    let ip   = req.connection_info().realip_remote_addr().map(str::to_owned);
    let body = body.into_inner();

    let sched = web::block(move || {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
        let now      = Utc::now();
        let next_at  = compute_next_fire_at(body.fire_hour, body.tz_offset_minutes);

        let new_s = NewNotificationSchedule {
            id:                Uuid::new_v4(),
            user_id:           target_user,
            template_id:       body.template_id,
            label:             body.label.clone(),
            fire_hour:         body.fire_hour,
            tz_offset_minutes: body.tz_offset_minutes,
            is_active:         true,
            last_fired_at:     None,
            next_fire_at:      next_at,
            created_at:        now,
            updated_at:        now,
        };

        let sched: NotificationSchedule = diesel::insert_into(notification_schedules::table)
            .values(&new_s)
            .returning(NotificationSchedule::as_returning())
            .get_result(&mut conn)
            .map_err(AppError::Database)?;

        audit_log::insert(
            &mut conn,
            NewAuditLog::new(
                Some(actor_id), "NOTIFICATION_SCHEDULE_CREATED", "notification_schedule", Some(sched.id), ip,
            )
            .with_new_value(serde_json::json!({
                "user_id":   target_user,
                "label":     sched.label,
                "fire_hour": sched.fire_hour,
                "next_fire": sched.next_fire_at,
            })),
        );

        Ok(sched)
    })
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))??;

    Ok(HttpResponse::Created().json(ScheduleResponse::from(sched)))
}

/// GET /notifications/schedules
/// List the caller's active schedules.
#[get("")]
async fn list_schedules(
    user: AuthenticatedUser,
    pool: web::Data<DbPool>,
) -> Result<HttpResponse, AppError> {
    let actor_id = user.user_id;
    let is_admin = user.role.is_admin();

    let scheds = web::block(move || {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        let mut q = notification_schedules::table.into_boxed();
        if !is_admin {
            q = q.filter(notification_schedules::user_id.eq(actor_id));
        }

        q.select(NotificationSchedule::as_select())
            .order(notification_schedules::created_at.desc())
            .load::<NotificationSchedule>(&mut conn)
            .map_err(AppError::Database)
    })
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))??;

    let body: Vec<ScheduleResponse> = scheds.into_iter().map(ScheduleResponse::from).collect();
    Ok(HttpResponse::Ok().json(body))
}

/// DELETE /notifications/schedules/{id}
/// Delete a schedule. Admin may delete any; others only their own.
#[delete("/{id}")]
async fn delete_schedule(
    req: HttpRequest,
    user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let ip       = req.connection_info().realip_remote_addr().map(str::to_owned);
    let actor_id = user.user_id;
    let is_admin = user.role.is_admin();
    let sched_id = *id;

    web::block(move || {
        let mut conn = pool.get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        // Ownership check
        let owner: Uuid = notification_schedules::table
            .find(sched_id)
            .select(notification_schedules::user_id)
            .first(&mut conn)
            .map_err(|_| AppError::NotFound("schedule not found".into()))?;

        if !is_admin && owner != actor_id {
            return Err(AppError::Forbidden);
        }

        diesel::delete(notification_schedules::table.find(sched_id))
            .execute(&mut conn)
            .map_err(AppError::Database)?;

        audit_log::insert(
            &mut conn,
            NewAuditLog::new(
                Some(actor_id), "NOTIFICATION_SCHEDULE_DELETED", "notification_schedule", Some(sched_id), ip,
            ),
        );

        Ok(())
    })
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))??;

    Ok(HttpResponse::NoContent().finish())
}
