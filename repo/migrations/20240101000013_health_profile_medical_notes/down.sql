ALTER TABLE health_profiles
    DROP COLUMN IF EXISTS medical_notes_enc,
    DROP COLUMN IF EXISTS medical_notes_nonce;
