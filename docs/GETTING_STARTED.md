# Getting Started with Carnelian OS

## Quick Start

### Prerequisites

- **Rust** 1.75+ (install via [rustup](https://rustup.rs/))
- **PostgreSQL** 16+ with pgvector extension
- **Node.js** 20+ (for Node.js worker skills)
- **Ollama** (for local AI inference) - [ollama.com](https://ollama.com/)

### Installation

1. **Clone the repository**
   ```bash
   git clone https://github.com/kordspace/carnelian.git
   cd carnelian
   ```

2. **Set up the database**
   ```bash
   # Create database
   createdb carnelian
   
   # Install pgvector extension
   psql carnelian -c "CREATE EXTENSION IF NOT EXISTS vector;"
   
   # Run migrations
   sqlx database setup
   ```

3. **Configure environment**
   ```bash
   cp .env.example .env
   # Edit .env with your settings
   ```

4. **Build and run**
   ```bash
   cargo build --release
   cargo run --bin carnelian-bin
   ```

5. **Pull AI model**
   ```bash
   ollama pull deepseek-r1:7b
   ```

### First Steps

1. **Start the core orchestrator**
   ```bash
   ./target/release/carnelian-bin
   ```
   The API will be available at `http://localhost:18789`

2. **Start the LLM gateway** (in another terminal)
   ```bash
   cd gateway
   npm install
   npm start
   ```
   The gateway will be available at `http://localhost:18790`

3. **Test the system**
   ```bash
   # Health check
   curl http://localhost:18789/health
   
   # List skills
   curl http://localhost:18789/api/skills
   ```

### Using Docker

For a containerized setup:

```bash
docker-compose up -d
```

See [DOCKER.md](DOCKER.md) for detailed Docker configuration.

## Next Steps

- **[ARCHITECTURE.md](ARCHITECTURE.md)** - Understand the system architecture
- **[DEVELOPMENT.md](DEVELOPMENT.md)** - Development workflow and guidelines
- **[API.md](API.md)** - HTTP API reference
- **[WASM_SKILLS.md](WASM_SKILLS.md)** - Create custom skills
- **[OPERATOR_GUIDE.md](OPERATOR_GUIDE.md)** - Production deployment guide

## Configuration

### Machine Profiles

Carnelian adapts to your hardware. Create a `machine.toml` file:

```toml
[machine]
name = "my-machine"
gpu_vram_gb = 8
ram_gb = 32
cpu_cores = 8

[models]
default = "deepseek-r1:7b"
```

See `machine.toml.example` for all options.

### Environment Variables

Key environment variables in `.env`:

```bash
# Database
DATABASE_URL=postgresql://user:pass@localhost/carnelian

# API
CARNELIAN_PORT=18789
GATEWAY_PORT=18790

# Models
OLLAMA_HOST=http://localhost:11434
DEFAULT_MODEL=deepseek-r1:7b

# Security
SAFE_MODE=true
REQUIRE_APPROVAL=true
```

## Troubleshooting

### Database Connection Issues

```bash
# Check PostgreSQL is running
pg_isready

# Verify pgvector extension
psql carnelian -c "SELECT * FROM pg_extension WHERE extname = 'vector';"
```

### Ollama Connection Issues

```bash
# Check Ollama is running
ollama list

# Test model
ollama run deepseek-r1:7b "Hello"
```

### Port Conflicts

If ports 18789 or 18790 are in use, update `.env`:

```bash
CARNELIAN_PORT=18800
GATEWAY_PORT=18801
```

## Community & Support

- **GitHub Issues**: [github.com/kordspace/carnelian/issues](https://github.com/kordspace/carnelian/issues)
- **Documentation**: [docs/](../README.md)
- **License**: MIT

## What's Next?

Once you have Carnelian running:

1. **Explore Skills** - Browse the 530+ built-in skills
2. **Create Custom Skills** - Build your own WASM or Node.js skills
3. **Configure Security** - Set up capability grants and approvals
4. **Deploy to Production** - Follow the [OPERATOR_GUIDE.md](OPERATOR_GUIDE.md)

Welcome to 🔥 Carnelian OS!
