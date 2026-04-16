# Test Coverage Audit

## Backend Endpoint Inventory

Source of truth: `repo/src/main.rs:140`-`repo/src/main.rs:150` + `repo/src/api/*.rs` route attributes/scopes.

1. `GET /healthz`
2. `GET /health`
3. `GET /internal/metrics`
4. `POST /auth/login`
5. `POST /auth/logout`
6. `GET /auth/me`
7. `POST /profile`
8. `GET /profile/{member_id}`
9. `PUT /profile/{member_id}`
10. `POST /metrics`
11. `GET /metrics`
12. `GET /metrics/summary`
13. `POST /goals`
14. `GET /goals`
15. `PUT /goals/{id}`
16. `POST /notifications`
17. `GET /notifications`
18. `POST /notifications/{id}/read`
19. `POST /notifications/read-all`
20. `GET /notifications/subscriptions`
21. `PATCH /notifications/subscriptions/{event_type}`
22. `POST /notifications/schedules`
23. `GET /notifications/schedules`
24. `DELETE /notifications/schedules/{id}`
25. `POST /work-orders`
26. `PATCH /work-orders/{id}/transition`
27. `GET /work-orders`
28. `POST /workflows/templates`
29. `POST /workflows/templates/{id}/nodes`
30. `POST /workflows/instances`
31. `POST /workflows/instances/{id}/actions`
32. `GET /workflows/instances/{id}`
33. `GET /analytics`
34. `POST /analytics/export`
35. `GET /analytics/export/{filename}`
36. `GET /audit-logs`

## API Test Mapping Table

