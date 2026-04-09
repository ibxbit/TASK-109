-- Migration 00014: Add amount_tier to workflow_templates
--
-- amount_tier captures the financial approval threshold associated with
-- a workflow template, complementing risk_tier with monetary controls.
-- Valid values are enforced at the application layer (AppError::BadRequest).
-- Examples: "under_1k", "1k_10k", "10k_100k", "over_100k"

ALTER TABLE workflow_templates
    ADD COLUMN IF NOT EXISTS amount_tier TEXT;

COMMENT ON COLUMN workflow_templates.amount_tier IS
    'Financial approval threshold tier. Application enforces allowed values.';
