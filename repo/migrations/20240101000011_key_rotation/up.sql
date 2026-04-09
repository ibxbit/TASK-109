-- Track AES-256 encryption key rotations.
-- A new row is inserted each time the active key version changes.
-- Startup checks the latest row; if it is older than 180 days a
-- warning is emitted and the SECURITY_KEY_ROTATION_NEEDED metric fires.
CREATE TABLE key_rotation_logs (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key_version    TEXT        NOT NULL,            -- e.g. "v1", "v2"
    rotated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    rotated_by     UUID        REFERENCES users(id) ON DELETE SET NULL,
    notes          TEXT,
    fields_updated INTEGER     NOT NULL DEFAULT 0   -- rows re-encrypted
);

-- Seed the initial key version so the first startup check has a baseline.
-- The version label matches ENCRYPTION_KEY_VERSION env-var default "v1".
INSERT INTO key_rotation_logs (id, key_version, notes)
VALUES (gen_random_uuid(), 'v1', 'initial key — baseline for rotation tracking');

-- Attach a key-version label to every encrypted field set.
-- When the key is rotated this column is updated after re-encryption.
ALTER TABLE health_profiles
    ADD COLUMN IF NOT EXISTS encryption_key_id TEXT NOT NULL DEFAULT 'v1';
