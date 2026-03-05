# Carnelian Local Launch Guide

Quick reference for running Carnelian locally with full stack.

## Prerequisites

- Docker Desktop running
- Rust toolchain installed
- PostgreSQL client tools (optional, for debugging)

## Quick Start

### 1. Start PostgreSQL Database

```bash
docker-compose up -d carnelian-postgres
```

**Verify it's running:**
```bash
docker ps
# Should show: carnelian-postgres (healthy)
```

### 2. Run Database Migrations

```powershell
# PowerShell
$env:DATABASE_URL="postgresql://carnelian:carnelian@localhost:5432/carnelian"
cargo run -p carnelian-bin --bin carnelian -- migrate --database-url $env:DATABASE_URL
```

```bash
# Bash/Linux
export DATABASE_URL="postgresql://carnelian:carnelian@localhost:5432/carnelian"
cargo run -p carnelian-bin --bin carnelian -- migrate --database-url $DATABASE_URL
```

### 3. Start Carnelian Server

```powershell
# PowerShell
$env:DATABASE_URL="postgresql://carnelian:carnelian@localhost:5432/carnelian"
$env:CARNELIAN_HTTP_PORT="8080"
cargo run --release -p carnelian-bin --bin carnelian start
```

```bash
# Bash/Linux
export DATABASE_URL="postgresql://carnelian:carnelian@localhost:5432/carnelian"
export CARNELIAN_HTTP_PORT="8080"
cargo run --release -p carnelian-bin --bin carnelian start
```

**Server will be available at:** `http://localhost:8080`

**Health check:** `curl http://localhost:8080/v1/health`

### 4. Run Benchmarks (Optional)

```powershell
# PowerShell
$env:DATABASE_URL="postgresql://carnelian:carnelian@localhost:5432/carnelian"
cargo bench --workspace
```

**Note:** First run will take 10-15 minutes to compile all dependencies.

## Full Docker Compose Stack

To run the complete stack (PostgreSQL + Ollama + Gateway + Core):

```bash
# Start all services
docker-compose up -d

# View logs
docker-compose logs -f

# Check status
docker-compose ps

# Stop all services
docker-compose down

# Stop and remove volumes (data loss!)
docker-compose down -v
```

## Database Management

### Reset Database

```bash
# Drop and recreate
docker exec carnelian-postgres psql -U carnelian -d postgres -c "DROP DATABASE IF EXISTS carnelian;"
docker exec carnelian-postgres psql -U carnelian -d postgres -c "CREATE DATABASE carnelian;"

# Run migrations
cargo run -p carnelian-bin --bin carnelian -- migrate --database-url postgresql://carnelian:carnelian@localhost:5432/carnelian
```

### Check Migration Status

```bash
docker exec carnelian-postgres psql -U carnelian -d carnelian -c "SELECT * FROM _sqlx_migrations ORDER BY version;"
```

### Access Database Shell

```bash
docker exec -it carnelian-postgres psql -U carnelian -d carnelian
```

## Testing

### Run All Tests

```bash
# Unit tests (no database required)
cargo test --workspace

# Integration tests (requires database)
docker-compose up -d carnelian-postgres
export DATABASE_URL="postgresql://carnelian:carnelian@localhost:5432/carnelian"
cargo test --workspace -- --ignored
```

### Run Specific Test Suite

```bash
# Native ops tests
cargo test --test native_ops_tests

# Server integration tests
cargo test --test server_integration_test -- --ignored
```

## Building Docker Images

### AMD64 (Standard)

```bash
docker build -t carnelian:latest .
```

### ARM64 (Cross-platform)

```bash
# Option 1: Native build (slow on non-ARM hardware)
docker build --platform linux/arm64 -t carnelian:arm64 .

# Option 2: Cross-compilation (faster)
rustup target add aarch64-unknown-linux-gnu
cargo install cargo-zigbuild
cargo zigbuild --release --target aarch64-unknown-linux-gnu --bin carnelian
```

## Troubleshooting

### Port Already in Use

```bash
# Find process using port 8080
netstat -ano | findstr :8080  # Windows
lsof -i :8080                 # Linux/Mac

# Kill the process
taskkill /PID <PID> /F        # Windows
kill -9 <PID>                 # Linux/Mac
```

### Database Connection Refused

```bash
# Check if PostgreSQL is running
docker ps | grep carnelian-postgres

# Check PostgreSQL logs
docker logs carnelian-postgres

# Restart PostgreSQL
docker-compose restart carnelian-postgres
```

### Migration Checksum Mismatch

This happens when migration files are modified after being applied.

**Solution:** Reset the database (see "Reset Database" above)

### Compilation Taking Too Long

First-time compilation can take 10-15 minutes. Subsequent builds are much faster due to caching.

**Speed up compilation:**
```bash
# Use more CPU cores
export CARGO_BUILD_JOBS=8

# Use sccache (if installed)
export RUSTC_WRAPPER=sccache
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | - | PostgreSQL connection string (required) |
| `CARNELIAN_HTTP_PORT` | `18789` | HTTP API port |
| `CARNELIAN_BIND_ADDRESS` | `0.0.0.0` | Bind address |
| `CARNELIAN_ENV` | `development` | Environment (development/production) |
| `LOG_LEVEL` | `INFO` | Logging level (TRACE/DEBUG/INFO/WARN/ERROR) |
| `CARNELIAN_OLLAMA_URL` | `http://localhost:11434` | Ollama API endpoint |
| `SQLX_OFFLINE` | `false` | Use offline mode for sqlx (CI only) |

## Performance Tips

### Development Builds

```bash
# Faster compilation, slower runtime
cargo build
cargo run -p carnelian-bin --bin carnelian start
```

### Release Builds

```bash
# Slower compilation, faster runtime
cargo build --release
cargo run --release -p carnelian-bin --bin carnelian start
```

### Incremental Compilation

Already enabled by default. To disable:
```bash
export CARGO_INCREMENTAL=0
```

## See Also

- [TESTING.md](TESTING.md) - Comprehensive testing guide
- [docker-compose.yml](../docker-compose.yml) - Full stack configuration
- [README.md](../README.md) - Project overview
