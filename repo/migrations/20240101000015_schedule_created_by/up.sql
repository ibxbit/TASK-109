-- Add created_by to notification_schedules so RBAC can distinguish
-- who created a schedule (admin creating for a user) from who owns it.
ALTER TABLE notification_schedules
    ADD COLUMN IF NOT EXISTS created_by UUID REFERENCES users (id) ON DELETE SET NULL;

-- Backfill: for existing rows treat user_id as creator
UPDATE notification_schedules SET created_by = user_id WHERE created_by IS NULL;
