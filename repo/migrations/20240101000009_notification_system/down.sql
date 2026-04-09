DROP INDEX IF EXISTS idx_notification_schedules_due;
DROP TABLE IF EXISTS notification_schedules;

DROP INDEX IF EXISTS idx_notification_subs_user;
DROP TABLE IF EXISTS notification_subscriptions;

DROP INDEX IF EXISTS idx_deliveries_pending;
ALTER TABLE deliveries
    DROP COLUMN IF EXISTS attempt_count,
    DROP COLUMN IF EXISTS next_attempt_at,
    DROP COLUMN IF EXISTS last_error;

DROP INDEX IF EXISTS idx_notifications_event_type;
ALTER TABLE notifications
    DROP COLUMN IF EXISTS read_at,
    DROP COLUMN IF EXISTS event_type,
    DROP COLUMN IF EXISTS entity_type,
    DROP COLUMN IF EXISTS entity_id;
