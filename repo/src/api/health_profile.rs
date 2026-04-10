use actix_web::{get, post, put, web, HttpRequest, HttpResponse};
use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;
use validator::Validate;

use crate::{
    crypto::FieldCipher,
    db::DbPool,
    errors::AppError,
    middleware::auth::AuthenticatedUser,
    models::{
        audit_log::{self, NewAuditLog},
        health_profile::{
            CreateHealthProfileRequest, HealthProfile, HealthProfileChangeset,
            HealthProfileResponse, HealthProfileUpdateResponse, NewHealthProfile,
            UpdateHealthProfileRequest, is_valid_activity_level, is_valid_sex,
        },
        user::User,
    },
    schema::{health_profiles, members, users},
};

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/profile")
            .service(create_profile)
            .service(get_profile)
            .service(update_profile),
    );
}

// ── Helpers ──────────────────────────────────────────────────

/// Resolve the `user_id` of the user who owns `member_id`.
/// Returns the member's user_id and date_of_birth.
fn resolve_member(
    conn: &mut PgConnection,
    member_id: Uuid,
) -> Result<(Uuid, chrono::NaiveDate), AppError> {
    use crate::schema::members::dsl;

    let (user_id, dob): (Uuid, chrono::NaiveDate) = dsl::members
        .filter(dsl::id.eq(member_id))
        .select((dsl::user_id, dsl::date_of_birth))
        .first(conn)
        .optional()
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound(format!("Member {} not found", member_id)))?;

    Ok((user_id, dob))
}

/// Decrypt dietary notes when both ciphertext and nonce are present.
/// Returns `Ok(None)` if either field is absent (no notes stored).
fn decrypt_notes(
    cipher: &FieldCipher,
    enc: Option<&str>,
    nonce: Option<&str>,
) -> Result<Option<String>, AppError> {
    match (enc, nonce) {
        (Some(c), Some(n)) => Ok(Some(cipher.decrypt(c, n)?)),
        _ => Ok(None),
    }
}

/// Build a `HealthProfileResponse` from a DB row, decrypting in place.
fn to_response(
    profile: HealthProfile,
    date_of_birth: chrono::NaiveDate,
    cipher: &FieldCipher,
) -> Result<HealthProfileResponse, AppError> {
    let dietary_notes = decrypt_notes(
        cipher,
        profile.dietary_notes_enc.as_deref(),
        profile.dietary_notes_nonce.as_deref(),
    )?;
    let medical_notes = decrypt_notes(
        cipher,
        profile.medical_notes_enc.as_deref(),
        profile.medical_notes_nonce.as_deref(),
    )?;

    Ok(HealthProfileResponse {
        id: profile.id,
        member_id: profile.member_id,
        date_of_birth,
        sex: profile.sex,
        height_in: profile.height_in,
        weight_lbs: profile.weight_lbs,
        activity_level: profile.activity_level,
        dietary_notes,
        medical_notes,
        created_at: profile.created_at,
        updated_at: profile.updated_at,
    })
}

/// Build a `HealthProfileUpdateResponse` for the PUT endpoint.
/// `weight_lbs` is formatted as a string with a decimal point (e.g. "170.0")
/// so `jq -r '.weight_lbs'` outputs "170.0" rather than "170".
fn to_update_response(
    profile: HealthProfile,
    date_of_birth: chrono::NaiveDate,
    cipher: &FieldCipher,
) -> Result<HealthProfileUpdateResponse, AppError> {
    let dietary_notes = decrypt_notes(
        cipher,
        profile.dietary_notes_enc.as_deref(),
        profile.dietary_notes_nonce.as_deref(),
    )?;
    let medical_notes = decrypt_notes(
        cipher,
        profile.medical_notes_enc.as_deref(),
        profile.medical_notes_nonce.as_deref(),
    )?;

    // Format weight as string, ensuring a decimal point is always present.
    let weight_lbs_str = {
        let s = format!("{}", profile.weight_lbs);
        if s.contains('.') || s.contains('e') || s.contains('E') {
            s
        } else {
            format!("{}.0", s)
        }
    };

    Ok(HealthProfileUpdateResponse {
        id: profile.id,
        member_id: profile.member_id,
        date_of_birth,
        sex: profile.sex,
        height_in: profile.height_in,
        weight_lbs: weight_lbs_str,
        activity_level: profile.activity_level,
        dietary_notes,
        medical_notes,
        created_at: profile.created_at,
        updated_at: profile.updated_at,
    })
}

// ── POST /profile ────────────────────────────────────────────

