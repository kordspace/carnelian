# Contributing to Carnelian Core

Thank you for your interest in contributing to Carnelian Core! We welcome contributions from the community and are excited to work with you.

## Repository Structure

The repository is organized by language and purpose:

```
CARNELIAN/
├── crates/              # Rust workspace (core, UI, workers, adapters, magic)
├── packages/            # TypeScript/JavaScript standalone services
│   ├── gateway/         # LLM gateway service
│   └── mcp-server/      # MCP server for Windsurf IDE integration
├── workers/             # Skill execution runtimes
│   ├── node-worker/     # Node.js/TypeScript worker
│   ├── python-worker/   # Python worker
│   └── shell-worker/    # Shell script worker
├── skills/              # Skill definitions (50+ curated skills)
├── db/                  # Database migrations and schemas
└── docs/                # Documentation
```

**Key Principles:**
- `packages/` contains standalone services that can be deployed independently
- `workers/` contains skill execution runtimes (all conceptually related)
- `crates/` contains the Rust monorepo workspace
- All TypeScript packages use `@carnelian/` scope

## Open Source & Licensing

**Carnelian is open source software** authored by **Marco Lopes**.

- **Free for Personal Use**: Anyone can use Carnelian Core for personal, educational, and non-commercial purposes without any license fees.
- **Commercial Use**: Requires a commercial license from Kordspace LLC. Contact info@kordspace.com with your use case.
- **Custodianship**: Kordspace LLC serves as the custodian of all Carnelian assets, providing IP protection, security auditing, commercial licensing, and community stewardship.

### Contributor License Agreement (CLA)

**By contributing to Carnelian Core, you agree to the CLA outlined in [LICENSE.md](LICENSE.md).**

Key points:
- You grant Kordspace LLC a perpetual, worldwide, non-exclusive, royalty-free, irrevocable license to use your contributions.
- Your contributions are your original work and do not infringe on third-party IP rights.
- You will be credited as a contributor in project documentation.
- Contributions are provided "as is" without warranty.

