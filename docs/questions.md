# Documentation Checklist

## questions.md (Mandatory)

Document your understanding of business gaps:

---

**Question:** How to handle expired sessions and tokens?

**Hypothesis:** Auto-expire sessions after 30 minutes of inactivity per token.

**Solution:** Implemented session expiry logic in authentication middleware.

---

**Question:** How to prevent duplicate metric entries for a member per day?

**Hypothesis:** Enforce unique constraint on (member_id, metric_type, entry_date).

**Solution:** Database schema and API validation ensure only one entry per metric per day per member.

---

**Question:** How to ensure auditability of all sensitive actions?

**Hypothesis:** Log all authentication, data changes, exports, and configuration edits with actor, timestamp, before/after hashes, and reason codes.

**Solution:** Implemented audit log table and logging hooks in all critical API endpoints.

---

**Question:** How to handle notification delivery retries and frequency caps?

**Hypothesis:** Cap notifications to 3 per template per user per day, retry up to 5 times with exponential backoff.

**Solution:** Notification system enforces caps and retry logic, with delivery logs and read receipts.

---

**Question:** How to support offline-first operation and backup/restore?

**Hypothesis:** All APIs and data persistence must work without external connectivity; backups stored locally with restore drills.

**Solution:** Docker deployment, local PostgreSQL, and backup scripts with 30-day retention and quarterly restore drills.

---

**Question:** How to enforce role-based access and workflow approvals?

**Hypothesis:** Use role-based access control and configurable workflow templates for approvals.

**Solution:** Roles and workflow engine implemented with configurable templates and state transitions.

---

**Question:** How to secure sensitive fields and support key rotation?

**Hypothesis:** Encrypt sensitive fields with AES-256 and rotate keys every 180 days.

**Solution:** Encryption implemented for dietary restrictions and medical notes, with key rotation logic and audit.

---

**Question:** How to meet performance targets under load?

**Hypothesis:** Optimize queries, use proper indexing, and monitor health metrics.

**Solution:** Indexed tables, structured logs, and health metrics APIs for observability.
