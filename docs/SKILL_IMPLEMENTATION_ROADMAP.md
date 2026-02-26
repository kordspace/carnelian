# CARNELIAN Skill Implementation Roadmap

**Last Updated**: 2026-02-26  
**Status**: Phase 10 - Skill Library Expansion

---

## Executive Summary

**Progress**: Created 85 new skill manifests, expanding registry from 50 to 135+ skills.

**Current State**:
- ✅ **135+ skill manifests** created and ready for implementation
- ✅ **50+ skills** fully implemented and tested
- ✅ **All 6 skill categories** represented
- 🔄 **85 skills** awaiting implementation (manifests complete)
- 📊 **~465 skills** remaining to reach 600-skill target

---

## Skill Breakdown by Runtime

### Node Skills (67 total, 47 with manifests)

#### Communication & Messaging (20 skills)
**Status**: 10 implemented, 10 manifests created
- ✅ `discord-send` - Send Discord messages
- 🆕 `discord-guild` - Manage Discord guilds/servers
- 🆕 `discord-channel` - Manage Discord channels
- 🆕 `discord-role` - Manage Discord roles
- 🆕 `discord-moderate` - Discord moderation actions
- ✅ `slack-send` - Send Slack messages
- 🆕 `slack-channel` - Manage Slack channels
- 🆕 `slack-file` - Upload files to Slack
- 🆕 `slack-react` - Add reactions to Slack messages
- ✅ `telegram-send` - Send Telegram messages
- 🆕 `telegram-group` - Manage Telegram groups
- 🆕 `telegram-media` - Send media to Telegram
- ✅ `whatsapp-send` - Send WhatsApp messages
- ✅ `message-send` - Multi-channel message sending
- ✅ `image-generate` - Generate images via AI
- ✅ `image-analyze` - Analyze images via AI
- ✅ `text-to-speech` - Convert text to speech
- ✅ `canvas-render` - Render canvas graphics
- ✅ `memory-write` - Write to memory system
- ✅ `memory-read` - Read from memory system

#### Browser Automation (6 skills)
**Status**: 1 implemented, 5 manifests created
- ✅ `browser-automation` - Full browser control
- 🆕 `browser-navigate` - Navigate to URLs
- 🆕 `browser-screenshot` - Capture screenshots
- 🆕 `browser-click` - Click elements
- 🆕 `browser-type` - Type into inputs
- 🆕 `browser-pdf` - Generate PDFs from pages

#### Session Management (4 skills)
**Status**: 1 implemented, 3 manifests created
- ✅ `session-spawn` - Spawn new sessions
- 🆕 `session-list` - List active sessions
- 🆕 `session-history` - Get session history
- 🆕 `session-status` - Get session status

#### Cron & Scheduling (5 skills)
**Status**: 1 implemented, 4 manifests created
- ✅ `cron-schedule` - Schedule cron jobs
- 🆕 `cron-list` - List cron jobs
- 🆕 `cron-add` - Add cron job
- 🆕 `cron-remove` - Remove cron job
- 🆕 `cron-run` - Trigger cron job immediately

#### Gateway & Infrastructure (4 skills)
**Status**: 1 implemented, 3 manifests created
- ✅ `gateway-query` - Query gateway status
- 🆕 `gateway-config` - Manage gateway config
- 🆕 `gateway-restart` - Restart gateway
- 🆕 `gateway-update` - Update gateway

#### Agent Orchestration (3 skills)
**Status**: 2 implemented, 1 manifest created
- ✅ `agent-step` - Execute agent step
- ✅ `cascade-run` - Run cascade workflow
- 🆕 `agents-list` - List available agents

#### Web & Network (5 skills)
**Status**: 2 implemented, 3 manifests created
- ✅ `web-search` - Search the web
- ✅ `web-fetch` - Fetch web pages
- 🆕 `http-request` - Make HTTP requests
- 🆕 `http-webhook` - Send webhooks
- 🆕 `graphql-query` - Execute GraphQL queries

#### Document Processing (6 skills)
**Status**: 0 implemented, 6 manifests created
- 🆕 `pdf-extract-text` - Extract text from PDFs
- 🆕 `pdf-metadata` - Get PDF metadata
- 🆕 `email-parse` - Parse email messages
- 🆕 `email-send` - Send emails via SMTP
- 🆕 `template-render` - Render templates
- 🆕 `currency-convert` - Convert currencies

