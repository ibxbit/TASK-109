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
