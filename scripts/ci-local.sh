#!/bin/bash
# Carnelian OS - Local CI Check
# Run this before pushing to catch issues that would fail in CI.
#
# Usage:
#   ./scripts/ci-local.sh          # Quick checks (fmt, clippy, unit tests, doc-tests)
#   ./scripts/ci-local.sh --full   # Full checks including integration tests (requires Docker)

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

FULL=false
if [[ "$1" == "--full" ]]; then
    FULL=true
fi

pass() { echo -e "${GREEN}✓ $1${NC}"; }
fail() { echo -e "${RED}✗ $1${NC}"; exit 1; }
info() { echo -e "${YELLOW}→ $1${NC}"; }

echo "=== Carnelian Local CI ==="
echo ""

# 1. Formatting
info "Checking formatting..."
if cargo fmt --all -- --check 2>/dev/null; then
    pass "cargo fmt"
else
    fail "cargo fmt -- run 'cargo fmt --all' to fix"
fi

# 2. Clippy
info "Running clippy..."
if SQLX_OFFLINE=true cargo clippy --workspace --all-targets -- -D warnings 2>&1; then
    pass "cargo clippy"
else
    fail "cargo clippy"
fi

# 3. Unit tests
info "Running unit tests..."
if SQLX_OFFLINE=true cargo test --workspace 2>&1; then
    pass "cargo test (unit)"
else
    fail "cargo test (unit)"
fi

# 4. Doc-tests (including --ignored to match CI)
info "Running doc-tests (including ignored)..."
if cargo test -p carnelian-core --doc -- --ignored 2>&1; then
    pass "doc-tests (--ignored)"
else
    fail "doc-tests (--ignored)"
fi

# 5. Integration tests (only with --full, requires Docker)
if $FULL; then
    echo ""
    info "Running integration tests (requires Docker)..."
    if cargo test --workspace -- --ignored 2>&1; then
        pass "integration tests"
    else
        fail "integration tests"
    fi
fi

echo ""
echo -e "${GREEN}=== All checks passed ===${NC}"
if ! $FULL; then
    echo ""
    echo "Tip: Run with --full to also run integration tests (requires Docker)."
fi
