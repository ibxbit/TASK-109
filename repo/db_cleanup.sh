#!/bin/bash
# db_cleanup.sh — resets the database to a clean state for testing
set -e

# Use psql directly since it's installed in the tester container
PGPASSWORD=vitalpath_secret psql -h db -U vitalpath -d vitalpath_db <<EOF
TRUNCATE 
    approvals, 
    audit_logs, 
    deliveries, 
    goals, 
    health_profiles, 
    key_rotation_logs, 
    metric_entries, 
    notification_schedules, 
    notification_subscriptions, 
    notifications, 
    sessions, 
    work_orders, 
    workflow_instances 
RESTART IDENTITY CASCADE;

UPDATE users SET failed_login_attempts = 0, locked_until = NULL;
EOF

echo "Database cleaned (audit_logs, goals, metrics, etc. truncated)"
