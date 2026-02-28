# Local Testing Guide

This guide explains how to run CARNELIAN tests locally with full database support.

## Quick Start - Local Testing with Database

### 1. Start PostgreSQL with Docker Compose

```bash
# Start just the PostgreSQL database
docker-compose up -d carnelian-postgres

# Verify it's running
docker-compose ps
docker-compose logs carnelian-postgres
```

The database will be available at:
- **Connection String**: `postgresql://carnelian:carnelian@localhost:5432/carnelian`
- **Host**: `localhost`
- **Port**: `5432`
- **User**: `carnelian`
- **Password**: `carnelian`
- **Database**: `carnelian`

### 2. Run Database Migrations

```bash
# Set the database URL
export DATABASE_URL="postgresql://carnelian:carnelian@localhost:5432/carnelian"

# Run migrations
cargo run --bin carnelian -- migrate --database-url $DATABASE_URL
```

### 3. Run Tests with Database

```bash
# Run all tests (unit + integration with database)
cargo test --workspace

# Run specific integration tests that require database
cargo test --test integration_test -- --ignored

# Run all ignored tests (requires Docker + database)
cargo test --workspace -- --ignored

# Run specific test suites
cargo test --test intelligence_integration_test -- --ignored
cargo test --test migration_test -- --ignored
cargo test --test encryption_tests -- --ignored
```

### 4. Run Benchmarks

```bash
# Benchmarks require database connection
cargo bench --bench memory_benchmarks
```

### 5. Run Load Tests

```bash
# Terminal 1: Start the database
docker-compose up -d carnelian-postgres

# Terminal 2: Run migrations and start server
export DATABASE_URL="postgresql://carnelian:carnelian@localhost:5432/carnelian"
cargo run --release --bin carnelian -- migrate --database-url $DATABASE_URL
cargo run --release --bin carnelian start

# Terminal 3: Run load tests (requires k6)
cd tests/performance
k6 run load_test.js
```

## Full Stack Testing (Database + Ollama)

```bash
# Start all services
docker-compose up -d

# Check status
docker-compose ps
docker-compose logs -f

# Pull a model for Ollama
docker exec carnelian-ollama ollama pull deepseek-r1:7b

# Run full integration tests
export DATABASE_URL="postgresql://carnelian:carnelian@localhost:5432/carnelian"
export OLLAMA_URL="http://localhost:11434"
cargo test --workspace -- --ignored
```

## Cleanup

```bash
# Stop services (preserve data)
docker-compose down

# Stop and remove volumes (data loss!)
docker-compose down -v

# Remove just the database container
docker-compose rm -sf carnelian-postgres
```

## CI/CD Test Categories

### Unit Tests (No Database Required)
- Run with: `cargo test --workspace`
- **262 passing tests** in `carnelian-core`
- **11 passing tests** in `carnelian-adapters`
- **5 passing tests** in `carnelian-bin`
- Total: **~280 unit tests**

### Integration Tests (Database Required - Marked as `#[ignore]`)
- Run with: `cargo test --workspace -- --ignored`
- Require PostgreSQL with pgvector extension
- Include:
  - `integration_test.rs` - Server startup, WebSocket, event stream
  - `intelligence_integration_test.rs` - Agentic loop, context assembly, memory
  - `migration_test.rs` - Database schema migrations
  - `encryption_tests.rs` - Database encryption with pgcrypto
  - `approval_api_tests.rs` - Approval workflow with signatures
  - `attestation_tests.rs` - Worker attestation and quarantine
  - And more...

### Benchmarks (Database Required)
- Run with: `cargo bench`
- `memory_benchmarks.rs` - Memory manager performance
- Require database pool for realistic performance testing

### Load Tests (Full Stack Required)
- Run with: `k6 run tests/performance/load_test.js`
- Require: PostgreSQL + running CARNELIAN server
- Test duration: ~6 minutes
- Stages: 10 → 50 → 100 concurrent users

## Troubleshooting

### Database Connection Errors

```bash
# Check if PostgreSQL is running
docker-compose ps carnelian-postgres

# Check PostgreSQL logs
docker-compose logs carnelian-postgres

# Restart PostgreSQL
docker-compose restart carnelian-postgres

# Verify connection manually
psql postgresql://carnelian:carnelian@localhost:5432/carnelian -c "SELECT version();"
```

### Port Conflicts

If port 5432 is already in use:

```bash
# Option 1: Stop conflicting service
sudo systemctl stop postgresql

# Option 2: Change port in docker-compose.yml
# Edit: ports: - "5433:5432"
# Then use: postgresql://carnelian:carnelian@localhost:5433/carnelian
```

### SQLx Offline Mode

For CI builds without database:

```bash
# Generate query cache (requires running database)
export DATABASE_URL="postgresql://carnelian:carnelian@localhost:5432/carnelian"
cargo sqlx prepare --workspace

# Build with offline mode
SQLX_OFFLINE=true cargo build --release
```

## Performance Test Timeout Issue

The load test in CI is timing out due to:
1. **Build time**: ~25 minutes for release build
2. **Test duration**: 6 minutes (30s + 1m + 2m + 1m + 1m + 30s stages)
3. **Total**: ~31 minutes > 30 minute timeout

**Solutions**:
- Increase timeout to 45 minutes in `.github/workflows/performance.yml`
- Use cached builds to reduce build time
- Run load tests separately from benchmarks
- Reduce test stages for CI (keep full test for local)

## Local vs CI Testing

| Test Type | Local | CI |
|-----------|-------|-----|
| Unit tests | ✅ Fast | ✅ Fast |
| Integration tests (ignored) | ✅ With docker-compose | ❌ Skipped |
| Integration tests (database) | ✅ With docker-compose | ✅ GitHub Services |
| Benchmarks | ✅ With docker-compose | ⚠️ Timeout issues |
| Load tests | ✅ With docker-compose | ⚠️ Timeout issues |
| Docker build | ✅ Local daemon | ✅ GitHub Actions |

## Recommended Local Workflow

```bash
# 1. Start database once
docker-compose up -d carnelian-postgres

# 2. Run migrations once
export DATABASE_URL="postgresql://carnelian:carnelian@localhost:5432/carnelian"
cargo run --bin carnelian -- migrate --database-url $DATABASE_URL

# 3. Develop and test iteratively
cargo test                           # Fast unit tests
cargo test --test integration_test  # Specific integration test
cargo clippy --workspace --all-targets -- -D warnings

# 4. Before committing, run full test suite
cargo test --workspace -- --ignored  # All tests including database

# 5. Cleanup when done
docker-compose down
```