#### Code Analysis (2 skills)
**Status**: 0 implemented, 2 manifests created
- 🆕 `code-lint-js` - Lint JavaScript/TypeScript
- 🆕 `code-deps-js` - Analyze JS dependencies

#### Data & Validation (3 skills)
**Status**: 0 implemented, 3 manifests created
- 🆕 `data-validate` - Validate against JSON Schema
- 🆕 `schema-generate` - Generate JSON Schema
- ✅ `nodes-list` - List available nodes

#### System (4 skills)
**Status**: 2 implemented, 2 manifests created
- ✅ `system-healthcheck` - System health check
- ✅ `model-usage` - Track model usage

---

### Native Ops (26 total, all with manifests)

#### File Operations (7 skills)
**Status**: 5 implemented, 2 manifests created
- ✅ `file-hash` - Hash file contents
- ✅ `file-write` - Write files
- ✅ `file-delete` - Delete files
- ✅ `file-move` - Move/rename files
- ✅ `file-search` - Search files
- 🆕 `file-watch` - Watch files for changes
- 🆕 `file-metadata` - Extract file metadata

#### Git Operations (5 skills)
**Status**: 1 implemented, 4 manifests created
- ✅ `git-status` - Get git status
- 🆕 `git-diff` - Show git diff
- 🆕 `git-commit` - Commit changes
- 🆕 `git-log` - Show git log
- 🆕 `git-branch` - Manage branches

#### Docker Operations (4 skills)
**Status**: 1 implemented, 3 manifests created
- ✅ `docker-ps` - List containers
- 🆕 `docker-exec` - Execute in container
- 🆕 `docker-logs` - Get container logs
- 🆕 `docker-stats` - Get container stats

#### System Operations (4 skills)
**Status**: 2 implemented, 2 manifests created
- ✅ `process-list` - List processes
- ✅ `disk-usage` - Get disk usage
- 🆕 `network-stats` - Get network stats
- ✅ `env-get` - Get environment variables

#### Database Operations (2 skills)
**Status**: 0 implemented, 2 manifests created
- 🆕 `sql-query` - Execute SQL queries
- 🆕 `sql-schema` - Introspect database schema

#### Archive Operations (2 skills)
**Status**: 0 implemented, 2 manifests created
- 🆕 `archive-zip` - Create/extract ZIP archives
- 🆕 `archive-tar` - Create/extract TAR archives

#### Utilities (2 skills)
**Status**: 2 implemented
- ✅ `system-healthcheck` - System health check
- ✅ `echo` - Echo input

---

### WASM Skills (37 total, all with manifests)

#### Data Format Parsing (11 skills)
**Status**: 2 implemented, 9 manifests created
- ✅ `markdown-parse` - Parse Markdown
- ✅ `yaml-parse` - Parse YAML
- ✅ `json-transform` - Transform JSON
- 🆕 `csv-parse` - Parse CSV
- 🆕 `csv-generate` - Generate CSV
- 🆕 `xml-parse` - Parse XML
- 🆕 `xml-generate` - Generate XML
- 🆕 `toml-parse` - Parse TOML
- 🆕 `toml-generate` - Generate TOML
- 🆕 `ini-parse` - Parse INI
- 🆕 `env-parse` - Parse .env files

#### Cryptography (5 skills)
**Status**: 0 implemented, 5 manifests created
- 🆕 `crypto-encrypt` - Encrypt data (AES-256-GCM)
- 🆕 `crypto-decrypt` - Decrypt data
- 🆕 `crypto-sign` - Create digital signatures
- 🆕 `crypto-verify` - Verify signatures
- 🆕 `crypto-keygen` - Generate key pairs

#### Text Processing (6 skills)
**Status**: 2 implemented, 4 manifests created
- ✅ `text-search` - Search text with patterns
- ✅ `hash-file` - Hash file contents
- 🆕 `regex-match` - Match regex patterns
- 🆕 `regex-replace` - Replace with regex
- 🆕 `diff-text` - Generate text diffs
- 🆕 `patch-apply` - Apply diff patches

