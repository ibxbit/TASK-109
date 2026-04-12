use std::{
    future::Future,
    ops::Deref,
    pin::Pin,
};

use actix_web::{web, FromRequest, HttpRequest};
use uuid::Uuid;

use crate::{
    auth::{role::Role, service},
    db::DbPool,
    errors::AppError,
};

// ── Core identity attached to every authenticated request ────

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: Uuid,
    pub username: String,
    pub role_id: Uuid,
    /// Resolved at session validation — no extra DB call needed.
    pub role: Role,
    pub session_id: Uuid,
}

impl AuthenticatedUser {
    // ── Centralised permission helpers ───────────────────────
    // All role enforcement logic lives here. Handlers MUST call
    // these; they MUST NOT re-implement checks themselves.

    /// Require the caller to be an Administrator. Returns 403 otherwise.
    pub fn require_admin(&self) -> Result<(), AppError> {
        if self.role.is_admin() {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }

    /// Require Administrator or Care Coach. Returns 403 otherwise.
    pub fn require_care_coach_or_above(&self) -> Result<(), AppError> {
        if self.role.can_manage_health_data() {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }

    /// Require Administrator or Approver. Returns 403 otherwise.
    pub fn require_approver_or_above(&self) -> Result<(), AppError> {
        if self.role.can_manage_workflows() {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }

    /// Allow if the caller is an Administrator **or** is accessing
    /// their own resource (`target_user_id == self.user_id`).
    /// Used for endpoints that serve both admins (all data) and
    /// members (own data only).
    #[allow(dead_code)]
    pub fn require_self_or_admin(&self, target_user_id: Uuid) -> Result<(), AppError> {
        if self.role.is_admin() || self.user_id == target_user_id {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }

    /// Returns true if this caller may read/write `member_user_id`'s data.
    ///
    /// - Administrator  → always true
    /// - Care Coach     → always true (manages all members)
    /// - Member         → only if it is their own record
    /// - Approver       → never (workflow-only role)
    pub fn can_access_member_data(&self, member_user_id: Uuid) -> bool {
        match &self.role {
            Role::Administrator => true,
            Role::CareCoach     => true,
            Role::Member        => self.user_id == member_user_id,
            Role::Approver      => false,
        }
    }

    /// Enforce `can_access_member_data` and return 403 on failure.
    pub fn require_member_data_access(&self, member_user_id: Uuid) -> Result<(), AppError> {
        if self.can_access_member_data(member_user_id) {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }
}

// ── FromRequest for AuthenticatedUser ────────────────────────

impl FromRequest for AuthenticatedUser {
    type Error = AppError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _: &mut actix_web::dev::Payload) -> Self::Future {
        let req = req.clone();

        Box::pin(async move {
            let token = extract_bearer(&req)?;

            let pool = req
                .app_data::<web::Data<DbPool>>()
                .ok_or_else(|| AppError::Internal(anyhow::anyhow!("DB pool missing")))?
                .clone();

            let valid = web::block(move || {
                let mut conn = pool
                    .get()
                    .map_err(|e: r2d2::Error| AppError::Internal(anyhow::anyhow!(e)))?;
                service::validate_and_slide(&mut conn, &token)
            })
            .await
            .map_err(|_| AppError::Internal(anyhow::anyhow!("Thread pool error")))?;

            let valid = valid?;

            Ok(AuthenticatedUser {
                user_id: valid.user.id,
                username: valid.user.username.clone(),
                role_id: valid.user.role_id,
                role: valid.role,
                session_id: valid.session.id,
            })
        })
    }
}

// ── Typed role extractors ────────────────────────────────────
// Declare one of these as a handler parameter to gate the entire
// endpoint to a specific permission level. The check is performed
// before the handler body runs; handlers contain zero role logic.

/// Grants access to Administrators only.
pub struct AdminAuth(pub AuthenticatedUser);

/// Grants access to Administrators and Care Coaches.
pub struct CareCoachAuth(pub AuthenticatedUser);

/// Grants access to Administrators and Approvers.
pub struct ApproverAuth(pub AuthenticatedUser);

// Deref lets handlers use `auth.user_id` etc. without `.0.`
impl Deref for AdminAuth     { type Target = AuthenticatedUser; fn deref(&self) -> &Self::Target { &self.0 } }
impl Deref for CareCoachAuth { type Target = AuthenticatedUser; fn deref(&self) -> &Self::Target { &self.0 } }
impl Deref for ApproverAuth  { type Target = AuthenticatedUser; fn deref(&self) -> &Self::Target { &self.0 } }

// ── Macro: delegate FromRequest to AuthenticatedUser + check ─

macro_rules! impl_role_extractor {
    ($extractor:ty, $check:ident) => {
        impl FromRequest for $extractor {
            type Error = AppError;
            type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

            fn from_request(
                req: &HttpRequest,
                payload: &mut actix_web::dev::Payload,
            ) -> Self::Future {
                let fut = AuthenticatedUser::from_request(req, payload);
                Box::pin(async move {
                    let auth = fut.await?;
                    auth.$check()?;
                    Ok(Self(auth))
                })
            }
        }
    };
}

impl_role_extractor!(AdminAuth,      require_admin);
impl_role_extractor!(CareCoachAuth,  require_care_coach_or_above);
impl_role_extractor!(ApproverAuth,   require_approver_or_above);

// ── Internal helper ──────────────────────────────────────────

fn extract_bearer(req: &HttpRequest) -> Result<String, AppError> {
    req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_owned)
        .ok_or(AppError::Unauthorized)
}
