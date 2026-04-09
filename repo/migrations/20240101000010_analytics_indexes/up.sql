-- ============================================================
-- Analytics performance indexes
-- All queries are read-only aggregations; no schema changes.
-- ============================================================

-- Work orders range queries by created_at + status/type
CREATE INDEX IF NOT EXISTS idx_work_orders_created_status
    ON work_orders (created_at DESC, status);

CREATE INDEX IF NOT EXISTS idx_work_orders_created_ticket
    ON work_orders (created_at DESC, ticket_type);

-- Workflow instances range queries
CREATE INDEX IF NOT EXISTS idx_workflow_instances_created_status
    ON workflow_instances (created_at DESC, status);

-- Metric entries by date for engagement queries
CREATE INDEX IF NOT EXISTS idx_metric_entries_created
    ON metric_entries (created_at DESC, member_id);

-- Goals range queries
CREATE INDEX IF NOT EXISTS idx_goals_created_type
    ON goals (created_at DESC, goal_type);

-- Approvals range queries
CREATE INDEX IF NOT EXISTS idx_approvals_created_status
    ON approvals (created_at DESC, status);

-- Notifications range queries
CREATE INDEX IF NOT EXISTS idx_notifications_created_event
    ON notifications (created_at DESC, event_type);

-- Health profiles — used in conversion funnel
CREATE INDEX IF NOT EXISTS idx_health_profiles_member
    ON health_profiles (member_id);
