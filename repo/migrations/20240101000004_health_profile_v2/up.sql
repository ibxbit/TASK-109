-- ============================================================
-- Redesign health_profiles to match the API field contract.
-- Old columns (height_cm, weight_kg, blood_type, allergies,
-- medical_notes) are replaced by the new set below.
-- dietary_notes is stored AES-256-GCM encrypted (ciphertext +
-- nonce stored separately, both base64-encoded).
-- ============================================================

ALTER TABLE health_profiles
    DROP COLUMN IF EXISTS height_cm,
    DROP COLUMN IF EXISTS weight_kg,
    DROP COLUMN IF EXISTS blood_type,
    DROP COLUMN IF EXISTS allergies,
    DROP COLUMN IF EXISTS medical_notes;

ALTER TABLE health_profiles
    ADD COLUMN sex                TEXT             NOT NULL DEFAULT 'prefer_not_to_say'
        CHECK (sex IN ('male', 'female', 'other', 'prefer_not_to_say')),
    ADD COLUMN height_in          DOUBLE PRECISION NOT NULL DEFAULT 0,
    ADD COLUMN weight_lbs         DOUBLE PRECISION NOT NULL DEFAULT 0,
    ADD COLUMN activity_level     TEXT             NOT NULL DEFAULT 'sedentary'
        CHECK (activity_level IN (
            'sedentary', 'lightly_active', 'moderately_active',
            'very_active', 'extra_active'
        )),
    ADD COLUMN dietary_notes_enc  TEXT,    -- AES-256-GCM ciphertext, base64
    ADD COLUMN dietary_notes_nonce TEXT;   -- 96-bit nonce, base64

-- Remove placeholder defaults (required only during column addition)
ALTER TABLE health_profiles
    ALTER COLUMN sex          DROP DEFAULT,
    ALTER COLUMN height_in    DROP DEFAULT,
    ALTER COLUMN weight_lbs   DROP DEFAULT,
    ALTER COLUMN activity_level DROP DEFAULT;
