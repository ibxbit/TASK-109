use chrono::{DateTime, Duration, Utc};
use diesel::prelude::*;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    auth::{captcha, passwords, role::Role},
    errors::AppError,
    models::{
        audit_log::{self, NewAuditLog},
        session::{NewSession, Session, SessionActivityUpdate},
        user::{FailedAttemptUpdate, ResetAuthState, User, UserPublic},
    },
    schema::{roles, sessions, users},
    security::masking,
};

// ── Constants ────────────────────────────────────────────────

const SESSION_DURATION_MINS: i64 = 30;
const LOCK_THRESHOLD: i32 = 10;
const CAPTCHA_THRESHOLD: i32 = 5;
/// Window during which failed attempts are counted.
const FAILURE_WINDOW_MINS: i64 = 15;
/// How long an account stays locked.
const LOCK_DURATION_MINS: i64 = 15;

// ── Public outcome type ──────────────────────────────────────

pub struct SessionToken {
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub user: UserPublic,
}

pub struct CaptchaData {
    pub question: String,
    pub token: String,
}

pub enum LoginOutcome {
    Success(SessionToken),
    CaptchaRequired(CaptchaData),
    InvalidCaptcha,
    InvalidCredentials,
    AccountLocked { until: DateTime<Utc> },
    AccountInactive,
}

// ── Login ────────────────────────────────────────────────────

pub fn login(
    conn: &mut PgConnection,
    jwt_secret: &str,
    username: &str,
    password: &str,
    captcha_token: Option<&str>,
    captcha_answer: Option<i32>,
    ip: Option<&str>,
) -> Result<LoginOutcome, AppError> {
    let now = Utc::now();

    // 1. Fetch user by username
    let user: Option<User> = users::table
        .filter(users::username.eq(username))
        .select(User::as_select())
        .first(conn)
        .optional()
        .map_err(AppError::Database)?;

    let user = match user {
        Some(u) => u,
        None => {
            // Log failure with no actor (unknown user) — critical: propagate error
            audit_log::insert_critical(
                conn,
                NewAuditLog::new(None, "LOGIN_FAILED", "auth", None, ip.map(str::to_owned))
                    .with_new_value(serde_json::json!({ "username": username, "reason": "user_not_found" })),
            )?;
            return Ok(LoginOutcome::InvalidCredentials);
        }
    };

    // 2. Active check
    if !user.is_active {
        return Ok(LoginOutcome::AccountInactive);
    }

    // 3. Lock check — reset window if it expired
    let (effective_attempts, effective_window_start) =
        if let Some(window_start) = user.failed_window_start {
            if now - window_start > Duration::minutes(FAILURE_WINDOW_MINS) {
                // Window expired; pretend we're starting fresh
                (0, None)
            } else {
                (user.failed_attempts, user.failed_window_start)
            }
        } else {
            (0, None)
        };

    if let Some(locked_until) = user.locked_until {
        if locked_until > now {
            audit_log::insert(
                conn,
                NewAuditLog::new(
                    Some(user.id),
                    "LOGIN_BLOCKED_LOCKED",
                    "auth",
                    Some(user.id),
                    ip.map(str::to_owned),
                ),
            );
            return Ok(LoginOutcome::AccountLocked { until: locked_until });
        }
        // Lock expired — clear it
        diesel::update(users::table.find(user.id))
            .set((
                users::locked_until.eq(None::<DateTime<Utc>>),
                users::failed_attempts.eq(0),
                users::failed_window_start.eq(None::<DateTime<Utc>>),
                users::captcha_required.eq(false),
                users::updated_at.eq(now),
            ))
            .execute(conn)
            .map_err(AppError::Database)?;
    }

    // 4. CAPTCHA check (required once threshold is crossed)
    let requires_captcha = user.captcha_required || effective_attempts >= CAPTCHA_THRESHOLD;
    if requires_captcha {
        match (captcha_token, captcha_answer) {
            (Some(tok), Some(ans)) => {
                if !captcha::verify(tok, ans, jwt_secret) {
                    return Ok(LoginOutcome::InvalidCaptcha);
                }
            }
            _ => {
                let challenge = captcha::generate(jwt_secret);
                return Ok(LoginOutcome::CaptchaRequired(CaptchaData {
                    question: challenge.question,
                    token: challenge.token,
                }));
            }
        }
    }

    // 5. Password verification
    if !passwords::verify(password, &user.password_hash) {
        let new_attempts = effective_attempts + 1;
        let window_start = effective_window_start.unwrap_or(now);

        let lock_until = if new_attempts >= LOCK_THRESHOLD {
            Some(now + Duration::minutes(LOCK_DURATION_MINS))
        } else {
            None
        };

        diesel::update(users::table.find(user.id))
            .set(FailedAttemptUpdate {
                failed_attempts: new_attempts,
                failed_window_start: Some(window_start),
                locked_until: lock_until,
                captcha_required: new_attempts >= CAPTCHA_THRESHOLD,
                updated_at: now,
            })
            .execute(conn)
            .map_err(AppError::Database)?;

        if lock_until.is_some() {
            warn!(user_id = %masking::mask_id(&user.id), "ACCOUNT_LOCKED after {} failed attempts", new_attempts);
            audit_log::insert(
                conn,
                NewAuditLog::new(
                    Some(user.id),
                    "ACCOUNT_LOCKED",
                    "auth",
                    Some(user.id),
                    ip.map(str::to_owned),
                )
                .with_new_value(serde_json::json!({
                    "failed_attempts": new_attempts,
                    "locked_until": lock_until,
                })),
            );
        }

        audit_log::insert_critical(
            conn,
            NewAuditLog::new(
                Some(user.id),
                "LOGIN_FAILED",
                "auth",
                Some(user.id),
                ip.map(str::to_owned),
            )
            .with_new_value(serde_json::json!({
                "reason": "wrong_password",
                "attempt": new_attempts,
            })),
        )?;
        return Ok(LoginOutcome::InvalidCredentials);
    }

    // 6. Success — reset failure counters
    diesel::update(users::table.find(user.id))
        .set(ResetAuthState {
            failed_attempts: 0,
            failed_window_start: Some(None),
            locked_until: Some(None),
            captcha_required: false,
            updated_at: now,
        })
        .execute(conn)
        .map_err(AppError::Database)?;

    // 7. Create session
    let token = Uuid::new_v4().to_string();
    let expires_at = now + Duration::minutes(SESSION_DURATION_MINS);

    diesel::insert_into(sessions::table)
        .values(NewSession {
            id: Uuid::new_v4(),
            user_id: user.id,
            token: token.clone(),
            expires_at,
            last_activity_at: now,
            ip_address: ip.map(str::to_owned),
            user_agent: None,
            created_at: now,
        })
        .execute(conn)
        .map_err(AppError::Database)?;

    info!(user_id = %masking::mask_id(&user.id), "LOGIN_SUCCESS");
    audit_log::insert_critical(
        conn,
        NewAuditLog::new(
            Some(user.id),
            "LOGIN_SUCCESS",
            "auth",
            Some(user.id),
            ip.map(str::to_owned),
        ),
    )?;

    Ok(LoginOutcome::Success(SessionToken {
        token,
        expires_at,
        user: UserPublic::from(user),
    }))
}

