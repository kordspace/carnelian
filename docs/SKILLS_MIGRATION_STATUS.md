# Skills Migration Status

## Overview

This document tracks the migration of skills from the THUMMIM TypeScript codebase to the CARNELIAN Rust-based system.

## Current State

### THUMMIM Skills Inventory
- **Tools in `src/agents/tools/`**: 58 TypeScript files
- **Total TypeScript files in agents**: 445+ files
- **Estimated skill implementations**: ~400-500 (many tools contain multiple skill implementations)

### CARNELIAN Skills Inventory
- **Skills in registry**: **135+ skills with manifests**
  - **47 Node runtime skills**: web-search, web-fetch, discord-send, discord-guild, discord-channel, discord-role, discord-moderate, telegram-send, telegram-group, telegram-media, slack-send, slack-channel, slack-file, slack-react, whatsapp-send, message-send, image-generate, image-analyze, text-to-speech, canvas-render, memory-write, memory-read, browser-automation, browser-navigate, browser-screenshot, browser-click, browser-type, browser-pdf, cron-schedule, cron-list, cron-add, cron-remove, cron-run, session-spawn, session-list, session-history, session-status, nodes-list, gateway-query, gateway-config, gateway-restart, gateway-update, cascade-run, agent-step, agents-list, http-request, http-webhook, template-render
  - **26 Native Ops** (Rust): file-hash, file-write, file-delete, file-move, file-search, file-watch, file-metadata, git-status, git-diff, git-commit, git-log, git-branch, docker-ps, docker-exec, docker-logs, docker-stats, process-list, disk-usage, network-stats, env-get, sql-query, sql-schema, archive-zip, archive-tar, system-healthcheck, echo
  - **37 WASM skills** (Rust): hello-wasm, markdown-parse, text-search, yaml-parse, hash-file, json-transform, code-format, csv-parse, csv-generate, xml-parse, xml-generate, toml-parse, toml-generate, ini-parse, env-parse, diff-text, patch-apply, crypto-encrypt, crypto-decrypt, crypto-sign, crypto-verify, crypto-keygen, regex-match, regex-replace, base64-encode, base64-decode, url-parse, url-build, image-resize, image-convert, image-metadata, qr-generate, qr-decode, datetime-parse, datetime-format, uuid-generate, slug-generate, color-convert, units-convert, math-calculate, code-ast-js
  - **5 Python skills**: code-ast-python, code-lint-python, code-deps-python, stats-analyze, chart-generate
  - **20+ Node skills** (additional with partial manifests): code-lint-js, code-deps-js, pdf-extract-text, pdf-metadata, email-parse, email-send, currency-convert, data-validate, schema-generate, graphql-query, model-usage
- **Note**: Skills with manifests are ready for implementation. Some have placeholder implementations pending full migration from THUMMIM.

### Worker Infrastructure
- **Node Worker**: ✅ Built and operational (13 TypeScript files)
- **Python Worker**: ✅ Built and operational
- **WASM Worker**: ✅ Built and operational (wasmtime 27 + WASI P1)
- **Native Ops Worker**: ✅ Built (20 operations: file ops, git ops, docker ops, system ops, env ops)

## Migration Gap Analysis

### Critical Gaps

1. **Skill Count Progress**: 
   - Target: 600 skills in default library
   - Current: 135+ skills with manifests (85 new manifests created)
   - Fully implemented: 50+ skills
   - **Gap: ~465 skills remaining** (down from ~550)

2. **Migration Progress by Category**:
   - ✅ **Discord actions**: 5 skills (guild, channel, role, moderate, send)
   - ✅ **Slack actions**: 4 skills (send, channel, file, react)
   - ✅ **Telegram actions**: 3 skills (send, group, media)
   - ✅ **Browser automation**: 6 skills (automation, navigate, screenshot, click, type, pdf)
   - ✅ **Session management**: 4 skills (spawn, list, history, status)
   - ✅ **Cron/scheduling**: 5 skills (schedule, list, add, remove, run)
   - ✅ **Gateway tools**: 4 skills (query, config, restart, update)
   - ✅ **Agent orchestration**: 3 skills (step, list, cascade)
   - ✅ **Data processing**: 15 skills (CSV, XML, TOML, INI, JSON, YAML, env)
   - ✅ **Cryptography**: 5 skills (encrypt, decrypt, sign, verify, keygen)
   - ✅ **Code analysis**: 6 skills (AST parsing, linting, dependency analysis)
   - ✅ **File operations**: 10 skills (extended ops, archives, metadata, watch)
   - ✅ **Image processing**: 6 skills (resize, convert, metadata, QR codes)
   - ✅ **Utilities**: 15+ skills (datetime, UUID, regex, base64, URL, colors, units)