#### Encoding & Formatting (4 skills)
**Status**: 1 implemented, 3 manifests created
- ✅ `code-format` - Format code
- 🆕 `base64-encode` - Encode to Base64
- 🆕 `base64-decode` - Decode Base64
- 🆕 `code-ast-js` - Parse JS/TS to AST

#### URL & Web (2 skills)
**Status**: 0 implemented, 2 manifests created
- 🆕 `url-parse` - Parse URLs
- 🆕 `url-build` - Build URLs

#### Image Processing (3 skills)
**Status**: 0 implemented, 3 manifests created
- 🆕 `image-resize` - Resize images
- 🆕 `image-convert` - Convert image formats
- 🆕 `image-metadata` - Extract image metadata

#### QR Codes (2 skills)
**Status**: 0 implemented, 2 manifests created
- 🆕 `qr-generate` - Generate QR codes
- 🆕 `qr-decode` - Decode QR codes

#### Date & Time (2 skills)
**Status**: 0 implemented, 2 manifests created
- 🆕 `datetime-parse` - Parse datetime strings
- 🆕 `datetime-format` - Format datetime values

#### Utilities (5 skills)
**Status**: 1 implemented, 4 manifests created
- ✅ `hello-wasm` - Hello world example
- 🆕 `uuid-generate` - Generate UUIDs
- 🆕 `slug-generate` - Generate URL slugs
- 🆕 `color-convert` - Convert color formats
- 🆕 `units-convert` - Convert units
- 🆕 `math-calculate` - Evaluate math expressions

---

### Python Skills (5 total, all with manifests)

#### Code Analysis (2 skills)
**Status**: 0 implemented, 2 manifests created
- 🆕 `code-ast-python` - Parse Python to AST
- 🆕 `code-lint-python` - Lint Python code
- 🆕 `code-deps-python` - Analyze Python dependencies

#### Data Science (2 skills)
**Status**: 0 implemented, 2 manifests created
- 🆕 `stats-analyze` - Statistical analysis
- 🆕 `chart-generate` - Generate charts/graphs

---

## Implementation Priority Queue

### Phase 10A: High-Impact Node Skills (Week 1-2)
**Target**: 15 skills implemented

1. **Discord Extended** (4 skills)
   - `discord-guild`, `discord-channel`, `discord-role`, `discord-moderate`
   - **Rationale**: Complete Discord integration suite
   - **Effort**: Medium (reuse THUMMIM implementations)

2. **Browser Automation Extended** (5 skills)
   - `browser-navigate`, `browser-screenshot`, `browser-click`, `browser-type`, `browser-pdf`
   - **Rationale**: Complete browser control capabilities
   - **Effort**: Medium (Playwright wrappers)

3. **Session Management** (3 skills)
   - `session-list`, `session-history`, `session-status`
   - **Rationale**: Essential for multi-session workflows
   - **Effort**: Low (gateway API wrappers)

4. **Gateway Control** (3 skills)
   - `gateway-config`, `gateway-restart`, `gateway-update`
   - **Rationale**: Self-management capabilities
   - **Effort**: Low (gateway API wrappers)

### Phase 10B: Essential WASM Skills (Week 3-4)
**Target**: 20 skills implemented

1. **Data Format Processing** (8 skills)
   - `csv-parse`, `csv-generate`, `xml-parse`, `xml-generate`
   - `toml-parse`, `toml-generate`, `ini-parse`, `env-parse`
   - **Rationale**: Universal data interchange
   - **Effort**: Medium (Rust crates available)

2. **Cryptography Suite** (5 skills)
   - `crypto-encrypt`, `crypto-decrypt`, `crypto-sign`, `crypto-verify`, `crypto-keygen`
   - **Rationale**: Security primitives
   - **Effort**: Medium (ring/RustCrypto crates)

3. **Text Utilities** (4 skills)
   - `regex-match`, `regex-replace`, `diff-text`, `patch-apply`
   - **Rationale**: Common text operations
   - **Effort**: Low (regex crate, similar crate)

4. **Encoding** (3 skills)
   - `base64-encode`, `base64-decode`, `url-parse`, `url-build`
   - **Rationale**: Web/API essentials
   - **Effort**: Low (standard Rust)

### Phase 10C: Native Ops Expansion (Week 5-6)
**Target**: 10 skills implemented

