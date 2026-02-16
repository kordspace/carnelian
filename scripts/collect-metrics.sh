#!/usr/bin/env bash
# Carnelian OS - Metrics Collection Script
# Standalone script for polling health, metrics, status, and database endpoints.
#
# Usage:
#   ./scripts/collect-metrics.sh                          # Single snapshot to stdout
#   ./scripts/collect-metrics.sh --url http://host:18789  # Custom server URL
#   ./scripts/collect-metrics.sh --pid 12345              # Include process metrics
#   ./scripts/collect-metrics.sh --output metrics.jsonl   # Append to file
#   ./scripts/collect-metrics.sh --loop 300               # Repeat every 300 seconds

set -euo pipefail

API_URL="http://localhost:18789"
SERVER_PID=""
OUTPUT_FILE=""
LOOP_INTERVAL=0
DB_URL="postgresql://carnelian:carnelian@localhost:5432/carnelian"

for i in "$@"; do
    case $i in
        --url)       shift; API_URL="${1:-$API_URL}"; shift ;;
        --pid)       shift; SERVER_PID="${1:-}"; shift ;;
        --output)    shift; OUTPUT_FILE="${1:-}"; shift ;;
        --loop)      shift; LOOP_INTERVAL="${1:-0}"; shift ;;
        --db-url)    shift; DB_URL="${1:-$DB_URL}"; shift ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --url URL        Server base URL (default: http://localhost:18789)"
            echo "  --pid PID        Server process ID for system metrics"
            echo "  --output FILE    Append JSON metrics to file (default: stdout)"
            echo "  --loop SECONDS   Repeat collection every N seconds (0 = once)"
            echo "  --db-url URL     PostgreSQL connection URL"
            echo "  --help           Show this help message"
            exit 0
            ;;
    esac
done

collect_snapshot() {
    local timestamp
    timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    # ── Health endpoint ──
    local health_json
    health_json=$(curl -sf --max-time 5 "$API_URL/v1/health" 2>/dev/null || echo '{"status":"unreachable","database":"unknown"}')
    local health_status
    health_status=$(echo "$health_json" | sed -n 's/.*"status":"\([^"]*\)".*/\1/p' | head -1)
    health_status=${health_status:-unreachable}
    local health_db
    health_db=$(echo "$health_json" | sed -n 's/.*"database":"\([^"]*\)".*/\1/p' | head -1)
    health_db=${health_db:-unknown}

    # ── Status endpoint ──
    local status_json
    status_json=$(curl -sf --max-time 5 "$API_URL/v1/status" 2>/dev/null || echo '{}')
    local queue_depth
    queue_depth=$(echo "$status_json" | sed -n 's/.*"queue_depth":\([0-9]*\).*/\1/p' | head -1)
    queue_depth=${queue_depth:-0}
    local worker_count
    worker_count=$(echo "$status_json" | grep -o '"id"' | wc -l | tr -d ' ')
    worker_count=${worker_count:-0}

    # ── Metrics endpoint ──
    local metrics_json
    metrics_json=$(curl -sf --max-time 5 "$API_URL/v1/metrics" 2>/dev/null || echo '{}')
    local task_latency_p50
    task_latency_p50=$(echo "$metrics_json" | sed -n 's/.*"task_latency_p50_ms":\([0-9.]*\).*/\1/p' | head -1)
    task_latency_p50=${task_latency_p50:-0}
    local task_latency_p95
    task_latency_p95=$(echo "$metrics_json" | sed -n 's/.*"task_latency_p95_ms":\([0-9.]*\).*/\1/p' | head -1)
    task_latency_p95=${task_latency_p95:-0}
    local task_latency_p99
    task_latency_p99=$(echo "$metrics_json" | sed -n 's/.*"task_latency_p99_ms":\([0-9.]*\).*/\1/p' | head -1)
    task_latency_p99=${task_latency_p99:-0}
    local event_throughput
    event_throughput=$(echo "$metrics_json" | sed -n 's/.*"event_throughput_per_sec":\([0-9.]*\).*/\1/p' | head -1)
    event_throughput=${event_throughput:-0}

    # ── Database queries ──
    local heartbeat_count=0
    local tasks_pending=0
    local tasks_running=0
    local tasks_completed=0
    local tasks_auto_queued=0

    if command -v psql &>/dev/null; then
        heartbeat_count=$(psql "$DB_URL" -t -A \
            -c "SELECT COUNT(*) FROM heartbeat_history WHERE created_at > NOW() - INTERVAL '24 hours'" \
            2>/dev/null || echo 0)
        heartbeat_count=$(echo "$heartbeat_count" | tr -d '[:space:]')

        # Task counts by state
        local task_states
        task_states=$(psql "$DB_URL" -t -A \
            -c "SELECT state, COUNT(*) FROM tasks GROUP BY state" \
            2>/dev/null || echo "")
        tasks_pending=$(echo "$task_states" | grep "^pending|" | cut -d'|' -f2 || echo 0)
        tasks_pending=${tasks_pending:-0}
        tasks_running=$(echo "$task_states" | grep "^running|" | cut -d'|' -f2 || echo 0)
        tasks_running=${tasks_running:-0}
        tasks_completed=$(echo "$task_states" | grep "^completed|" | cut -d'|' -f2 || echo 0)
        tasks_completed=${tasks_completed:-0}

        # Auto-queued tasks
        tasks_auto_queued=$(psql "$DB_URL" -t -A \
            -c "SELECT COUNT(*) FROM tasks WHERE title LIKE '[TODO]%' OR title LIKE '[TASK]%'" \
            2>/dev/null || echo 0)
        tasks_auto_queued=$(echo "$tasks_auto_queued" | tr -d '[:space:]')
    fi

    # ── System metrics ──
    local memory_rss_kb=0
    local memory_vsz_kb=0
    local cpu_percent=0
    local mem_percent=0

    if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
        if [[ "$OSTYPE" == "msys"* ]] || [[ "$OSTYPE" == "cygwin"* ]]; then
            memory_rss_kb=$(tasklist /FI "PID eq $SERVER_PID" /FO CSV /NH 2>/dev/null | \
                awk -F',' '{gsub(/[" K,]/, "", $5); print $5}' 2>/dev/null || echo 0)
        else
            memory_rss_kb=$(ps -p "$SERVER_PID" -o rss= 2>/dev/null | tr -d ' ' || echo 0)
            memory_vsz_kb=$(ps -p "$SERVER_PID" -o vsz= 2>/dev/null | tr -d ' ' || echo 0)
            cpu_percent=$(ps -p "$SERVER_PID" -o %cpu= 2>/dev/null | tr -d ' ' || echo 0)
            mem_percent=$(ps -p "$SERVER_PID" -o %mem= 2>/dev/null | tr -d ' ' || echo 0)
        fi
    fi

    # ── Build JSON output ──
    local json
    json=$(cat <<EOF
{"timestamp":"${timestamp}","health":{"status":"${health_status}","database":"${health_db}"},"metrics":{"task_latency_p50_ms":${task_latency_p50},"task_latency_p95_ms":${task_latency_p95},"task_latency_p99_ms":${task_latency_p99},"event_throughput_per_sec":${event_throughput}},"status":{"queue_depth":${queue_depth},"workers":${worker_count}},"heartbeats_24h":${heartbeat_count:-0},"tasks":{"pending":${tasks_pending:-0},"running":${tasks_running:-0},"completed":${tasks_completed:-0},"auto_queued":${tasks_auto_queued:-0}},"memory_rss_kb":${memory_rss_kb:-0},"memory_vsz_kb":${memory_vsz_kb:-0},"cpu_percent":${cpu_percent:-0},"mem_percent":${mem_percent:-0}}
EOF
)

    if [ -n "$OUTPUT_FILE" ]; then
        echo "$json" >> "$OUTPUT_FILE"
    else
        echo "$json"
    fi
}

# ── Main ──
if [ "$LOOP_INTERVAL" -gt 0 ]; then
    echo "Collecting metrics every ${LOOP_INTERVAL}s. Press Ctrl+C to stop."
    while true; do
        collect_snapshot
        sleep "$LOOP_INTERVAL"
    done
else
    collect_snapshot
fi