| Endpoint | Covered | Test type | Test files | Evidence |
|---|---|---|---|---|
| `GET /healthz` | yes | true no-mock HTTP | `repo/tests/health_endpoints.rs`, `repo/tests/security_headers.rs` | `liveness_returns_ok_with_status_field` |
| `GET /health` | yes | true no-mock HTTP | `repo/unit_tests/test_01_health.sh`, `repo/API_tests/test_06_persistence.sh` | `http_get "/health"` |
| `GET /internal/metrics` | yes | true no-mock HTTP | `repo/unit_tests/test_04_rbac.sh`, `repo/API_tests/test_06_persistence.sh` | `http_get "/internal/metrics"` |
| `POST /auth/login` | yes | true no-mock HTTP | `repo/unit_tests/test_02_auth_success.sh`, `repo/API_tests/test_01_auth_lifecycle.sh` | `http_post "/auth/login"` |
| `POST /auth/logout` | yes | true no-mock HTTP | `repo/unit_tests/test_02_auth_success.sh`, `repo/API_tests/test_01_auth_lifecycle.sh` | `http_post "/auth/logout"` |
| `GET /auth/me` | yes | true no-mock HTTP | `repo/unit_tests/test_02_auth_success.sh`, `repo/API_tests/test_01_auth_lifecycle.sh` | `http_get "/auth/me"` |
| `POST /profile` | yes | true no-mock HTTP | `repo/API_tests/test_02_health_profile.sh`, `repo/unit_tests/test_05_validation.sh` | `http_post "/profile"` |
| `GET /profile/{member_id}` | yes | true no-mock HTTP | `repo/API_tests/test_02_health_profile.sh`, `repo/unit_tests/test_04_rbac.sh` | `http_get "/profile/$MEMBER_ID"` |
| `PUT /profile/{member_id}` | yes | true no-mock HTTP | `repo/API_tests/test_02_health_profile.sh`, `repo/API_tests/test_09_key_rotation.sh` | `http_put "/profile/$MEMBER_ID"` |
| `POST /metrics` | yes | true no-mock HTTP | `repo/API_tests/test_03_metrics.sh`, `repo/unit_tests/test_05_validation.sh` | `http_post "/metrics"` |
| `GET /metrics` | yes | true no-mock HTTP | `repo/API_tests/test_03_metrics.sh` | `http_get "/metrics?..."` |
| `GET /metrics/summary` | yes | true no-mock HTTP | `repo/API_tests/test_03_metrics.sh` | `http_get "/metrics/summary?..."` |
| `POST /goals` | yes | true no-mock HTTP | `repo/API_tests/test_04_goals.sh`, `repo/unit_tests/test_05_validation.sh` | `http_post "/goals"` |
| `GET /goals` | yes | true no-mock HTTP | `repo/API_tests/test_04_goals.sh` | `http_get "/goals?..."` |
| `PUT /goals/{id}` | yes | true no-mock HTTP | `repo/API_tests/test_04_goals.sh` | `http_put "/goals/$GOAL_ID"` |
| `POST /notifications` | yes | true no-mock HTTP | `repo/API_tests/test_12_notifications.sh` | `http_post "/notifications"` |
| `GET /notifications` | yes | true no-mock HTTP | `repo/API_tests/test_12_notifications.sh` | `http_get "/notifications"` |
| `POST /notifications/{id}/read` | yes | true no-mock HTTP | `repo/API_tests/test_12_notifications.sh` | `http_post "/notifications/$NOTIF_ID/read"` |
| `POST /notifications/read-all` | yes | true no-mock HTTP | `repo/API_tests/test_12_notifications.sh` | `http_post "/notifications/read-all"` |
| `GET /notifications/subscriptions` | yes | true no-mock HTTP | `repo/API_tests/test_12_notifications.sh` | `http_get "/notifications/subscriptions"` |
| `PATCH /notifications/subscriptions/{event_type}` | yes | true no-mock HTTP | `repo/API_tests/test_12_notifications.sh` | `curl -X PATCH ... /notifications/subscriptions/sla_breach` |
| `POST /notifications/schedules` | yes | true no-mock HTTP | `repo/API_tests/test_12_notifications.sh` | `http_post "/notifications/schedules"` |
| `GET /notifications/schedules` | yes | true no-mock HTTP | `repo/API_tests/test_12_notifications.sh` | `http_get "/notifications/schedules"` |
| `DELETE /notifications/schedules/{id}` | yes | true no-mock HTTP | `repo/API_tests/test_12_notifications.sh` | `curl -X DELETE ... /notifications/schedules/$SCHED_ID` |
| `POST /work-orders` | yes | true no-mock HTTP | `repo/API_tests/test_06_persistence.sh`, `repo/API_tests/test_13_security_matrix.sh` | `http_post "/work-orders"` |
| `PATCH /work-orders/{id}/transition` | yes | true no-mock HTTP | `repo/API_tests/test_06_persistence.sh`, `repo/API_tests/test_13_security_matrix.sh` | `http_patch "/work-orders/$WO_ID/transition"` |
| `GET /work-orders` | yes | true no-mock HTTP | `repo/API_tests/test_06_persistence.sh` | `http_get "/work-orders"` |
| `POST /workflows/templates` | yes | true no-mock HTTP | `repo/API_tests/test_11_workflows.sh`, `repo/API_tests/test_13_security_matrix.sh` | `http_post "/workflows/templates"` |
| `POST /workflows/templates/{id}/nodes` | yes | true no-mock HTTP | `repo/API_tests/test_11_workflows.sh`, `repo/API_tests/test_13_security_matrix.sh` | `http_post "/workflows/templates/$TEMPLATE_ID/nodes"` |
| `POST /workflows/instances` | yes | true no-mock HTTP | `repo/API_tests/test_11_workflows.sh`, `repo/API_tests/test_13_security_matrix.sh` | `http_post "/workflows/instances"` |
| `POST /workflows/instances/{id}/actions` | yes | true no-mock HTTP | `repo/API_tests/test_11_workflows.sh`, `repo/API_tests/test_13_security_matrix.sh` | `http_post "/workflows/instances/$INSTANCE_ID/actions"` |
| `GET /workflows/instances/{id}` | yes | true no-mock HTTP | `repo/API_tests/test_11_workflows.sh` | `http_get "/workflows/instances/$INSTANCE_ID"` |
| `GET /analytics` | yes | true no-mock HTTP | `repo/API_tests/test_13_security_matrix.sh` | `http_get "/analytics"` |
| `POST /analytics/export` | yes | true no-mock HTTP | `repo/API_tests/test_07_hmac_signing.sh`, `repo/API_tests/test_14_export_download.sh` | `curl -X POST "$BASE_URL/analytics/export"` |
| `GET /analytics/export/{filename}` | yes | true no-mock HTTP | `repo/API_tests/test_14_export_download.sh` | Step 2 download with auth + status/content checks |
| `GET /audit-logs` | yes | true no-mock HTTP | `repo/API_tests/test_05_audit_logs.sh`, `repo/unit_tests/test_04_rbac.sh` | `http_get "/audit-logs"` |

