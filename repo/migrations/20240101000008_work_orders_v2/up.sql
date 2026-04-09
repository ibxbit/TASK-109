-- ============================================================
-- Work Orders v2
-- Replaces the placeholder status set with a full lifecycle,
-- and adds ticket_type, processing_notes, routing, and
-- lifecycle timestamps.
-- ============================================================

-- 1. Migrate existing rows to the new status vocabulary
UPDATE work_orders SET status = 'intake'      WHERE status = 'open';
UPDATE work_orders SET status = 'in_progress' WHERE status = 'in_progress'; -- unchanged
UPDATE work_orders SET status = 'closed'      WHERE status IN ('completed', 'cancelled');

-- 2. Replace the status CHECK constraint
ALTER TABLE work_orders
    DROP CONSTRAINT IF EXISTS work_orders_status_check;
ALTER TABLE work_orders
    ALTER COLUMN status SET DEFAULT 'intake';
ALTER TABLE work_orders
    ADD CONSTRAINT work_orders_status_check
    CHECK (status IN (
        'intake', 'triage', 'in_progress',
        'waiting_on_member', 'resolved', 'closed'
    ));

-- 3. New columns
ALTER TABLE work_orders
    ADD COLUMN IF NOT EXISTS ticket_type           TEXT
        CHECK (ticket_type IN (
            'health_query', 'equipment', 'scheduling', 'nutrition', 'emergency'
        )),
    ADD COLUMN IF NOT EXISTS processing_notes      TEXT,
    ADD COLUMN IF NOT EXISTS routed_to_org_unit_id UUID
        REFERENCES org_units (id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS resolved_at           TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS closed_at             TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_work_orders_ticket_type
    ON work_orders (ticket_type);

CREATE INDEX IF NOT EXISTS idx_work_orders_routed_org
    ON work_orders (routed_to_org_unit_id);
