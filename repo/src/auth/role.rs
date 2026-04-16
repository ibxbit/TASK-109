use uuid::{uuid, Uuid};

// ── Fixed role UUIDs (match seed migration 00003) ────────────
// Using compile-time constants avoids DB round-trips for role checks.

#[allow(dead_code)]
pub const ADMINISTRATOR_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000001");
pub const CARE_COACH_ID: Uuid    = uuid!("00000000-0000-0000-0000-000000000002");
#[allow(dead_code)]
pub const APPROVER_ID: Uuid      = uuid!("00000000-0000-0000-0000-000000000003");
#[allow(dead_code)]
pub const MEMBER_ID: Uuid        = uuid!("00000000-0000-0000-0000-000000000004");

// ── Role enum ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    Administrator,
    CareCoach,
    Approver,
    Member,
}

impl Role {
    /// Resolve from the DB `name` column. Returns `None` for unknown values.
    pub fn from_db_name(name: &str) -> Option<Self> {
        match name {
            "administrator" => Some(Self::Administrator),
            "care_coach"    => Some(Self::CareCoach),
            "approver"      => Some(Self::Approver),
            "member"        => Some(Self::Member),
            _               => None,
        }
    }

    #[allow(dead_code)]
    pub fn as_db_name(&self) -> &'static str {
        match self {
            Self::Administrator => "administrator",
            Self::CareCoach     => "care_coach",
            Self::Approver      => "approver",
            Self::Member        => "member",
        }
    }

    // ── Permission queries ───────────────────────────────────

    /// Full unrestricted access.
    pub fn is_admin(&self) -> bool {
        matches!(self, Self::Administrator)
    }

    /// Can read/write member profiles, metrics, and goals.
    pub fn can_manage_health_data(&self) -> bool {
        matches!(self, Self::Administrator | Self::CareCoach)
    }

    /// Can initiate, advance, and approve workflow steps.
    pub fn can_manage_workflows(&self) -> bool {
        matches!(self, Self::Administrator | Self::Approver)
    }

    /// Self-service: a member may only read their own data.
    #[allow(dead_code)]
    pub fn is_member(&self) -> bool {
        matches!(self, Self::Member)
    }
}

// ─────────────────────────────────────────────────────────────────
// Unit tests — role parsing + permission matrix.
//
// These tests document and lock in the authoritative permission
// boundaries used everywhere in the codebase. A regression here is
// a security regression.
// ─────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_db_name_recognises_every_canonical_role() {
        assert_eq!(Role::from_db_name("administrator"), Some(Role::Administrator));
        assert_eq!(Role::from_db_name("care_coach"), Some(Role::CareCoach));
        assert_eq!(Role::from_db_name("approver"), Some(Role::Approver));
        assert_eq!(Role::from_db_name("member"), Some(Role::Member));
    }

    #[test]
    fn from_db_name_rejects_unknown_values() {
        assert_eq!(Role::from_db_name(""), None);
        assert_eq!(Role::from_db_name("ADMIN"), None);          // case-sensitive
        assert_eq!(Role::from_db_name("Administrator"), None);  // case-sensitive
        assert_eq!(Role::from_db_name("coach"), None);
        assert_eq!(Role::from_db_name("root"), None);
    }

    #[test]
    fn as_db_name_roundtrips_with_from_db_name() {
        for role in [Role::Administrator, Role::CareCoach, Role::Approver, Role::Member] {
            let name = role.as_db_name();
            assert_eq!(Role::from_db_name(name), Some(role));
        }
    }

    #[test]
    fn only_administrator_is_admin() {
        assert!(Role::Administrator.is_admin());
        assert!(!Role::CareCoach.is_admin());
        assert!(!Role::Approver.is_admin());
        assert!(!Role::Member.is_admin());
    }

    #[test]
    fn admin_and_care_coach_manage_health_data() {
        assert!(Role::Administrator.can_manage_health_data());
        assert!(Role::CareCoach.can_manage_health_data());
        assert!(!Role::Approver.can_manage_health_data());
        assert!(!Role::Member.can_manage_health_data());
    }

    #[test]
    fn admin_and_approver_manage_workflows() {
        assert!(Role::Administrator.can_manage_workflows());
        assert!(Role::Approver.can_manage_workflows());
        assert!(!Role::CareCoach.can_manage_workflows());
        assert!(!Role::Member.can_manage_workflows());
    }

    #[test]
    fn is_member_identifies_only_member() {
        assert!(Role::Member.is_member());
        assert!(!Role::Administrator.is_member());
        assert!(!Role::CareCoach.is_member());
        assert!(!Role::Approver.is_member());
    }

    #[test]
    fn fixed_role_uuids_are_stable() {
        // These UUIDs are baked into the seed migration; tests pin them
        // so an accidental renumbering trips immediately.
        assert_eq!(ADMINISTRATOR_ID.to_string(), "00000000-0000-0000-0000-000000000001");
        assert_eq!(CARE_COACH_ID.to_string(),    "00000000-0000-0000-0000-000000000002");
        assert_eq!(APPROVER_ID.to_string(),      "00000000-0000-0000-0000-000000000003");
        assert_eq!(MEMBER_ID.to_string(),        "00000000-0000-0000-0000-000000000004");
    }
}
