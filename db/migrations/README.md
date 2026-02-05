# Carnelian Database Migrations

SQL migration files for PostgreSQL schema management.

## Tools

Migrations are managed using `sqlx-cli`:

```bash
cargo install sqlx-cli --no-default-features --features postgres
```

## Running Migrations

```bash
sqlx migrate run --database-url postgresql://carnelian:carnelian@localhost:5432/carnelian
```
