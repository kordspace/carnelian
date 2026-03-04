# 🔥 Carnelian OS — macOS Setup Guide

Carnelian runs natively on macOS. GPU passthrough is not supported — Ollama runs CPU-only.

---

## Prerequisites Checklist

| Requirement | Version | Notes |
|-------------|---------|-------|
| macOS | 12 Monterey+ | Apple Silicon or Intel |
| Homebrew | Latest | Package manager |
| Docker Desktop | Latest | For PostgreSQL and Ollama |
| Rust | 1.85+ | Native toolchain |
| Node.js | 22+ | For Gateway service and Node.js workers |
| Python | 3.10+ | For Python worker |

---

## No GPU Passthrough

> **Important:** Docker on macOS does not support GPU passthrough. Ollama will run in **CPU-only mode** with reduced inference performance. For acceptable speeds, use smaller models only:
>
> - ✅ `deepseek-r1:7b` — Works on CPU, slower but functional
> - ❌ `deepseek-r1:32b` — Too slow for practical use on CPU
>
> For GPU-accelerated workloads, use a Linux machine or cloud instance.

---

## Apple Silicon Notes

If you're on an M1/M2/M3/M4 Mac:

- Rust compiles natively for `aarch64-apple-darwin` — no cross-compilation needed.
- Docker Desktop runs containers via Rosetta 2 or native ARM images. Most images (PostgreSQL, Ollama) have ARM builds.
- If you encounter architecture issues:

```bash
# Check your Rust target
rustc --print target-list | grep apple

# Ensure native target is installed
rustup target add aarch64-apple-darwin
```

---

## Step-by-Step Setup

### 1. Install Homebrew

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

For Apple Silicon, add Homebrew to your PATH if prompted:

```bash
echo 'eval "$(/opt/homebrew/bin/brew shellenv)"' >> ~/.zprofile
eval "$(/opt/homebrew/bin/brew shellenv)"
```

### 2. Install Docker Desktop

```bash
brew install --cask docker
```

Or download from [docker.com/products/docker-desktop](https://www.docker.com/products/docker-desktop).

After installation, launch Docker Desktop and complete the setup wizard.

### 3. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Verify
rustc --version
cargo --version
```

### 4. Install Node.js and Python

```bash
brew install node python@3.12

# Verify
node --version
npm --version
python3 --version
```

### 5. Install Development Tools

```bash
# SQLx CLI for database migrations
cargo install sqlx-cli --no-default-features --features postgres

# Pre-commit hooks
cargo install prek
```

---

## Carnelian First Run

```bash
# 1. Clone repository
git clone https://github.com/kordspace/carnelian.git
cd carnelian

# 2. Build the project
cargo build --release

# 3. Run the interactive setup wizard
# This detects your hardware, sets up Docker containers, creates the database,
# generates your owner keypair, and activates starter skills.
# On macOS, GPU VRAM will be 0 (CPU-only), so you'll be offered the "custom" profile.
carnelian init

# 4. Start the system
carnelian start
```

> **Non-interactive (CI/scripted):** `carnelian init --non-interactive`
> 
> For automated deployments, use the `--non-interactive` flag to skip all prompts.
> See [INSTALL.md](INSTALL.md) for detailed CI/CD setup options.

### Post-Init Setup

After running `carnelian init`, your system is ready. The wizard automatically:
- Starts PostgreSQL and Ollama Docker containers
- Runs database migrations
- Pulls the appropriate model for CPU inference (e.g., `deepseek-r1:7b`)
- Generates and secures your owner keypair

Access the dashboard at: http://localhost:18789

### macOS-Specific Note

Since macOS Docker doesn't support GPU passthrough, the init wizard will detect 0 GB VRAM and suggest the **"custom"** profile. This is expected behavior. The system will work in CPU-only mode — suitable for the 7B model but slower than GPU inference.

---

## macOS-Specific Troubleshooting

### Docker Desktop Resource Limits

Docker Desktop on macOS has default resource limits that may be too low. Adjust in **Docker Desktop → Settings → Resources**:

| Resource | Recommended |
|----------|-------------|
| CPUs | 4+ |
| Memory | 8GB+ (16GB if running Ollama) |
| Swap | 2GB |
| Disk | 64GB+ |

### Gatekeeper Warnings

If macOS blocks the Carnelian binary:

```bash
# Allow the binary
xattr -d com.apple.quarantine target/debug/carnelian

# Or in System Settings → Privacy & Security → Allow
```

### Homebrew Dependency Issues

```bash
# Diagnose Homebrew problems
brew doctor

# Update all packages
brew update && brew upgrade

# If OpenSSL issues occur during Rust builds
export OPENSSL_DIR=$(brew --prefix openssl@3)
export PKG_CONFIG_PATH="$OPENSSL_DIR/lib/pkgconfig"
```

### Port Conflicts

If port 18789 is already in use:

```bash
# Find the process
lsof -i :18789

# Use a different port
cargo run --bin carnelian -- start --port 18790
```

### Docker Compose v1 vs v2

macOS Docker Desktop ships with Compose v2. If `docker-compose` is not found:

```bash
# Use the v2 syntax
docker compose up -d

# Or create an alias
alias docker-compose='docker compose'
```

---

## Performance Tips

1. **Use smaller models** — `deepseek-r1:7b` is the practical maximum for CPU-only inference.
2. **Allocate Docker memory** — Give Docker at least 8GB for PostgreSQL + Ollama.
3. **Parallel builds** — `cargo build --jobs $(sysctl -n hw.ncpu)` to use all cores.
4. **SSD storage** — Ensure Docker's virtual disk is on your fastest drive.
5. **Activity Monitor** — Watch for `com.docker.hyperkit` or `qemu-system` consuming excessive resources.