3. **Rust Migration Status**:
   - Native Ops Worker: 4 operations in Rust ✅
   - Core system: 100% Rust ✅
   - Skills: <1% in Rust/WASM ❌
   - **Most skills still TypeScript-dependent via Node worker**

## Migration Strategy

### Phase 1: Immediate Actions (Week 1-2)
1. **Audit THUMMIM skills directory structure**
   - Map all 58 tool files to skill implementations
   - Identify dependencies and groupings
   - Categorize by runtime suitability (Node/Python/WASM/Native)

2. **Create skill manifest templates**
   - Standardize `skill.json` format
   - Define capability requirements per skill
   - Document sandbox configurations

3. **Bulk import Node-compatible skills**
   - Copy skill implementations to `workers/node-worker/skills/`
   - Generate manifests for each skill
   - Register in `skills/registry/`

### Phase 2: Rust Migration Priorities (Week 3-8)
1. **High-value Native Ops** (extend `carnelian-worker-native/`):
   - File operations (read, write, search, hash)
   - Git operations (status, diff, commit, push)
   - Docker operations (ps, exec, logs, stats)
   - System operations (process list, disk usage, network stats)

2. **WASM Candidates** (sandboxed, portable):
   - Text processing (markdown, JSON, YAML parsing)
   - Data transformation (CSV, XML, format conversion)
   - Cryptographic operations (hashing, signing, verification)
   - Code analysis (linting, formatting, AST parsing)

3. **Keep in Node/Python**:
   - Browser automation (Playwright dependency)
   - Complex API integrations (npm ecosystem)
   - ML/AI operations (Python ecosystem)
   - Channel adapters (existing SDK dependencies)

### Phase 3: Skill Book Population (Week 9-12)
1. **Organize into 6 categories**:
   - Code (read_file, search_code, run_tests, git_status, etc.)
   - Research (web_search, docs_lookup, paper_retrieval)
   - Communication (send_message, schedule_meeting, draft_email)
   - Creative (image_gen, audio_synthesis, copywriting)
   - Data (query_db, transform_dataset, generate_report)
   - Automation (browser_automation, api_orchestration, cron_tasks)

2. **Create activation flows**:
   - API token management
   - Capability grant workflows
   - Sandbox configuration UI

## Rust Migration Checklist

### Native Ops (Rust inline)
- [x] `file_hash` (blake3)
- [x] `git_status` (git2)
- [x] `docker_ps` (bollard)
- [x] `dir_list` (walkdir)
- [x] `file_read`
- [x] `file_write`
- [x] `file_search` (ripgrep)
- [x] `file_delete`
- [x] `file_move`
- [x] `git_diff`
- [x] `git_commit`
- [x] `git_log`
- [x] `git_branch`
- [x] `docker_exec`
- [x] `docker_logs`
- [x] `docker_stats`
- [x] `process_list` (sysinfo)
- [x] `disk_usage` (sysinfo)
- [x] `network_stats`
- [x] `env_get`

### WASM Skills (wasmtime)
- [x] `hello-wasm` (demo)
- [x] `markdown_parse`
- [x] `json_transform`
- [x] `yaml_parse`
- [x] `code_format`
- [x] `hash_file` (blake3 WASM)
- [x] `text_search` (regex WASM)

### Node Skills (TypeScript via Node worker)
- [x] `echo`
- [x] `healthcheck`
- [x] `web-search`
- [x] `web-fetch`
- [x] `discord-send`
- [x] `telegram-send`
- [x] `slack-send`
- [x] `whatsapp-send`
- [x] `message-send`
- [x] `image-generate`
- [x] `image-analyze`
- [x] `text-to-speech`
- [x] `canvas-render`
- [x] `memory-write`
- [x] `memory-read`
- [x] `browser-automation`
- [x] `cron-schedule`
- [x] `session-spawn`
- [x] `nodes-list`
- [x] `gateway-query`
- [x] `cascade-run`
- [x] `agent-step`

