use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Serialize;
use uuid::Uuid;

use crate::schema::users;

/// Full DB row — never sent directly to clients.
#[derive(Debug, Queryable, Selectable, Identifiable)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub role_id: Uuid,
    pub org_unit_id: Option<Uuid>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub failed_attempts: i32,
    pub failed_window_start: Option<DateTime<Utc>>,
    pub locked_until: Option<DateTime<Utc>>,
    pub captcha_required: bool,
}

/// Safe representation returned to API clients.
#[derive(Debug, Serialize)]
pub struct UserPublic {
    pub id: Uuid,
    pub username: String,
    pub role_id: Uuid,
    pub org_unit_id: Option<Uuid>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserPublic {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            username: u.username,
            role_id: u.role_id,
            org_unit_id: u.org_unit_id,
            is_active: u.is_active,
            created_at: u.created_at,
        }
    }
}

/// Insert a new user row.
#[derive(Debug, Insertable)]
#[diesel(table_name = users)]
pub struct NewUser {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub role_id: Uuid,
    pub org_unit_id: Option<Uuid>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Changeset applied after a failed login attempt.
#[derive(Debug, AsChangeset)]
#[diesel(table_name = users)]
pub struct FailedAttemptUpdate {
    pub failed_attempts: i32,
    pub failed_window_start: Option<DateTime<Utc>>,
    pub locked_until: Option<DateTime<Utc>>,
    pub captcha_required: bool,
    pub updated_at: DateTime<Utc>,
}

/// Changeset applied on successful login (reset all failure state).
#[derive(Debug, AsChangeset)]
#[diesel(table_name = users)]
pub struct ResetAuthState {
    pub failed_attempts: i32,
    pub failed_window_start: Option<Option<DateTime<Utc>>>,
    pub locked_until: Option<Option<DateTime<Utc>>>,
    pub captcha_required: bool,
    pub updated_at: DateTime<Utc>,
}
