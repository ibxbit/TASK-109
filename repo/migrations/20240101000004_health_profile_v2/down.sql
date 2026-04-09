ALTER TABLE health_profiles
    DROP COLUMN IF EXISTS sex,
    DROP COLUMN IF EXISTS height_in,
    DROP COLUMN IF EXISTS weight_lbs,
    DROP COLUMN IF EXISTS activity_level,
    DROP COLUMN IF EXISTS dietary_notes_enc,
    DROP COLUMN IF EXISTS dietary_notes_nonce;

ALTER TABLE health_profiles
    ADD COLUMN IF NOT EXISTS height_cm     DOUBLE PRECISION,
    ADD COLUMN IF NOT EXISTS weight_kg     DOUBLE PRECISION,
    ADD COLUMN IF NOT EXISTS blood_type    TEXT,
    ADD COLUMN IF NOT EXISTS allergies     TEXT,
    ADD COLUMN IF NOT EXISTS medical_notes TEXT;
