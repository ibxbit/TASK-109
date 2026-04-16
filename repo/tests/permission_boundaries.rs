//! Integration test: role permission matrix exercised through
//! `AuthenticatedUser`'s public helpers.
//!
//! This is the security contract every API handler relies on. A
//! regression here is a permission escalation, so we exhaustively
//! pin the (role, action) outcomes.

use uuid::Uuid;
use vitalpath::auth::role::Role;
use vitalpath::middleware::auth::AuthenticatedUser;

fn user(role: Role) -> AuthenticatedUser {
    AuthenticatedUser {
        user_id:    Uuid::new_v4(),
        username:   "test-user".into(),
        role_id:    Uuid::new_v4(),
        role,
        session_id: Uuid::new_v4(),
    }
}

// ── require_admin ─────────────────────────────────────────────

#[test]
fn require_admin_only_allows_administrator() {
    assert!(user(Role::Administrator).require_admin().is_ok());
    assert!(user(Role::CareCoach).require_admin().is_err());
    assert!(user(Role::Approver).require_admin().is_err());
    assert!(user(Role::Member).require_admin().is_err());
}

// ── require_care_coach_or_above ──────────────────────────────

#[test]
fn require_care_coach_or_above_admits_admin_and_coach() {
    assert!(user(Role::Administrator).require_care_coach_or_above().is_ok());
    assert!(user(Role::CareCoach).require_care_coach_or_above().is_ok());
    assert!(user(Role::Approver).require_care_coach_or_above().is_err());
    assert!(user(Role::Member).require_care_coach_or_above().is_err());
}

// ── require_approver_or_above ────────────────────────────────

#[test]
fn require_approver_or_above_admits_admin_and_approver() {
    assert!(user(Role::Administrator).require_approver_or_above().is_ok());
    assert!(user(Role::Approver).require_approver_or_above().is_ok());
    assert!(user(Role::CareCoach).require_approver_or_above().is_err());
    assert!(user(Role::Member).require_approver_or_above().is_err());
}

// ── require_self_or_admin ────────────────────────────────────

#[test]
fn require_self_or_admin_allows_admin_or_owner() {
    let admin = user(Role::Administrator);
    let other = Uuid::new_v4();
    assert!(admin.require_self_or_admin(other).is_ok());
    assert!(admin.require_self_or_admin(admin.user_id).is_ok());
}

#[test]
fn require_self_or_admin_blocks_member_accessing_others() {
    let m = user(Role::Member);
    let other = Uuid::new_v4();
    assert!(m.require_self_or_admin(m.user_id).is_ok());
    assert!(m.require_self_or_admin(other).is_err());
}

// ── can_access_member_data / require_member_data_access ──────

#[test]
fn can_access_member_data_admin_and_coach_see_everyone() {
    let admin = user(Role::Administrator);
    let coach = user(Role::CareCoach);
    let other = Uuid::new_v4();
    assert!(admin.can_access_member_data(other));
    assert!(coach.can_access_member_data(other));
}

#[test]
fn can_access_member_data_member_only_sees_self() {
    let m = user(Role::Member);
    assert!(m.can_access_member_data(m.user_id));
    assert!(!m.can_access_member_data(Uuid::new_v4()));
}

#[test]
fn can_access_member_data_approver_never() {
    let a = user(Role::Approver);
    assert!(!a.can_access_member_data(Uuid::new_v4()));
    assert!(!a.can_access_member_data(a.user_id));
}

#[test]
fn require_member_data_access_returns_forbidden_for_unauthorised() {
    let approver = user(Role::Approver);
    let target = Uuid::new_v4();
    let err = approver.require_member_data_access(target).unwrap_err();
    assert_eq!(err.to_string(), "Forbidden");
}
