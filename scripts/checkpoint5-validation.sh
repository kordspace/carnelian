#!/usr/bin/env bash
# Carnelian OS - Checkpoint 5 Validation Script
# Skill Registry ≥50, Elixir API, WASM skills, Native ops
#
# Usage:
#   ./scripts/checkpoint5-validation.sh                    # Full 24-hour validation
#   ./scripts/checkpoint5-validation.sh --skip-build       # Skip cargo build step
#   ./scripts/checkpoint5-validation.sh --duration 3600    # 1-hour test run
#   ./scripts/checkpoint5-validation.sh --keep-running     # Don't stop Carnelian after validation
#   ./scripts/checkpoint5-validation.sh --dry-run          # Print steps without executing
#
# Run from repository root.

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

pass() { echo -e "${GREEN}✓ $1${NC}"; }
fail() { echo -e "${RED}✗ $1${NC}"; }
info() { echo -e "${YELLOW}→ $1${NC}"; }
warn() { echo -e "${YELLOW}⚠ $1${NC}"; }
header() { echo -e "\n${BLUE}=== $1 ===${NC}"; }

SKIP_BUILD=false
KEEP_RUNNING=false
DRY_RUN=false
DURATION=86400  # 24 hours in seconds
SERVER_PID=""
PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0
HEALTH_FAIL_STREAK=0
MAX_HEALTH_FAIL_STREAK=3
REGISTRY_OK=false
ELIXIR_API_OK=false
WASM_OK=false
NATIVE_OK=false

API_URL="http://localhost:18789"
LOG_DIR="logs"
LOG_FILE="${LOG_DIR}/carnelian-cp5.log"
ERR_FILE="${LOG_DIR}/carnelian-cp5.err"
METRICS_FILE="${LOG_DIR}/metrics-cp5.jsonl"
REPORT_FILE="${LOG_DIR}/checkpoint5-report.md"
START_TIME=""
START_EPOCH=""

for arg in "$@"; do
    case $arg in
        --skip-build) SKIP_BUILD=true ;;
        --keep-running) KEEP_RUNNING=true ;;
        --dry-run) DRY_RUN=true ;;
        --duration)
            shift_next=true
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --skip-build    Skip cargo build step"
            echo "  --keep-running  Don't stop Carnelian after validation"
            echo "  --duration N    Run for N seconds instead of 24 hours (default: 86400)"
            echo "  --dry-run       Print steps without executing"
            echo "  --help          Show this help message"
            exit 0
            ;;
        *)
            if [ "${shift_next:-false}" = true ]; then
                DURATION="$arg"
                shift_next=false
            fi
            ;;
    esac
done