1. **Git Extended** (4 skills)
   - `git-diff`, `git-commit`, `git-log`, `git-branch`
   - **Rationale**: Complete git workflow
   - **Effort**: Medium (git2-rs crate)

2. **Database Operations** (2 skills)
   - `sql-query`, `sql-schema`
   - **Rationale**: Direct DB access
   - **Effort**: Medium (sqlx already integrated)

3. **Archive Operations** (2 skills)
   - `archive-zip`, `archive-tar`
   - **Rationale**: File compression/extraction
   - **Effort**: Medium (zip/tar crates)

4. **File Extended** (2 skills)
   - `file-watch`, `file-metadata`
   - **Rationale**: Advanced file operations
   - **Effort**: Low (notify crate, std::fs)

### Phase 10D: Python Skills Foundation (Week 7-8)
**Target**: 5 skills implemented

1. **Code Analysis** (3 skills)
   - `code-ast-python`, `code-lint-python`, `code-deps-python`
   - **Rationale**: Python ecosystem support
   - **Effort**: Medium (ast, pylint, pipdeptree)

2. **Data Science** (2 skills)
   - `stats-analyze`, `chart-generate`
   - **Rationale**: Analytics capabilities
   - **Effort**: Medium (numpy, matplotlib)

---

## Next Steps (Immediate)

### 1. Implement Discord Extended Skills
**Files to create**:
- `workers/node-worker/skills/discord-guild.ts`
- `workers/node-worker/skills/discord-channel.ts`
- `workers/node-worker/skills/discord-role.ts`
- `workers/node-worker/skills/discord-moderate.ts`

**Reference**: `THUMMIM/thummim/src/agents/tools/discord-actions-*.ts`

### 2. Implement Browser Extended Skills
**Files to create**:
- `workers/node-worker/skills/browser-navigate.ts`
- `workers/node-worker/skills/browser-screenshot.ts`
- `workers/node-worker/skills/browser-click.ts`
- `workers/node-worker/skills/browser-type.ts`
- `workers/node-worker/skills/browser-pdf.ts`

**Reference**: `THUMMIM/thummim/src/agents/tools/browser-tool.ts`

### 3. Implement CSV Processing (WASM)
**Files to create**:
- `skills/registry/csv-parse/src/main.rs`
- `skills/registry/csv-parse/Cargo.toml`
- `skills/registry/csv-generate/src/main.rs`
- `skills/registry/csv-generate/Cargo.toml`

**Dependencies**: `csv = "1.3"`, `serde_json = "1.0"`

### 4. Update Build Scripts
- Add new WASM skills to `scripts/build-wasm-skills.sh`
- Update skill registry loader to recognize new manifests

---

## Remaining Skill Categories (465 skills)

### High-Priority Additions (Next 100 skills)

1. **Cloud Operations** (30 skills)
   - AWS: S3, EC2, Lambda, DynamoDB, CloudWatch
   - Docker: Compose, Swarm, Registry
   - Kubernetes: Pods, Services, Deployments, ConfigMaps

2. **API Integrations** (25 skills)
   - GitHub, GitLab, Bitbucket
   - Jira, Linear, Asana
   - Stripe, PayPal
   - Twilio, SendGrid

3. **Database Drivers** (15 skills)
   - PostgreSQL, MySQL, SQLite
   - MongoDB, Redis, Elasticsearch
   - ClickHouse, TimescaleDB

4. **Testing & QA** (10 skills)
   - Unit test generation
   - Integration test runners
   - Load testing
   - Security scanning

5. **Monitoring & Logging** (10 skills)
   - Prometheus metrics
   - Grafana dashboards
   - Log aggregation
   - Trace analysis

6. **ML/AI Operations** (10 skills)
   - Model inference (ONNX)
   - Dataset preprocessing
   - Feature engineering
   - Model evaluation

---

## Success Metrics

- ✅ **135+ skill manifests** created (Target: 150 by end of Phase 10)
- 🔄 **50+ skills** fully implemented (Target: 100 by end of Phase 10)
- 📊 **~465 skills** remaining (Target: <400 by end of Phase 10)
- ✅ **All 6 categories** represented
- 🎯 **600-skill library** target by Q2 2026

---

*This roadmap is a living document. Update as skills are implemented and priorities shift.*
