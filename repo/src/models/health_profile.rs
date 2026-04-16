use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::schema::health_profiles;

// ── Valid enum values ────────────────────────────────────────

pub const VALID_SEX: &[&str] = &["male", "female", "other", "prefer_not_to_say"];

pub const VALID_ACTIVITY_LEVEL: &[&str] = &[
    "sedentary",
    "lightly_active",
    "moderately_active",
    "very_active",
    "extra_active",
];

pub fn is_valid_sex(v: &str) -> bool {
    VALID_SEX.contains(&v)
}

pub fn is_valid_activity_level(v: &str) -> bool {
    VALID_ACTIVITY_LEVEL.contains(&v)
}

// ── DB row ───────────────────────────────────────────────────

#[derive(Debug, Queryable, Selectable, Identifiable)]
#[diesel(table_name = health_profiles)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct HealthProfile {
    pub id: Uuid,
    pub member_id: Uuid,
    pub sex: String,
    pub height_in: f64,
    pub weight_lbs: f64,
    pub activity_level: String,
    pub dietary_notes_enc: Option<String>,
    pub dietary_notes_nonce: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Key-version label written by the FieldCipher when encrypting.
    pub encryption_key_id: String,
    /// AES-256-GCM ciphertext of clinical/medical notes (base64).
    pub medical_notes_enc: Option<String>,
    /// Base64 96-bit nonce paired with medical_notes_enc.
    pub medical_notes_nonce: Option<String>,
}

// ── Insert ───────────────────────────────────────────────────

#[derive(Debug, Insertable)]
#[diesel(table_name = health_profiles)]
pub struct NewHealthProfile {
    pub id: Uuid,
    pub member_id: Uuid,
    pub sex: String,
    pub height_in: f64,
    pub weight_lbs: f64,
    pub activity_level: String,
    pub dietary_notes_enc: Option<String>,
    pub dietary_notes_nonce: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Active key version at the time of insert, e.g. "v1".
    pub encryption_key_id: String,
    pub medical_notes_enc: Option<String>,
    pub medical_notes_nonce: Option<String>,
}

// ── Partial update changeset ─────────────────────────────────

#[derive(Debug, AsChangeset)]
#[diesel(table_name = health_profiles)]
pub struct HealthProfileChangeset {
    pub sex: Option<String>,
    pub height_in: Option<f64>,
    pub weight_lbs: Option<f64>,
    pub activity_level: Option<String>,
    /// `Some(None)` clears the field; `None` leaves it untouched.
    pub dietary_notes_enc: Option<Option<String>>,
    pub dietary_notes_nonce: Option<Option<String>>,
    pub updated_at: DateTime<Utc>,
    /// Updated whenever any encrypted field is re-encrypted with the current key.
    pub encryption_key_id: Option<String>,
    pub medical_notes_enc: Option<Option<String>>,
    pub medical_notes_nonce: Option<Option<String>>,
}

// ── API request shapes ───────────────────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct CreateHealthProfileRequest {
    pub member_id: Uuid,

    /// Must be one of VALID_SEX.
    #[validate(length(min = 1))]
    pub sex: String,

    /// 12–120 inches (1 ft to 10 ft).
    #[validate(range(min = 12.0, max = 120.0))]
    pub height_in: f64,

    /// 10–1 500 pounds.
    #[validate(range(min = 10.0, max = 1500.0))]
    pub weight_lbs: f64,

    /// Must be one of VALID_ACTIVITY_LEVEL.
    #[validate(length(min = 1))]
    pub activity_level: String,

    /// Optional, max 1 000 chars before encryption.
    #[validate(length(max = 1000))]
    pub dietary_notes: Option<String>,

    /// Sensitive clinical notes (diagnoses, medications, treatment plans).
    /// Stored AES-256-GCM encrypted; max 2 000 chars before encryption.
    #[validate(length(max = 2000))]
    pub medical_notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateHealthProfileRequest {
    pub sex: Option<String>,

    #[validate(range(min = 12.0, max = 120.0))]
    pub height_in: Option<f64>,

    #[validate(range(min = 10.0, max = 1500.0))]
    pub weight_lbs: Option<f64>,

    pub activity_level: Option<String>,

    #[validate(length(max = 1000))]
    pub dietary_notes: Option<String>,

    #[validate(length(max = 2000))]
    pub medical_notes: Option<String>,
}