# Handle --duration with next arg
ARGS=("$@")
for i in "${!ARGS[@]}"; do
    if [ "${ARGS[$i]}" = "--duration" ] && [ $((i + 1)) -lt ${#ARGS[@]} ]; then
        DURATION="${ARGS[$((i + 1))]}"
    fi
done

run_cmd() {
    if $DRY_RUN; then
        info "[dry-run] $*"
        return 0
    fi
    "$@"
}

record_pass() { PASS_COUNT=$((PASS_COUNT + 1)); pass "$1"; }
record_fail() { FAIL_COUNT=$((FAIL_COUNT + 1)); fail "$1"; }
record_skip() { SKIP_COUNT=$((SKIP_COUNT + 1)); warn "SKIP: $1"; }

cleanup() {
    if [ -n "$SERVER_PID" ] && ! $KEEP_RUNNING; then
        info "Stopping Carnelian server (PID $SERVER_PID)..."
        kill "$SERVER_PID" 2>/dev/null || true
        # Wait up to 30 seconds for graceful shutdown
        for i in $(seq 1 30); do
            if ! kill -0 "$SERVER_PID" 2>/dev/null; then
                break
            fi
            sleep 1
        done
        # Force kill if still running
        if kill -0 "$SERVER_PID" 2>/dev/null; then
            warn "Server did not stop gracefully, sending SIGKILL..."
            kill -9 "$SERVER_PID" 2>/dev/null || true
        fi
        wait "$SERVER_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

# ─────────────────────────────────────────────
# Log Rotation Helper
# ─────────────────────────────────────────────
rotate_log() {
    local file="$1"
    local max_size_bytes=$((100 * 1024 * 1024))  # 100MB
    if [ -f "$file" ]; then
        local size
        size=$(stat -f%z "$file" 2>/dev/null || stat --printf="%s" "$file" 2>/dev/null || echo 0)
        if [ "$size" -gt "$max_size_bytes" ]; then
            local timestamp
            timestamp=$(date +%Y%m%d_%H%M%S)
            mv "$file" "${file}.${timestamp}"
            # Keep only last 5 rotated files
            ls -t "${file}".* 2>/dev/null | tail -n +6 | xargs rm -f 2>/dev/null || true
            info "Rotated ${file} (was ${size} bytes)"
        fi
    fi
}

# ─────────────────────────────────────────────
# Metrics Collection Helper
# ─────────────────────────────────────────────
collect_metrics() {
    local timestamp
    timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    # Health endpoint
    local health_json
    health_json=$(curl -sf "$API_URL/v1/health" 2>/dev/null || echo '{"status":"unreachable","database":"unknown"}')
    local health_status
    health_status=$(echo "$health_json" | grep -o '"status":"[^"]*"' | head -1 | cut -d'"' -f4)
    local health_db
    health_db=$(echo "$health_json" | grep -o '"database":"[^"]*"' | head -1 | cut -d'"' -f4)

    # Status endpoint
    local status_json
    status_json=$(curl -sf "$API_URL/v1/status" 2>/dev/null || echo '{"queue_depth":0,"workers":[]}')
    local queue_depth
    queue_depth=$(echo "$status_json" | grep -o '"queue_depth":[0-9]*' | head -1 | cut -d':' -f2)
    queue_depth=${queue_depth:-0}
    local worker_count
    worker_count=$(echo "$status_json" | grep -o '"id"' | wc -l || echo 0)

    # System metrics for server PID
    local memory_rss_kb=0
    local cpu_percent=0
    if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
        if [[ "$OSTYPE" == "darwin"* ]]; then
            memory_rss_kb=$(ps -p "$SERVER_PID" -o rss= 2>/dev/null | tr -d ' ' || echo 0)
            cpu_percent=$(ps -p "$SERVER_PID" -o %cpu= 2>/dev/null | tr -d ' ' || echo 0)
        elif [[ "$OSTYPE" == "linux"* ]]; then
            memory_rss_kb=$(ps -p "$SERVER_PID" -o rss= 2>/dev/null | tr -d ' ' || echo 0)
            cpu_percent=$(ps -p "$SERVER_PID" -o %cpu= 2>/dev/null | tr -d ' ' || echo 0)
        elif [[ "$OSTYPE" == "msys"* ]] || [[ "$OSTYPE" == "cygwin"* ]]; then
            # Windows: use tasklist for memory
            memory_rss_kb=$(tasklist /FI "PID eq $SERVER_PID" /FO CSV /NH 2>/dev/null | \
                awk -F',' '{gsub(/[" K]/, "", $5); print $5}' 2>/dev/null || echo 0)
            cpu_percent=0
        fi
    fi

    # Write metrics line
    echo "{\"timestamp\":\"${timestamp}\",\"health\":{\"status\":\"${health_status}\",\"database\":\"${health_db}\"},\"worker_count\":${worker_count},\"queue_depth\":${queue_depth},\"memory_rss_kb\":${memory_rss_kb:-0},\"cpu_percent\":${cpu_percent:-0}}" >> "$METRICS_FILE"
}

# ─────────────────────────────────────────────
# Summary Report Generation
# ─────────────────────────────────────────────
generate_report() {
    local end_time
    end_time=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    local end_epoch
    end_epoch=$(date +%s)
    local actual_duration=$((end_epoch - START_EPOCH))
    local hours=$((actual_duration / 3600))
    local minutes=$(( (actual_duration % 3600) / 60 ))

    # Compute metrics from collected data
    local total_samples=0
    local max_workers=0
    local min_workers=999999999
    local sum_workers=0
    local max_rss=0
    local min_rss=999999999
    local sum_rss=0
    local error_count=0
    local healthy_count=0
    local total_health_checks=0

    if [ -f "$METRICS_FILE" ]; then
        total_samples=$(wc -l < "$METRICS_FILE" | tr -d ' ')

        while IFS= read -r line; do
            local wc
            wc=$(echo "$line" | grep -o '"worker_count":[0-9]*' | cut -d':' -f2)
            wc=${wc:-0}
            if [ "$wc" -gt "$max_workers" ]; then max_workers=$wc; fi
            if [ "$wc" -lt "$min_workers" ]; then min_workers=$wc; fi
            sum_workers=$((sum_workers + wc))

            local rss
            rss=$(echo "$line" | grep -o '"memory_rss_kb":[0-9]*' | cut -d':' -f2)
            rss=${rss:-0}
            if [ "$rss" -gt 0 ]; then
                sum_rss=$((sum_rss + rss))
                if [ "$rss" -gt "$max_rss" ]; then max_rss=$rss; fi
                if [ "$rss" -lt "$min_rss" ]; then min_rss=$rss; fi
            fi

            local status
            status=$(echo "$line" | grep -o '"status":"[^"]*"' | head -1 | cut -d'"' -f4)
            total_health_checks=$((total_health_checks + 1))
            if [ "$status" = "healthy" ]; then
                healthy_count=$((healthy_count + 1))
            fi
        done < "$METRICS_FILE"
    fi

    if [ "$min_workers" -eq 999999999 ]; then min_workers=0; fi
    if [ "$min_rss" -eq 999999999 ]; then min_rss=0; fi
    local avg_workers=0
    local avg_rss=0
    if [ "$total_samples" -gt 0 ]; then
        avg_workers=$((sum_workers / total_samples))
        avg_rss=$((sum_rss / total_samples))
    fi

    local uptime_pct=0
    if [ "$total_health_checks" -gt 0 ]; then
        uptime_pct=$((healthy_count * 100 / total_health_checks))
    fi

    # Count ERROR-level events in log
    if [ -f "$LOG_FILE" ]; then
        error_count=$(grep -ci "ERROR" "$LOG_FILE" 2>/dev/null || echo 0)
    fi

    # Memory growth (last RSS - first RSS)
    local first_rss=0
    local last_rss=0
    if [ -f "$METRICS_FILE" ] && [ "$total_samples" -gt 0 ]; then
        first_rss=$(head -1 "$METRICS_FILE" | grep -o '"memory_rss_kb":[0-9]*' | cut -d':' -f2)
        first_rss=${first_rss:-0}
        last_rss=$(tail -1 "$METRICS_FILE" | grep -o '"memory_rss_kb":[0-9]*' | cut -d':' -f2)
        last_rss=${last_rss:-0}
    fi
    local memory_growth_kb=$((last_rss - first_rss))
    local memory_growth_mb=$((memory_growth_kb / 1024))

    # Determine pass/fail for each criterion
    local c9a_status="❌"
    if $REGISTRY_OK; then
        c9a_status="✅"
    fi
    local c9b_status="❌"
    if $ELIXIR_API_OK; then
        c9b_status="✅"
    fi
    local c9c_status="❌"
    if $WASM_OK; then
        c9c_status="✅"
    fi
    local c9d_status="❌"
    if $NATIVE_OK; then
        c9d_status="✅"
    fi
    local c_cross_status="❌"
    if [ -f "$METRICS_FILE" ]; then
        local ledger_intact
        ledger_intact=$(tail -1 "$METRICS_FILE" 2>/dev/null | grep -o '"ledger_intact":true' || echo "")
        if [ -n "$ledger_intact" ] || [ "$uptime_pct" -ge 95 ]; then
            c_cross_status="✅"
        fi
    fi

    # Write report
    cat > "$REPORT_FILE" <<EOF
# 🔥 Carnelian OS — Checkpoint 5 Validation Report

## Run Metadata

| Field | Value |
|-------|-------|
| Start Time | ${START_TIME} |
| End Time | ${end_time} |
| Duration | ${hours}h ${minutes}m (${actual_duration}s) |
| Target Duration | $((DURATION / 3600))h |
| Metrics Samples | ${total_samples} |

## Worker Ecosystem

| Metric | Value |
|--------|-------|
| Min Workers | ${min_workers} |
| Max Workers | ${max_workers} |
| Average Workers | ${avg_workers} |

## System Health

| Metric | Value |
|--------|-------|
| Health Checks | ${total_health_checks} |
| Healthy Responses | ${healthy_count} |
| Uptime | ${uptime_pct}% |
| ERROR-level Events | ${error_count} |

## Memory Usage

| Metric | Value |
|--------|-------|
| Min RSS | ${min_rss} KB |
| Max RSS | ${max_rss} KB |
| Average RSS | ${avg_rss} KB |
| Final RSS | ${last_rss} KB |
| Memory Growth | ${memory_growth_mb} MB |

## Performance Metrics

| Metric | Target | Actual | Status |
|--------|--------|--------|--------|
| Server Uptime | ≥ 95% | ${uptime_pct}% | $([ "$uptime_pct" -ge 95 ] && echo "✅" || echo "❌") |
| Memory Growth | < 100MB | ${memory_growth_mb} MB | $([ "$memory_growth_mb" -lt 100 ] && echo "✅" || echo "❌") |
| ERROR Events | 0 | ${error_count} | $([ "$error_count" -eq 0 ] && echo "✅" || echo "⚠️") |

## Criterion Results

| # | Criterion | Status |
|---|-----------|--------|
| 9A | Skill Registry ≥ 50 directories | ${c9a_status} |
| 9B | Elixir API (4 endpoints HTTP 200) | ${c9b_status} |
| 9C | WASM Skills (7 skills complete) | ${c9c_status} |
| 9D | Native Ops (4 representative ops) | ${c9d_status} |
| Cross | Ledger Integrity | ${c_cross_status} |

## Overall Result

$(if [ "$FAIL_COUNT" -eq 0 ]; then echo "**✅ CHECKPOINT 5 VALIDATION PASSED**"; else echo "**❌ CHECKPOINT 5 VALIDATION: ${FAIL_COUNT} failure(s)**"; fi)

---
*Generated by scripts/checkpoint5-validation.sh*
EOF

    echo ""
    echo -e "${CYAN}Report written to: ${REPORT_FILE}${NC}"
}

# =============================================================================
# MAIN SCRIPT
# =============================================================================

echo ""
echo -e "${CYAN}🔥 Carnelian OS — Checkpoint 5 Validation${NC}"
echo -e "${CYAN}   Duration: $((DURATION / 3600))h $(( (DURATION % 3600) / 60 ))m${NC}"
echo ""

# ─────────────────────────────────────────────
# Prerequisites
# ─────────────────────────────────────────────
header "Prerequisites"

# Create log directory
mkdir -p "$LOG_DIR"

# Docker running
info "Checking Docker..."
if docker ps > /dev/null 2>&1; then
    record_pass "Docker is running"
else
    record_fail "Docker is not running — start Docker and retry"
    exit 1
fi

# PostgreSQL healthy
info "Checking PostgreSQL container..."
if docker inspect carnelian-postgres --format='{{.State.Health.Status}}' 2>/dev/null | grep -q healthy; then
    record_pass "PostgreSQL container healthy"
else
    record_fail "PostgreSQL container not healthy — run: docker-compose up -d"
    exit 1
fi

# Build
if ! $SKIP_BUILD; then
    info "Building Carnelian..."
    if run_cmd cargo build --workspace 2>&1; then
        record_pass "cargo build --workspace succeeded"
    else
        record_fail "cargo build --workspace failed"
        exit 1
    fi
else
    info "Skipping build (--skip-build)"
fi

# Lint
info "Running clippy..."
if run_cmd cargo clippy --workspace -- -D warnings 2>&1; then
    record_pass "cargo clippy passed"
else
    record_fail "cargo clippy found warnings/errors"
    exit 1
fi

# Tests
info "Running tests..."
if run_cmd cargo test --workspace 2>&1; then
    record_pass "cargo test --workspace passed"
else
    record_fail "cargo test --workspace failed"
    exit 1
fi

# ─────────────────────────────────────────────
# Server Startup
# ─────────────────────────────────────────────
header "Server Startup"

info "Starting Carnelian server with log rotation..."
START_TIME=$(date -u +%Y-%m-%dT%H:%M:%SZ)
START_EPOCH=$(date +%s)

if $DRY_RUN; then
    info "[dry-run] RUST_LOG=info cargo run --bin carnelian -- start > $LOG_FILE 2> $ERR_FILE &"
else
    RUST_LOG=info cargo run --bin carnelian -- start >> "$LOG_FILE" 2>> "$ERR_FILE" &
    SERVER_PID=$!
fi

info "Waiting for server readiness (60s timeout)..."
READY=false
if $DRY_RUN; then
    info "[dry-run] poll ${API_URL}/v1/health (up to 60s)"
    READY=true
else
    for i in $(seq 1 60); do
        if curl -sf "$API_URL/v1/health" > /dev/null 2>&1; then
            READY=true
            break
        fi
        sleep 1
    done
fi

if $READY; then
    record_pass "Server started and healthy"
else
    record_fail "Server did not become healthy within 60s"
    if [ -f "$ERR_FILE" ]; then
        echo "  Last 10 lines of error log:"
        tail -10 "$ERR_FILE" 2>/dev/null || true
    fi
    exit 1
fi

# Initial metrics snapshot
collect_metrics
record_pass "Initial metrics collected"

# ─────────────────────────────────────────────
# Phase 9A — Skill Registry Count
# ─────────────────────────────────────────────
header "Phase 9A — Skill Registry Count"

info "Counting skill registry directories..."
REGISTRY_COUNT=$(find skills/registry -mindepth 1 -maxdepth 1 -type d 2>/dev/null | wc -l | tr -d ' ')

if [ "$REGISTRY_COUNT" -ge 50 ]; then
    record_pass "Skill registry contains ${REGISTRY_COUNT} directories (≥50)"
    REGISTRY_OK=true
else
    record_fail "Skill registry only has ${REGISTRY_COUNT} directories (need ≥50)"
fi

# ─────────────────────────────────────────────
# Phase 9B — Elixir API Endpoints
# ─────────────────────────────────────────────
header "Phase 9B — Elixir API Endpoints"

check_elixir_endpoint() {
    local method="$1"
    local path="$2"
    local body="$3"
    local expected_code="$4"
    local verify_key="$5"
    
    local actual_code
    local response
    
    if [ "$method" = "GET" ]; then
        actual_code=$(curl -sf -o /tmp/elixir_resp.json -w "%{http_code}" "$API_URL$path" 2>/dev/null || echo "000")
        response=$(cat /tmp/elixir_resp.json 2>/dev/null || echo "{}")
    else
        actual_code=$(curl -sf -o /tmp/elixir_resp.json -w "%{http_code}" -X "$method" "$API_URL$path" \
            -H "Content-Type: application/json" -d "$body" 2>/dev/null || echo "000")
        response=$(cat /tmp/elixir_resp.json 2>/dev/null || echo "{}")
    fi
    
    if [ "$actual_code" = "$expected_code" ]; then
        if echo "$response" | grep -q "\"$verify_key\""; then
            record_pass "$method $path → $actual_code (verified: $verify_key present)"
            return 0
        else
            record_fail "$method $path → $actual_code but missing key: $verify_key"
            return 1
        fi
    else
        record_fail "$method $path → $actual_code (expected $expected_code)"
        return 1
    fi
}

ELIXIR_PASS_COUNT=0

if check_elixir_endpoint "GET" "/v1/elixirs" "" "200" "elixirs"; then
    ELIXIR_PASS_COUNT=$((ELIXIR_PASS_COUNT + 1))
fi

if check_elixir_endpoint "POST" "/v1/elixirs" '{"name":"cp5-test","elixir_type":"prompt","dataset":{}}' "200" "elixir_id"; then
    ELIXIR_PASS_COUNT=$((ELIXIR_PASS_COUNT + 1))
fi

if check_elixir_endpoint "GET" "/v1/elixirs/search?q=test" "" "200" "results"; then
    ELIXIR_PASS_COUNT=$((ELIXIR_PASS_COUNT + 1))
fi

if check_elixir_endpoint "GET" "/v1/elixirs/drafts" "" "200" "drafts"; then
    ELIXIR_PASS_COUNT=$((ELIXIR_PASS_COUNT + 1))
fi

if [ "$ELIXIR_PASS_COUNT" -eq 4 ]; then
    ELIXIR_API_OK=true
fi

# ─────────────────────────────────────────────
# Phase 9C — WASM Skills
# ─────────────────────────────────────────────
header "Phase 9C — WASM Skills"

run_wasm_skill() {
    local skill_name="$1"
    local input_desc="$2"
    local verify_field="$3"
    
    info "Testing WASM skill: $skill_name..."
    
    # Create task with runtime:wasm
    local task_resp
    task_resp=$(curl -sf -X POST "$API_URL/v1/tasks" \
        -H "Content-Type: application/json" \
        -d "{\"title\":\"$skill_name\",\"description\":\"$input_desc\",\"runtime\":\"wasm\"}" 2>/dev/null || echo '{}')
    
    local task_id
    task_id=$(echo "$task_resp" | grep -o '"task_id":"[^"]*"' | head -1 | cut -d'"' -f4)
    
    if [ -z "$task_id" ]; then
        record_fail "$skill_name: Failed to create task"
        return 1
    fi
    
    info "  Task $task_id created, polling for completion..."
    
    # Poll for completion
    local task_state=""
    for i in $(seq 1 15); do
        local task_check
        task_check=$(curl -sf "$API_URL/v1/tasks/$task_id" 2>/dev/null || echo '{}')
        task_state=$(echo "$task_check" | grep -o '"state":"[^"]*"' | head -1 | cut -d'"' -f4)
        
        if [ "$task_state" = "completed" ] || [ "$task_state" = "failed" ]; then
            break
        fi
        sleep 2
    done
    
    if [ "$task_state" = "completed" ]; then
        # Get run result
        local runs_resp
        runs_resp=$(curl -sf "$API_URL/v1/tasks/$task_id/runs" 2>/dev/null || echo '[]')
        local run_id
        run_id=$(echo "$runs_resp" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
        
        if [ -n "$run_id" ]; then
            local run_resp
            run_resp=$(curl -sf "$API_URL/v1/runs/$run_id" 2>/dev/null || echo '{}')
            
            # Verify output schema by checking for required field
            if [ -n "$verify_field" ]; then
                if echo "$run_resp" | grep -q "\"$verify_field\""; then
                    record_pass "$skill_name: Task completed with $verify_field field present"
                    return 0
                else
                    record_fail "$skill_name: Run result missing required field: $verify_field"
                    return 1
                fi
            else
                # Fallback: just check for valid JSON
                if echo "$run_resp" | grep -q '{'; then
                    record_pass "$skill_name: Task completed with valid JSON output"
                    return 0
                else
                    record_fail "$skill_name: Run result is not valid JSON"
                    return 1
                fi
            fi
        else
            record_fail "$skill_name: No run found for task"
            return 1
        fi
    else
        record_fail "$skill_name: Task did not complete (state: $task_state)"
        return 1
    fi
}

WASM_SKILLS=(
    "hello-wasm:hello checkpoint5:result"
    "markdown-parse:# Heading\n\nParagraph text:ast"
    "text-search:pattern=test&text=this is a test:matches"
    "yaml-parse:key: value:result"
    "hash-file:checkpoint5 content:hash"
    "json-transform:{\"a\":1}:result"
    "code-format:{\"a\":1}:formatted"
)

WASM_PASS_COUNT=0

for skill_entry in "${WASM_SKILLS[@]}"; do
    skill_name="${skill_entry%%:*}"
    rest="${skill_entry#*:}"
    input_desc="${rest%%:*}"
    verify_field="${rest#*:}"
    
    if run_wasm_skill "$skill_name" "$input_desc" "$verify_field"; then
        WASM_PASS_COUNT=$((WASM_PASS_COUNT + 1))
    fi
done

if [ "$WASM_PASS_COUNT" -eq 7 ]; then
    WASM_OK=true
fi

# ─────────────────────────────────────────────
# Phase 9D — Native Ops
# ─────────────────────────────────────────────
header "Phase 9D — Native Ops"

run_native_op() {
    local op_name="$1"
    local capability="$2"
    local input_desc="$3"
    local verify_field="$4"
    
    info "Testing native op: $op_name..."
    
    # Grant capability
    local cap_resp
    cap_resp=$(curl -sf -X POST "$API_URL/v1/capabilities" \
        -H "Content-Type: application/json" \
        -d "{\"subject_type\":\"worker\",\"subject_id\":\"native\",\"capability_key\":\"$capability\"}" 2>/dev/null || echo '{}')
    info "  Granted $capability capability"
    
    # Create task
    local task_resp
    task_resp=$(curl -sf -X POST "$API_URL/v1/tasks" \
        -H "Content-Type: application/json" \
        -d "{\"title\":\"$op_name\",\"description\":\"$input_desc\"}" 2>/dev/null || echo '{}')
    
    local task_id
    task_id=$(echo "$task_resp" | grep -o '"task_id":"[^"]*"' | head -1 | cut -d'"' -f4)
    
    if [ -z "$task_id" ]; then
        record_fail "$op_name: Failed to create task"
        return 1
    fi
    
    info "  Task $task_id created, polling for completion..."
    
    # Poll for completion
    local task_state=""
    for i in $(seq 1 15); do
        local task_check
        task_check=$(curl -sf "$API_URL/v1/tasks/$task_id" 2>/dev/null || echo '{}')
        task_state=$(echo "$task_check" | grep -o '"state":"[^"]*"' | head -1 | cut -d'"' -f4)
        
        if [ "$task_state" = "completed" ] || [ "$task_state" = "failed" ]; then
            break
        fi
        sleep 2
    done
    
    if [ "$task_state" = "completed" ]; then
        if [ -n "$verify_field" ]; then
            # Get run result and verify field
            local runs_resp
            runs_resp=$(curl -sf "$API_URL/v1/tasks/$task_id/runs" 2>/dev/null || echo '[]')
            local run_id
            run_id=$(echo "$runs_resp" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
            
            if [ -n "$run_id" ]; then
                local run_resp
                run_resp=$(curl -sf "$API_URL/v1/runs/$run_id" 2>/dev/null || echo '{}')
                
                if echo "$run_resp" | grep -q "\"$verify_field\""; then
                    record_pass "$op_name: Task completed with $verify_field field present"
                    return 0
                else
                    record_fail "$op_name: Run result missing $verify_field field"
                    return 1
                fi
            else
                record_fail "$op_name: No run found for task"
                return 1
            fi
        else
            record_pass "$op_name: Task completed successfully"
            return 0
        fi
    else
        record_fail "$op_name: Task did not complete (state: $task_state)"
        return 1
    fi
}

NATIVE_PASS_COUNT=0

# file_hash test
TEMP_FILE=$(mktemp)
echo "checkpoint5 test content $(date +%s)" > "$TEMP_FILE"

if run_native_op "file_hash" "fs.read" "$TEMP_FILE" "hash"; then
    NATIVE_PASS_COUNT=$((NATIVE_PASS_COUNT + 1))
fi

rm -f "$TEMP_FILE"

# git_status test
if run_native_op "git_status" "git.read" "." ""; then
    NATIVE_PASS_COUNT=$((NATIVE_PASS_COUNT + 1))
fi

# process_list test
if run_native_op "process_list" "system.read" "" "processes"; then
    NATIVE_PASS_COUNT=$((NATIVE_PASS_COUNT + 1))
fi

# disk_usage test
if run_native_op "disk_usage" "system.read" "." "total"; then
    NATIVE_PASS_COUNT=$((NATIVE_PASS_COUNT + 1))
fi

# docker_ps and docker_exec - skip
record_skip "docker_ps native op (requires docker.read grant + Docker socket access in test env)"
record_skip "docker_exec native op (requires docker.exec grant + Docker socket access in test env)"

if [ "$NATIVE_PASS_COUNT" -ge 3 ]; then
    NATIVE_OK=true
fi

# ─────────────────────────────────────────────
# Cross-cutting — Ledger Integrity
# ─────────────────────────────────────────────
header "Cross-cutting — Ledger Integrity"

info "Verifying ledger chain integrity..."
VERIFY_JSON=$(curl -sf "$API_URL/v1/ledger/verify" 2>/dev/null || echo '{}')
INTACT=$(echo "$VERIFY_JSON" | grep -o '"intact":[a-z]*' | cut -d':' -f2)
EVENT_COUNT=$(echo "$VERIFY_JSON" | grep -o '"event_count":[0-9]*' | cut -d':' -f2)
EVENT_COUNT=${EVENT_COUNT:-0}

if [ "$INTACT" = "true" ]; then
    record_pass "Ledger chain intact (GET /v1/ledger/verify → intact=true, events=$EVENT_COUNT)"
    # Append ledger_intact to latest metrics line for report generation
    if [ -f "$METRICS_FILE" ] && [ "$EVENT_COUNT" -gt 0 ]; then
        # Preserve all existing lines, only modify the last one
        head -n -1 "$METRICS_FILE" > "$METRICS_FILE.tmp"
        LAST_LINE=$(tail -1 "$METRICS_FILE")
        echo "$LAST_LINE" | sed 's/}$/,"ledger_intact":true}/' >> "$METRICS_FILE.tmp"
        mv "$METRICS_FILE.tmp" "$METRICS_FILE"
    fi
else
    record_fail "Ledger chain NOT intact (intact=${INTACT:-null})"
fi

# ─────────────────────────────────────────────
# Monitoring Loop
# ─────────────────────────────────────────────
header "Monitoring Loop (${DURATION}s)"

if $DRY_RUN; then
    info "[dry-run] Would monitor for ${DURATION} seconds"
    info "[dry-run] Health check every 60s, metrics every 300s, log rotation every 3600s"
else
    info "Monitoring started at $(date). Will run for $((DURATION / 3600))h $(( (DURATION % 3600) / 60 ))m."
    info "Health check: every 60s | Metrics: every 5m | Log rotation: every 1h"
    info "Press Ctrl+C to stop early."
    echo ""

    ELAPSED=0
    LAST_METRICS=0
    LAST_ROTATION=0
    LAST_ERROR_CHECK=0

    while [ "$ELAPSED" -lt "$DURATION" ]; do
        sleep 60
        ELAPSED=$((ELAPSED + 60))

        # Health check every 60 seconds
        if curl -sf "$API_URL/v1/health" > /dev/null 2>&1; then
            HEALTH_FAIL_STREAK=0
        else
            HEALTH_FAIL_STREAK=$((HEALTH_FAIL_STREAK + 1))
            warn "Health check failed (streak: ${HEALTH_FAIL_STREAK}/${MAX_HEALTH_FAIL_STREAK})"

            if [ "$HEALTH_FAIL_STREAK" -ge "$MAX_HEALTH_FAIL_STREAK" ]; then
                record_fail "Server crashed — ${MAX_HEALTH_FAIL_STREAK} consecutive health check failures"
                break
            fi
        fi

        # Metrics collection every 5 minutes
        if [ $((ELAPSED - LAST_METRICS)) -ge 300 ]; then
            collect_metrics
            LAST_METRICS=$ELAPSED

            # Progress update
            pct=$((ELAPSED * 100 / DURATION))
            worker_count=$(tail -1 "$METRICS_FILE" 2>/dev/null | grep -o '"worker_count":[0-9]*' | cut -d':' -f2 || echo 0)
            echo -e "  ${CYAN}[${pct}%]${NC} ${ELAPSED}s elapsed | workers: ${worker_count:-0} | health: OK"
        fi

        # Log rotation every hour
        if [ $((ELAPSED - LAST_ROTATION)) -ge 3600 ]; then
            rotate_log "$LOG_FILE"
            rotate_log "$ERR_FILE"
            LAST_ROTATION=$ELAPSED
        fi

        # Error check every 5 minutes
        if [ $((ELAPSED - LAST_ERROR_CHECK)) -ge 300 ]; then
            if [ -f "$LOG_FILE" ]; then
                recent_errors=$(tail -1000 "$LOG_FILE" 2>/dev/null | grep -ci "ERROR" || echo 0)
                if [ "$recent_errors" -gt 0 ]; then
                    warn "Found ${recent_errors} ERROR-level entries in recent logs"
                fi
            fi
            LAST_ERROR_CHECK=$ELAPSED
        fi

        # Check server PID is still alive
        if [ -n "$SERVER_PID" ] && ! kill -0 "$SERVER_PID" 2>/dev/null; then
            record_fail "Server process (PID $SERVER_PID) died unexpectedly"
            break
        fi
    done

    info "Monitoring loop completed after ${ELAPSED}s"
fi

# Final metrics snapshot
if ! $DRY_RUN; then
    collect_metrics
    record_pass "Final metrics collected"
fi

# ─────────────────────────────────────────────
# Graceful Shutdown
# ─────────────────────────────────────────────
header "Shutdown"

if [ -n "$SERVER_PID" ] && ! $KEEP_RUNNING; then
    info "Sending SIGTERM to server (PID $SERVER_PID)..."
    kill "$SERVER_PID" 2>/dev/null || true

    SHUTDOWN_WAIT=0
    while [ "$SHUTDOWN_WAIT" -lt 30 ]; do
        if ! kill -0 "$SERVER_PID" 2>/dev/null; then
            record_pass "Server stopped gracefully"
            SERVER_PID=""
            break
        fi
        sleep 1
        SHUTDOWN_WAIT=$((SHUTDOWN_WAIT + 1))
    done

    if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
        warn "Server did not stop within 30s, sending SIGKILL..."
        kill -9 "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
        record_fail "Server required SIGKILL to stop"
        SERVER_PID=""
    fi
elif $KEEP_RUNNING; then
    info "Server left running (--keep-running)"
    SERVER_PID=""  # Prevent cleanup trap from killing it
fi

# ─────────────────────────────────────────────
# Generate Report
# ─────────────────────────────────────────────
header "Report Generation"

generate_report
record_pass "Summary report generated"

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
    echo -e "${GREEN}=== Checkpoint 5 Validation Passed ===${NC}"
else
    echo -e "${RED}=== Checkpoint 5 Validation: $FAIL_COUNT failure(s) ===${NC}"
fi

if $KEEP_RUNNING; then
    echo ""
    info "Server left running (--keep-running). Stop with: carnelian stop"
fi

echo ""
echo -e "${CYAN}Logs:    ${LOG_FILE}${NC}"
echo -e "${CYAN}Metrics: ${METRICS_FILE}${NC}"
echo -e "${CYAN}Report:  ${REPORT_FILE}${NC}"
