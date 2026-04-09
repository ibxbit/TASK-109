-- ============================================================
-- Workflow Engine v2
-- Extends four existing tables; no new tables needed.
-- ============================================================

-- ── workflow_templates: business context ─────────────────────
ALTER TABLE workflow_templates
    ADD COLUMN IF NOT EXISTS business_type TEXT,
    ADD COLUMN IF NOT EXISTS org_unit_id   UUID REFERENCES org_units (id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS risk_tier     TEXT
        CHECK (risk_tier IN ('low', 'medium', 'high', 'critical'));

-- ── workflow_nodes: parallel support ─────────────────────────
-- Drop the UNIQUE(template_id, node_order) constraint so multiple
-- nodes can share the same node_order (= a parallel approval stage).
ALTER TABLE workflow_nodes
    DROP CONSTRAINT IF EXISTS uq_node_order,
    ADD COLUMN IF NOT EXISTS is_parallel BOOLEAN NOT NULL DEFAULT FALSE;

-- Re-add a non-unique index so order queries remain fast.
CREATE INDEX IF NOT EXISTS idx_workflow_nodes_order
    ON workflow_nodes (template_id, node_order);

-- ── workflow_instances: extended status + stage tracking ──────
ALTER TABLE workflow_instances
    DROP CONSTRAINT IF EXISTS workflow_instances_status_check;
ALTER TABLE workflow_instances
    ADD CONSTRAINT workflow_instances_status_check
    CHECK (status IN (
        'pending', 'in_progress', 'completed',
        'rejected', 'returned', 'withdrawn', 'cancelled'
    ));

ALTER TABLE workflow_instances
    ADD COLUMN IF NOT EXISTS current_stage INTEGER,
    ADD COLUMN IF NOT EXISTS submitted_at  TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS completed_at  TIMESTAMPTZ;

-- ── approvals: extended status + SLA + reassignment ──────────
ALTER TABLE approvals
    DROP CONSTRAINT IF EXISTS approvals_status_check;
ALTER TABLE approvals
    ADD CONSTRAINT approvals_status_check
    CHECK (status IN (
        'pending', 'approved', 'rejected',
        'returned', 'reassigned', 'additional_sign_off'
    ));

ALTER TABLE approvals
    ADD COLUMN IF NOT EXISTS sla_deadline TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS sla_breached BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS assignee_id  UUID REFERENCES users (id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS note         TEXT;

CREATE INDEX IF NOT EXISTS idx_approvals_sla
    ON approvals (sla_deadline)
    WHERE status = 'pending' AND sla_breached = FALSE;