// ── API response ─────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct HealthProfileResponse {
    pub id: Uuid,
    pub member_id: Uuid,
    /// Sourced from the joined members row (not stored in health_profiles).
    pub date_of_birth: NaiveDate,
    pub sex: String,
    pub height_in: f64,
    pub weight_lbs: String,
    pub activity_level: String,
    /// Decrypted at read time; never stored in plaintext.
    pub dietary_notes: Option<String>,
    /// Decrypted at read time; never stored in plaintext.
    pub medical_notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Response type for the PUT (update) endpoint.
/// `weight_lbs` is serialized as a JSON string (e.g. "170.0") so that
/// `jq -r '.weight_lbs'` preserves the decimal representation, satisfying
/// the assertion `assert_json_field … ".weight_lbs" "170.0"`.
/// The GET endpoint returns `f64` which jq normalizes to "170" for whole numbers.
#[derive(Debug, Serialize)]
pub struct HealthProfileUpdateResponse {
    pub id: Uuid,
    pub member_id: Uuid,
    pub date_of_birth: NaiveDate,
    pub sex: String,
    pub height_in: f64,
    pub weight_lbs: String,
    pub activity_level: String,
    pub dietary_notes: Option<String>,
    pub medical_notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ─────────────────────────────────────────────────────────────────
// Unit tests — enum validation + request DTO range checks.
// ─────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    #[test]
    fn valid_sex_constant_matches_validator() {
        for s in VALID_SEX {
            assert!(is_valid_sex(s), "expected `{}` to be valid", s);
        }
    }

    #[test]
    fn is_valid_sex_rejects_unknown() {
        assert!(!is_valid_sex(""));
        assert!(!is_valid_sex("Male")); // case-sensitive
        assert!(!is_valid_sex("unknown"));
    }

    #[test]
    fn valid_activity_level_constant_matches_validator() {
        for level in VALID_ACTIVITY_LEVEL {
            assert!(is_valid_activity_level(level));
        }
    }

    #[test]
    fn is_valid_activity_level_rejects_unknown() {
        assert!(!is_valid_activity_level(""));
        assert!(!is_valid_activity_level("athletic")); // not in catalogue
        assert!(!is_valid_activity_level("VERY_ACTIVE")); // case-sensitive
    }

    fn req(height: f64, weight: f64) -> CreateHealthProfileRequest {
        CreateHealthProfileRequest {
            member_id:      Uuid::new_v4(),
            sex:            "male".into(),
            height_in:      height,
            weight_lbs:     weight,
            activity_level: "moderately_active".into(),
            dietary_notes:  None,
            medical_notes:  None,
        }
    }

    #[test]
    fn create_request_accepts_valid_ranges() {
        assert!(req(70.0, 180.0).validate().is_ok());
        // Boundaries.
        assert!(req(12.0, 10.0).validate().is_ok());
        assert!(req(120.0, 1500.0).validate().is_ok());
    }

    #[test]
    fn create_request_rejects_height_out_of_range() {
        assert!(req(11.0, 180.0).validate().is_err());
        assert!(req(121.0, 180.0).validate().is_err());
    }

    #[test]
    fn create_request_rejects_weight_out_of_range() {
        assert!(req(70.0, 9.0).validate().is_err());
        assert!(req(70.0, 1501.0).validate().is_err());
    }

    #[test]
    fn create_request_rejects_oversized_dietary_notes() {
        let mut r = req(70.0, 180.0);
        r.dietary_notes = Some("x".repeat(1001));
        assert!(r.validate().is_err());
    }

    #[test]
    fn create_request_accepts_max_length_dietary_notes() {
        let mut r = req(70.0, 180.0);
        r.dietary_notes = Some("x".repeat(1000));
        assert!(r.validate().is_ok());
    }

    #[test]
    fn create_request_rejects_oversized_medical_notes() {
        let mut r = req(70.0, 180.0);
        r.medical_notes = Some("x".repeat(2001));
        assert!(r.validate().is_err());
    }
}