## Elixir System Status

### Database Schema
- ✅ `elixirs` table (RAG-based knowledge persistence)
- ✅ `elixir_versions` table (version history)
- ✅ `elixir_usage` table (usage tracking)
- ✅ `sub_agent_elixirs` table (assignments)
- ✅ `elixir_drafts` table (auto-generated proposals)

### Elixir Types
1. **skill_backup** - Skill knowledge snapshots
2. **domain_knowledge** - Domain-specific expertise
3. **context_cache** - Cached context for performance
4. **training_data** - Training datasets for fine-tuning

### XP Integration
- Elixir creation awards XP
- Elixir usage tracked for effectiveness scoring
- Quality scores (0-100) affect XP rewards

### Implemented Components
- [x] Elixir creation API endpoints
- [x] Elixir retrieval/search endpoints
- [x] Elixir activation UI
- [x] Auto-draft generation logic
- [x] Effectiveness scoring algorithm
- [x] Elixir-to-skill binding system

## Next Steps

### ✅ Phase 9 Complete (Checkpoint 5)

**Phase 9A — Skill Registry Expansion**:
- ✅ Added 15 missing Native Op `skill.json` manifests (file-write, file-delete, file-move, file-search, git-diff, git-commit, git-log, git-branch, docker-exec, docker-logs, docker-stats, process-list, disk-usage, network-stats, env-get)
- ✅ Registry now contains **50+ skills** (exceeds Checkpoint 5 target of ≥50)
- ✅ All 6 skill categories represented (Node, Native, WASM, Python, Shell, Elixir)

**Phase 9B — Elixir XP Integration**:
- ✅ Moved XP awarding logic from HTTP handlers into `ElixirManager`
- ✅ `create_elixir()` awards 50 XP for `ElixirCreated` event
- ✅ `approve_draft()` awards 25 XP for `ElixirApproved` event
- ✅ Broke circular dependency between `xp.rs` and `elixir.rs` by inlining SQL logic

**Phase 9C — Validation Infrastructure**:
- ✅ Created `scripts/checkpoint5-validation.sh` with 5 validation phases
- ✅ Verified all 4 Elixir API endpoints return HTTP 200/201
- ✅ Verified all 7 WASM skills produce valid JSON output
- ✅ Verified 4 representative Native Ops complete successfully
- ✅ Confirmed ledger integrity maintained across all operations

**Checkpoint 5 Status**: **VALIDATED** ✅

---

### 🎯 Phase 10 Targets

1. **Python Skills** (Gap: 50 skills):
   - Implement Python worker skill execution pipeline
   - Create 10-15 data science/ML skills (pandas, numpy, scikit-learn)
   - Add Python skill manifests to registry

2. **WASM Skill Expansion** (Gap: 23 skills toward 30 target):
   - Port 10-15 compute-intensive operations to WASM
   - Add cryptographic primitives (AES, RSA, ECDSA)
   - Implement data transformation skills (CSV, XML, Protobuf parsers)

3. **Node Skill Migration** (Gap: ~378 skills toward 400 target):
   - Migrate remaining THUMMIM tools (Discord, Slack, Telegram actions)
   - Add browser automation skills (Playwright/Puppeteer wrappers)
   - Implement canvas rendering and image manipulation skills

4. **Elixir API Completion** (Gap: 3 endpoints toward 10 target):
   - Add `PUT /v1/elixirs/:id` (update elixir)
   - Add `DELETE /v1/elixirs/:id` (deactivate elixir)
   - Add `POST /v1/elixirs/:id/versions` (create new version)

## How to Add a New Skill from THUMMIM

### SKILL.md Frontmatter Format

Every skill must have a `SKILL.md` file with YAML frontmatter. The following fields are parsed by `validateManifest()` in `workers/node-worker/src/manifest.ts`:

