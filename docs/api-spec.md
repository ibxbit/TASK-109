
# API Specification – VitalPath Health Operations

This document outlines the RESTful API endpoints for the VitalPath Health Operations backend. All endpoints return JSON and expect JSON payloads unless otherwise specified.

**Base URL**: `http://<server-ip>:8080`

---

## Global Headers & Security

### Required for All Requests
- **`Authorization`**: `Bearer <token>` (Required for all routes except `/auth/login` and `/health`)
- **`X-Actor-User-Id`**: UUID of the acting user (from session)
- **`X-User-Role`**: One of `administrator`, `care_coach`, `approver`, `member`

### Write Operations
- **`X-Idempotency-Key`**: Unique client-generated string (UUID recommended) for safe retries

### Advanced Security
- **`X-Timestamp`**: Unix timestamp (seconds) for anti-replay (required for HMAC-signed endpoints)
- **`X-Signature`**: HMAC-SHA256 signature for privileged endpoints (see Security section)

---

## 1. Authentication & Session
| Method | Endpoint | Description | Access |
| :--- | :--- | :--- | :--- |
| `POST` | `/auth/login` | Login with username/password | Public |
| `POST` | `/auth/logout` | Invalidate session token | Authenticated |
| `GET`  | `/auth/me`    | Get current user info | Authenticated |

**Login Response Example:**
```json
{
	"user_id": "uuid",
	"username": "admin",
	"role": "administrator",
	"token": "session-token-uuid",
	"token_expires_at": "ISO-8601"
}
```

---

## 2. Health Profile
| Method | Endpoint | Description | Access |
| :--- | :--- | :--- | :--- |
| `POST` | `/profile` | Create member profile | Admin, Care Coach |
| `GET`  | `/profile/{member_id}` | Get member profile | Self, Care Coach, Admin |
| `PUT`  | `/profile/{member_id}` | Update profile | Self, Care Coach, Admin |

**Profile Fields:**
- `date_of_birth` (YYYY-MM-DD)
- `sex` (`male`/`female`/`other`)
- `height_inches`, `weight_pounds`
- `activity_level` (enum)
- `allergies_dietary_notes` (string, max 1000 chars, encrypted)

---

## 3. Metric Entries
| Method | Endpoint | Description | Access |
| :--- | :--- | :--- | :--- |
| `POST` | `/metrics` | Record a metric entry | Self, Care Coach, Admin |
| `GET`  | `/metrics` | List entries (by member, range) | Self, Care Coach, Admin |
| `GET`  | `/metrics/summary` | Aggregated summary | Self, Care Coach, Admin |

**Supported Metrics:** `weight`, `body_fat_percentage`, `waist`, `hip`, `chest`, `blood_glucose`

---

## 4. Goals
| Method | Endpoint | Description | Access |
| :--- | :--- | :--- | :--- |
| `POST` | `/goals` | Create goal | Self, Care Coach, Admin |
| `GET`  | `/goals?member_id=<id>` | List goals | Self, Care Coach, Admin |
| `PUT`  | `/goals/{id}` | Update goal | Self, Care Coach, Admin |

**Goal Fields:**
- `goal_type`: `fat_loss`, `muscle_gain`, `glucose_control`
- `start_date`, `target_date` (optional)
- `baseline_value`, `target_value`
- `status`: `active`, `paused`, `completed`

---

## 5. Workflow Engine
| Method | Endpoint | Description | Access |
| :--- | :--- | :--- | :--- |
| `POST` | `/workflow/templates` | Create approval template | Admin |
| `GET`  | `/workflow/templates` | List templates | Admin |
| `POST` | `/workflow/instances` | Start workflow instance | Authenticated |
| `PATCH`| `/workflow/instances/{id}` | Advance/reassign/return | Approver, Admin |

**Template Fields:**
- `business_type`, `org_unit_id`, `risk_tier`
- `nodes`: serial/parallel approval steps

**Instance Actions:**
- `submit`, `approve`, `reject`, `return_for_edit`, `withdraw`, `reassign`, `add_signoff`

---

## 6. Work Orders
| Method | Endpoint | Description | Access |
| :--- | :--- | :--- | :--- |
| `POST` | `/work-orders` | Create work order ticket | Authenticated |
| `PATCH`| `/work-orders/{id}/transition` | Advance state | Assigned roles |
| `GET`  | `/work-orders` | List/filter tickets | Role-based |

**States:** `intake → triage → in_progress → waiting_on_member → resolved → closed`

---

## 7. Notifications & Reminders
| Method | Endpoint | Description | Access |
| :--- | :--- | :--- | :--- |
| `POST` | `/notifications` | Create notification | System, Admin |
| `GET`  | `/notifications` | List notifications | Authenticated |
| `POST` | `/notifications/mark-read` | Mark as read | Authenticated |
| `POST` | `/subscriptions` | Subscribe to template | Authenticated |

**Features:**
- In-app only (no SMS/email)
- Frequency cap: 3 per template/user/day
- Retry: up to 5 times, exponential backoff
- Scheduled (e.g., daily 8:00 AM local), event-triggered

---

## 8. Analytics
| Method | Endpoint | Description | Access |
| :--- | :--- | :--- | :--- |
| `GET`  | `/analytics/metrics` | Aggregated program metrics | Admin, Care Coach |
| `POST` | `/analytics/export` | Export to CSV/Excel (HMAC required) | Admin, Care Coach |

**Export Response Example:**
```json
{
	"filename": "export_20260408.csv",
	"download_url": "/exports/export_20260408.csv"
}
```

---

## 9. Observability & Audit
| Method | Endpoint | Description | Access |
| :--- | :--- | :--- | :--- |
| `GET` | `/health` | Service health, DB status | Public |
| `GET` | `/internal/metrics` | Prometheus metrics | Admin |
| `GET` | `/audit-logs` | Tamper-evident audit trail | Admin |

---

## 10. Security & Compliance

- **Input Validation**: Strict, with parameterized queries
- **CSRF/XSS**: Protections for token-based APIs
- **HMAC Signing**: Privileged endpoints require HMAC-SHA256 signature
- **Encryption**: AES-256 for sensitive fields, key rotation every 180 days
- **Audit Trails**: All sensitive actions logged with before/after hashes
- **Rate Limiting**: 60 requests/min/user, 10 failed logins/15 min, lockout, CAPTCHA

**HMAC Signing Protocol:**
`hex(HMAC-SHA256(HMAC_SECRET, "{unix_ts}:{METHOD}:{path}"))`
Timestamp tolerance: ±300 seconds

---

## 11. Backup & Restore

- Daily encrypted backups (AES-256), 30-day retention
- Quarterly restore drills
- All backup/restore actions audited

---

## Example Error Response
```json
{
	"error": "forbidden",
	"message": "You do not have access to this resource."
}
```