This CLA ensures the project can remain open source while protecting the technology and enabling commercial licensing for enterprise users.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [How to Contribute](#how-to-contribute)
  - [Reporting Bugs](#reporting-bugs)
  - [Suggesting Features](#suggesting-features)
  - [Pull Requests](#pull-requests)
- [Development Guidelines](#development-guidelines)
  - [Code Style](#code-style)
  - [Testing](#testing)
  - [Documentation](#documentation)
- [Community](#community)

## Code of Conduct

This project and everyone participating in it is governed by our [Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

## Getting Started

1. Fork the repository on GitHub
2. Clone your fork locally
3. Set up the development environment
4. Create a branch for your changes

## Development Setup

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable version)
- [Node.js](https://nodejs.org/) (for E2E tests and skill development)
- [Docker](https://docs.docker.com/get-docker/) and Docker Compose
- [PostgreSQL](https://www.postgresql.org/) (or use Docker)

### Installation

```bash
# Clone your fork
git clone https://github.com/YOUR_USERNAME/carnelian.git
cd carnelian

# Install Rust dependencies
cargo build

# Set up environment
cp .env.example .env
# Edit .env with your configuration

# Start development services
docker-compose up -d postgres ollama

# Run database migrations
cargo sqlx migrate run

# Run tests
cargo test --all
```

## How to Contribute

### Reporting Bugs

Before creating a bug report, please:

1. Check the [existing issues](https://github.com/kordspace/carnelian/issues) to see if the problem has already been reported
2. Try to reproduce the issue with the latest `main` branch
3. Collect information about the bug:
   - Stack traces
   - Error messages
   - Steps to reproduce
   - Expected vs actual behavior

When creating a bug report, please include:

- **Title**: Clear and descriptive
- **Description**: Detailed explanation of the issue
- **Environment**: OS, Rust version, CARNELIAN version
- **Reproduction steps**: Step-by-step instructions
- **Expected behavior**: What you expected to happen
- **Actual behavior**: What actually happened
- **Screenshots/Logs**: If applicable

### Suggesting Features

We welcome feature suggestions! Please:

1. Check if the feature has already been suggested
2. Provide a clear use case
3. Explain why this feature would be useful
4. Consider how it fits with the project's goals

Feature requests should include:

- **Title**: Clear and concise
- **Description**: What you want to achieve
- **Motivation**: Why this feature is needed
- **Proposed solution**: How you think it should work
- **Alternatives**: Other approaches you've considered

### Pull Requests

1. **Create a branch**: `git checkout -b feature/your-feature-name`
2. **Make your changes**: Follow our development guidelines
3. **Test your changes**: Run the full test suite
4. **Commit your changes**: Use clear, descriptive commit messages
5. **Push to your fork**: `git push origin feature/your-feature-name`
6. **Open a Pull Request**: Include a clear description of changes

#### Pull Request Process

1. Update the README.md if needed
2. Update documentation for any API changes
3. Ensure all tests pass
4. Ensure code is properly formatted (`cargo fmt`)
5. Address any review feedback
6. Squash commits if requested

## Development Guidelines

### Code Style

We follow standard Rust conventions:

- Use `cargo fmt` to format code
- Use `cargo clippy` to catch common mistakes
- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Write descriptive variable and function names
- Add comments for complex logic
- Keep functions focused and small

#### Formatting

```bash
# Format all code
cargo fmt --all

# Check formatting without making changes
cargo fmt --all -- --check
```

#### Linting

```bash
# Run clippy
cargo clippy --all -- -D warnings
```

### Testing

All code should be tested:

- Write unit tests for new functionality
- Add integration tests for API changes
- Ensure E2E tests pass for user-facing changes
- Aim for high test coverage

```bash
# Run all tests
cargo test --all

# Run specific test
cargo test test_name

# Run with output
cargo test --all -- --nocapture

# Run benchmarks
cargo bench

# Run E2E tests
cd tests/e2e && npm test
```

### Documentation

- Update README.md for user-facing changes
- Add rustdoc comments for public APIs
- Update TESTING_GUIDE.md for test changes
- Update SECURITY_CHECKLIST.md for security changes
- Write clear commit messages

## Project Structure

```
carnelian/
├── crates/              # Rust crates
│   ├── carnelian-core/  # Core library
│   └── carnelian-common/# Shared types
├── skills/              # WASM skills
├── tests/               # Test suites
├── docs/                # Documentation
├── scripts/             # Utility scripts
└── monitoring/          # Observability configs
```

## Community

- **Discussions**: Use GitHub Discussions for questions
- **Issues**: Report bugs and request features via GitHub Issues
- **Security**: Report security issues privately (see SECURITY.md)

## Questions?

If you have questions, feel free to:

- Open a [GitHub Discussion](https://github.com/kordspace/carnelian/discussions)
- Check existing documentation in `docs/`
- Ask in an issue (if related to existing work)

## License

By contributing to CARNELIAN, you agree that your contributions will be licensed under the [LICENSE](LICENSE) file in this repository.

## MAGIC Development Guide

This section covers development workflows specific to the MAGIC (Mixed Authenticated Quantum Intelligence Core) subsystem.

### Adding a New Mantra Category

Mantra categories live in the `mantra_categories` table, seeded in `db/migrations/00000000000016_magic_mantras.sql`.

To add a new category:

1. Create a new SQL migration file under `db/migrations/` with the next sequence number (e.g., `00000000000018_add_mantra_category.sql`).
2. Write an `INSERT` statement into `mantra_categories` supplying:
   - `name` — Category name (e.g., "Debugging", "Optimization")
   - `description` — Brief description of the category's purpose
   - `system_message` — System prompt template for this category
   - `user_message` — User prompt template for this category
   - `base_weight` — Initial selection weight (default: 100)
   - `cooldown_beats` — Number of heartbeat cycles before re-selection is allowed
   - `suggested_skill_tags` — Array of skill tags to suggest when this category is selected
   - `elixir_types` — Array of elixir types that boost this category's weight

Example:

```sql
INSERT INTO mantra_categories (
    name, description, system_message, user_message,
    base_weight, cooldown_beats, suggested_skill_tags, elixir_types
) VALUES (
    'Debugging',
    'Analyze errors and identify root causes',
    'You are a debugging expert analyzing system errors.',
    'Review recent errors and identify patterns or root causes.',
    100,
    3,
    ARRAY['error-analysis', 'debugging'],
    ARRAY['domain_knowledge']
);
```

3. Run `cargo sqlx migrate run` to apply the migration.

### Adding Mantras to an Existing Category

Mantras are rows in the `mantra_entries` table, keyed by `category_id`.

To add mantras to an existing category:

1. Create a new migration file or run directly against a dev database.
2. Write an `INSERT INTO mantra_entries` statement using a CTE to resolve `category_id` by name:

```sql
WITH category AS (
    SELECT category_id FROM mantra_categories WHERE name = 'Debugging'
)
INSERT INTO mantra_entries (category_id, text)
SELECT category_id, 'What error patterns emerge from the last 10 failures?'
FROM category;
```

This pattern follows the seed migration structure and ensures the category exists before inserting entries.

### Writing a Quantum Circuit Skill (Python)

Quantum circuit skills live under `skills/python-registry/` and integrate with the entropy provider chain.

#### Skill Types

There are two quantum skill types:

1. **pytket** — For Quantinuum H2 devices (emulator or hardware)
2. **Qiskit** — For IBM Quantum backends

#### pytket Skill (Quantinuum H2)

Implement a `QuantumCircuit` using `pytket.circuit`:

```python
from pytket.circuit import Circuit
from pytket.extensions.quantinuum import QuantinuumBackend

def run(params):
    n_bits = params.get('n_bits', 256)  # Provider passes n_bits parameter
    n_bytes = n_bits // 8
    
    # Build Hadamard circuit
    circuit = Circuit(n_bits)
    for i in range(n_bits):
        circuit.H(i)
    circuit.measure_all()
    
    # Submit to Quantinuum
    backend = QuantinuumBackend(device_name="H1-1E")
    compiled = backend.get_compiled_circuit(circuit)
    handle = backend.process_circuit(compiled, n_shots=1)
    result = backend.get_result(handle)
    
    # Extract bits and encode as hex
    bits = result.get_counts().most_common(1)[0][0]
    hex_bytes = hex(int(bits, 2))[2:].zfill(n_bytes * 2)
    
    return {"bytes": hex_bytes}
```

#### Qiskit Skill (IBM Quantum)

Build a `QuantumCircuit` with Hadamard + measure:

```python
from qiskit import QuantumCircuit
from qiskit_ibm_runtime import QiskitRuntimeService, Sampler

def run(params):
    shots = params.get('shots', 256)  # Provider passes shots parameter (bits requested)
    n_bits = shots
    n_bytes = n_bits // 8
    
    # Build circuit
    qc = QuantumCircuit(n_bits, n_bits)
    for i in range(n_bits):
        qc.h(i)
    qc.measure(range(n_bits), range(n_bits))
    
    # Run on IBM Quantum
    service = QiskitRuntimeService()
    backend = service.backend("ibm_brisbane")
    sampler = Sampler(backend)
    job = sampler.run(qc, shots=1)
    result = job.result()
    
    # Decode bitstring
    bitstring = list(result.quasi_dists[0].keys())[0]
    hex_bytes = hex(bitstring)[2:].zfill(n_bytes * 2)
    
    return {"bytes": hex_bytes}
```

#### Output Contract

The skill's output JSON **must** include a `bytes` field (hex-encoded) matching the requested bit count. This is what `QuantinuumH2Provider` and `QiskitProvider` in `crates/carnelian-magic/src/entropy.rs` parse via `SkillBridge.invoke_skill`.

#### Registration

Register the skill in `skills/python-registry/<skill-name>/` with appropriate metadata:

```json
{
  "name": "quantinuum-h2-rng",
  "runtime": "python",
  "tags": ["quantum", "entropy", "rng"],
  "required_config": ["QUANTINUUM_API_KEY"]
}
```

### Implementing a New `EntropyProvider`

The `EntropyProvider` trait is defined in `crates/carnelian-magic/src/entropy.rs`.

#### Required Methods

Implement the four required methods:

```rust
#[async_trait]
pub trait EntropyProvider: Send + Sync {
    /// Return a unique slug identifying this provider
    fn source_name(&self) -> &str;
    
    /// Lightweight reachability check (no network calls if possible)
    async fn is_available(&self) -> bool;
    
    /// Return exactly `n` bytes or a `MagicError`
    async fn get_bytes(&self, n: usize) -> Result<Vec<u8>, MagicError>;
    
    /// Call `get_bytes(32)`, measure latency, populate `EntropyHealth`
    async fn health(&self) -> EntropyHealth {
        let start = std::time::Instant::now();
        match self.get_bytes(32).await {
            Ok(_) => EntropyHealth {
                available: true,
                latency_ms: start.elapsed().as_millis() as u64,
                error: None,
            },
            Err(e) => EntropyHealth {
                available: false,
                latency_ms: 0,
                error: Some(e.to_string()),
            },
        }
    }
}
```

#### Integration

1. Add the new provider as an `Option<YourProvider>` field on `MixedEntropyProvider`.
2. Wire it into the priority chain in `get_bytes` (after `QiskitProvider`, before `OsEntropyProvider`).
3. Expose construction of the new provider in `lib.rs` / config wiring.

Example:

```rust
pub struct MixedEntropyProvider {
    quantum_origin: Option<QuantumOriginProvider>,
    quantinuum: Option<QuantinuumH2Provider>,
    qiskit: Option<QiskitProvider>,
    your_provider: Option<YourProvider>,  // Add here
    os: OsEntropyProvider,
}

impl MixedEntropyProvider {
    async fn get_bytes(&self, n: usize) -> Result<Vec<u8>, MagicError> {
        // Try quantum providers in priority order
        if let Some(qo) = &self.quantum_origin {
            if let Ok(bytes) = qo.get_bytes(n).await { return Ok(bytes); }
        }
        if let Some(q) = &self.quantinuum {
            if let Ok(bytes) = q.get_bytes(n).await { return Ok(bytes); }
        }
        if let Some(qk) = &self.qiskit {
            if let Ok(bytes) = qk.get_bytes(n).await { return Ok(bytes); }
        }
        if let Some(yp) = &self.your_provider {  // Add to chain
            if let Ok(bytes) = yp.get_bytes(n).await { return Ok(bytes); }
        }
        
        // Always fall back to OS
        self.os.get_bytes(n).await
    }
}
```

### Testing MAGIC Without Quantum Credentials

All quantum providers are `Option<_>` in `MixedEntropyProvider` — passing `None` for all three quantum providers causes automatic fallback to `OsEntropyProvider`.

#### Test Strategies

1. **Mock Skill Bridge**: Use `MockSkillBridge { fail: true }` (already exists in `entropy.rs` test module) to simulate unavailable quantum hardware.

2. **Empty Environment Variables**: Set `CARNELIAN_QUANTUM_ORIGIN_API_KEY` to empty string or omit it — `QuantumOriginProvider::new` reads the env var and treats an empty key as unconfigured.

3. **Unit Tests**: Run `cargo test -p carnelian-magic` to exercise the full fallback chain without any network calls.

Example tests demonstrating this:

- `test_mixed_provider_os_fallback` — Verifies OS fallback when all quantum providers are `None`
- `test_provider_priority_selection` — Verifies priority chain behavior

All tests run offline and never require actual quantum credentials.

---

## Post-Quantum Cryptography Development

### Current Status (v1.0.0)

The `crates/carnelian-magic/src/pqc.rs` module ships fully implemented in v1.0.0 with 8 comprehensive tests, but is **not activated by default**. The three key types are:

- **`HybridSigningKey`** — Dual-signature scheme combining CRYSTALS-Dilithium3 and Ed25519; both signatures must pass verification for trust
- **`KyberKem`** — CRYSTALS-Kyber1024 key encapsulation mechanism for quantum-resistant shared-secret exchange
- **`KeyAlgorithm` enum** — Variants: `Ed25519` (current v1.0.0 default), `HybridDilithiumEd25519` (v1.1.0 opt-in), `Dilithium3` (v2.0.0+)

v1.0.0 deployments use `KeyAlgorithm::Ed25519` via the owner keypair in `carnelian-core`. The full migration path is documented in `DOCUMENTATION/FUTURE_PQC.md`.

### Running the PQC Tests

Run the full PQC test suite:

```bash
cargo test -p carnelian-magic
```

The 8 test cases cover:

- `test_hybrid_signing_key_generation` — Async quantum entropy keypair generation
- `test_hybrid_signature_roundtrip` — Sign/verify with dual signatures
- `test_hybrid_signature_fails_on_wrong_message` — Signature validation
- `test_kyber_kem_roundtrip` — Encapsulate/decapsulate shared secret
- `test_kyber_kem_fails_on_wrong_ciphertext` — KEM error handling
- `test_public_key_export` — Public key serialization
- `test_aes_key_derivation` — AES-256 key derivation from Ed25519 seed

All tests run offline using `MixedEntropyProvider::new_os_only()` — no quantum credentials required.

### Activating PQC (v1.1.0 Opt-In)

PQC primitives are activated by enabling MAGIC quantum entropy and calling `generate_hybrid_keypair_with_entropy()` from `carnelian-core/src/crypto.rs`. The `key_algorithm` column in `config_store` (Migration 18) tracks which algorithm is active.

The planned CLI migration command for v1.1.0:

```bash
carnelian crypto migrate --to hybrid
```

This command will handle key rotation from Ed25519 to HybridDilithiumEd25519. Direct readers to `DOCUMENTATION/FUTURE_PQC.md` for the complete v1.1.0/v1.2.0/v2.0.0 roadmap.

### Extending `HybridSigningKey`

To add a new signing algorithm:

1. Implement the same `sign()` / `verify()` interface as `HybridSigningKey` in `crates/carnelian-magic/src/pqc.rs`
2. Add a new `KeyAlgorithm` variant to the enum
3. Wire the new algorithm into `carnelian-core/src/crypto.rs` hybrid helper functions
4. Add comprehensive tests following the existing 8-test pattern
5. Update `DOCUMENTATION/FUTURE_PQC.md` with migration documentation

---

Thank you for contributing to CARNELIAN! 🎉
