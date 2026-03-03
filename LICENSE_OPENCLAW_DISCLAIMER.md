## Relationship to OpenClaw

Carnelian Core was inspired by [OpenClaw](https://github.com/openclaw), an AI agent framework created by Peter Steinberger. While OpenClaw provided foundational inspiration for agent orchestration concepts, **Carnelian Core is a fundamentally different and unique implementation**.

### Key Architectural Differences

| Aspect | OpenClaw | Carnelian Core |
|--------|----------|----------------|
| **Language** | Python | Rust (core), TypeScript (UI), Python (workers) |
| **Architecture** | Monolithic agent framework | Multi-runtime worker orchestration with JSONL transport |
| **Security Model** | Traditional permissions | Capability-based deny-by-default with Ed25519 signatures |
| **State Management** | In-memory/file-based | PostgreSQL with pgvector, ledger-backed event sourcing |
| **Quantum Integration** | None | MAGIC subsystem with quantum entropy providers (Quantum Origin, Quantinuum H2, Qiskit) |
| **Knowledge Persistence** | RAG with vector DB | Elixir system with approval workflow, quality scoring, version control |
| **Skill System** | Python plugins | Multi-runtime (Node.js, Python, WASM, Rust) with 50+ curated skills |
| **Mantra System** | None | Quantum-seeded weighted context injection with cooldowns |
| **XP Progression** | None | Ledger-backed XP with automatic event sourcing and BLAKE3 hash-chaining |
| **Desktop UI** | CLI/Web | Dioxus native desktop application |
| **License** | MIT | Open source (free personal use), commercial licensing via Kordspace LLC |

### Novel Contributions

Carnelian Core introduces several innovations not present in OpenClaw or other agent frameworks:

1. **Quantum-Enhanced Entropy Generation**: First-of-its-kind multi-provider quantum entropy chain with cryptographic mixing for key generation, ledger salting, and mantra scheduling.

2. **Mantra Matrix System**: Weighted, cooldown-enforced context injection using quantum-seeded selection across 18 categories with 100+ mantras.

3. **Capability-Based Security**: Deny-by-default security model with Ed25519-signed authority chains, eliminating ambient authority vulnerabilities.

4. **Ledger-Backed XP Progression**: Immutable event sourcing for agent progression with BLAKE3 hash-chaining and quantum integrity verification.

5. **Multi-Runtime Worker Orchestration**: Unified orchestration of Node.js, Python, WASM, and native Rust workers via JSONL transport protocol.

6. **Elixir Knowledge System**: RAG-based knowledge persistence with pgvector embeddings, approval workflow, quality scoring, and version control.

### Acknowledgment

We acknowledge OpenClaw as an inspirational source that demonstrated the potential of AI agent frameworks. Carnelian Core builds upon these concepts while introducing a completely new architecture, security model, and feature set designed for production deployment and commercial use.

For a detailed comparison of agent frameworks, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#comparison-with-other-frameworks).
