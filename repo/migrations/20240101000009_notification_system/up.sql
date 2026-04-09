-- ============================================================
-- Notification System v1
-- Extends existing notifications/deliveries tables.
-- Adds per-user subscriptions and scheduled reminders.
-- ============================================================

-- ── Extend notifications ──────────────────────────────────────
-- read_at : precise read receipt timestamp
-- event_type : what triggered this notification
-- entity_type / entity_id : optional link to triggering entity
ALTER TABLE notifications
    ADD COLUMN IF NOT EXISTS read_at     TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS event_type  TEXT
        CHECK (event_type IN (
            'sla_breach', 'return_for_edit', 'scheduled_reminder',
            'work_order_assigned', 'workflow_action', 'manual'
        )),
    ADD COLUMN IF NOT EXISTS entity_type TEXT,
    ADD COLUMN IF NOT EXISTS entity_id   UUID;

CREATE INDEX IF NOT EXISTS idx_notifications_event_type
    ON notifications (user_id, event_type);

-- ── Extend deliveries ─────────────────────────────────────────
-- attempt_count   : how many delivery attempts so far
-- next_attempt_at : when to retry (NULL = no retry scheduled)
-- last_error      : reason for most recent failure
ALTER TABLE deliveries
    ADD COLUMN IF NOT EXISTS attempt_count   INTEGER     NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS next_attempt_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS last_error      TEXT;

CREATE INDEX IF NOT EXISTS idx_deliveries_pending
    ON deliveries (status, next_attempt_at)
    WHERE status = 'pending';

-- ── Per-user notification subscriptions ──────────────────────
-- Default state = subscribed (opt-out model).
-- A row with is_subscribed = false suppresses that event_type.
CREATE TABLE IF NOT EXISTS notification_subscriptions (
    id            UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id       UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    event_type    TEXT        NOT NULL
        CHECK (event_type IN (
            'sla_breach', 'return_for_edit', 'scheduled_reminder',
            'work_order_assigned', 'workflow_action', 'manual'
        )),
    is_subscribed BOOLEAN     NOT NULL DEFAULT TRUE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT uq_user_event_subscription UNIQUE (user_id, event_type)
);

CREATE INDEX IF NOT EXISTS idx_notification_subs_user
    ON notification_subscriptions (user_id);

-- ── Scheduled reminders ───────────────────────────────────────
-- fire_hour         : local hour (0-23) to fire the reminder
-- tz_offset_minutes : org timezone offset from UTC in minutes
--                     (e.g. -300 = UTC-5, 330 = UTC+5:30)
-- next_fire_at      : pre-computed next UTC fire time (updated after each fire)
CREATE TABLE IF NOT EXISTS notification_schedules (
    id                UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id           UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    template_id       UUID        REFERENCES notification_templates (id) ON DELETE SET NULL,
    label             TEXT        NOT NULL,
    fire_hour         INTEGER     NOT NULL CHECK (fire_hour BETWEEN 0 AND 23),
    tz_offset_minutes INTEGER     NOT NULL DEFAULT 0,
    is_active         BOOLEAN     NOT NULL DEFAULT TRUE,
    last_fired_at     TIMESTAMPTZ,
    next_fire_at      TIMESTAMPTZ NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_notification_schedules_due
    ON notification_schedules (next_fire_at)
    WHERE is_active = TRUE;
