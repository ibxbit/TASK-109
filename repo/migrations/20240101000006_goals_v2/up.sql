-- ============================================================
-- Goals v2: add goal_type, start_date, baseline_value columns.
-- Also seeds the blood_glucose metric type required by the
-- glucose_control goal type.
-- ============================================================

-- Add blood_glucose metric type needed for glucose_control goals
INSERT INTO metric_types (id, name, unit, description, is_active) VALUES
    ('00000000-0000-0000-0002-000000000006', 'blood_glucose', 'mg/dL',
     'Blood glucose concentration', TRUE)
ON CONFLICT (name) DO NOTHING;

-- Extend goals table
ALTER TABLE goals
    ADD COLUMN IF NOT EXISTS goal_type      TEXT             NOT NULL DEFAULT 'fat_loss'
        CHECK (goal_type IN ('fat_loss', 'muscle_gain', 'glucose_control')),
    ADD COLUMN IF NOT EXISTS start_date     DATE             NOT NULL DEFAULT CURRENT_DATE,
    ADD COLUMN IF NOT EXISTS baseline_value DOUBLE PRECISION NOT NULL DEFAULT 0;

-- Remove placeholder defaults (only needed for ADD COLUMN on existing rows)
ALTER TABLE goals
    ALTER COLUMN goal_type      DROP DEFAULT,
    ALTER COLUMN start_date     DROP DEFAULT,
    ALTER COLUMN baseline_value DROP DEFAULT;
