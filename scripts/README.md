# Carnelian Scripts

Production-ready scripts for building, testing, and deploying Carnelian OS.

## Build Scripts

### `build-wasm-skills.sh`
Builds all WASM skills from Rust source.

```bash
./scripts/build-wasm-skills.sh
```

**What it does:**
- Compiles all Rust skills in `skills/registry/` to WASM
- Optimizes with `wasm-opt` for size
- Outputs to `skills/wasm/` directory
- Validates WASM modules

**Requirements:**
- Rust toolchain with `wasm32-unknown-unknown` target
- `wasm-opt` (from binaryen package)

### `package.sh`
Creates distribution packages for different platforms.

```bash
./scripts/package.sh [platform]
```

**Platforms:**
- `linux` - Linux x86_64 tarball
- `macos` - macOS universal binary
- `windows` - Windows x86_64 zip
- `all` - All platforms

**Output:** `dist/carnelian-{version}-{platform}.{ext}`

## Testing Scripts

### `ci-local.sh`
Runs the full CI pipeline locally before pushing.

```bash
./scripts/ci-local.sh
```

**What it does:**
- Runs `cargo test` on all crates
- Runs `cargo clippy` with strict lints
- Checks code formatting with `rustfmt`
- Validates WASM skill builds
- Runs integration tests

**Use before:** Committing or creating pull requests

## Monitoring Scripts

### `collect-metrics.sh`
Collects system metrics and performance data.

```bash
./scripts/collect-metrics.sh [duration]
```

**Arguments:**
- `duration` - Collection period in seconds (default: 60)

**Output:** `metrics-{timestamp}.json`

**Metrics collected:**
- CPU usage per component
- Memory usage (RSS, heap)
- Task execution times
- Event stream throughput
- Database query performance

## Setup Scripts

### `setup-hooks.sh`
Installs Git hooks for development workflow.

```bash
./scripts/setup-hooks.sh
```

**Hooks installed:**
- `pre-commit` - Runs formatting and linting
- `pre-push` - Runs tests
- `commit-msg` - Validates commit message format

## Usage Examples

### Full Build and Test
```bash
# Build WASM skills
./scripts/build-wasm-skills.sh

# Run local CI
./scripts/ci-local.sh

# Package for distribution
./scripts/package.sh all
```

### Development Workflow
```bash
# Set up Git hooks
./scripts/setup-hooks.sh

# Make changes...

# Test locally before pushing
./scripts/ci-local.sh
```

### Performance Monitoring
```bash
# Start Carnelian
cargo run --release &

# Collect metrics for 5 minutes
./scripts/collect-metrics.sh 300

# Analyze output
cat metrics-*.json | jq '.cpu_usage'
```

## Script Requirements

All scripts require:
- Bash 4.0+
- Standard Unix utilities (grep, sed, awk)
- Rust toolchain (for build scripts)

Platform-specific requirements are noted in each script's documentation.

## Troubleshooting

### WASM Build Failures
```bash
# Install wasm32 target
rustup target add wasm32-unknown-unknown

# Install wasm-opt
# macOS: brew install binaryen
# Linux: apt-get install binaryen
# Windows: Download from https://github.com/WebAssembly/binaryen/releases
```

### CI Script Failures
```bash
# Update dependencies
cargo update

# Clean and rebuild
cargo clean
cargo build --release

# Run specific test
cargo test --package carnelian-core --test integration_tests
```

### Permission Issues
```bash
# Make scripts executable
chmod +x scripts/*.sh
```

## Contributing

When adding new scripts:
1. Follow existing naming conventions
2. Add usage documentation in this README
3. Include error handling and validation
4. Test on Linux, macOS, and Windows (Git Bash)
5. Update CI pipeline if needed

## Development Scripts (Moved)

Historical development and validation scripts have been moved to `../DOCUMENTATION/`:
- Checkpoint validation scripts
- Demo scripts
- Migration utilities

These are preserved for reference but not needed for production use.
