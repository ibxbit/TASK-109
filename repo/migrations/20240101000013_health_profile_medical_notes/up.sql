-- Migration 00013: Add encrypted medical_notes to health_profiles
--
-- medical_notes stores sensitive clinical notes (diagnoses, treatment plans,
-- medication records) that require stricter protection than dietary_notes.
-- Stored encrypted (AES-256-GCM) using the same FieldCipher used for
-- dietary_notes; nonce is stored alongside the ciphertext.
-- The encryption_key_id column (added by migration 00011) tracks which
-- key version encrypted both note fields.

ALTER TABLE health_profiles
    ADD COLUMN IF NOT EXISTS medical_notes_enc   TEXT,
    ADD COLUMN IF NOT EXISTS medical_notes_nonce TEXT;

COMMENT ON COLUMN health_profiles.medical_notes_enc   IS
    'AES-256-GCM ciphertext of clinical notes (base64). Never stored in plaintext.';
COMMENT ON COLUMN health_profiles.medical_notes_nonce IS
    'Base64-encoded 96-bit nonce paired with medical_notes_enc.';
