# System Design

## Architecture Overview
- Rust backend using Actix-web
- PostgreSQL database via Diesel ORM
- Dockerized, single-container offline deployment
- Local-only APIs (no external network dependencies)

## Key Modules
- Authentication & Session Management
- Health Profile & Metrics
- Goals Management
- Workflow Engine
- Work-Order State Machine
- Notification & Task Reminder Center
- Analytics & Reporting
- Security & Compliance
- Backup & Restore

## Data Model Highlights
- Normalized tables, UUID primary keys
- Indexed metric_entries (member_id, metric_type, entry_date)
- Unique constraints for usernames, daily metrics
- Encrypted fields for sensitive data
- Audit log for all critical actions

## Security
- AES-256 encryption for sensitive fields
- Key rotation every 180 days
- Strict input validation, parameterized queries
- HMAC signing for privileged APIs
- Rate limiting, CAPTCHA, lockout
- Masked display of internal IDs

## Observability & Maintenance
- Structured logs, health metrics APIs
- Daily encrypted backups, 30-day retention
- Quarterly restore drills

## Performance
- Indexed queries for p95 < 300ms under 50 concurrent users
- Local observability, no external services

## Deployment
- Single Docker container
- Local volume for backups
- No external connectivity required
