#!/usr/bin/env bash
# =============================================================================
# tests_common.sh — shared helper library
# Source this file at the top of every unit and API test script.
# =============================================================================

BASE_URL="${BASE_URL:-http://localhost:8080}"

# ── Seeded test credentials (from db::seed_initial_data) ────────────────────
ADMIN_USER="admin"
ADMIN_PASS="Admin1234!"
COACH_USER="coach"
COACH_PASS="Coach1234!"
APPROVER_USER="approver"
APPROVER_PASS="Approver1234!"
MEMBER_USER="member"
MEMBER_PASS="Member1234!"
MEMBER_ID="30000000-0000-0000-0000-000000000001"
ORG_UNIT_ID="10000000-0000-0000-0000-000000000001"

# ── Counters ─────────────────────────────────────────────────────────────────
PASS=0
FAIL=0
_FAILED_NAMES=""

# ── Terminal colours ─────────────────────────────────────────────────────────
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

# ── Assertion helpers ────────────────────────────────────────────────────────

pass() {
    echo -e "  ${GREEN}PASS${NC}  $1"
    PASS=$((PASS + 1))
}

fail() {
    echo -e "  ${RED}FAIL${NC}  $1"
    FAIL=$((FAIL + 1))
    _FAILED_NAMES="${_FAILED_NAMES}    ✗ $1\n"
}

# assert_status <expected_code> <actual_code> <label>
assert_status() {
    local expected="$1" actual="$2" label="$3"
    if [ "$actual" = "$expected" ]; then
        pass "$label (HTTP $actual)"
    else
        fail "$label — expected HTTP $expected, got HTTP $actual"
    fi
}

# assert_eq <expected> <actual> <label>
assert_eq() {
    local expected="$1" actual="$2" label="$3"
    if [ "$actual" = "$expected" ]; then
        pass "$label"
    else
        fail "$label — expected='$expected' got='$actual'"
    fi
}

# assert_json_field <json_body> <jq_expr> <expected_value> <label>
assert_json_field() {
    local body="$1" expr="$2" expected="$3" label="$4"
    local actual
    actual=$(printf '%s' "$body" | jq -r "$expr" 2>/dev/null)
    if [ "$actual" = "$expected" ]; then
        pass "$label"
    else
        fail "$label — $expr: expected='$expected' got='$actual'"
    fi
}

# assert_json_present <json_body> <jq_expr> <label>  — value must be non-null/non-empty
assert_json_present() {
    local body="$1" expr="$2" label="$3"
    local actual
    actual=$(printf '%s' "$body" | jq -r "$expr" 2>/dev/null)
    if [ -n "$actual" ] && [ "$actual" != "null" ]; then
        pass "$label"
    else
        fail "$label — '$expr' missing or null"
    fi
}

# assert_json_ge <json_body> <jq_expr> <min_value> <label>  — numeric ≥
assert_json_ge() {
    local body="$1" expr="$2" min="$3" label="$4"
    local actual
    actual=$(printf '%s' "$body" | jq -r "$expr" 2>/dev/null)
    if awk -v a="$actual" -v b="$min" 'BEGIN { exit (a >= b ? 0 : 1) }'; then
        pass "$label ($actual ≥ $min)"
    else
        fail "$label — expected $expr ≥ $min, got '$actual'"
    fi
}

# ── HTTP helpers ─────────────────────────────────────────────────────────────

# GET <path> [token]  → prints response body; sets HTTP_STATUS
http_get() {
    local path="$1" token="${2:-}"
    if [ -n "$token" ]; then
        curl -s -w "\n%{http_code}" \
            -H "Authorization: Bearer $token" \
            "$BASE_URL$path"
    else
        curl -s -w "\n%{http_code}" \
            "$BASE_URL$path"
    fi
}

# POST <path> <json_body> [token]
http_post() {
    local path="$1" body="$2" token="${3:-}"
    if [ -n "$token" ]; then
        curl -s -w "\n%{http_code}" \
            -X POST \
            -H "Content-Type: application/json" \
            -H "Authorization: Bearer $token" \
            -d "$body" \
            "$BASE_URL$path"
    else
        curl -s -w "\n%{http_code}" \
            -X POST \
            -H "Content-Type: application/json" \
            -d "$body" \
            "$BASE_URL$path"
    fi
}

# PUT <path> <json_body> [token]
http_put() {
    local path="$1" body="$2" token="${3:-}"
    curl -s -w "\n%{http_code}" \
        -X PUT \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $token" \
        -d "$body" \
        "$BASE_URL$path"
}

# PATCH <path> <json_body> [token]
http_patch() {
    local path="$1" body="$2" token="${3:-}"
    curl -s -w "\n%{http_code}" \
        -X PATCH \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $token" \
        -d "$body" \
        "$BASE_URL$path"
}

# Split curl output (body + status on last line) into two variables
# Usage: split_response <raw>  → sets RESP_BODY and RESP_STATUS
split_response() {
    local raw="$1"
    RESP_STATUS=$(printf '%s' "$raw" | tail -n1)
    RESP_BODY=$(printf '%s' "$raw" | sed '$d')
}

# ── Auth helper ──────────────────────────────────────────────────────────────

# login <username> <password>  → prints Bearer token (empty on failure)
login() {
    local username="$1" password="$2"
    local raw
    raw=$(http_post "/auth/login" "{\"username\":\"$username\",\"password\":\"$password\"}")
    split_response "$raw"
    printf '%s' "$RESP_BODY" | jq -r '.token // empty' 2>/dev/null
}

# ── Summary ──────────────────────────────────────────────────────────────────

# Print pass/fail tally.  Returns exit code 1 if any tests failed.
summary() {
    local total=$((PASS + FAIL))
    echo ""
    echo "──────────────────────────────────────────────"
    printf "  Total: %d   ${GREEN}Pass: %d${NC}   ${RED}Fail: %d${NC}\n" \
        "$total" "$PASS" "$FAIL"
    if [ "$FAIL" -gt 0 ]; then
        echo -e "${RED}Failed tests:${NC}"
        printf '%b' "$_FAILED_NAMES"
        echo "──────────────────────────────────────────────"
        return 1
    fi
    echo -e "  ${GREEN}All $total tests passed${NC}"
    echo "──────────────────────────────────────────────"
    return 0
}
