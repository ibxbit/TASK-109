#!/usr/bin/env bash
# =============================================================================
# run_tests.sh — VitalPath test runner
# =============================================================================
# Executes all unit tests and API tests against the running Docker stack.
# Can be run repeatedly without manual setup (idempotent).
#
# Usage:
#   ./run_tests.sh               # start stack if needed, run all tests
#   ./run_tests.sh --no-start    # skip docker compose up (stack already running)
#   ./run_tests.sh --teardown    # bring stack down after tests
#
# Exit code: 0 if all tests pass, 1 if any test fails.
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_URL="${BASE_URL:-http://localhost:8080}"
COMPOSE_FILE="$SCRIPT_DIR/docker-compose.yml"

NO_START=false
TEARDOWN=false
for arg in "$@"; do
    case "$arg" in
        --no-start)  NO_START=true  ;;
        --teardown)  TEARDOWN=true  ;;
        --help|-h)
            echo "Usage: $0 [--no-start] [--teardown]"
            echo "  --no-start   Skip 'docker compose up' (assume stack is already running)"
            echo "  --teardown   Run 'docker compose down' after tests complete"
            exit 0
            ;;
    esac
done

# ── Colours ───────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m'

banner() { echo -e "\n${BOLD}═══ $* ═══${NC}"; }
info()   { echo -e "${YELLOW}▸ $*${NC}"; }
ok()     { echo -e "${GREEN}✓ $*${NC}"; }
err()    { echo -e "${RED}✗ $*${NC}"; }

# ── Dependency checks ────────────────────────────────────────────────────────
for cmd in curl jq docker; do
    if ! command -v "$cmd" &>/dev/null; then
        err "Required command not found: $cmd"
        echo "  Install it and re-run this script."
        exit 1
    fi
done

if [ ! -f /.dockerenv ] && [ "$NO_START" = false ]; then
    banner "Delegating to Docker Test Runner"
    # Ensure all services are up
    if ! docker compose -f "$COMPOSE_FILE" up -d --build; then
        err "Docker Compose up failed"
        docker compose -f "$COMPOSE_FILE" logs app
        exit 1
    fi
    if ! docker compose -f "$COMPOSE_FILE" run --rm tester; then
        err "Test execution failed"
        docker compose -f "$COMPOSE_FILE" logs app
        exit 1
    fi
    exit 0
fi

# ── Start stack ───────────────────────────────────────────────────────────────
if [ "$NO_START" = false ]; then
    banner "Starting Docker Compose stack"
    docker compose -f "$COMPOSE_FILE" up -d --build
    ok "Stack started"
fi

# ── Wait for the app to become healthy ───────────────────────────────────────
banner "Waiting for application to be ready"
MAX_WAIT=120
INTERVAL=3
elapsed=0
until curl -sf "$BASE_URL/health" | jq -e '.status == "ok"' > /dev/null 2>&1; do
    if [ "$elapsed" -ge "$MAX_WAIT" ]; then
        err "Application did not become healthy within ${MAX_WAIT}s"
        echo "  Check logs: docker compose logs app"
        exit 1
    fi
    printf "  waiting… (%ds)\r" "$elapsed"
    sleep "$INTERVAL"
    elapsed=$((elapsed + INTERVAL))
done
ok "Application is healthy at $BASE_URL"

# ── Locate test scripts ───────────────────────────────────────────────────────
UNIT_DIR="$SCRIPT_DIR/unit_tests"
API_DIR="$SCRIPT_DIR/API_tests"

# Make all test scripts executable
find "$UNIT_DIR" "$API_DIR" -name "*.sh" -exec chmod +x {} \;

# ── Run tests ─────────────────────────────────────────────────────────────────
SUITE_PASS=0
SUITE_FAIL=0
FAILED_SUITES=""

run_suite() {
    local dir="$1" label="$2"
    local scripts
    mapfile -t scripts < <(find "$dir" -name "test_*.sh" | sort)

    if [ "${#scripts[@]}" -eq 0 ]; then
        info "No test scripts found in $dir"
        return
    fi

    banner "$label (${#scripts[@]} suites)"

    for script in "${scripts[@]}"; do
        local name
        name=$(basename "$script")
        echo ""
        info "Running $name"
        if BASE_URL="$BASE_URL" bash "$script"; then
            SUITE_PASS=$((SUITE_PASS + 1))
        else
            SUITE_FAIL=$((SUITE_FAIL + 1))
            FAILED_SUITES="$FAILED_SUITES  ✗ $label/$name\n"
        fi
    done
}

run_suite "$UNIT_DIR" "Unit Tests"
run_suite "$API_DIR"  "API Tests"

# ── Final report ──────────────────────────────────────────────────────────────
TOTAL=$((SUITE_PASS + SUITE_FAIL))
echo ""
banner "Test Results"
printf "  Total suites: %d   ${GREEN}Pass: %d${NC}   ${RED}Fail: %d${NC}\n" \
    "$TOTAL" "$SUITE_PASS" "$SUITE_FAIL"

if [ "$SUITE_FAIL" -gt 0 ]; then
    echo -e "${RED}Failed suites:${NC}"
    printf '%b' "$FAILED_SUITES"
fi

# ── Teardown ─────────────────────────────────────────────────────────────────
if [ "$TEARDOWN" = true ]; then
    banner "Tearing down stack"
    docker compose -f "$COMPOSE_FILE" down
    ok "Stack stopped"
fi

echo ""
if [ "$SUITE_FAIL" -eq 0 ]; then
    echo -e "${GREEN}${BOLD}All $TOTAL test suites passed.${NC}"
    exit 0
else
    echo -e "${RED}${BOLD}$SUITE_FAIL of $TOTAL suites FAILED.${NC}"
    exit 1
fi
