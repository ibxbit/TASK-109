DROP INDEX IF EXISTS idx_work_orders_ticket_type;
DROP INDEX IF EXISTS idx_work_orders_routed_org;

ALTER TABLE work_orders
    DROP COLUMN IF EXISTS ticket_type,
    DROP COLUMN IF EXISTS processing_notes,
    DROP COLUMN IF EXISTS routed_to_org_unit_id,
    DROP COLUMN IF EXISTS resolved_at,
    DROP COLUMN IF EXISTS closed_at;

-- Best-effort status rollback
UPDATE work_orders SET status = 'open'        WHERE status IN ('intake', 'triage');
UPDATE work_orders SET status = 'in_progress' WHERE status = 'waiting_on_member';
UPDATE work_orders SET status = 'completed'   WHERE status = 'resolved';
UPDATE work_orders SET status = 'cancelled'   WHERE status = 'closed';

ALTER TABLE work_orders
    DROP CONSTRAINT IF EXISTS work_orders_status_check;
ALTER TABLE work_orders
    ALTER COLUMN status SET DEFAULT 'open';
ALTER TABLE work_orders
    ADD CONSTRAINT work_orders_status_check
    CHECK (status IN ('open', 'in_progress', 'completed', 'cancelled'));
