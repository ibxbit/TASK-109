-- Seed the four roles with fixed UUIDs so application constants
-- can reference them without runtime lookups.
INSERT INTO roles (id, name, description) VALUES
    ('00000000-0000-0000-0000-000000000001', 'administrator', 'Full system access'),
    ('00000000-0000-0000-0000-000000000002', 'care_coach',    'Member, metrics, and goals management'),
    ('00000000-0000-0000-0000-000000000003', 'approver',      'Workflow and approval access'),
    ('00000000-0000-0000-0000-000000000004', 'member',        'Own data access only')
ON CONFLICT (name) DO NOTHING;
