DROP TABLE IF EXISTS sessions;

ALTER TABLE users
    DROP COLUMN IF EXISTS failed_attempts,
    DROP COLUMN IF EXISTS failed_window_start,
    DROP COLUMN IF EXISTS locked_until,
    DROP COLUMN IF EXISTS captcha_required;
