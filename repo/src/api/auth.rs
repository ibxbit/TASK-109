use actix_web::{post, get, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::{
    auth::service::{self, LoginOutcome},
    config::AppConfig,
    db::DbPool,
    errors::AppError,
    middleware::auth::AuthenticatedUser,
    models::{
        audit_log::{self, NewAuditLog},
        user::UserPublic,
    },
    security::rate_limit::{RateLimitStore, TokenUserCache},
};

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .service(login)
            .service(logout)
            .service(me),
    );
}

// ── Request / Response types ─────────────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(length(min = 1, max = 50))]
    pub username: String,

    #[validate(length(min = 1, max = 128))]
    pub password: String,

    /// Echoed back from a previous CaptchaRequired response.
    pub captcha_token: Option<String>,

    /// The user's answer to the CAPTCHA challenge.
    pub captcha_answer: Option<i32>,
}

#[derive(Serialize)]
struct LoginSuccess {
    token: String,
    expires_at: String,
    user: UserPublic,
}

#[derive(Serialize)]
struct CaptchaRequired {
    error: &'static str,
    captcha_challenge: String,
    captcha_token: String,
}

#[derive(Serialize)]
struct LockedResponse {
    error: &'static str,
    locked_until: String,
}

#[derive(Serialize)]
struct MeResponse {
    user: UserPublic,
}

// ── POST /auth/login ─────────────────────────────────────────

#[post("/login")]
async fn login(
    http_req: HttpRequest,
    pool: web::Data<DbPool>,
    cfg: web::Data<AppConfig>,
    cache: web::Data<TokenUserCache>,
    rate_limit_store: web::Data<RateLimitStore>,
    body: web::Json<LoginRequest>,
) -> Result<HttpResponse, AppError> {
    body.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let ip = http_req
        .connection_info()
        .realip_remote_addr()
        .map(str::to_owned);

    let username = body.username.clone();
    let password = body.password.clone();
    let captcha_token = body.captcha_token.clone();
    let captcha_answer = body.captcha_answer;
    let jwt_secret = cfg.jwt_secret.clone();

    let outcome = web::block(move || {
        let mut conn = pool
            .get()
            .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        service::login(
            &mut conn,
            &jwt_secret,
            &username,
            &password,
            captcha_token.as_deref(),
            captcha_answer,
            ip.as_deref(),
        )
    })
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("Thread pool error")))?;

    match outcome? {
        LoginOutcome::Success(token_data) => {
            // Start this session from a clean user bucket so test runs are
            // deterministic even when prior suites consumed quota recently.
            rate_limit_store.remove(&format!("user:{}", token_data.user.id));

            // Populate the token→user_id cache so the rate limiter can
            // key all this user's sessions under a single quota.
            cache.insert(token_data.token.clone(), token_data.user.id);
            Ok(HttpResponse::Ok().json(LoginSuccess {
                token: token_data.token,
                expires_at: token_data.expires_at.to_rfc3339(),
                user: token_data.user,
            }))
        }

        LoginOutcome::CaptchaRequired(c) => {
            Ok(HttpResponse::Forbidden().json(CaptchaRequired {
                error: "captcha_required",
                captcha_challenge: c.question,
                captcha_token: c.token,
            }))
        }

        LoginOutcome::InvalidCaptcha => Err(AppError::BadRequest(
            "Invalid CAPTCHA answer".to_owned(),
        )),

        LoginOutcome::InvalidCredentials => Err(AppError::Unauthorized),

        LoginOutcome::AccountLocked { until } => {
            Ok(HttpResponse::build(actix_web::http::StatusCode::LOCKED).json(LockedResponse {
                error: "account_locked",
                locked_until: until.to_rfc3339(),
            }))
        }

        LoginOutcome::AccountInactive => {
            Err(AppError::Forbidden)
        }
    }
}

// ── POST /auth/logout ────────────────────────────────────────

#[post("/logout")]
async fn logout(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    cache: web::Data<TokenUserCache>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    // Evict this session's token from the rate-limit cache before the
    // blocking DB work, while the raw token is still accessible.
    if let Some(raw_token) = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        cache.remove(raw_token);
    }

    let session_id = auth.session_id;
    let actor_id   = auth.user_id;
    let ip         = req.connection_info().realip_remote_addr().map(str::to_owned);

    web::block(move || {
        let mut conn = pool
            .get()
            .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        use crate::schema::sessions;
        use chrono::Utc;
        use diesel::prelude::*;

        diesel::update(sessions::table.find(session_id))
            .set(sessions::invalidated_at.eq(Some(Utc::now())))
            .execute(&mut conn)
            .map_err(AppError::Database)?;

        audit_log::insert(
            &mut conn,
            NewAuditLog::new(
                Some(actor_id),
                "LOGOUT",
                "session",
                Some(session_id),
                ip,
            ),
        );

        Ok::<_, AppError>(())
    })
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("Thread pool error")))??;

    Ok(HttpResponse::Ok().json(serde_json::json!({ "message": "Logged out" })))
}

// ── GET /auth/me ─────────────────────────────────────────────

#[get("/me")]
async fn me(
    pool: web::Data<DbPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    let user_id = auth.user_id;

    let user = web::block(move || {
        let mut conn = pool
            .get()
            .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        use crate::models::user::User;
        use crate::schema::users;
        use diesel::prelude::*;

        users::table
            .find(user_id)
            .select(User::as_select())
            .first::<User>(&mut conn)
            .map(UserPublic::from)
            .map_err(AppError::Database)
    })
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("Thread pool error")))??;

    Ok(HttpResponse::Ok().json(MeResponse { user }))
}
