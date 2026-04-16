# VitalPath Health Operations API

**Type: backend**

[![Coverage](https://img.shields.io/badge/coverage-tarpaulin-blue.svg)](./tarpaulin.toml) [![Tests](https://img.shields.io/badge/tests-cargo%20%2B%20bash-brightgreen.svg)](#running-tests)

A secure, offline-capable health operations backend built with Rust (Actix-web 4), PostgreSQL 16, and Docker.

### Project Documentation

| Document | Purpose |
|----------|---------|
| [`docs/api-spec.md`](docs/api-spec.md) | Comprehensive API endpoint reference |
| [`docs/design.md`](docs/design.md) | High-level system architecture and security design |
| [`docs/questions.md`](docs/questions.md) | Project Q&A and design rationale |

---

## Quick Start

```bash
docker compose up
```

The first run builds the Rust binary (~2–3 min), starts PostgreSQL, runs database migrations, and seeds test credentials automatically. No manual configuration required.

---

## Services

| Service | Address | Description |
|---------|---------|-------------|
| **API** | `http://localhost:8080` | Rust/Actix-web REST API |
| **PostgreSQL** | `localhost:5432` | Primary datastore (internal) |
| **Backup** | — | Encrypted daily backup + quarterly restore drill (cron, no port) |

---

## Seeded Test Credentials

Created automatically on first startup. **Replace before any production use.**

| Username | Password | Role |
|----------|----------|------|
| `admin` | `Admin1234!` | Administrator (full access) |
| `coach` | `Coach1234!` | Care Coach (member + health data) |
| `approver` | `Approver1234!` | Approver (workflow approvals) |
| `member` | `Member1234!` | Member (own data only) |

Fixed IDs for use in tests:

| Resource | UUID |
|----------|------|
| Member record | `30000000-0000-0000-0000-000000000001` |
| Org unit | `10000000-0000-0000-0000-000000000001` |

---

## API Reference

### Authentication

```
POST /auth/login    — obtain Bearer token
POST /auth/logout   — invalidate session
GET  /auth/me       — current user info
```

### Health Profile

```
POST /profile              — create profile (admin/care_coach)
GET  /profile/{member_id}  — read profile
PUT  /profile/{member_id}  — update profile
```

### Metric Entries

```
POST /metrics                                        — record a measurement
GET  /metrics?member_id=<id>&range=7d|30d|90d|all   — list entries
GET  /metrics/summary?member_id=<id>&range=30d       — aggregated summary
```

Supported metric types: `weight`, `body_fat_percentage`, `waist`, `hip`, `chest`, `blood_glucose`

### Goals

```
POST /goals                       — create goal
GET  /goals?member_id=<id>        — list goals
PUT  /goals/{id}                  — update title/target/status
```

Goal types: `fat_loss`, `muscle_gain`, `glucose_control`

### Work Orders

```
POST  /work-orders                      — create ticket
PATCH /work-orders/{id}/transition      — advance state
GET   /work-orders                      — list (role-filtered)
```

States: `intake → triage → in_progress → waiting_on_member → resolved → closed`

### Observability

```
GET /health             — liveness + DB ping + pool stats
GET /internal/metrics   — Prometheus text format
GET /audit-logs         — tamper-evident audit trail (admin only)
```

---

## Step-by-Step Verification

### 1. Confirm the service is running

```bash
curl http://localhost:8080/health
# Expected: {"status":"ok","checks":{"database":{"status":"ok",...}},...}
```

### 2. Login and capture token

```bash
TOKEN=$(curl -s -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"Admin1234!"}' \
  | jq -r '.token')

echo "Token: $TOKEN"
```

### 3. Confirm identity

```bash
curl -s http://localhost:8080/auth/me \
  -H "Authorization: Bearer $TOKEN" | jq .
# Expected: {"user":{"username":"admin",...}}
```

### 4. Record a health metric

```bash
MEMBER_ID="30000000-0000-0000-0000-000000000001"

curl -s -X POST http://localhost:8080/metrics \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"member_id\":\"$MEMBER_ID\",\"metric_type\":\"weight\",\"value\":175.0}" \
  | jq .
# Expected: HTTP 201 with metric entry
```

### 5. List metrics

```bash
curl -s "http://localhost:8080/metrics?member_id=$MEMBER_ID&range=7d" \
  -H "Authorization: Bearer $TOKEN" | jq '.total, .entries[0].value'
```

### 6. Create a goal

```bash
curl -s -X POST http://localhost:8080/goals \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"member_id\":\"$MEMBER_ID\",
    \"goal_type\":\"fat_loss\",
    \"title\":\"Reduce body fat\",
    \"start_date\":\"$(date -u +%Y-%m-%d)\",
    \"baseline_value\":25.0,
    \"target_value\":20.0
  }" | jq .
```

### 7. View audit trail (admin only)

```bash
curl -s "http://localhost:8080/audit-logs?per_page=5" \
  -H "Authorization: Bearer $TOKEN" | jq '.data[0]'
```

### 8. RBAC — verify member is denied audit logs

```bash
MEMBER_TOKEN=$(curl -s -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"member","password":"Member1234!"}' \
  | jq -r '.token')

curl -s http://localhost:8080/audit-logs \
  -H "Authorization: Bearer $MEMBER_TOKEN"
# Expected: HTTP 403 {"error":"Forbidden","message":"..."}
```

### 9. Prometheus metrics

```bash
curl -s http://localhost:8080/internal/metrics | grep http_requests_total
```

### 10. Logout

```bash
curl -s -X POST http://localhost:8080/auth/logout \
  -H "Authorization: Bearer $TOKEN" | jq .
# Expected: {"message":"Logged out"}
```

---

## Advanced Security Feature Verification

> **Static audit reference:** [`docs/SECURITY_EVIDENCE.md`](docs/SECURITY_EVIDENCE.md) provides
> complete static evidence (code references, migration excerpts, sample artifacts) for all features
> below, suitable for audit review without running the system.

### HMAC Request Signing

Privileged endpoint `POST /analytics/export` requires HMAC-SHA256 signed requests in addition to a Bearer token:

```bash
# Compute signature
HMAC_SECRET="change_me_in_production_use_openssl_rand_hex_32"
TS=$(date -u +%s)
MESSAGE="${TS}:POST:/analytics/export"
SIG=$(printf '%s' "$MESSAGE" | openssl dgst -sha256 -hmac "$HMAC_SECRET" | awk '{print $NF}')

# Call with valid HMAC
COACH_TOKEN=$(curl -s -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"coach","password":"Coach1234!"}' | jq -r '.token')

curl -s -X POST http://localhost:8080/analytics/export \
  -H "Authorization: Bearer $COACH_TOKEN" \
  -H "Content-Type: application/json" \
  -H "X-Timestamp: $TS" \
  -H "X-Signature: $SIG" \
  -d '{"format":"csv"}' | jq .
# Expected: HTTP 201 with filename and download_url

# Missing signature → 400
curl -s -o /dev/null -w "%{http_code}" -X POST http://localhost:8080/analytics/export \
  -H "Authorization: Bearer $COACH_TOKEN" \
  -H "Content-Type: application/json" \
  -H "X-Timestamp: $TS" \
  -d '{"format":"csv"}'
# Expected: 400

# Wrong signature → 403
curl -s -o /dev/null -w "%{http_code}" -X POST http://localhost:8080/analytics/export \
  -H "Authorization: Bearer $COACH_TOKEN" \
  -H "Content-Type: application/json" \
  -H "X-Timestamp: $TS" \
  -H "X-Signature: deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef" \
  -d '{"format":"csv"}'
# Expected: 403
```

**Signing protocol**: `hex(HMAC-SHA256(HMAC_SECRET, "{unix_ts}:{METHOD}:{path}"))`.
Timestamp tolerance: ±300 seconds (replay protection).

---

### Rate Limiting

Policy: **60 requests / 60-second sliding window** per Bearer token (or per IP for unauthenticated).

```bash
# Trigger rate limit: send 61 rapid requests with the same token
TOKEN=$(curl -s -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"Admin1234!"}' | jq -r '.token')

for i in $(seq 1 62); do
  STATUS=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer $TOKEN" http://localhost:8080/health)
  echo "Request $i: HTTP $STATUS"
  [ "$STATUS" = "429" ] && echo "Rate limit triggered at request $i" && break
done
# Expected: HTTP 429 at request 61 with Retry-After header
```

---

### Account Lockout and CAPTCHA

Policy:
- **5 wrong passwords** in 15 minutes → CAPTCHA required on next attempt
- **10 wrong passwords** in 15 minutes → account locked for 15 minutes

```bash
# Simulate CAPTCHA threshold (5 wrong passwords)
for i in $(seq 1 5); do
  curl -s -X POST http://localhost:8080/auth/login \
    -H "Content-Type: application/json" \
    -d '{"username":"admin","password":"WrongPass!"}' | jq '.error // "401"'
done

# 6th attempt without CAPTCHA → captcha_required
curl -s -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"WrongPass!"}' | jq .
# Expected: HTTP 403 {"error":"captcha_required","captcha_challenge":"...","captcha_token":"..."}

# Recover with correct credentials to unlock (reset failure counter):
curl -s -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"Admin1234!"}' | jq '.user.username'
# Expected: "admin"
```

> **Note**: Do not run this against the `admin` user in production — use the `testlockout`
> test user (created by `run_tests.sh`) or a dedicated test account.

---

### Encryption Key Rotation

**Check current key age:**

```bash
docker compose exec db psql -U vitalpath -d vitalpath_db -c \
  "SELECT key_version, rotated_at,
          EXTRACT(DAY FROM NOW() - rotated_at)::int AS age_days
   FROM key_rotation_logs ORDER BY rotated_at DESC LIMIT 5;"
```

**Rotation procedure** (when key age ≥ 180 days):

```bash
# 1. Generate new 32-byte key
NEW_KEY=$(openssl rand -base64 32)
echo "New key: $NEW_KEY"

# 2. Update .env
#    FIELD_ENCRYPTION_KEY=<new_key>
#    ENCRYPTION_KEY_VERSION=v2   (increment each rotation)

# 3. Restart application (picks up new key, runs re-encryption migration)
docker compose up -d --build app

# 4. Record the rotation in key_rotation_logs
docker compose exec db psql -U vitalpath -d vitalpath_db -c \
  "INSERT INTO key_rotation_logs (key_version, notes, fields_updated)
   VALUES ('v2', 'Manual rotation', (SELECT COUNT(*) FROM health_profiles));"

# 5. Verify encrypted fields still readable (decryption with new key)
curl -s http://localhost:8080/profile/30000000-0000-0000-0000-000000000001 \
  -H "Authorization: Bearer $TOKEN" | jq '.dietary_notes'

# 6. Confirm no NULL encryption_key_id
docker compose exec db psql -U vitalpath -d vitalpath_db -c \
  "SELECT COUNT(*) FROM health_profiles WHERE encryption_key_id IS NULL OR encryption_key_id = '';"
# Expected: 0
```

**Expected startup log** (check with `docker compose logs app | grep KEY_ROTATION`):
- Within threshold: `Key rotation status: OK  days_until_rotation=N`
- Overdue: `SECURITY_KEY_ROTATION_NEEDED: encryption key is overdue for rotation`

---

### Backup and Restore

```bash
# Manual backup
docker compose exec backup /scripts/backup.sh
# Produces: /backups/vitalpath_backup_YYYYMMDD_HHMMSSZ.sql.gz.enc
#           /backups/vitalpath_backup_YYYYMMDD_HHMMSSZ.sha256
#           Updated: /backups/manifest.csv

# Verify backup integrity
docker compose exec backup sh -c \
  'sha256sum --check $(ls /backups/*.sha256 | tail -1) && echo "Checksum OK"'

# Interactive restore (prompts for confirmation)
docker compose exec backup /scripts/restore.sh

# Restore to a specific DSN (no prompt)
RESTORE_NO_CONFIRM=1 docker compose exec backup \
  /scripts/restore.sh /backups/<file>.sql.gz.enc postgres://user:pass@host/db

# Run quarterly restore drill immediately
docker compose exec backup /scripts/restore_drill.sh
# Expected exit code: 0 (PASS)
# Drill log: /backups/drill.log
# Drill history: /backups/drill_history.csv

# View drill history
docker compose exec backup cat /backups/drill_history.csv
```

**Backup encryption**: AES-256-CBC with PBKDF2 key derivation (100,000 iterations, random salt per backup). The passphrase is `BACKUP_ENCRYPTION_KEY` from the environment.

---

## Running Tests

The project ships with **two complementary test layers**, both executed inside Docker:

1. **Rust-native tests** — fast, in-process unit and integration tests covering business
   logic, model validation, error handling, permission boundaries, middleware behaviour,
   and cryptographic round-trips. No Postgres needed.
2. **Bash API tests** (`./run_tests.sh`) — end-to-end black-box tests that exercise the
   running binary against a live database in Docker.

### Rust unit + integration tests

All Rust tests run inside Docker — no local Rust toolchain required:

```bash
# Run every Rust test (unit modules under src/ + integration tests under tests/)
docker compose run --rm app cargo test

# Run a single test binary (file under tests/)
docker compose run --rm app cargo test --test crypto_roundtrip

# Run a single inline #[cfg(test)] mod
docker compose run --rm app cargo test --lib auth::role::tests
```

The Rust test suite covers:

| Area | Test location |
|------|---------------|
| AppError ↔ HTTP status code mapping | `src/errors.rs::tests`, `tests/error_responses.rs` |
| AES-256-GCM field cipher (round-trip, tamper, key/nonce mismatch) | `src/crypto.rs::tests`, `tests/crypto_roundtrip.rs` |
| Argon2id password hash + verify | `src/auth/passwords.rs::tests` |
| Role parsing + permission helpers | `src/auth/role.rs::tests`, `tests/permission_boundaries.rs` |
| CAPTCHA generate / verify (incl. expiry, tamper) | `src/auth/captcha.rs::tests` |
| HMAC request signing (success + every failure path) | `src/security/hmac_sign.rs::tests` |
| Sliding-window rate limiter (logic + middleware) | `src/security/rate_limit.rs::tests`, `tests/rate_limit_middleware.rs` |
| Identifier masking | `src/security/masking.rs::tests` |
| AppConfig env loading + panic paths | `src/config.rs::tests` |
| Prometheus registry, p95 estimation, pool gauges | `src/metrics.rs::tests` |
| Goal direction + completion logic | `src/models/goal.rs::tests` |
| Metric type catalogue + range validation | `src/models/metric.rs::tests` |
| Work-order state machine (full transition matrix) | `src/models/work_order.rs::tests` |
| Health profile DTO validation | `src/models/health_profile.rs::tests` |
| Analytics filter parsing + export validation | `src/models/analytics.rs::tests` |
| Notification + workflow DTO + constants | `src/models/notification.rs::tests`, `src/models/workflow.rs::tests` |
| Liveness endpoint + security headers | `tests/health_endpoints.rs`, `tests/security_headers.rs` |
| Auth extractor (missing / malformed Bearer) | `tests/middleware_unauthenticated.rs` |

### Code coverage

Coverage is measured with [`cargo-tarpaulin`](https://github.com/xd009642/tarpaulin),
configured in `tarpaulin.toml`. The configuration enforces a **90 % line-coverage
floor** — `cargo tarpaulin` exits non-zero if coverage drops below this threshold
(use this as a CI gate).

```bash
# Run coverage via Docker (works on Linux, Windows, and macOS — no local install needed).
./scripts/coverage.sh --docker
```

Outputs land under `target/tarpaulin/`:

| Artifact | Use |
|----------|-----|
| `tarpaulin-report.html` | Browseable per-file report |
| `lcov.info` | Upload to Codecov / Coveralls |
| `tarpaulin-report.json` | Badge / dashboard data |

The badge at the top of this README points to the local config; in CI, replace
it with the Codecov / Coveralls URL once configured.

### Bash API tests

```bash
./run_tests.sh
```

Starts the stack if needed, waits for health, runs all unit and API tests, reports pass/fail.

```bash
./run_tests.sh --no-start    # stack already running
./run_tests.sh --teardown    # stop stack after tests
```

### Test Structure

```
repo/
├── src/
│   ├── …/                  # every business-logic module ships its own
│   │                       # `#[cfg(test)] mod tests` block
├── tests/                  # blackbox integration tests (cargo test --test …)
│   ├── crypto_roundtrip.rs
│   ├── error_responses.rs
│   ├── health_endpoints.rs
│   ├── middleware_unauthenticated.rs
│   ├── permission_boundaries.rs
│   ├── rate_limit_middleware.rs
│   └── security_headers.rs
├── unit_tests/             # focused single-operation Bash tests
│   ├── test_01_health.sh
│   ├── test_02_auth_success.sh
│   ├── test_03_auth_failures.sh
│   ├── test_04_rbac.sh
│   └── test_05_validation.sh
├── API_tests/              # end-to-end workflow Bash tests
│   ├── test_01_auth_lifecycle.sh
│   ├── test_02_health_profile.sh
│   ├── test_03_metrics.sh
│   ├── test_04_goals.sh
│   ├── test_05_audit_logs.sh
│   ├── test_06_persistence.sh
│   ├── test_07_hmac_signing.sh     # HMAC signing: valid/invalid/stale
│   ├── test_08_rate_limiting.sh    # rate limit 429, lockout 423, CAPTCHA 403
│   ├── test_09_key_rotation.sh     # key age, column existence, encrypt/decrypt round-trip
│   ├── test_10_backup_restore.sh  # manual backup, checksum, drill, history
│   ├── test_11_workflows.sh        # template CRUD, instance state machine, approvals, SLA
│   ├── test_12_notifications.sh   # create, list, mark-read, subscriptions, schedules
│   ├── test_13_security_matrix.sh # 401 matrix, RBAC negatives, object-level auth, org isolation
│   └── test_14_export_download.sh # export download, path traversal, role auth
├── run_tests.sh
└── tests_common.sh      # shared helpers sourced by all tests
```

### Coverage Matrix

| Feature | Test file(s) |
|---------|-------------|
| Health endpoint | `unit_tests/test_01_health.sh` |
| Auth success + token lifecycle | `unit_tests/test_02_auth_success.sh`, `API_tests/test_01_auth_lifecycle.sh` |
| Auth failure + error shape | `unit_tests/test_03_auth_failures.sh` |
| RBAC enforcement | `unit_tests/test_04_rbac.sh`, `API_tests/test_05_audit_logs.sh` |
| Input validation + boundaries | `unit_tests/test_05_validation.sh` |
| Health profile CRUD | `API_tests/test_02_health_profile.sh` |
| Metric entries + summary | `API_tests/test_03_metrics.sh` |
| Goals workflow + direction | `API_tests/test_04_goals.sh` |
| Audit log access + pagination | `API_tests/test_05_audit_logs.sh` |
| Data persistence + work orders | `API_tests/test_06_persistence.sh` |
| HMAC signing | `API_tests/test_07_hmac_signing.sh` |
| Rate limiting + lockout + CAPTCHA | `API_tests/test_08_rate_limiting.sh` |
| Key rotation enforcement | `API_tests/test_09_key_rotation.sh` |
| Backup/restore drill | `API_tests/test_10_backup_restore.sh` |
| Workflow templates + approval state machine | `API_tests/test_11_workflows.sh` |
| Notifications + subscriptions + schedules | `API_tests/test_12_notifications.sh` |
| Security matrix: 401, RBAC, object-level auth, org isolation | `API_tests/test_13_security_matrix.sh` |
| Export download + path traversal + role auth | `API_tests/test_14_export_download.sh` |

---

## Configuration

All configuration is environment-based. Copy `.env.example` to `.env` and set values before production deployment.

| Variable | Default | Description |
|----------|---------|-------------|
| `JWT_SECRET` | *(placeholder)* | JWT signing key (min 32 chars) |
| `FIELD_ENCRYPTION_KEY` | *(placeholder)* | AES-256 field encryption key (base64, 32 bytes) |
| `HMAC_SECRET` | *(placeholder)* | HMAC-SHA256 signing secret |
| `BACKUP_ENCRYPTION_KEY` | *(placeholder)* | AES-256-CBC backup passphrase |
| `RUST_LOG` | `info` | Log level (`error`, `warn`, `info`, `debug`) |
| `RETAIN_DAYS` | `30` | Days of backups to retain |

Generate secrets:
```bash
openssl rand -base64 32   # JWT_SECRET, FIELD_ENCRYPTION_KEY
openssl rand -hex 32      # HMAC_SECRET, BACKUP_ENCRYPTION_KEY
```

---

## Data Volumes

| Volume | Mount | Contents |
|--------|-------|----------|
| `pg_data` | `/var/lib/postgresql/data` | PostgreSQL data files |
| `backup_data` | `/backups` | Encrypted `.sql.gz.enc` archives + logs |
| `exports_data` | `/exports` | Analytics export files |

---

## Backup Operations

```bash
# Manual immediate backup
docker compose exec backup /scripts/backup.sh

# Restore from latest backup (interactive confirmation)
docker compose exec backup /scripts/restore.sh

# Run restore drill immediately
docker compose exec backup /scripts/restore_drill.sh
```

---

## Offline Compliance

All services run locally with no external network dependencies:
- No external APIs called at runtime
- No cloud storage — all data in named Docker volumes
- All container images are standard public images (postgres, debian, rust)
- Build dependencies (Rust crates) are fetched at build time and cached in Docker layers
