#!/usr/bin/env bash
# =============================================================================
# Backup service entrypoint
# =============================================================================
# Cron jobs run in a minimal environment and do not inherit the variables set
# by Docker.  This script exports all relevant environment variables into
# /etc/environment so that cron can pick them up, then starts cron in the
# foreground.
# =============================================================================

set -euo pipefail

# Write env vars that cron jobs need into /etc/environment
# (cron reads /etc/environment on Debian/Ubuntu)
{
    echo "DATABASE_URL=${DATABASE_URL:?}"
    echo "BACKUP_ENCRYPTION_KEY=${BACKUP_ENCRYPTION_KEY:?}"
    echo "BACKUP_DIR=${BACKUP_DIR:-/backups}"
    echo "RETAIN_DAYS=${RETAIN_DAYS:-30}"
    echo "SCRIPTS_DIR=/scripts"
} > /etc/environment

echo "Env vars written to /etc/environment"
echo "Starting cron in foreground..."

# Run cron in foreground (-f) so the container stays alive
exec cron -f