// ── Logout ───────────────────────────────────────────────────

pub fn logout(conn: &mut PgConnection, raw_token: &str) -> Result<(), AppError> {
    let now = Utc::now();
    let rows = diesel::update(
        sessions::table
            .filter(sessions::token.eq(raw_token))
            .filter(sessions::invalidated_at.is_null()),
    )
    .set(sessions::invalidated_at.eq(Some(now)))
    .execute(conn)
    .map_err(AppError::Database)?;

    if rows == 0 {
        return Err(AppError::Unauthorized);
    }
    Ok(())
}

// ── Session lookup + sliding expiry ─────────────────────────

pub struct ValidSession {
    pub session: Session,
    pub user: User,
    pub role: Role,
}

/// Finds an active, non-expired session by token, resolves the user's
/// role in the same query, slides the expiry, and returns all three.
pub fn validate_and_slide(
    conn: &mut PgConnection,
    raw_token: &str,
) -> Result<ValidSession, AppError> {
    let now = Utc::now();

    // Single join: sessions → users → roles
    let row: Option<(Session, User, String)> = sessions::table
        .inner_join(users::table.inner_join(roles::table))
        .filter(sessions::token.eq(raw_token))
        .filter(sessions::invalidated_at.is_null())
        .filter(sessions::expires_at.gt(now))
        .select((Session::as_select(), User::as_select(), roles::name))
        .first(conn)
        .optional()
        .map_err(AppError::Database)?;

    let (session, user, role_name) = row.ok_or(AppError::Unauthorized)?;

    if !user.is_active {
        return Err(AppError::Unauthorized);
    }

    let role = Role::from_db_name(&role_name)
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Unknown role: {}", role_name)))?;

    // Slide expiry
    let new_expiry = now + Duration::minutes(SESSION_DURATION_MINS);
    diesel::update(sessions::table.find(session.id))
        .set(SessionActivityUpdate {
            last_activity_at: now,
            expires_at: new_expiry,
        })
        .execute(conn)
        .map_err(AppError::Database)?;

    Ok(ValidSession { session, user, role })
}
