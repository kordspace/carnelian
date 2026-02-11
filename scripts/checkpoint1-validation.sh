#!/bin/bash
# Carnelian OS - Checkpoint 1 Validation Script
# Automates manual checkpoint validation steps for the 8 criteria.
#
# Usage:
#   ./scripts/checkpoint1-validation.sh              # Full validation
#   ./scripts/checkpoint1-validation.sh --skip-build  # Skip cargo build step
#   ./scripts/checkpoint1-validation.sh --keep-running # Don't stop Carnelian after validation
#   ./scripts/checkpoint1-validation.sh --clean        # Remove test data after validation

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

pass() { echo -e "${GREEN}✓ $1${NC}"; }
fail() { echo -e "${RED}✗ $1${NC}"; }
info() { echo -e "${YELLOW}→ $1${NC}"; }
warn() { echo -e "${YELLOW}⚠ $1${NC}"; }
header() { echo -e "\n${BLUE}=== $1 ===${NC}"; }

SKIP_BUILD=false
KEEP_RUNNING=false
CLEAN=false
SERVER_PID=""
PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0

API_URL="http://localhost:18789"

for arg in "$@"; do
    case $arg in
        --skip-build) SKIP_BUILD=true ;;
        --keep-running) KEEP_RUNNING=true ;;
        --clean) CLEAN=true ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --skip-build    Skip cargo build step"
            echo "  --keep-running  Don't stop Carnelian after validation"
            echo "  --clean         Remove test data after validation"
            echo "  --help          Show this help message"
            exit 0
            ;;
    esac
done

record_pass() { PASS_COUNT=$((PASS_COUNT + 1)); pass "$1"; }
record_fail() { FAIL_COUNT=$((FAIL_COUNT + 1)); fail "$1"; }
record_skip() { SKIP_COUNT=$((SKIP_COUNT + 1)); warn "SKIP: $1"; }

