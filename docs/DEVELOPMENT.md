# Development Guide

This guide covers setting up your development environment and workflow for contributing to Carnelian OS.

## Prerequisites

- **Rust 1.85+** - Install from [rustup.rs](https://rustup.rs)
- **prek** - Rust-based pre-commit tool: `cargo install prek`
- **Docker** - For local PostgreSQL and Ollama services (see [DOCKER.md](DOCKER.md))

## Initial Setup

### 1. Clone the Repository

```bash
git clone https://github.com/kordspace/carnelian.git
cd carnelian
```

### 2. Run Setup Script

```bash
# Make the script executable (Linux/macOS)
chmod +x scripts/setup-hooks.sh

# Run setup
./scripts/setup-hooks.sh
```

This script will:
- Verify prek and cargo are installed
- Install Git pre-commit hooks
- Format existing code
- Run Clippy linting
- Build the workspace

### 3. Start Development Services

```bash
docker-compose up -d
```

See [DOCKER.md](DOCKER.md) for detailed Docker setup instructions.

## Development Workflow

### Format Code

```bash
cargo fmt --all
```

### Lint Code

```bash
cargo clippy --workspace --all-targets
```

### Run Tests

```bash
cargo test --workspace
```

### Build

```bash
cargo build --workspace
```

### Run Desktop UI

```bash
cargo run -p carnelian-ui
```

## Pre-commit Hooks

Git hooks are managed by [prek](https://github.com/prek-rs/prek) and run automatically on every commit.

### What Hooks Run

1. **File hygiene** - Trailing whitespace, end-of-file newlines, YAML/TOML validation
2. **Secret detection** - Prevents accidental commit of secrets
3. **cargo fmt** - Ensures code is properly formatted
4. **cargo clippy** - Enforces linting rules

### Manual Hook Execution

```bash
# Run all hooks on all files
prek run --all-files

# Run specific hook
prek run cargo-fmt --all-files
```

### Bypassing Hooks (Emergency Only)

```bash
git commit --no-verify -m "Emergency fix"
```

**Note:** Only use `--no-verify` in emergencies. CI will still enforce these checks.

## Editor Integration

### VS Code / Windsurf

Install the **rust-analyzer** extension and add to `.vscode/settings.json`:

```json
{
  "editor.formatOnSave": true,
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer"
  },
  "rust-analyzer.checkOnSave.command": "clippy"
}
```

### JetBrains IDEs

Install the **Rust** plugin. Configure:
- Settings → Languages & Frameworks → Rust → Rustfmt → Run rustfmt on save
- Settings → Languages & Frameworks → Rust → External Linters → Run Clippy

## Project Structure

```
carnelian/
├── crates/                     # Rust workspace crates
│   ├── carnelian-common/       # Shared types and utilities
│   ├── carnelian-core/         # Core orchestrator
│   ├── carnelian-ui/           # Dioxus desktop UI
│   ├── carnelian-worker-node/  # Node.js worker wrapper
│   ├── carnelian-worker-python/# Python worker wrapper
│   └── carnelian-worker-shell/ # Shell worker wrapper
├── workers/                    # Worker implementations
│   ├── node-worker/            # Node.js worker
│   ├── python-worker/          # Python worker
│   └── shell-worker/           # Shell worker
├── skills/                     # Skill bundles and registry
├── db/                         # Database migrations
├── docs/                       # Documentation
└── scripts/                    # Development scripts
```

## Troubleshooting

### Rust Version Mismatch

If you see errors about Rust version:

```bash
# Check current version
rustc --version

# Update Rust
rustup update

# Install specific version
rustup install 1.85
rustup default 1.85
```

### Missing Components

```bash
# Install required components
rustup component add rustfmt clippy rust-src
```

### Formatting Conflicts

If `cargo fmt` changes files unexpectedly:

1. Check `rustfmt.toml` settings match your editor
2. Ensure editor uses the project's `rustfmt.toml`
3. Run `cargo fmt --all` before committing

### Pre-commit Hook Failures

```bash
# See detailed output
prek run --all-files --verbose

# Fix formatting issues
cargo fmt --all

# Fix clippy issues
cargo clippy --workspace --all-targets --fix --allow-dirty
```

### Build Failures After Pull

```bash
# Clean and rebuild
cargo clean
cargo build --workspace
```

## Code Style

- Follow Rust idioms and conventions
- Use `rustfmt` defaults with project overrides in `rustfmt.toml`
- Address all Clippy warnings (workspace lints are in `Cargo.toml`)
- Write documentation for public APIs
- Add tests for new functionality

## Machine Profiles

Development is optimized for two machine profiles:

| Profile | GPU | RAM | Notes |
|---------|-----|-----|-------|
| Thummim | RTX 2080 Super (8GB) | 32GB | Constrained, use smaller models |
| Urim | RTX 2080 Ti (11GB) | 64GB | High-end, can run larger models |

See [DOCKER.md](DOCKER.md) for profile-specific configuration.