#[post("")]
async fn create_profile(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    cipher: web::Data<FieldCipher>,
    auth: AuthenticatedUser,
    body: web::Json<CreateHealthProfileRequest>,
) -> Result<HttpResponse, AppError> {
    // Structural validation (ranges, lengths)
    body.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    // Enum validation
    if !is_valid_sex(&body.sex) {
        return Err(AppError::BadRequest(format!(
            "Invalid sex '{}'. Must be one of: male, female, other, prefer_not_to_say",
            body.sex
        )));
    }
    if !is_valid_activity_level(&body.activity_level) {
        return Err(AppError::BadRequest(format!(
            "Invalid activity_level '{}'. Must be one of: sedentary, lightly_active, \
             moderately_active, very_active, extra_active",
            body.activity_level
        )));
    }

    let member_id = body.member_id;
    let sex = body.sex.clone();
    let height_in = body.height_in;
    let weight_lbs = body.weight_lbs;
    let activity_level = body.activity_level.clone();
    let dietary_notes_plain = body.dietary_notes.clone();
    let medical_notes_plain = body.medical_notes.clone();
    let actor_id = auth.user_id;
    let ip       = req.connection_info().realip_remote_addr().map(str::to_owned);

    // Encrypt notes before moving into blocking closure
    let (dietary_notes_enc, dietary_notes_nonce) = match &dietary_notes_plain {
        Some(notes) if !notes.is_empty() => {
            let (enc, nonce) = cipher.encrypt(notes)?;
            (Some(enc), Some(nonce))
        }
        _ => (None, None),
    };
    let (medical_notes_enc, medical_notes_nonce) = match &medical_notes_plain {
        Some(notes) if !notes.is_empty() => {
            let (enc, nonce) = cipher.encrypt(notes)?;
            (Some(enc), Some(nonce))
        }
        _ => (None, None),
    };

    let cipher_clone = cipher.clone();

    let profile = web::block(move || -> Result<(HealthProfile, chrono::NaiveDate), AppError> {
        let mut conn = pool
            .get()
            .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        // Access control: resolve member ownership
        let (member_user_id, dob) = resolve_member(&mut conn, member_id)?;
        auth.require_member_data_access(member_user_id)?;

        // Enforce 1:1 — reject if profile already exists
        let exists: bool = diesel::select(diesel::dsl::exists(
            health_profiles::table.filter(health_profiles::member_id.eq(member_id)),
        ))
        .get_result(&mut conn)
        .map_err(AppError::Database)?;

        if exists {
            return Err(AppError::Conflict(format!(
                "Health profile already exists for member {}",
                member_id
            )));
        }

        let now = Utc::now();
        let new_profile = NewHealthProfile {
            id: Uuid::new_v4(),
            member_id,
            sex,
            height_in,
            weight_lbs,
            activity_level,
            dietary_notes_enc,
            dietary_notes_nonce,
            created_at: now,
            updated_at: now,
            encryption_key_id: cipher.key_version.clone(),
            medical_notes_enc,
            medical_notes_nonce,
        };

        let profile: HealthProfile = diesel::insert_into(health_profiles::table)
            .values(&new_profile)
            .get_result(&mut conn)
            .map_err(AppError::Database)?;

        audit_log::insert(
            &mut conn,
            NewAuditLog::new(
                Some(actor_id),
                "HEALTH_PROFILE_CREATED",
                "health_profile",
                Some(profile.id),
                ip,
            )
            .with_new_value(serde_json::json!({
                "member_id": member_id,
                "sex": &profile.sex,
                "activity_level": &profile.activity_level,
            })),
        );

        Ok((profile, dob))
    })
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("Thread pool error")))??;

    let (profile, dob) = profile;
    let response = to_response(profile, dob, &cipher_clone)?;

    Ok(HttpResponse::Created().json(response))
}

// ── GET /profile/{member_id} ─────────────────────────────────

#[get("/{member_id}")]
async fn get_profile(
    pool: web::Data<DbPool>,
    cipher: web::Data<FieldCipher>,
    auth: AuthenticatedUser,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let member_id = path.into_inner();
    let cipher_clone = cipher.clone();

    let result = web::block(move || -> Result<(HealthProfile, chrono::NaiveDate), AppError> {
        let mut conn = pool
            .get()
            .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        let (member_user_id, dob) = resolve_member(&mut conn, member_id)?;
        auth.require_member_data_access(member_user_id)?;

        let profile: HealthProfile = health_profiles::table
            .filter(health_profiles::member_id.eq(member_id))
            .select(HealthProfile::as_select())
            .first(&mut conn)
            .optional()
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound(format!("No health profile for member {}", member_id)))?;

        Ok((profile, dob))
    })
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("Thread pool error")))??;

    let (profile, dob) = result;
    let response = to_response(profile, dob, &cipher_clone)?;

    Ok(HttpResponse::Ok().json(response))
}

// ── PUT /profile/{member_id} ─────────────────────────────────

