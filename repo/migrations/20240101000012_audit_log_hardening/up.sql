-- ── Audit log hardening ───────────────────────────────────────
-- Migration 00012
--
-- 1. Add reason_code  — structured categorisation code (e.g. "WRONG_PASSWORD")
-- 2. Add old_hash     — SHA-256 hex of the old_value JSON (tamper evidence)
-- 3. Add new_hash     — SHA-256 hex of the new_value JSON (tamper evidence)
-- 4. Add immutability trigger — prohibits UPDATE/DELETE on audit_logs forever
-- 5. Add composite indexes for efficient filtering queries

-- ── New columns ───────────────────────────────────────────────

ALTER TABLE audit_logs
    ADD COLUMN IF NOT EXISTS reason_code TEXT,
    ADD COLUMN IF NOT EXISTS old_hash    TEXT,
    ADD COLUMN IF NOT EXISTS new_hash    TEXT;

-- ── Immutability trigger ──────────────────────────────────────
-- Any attempt to UPDATE or DELETE an existing row raises an
-- unrecoverable exception.  INSERTs are still allowed.
-- This cannot be bypassed without superuser access to drop the trigger.

CREATE OR REPLACE FUNCTION fn_audit_log_immutable()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION
        'audit_logs is immutable — UPDATE and DELETE are prohibited (row id: %)',
        OLD.id
        USING ERRCODE = 'restrict_violation';
END;
$$;

CREATE TRIGGER trg_audit_log_no_update
    BEFORE UPDATE ON audit_logs
    FOR EACH ROW EXECUTE FUNCTION fn_audit_log_immutable();

CREATE TRIGGER trg_audit_log_no_delete
    BEFORE DELETE ON audit_logs
    FOR EACH ROW EXECUTE FUNCTION fn_audit_log_immutable();

-- ── Query-optimised indexes ───────────────────────────────────
-- These complement the three existing indexes already created in
-- migration 00001 (idx_audit_logs_actor_id, idx_audit_logs_entity,
-- idx_audit_logs_created_at).

-- "Show all actions of type X in the last N days"
CREATE INDEX IF NOT EXISTS idx_audit_logs_action_created
    ON audit_logs (action, created_at DESC);

-- "Show all events for actor X ordered by recency"
CREATE INDEX IF NOT EXISTS idx_audit_logs_actor_created
    ON audit_logs (actor_id, created_at DESC)
    WHERE actor_id IS NOT NULL;

-- "Show all events for entity Y ordered by recency"
CREATE INDEX IF NOT EXISTS idx_audit_logs_entity_created
    ON audit_logs (entity_type, entity_id, created_at DESC);

-- "Show all events with a given reason_code"
CREATE INDEX IF NOT EXISTS idx_audit_logs_reason_code
    ON audit_logs (reason_code)
    WHERE reason_code IS NOT NULL;
