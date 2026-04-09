DROP INDEX IF EXISTS idx_approvals_sla;
DROP INDEX IF EXISTS idx_workflow_nodes_order;

ALTER TABLE approvals
    DROP COLUMN IF EXISTS sla_deadline,
    DROP COLUMN IF EXISTS sla_breached,
    DROP COLUMN IF EXISTS assignee_id,
    DROP COLUMN IF EXISTS note;

ALTER TABLE approvals
    DROP CONSTRAINT IF EXISTS approvals_status_check;
ALTER TABLE approvals
    ADD CONSTRAINT approvals_status_check
    CHECK (status IN ('pending', 'approved', 'rejected'));

ALTER TABLE workflow_instances
    DROP COLUMN IF EXISTS current_stage,
    DROP COLUMN IF EXISTS submitted_at,
    DROP COLUMN IF EXISTS completed_at;

ALTER TABLE workflow_instances
    DROP CONSTRAINT IF EXISTS workflow_instances_status_check;
ALTER TABLE workflow_instances
    ADD CONSTRAINT workflow_instances_status_check
    CHECK (status IN ('pending', 'in_progress', 'completed', 'cancelled'));

ALTER TABLE workflow_nodes
    DROP COLUMN IF EXISTS is_parallel;
ALTER TABLE workflow_nodes
    ADD CONSTRAINT uq_node_order UNIQUE (template_id, node_order);

ALTER TABLE workflow_templates
    DROP COLUMN IF EXISTS business_type,
    DROP COLUMN IF EXISTS org_unit_id,
    DROP COLUMN IF EXISTS risk_tier;
