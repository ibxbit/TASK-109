-- Seed the five supported metric types with fixed UUIDs.
-- The UUIDs are referenced as constants in src/models/metric.rs.
INSERT INTO metric_types (id, name, unit, description, is_active) VALUES
    ('00000000-0000-0000-0002-000000000001', 'weight',              'lbs',    'Body weight in pounds',                          TRUE),
    ('00000000-0000-0000-0002-000000000002', 'body_fat_percentage', '%',      'Body fat as a percentage of total body weight',  TRUE),
    ('00000000-0000-0000-0002-000000000003', 'waist',               'inches', 'Waist circumference in inches',                  TRUE),
    ('00000000-0000-0000-0002-000000000004', 'hip',                 'inches', 'Hip circumference in inches',                    TRUE),
    ('00000000-0000-0000-0002-000000000005', 'chest',               'inches', 'Chest circumference in inches',                  TRUE)
ON CONFLICT (name) DO NOTHING;
