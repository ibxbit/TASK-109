-- Revert metric_entries uniqueness constraint to previous (less strict) version
ALTER TABLE metric_entries DROP CONSTRAINT IF EXISTS uq_metric_entry;
ALTER TABLE metric_entries ADD CONSTRAINT uq_metric_entry UNIQUE (member_id, metric_type_id, entry_date, value);