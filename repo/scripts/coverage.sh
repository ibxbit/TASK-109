#!/usr/bin/env bash
# Generate a coverage report for the Rust-native test suite.
#
# Usage:
#   ./scripts/coverage.sh           # Run locally (requires cargo-tarpaulin on Linux).
#   ./scripts/coverage.sh --docker  # Run in the official tarpaulin Docker image.
#
# Outputs:
#   target/tarpaulin/tarpaulin-report.html   — browseable HTML report.
#   target/tarpaulin/lcov.info               — for Codecov / Coveralls upload.
#   target/tarpaulin/tarpaulin-report.json   — machine-readable summary.
#
# The build fails (non-zero exit) when total line coverage falls below
# the `fail-under` threshold defined in tarpaulin.toml (currently 90 %).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

if [[ "${1:-}" == "--docker" ]]; then
    echo "[coverage] running inside xd009642/tarpaulin"
    exec docker run --rm \
        --security-opt seccomp=unconfined \
        -v "$REPO_ROOT:/volume" \
        xd009642/tarpaulin \
        cargo tarpaulin --config tarpaulin.toml
fi

if ! command -v cargo-tarpaulin >/dev/null 2>&1; then
    echo "cargo-tarpaulin not installed."
    echo "Install it with:"
    echo "    cargo install cargo-tarpaulin --locked"
    echo "or rerun this script with --docker."
    exit 1
fi

cargo tarpaulin --config tarpaulin.toml

echo
echo "[coverage] HTML report: target/tarpaulin/tarpaulin-report.html"
echo "[coverage] LCOV file:   target/tarpaulin/lcov.info"