```yaml
---
name: skill-name              # Required: kebab-case skill identifier
description: "One-line description"  # Required: human-readable description
homepage: https://example.com # Optional: URL to docs or source

metadata:
  openclaw:
    emoji: "🔍"              # Display emoji
    requires:
      bins:                  # Host binaries that must be on PATH
        - curl
        - jq
      env:                   # Environment variables that must be set
        - API_KEY
        - SECRET_TOKEN
    primaryEnv: API_KEY      # The single most important env var key
    install:                 # InstallInstruction objects
      - id: homebrew-curl
        kind: homebrew
        formula: curl
      - id: apt-jq
        kind: apt
        bins: [jq]
    os:                      # Supported OS list
      - linux
      - darwin
      - windows

  carnelian:
    runtime: node            # Required: "node" | "python" | "shell"
    version: "1.0.0"         # Required: semver string
    capabilities:            # String array
      - net.http
      - fs.read
    sandbox:
      network: full          # "none" | "localhost" | "full"
      resourceLimits:
        maxMemoryMB: 512     # Integer (default 512)
        maxCpuPercent: 50    # Integer (default 50)
        timeoutSecs: 300     # Integer (default 300)
      mounts:                # Volume mounts
        - host: /tmp
          container: /workspace
          readonly: false
      env:                   # Key/value map; values may use ${HOST_VAR} interpolation
        API_KEY: "${API_KEY}"
        BASE_URL: "http://localhost:3000"
---
```

### scripts/index.js Wrapper Contract

The Node worker (`executeNode()` in `workers/node-worker/src/sandbox.ts`) expects:

- **File location**: `scripts/index.js` (or `scripts/main.js`) inside the skill directory
- **Export format**: `module.exports.run = async (input) => { ... }` (or `.execute`)
- **Input**: `input` is the raw JSON object passed by the caller
- **Output**: Must return a plain JSON-serializable object
- **Logging**: Use `console.log/warn/error/debug` to emit structured log events to the worker
  - Do **not** use `process.stdout.write` directly
- **Shell runtime**: For `shell` runtime skills (e.g., `browser-automation`), the entry point is `scripts/index.sh` and the result must be printed as a single JSON line to stdout

### Sandbox Globals Reference

The following globals are available inside the VM context (from `createNodeSandbox()` in `workers/node-worker/src/sandbox.ts`):

| Global | Notes |
|--------|-------|
| `input` | The skill's input object (same as the `run(input)` argument) |
| `module.exports` / `exports` | CommonJS export surface |
| `console` | `.log` `.warn` `.error` `.debug` — emits structured log events |
| `fetch` | Node 18+ global; **only available when `sandbox.network` ≠ `"none"`** |
| `URL`, `URLSearchParams` | Standard web URL APIs |
| `Headers`, `Request`, `Response` | Fetch API types |
| `WebSocket` | **Requires Node ≥ 21**; gated by `sandbox.network` |
| `Buffer` | Node.js Buffer (encoding/decoding) |
| `crypto` | Full `node:crypto` module (hashing, HMAC, random bytes, etc.) |
| `fs` | `node:fs/promises` (readFile, writeFile, mkdir, etc.) |
| `process.env` | Read-only proxy of host env + `sandbox.env` overrides; throws on write |
| `abortSignal` | `AbortSignal` — check `.aborted` in long-running async loops |
| `setTimeout`, `clearTimeout`, `setInterval`, `clearInterval` | Standard timers |
| `JSON`, `Math`, `Date`, `Promise`, `Map`, `Set`, `RegExp` | Standard JS globals |
| `parseInt`, `parseFloat`, `isNaN`, `isFinite` | Standard JS functions |
| `encodeURIComponent`, `decodeURIComponent`, `encodeURI`, `decodeURI` | URI helpers |

> **Not available in the sandbox:** `require`, `__dirname`, `__filename`, `process.exit`, `process.stdout`, `child_process`. Skills that need npm packages or subprocess spawning must use the `shell` runtime instead.

## Metrics

| Metric | Target | Current | Gap |
|--------|--------|---------|-----|
| Total Skills | 600 | 135+ | ~465 |
| Rust Native Ops | 30 | 26 | 4 |
| WASM Skills | 50 | 37 | 13 |
| Node Skills | 400 | 67+ | ~333 |
| Python Skills | 50 | 5 | 45 |
| Elixir API Endpoints | 10 | 7 | 3 |
| Skill Categories | 6 | 6 | 0 |

---

*Last Updated: 2026-02-26*
*Maintainer: Marco + Mim*
