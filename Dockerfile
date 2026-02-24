# =============================================================================
# Carnelian OS - Production Dockerfile
# =============================================================================
# Multi-stage build for efficient, secure production image
#
# Build:
#   docker build -t carnelian/carnelian-core:latest .
#
# Run:
#   docker run -p 18789:18789 carnelian/carnelian-core:latest
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1: Builder
# -----------------------------------------------------------------------------
FROM rust:1.80-bookworm AS builder

WORKDIR /app

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Enable SQLx offline mode for builds without database
ENV SQLX_OFFLINE=true

# Copy workspace configuration
COPY Cargo.toml Cargo.lock ./
COPY rust-toolchain.toml ./

# Copy all crate manifests for dependency caching
COPY crates/carnelian-common/Cargo.toml ./crates/carnelian-common/
COPY crates/carnelian-core/Cargo.toml ./crates/carnelian-core/
COPY crates/carnelian-adapters/Cargo.toml ./crates/carnelian-adapters/
COPY crates/carnelian-ui/Cargo.toml ./crates/carnelian-ui/
COPY crates/carnelian-worker-node/Cargo.toml ./crates/carnelian-worker-node/
COPY crates/carnelian-worker-python/Cargo.toml ./crates/carnelian-worker-python/
COPY crates/carnelian-worker-shell/Cargo.toml ./crates/carnelian-worker-shell/

# Build dependencies (cached layer)
RUN mkdir -p crates/carnelian-common/src crates/carnelian-core/src \
    crates/carnelian-adapters/src crates/carnelian-ui/src \
    crates/carnelian-worker-node/src crates/carnelian-worker-python/src \
    crates/carnelian-worker-shell/src
RUN echo 'fn main() {}' > crates/carnelian-core/src/main.rs
RUN cargo build --release --package carnelian-core 2>/dev/null || true

# Copy actual source code
COPY crates/ ./crates/
COPY db/ ./db/
COPY workers/ ./workers/
COPY skills/ ./skills/

# Copy build assets
COPY assets/ ./assets/
COPY machine.toml.example ./machine.toml

# Build the application
RUN cargo build --release --package carnelian-core

# -----------------------------------------------------------------------------
# Stage 2: Runtime
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r carnelian && useradd -r -g carnelian carnelian

# Copy binary from builder
COPY --from=builder /app/target/release/carnelian /usr/local/bin/carnelian
COPY --from=builder /app/crates/carnelian-core/src/bin/migrate /usr/local/bin/carnelian-migrate

# Copy default machine.toml
COPY --from=builder /app/machine.toml /app/machine.toml.example

# Copy skills directory structure (for runtime skill discovery)
COPY --from=builder /app/skills/ /app/skills/

# Create data directory
RUN mkdir -p /app/data && chown -R carnelian:carnelian /app

# Switch to non-root user
USER carnelian

# Expose the API port
EXPOSE 18789

# Health check
HEALTHCHECK --interval=15s --timeout=5s --start-period=30s --retries=5 \
    CMD curl -f http://localhost:18789/v1/health || exit 1

# Default command
CMD ["carnelian", "start"]
