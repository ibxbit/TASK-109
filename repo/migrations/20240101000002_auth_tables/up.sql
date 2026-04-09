-- ============================================================
-- Auth: extend users + add sessions table
-- ============================================================

-- Add auth-tracking columns to users
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS failed_attempts     INTEGER     NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS failed_window_start TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS locked_until        TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS captcha_required    BOOLEAN     NOT NULL DEFAULT FALSE;

-- Sessions (opaque token, sliding 30-min expiry)
CREATE TABLE sessions (
    id               UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id          UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token            TEXT        NOT NULL UNIQUE,
    expires_at       TIMESTAMPTZ NOT NULL,
    last_activity_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ip_address       TEXT,
    user_agent       TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    invalidated_at   TIMESTAMPTZ
);

CREATE INDEX idx_sessions_token      ON sessions (token);
CREATE INDEX idx_sessions_user_id    ON sessions (user_id);
CREATE INDEX idx_sessions_expires_at ON sessions (expires_at);
