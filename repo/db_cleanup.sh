#!/bin/bash
# db_cleanup.sh — resets the database to a clean state for testing
set -e

# Use psql if available, otherwise use docker compose exec
if command -v psql >/dev/null 2>&1; then
    PSQL_CMD="psql -h db -U vitalpath -d vitalpath_db"
    export PGPASSWORD=vitalpath_secret
elif command -v docker >/dev/null 2>&1; then
    PSQL_CMD="docker compose exec -T db psql -U vitalpath -d vitalpath_db"
else
    echo "Error: Neither psql nor docker found."
    exit 1
fi

$PSQL_CMD <<EOF
TRUNCATE 
    approvals, 
    audit_logs, 
    deliveries, 
    goals, 
    health_profiles, 
    metric_entries, 
    notification_schedules, 
    notification_subscriptions, 
    notifications, 
    sessions, 
    work_orders, 
    workflow_instances 
RESTART IDENTITY CASCADE;

UPDATE users SET failed_attempts = 0, locked_until = NULL, captcha_required = false;
EOF

echo "Database cleaned (audit_logs, goals, metrics, etc. truncated)"