## Coverage Summary
- Total endpoints: **36**
- Endpoints with HTTP tests: **36**
- Endpoints with TRUE no-mock tests: **36**
- HTTP coverage: **100.00%**
- True API coverage: **100.00%**

## Unit Test Summary
- Test files: `repo/tests/*.rs`, `repo/unit_tests/*.sh`, `repo/API_tests/*.sh`, plus inline `#[cfg(test)]` in `repo/src/**`.
- Modules covered:
  - controllers: all route modules in `repo/src/api/*.rs` via HTTP scripts.
  - services/security: auth, crypto, hmac, rate-limit, masking (unit + integration evidence).
  - auth/guards/middleware: `repo/tests/middleware_unauthenticated.rs`, `repo/tests/permission_boundaries.rs`, `repo/tests/rate_limit_middleware.rs`.
  - repositories/data: mostly via DB-backed integration/API tests.
- Important modules not strongly isolated by unit tests: `repo/src/db.rs`, `repo/src/notifications.rs` worker internals, full app assembly in `repo/src/main.rs`.

## Tests Check
- API test classification:
  - True No-Mock HTTP: all bash suites + Actix HTTP integration tests.
  - HTTP with Mocking: none found.
  - Non-HTTP: `repo/tests/error_responses.rs`, `repo/tests/permission_boundaries.rs`, `repo/tests/crypto_roundtrip.rs`, plus inline unit tests.
- Mock detection: no `jest.mock`, `vi.mock`, `sinon.stub` patterns found.
- API observability: generally strong; some tests still allow permissive compatibility statuses in specific paths (e.g., path traversal/nonexistent export in `repo/API_tests/test_14_export_download.sh`).
- `run_tests.sh`: Docker-based execution (`repo/run_tests.sh:63`, `repo/run_tests.sh:68`) with required host Docker dependency (`repo/run_tests.sh:54`).

## Test Coverage Score (0–100)
**96/100**

## Score Rationale
- Full endpoint coverage and strong real HTTP-path testing depth.
- No over-mocking evidence.
- Remaining deductions are for assertion strictness in a few compatibility branches and limited isolated unit coverage for DB/worker internals.

## Key Gaps
1. Tighten permissive status branches where deterministic setup is possible.
2. Add isolated unit tests for `repo/src/db.rs` and notification worker internals.

## Confidence & Assumptions
- Confidence: high for static mapping; medium for runtime behavior (execution intentionally skipped).
- Assumption: all active endpoints are declared through inspected route configuration.

---

# README Audit

## High Priority Issues
- None that violate hard gates under current strict checks.

## Medium Priority Issues
- None. All `docker compose` command styles are now used consistently in README.

## Low Priority Issues
- README remains long; usability can improve with modularization.

## Hard Gate Failures
- **None found by static inspection.**

Gate evidence:
- Project type declared at top: `repo/README.md:3`.
- Required startup string present: `docker compose up` at `repo/README.md:22`.
- Access method includes URL + port: `repo/README.md:39`.
- Verification method includes curl-based API confirmation: `repo/README.md:123` onward.
- Environment rules: no `npm install`, `pip install`, `apt-get`, or manual DB setup instructions in README.
- Demo credentials include all roles (admin, coach, approver, member): `repo/README.md:51`-`repo/README.md:54`.

## README Verdict
**PASS**
