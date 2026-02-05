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
# Generate query metadata for offline compilation
cargo sqlx prepare

# This creates .sqlx/ directory - commit it to version control
git add .sqlx/
```

For CI builds without database access:

```bash
export SQLX_OFFLINE=true
cargo build
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
| `ledger_events` | Tamper-resistant audit log with hash chain |
| `memories` | Memory storage with vector embeddings (pgvector) |
| `model_providers` | LLM provider configurations |
| `usage_costs` | Token usage and cost tracking |
| `config_store` | Key-value configuration storage |
| `config_versions` | Configuration change history |
| `heartbeat_history` | Heartbeat tracking for wake routine |

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
