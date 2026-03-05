# Carnelian OS - Docker Ecosystem Guide

## Overview

This document describes the complete Docker ecosystem for Carnelian OS, including all services, their configurations, and how they interact.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Docker Host                                        │
│                                                                              │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────────────┐ │
│  │  carnelian-ui   │    │  carnelian-core │    │    carnelian-postgres   │ │
│  │   (Desktop App) │◄──►│   (Orchestrator)│◄──►│  (PostgreSQL+pgvector)  │ │
│  │                 │    │                 │    │                         │ │
│  │  Port: N/A      │    │  Port: 18789    │    │  Port: 5432              │ │
│  └─────────────────┘    └────────┬────────┘    └─────────────────────────┘ │
│                                  │                                          │
│                                  ▼                                          │
│                       ┌─────────────────────────┐                            │
│                       │    carnelian-ollama   │                            │
│                       │   (LLM Runtime GPU)   │                            │
│                       │                         │                            │
│                       │      Port: 11434        │                            │
│                       └─────────────────────────┘                            │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Services

### 1. carnelian-postgres
**Purpose:** PostgreSQL database with pgvector extension for vector embeddings

**Image:** `pgvector/pgvector:pg16`

**Ports:**
- `5432:5432` (PostgreSQL)

**Environment:**
- `POSTGRES_USER=carnelian`
- `POSTGRES_PASSWORD=carnelian`
- `POSTGRES_DB=carnelian`
- `PGVECTOR_DIMS=1536`

**Volumes:**
- `postgres-data` - Persistent database storage

**Health Check:**
```bash
pg_isready -U carnelian
```

### 2. carnelian-ollama
**Purpose:** Ollama runtime for local LLM inference with NVIDIA GPU support

**Image:** `ollama/ollama:latest`

**Ports:**
- `11434:11434` (Ollama API)

**Environment:**
- `OLLAMA_HOST=0.0.0.0:11434`
- `NVIDIA_VISIBLE_DEVICES=all`
- `OLLAMA_KEEP_ALIVE=-1` (Keep models loaded)
- `OLLAMA_NUM_GPU=999` (Use all GPU layers)

**Volumes:**
- `ollama-models` - Downloaded model cache

**GPU Requirements:**
- NVIDIA GPU with CUDA support
- NVIDIA Container Toolkit
- Docker configured for GPU access

**Health Check:**
```bash
ollama ps
```

### 3. carnelian-core
**Purpose:** Main orchestrator - API server, skill execution, scheduling

**Image:** `carnelian/carnelian-core:latest` (built from Dockerfile)

**Ports:**
- `18789:18789` (HTTP API + WebSocket)

**Environment:**
- `DATABASE_URL` - Connection to postgres
- `CARNELIAN_OLLAMA_URL` - Connection to ollama
- `CARNELIAN_ENV=production`
- `LOG_LEVEL=INFO`
- `OPENAI_API_KEY` - Optional, for OpenAI provider
- `ANTHROPIC_API_KEY` - Optional, for Anthropic provider
- `FIREWORKS_API_KEY` - Optional, for Fireworks provider

**Volumes:**
- `./machine.toml:/app/machine.toml:ro` - Machine configuration
- `carnelian-data:/app/data` - Application data
- `~/.carnelian:/root/.carnelian:ro` - Carnelian home directory

**Dependencies:**
- `carnelian-postgres` (must be healthy)
- `carnelian-ollama` (must be healthy)

**Health Check:**
```bash
curl -f http://localhost:18789/v1/health
```

## Quick Start

### Prerequisites

1. **Docker & Docker Compose**
   ```bash
   docker --version
   docker-compose --version
   ```

2. **NVIDIA Container Toolkit** (for GPU support)
   ```bash
   # Install on Ubuntu/Debian
   sudo apt-get install nvidia-container-toolkit
   sudo systemctl restart docker
   ```

3. **Build the carnelian-core image**
   ```bash
   docker build -t carnelian/carnelian-core:latest .
   ```

### Start the Ecosystem

```bash
# Standard setup (uses default resources)
docker-compose up -d

# With machine profile override (recommended)
docker-compose -f docker-compose.yml -f docker-compose.standard.yml up -d
# OR
docker-compose -f docker-compose.yml -f docker-compose.performance.yml up -d
```

### Verify Services

```bash
# Check status
docker-compose ps

# View logs
docker-compose logs -f

# Check specific service logs
docker-compose logs -f carnelian-core
```

### Download Models

```bash
# For Standard profile (8GB VRAM)
docker exec carnelian-ollama ollama pull deepseek-r1:7b

# For Performance profile (24GB VRAM)
docker exec carnelian-ollama ollama pull deepseek-r1:32b
```

## Machine Profiles

### Standard (RTX 2080 Super, 8GB VRAM, 32GB RAM)
```bash
docker-compose -f docker-compose.yml -f docker-compose.standard.yml up -d
```

