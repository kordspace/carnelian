# SQLx Offline Query Metadata

This directory contains SQLx query metadata for compile-time verification in offline builds.

## Generating Metadata

To generate or update this metadata, you need a running PostgreSQL database with migrations applied:

```bash
# 1. Start the database
docker-compose up -d carnelian-postgres

# 2. Wait for it to be healthy
docker-compose ps

# 3. Run migrations
export DATABASE_URL="postgresql://carnelian:carnelian@localhost:5432/carnelian"
sqlx migrate run

# 4. Generate query metadata from carnelian-core crate
cd crates/carnelian-core
cargo sqlx prepare

# 5. Commit the generated files
git add .sqlx/
git commit -m "chore: update SQLx query metadata"
```

## Using Offline Mode

For CI builds without database access:

```bash
export SQLX_OFFLINE=true
cargo build
```

## Files

- `query-*.json` - Individual query metadata (auto-generated)
- This README - Documentation

## When to Regenerate

Regenerate metadata after:
- Adding new `sqlx::query!` or `sqlx::query_as!` macros
- Modifying existing queries
- Changing database schema (migrations)
