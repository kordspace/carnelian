# =============================================================================
# Carnelian LLM Gateway - Multi-stage Docker Build
# =============================================================================
# Builds the TypeScript gateway service for unified LLM completion API
# with local-first routing and remote fallback.
#
# Build: docker build -t carnelian/gateway:latest -f packages/gateway/Dockerfile .
# Run:   docker run -p 18790:18790 carnelian/gateway:latest
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1: Build TypeScript
# -----------------------------------------------------------------------------
FROM node:22-alpine AS builder

WORKDIR /build

# Copy package files
COPY packages/gateway/package.json packages/gateway/package-lock.json* ./

# Install ALL dependencies (including dev deps for TypeScript build)
RUN npm ci && \
    npm cache clean --force

# Copy source code
COPY packages/gateway/tsconfig.json ./
COPY packages/gateway/src ./src

# Build TypeScript
RUN npm run build

# -----------------------------------------------------------------------------
# Stage 2: Production Runtime
# -----------------------------------------------------------------------------
FROM node:22-alpine

WORKDIR /app

# Install curl for health checks
RUN apk add --no-cache curl

# Copy package files
COPY packages/gateway/package.json packages/gateway/package-lock.json* ./

# Install production dependencies only
RUN npm ci --only=production && \
    npm cache clean --force

# Copy built JavaScript from builder
COPY --from=builder /build/dist ./dist

# Create non-root user
RUN addgroup -g 1001 -S carnelian && \
    adduser -u 1001 -S carnelian -G carnelian && \
    chown -R carnelian:carnelian /app

USER carnelian

# Expose gateway port
EXPOSE 18790

# Health check
HEALTHCHECK --interval=15s --timeout=5s --retries=3 --start-period=10s \
  CMD curl -f http://localhost:18790/health || exit 1

# Start gateway
CMD ["node", "dist/index.js"]
