DROP TRIGGER IF EXISTS trg_audit_log_no_update ON audit_logs;
DROP TRIGGER IF EXISTS trg_audit_log_no_delete ON audit_logs;
DROP FUNCTION IF EXISTS fn_audit_log_immutable();

DROP INDEX IF EXISTS idx_audit_logs_action_created;
DROP INDEX IF EXISTS idx_audit_logs_actor_created;
DROP INDEX IF EXISTS idx_audit_logs_entity_created;
DROP INDEX IF EXISTS idx_audit_logs_reason_code;

ALTER TABLE audit_logs
    DROP COLUMN IF EXISTS reason_code,
    DROP COLUMN IF EXISTS old_hash,
    DROP COLUMN IF EXISTS new_hash;
