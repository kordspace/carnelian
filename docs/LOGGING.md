# 🔥 Logging Philosophy for Carnelian OS

## What Logs Are For (in order)

1. **Operational truth:** "What happened?" (state transitions, failures, retries, latency)
2. **Debuggability:** "Why did it happen?" (inputs/outputs, decisions, guards, fallbacks)
3. **Auditability:** "Who/what triggered it?" (tenant/user, request, tool invocation chain)
4. **Performance:** "Where is time spent?" (timers, spans, queue times, DB duration)
5. **Safety:** "Did we leak secrets?" (redaction + strict whitelists)

---

## Golden Rules

1. **Structured first:** JSON logs or logfmt, not prose
2. **Every request has a trace:** `trace_id`, `span_id`, and `request_id`
3. **Stable event names:** treat `event` as an API (don't change it casually)
4. **Errors are shaped:** `err.kind`, `err.code`, `err.msg`, `err.stack` (if available)
5. **Redact by default:** whitelist fields, never blacklist

---

## Brand Mapping: How Logs "Feel" Like 🔥 Carnelian OS

Think of branding as a thin "skin" around serious structured logs.

### Prefix Conventions

| Icon | Brand | Usage |
|------|-------|-------|
| 🔥 | Carnelian OS | System/runtime logs (core/gateway/services) |
| 🦎 | Lian | Agent reasoning, memory shaping decisions, tool selection |
| 💎 | Core | Architectural guarantees, security invariants, foundational events |
| ✅ | — | Health state / completed milestone |
| 🟣 | — | Insight / design rationale / non-fatal unusual decision |
| 🟢 🟡 🔴 | — | Optional level glyphs for human readability |

**Important:** Emojis go in `icon` or `brand` fields OR in the message prefix—but keep the structured fields as the source of truth.

---

## Logging Levels and When to Use Them

### TRACE (very noisy)
Use only behind a feature flag.
- Token counts
- Per-step memory candidates
- Gateway shaping transformations

### DEBUG
Developer intent + decision points.
- "picked compression strategy D"
- "tool schema validated"
- "cache hit"

### INFO
High-level system events.
- Request start/end
- Tool invocation begin/end
- Memory commit
- Migration start

### WARN
Unexpected but recoverable.
- Retry
- Partial degradation
- Fallbacks
- Non-fatal schema mismatch corrected

### ERROR
Request failed or subsystem failed.
- Tool invocation failed
- DB write failed
- Invalid config
- Guard violation

### FATAL
Process can't continue safely.
- Corrupted state
- Critical invariant failure
- Unsafe config detected

---

## Canonical Fields

These fields should be in every log line (when applicable):

### Identity & Routing

| Field | Example |
|-------|---------|
| `service` | `carnelian-core`, `gateway`, `fs-gateway` |
| `component` | `memory`, `tools`, `scheduler`, `auth` |
| `env` | `dev`, `staging`, `prod` |
| `version` | Git SHA or semver |
| `host`, `pid` | — |

### Correlation

| Field | Description |
|-------|-------------|
| `trace_id` | Distributed trace ID |
| `span_id` | Current span |
| `request_id` | External or generated |
| `session_id` | Agent session |
| `tenant_id`, `user_id` | If present |

### Brand / Persona

| Field | Values |
|-------|--------|
| `brand` | `carnelian`, `lian`, `core` |
| `icon` | `🔥`, `🦎`, `💎`, `✅`, `🟣` |
| `agent` | `lian` |
| `agent_id` | UUID |

### Event Shape

| Field | Description |
|-------|-------------|
| `event` | Stable machine name, snake_case |
| `msg` | Human short sentence |
| `severity` | Level |
| `ts` | Timestamp |

### Timings

| Field | Description |
|-------|-------------|
| `duration_ms` | Total duration |
| `db_ms` | Database time |
| `tool_ms` | Tool execution time |
| `queue_ms` | Queue wait time |

### Error Shape (only on warn/error)

| Field | Description |
|-------|-------------|
| `err.kind` | Enum-ish: `db_timeout`, `invalid_input`, `tool_exec_failed` |
| `err.code` | Numeric/string code |
| `err.msg` | Error message |
| `err.stack` | Stack trace (if available) |

---

## Event Taxonomy

### 🔥 Runtime Lifecycle

```
runtime_start
runtime_ready
runtime_shutdown
config_loaded
config_invalid
```

### 💎 Security & Ledger

```
capability_granted
capability_denied
capability_revoked
ledger_event_created
ledger_hash_verified
ledger_integrity_check
signature_verified
signature_failed
```

### 🧠 Memory

```
memory_fetch_start
memory_fetch_end
memory_compress_start
memory_compress_end
memory_write_start
memory_write_end
memory_redaction_applied
memory_budget_exceeded
```

### 🛠 Tools

```
tool_registry_loaded
tool_invoke_start
tool_invoke_end
tool_invoke_failed
tool_timeout
tool_schema_violation
```

### 🌉 Gateway

```
gateway_request_start
gateway_request_end
gateway_shape_start
gateway_shape_end
gateway_rate_limited
```

### 🗃 DB

```
db_query_start
db_query_end
db_tx_begin
db_tx_commit
db_tx_rollback
```

---

## Examples: Branded Log Implementations

### 1) Request Start/End (🔥 Carnelian OS)

```json
{
  "ts": "2026-02-04T19:21:33.120Z",
  "severity": "INFO",
  "icon": "🔥",
  "brand": "carnelian",
  "service": "gateway",
  "component": "http",
  "event": "gateway_request_start",
  "msg": "Request received",
  "trace_id": "c2f7b2a9c1b64b9c",
  "span_id": "a1d03c9b8e2b",
  "request_id": "req_8b7b",
  "method": "POST",
  "path": "/v1/agent/run",
  "tenant_id": "t_1042",
  "user_id": "u_7781"
}
```

```json
{
  "ts": "2026-02-04T19:21:33.982Z",
  "severity": "INFO",
  "icon": "✅",
  "brand": "carnelian",
  "service": "gateway",
  "component": "http",
  "event": "gateway_request_end",
  "msg": "Request completed",
  "trace_id": "c2f7b2a9c1b64b9c",
  "span_id": "a1d03c9b8e2b",
  "request_id": "req_8b7b",
  "status": 200,
  "duration_ms": 862,
  "tool_ms": 441,
  "db_ms": 122
}
```

### 2) Memory Compression Decision (🦎 Lian)

```json
{
  "ts": "2026-02-04T19:21:33.310Z",
  "severity": "DEBUG",
  "icon": "🦎",
  "brand": "lian",
  "service": "carnelian-core",
  "component": "memory",
  "event": "memory_compress_start",
  "msg": "Starting memory compression",
  "trace_id": "c2f7b2a9c1b64b9c",
  "session_id": "sess_2f1a",
  "agent": "lian",
  "strategy": "hybrid",
  "budget_tokens": 4200,
  "mem_items_in": 182
}
```

```json
{
  "ts": "2026-02-04T19:21:33.612Z",
  "severity": "INFO",
  "icon": "🟣",
  "brand": "lian",
  "service": "carnelian-core",
  "component": "memory",
  "event": "memory_compress_end",
  "msg": "Compression complete (deterministic)",
  "trace_id": "c2f7b2a9c1b64b9c",
  "session_id": "sess_2f1a",
  "agent": "lian",
  "mem_items_out": 37,
  "tokens_out": 3980,
  "duration_ms": 302,
  "notes": "Dropped low-signal items; preserved user prefs + active tasks"
}
```

### 3) Tool Invocation (🔥 start, ✅ end, 🔴 error)

```json
{
  "ts": "2026-02-04T19:21:33.702Z",
  "severity": "INFO",
  "icon": "🔥",
  "brand": "carnelian",
  "service": "gateway",
  "component": "tools",
  "event": "tool_invoke_start",
  "msg": "Invoking tool",
  "trace_id": "c2f7b2a9c1b64b9c",
  "tool_name": "filesystem.read",
  "tool_version": "1.3.0",
  "tool_timeout_ms": 2000,
  "args_schema": "v2",
  "args_fingerprint": "sha256:9f1d..."
}
```

```json
{
  "ts": "2026-02-04T19:21:33.934Z",
  "severity": "INFO",
  "icon": "✅",
  "brand": "carnelian",
  "service": "gateway",
  "component": "tools",
  "event": "tool_invoke_end",
  "msg": "Tool completed",
  "trace_id": "c2f7b2a9c1b64b9c",
  "tool_name": "filesystem.read",
  "status": "ok",
  "duration_ms": 232,
  "result_bytes": 18422
}
```

```json
{
  "ts": "2026-02-04T19:21:36.104Z",
  "severity": "ERROR",
  "icon": "🔥",
  "brand": "carnelian",
  "service": "gateway",
  "component": "tools",
  "event": "tool_invoke_failed",
  "msg": "Tool failed",
  "trace_id": "c2f7b2a9c1b64b9c",
  "tool_name": "db.query",
  "duration_ms": 2003,
  "err.kind": "db_timeout",
  "err.code": "DB_TIMEOUT",
  "err.msg": "query exceeded timeout",
  "retrying": true,
  "retry_in_ms": 250
}
```

### 4) Logfmt for Local Dev

```
ts=2026-02-04T19:21:33.120Z lvl=INFO icon=🔥 brand=carnelian svc=gateway cmp=http event=gateway_request_start trace=c2f7b2a9c1b64b9c req=req_8b7b msg="Request received" method=POST path=/v1/agent/run
```

```
ts=2026-02-04T19:21:33.612Z lvl=INFO icon=🟣 brand=lian svc=carnelian-core cmp=memory event=memory_compress_end trace=c2f7b2a9c1b64b9c sess=sess_2f1a msg="Compression complete (deterministic)" tokens_out=3980 dur_ms=302
```

---

## Branding Patterns

### Pattern A: "Two-Voice" Logging (system vs agent)

- **🔥 Carnelian OS logs:** boundaries, contracts, durations, results
- **🦎 Lian logs:** decisions, reasons, strategy picks, compress/extract logic

This keeps agent personality without polluting operational logs.

### Pattern B: "Event + Result" Pairs

For anything important:
- `*_start` at INFO/DEBUG
- `*_end` at INFO with `duration_ms`
- `*_failed` at ERROR with `err.*`

Example pairs:
- `tool_invoke_start` → `tool_invoke_end` / `tool_invoke_failed`
- `memory_compress_start` → `memory_compress_end`

### Pattern C: "Human-Friendly Prefix, Machine Fields"

If you want the emoji in the message:
```json
"msg": "🔥 Request received"
```
But keep `icon` and `brand` too.

---

## Redaction + Security Logging

### Never Log

- Raw prompts, raw memory text
- Secrets, tokens, cookies, auth headers
- Full tool args unless explicitly marked safe

### Do Log (safe fingerprints)

- `args_fingerprint` (sha256 of canonical JSON)
- `payload_bytes`
- `schema_version`
- Counts: tokens, items, durations, result size

### Example

```json
{
  "severity": "WARN",
  "icon": "🔥",
  "brand": "carnelian",
  "event": "memory_redaction_applied",
  "msg": "Redaction applied to tool output",
  "tool_name": "web.search",
  "redacted_fields": ["api_key", "Authorization"],
  "trace_id": "..."
}
```

---

## Dark-Mode / Light-Mode Log Viewing

If you build a log viewer UI:

**Light mode ("Forge")**
- INFO/OK highlights: gold
- Brand headers: ember

**Dark mode ("Night Lab")**
- INFO links/interactive: jade
- "insight/agent" callouts: amethyst
- Errors: keep red, but use sparingly on black backgrounds

Optional: add a `tone` field
- `tone: "forge"` in light-mode exports
- `tone: "night_lab"` in dark-mode exports

(This is purely for UI rendering; don't use it for logic.)

---

## Implementation Checklist

- [ ] Define canonical fields + event taxonomy in this doc
- [ ] Add `trace_id`/`span_id` propagation across gateway → core → tools
- [ ] Standardize start/end/failed event pairs for memory + tool execution
- [ ] Implement redaction + safe fingerprints
- [ ] Add 🦎 Lian logs only for decisions (not raw content)
- [ ] Ship JSON logs in prod, logfmt in dev
