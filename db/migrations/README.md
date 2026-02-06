# 🔥 Carnelian Database Migrations

SQL migration files for PostgreSQL schema management using SQLx.

## Prerequisites

- PostgreSQL 15+ with pgvector extension
- SQLx CLI tool

## Installation

Install the SQLx CLI:

```bash
cargo install sqlx-cli --no-default-features --features postgres
```

## Creating New Migrations

```bash
# Create a new migration file
sqlx migrate add <migration_name>

# Example: Add a new table
sqlx migrate add add_sessions_table
```

This creates a new timestamped file in `db/migrations/`.

## Running Migrations

```bash
# Set the database URL
export DATABASE_URL="postgresql://carnelian:carnelian@localhost:5432/carnelian"

# Run all pending migrations
sqlx migrate run

# Or specify the URL directly
sqlx migrate run --database-url postgresql://carnelian:carnelian@localhost:5432/carnelian
```

## Reverting Migrations

SQLx doesn't support automatic rollback. To revert:

1. Create a new migration with the reverse operations
2. Or manually execute SQL to undo changes

## Compile-Time Query Verification

SQLx verifies queries at compile time. For offline builds (CI/CD):

```bash
# Install sqlx-cli if not already installed
cargo install sqlx-cli --no-default-features --features postgres

# Generate query metadata for offline compilation
cargo sqlx prepare --workspace

# This creates .sqlx/ directory in each crate - commit to version control
git add crates/carnelian-core/.sqlx/
```

For CI builds without database access:

```bash
export SQLX_OFFLINE=true
cargo build
```

### Migration Path Resolution

The `sqlx::migrate!` macro resolves paths **relative to the crate's `Cargo.toml`**, not the source file. For `carnelian-core`:

- Crate location: `crates/carnelian-core/`
- Migration path: `../../db/migrations` → resolves to `db/migrations/`

To verify the path is correct:
```bash
# From crates/carnelian-core/, this should list migration files:
ls ../../db/migrations/
```

## Core Schema

The `00000000000001_core_schema.sql` migration creates the foundational tables:

| Table | Purpose |
|-------|---------|
| `identities` | Core agent (🦎 Lian) and sub-agent identities |
| `capabilities` | Capability types for security model |
| `capability_grants` | Capability assignments to subjects |
| `skills` | Skill catalog for worker execution |
| `tasks` | Work queue for orchestrator |
| `task_runs` | Execution attempts for tasks |
| `run_logs` | Structured logs for task runs |
| `ledger_events` | Tamper-resistant audit log with **blake3** hash chain |
| `memories` | Memory storage with vector embeddings (pgvector) |
| `model_providers` | LLM provider configurations |
| `usage_costs` | Token usage and cost tracking |
| `config_store` | Key-value configuration storage |
| `config_versions` | Configuration change history |
| `heartbeat_history` | Heartbeat tracking for wake routine |

### Ledger Hash-Chain Architecture

The ledger uses **blake3** for cryptographic hashing (not SHA-256). Each event contains:

- `payload_hash` — blake3 of the canonical JSON payload
- `prev_hash` — previous event's `event_hash`, linking the chain
- `event_hash` — blake3 of all fields (timestamp, actor, action, payload_hash, prev_hash)

Verification: `Ledger::verify_chain()` replays the entire chain and recomputes hashes. See `crates/carnelian-core/src/ledger.rs` for implementation details.

Note: The ledger and policy engine shipped in Phase 1 (originally planned for Phase 4).

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection string | Required |
| `SQLX_OFFLINE` | Enable offline mode for builds | `false` |

## Troubleshooting

### pgvector extension not found

Ensure pgvector is installed in your PostgreSQL instance:

```sql
CREATE EXTENSION IF NOT EXISTS vector;
```

### Migration already applied

Check migration status:

```bash
sqlx migrate info
```

### Connection refused

Ensure PostgreSQL is running:

```bash
docker-compose ps
docker-compose up -d carnelian-postgres
```

## Seed Data

The core schema migration creates the following seed data:

### Default Identity (🦎 Lian)

| Field | Value |
|-------|-------|
| `name` | `Lian` |
| `pronouns` | `she/her` |
| `identity_type` | `core` |
| `soul_file_path` | `souls/lian.md` (placeholder) |
| `directives` | Assist Marco, maintain system integrity, learn and adapt |

### Default Capabilities

| Capability Key | Description | Requirement Mapping |
|----------------|-------------|---------------------|
| `fs.read` | Read files from filesystem | filesystem access |
| `fs.write` | Write files to filesystem | filesystem access |
| `fs.delete` | Delete files from filesystem | filesystem access |
| `net.http` | Make HTTP requests | network access |
| `net.websocket` | Establish WebSocket connections | network access |
| `process.spawn` | Spawn child processes | exec.shell equivalent |
| `process.kill` | Kill running processes | process management |
| `db.read` | Read from database | database access |
| `db.write` | Write to database | database access |
| `model.inference` | Request model inference | model.local + model.remote |
| `skill.execute` | Execute skills | skill execution |
| `task.create` | Create new tasks | task management |
| `task.cancel` | Cancel running tasks | task management |
| `config.read` | Read configuration | configuration access |
| `config.write` | Write configuration | configuration access |
| `ledger.read` | Read audit ledger | ledger access |
| `ledger.write` | Write to audit ledger | ledger access |

### Default Model Provider (Ollama)

| Field | Value |
|-------|-------|
| `provider_type` | `local` |
| `name` | `ollama` |
| `config.base_url` | `http://localhost:11434` |
| `config.default_model` | `deepseek-r1:7b` |

## Using Carnelian CLI

The `carnelian` CLI provides migration commands:

```bash
# Run all pending migrations
carnelian migrate

# Show pending migrations without applying (dry-run)
carnelian migrate --dry-run

# Use a specific database URL
carnelian migrate --database-url postgresql://user:pass@host/db

# With custom config and log level
carnelian migrate --config custom.toml --log-level DEBUG
```
