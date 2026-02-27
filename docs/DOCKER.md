# Docker Development Environment

This guide covers setting up the local development infrastructure for 🔥 Carnelian OS using Docker Compose.

## Prerequisites

- **Docker 24+** with Docker Compose v2+
- **NVIDIA Container Toolkit** (for GPU support)
- **NVIDIA GPU** with CUDA support (RTX 2080 or better recommended)

### Installing NVIDIA Container Toolkit

#### Linux (Ubuntu/Debian)

```bash
# Add NVIDIA package repository
curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | sudo gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg
curl -s -L https://nvidia.github.io/libnvidia-container/stable/deb/nvidia-container-toolkit.list | \
  sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' | \
  sudo tee /etc/apt/sources.list.d/nvidia-container-toolkit.list

# Install toolkit
sudo apt-get update
sudo apt-get install -y nvidia-container-toolkit

# Configure Docker
sudo nvidia-ctk runtime configure --runtime=docker
sudo systemctl restart docker
```

#### Windows (WSL2)

1. Install [NVIDIA GPU drivers for Windows](https://www.nvidia.com/Download/index.aspx)
2. Enable WSL2 GPU support (included in recent Windows 11 / Windows 10 21H2+)
3. Install Docker Desktop with WSL2 backend
4. In Docker Desktop Settings → Resources → WSL Integration, enable your distro

```bash
# Verify GPU access in WSL2
nvidia-smi
```

#### macOS

GPU passthrough is **not supported** on macOS. Ollama will run in CPU-only mode with significantly reduced performance.

## Quick Start

### 1. Start Services

```bash
# Start all services in background
docker-compose up -d

# View service status
docker-compose ps

# Follow logs
docker-compose logs -f
```

### 2. Verify Services

Both services should show as "healthy" after startup:

```bash
docker-compose ps
```

Expected output:
```
NAME                 STATUS                   PORTS
carnelian-ollama     running (healthy)        0.0.0.0:11434->11434/tcp
carnelian-postgres   running (healthy)        0.0.0.0:5432->5432/tcp
```

### 3. Test PostgreSQL Connection

```bash
# Using docker-compose exec
docker-compose exec carnelian-postgres psql -U carnelian -c "SELECT version();"

# Or using psql directly (if installed)
psql postgresql://carnelian:carnelian@localhost:5432/carnelian -c "SELECT version();"
```

### 4. Verify pgvector Extension

```bash
docker-compose exec carnelian-postgres psql -U carnelian -c "CREATE EXTENSION IF NOT EXISTS vector;"
docker-compose exec carnelian-postgres psql -U carnelian -c "SELECT * FROM pg_extension WHERE extname = 'vector';"
```

### 5. Test Ollama Connection

```bash
# Check available models
curl http://localhost:11434/api/tags

# Check Ollama version
curl http://localhost:11434/api/version
```

### 6. Verify GPU Access

```bash
docker-compose exec carnelian-ollama nvidia-smi
```

## Model Download

Download the recommended model for your machine profile:

### Standard Profile (RTX 2080 Super, 8GB VRAM)

```bash
docker exec carnelian-ollama ollama pull deepseek-r1:7b
```

### Performance Profile (RTX 3090, 24GB VRAM)

```bash
# 7B model (faster, less VRAM)
docker exec carnelian-ollama ollama pull deepseek-r1:7b

# 32B model (better quality, requires more VRAM)
docker exec carnelian-ollama ollama pull deepseek-r1:32b
```

### Test Model Inference

```bash
curl http://localhost:11434/api/generate -d '{
  "model": "deepseek-r1:7b",
  "prompt": "Hello, world!",
  "stream": false
}'
```

## Database Migrations

Run SQLx migrations to set up the database schema:

```bash
# Install sqlx-cli if not already installed
cargo install sqlx-cli --no-default-features --features postgres

# Run migrations
sqlx migrate run --database-url postgresql://carnelian:carnelian@localhost:5432/carnelian
```

See [db/migrations/README.md](../db/migrations/README.md) for more details.

## Connection Strings

| Service | URL | Purpose |
|---------|-----|---------|
| PostgreSQL (local) | `postgresql://carnelian:carnelian@localhost:5432/carnelian` | Rust crates, CLI tools |
| PostgreSQL (Docker) | `postgresql://carnelian:carnelian@carnelian-postgres:5432/carnelian` | Inter-service |
| Ollama API (local) | `http://localhost:11434` | Rust crates, CLI tools |
| Ollama API (Docker) | `http://carnelian-ollama:11434` | Inter-service |

## Troubleshooting

### GPU Not Detected

```bash
# Check if NVIDIA driver is installed
nvidia-smi

# Check Docker GPU support
docker run --rm --gpus all nvidia/cuda:12.0-base nvidia-smi

# Check container GPU access
docker-compose exec carnelian-ollama nvidia-smi
```

If GPU is not detected:
1. Ensure NVIDIA Container Toolkit is installed
2. Restart Docker daemon: `sudo systemctl restart docker`
3. On Windows WSL2, ensure GPU drivers are installed on Windows host

### Service Logs

```bash
# All services
docker-compose logs -f

# Specific service
docker-compose logs -f carnelian-postgres
docker-compose logs -f carnelian-ollama
```

### Health Check Failures

```bash
# Check health status
docker inspect carnelian-postgres --format='{{.State.Health.Status}}'
docker inspect carnelian-ollama --format='{{.State.Health.Status}}'

# View health check logs
docker inspect carnelian-postgres --format='{{json .State.Health}}' | jq
```

### Volume Inspection

```bash
# List volumes
docker volume ls | grep carnelian

# Inspect volume
docker volume inspect carnelian_postgres-data
docker volume inspect carnelian_ollama-models
```

### Port Conflicts

If ports 5432 or 11434 are already in use:

```bash
# Check what's using the port
lsof -i :5432
lsof -i :11434

# Or on Windows
netstat -ano | findstr :5432
```

Edit `docker-compose.yml` to use different ports if needed.

### Reset Everything

```bash
# Stop services and remove volumes (DATA LOSS!)
docker-compose down -v

# Remove all Carnelian containers and images
docker-compose down --rmi all -v

# Fresh start
docker-compose up -d
```

## Shutdown

### Preserve Data

```bash
docker-compose down
```

### Remove Data (Clean Slate)

```bash
# WARNING: This deletes all data including database and downloaded models!
docker-compose down -v
```

## Machine Profile Reference

| Profile | GPU | VRAM | RAM | Recommended Model | Ollama Memory Limit |
|---------|-----|------|-----|-------------------|---------------------|
| Standard | RTX 2080 Super | 8GB | 32GB | `deepseek-r1:7b` (default) | 10GB |
| Performance | RTX 3090 | 24GB | 64GB+ | `deepseek-r1:32b` or `deepseek-r1:70b` | 16GB |

To adjust resource limits for your profile, edit `docker-compose.yml` or create a `docker-compose.override.yml`:

```yaml
# docker-compose.override.yml (Performance profile)
services:
  carnelian-ollama:
    deploy:
      resources:
        limits:
          memory: 16G
        reservations:
          memory: 8G
```