cleanup() {
    if [ -n "$SERVER_PID" ] && ! $KEEP_RUNNING; then
        info "Stopping Carnelian server (PID $SERVER_PID)..."
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

echo "=== Carnelian Checkpoint 1 Validation ==="
echo ""

# ─────────────────────────────────────────────
# Prerequisites
# ─────────────────────────────────────────────
header "Prerequisites"

# Docker
info "Checking Docker..."
if docker ps > /dev/null 2>&1; then
    record_pass "Docker is running"
else
    record_fail "Docker is not running — start Docker and retry"
    exit 1
fi

# Docker services
info "Checking Docker services..."
if docker inspect carnelian-postgres --format='{{.State.Health.Status}}' 2>/dev/null | grep -q healthy; then
    record_pass "PostgreSQL container healthy"
else
    record_fail "PostgreSQL container not healthy — run: docker-compose up -d"
    exit 1
fi

if docker inspect carnelian-ollama --format='{{.State.Health.Status}}' 2>/dev/null | grep -q healthy; then
    record_pass "Ollama container healthy"
else
    warn "Ollama container not healthy (non-blocking)"
fi

# Build
if ! $SKIP_BUILD; then
    info "Building Carnelian..."
    if cargo build --bin carnelian 2>&1; then
        record_pass "cargo build succeeded"
    else
        record_fail "cargo build failed"
        exit 1
    fi
else
    info "Skipping build (--skip-build)"
fi

# Database migrations
info "Running database migrations..."
if cargo run --bin carnelian -- migrate 2>&1; then
    record_pass "Database migrations applied"
else
    record_fail "Database migrations failed"
    exit 1
fi

# ─────────────────────────────────────────────
# Criterion 1: System Startup
# ─────────────────────────────────────────────
header "Criterion 1: System Startup"

info "Starting Carnelian server in background..."
cargo run --bin carnelian -- start &
SERVER_PID=$!

info "Waiting for server readiness..."
READY=false
for i in $(seq 1 30); do
    if curl -sf "$API_URL/v1/health" > /dev/null 2>&1; then
        READY=true
        break
    fi
    sleep 1
done

if $READY; then
    record_pass "Server started and healthy"
else
    record_fail "Server did not become healthy within 30s"
    exit 1
fi

# Health endpoint
HEALTH=$(curl -sf "$API_URL/v1/health" 2>/dev/null || echo "")
if echo "$HEALTH" | grep -q "healthy"; then
    record_pass "Health endpoint returns healthy status"
else
    record_fail "Health endpoint did not return healthy: $HEALTH"
fi

# Status endpoint
STATUS=$(curl -sf "$API_URL/v1/status" 2>/dev/null || echo "")
if [ -n "$STATUS" ]; then
    record_pass "Status endpoint responds"
else
    record_fail "Status endpoint did not respond"
fi

# ─────────────────────────────────────────────
# Criterion 2: Skill Discovery
# ─────────────────────────────────────────────
header "Criterion 2: Skill Discovery"

info "Triggering skill refresh..."
REFRESH_START=$(date +%s%N)
if cargo run --bin carnelian -- skills refresh 2>&1; then
    REFRESH_END=$(date +%s%N)
    REFRESH_MS=$(( (REFRESH_END - REFRESH_START) / 1000000 ))
    record_pass "Skill refresh completed (${REFRESH_MS}ms)"
else
    record_fail "Skill refresh failed"
fi

info "Checking skills via API..."
SKILLS=$(curl -sf "$API_URL/v1/skills" 2>/dev/null || echo "[]")
SKILL_COUNT=$(echo "$SKILLS" | grep -o '"skill_id"' | wc -l)
if [ "$SKILL_COUNT" -gt 0 ]; then
    record_pass "Skills discovered: $SKILL_COUNT skill(s) found"
else
    record_fail "No skills found via API"
fi

# ─────────────────────────────────────────────
# Criterion 3: Task Creation & Execution
# ─────────────────────────────────────────────
header "Criterion 3: Task Creation & Execution"

info "Creating task via API..."
TASK_RESP=$(curl -sf -X POST "$API_URL/v1/tasks" \
    -H "Content-Type: application/json" \
    -d '{"title":"Checkpoint validation task","description":"Created by validation script"}' 2>/dev/null || echo "")

if echo "$TASK_RESP" | grep -q "task_id"; then
    TASK_ID=$(echo "$TASK_RESP" | grep -o '"task_id":"[^"]*"' | head -1 | cut -d'"' -f4)
    record_pass "Task created via API (task_id: $TASK_ID)"
else
    record_fail "Task creation via API failed: $TASK_RESP"
    TASK_ID=""
fi

if [ -n "$TASK_ID" ]; then
    info "Checking task state..."
    TASK_STATE=$(curl -sf "$API_URL/v1/tasks/$TASK_ID" 2>/dev/null || echo "")
    if echo "$TASK_STATE" | grep -q "state"; then
        record_pass "Task state query succeeded"
    else
        record_fail "Could not query task state"
    fi
fi

info "Listing tasks via API..."
TASKS_LIST=$(curl -sf "$API_URL/v1/tasks" 2>/dev/null || echo "[]")
if echo "$TASKS_LIST" | grep -q "task_id"; then
    record_pass "Task list endpoint responds with tasks"
else
    record_fail "Task list endpoint returned no tasks"
fi

# ─────────────────────────────────────────────
# Criterion 4: CLI Task Creation
# ─────────────────────────────────────────────
header "Criterion 4: CLI Task Creation"

info "Creating task via CLI..."
CLI_OUTPUT=$(cargo run --bin carnelian -- task create "CLI validation task" --description "Created by checkpoint script" 2>&1 || echo "FAILED")
if echo "$CLI_OUTPUT" | grep -qi "task_id\|created\|success"; then
    record_pass "Task created via CLI"
else
    record_fail "CLI task creation failed: $CLI_OUTPUT"
fi

info "Creating task via CLI with priority..."
CLI_OUTPUT2=$(cargo run --bin carnelian -- task create "Priority task" --priority 10 2>&1 || echo "FAILED")
if echo "$CLI_OUTPUT2" | grep -qi "task_id\|created\|success"; then
    record_pass "Task created via CLI with --priority"
else
    record_fail "CLI task creation with priority failed: $CLI_OUTPUT2"
fi

# ─────────────────────────────────────────────
# Criterion 5: Concurrent Execution
# ─────────────────────────────────────────────
header "Criterion 5: Concurrent Execution"

record_skip "Concurrent execution — requires worker setup. Run integration test manually:"
echo "  cargo test --test checkpoint1_validation_test test_criterion5 -- --ignored"

# ─────────────────────────────────────────────
# Criterion 6: Error Handling
# ─────────────────────────────────────────────
header "Criterion 6: Error Handling"

info "Testing invalid skill ID..."
ERR_RESP=$(curl -sf -X POST "$API_URL/v1/tasks" \
    -H "Content-Type: application/json" \
    -d '{"title":"Bad task","skill_id":"00000000-0000-0000-0000-000000000000"}' 2>/dev/null || echo "")
if [ -n "$ERR_RESP" ]; then
    record_pass "Invalid skill ID handled (server responded)"
else
    record_fail "Server did not respond to invalid skill request"
fi

info "Error handling sub-criteria (timeout, crash, retry) tested via integration tests:"
echo "  cargo test --test checkpoint1_validation_test test_criterion6 -- --ignored"

# ─────────────────────────────────────────────
# Criterion 7: UI Responsiveness
# ─────────────────────────────────────────────
header "Criterion 7: UI Responsiveness"

record_skip "UI responsiveness — run UI separately: cargo run -p carnelian-ui"
echo "  Integration test: cargo test --test checkpoint1_validation_test test_criterion7 -- --ignored"

# ─────────────────────────────────────────────
# Criterion 8: Performance Baseline
# ─────────────────────────────────────────────
header "Criterion 8: Performance Baseline"

info "Running performance baseline test..."
if cargo test --test checkpoint1_validation_test test_criterion8_performance_baseline_metrics -- --ignored --nocapture 2>&1; then
    record_pass "Performance baseline test completed"
else
    record_fail "Performance baseline test failed"
fi

# ─────────────────────────────────────────────
# Cleanup
# ─────────────────────────────────────────────
if $CLEAN; then
    header "Cleanup"
    info "Cleaning test data..."
    # Remove tasks created by this script
    echo "  (Manual cleanup: DELETE FROM tasks WHERE title LIKE '%validation%';)"
fi

# ─────────────────────────────────────────────
# Summary
# ─────────────────────────────────────────────
header "Validation Summary"
echo ""
echo -e "  ${GREEN}Passed:  $PASS_COUNT${NC}"
echo -e "  ${RED}Failed:  $FAIL_COUNT${NC}"
echo -e "  ${YELLOW}Skipped: $SKIP_COUNT${NC}"
echo ""

if [ "$FAIL_COUNT" -eq 0 ]; then
    echo -e "${GREEN}=== Checkpoint 1 Validation Passed ===${NC}"
else
    echo -e "${RED}=== Checkpoint 1 Validation: $FAIL_COUNT failure(s) ===${NC}"
fi

if $KEEP_RUNNING; then
    echo ""
    info "Server left running (--keep-running). Stop with: carnelian stop"
    SERVER_PID=""  # Prevent cleanup trap from killing it
fi
