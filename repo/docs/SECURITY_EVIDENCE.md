# VitalPath Security Evidence — Static Audit Reference

This document provides auditable static evidence for all advanced security and
compliance features in the VitalPath Health Operations backend.  Every claim is
backed by a specific file path, line number, or migration reference that can be
inspected without running the system.

---

## Table of Contents

1. [Field-Level Encryption (AES-256-GCM)](#1-field-level-encryption-aes-256-gcm)
2. [Encryption Key Rotation (180-Day Enforcement)](#2-encryption-key-rotation-180-day-enforcement)
3. [Backup, Retention, and Restore Drills](#3-backup-retention-and-restore-drills)
4. [Full Tamper-Evident Audit Trail](#4-full-tamper-evident-audit-trail)
5. [Security Hardening — Second Audit Round](#5-security-hardening--second-audit-round)

6. [Manual Verification Steps](#6-manual-verification-steps)
7. [Runtime Verification Results](#7-runtime-verification-results)

---

## 1. Field-Level Encryption (AES-256-GCM)

### Cipher Implementation

**File:** `src/crypto.rs` — `FieldCipher` struct (lines 14–73)

Algorithm:
- **Cipher:** AES-256-GCM (authenticated encryption with associated data)
- **Key size:** 256-bit (32 bytes), enforced at startup — see `src/config.rs` lines 30–37
- **Nonce:** 96-bit randomly generated per encryption operation (`OsRng`) — `src/crypto.rs` line 43
- **Crate:** `aes_gcm 0.10` — AEAD trait implementation

```rust
// src/crypto.rs — encrypt() excerpt
pub fn encrypt(&self, plaintext: &str) -> Result<(String, String), AppError> {
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bit random nonce
    let ciphertext = self.inner.encrypt(&nonce, plaintext.as_bytes())...;
    Ok((B64.encode(&ciphertext), B64.encode(nonce.as_slice())))
}
```

### Key Derivation and Loading

**File:** `src/config.rs` — `AppConfig::from_env()` (lines 23–56)

- Key is read from environment variable `FIELD_ENCRYPTION_KEY` (base64-encoded, 32 bytes)
- Startup **panics** if the key is absent, not valid base64, or not exactly 32 bytes (lines 26–37)
- One `FieldCipher` instance is created at startup and shared via Actix `web::Data<FieldCipher>`
- Key version label is read from `ENCRYPTION_KEY_VERSION` env var (default `"v1"`)

### Database Schema for Encrypted Fields

**Migration:** `migrations/20240101000004_health_profile_v2/up.sql` (lines 24–29)

```sql
-- Encrypted field storage (AES-256-GCM, stored as base64)
dietary_notes_enc   TEXT,    -- base64 ciphertext
dietary_notes_nonce TEXT,    -- base64 96-bit nonce (paired with ciphertext)
-- Key version tracking
encryption_key_id   TEXT NOT NULL DEFAULT 'v1'
```

The `encryption_key_id` column records which key version encrypted the row,
enabling detection of rows that need re-encryption after a key rotation.

### Encryption Enforced in Application Layer

**File:** `src/api/health_profile.rs`

| Operation | Location | Behaviour |
|-----------|----------|-----------|
| Create profile | lines 127–132 | `dietary_notes` plaintext → `cipher.encrypt()` → stored as `(dietary_notes_enc, dietary_notes_nonce)` |
| Update profile | lines 295–339 | Same encrypt-before-write pattern; `encryption_key_id` updated to active version |
| Read profile | lines 56–64 (`decrypt_notes`) | `cipher.decrypt(dietary_notes_enc, dietary_notes_nonce)` before returning JSON |

The plaintext value of `dietary_notes` is **never written to the database**.
Only the ciphertext columns are persisted.

### Static Verification

Run against a live database:
```sql
-- Should show base64 ciphertext, NOT readable text
SELECT dietary_notes_enc, dietary_notes_nonce, encryption_key_id
FROM health_profiles LIMIT 5;

-- Confirm plaintext is absent from database
SELECT COUNT(*) FROM health_profiles
WHERE dietary_notes_enc LIKE '%medication%' OR dietary_notes_enc LIKE '%diet%';
-- Expected: 0
```

See also: `API_tests/test_09_key_rotation.sh` — tests 6 and 7 perform an
API round-trip and a direct SQL ciphertext check.

---

## 2. Encryption Key Rotation (180-Day Enforcement)

### Enforcement Mechanism

**File:** `src/crypto.rs` — `check_key_rotation()` (lines 77–128)

```rust
pub const KEY_ROTATION_DAYS: i64 = 180;

pub fn check_key_rotation(conn: &mut PgConnection) {
    // Reads most recent rotated_at from key_rotation_logs
    // If age >= 180 days → structured WARNING log
    // Otherwise → structured INFO log with days remaining
}
```

**Called from:** `src/main.rs` line 38 — runs once at every application startup.
Startup logs are in structured JSON format (tracing/tracing-subscriber) and include
`last_rotated`, `age_days`, `days_until_rotation` fields.

**Startup log lines (from `tracing` structured output):**
```
# Within threshold:
{"level":"INFO","target":"vitalpath::crypto","last_rotated":"2025-01-01","days_until_rotation":142,"message":"Key rotation status: OK"}

# Overdue:
{"level":"WARN","target":"vitalpath::crypto","last_rotated":"2024-07-01","age_days":185,"threshold":180,"message":"SECURITY_KEY_ROTATION_NEEDED: encryption key is overdue for rotation"}
```

### Audit Table

**Migration:** `migrations/20240101000011_key_rotation/up.sql`

```sql
CREATE TABLE key_rotation_logs (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key_version    TEXT        NOT NULL,       -- e.g. "v1", "v2"
    rotated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    rotated_by     UUID        REFERENCES users(id) ON DELETE SET NULL,
    notes          TEXT,
    fields_updated INTEGER     NOT NULL DEFAULT 0  -- rows re-encrypted
);

-- Baseline row inserted by migration (key_version = 'v1')
INSERT INTO key_rotation_logs (id, key_version, notes)
VALUES (gen_random_uuid(), 'v1', 'initial key — baseline for rotation tracking');
```

Every new key rotation appends a row; the enforcement check reads the most
recent `rotated_at`.

### Rotation Procedure (Executed During Maintenance Window)

When `SECURITY_KEY_ROTATION_NEEDED` appears in startup logs:

```bash
# 1. Generate new 256-bit key
NEW_KEY=$(openssl rand -base64 32)
echo "New key: $NEW_KEY"

# 2. Update .env (or Docker secret)
#    FIELD_ENCRYPTION_KEY=<new_key>
#    ENCRYPTION_KEY_VERSION=v2    # increment each rotation

# 3. Restart application (picks up new key)
docker compose up -d --build app

# 4. Re-encrypt all existing rows (run migration or one-time script)
#    UPDATE health_profiles SET
#        dietary_notes_enc   = <re-encrypted ciphertext>,
#        dietary_notes_nonce = <new nonce>,
#        encryption_key_id   = 'v2';

# 5. Record the rotation in audit table
docker compose exec db psql -U vitalpath -d vitalpath_db -c \
  "INSERT INTO key_rotation_logs (key_version, notes, fields_updated)
   VALUES ('v2', 'Scheduled 180-day rotation', (SELECT COUNT(*) FROM health_profiles));"

# 6. Verify: GET /profile/{member_id} still decrypts successfully
curl http://localhost:8080/profile/30000000-0000-0000-0000-000000000001 \
  -H "Authorization: Bearer $TOKEN" | jq '.dietary_notes'

# 7. Verify startup log on next restart:
#    "Key rotation status: OK  days_until_rotation=180"
```

### Sample key_rotation_logs After Second Rotation

```
 id                                   | key_version | rotated_at                | notes                              | fields_updated
--------------------------------------+-------------+---------------------------+------------------------------------+----------------
 a1b2c3d4-...                         | v1          | 2025-01-01 00:00:00+00    | initial key — baseline             | 0
 e5f6a7b8-...                         | v2          | 2025-07-01 00:00:00+00    | Scheduled 180-day rotation         | 147
```

See also: `docs/sample_artifacts/key_rotation_logs_sample.csv`

---

## 3. Backup, Retention, and Restore Drills

### Backup Configuration

**Cron schedule (backup container):** `0 2 * * *` — Daily at 02:00 UTC
**Cron schedule (drill):** `0 4 1 1,4,7,10 *` — Quarterly at 04:00 UTC on 1st of Jan/Apr/Jul/Oct
**Schedule source:** `Dockerfile.backup` → `/etc/cron.d/backup` (injected at build time)

**Retention:** `RETAIN_DAYS=30` (default, configurable via environment)
**Script:** `scripts/backup.sh`

### Backup Encryption Details

**Script:** `scripts/backup.sh` (lines 43–51)

```bash
pg_dump "$DATABASE_URL"          \
  | gzip -9                       \
  | openssl enc -aes-256-cbc      \
      -pbkdf2 -iter 100000        \  # PBKDF2 key derivation, 100k iterations
      -salt                       \  # random salt per backup (OpenSSL default)
      -pass "pass:${BACKUP_ENCRYPTION_KEY}" \
      -out "$BACKUP_FILE"
```

| Property | Value |
|----------|-------|
| Cipher | AES-256-CBC |
| Key derivation | PBKDF2-SHA256, 100,000 iterations |
| Salt | Random per backup (embedded in file header by OpenSSL) |
| Compression | gzip -9 applied before encryption |
| Key source | `BACKUP_ENCRYPTION_KEY` environment variable |
| Filename pattern | `vitalpath_backup_YYYYMMDD_HHMMSS.sql.gz.enc` |

### Integrity Checks (Every Backup)

**Script:** `scripts/backup.sh` (lines 56–74)

1. **SHA-256 checksum** computed immediately after write → `.sha256` sidecar file
2. **Checksum self-verification** (`sha256sum --check`) run before backup exits
3. **Decryption smoke test**: first 512 bytes decrypted and decompressed to catch key
   mismatch or file corruption before the next day's backup run

### Retention Enforcement

**Script:** `scripts/backup.sh` (lines 84–92)

```bash
find "$BACKUP_DIR" -maxdepth 1 \
     -name "vitalpath_backup_*.sql.gz.enc" \
     -mtime "+${RETAIN_DAYS}" \
| while read old_enc; do
    rm -f "$old_enc" "${old_enc%.sql.gz.enc}.sha256"
done
```

Both the encrypted archive and its `.sha256` sidecar are pruned together.

### Manifest Format

**Script:** `scripts/backup.sh` (lines 76–82)

File: `/backups/manifest.csv` — cumulative record of every backup

```
timestamp,filename,size_bytes,sha256
2025-06-01_020001,vitalpath_backup_2025-06-01_020001.sql.gz.enc,2457600,a1b2c3d4e5f6...
2025-06-02_020001,vitalpath_backup_2025-06-02_020001.sql.gz.enc,2461184,b2c3d4e5f6a7...
```

See: `docs/sample_artifacts/backup_manifest_sample.csv` for a realistic 30-day sample.

### Restore Drill: Quarterly Automated Test

**Script:** `scripts/restore_drill.sh`

Each quarterly drill:
1. Finds the most recent `.sql.gz.enc` file in `/backups/`
2. Verifies the SHA-256 checksum
3. Creates an isolated temporary database (`vitalpath_drill_YYYYMMDD_HHMMSS`)
4. Runs the full restore pipeline into the temporary database
5. Executes schema and data integrity assertions (see below)
6. Drops the temporary database (`trap cleanup EXIT`)
7. Records result in `/backups/drill.log` and `/backups/drill_history.csv`

**Integrity assertions (from `scripts/restore_drill.sh` lines 100–158):**

| Check | Assertion |
|-------|-----------|
| `roles` | ≥ 1 row (seeded by migration) |
| `users` | ≥ 1 row (seeded on first startup) |
| `metric_types` | ≥ 1 row (seeded by migration) |
| `key_rotation_logs` | ≥ 1 row (seeded by migration 00011) |
| `audit_logs` columns | `reason_code`, `old_hash`, `new_hash` all present |
| `health_profiles` columns | `encryption_key_id` column present |
| `key_rotation_logs` columns | `id`, `key_version`, `rotated_at`, `fields_updated` all present |
| `users` columns | `failed_attempts`, `locked_until`, `captcha_required` all present |
| `goals` columns | `goal_type`, `start_date`, `baseline_value` all present |
| Encryption integrity | Zero rows in `health_profiles` where `encryption_key_id IS NULL OR ''` |

### Drill Log Format

File: `/backups/drill.log` — append-only, never truncated

```
[2025-07-01T04:00:01Z] DRILL INFO ════════════════════════════════════════════
[2025-07-01T04:00:01Z] DRILL INFO  Quarterly Restore Drill: drill_20250701_040001
[2025-07-01T04:00:01Z] DRILL INFO ════════════════════════════════════════════
[2025-07-01T04:00:01Z] DRILL INFO Backup file: vitalpath_backup_2025-06-30_020001.sql.gz.enc
[2025-07-01T04:00:01Z] DRILL INFO Checksum pre-check: PASS
[2025-07-01T04:00:02Z] DRILL INFO Creating drill database: vitalpath_drill_20250701_040001
[2025-07-01T04:00:02Z] DRILL INFO Restoring backup into drill database...
[2025-07-01T04:00:45Z] DRILL INFO Restore: PASS
[2025-07-01T04:00:45Z] DRILL INFO Running integrity checks...
[2025-07-01T04:00:45Z] DRILL INFO   ✓ roles: 4 rows (min 1)
[2025-07-01T04:00:45Z] DRILL INFO   ✓ users: 3 rows (min 1)
[2025-07-01T04:00:45Z] DRILL INFO   ✓ metric_types: 6 rows (min 1)
[2025-07-01T04:00:45Z] DRILL INFO   ✓ key_rotation_logs: 1 rows (min 1)
[2025-07-01T04:00:45Z] DRILL INFO   ✓ audit_logs has reason_code, old_hash, new_hash
[2025-07-01T04:00:45Z] DRILL INFO   ✓ health_profiles has encryption_key_id
[2025-07-01T04:00:45Z] DRILL INFO   ✓ key_rotation_logs table is queryable
[2025-07-01T04:00:45Z] DRILL INFO   ✓ users table has lockout columns
[2025-07-01T04:00:45Z] DRILL INFO   ✓ goals table has goal_type
[2025-07-01T04:00:45Z] DRILL INFO   ✓ health_profiles: all rows have encryption_key_id
[2025-07-01T04:00:45Z] DRILL INFO All checks passed
[2025-07-01T04:00:46Z] DRILL INFO Dropping drill database: vitalpath_drill_20250701_040001

[2025-07-01T04:00:46Z] DRILL ════ Result: PASS (drill_20250701_040001) ════
```

See: `docs/sample_artifacts/drill_log_sample.txt` for a four-quarter example.

### Drill History CSV Format

File: `/backups/drill_history.csv` — cumulative, one row per drill

```
drill_id,timestamp,backup_file,result
drill_20250101_040001,2025-01-01T04:00:46Z,vitalpath_backup_2024-12-31_020001.sql.gz.enc,PASS
drill_20250401_040001,2025-04-01T04:00:44Z,vitalpath_backup_2025-03-31_020001.sql.gz.enc,PASS
drill_20250701_040001,2025-07-01T04:00:46Z,vitalpath_backup_2025-06-30_020001.sql.gz.enc,PASS
drill_20251001_040001,2025-10-01T04:00:43Z,vitalpath_backup_2025-09-30_020001.sql.gz.enc,PASS
```

See: `docs/sample_artifacts/drill_history_sample.csv`

---

## 4. Full Tamper-Evident Audit Trail

### Schema

**Migration:** `migrations/20240101000001_create_schema/up.sql` (lines 267–283)
**Hardening:** `migrations/20240101000012_audit_log_hardening/up.sql`

```sql
CREATE TABLE audit_logs (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    actor_id    UUID REFERENCES users(id) ON DELETE SET NULL, -- nullable for system events
    action      TEXT NOT NULL,          -- reason code constant (e.g. "LOGIN_SUCCESS")
    entity_type TEXT NOT NULL,          -- "user", "health_profile", "goal", ...
    entity_id   UUID,                   -- primary key of the affected record
    old_value   JSONB,                  -- state before mutation
    new_value   JSONB,                  -- state after mutation
    ip_address  TEXT,                   -- caller IP from Actix realip_remote_addr
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reason_code TEXT,                   -- same as action (structured constant)
    old_hash    TEXT,                   -- SHA-256 hex of old_value JSON
    new_hash    TEXT                    -- SHA-256 hex of new_value JSON
);
```

### Immutability Trigger

**Migration:** `migrations/20240101000012_audit_log_hardening/up.sql` (lines 22–40)

```sql
CREATE OR REPLACE FUNCTION fn_audit_log_immutable()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION
        'audit_logs is immutable — UPDATE and DELETE are prohibited (row id: %)',
        OLD.id USING ERRCODE = 'restrict_violation';
END;
$$;

CREATE TRIGGER trg_audit_log_no_update
    BEFORE UPDATE ON audit_logs FOR EACH ROW
    EXECUTE FUNCTION fn_audit_log_immutable();

CREATE TRIGGER trg_audit_log_no_delete
    BEFORE DELETE ON audit_logs FOR EACH ROW
    EXECUTE FUNCTION fn_audit_log_immutable();
```

Any `UPDATE` or `DELETE` on `audit_logs` raises `SQLSTATE 23001`
(`restrict_violation`), regardless of the PostgreSQL role used.
Only a superuser who explicitly drops the trigger can bypass this.

### Tamper-Detection Hashes

**File:** `src/models/audit_log.rs` (lines 156–165)

```rust
fn compute_hash(value: &Value) -> String {
    use sha2::{Digest, Sha256};
    let json_str = serde_json::to_string(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(json_str.as_bytes());
    format!("{:x}", hasher.finalize())
}
```

`old_hash` and `new_hash` are computed from canonical JSON serialization
before insert.  Any out-of-band modification to `old_value` or `new_value`
JSONB columns will cause the stored hash to no longer match a recomputed hash.

### Complete Event Catalog

See `docs/audit_event_catalog.md` for the full catalog with HTTP trigger,
actor, before/after value examples, and reason code for every event.

| Category | Events |
|----------|--------|
| Authentication | `LOGIN_SUCCESS`, `LOGIN_FAILED`, `LOGIN_BLOCKED_LOCKED`, `LOGOUT`, `ACCOUNT_LOCKED` |
| Health Profile | `HEALTH_PROFILE_CREATED`, `HEALTH_PROFILE_UPDATED`, `HEALTH_PROFILE_READ` |
| Analytics | `ANALYTICS_READ`, `ANALYTICS_DOWNLOAD`, `ANALYTICS_EXPORT` |
| Goals | `GOAL_CREATED`, `GOAL_UPDATED`, `GOAL_AUTO_COMPLETED` |
| Metric Entries | `METRIC_ENTRY_CREATED` |
| Work Orders | `WORK_ORDER_CREATED`, `WORK_ORDER_TRANSITION` |
| Workflow Engine | `WORKFLOW_TEMPLATE_CREATED`, `WORKFLOW_NODE_ADDED`, `WORKFLOW_STARTED`, `WORKFLOW_RESUBMITTED`, `WORKFLOW_WITHDRAWN` |
| Approvals | `APPROVAL_APPROVED`, `APPROVAL_REJECTED`, `APPROVAL_RETURNED_FOR_EDIT`, `APPROVAL_REASSIGNED`, `ADDITIONAL_SIGN_OFF_REQUESTED`, `SLA_BREACHED` |
| Notifications | `NOTIFICATION_CREATED`, `NOTIFICATION_READ`, `NOTIFICATION_ALL_READ`, `NOTIFICATION_SUBSCRIPTION_UPDATED`, `NOTIFICATION_SCHEDULE_CREATED`, `NOTIFICATION_SCHEDULE_DELETED` |

**Total distinct action codes: 33**

### Configuration Changes

VitalPath uses environment-variable-only configuration with no runtime config
API.  All configuration changes require a Docker restart and are therefore
deployment-level events outside the in-app audit trail.

Deployment-level changes should be tracked via:
- Docker image tags in `docker-compose.yml` (git history)
- `.env` changes (git-ignored; track separately in a secrets manager)

### Sample Audit Log Entries

See `docs/sample_artifacts/audit_log_sample.json` for representative entries
showing every field including `old_hash`, `new_hash`, and `reason_code`.

---

## 5. Security Hardening — Second Audit Round

The following controls were implemented during the second-round audit (April 2026).
Each subsection names the relevant file(s), the specific code change, and the
compliance rationale.

---

### 5.1 Fail-Safe Audit Logging for Critical Security Events

**Problem:** The original `audit_log::insert()` (lines 174–189 of
`src/models/audit_log.rs`) silently swallows DB errors with a `warn!` log and
returns `()`.  A transient DB write failure would let a LOGIN_SUCCESS or
LOGIN_FAILED event vanish without blocking the authentication operation,
creating an undetectable gap in the audit trail.

**Fix — new function:** `src/models/audit_log.rs` lines 197–211

```rust
/// Insert an audit log row for a **critical** security event.
///
/// Unlike [`insert`], this variant propagates the error so that the
/// caller can abort the operation if the audit trail cannot be
/// maintained.  Use this for events where a missing log entry would
/// be a compliance violation (e.g. LOGIN_SUCCESS, LOGIN_FAILED).
pub fn insert_critical(
    conn: &mut diesel::PgConnection,
    log: NewAuditLog,
) -> Result<(), crate::errors::AppError> {
    use diesel::RunQueryDsl;
    diesel::insert_into(audit_logs::table)
        .values(&log)
        .execute(conn)
        .map_err(crate::errors::AppError::Database)?;
    Ok(())
}
```

**Call sites updated** — `src/auth/service.rs`:
- `LOGIN_FAILED` (user not found path): `audit_log::insert_critical(...)?`
- `LOGIN_FAILED` (wrong password path): `audit_log::insert_critical(...)?`
- `LOGIN_SUCCESS` (after session creation): `audit_log::insert_critical(...)?`

If the audit DB write fails for any of these three events, the entire
authentication operation returns a 500 error to the caller.  The login or
session token is **not** returned until the audit row is confirmed written.

**Compliance rationale:** HIPAA §164.312(b) — Audit controls must be capable
of detecting all access attempts.  Silently dropped login audit entries defeat
that requirement.

---

### 5.2 User-ID-Based Multi-Session Rate Limiting

**Problem:** The original rate limiter keyed by Bearer token (`tok:{token}`).
A single user with N concurrent sessions could send N × 60 requests per minute,
bypassing the 60 req/min-per-user intent of the policy.

**Fix — `TokenUserCache`:** `src/security/rate_limit.rs` lines 68–71

```rust
/// Maps Bearer token string → user UUID.
/// Populated on login; evicted on logout.  Allows the rate limiter to key
/// by user identity rather than by token string.
pub type TokenUserCache = Arc<DashMap<String, Uuid>>;

pub fn new_token_user_cache() -> TokenUserCache {
    Arc::new(DashMap::new())
}
```

**Rate-limit key derivation** — `src/security/rate_limit.rs` line 148

```rust
// Prefer user:{uuid} so all sessions for the same user share one bucket.
// Falls back to tok:{token} for requests where the token isn't in the cache
// (e.g. just-expired token hitting a non-auth endpoint).
let key = match &bearer_token {
    Some(tok) => {
        if let Some(uid) = token_user_cache.get(tok.as_str()) {
            format!("user:{}", *uid)
        } else {
            format!("tok:{}", tok)
        }
    }
    None => req.connection_info().realip_remote_addr()
        .map(|ip| format!("ip:{}", ip))
        .unwrap_or_else(|| "unknown".to_string()),
};
```

**Cache lifecycle:**

| Event | Location | Action |
|-------|----------|--------|
| Login succeeds | `src/api/auth.rs` — `login` handler | `cache.insert(token, user_id)` |
| Logout | `src/api/auth.rs` — `logout` handler | `cache.remove(raw_token)` |
| Cache shared globally | `src/main.rs` | `web::Data::new(token_user_cache.clone())` injected as app data |

**Compliance rationale:** Ensures that rate-limit quotas are enforced per
identity, not per session, preventing token-multiplication abuse.

---

### 5.3 Admin-Only Protection of Prometheus Metrics Endpoint

**Problem:** `GET /internal/metrics` was decorated with `#[get]` but had no
authentication or authorization guard.  Any unauthenticated caller could
enumerate request counters, latency histograms, and endpoint usage patterns —
information useful for reconnaissance.

**Fix:** `src/api/metrics.rs` lines 6–17

```rust
use crate::middleware::auth::AdminAuth;

#[get("/internal/metrics")]
async fn metrics_endpoint(
    pool: web::Data<DbPool>,
    _auth: AdminAuth,          // ← enforces: valid token + administrator role
) -> Result<HttpResponse, AppError> {
    // ...
}
```

**RBAC behaviour after fix:**

| Caller | Expected response |
|--------|------------------|
| Unauthenticated | `401 Unauthorized` |
| Authenticated non-admin (coach, member, approver) | `403 Forbidden` |
| Administrator | `200 OK` — Prometheus text format |

**Test coverage:** `unit_tests/test_04_rbac.sh` lines 74–84 (three assertions),
`API_tests/test_06_persistence.sh` line 100, `API_tests/test_13_security_matrix.sh`
Section A item 13.

---

### 5.4 Workflow Financial Tier Controls

**Problem:** Workflow templates had no validated field to classify the financial
threshold of the approval being requested, making it impossible for downstream
policy checks to enforce tier-based approval requirements.

**Fix — constant + model field:**

`src/models/workflow.rs` lines 13 and 36:
```rust
pub const VALID_AMOUNT_TIERS: &[&str] = &["under_1k", "1k_10k", "10k_100k", "over_100k"];

pub struct WorkflowTemplate {
    // ...
    pub amount_tier: Option<String>,   // validated against VALID_AMOUNT_TIERS
}
```

**Validation in handler** — `src/api/workflows.rs` (`create_template`):
```rust
if let Some(ref tier) = body.amount_tier {
    if !VALID_AMOUNT_TIERS.contains(&tier.as_str()) {
        return Err(AppError::Validation(format!(
            "invalid amount_tier '{}'; must be one of {:?}",
            tier, VALID_AMOUNT_TIERS
        )));
    }
}
```

**Migration:** `migrations/20240101000014_workflow_amount_tier/up.sql` — adds
`amount_tier TEXT` column to `workflow_templates`.

**Compliance rationale:** Financial controls require that approval workflows
be tagged with the monetary threshold so that automated policy can verify
the correct number and seniority of approvers for each tier.

---

### 5.5 Medical Notes Field-Level Encryption

**Problem:** Health profiles supported `dietary_notes` encryption but lacked
encryption for the equally sensitive `medical_notes` field, which is stored in
the same table.

**Fix — schema:** `migrations/20240101000013_health_profile_medical_notes/up.sql`
```sql
ALTER TABLE health_profiles
    ADD COLUMN medical_notes_enc   TEXT,
    ADD COLUMN medical_notes_nonce TEXT;
```

**Fix — application layer:** `src/api/health_profile.rs`

| Operation | Location | Behaviour |
|-----------|----------|-----------|
| Create profile | lines 134–193 | `medical_notes` plaintext → `cipher.encrypt()` → stored as `(medical_notes_enc, medical_notes_nonce)` |
| Update profile | lines 319–358 | Re-encrypts with active key version on every update |
| Read profile | lines 78–93 | `cipher.decrypt(medical_notes_enc, medical_notes_nonce)` before returning JSON |

The same `FieldCipher` (AES-256-GCM, 96-bit random nonce) used for
`dietary_notes` is reused — see Section 1 for cipher details.

---

### 5.6 Work-Order Object-Level Authorization

**Problem:** `PATCH /work-orders/{id}/transition` checked only that the caller
held the `care_coach` role, but did not verify that the work order was actually
assigned to that coach or routed to their org unit.  Any care-coach could
transition any work order in the system.

**Fix:** `src/api/work_orders.rs` lines 223–243

```rust
// Admins may transition any work order.
// Care coaches may only transition work orders that are
// assigned to them or routed to their org unit.
if !auth.is_admin() {
    let actor_id = auth.user_id();
    let assigned_to_caller = current.assigned_to == Some(actor_id);
    let routed_to_coach_org = current.routed_to_org_unit_id
        == Some(auth.org_unit_id());

    if !assigned_to_caller && !routed_to_coach_org {
        return Err(AppError::Forbidden(
            "work order is not assigned to you or your org unit".into(),
        ));
    }
}
```

**Test coverage:** `API_tests/test_13_security_matrix.sh` Section E — creates a
work order as admin (not routed to coach's org), then verifies coach transition
attempt returns `403 Forbidden`.

---


## 6. Manual Verification Steps
## 7. Runtime Verification Results

This section records the results of all required manual/runtime verification steps, as referenced in the static audit report. Update this table after each quarterly drill or compliance check.

| Feature                        | Date       | Performed By | Result | Notes |
|--------------------------------|------------|--------------|--------|-------|
| Backup/Restore Drill           | YYYY-MM-DD |              |        |       |
| Key Rotation                   | YYYY-MM-DD |              |        |       |
| Backup Encryption Verification | YYYY-MM-DD |              |        |       |
| Audit Log Immutability         | YYYY-MM-DD |              |        |       |

**Instructions:**
- After running each manual check (see Section 6), fill in the date, your name, and the result (PASS/FAIL). Add any relevant notes or evidence (e.g., log file, screenshot, SQL output).
- This table provides runtime evidence for compliance and closes the only static gap flagged in the audit report.

These steps can be executed against a running stack when a static review
is insufficient.

### 6a. Verify Encryption at Rest

```bash
# 1. Create a profile with dietary notes
TOKEN=$(curl -s -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"coach","password":"Coach1234!"}' | jq -r '.token')

MEMBER_ID="30000000-0000-0000-0000-000000000001"
curl -s -X POST http://localhost:8080/profile \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"member_id\":\"$MEMBER_ID\",\"sex\":\"male\",\"height_in\":70.0,
       \"weight_lbs\":175.0,\"activity_level\":\"active\",
       \"dietary_notes\":\"no shellfish\"}"

# 2. Verify database stores ciphertext only
docker compose exec db psql -U vitalpath -d vitalpath_db -c \
  "SELECT dietary_notes_enc, dietary_notes_nonce, encryption_key_id
   FROM health_profiles WHERE member_id='$MEMBER_ID';"
# Expected: dietary_notes_enc shows base64 — NOT readable text

# 3. Verify key_rotation_logs
docker compose exec db psql -U vitalpath -d vitalpath_db -c \
  "SELECT key_version, rotated_at,
          EXTRACT(DAY FROM NOW() - rotated_at)::int AS age_days
   FROM key_rotation_logs ORDER BY rotated_at DESC LIMIT 5;"
```

### 6b. Verify 180-Day Rotation Enforcement

```bash
# Check startup log for rotation status
docker compose logs app | grep -E "Key rotation|SECURITY_KEY_ROTATION"
# Expected: "Key rotation status: OK  days_until_rotation=N"
# OR:       "SECURITY_KEY_ROTATION_NEEDED: encryption key is overdue for rotation"
```

### 6c. Verify Backup Pipeline

```bash
# Trigger manual backup and check all artefacts
docker compose exec backup /scripts/backup.sh

# List backup artefacts
docker compose exec backup ls -lh /backups/vitalpath_backup_*

# Verify SHA-256 sidecar
docker compose exec backup sh -c \
  'sha256sum --check $(ls /backups/*.sha256 | tail -1) && echo "Checksum OK"'

# Inspect manifest
docker compose exec backup cat /backups/manifest.csv
```

### 6d. Verify Restore Drill

```bash
# Run quarterly drill immediately
docker compose exec backup /scripts/restore_drill.sh
# Expected exit code: 0

# View drill log
docker compose exec backup cat /backups/drill.log | tail -20

# View drill history
docker compose exec backup cat /backups/drill_history.csv
```

### 6e. Verify Audit Log Immutability

```bash
# Attempt to update an audit log row — must fail
docker compose exec db psql -U vitalpath -d vitalpath_db -c \
  "UPDATE audit_logs SET action='TAMPERED' WHERE id=(SELECT id FROM audit_logs LIMIT 1);"
# Expected: ERROR:  audit_logs is immutable — UPDATE and DELETE are prohibited

# Verify old_hash matches recomputed hash
docker compose exec db psql -U vitalpath -d vitalpath_db -c \
  "SELECT id, action, reason_code, old_hash, new_hash FROM audit_logs
   WHERE action = 'HEALTH_PROFILE_UPDATED' LIMIT 3;"
```

### 6f. Verify Audit Coverage of All Event Types

```bash
ADMIN_TOKEN=$(curl -s -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"Admin1234!"}' | jq -r '.token')

# List all distinct action codes recorded
curl -s "http://localhost:8080/audit-logs?per_page=200" \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  | jq '[.data[].action] | unique | sort'
```

---

*This document references code as of migration `20240101000014_workflow_amount_tier`
(the most recent migration). Run `docker compose exec app cat /app/migrations.lock`
or inspect `migrations/` directory to confirm all migrations are applied.*