**Resource Limits:**
- carnelian-core: 4 CPUs, 4GB RAM

**Recommended Model:** `deepseek-r1:7b`

### Performance (RTX 3090, 24GB VRAM, 64GB+ RAM)
```bash
docker-compose -f docker-compose.yml -f docker-compose.performance.yml up -d
```

**Resource Limits:**
- carnelian-core: 8 CPUs, 8GB RAM

**Recommended Model:** `deepseek-r1:32b`

## Data Persistence

### Volumes

| Volume | Purpose | Backup Strategy |
|--------|---------|-----------------|
| `postgres-data` | Database files | Use `pg_dump` for logical backups |
| `ollama-models` | Downloaded LLMs | Can be re-downloaded, but cache saves bandwidth |
| `carnelian-data` | Application data | Back up regularly |

### Backup Commands

```bash
# Backup PostgreSQL
docker exec carnelian-postgres pg_dump -U carnelian carnelian > backup.sql

# Restore PostgreSQL
cat backup.sql | docker exec -i carnelian-postgres psql -U carnelian carnelian
```

## Environment Variables

### Core Service

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection string | `postgresql://carnelian:carnelian@carnelian-postgres:5432/carnelian` |
| `CARNELIAN_OLLAMA_URL` | Ollama endpoint | `http://carnelian-ollama:11434` |
| `CARNELIAN_ENV` | Environment mode | `production` |
| `LOG_LEVEL` | Log verbosity | `INFO` |
| `OPENAI_API_KEY` | OpenAI API key | (optional) |
| `ANTHROPIC_API_KEY` | Anthropic API key | (optional) |
| `FIREWORKS_API_KEY` | Fireworks API key | (optional) |

### PostgreSQL

| Variable | Description | Default |
|----------|-------------|---------|
| `POSTGRES_USER` | Database user | `carnelian` |
| `POSTGRES_PASSWORD` | Database password | `carnelian` |
| `POSTGRES_DB` | Database name | `carnelian` |
| `PGVECTOR_DIMS` | Vector dimensions | `1536` |

### Ollama

| Variable | Description | Default |
|----------|-------------|---------|
| `OLLAMA_HOST` | Bind address | `0.0.0.0:11434` |
| `NVIDIA_VISIBLE_DEVICES` | GPU visibility | `all` |
| `OLLAMA_KEEP_ALIVE` | Model persistence | `-1` (indefinite) |
| `OLLAMA_NUM_GPU` | GPU layers | `999` (all) |

## Troubleshooting

### Service Won't Start

```bash
# Check logs
docker-compose logs [service-name]

# Check for port conflicts
netstat -tlnp | grep 18789
netstat -tlnp | grep 5432
netstat -tlnp | grep 11434
```

### Database Connection Issues

```bash
# Test database connectivity
docker exec carnelian-core pg_isready -h carnelian-postgres -U carnelian

# Check database logs
docker-compose logs carnelian-postgres
```

### GPU Not Available

```bash
# Verify NVIDIA runtime
docker run --rm --gpus all nvidia/cuda:11.0-base nvidia-smi

# Check Ollama GPU status
docker exec carnelian-ollama ollama ps
```

### Reset Everything

```bash
# Stop and remove containers
docker-compose down

# Remove volumes (DATA LOSS!)
docker-compose down -v

# Rebuild from scratch
docker-compose up -d --build
```

## Migration from TypeScript Gateway

The TypeScript gateway has been replaced with native Rust providers. No separate gateway container is needed.

**Before:**
```
carnelian-core → TypeScript Gateway → Ollama/OpenAI/Anthropic
```

**After:**
```
carnelian-core → Native Providers → Ollama/OpenAI/Anthropic
```

The gateway directory can be safely removed:
```bash
rm -rf packages/gateway/
```

## Security Considerations

1. **Never commit API keys** - Use environment variables or mounted files
2. **Use strong database passwords** in production
3. **Enable SSL/TLS** for production deployments
4. **Restrict network access** to necessary ports only
5. **Run with non-root user** (the Dockerfile already does this)

## Production Deployment Checklist

- [ ] Build production image: `docker build -t carnelian/carnelian-core:latest .`
- [ ] Configure machine.toml for your hardware
- [ ] Set appropriate API keys in environment
- [ ] Set strong PostgreSQL password
- [ ] Configure firewall rules
- [ ] Set up log rotation
- [ ] Configure backup jobs
- [ ] Test health endpoints
- [ ] Verify GPU access (if applicable)
- [ ] Download required models
- [ ] Run database migrations
- [ ] Test skill execution
- [ ] Verify WebSocket connections

## Additional Resources

- [Database Migrations](../db/migrations/README.md)
- [Machine Configuration](../machine.toml.example)
- [API Documentation](API.md)