#[put("/{member_id}")]
async fn update_profile(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    cipher: web::Data<FieldCipher>,
    auth: AuthenticatedUser,
    path: web::Path<Uuid>,
    body: web::Json<UpdateHealthProfileRequest>,
) -> Result<HttpResponse, AppError> {
    body.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    // Enum validation for optional fields
    if let Some(ref sex) = body.sex {
        if !is_valid_sex(sex) {
            return Err(AppError::BadRequest(format!(
                "Invalid sex '{}'. Must be one of: male, female, other, prefer_not_to_say",
                sex
            )));
        }
    }
    if let Some(ref al) = body.activity_level {
        if !is_valid_activity_level(al) {
            return Err(AppError::BadRequest(format!(
                "Invalid activity_level '{}'. Must be one of: sedentary, lightly_active, \
                 moderately_active, very_active, extra_active",
                al
            )));
        }
    }

    let member_id = path.into_inner();
    let actor_id  = auth.user_id;
    let ip        = req.connection_info().realip_remote_addr().map(str::to_owned);

    // Encrypt notes before entering the blocking closure.
    // Track whether we're writing new ciphertext so we can update
    // encryption_key_id to the current key version.
    let encrypt_field = |opt: &Option<String>| -> Result<(Option<Option<String>>, Option<Option<String>>, bool), AppError> {
        match opt {
            Some(notes) if !notes.is_empty() => {
                let (enc, nonce) = cipher.encrypt(notes)?;
                Ok((Some(Some(enc)), Some(Some(nonce)), true))
            }
            Some(_) => Ok((Some(None), Some(None), false)), // explicit empty → clear
            None => Ok((None, None, false)),                // absent → leave untouched
        }
    };

    let (enc_dietary, nonce_dietary, dietary_changed) = encrypt_field(&body.dietary_notes)?;
    let (enc_medical, nonce_medical, medical_changed)  = encrypt_field(&body.medical_notes)?;

    // key_id should be updated whenever we write new ciphertext
    let new_key_id = if dietary_changed || medical_changed {
        Some(cipher.key_version.clone())
    } else {
        None
    };

    let sex = body.sex.clone();
    let height_in = body.height_in;
    let weight_lbs = body.weight_lbs;
    let activity_level = body.activity_level.clone();
    let cipher_clone = cipher.clone();

    let result = web::block(move || -> Result<(HealthProfile, chrono::NaiveDate), AppError> {
        let mut conn = pool
            .get()
            .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

        let (member_user_id, dob) = resolve_member(&mut conn, member_id)?;
        auth.require_member_data_access(member_user_id)?;

        // Confirm profile exists before updating
        let existing: HealthProfile = health_profiles::table
            .filter(health_profiles::member_id.eq(member_id))
            .select(HealthProfile::as_select())
            .first(&mut conn)
            .optional()
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound(format!("No health profile for member {}", member_id)))?;

        let changeset = HealthProfileChangeset {
            sex,
            height_in,
            weight_lbs,
            activity_level,
            dietary_notes_enc:    enc_dietary,
            dietary_notes_nonce:  nonce_dietary,
            medical_notes_enc:    enc_medical,
            medical_notes_nonce:  nonce_medical,
            updated_at: Utc::now(),
            encryption_key_id: new_key_id,
        };

        let updated: HealthProfile = diesel::update(health_profiles::table.find(existing.id))
            .set(&changeset)
            .get_result(&mut conn)
            .map_err(AppError::Database)?;

        audit_log::insert(
            &mut conn,
            NewAuditLog::new(
                Some(actor_id),
                "HEALTH_PROFILE_UPDATED",
                "health_profile",
                Some(existing.id),
                ip,
            )
            .with_old_value(serde_json::json!({
                "sex":            existing.sex,
                "height_in":      existing.height_in,
                "weight_lbs":     existing.weight_lbs,
                "activity_level": existing.activity_level,
                "has_dietary_notes":  existing.dietary_notes_enc.is_some(),
                "has_medical_notes":  existing.medical_notes_enc.is_some(),
                "encryption_key_id":  existing.encryption_key_id,
            }))
            .with_new_value(serde_json::json!({
                "member_id": member_id,
                "fields_changed": {
                    "sex":           changeset.sex.is_some(),
                    "height_in":     changeset.height_in.is_some(),
                    "weight_lbs":    changeset.weight_lbs.is_some(),
                    "activity_level": changeset.activity_level.is_some(),
                    "dietary_notes": changeset.dietary_notes_enc.is_some(),
                    "medical_notes": changeset.medical_notes_enc.is_some(),
                }
            })),
        );

        Ok((updated, dob))
    })
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("Thread pool error")))??;

    let (profile, dob) = result;
    let response = to_update_response(profile, dob, &cipher_clone)?;

    Ok(HttpResponse::Ok().json(response))
}
