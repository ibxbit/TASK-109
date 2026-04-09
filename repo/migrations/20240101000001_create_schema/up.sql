-- ============================================================
-- VitalPath Health Operations — Core Schema
-- All timestamps stored in UTC (TIMESTAMPTZ).
-- All PKs are UUIDs generated via uuid_generate_v4().
-- ============================================================

-- ------------------------------------------------------------
-- roles
-- ------------------------------------------------------------
CREATE TABLE roles (
    id          UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    name        TEXT        NOT NULL UNIQUE
                            CHECK (name IN ('administrator', 'care_coach', 'approver', 'member')),
    description TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ------------------------------------------------------------
-- org_units  (self-referential hierarchy)
-- ------------------------------------------------------------
CREATE TABLE org_units (
    id          UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    name        TEXT        NOT NULL,
    parent_id   UUID        REFERENCES org_units (id) ON DELETE RESTRICT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ------------------------------------------------------------
-- users
-- ------------------------------------------------------------
CREATE TABLE users (
    id            UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    username      TEXT        NOT NULL UNIQUE,
    password_hash TEXT        NOT NULL,
    role_id       UUID        NOT NULL REFERENCES roles (id) ON DELETE RESTRICT,
    org_unit_id   UUID        REFERENCES org_units (id) ON DELETE SET NULL,
    is_active     BOOLEAN     NOT NULL DEFAULT TRUE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_users_role_id     ON users (role_id);
CREATE INDEX idx_users_org_unit_id ON users (org_unit_id);

-- ------------------------------------------------------------
-- members  (one user → one member record)
-- ------------------------------------------------------------
CREATE TABLE members (
    id                UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id           UUID        NOT NULL UNIQUE REFERENCES users (id) ON DELETE RESTRICT,
    org_unit_id       UUID        NOT NULL REFERENCES org_units (id) ON DELETE RESTRICT,
    first_name        TEXT        NOT NULL,
    last_name         TEXT        NOT NULL,
    date_of_birth     DATE        NOT NULL,
    phone             TEXT,
    emergency_contact TEXT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_members_org_unit_id ON members (org_unit_id);

-- ------------------------------------------------------------
-- health_profiles  (1:1 with members)
-- ------------------------------------------------------------
CREATE TABLE health_profiles (
    id            UUID             PRIMARY KEY DEFAULT uuid_generate_v4(),
    member_id     UUID             NOT NULL UNIQUE REFERENCES members (id) ON DELETE CASCADE,
    height_cm     DOUBLE PRECISION,
    weight_kg     DOUBLE PRECISION,
    blood_type    TEXT             CHECK (blood_type IN ('A+','A-','B+','B-','AB+','AB-','O+','O-')),
    allergies     TEXT,
    medical_notes TEXT,
    created_at    TIMESTAMPTZ      NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ      NOT NULL DEFAULT NOW()
);

-- ------------------------------------------------------------
-- metric_types
-- ------------------------------------------------------------
CREATE TABLE metric_types (
    id          UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    name        TEXT        NOT NULL UNIQUE,
    unit        TEXT        NOT NULL,
    description TEXT,
    is_active   BOOLEAN     NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ------------------------------------------------------------
-- metric_entries  (one entry per member per type per day)
-- ------------------------------------------------------------
CREATE TABLE metric_entries (
    id             UUID             PRIMARY KEY DEFAULT uuid_generate_v4(),
    member_id      UUID             NOT NULL REFERENCES members (id) ON DELETE CASCADE,
    metric_type_id UUID             NOT NULL REFERENCES metric_types (id) ON DELETE RESTRICT,
    value          DOUBLE PRECISION NOT NULL,
    entry_date     DATE             NOT NULL,
    recorded_by    UUID             NOT NULL REFERENCES users (id) ON DELETE RESTRICT,
    notes          TEXT,
    created_at     TIMESTAMPTZ      NOT NULL DEFAULT NOW(),

    CONSTRAINT uq_metric_entry UNIQUE (member_id, metric_type_id, entry_date)
);

CREATE INDEX idx_metric_entries_lookup
    ON metric_entries (member_id, metric_type_id, entry_date);

-- ------------------------------------------------------------
-- goals
-- ------------------------------------------------------------
CREATE TABLE goals (
    id             UUID             PRIMARY KEY DEFAULT uuid_generate_v4(),
    member_id      UUID             NOT NULL REFERENCES members (id) ON DELETE CASCADE,
    metric_type_id UUID             REFERENCES metric_types (id) ON DELETE SET NULL,
    title          TEXT             NOT NULL,
    description    TEXT,
    target_value   DOUBLE PRECISION,
    target_date    DATE,
    status         TEXT             NOT NULL DEFAULT 'active'
                                   CHECK (status IN ('active', 'completed', 'cancelled', 'paused')),
    assigned_by    UUID             NOT NULL REFERENCES users (id) ON DELETE RESTRICT,
    created_at     TIMESTAMPTZ      NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ      NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_goals_member_id ON goals (member_id);
CREATE INDEX idx_goals_status    ON goals (status);

-- ------------------------------------------------------------
-- workflow_templates
-- ------------------------------------------------------------
CREATE TABLE workflow_templates (
    id          UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    name        TEXT        NOT NULL UNIQUE,
    description TEXT,
    is_active   BOOLEAN     NOT NULL DEFAULT TRUE,
    created_by  UUID        NOT NULL REFERENCES users (id) ON DELETE RESTRICT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ------------------------------------------------------------
-- workflow_nodes  (ordered steps within a template)
-- ------------------------------------------------------------
CREATE TABLE workflow_nodes (
    id           UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    template_id  UUID        NOT NULL REFERENCES workflow_templates (id) ON DELETE CASCADE,
    name         TEXT        NOT NULL,
    node_order   INTEGER     NOT NULL,
    role_required UUID       REFERENCES roles (id) ON DELETE RESTRICT,
    action_type  TEXT        NOT NULL
                             CHECK (action_type IN ('review', 'approve', 'notify', 'complete')),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT uq_node_order UNIQUE (template_id, node_order)
);

CREATE INDEX idx_workflow_nodes_template_id ON workflow_nodes (template_id);

-- ------------------------------------------------------------
-- work_orders
-- ------------------------------------------------------------
CREATE TABLE work_orders (
    id          UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    member_id   UUID        NOT NULL REFERENCES members (id) ON DELETE RESTRICT,
    title       TEXT        NOT NULL,
    description TEXT,
    priority    TEXT        NOT NULL DEFAULT 'medium'
                            CHECK (priority IN ('low', 'medium', 'high', 'urgent')),
    status      TEXT        NOT NULL DEFAULT 'open'
                            CHECK (status IN ('open', 'in_progress', 'completed', 'cancelled')),
    assigned_to UUID        REFERENCES users (id) ON DELETE SET NULL,
    created_by  UUID        NOT NULL REFERENCES users (id) ON DELETE RESTRICT,
    due_date    DATE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_work_orders_member_id   ON work_orders (member_id);
CREATE INDEX idx_work_orders_status      ON work_orders (status);
CREATE INDEX idx_work_orders_assigned_to ON work_orders (assigned_to);

-- ------------------------------------------------------------
-- workflow_instances  (runtime execution of a template)
-- ------------------------------------------------------------
CREATE TABLE workflow_instances (
    id              UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    template_id     UUID        NOT NULL REFERENCES workflow_templates (id) ON DELETE RESTRICT,
    work_order_id   UUID        REFERENCES work_orders (id) ON DELETE SET NULL,
    current_node_id UUID        REFERENCES workflow_nodes (id) ON DELETE SET NULL,
    status          TEXT        NOT NULL DEFAULT 'pending'
                                CHECK (status IN ('pending', 'in_progress', 'completed', 'cancelled')),
    initiated_by    UUID        NOT NULL REFERENCES users (id) ON DELETE RESTRICT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_workflow_instances_work_order_id ON workflow_instances (work_order_id);
CREATE INDEX idx_workflow_instances_status        ON workflow_instances (status);

-- ------------------------------------------------------------
-- approvals
-- ------------------------------------------------------------
CREATE TABLE approvals (
    id                   UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    workflow_instance_id UUID        NOT NULL REFERENCES workflow_instances (id) ON DELETE CASCADE,
    node_id              UUID        NOT NULL REFERENCES workflow_nodes (id) ON DELETE RESTRICT,
    approver_id          UUID        NOT NULL REFERENCES users (id) ON DELETE RESTRICT,
    status               TEXT        NOT NULL DEFAULT 'pending'
                                     CHECK (status IN ('pending', 'approved', 'rejected')),
    comments             TEXT,
    decided_at           TIMESTAMPTZ,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_approvals_workflow_instance_id ON approvals (workflow_instance_id);
CREATE INDEX idx_approvals_approver_id          ON approvals (approver_id);
CREATE INDEX idx_approvals_status               ON approvals (status);

-- ------------------------------------------------------------
-- notification_templates
-- ------------------------------------------------------------
CREATE TABLE notification_templates (
    id            UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    name          TEXT        NOT NULL UNIQUE,
    subject       TEXT        NOT NULL,
    body_template TEXT        NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ------------------------------------------------------------
-- notifications
-- ------------------------------------------------------------
CREATE TABLE notifications (
    id          UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id     UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    template_id UUID        REFERENCES notification_templates (id) ON DELETE SET NULL,
    title       TEXT        NOT NULL,
    body        TEXT        NOT NULL,
    is_read     BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_notifications_user_id  ON notifications (user_id);
CREATE INDEX idx_notifications_is_read  ON notifications (user_id, is_read);

-- ------------------------------------------------------------
-- deliveries
-- ------------------------------------------------------------
CREATE TABLE deliveries (
    id              UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    notification_id UUID        NOT NULL REFERENCES notifications (id) ON DELETE CASCADE,
    channel         TEXT        NOT NULL CHECK (channel IN ('in_app', 'email')),
    status          TEXT        NOT NULL DEFAULT 'pending'
                                CHECK (status IN ('pending', 'delivered', 'failed')),
    delivered_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_deliveries_notification_id ON deliveries (notification_id);
CREATE INDEX idx_deliveries_status          ON deliveries (status);

-- ------------------------------------------------------------
-- audit_logs  (append-only, actor_id nullable for system ops)
-- ------------------------------------------------------------
CREATE TABLE audit_logs (
    id          UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    actor_id    UUID        REFERENCES users (id) ON DELETE SET NULL,
    action      TEXT        NOT NULL,
    entity_type TEXT        NOT NULL,
    entity_id   UUID,
    old_value   JSONB,
    new_value   JSONB,
    ip_address  TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_logs_actor_id    ON audit_logs (actor_id);
CREATE INDEX idx_audit_logs_entity      ON audit_logs (entity_type, entity_id);
CREATE INDEX idx_audit_logs_created_at  ON audit_logs (created_at DESC);
