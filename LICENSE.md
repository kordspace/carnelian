# Carnelian Core License

**Copyright © 2024-2026 Marco Julio Lopes. All Rights Reserved.**

---

## Open Source Software — Free for Personal Use

Carnelian Core is **open source software** authored by **Marco Julio Lopes**. The source code is publicly available for transparency, learning, and community collaboration.

### Free Personal Use

You are **free to use Carnelian Core for personal, educational, and non-commercial purposes** without any license fees or restrictions, including:

- Personal projects and experimentation
- Academic research and education
- Learning and skill development
- Non-profit community initiatives
- Open source contributions

### Commercial Use Licensing

**Commercial use requires a separate commercial license from Kordspace LLC.**

If you wish to use Carnelian Core in a commercial context, including but not limited to:
- Production deployments in for-profit organizations
- Integration into commercial products or services
- Consulting or professional services using Carnelian
- Revenue-generating applications or platforms

Please contact **info@kordspace.com** with:
1. Your use case and deployment context
2. Organization details and scale of deployment
3. Any specific security or compliance requirements

Kordspace LLC, as the custodian and service provider for Carnelian Core, will:
- Issue a commercial license tailored to your use case
- Provide a comprehensive security audit for your deployment
- Offer ongoing support and validation services
- Ensure compliance with patent-pending technology protections

---

## Custodianship and Asset Protection

**Kordspace LLC** serves as the custodian of Carnelian Core assets, providing:
- Intellectual property protection and management
- Security auditing and validation services
- Commercial licensing and support
- Fair use promotion and community stewardship

This custodianship arrangement protects the technology while ensuring it remains accessible for personal and educational use, promoting fair and responsible adoption of this new technology.

---

## Patent Pending

Certain features and methodologies embodied in this Software, including but not limited to:
- Quantum-enhanced entropy generation with multi-provider fallback chains
- Mantra-based context injection with weighted category selection
- Ledger-backed XP progression with automatic event sourcing
- Capability-based deny-by-default security architecture
- Multi-runtime worker orchestration with JSONL transport protocol

are the subject of pending patent applications and/or trade secret protection.

---

## Contributor License Agreement (CLA)

### Contribution Requirements

By contributing code, documentation, or other materials to Carnelian Core, you agree that:

1. **Grant of Rights**: You grant Marco Julio Lopes and Kordspace LLC a perpetual, worldwide, non-exclusive, royalty-free, irrevocable license to use, reproduce, modify, display, perform, sublicense, and distribute your contributions.

2. **Original Work**: Your contributions are your original work and do not infringe on any third-party intellectual property rights.

3. **No Warranty**: Contributions are provided "as is" without warranty of any kind.

4. **Attribution**: You will be credited as a contributor in project documentation.

### Contribution Process

1. Fork the repository and create a feature branch
2. Make your changes following the project's coding standards
3. Submit a pull request with a clear description
4. Sign the CLA when prompted (first-time contributors)
5. Await review and approval from project maintainers

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed contribution guidelines.

### Major Contributor Co-Authorship

**Major contributors** who make substantial, sustained contributions to Carnelian Core may be granted **co-authorship status** and added to the copyright and licensing documentation.

Co-authorship is awarded based on:
- Significant code contributions (features, architecture, core systems)
- Sustained engagement and maintenance over multiple release cycles
- Demonstrated expertise and leadership in project areas
- Alignment with project vision and quality standards

Co-authors receive:
- Recognition in copyright notices and LICENSE.md
- Credit in project documentation and release notes
- Participation in technical decision-making processes
- Shared stewardship of the project's direction

Co-authorship decisions are made by Marco Julio Lopes and Kordspace LLC after thorough review of contribution history and impact.

---

## Disclaimer of Warranty

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE, AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES, OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT, OR OTHERWISE, ARISING FROM, OUT OF, OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

---

## Third-Party Dependencies

Carnelian Core incorporates the following open source frameworks and libraries, each governed by its respective license:

### Rust Ecosystem

| Dependency | License | Purpose |
|------------|---------|---------|
| **tokio** | MIT | Async runtime for Rust |
| **axum** | MIT | Web framework for HTTP API |
| **sqlx** | MIT / Apache-2.0 | Async PostgreSQL driver |
| **serde** | MIT / Apache-2.0 | Serialization framework |
| **ed25519-dalek** | BSD-3-Clause | Ed25519 signatures |
| **blake3** | CC0-1.0 / Apache-2.0 | Cryptographic hashing |
| **uuid** | MIT / Apache-2.0 | UUID generation |
| **chrono** | MIT / Apache-2.0 | Date and time handling |
| **tracing** | MIT | Structured logging |
| **reqwest** | MIT / Apache-2.0 | HTTP client |
| **wasmtime** | Apache-2.0 | WebAssembly runtime |
| **config** | MIT / Apache-2.0 | Configuration management |

### JavaScript/TypeScript Ecosystem

| Dependency | License | Purpose |
|------------|---------|---------|
| **Dioxus** | MIT / Apache-2.0 | Desktop UI framework |
| **TypeScript** | Apache-2.0 | Type-safe JavaScript |
| **Node.js** | MIT | JavaScript runtime |

### Database & Infrastructure

| Dependency | License | Purpose |
|------------|---------|---------|
| **PostgreSQL** | PostgreSQL License | Relational database |
| **pgvector** | PostgreSQL License | Vector similarity search |
| **Docker** | Apache-2.0 | Containerization |

### Python Ecosystem (Optional)

| Dependency | License | Purpose |
|------------|---------|---------|
| **pytket** | Apache-2.0 | Quantum circuit framework |
| **pytket-quantinuum** | Apache-2.0 | Quantinuum backend |
| **qiskit** | Apache-2.0 | IBM Quantum framework |
| **qiskit-ibm-runtime** | Apache-2.0 | IBM Quantum runtime |

### AI/ML Frameworks (Optional)

| Dependency | License | Purpose |
|------------|---------|---------|
| **Ollama** | MIT | Local LLM inference |
| **OpenAI SDK** | MIT | OpenAI API client |
| **Anthropic SDK** | MIT | Anthropic API client |

All third-party dependencies retain their original licenses and copyrights. Carnelian Core's use of these dependencies complies with their respective license terms.

---

## Governing Law

This License shall be governed by and construed in accordance with the laws of the jurisdiction in which Kordspace LLC is registered, without regard to its conflict of law provisions.

---

## Acknowledgments

Carnelian Core has been inspired by and built upon the work of many talented individuals and projects in the AI agent and software development communities.

### Special Recognition

- **Peter Steinberger** — Creator of [OpenClaw](https://github.com/openclaw), whose pioneering work on AI agent frameworks and tool orchestration provided foundational inspiration for Carnelian's architecture.

- **Justin Oberg** — Early collaborator who started this journey with Marco by working on getting the original GPT-2 1B model running on local hardware, kicking off the exploration that led to Carnelian's local-first LLM architecture.

- **Jonathan Essex** — Founder of [Software Plumbers](https://softwareplumbers.com), whose mentorship in cryptography, PostgreSQL transactional ledgers, Merkle tree architectures, and blake3 hash-chain design directly influenced Carnelian's ledger system and capability-based security model.

- **Vincent Haliburton** — Provided inspiration and strategic guidance for Carnelian's planned utility token on Base (Ethereum L2), shaping the vision for future Web3 integration and token-gated capability grants.

- **Marcelino Class** — Contributed UI/UX design insights, animation knowledge, and research assistance that helped refine the Dioxus desktop UI and agent interaction patterns.

These individuals represent the collaborative spirit of innovation and mentorship that made Carnelian Core possible. For detailed contributor acknowledgments, see [CONTRIBUTORS.md](CONTRIBUTORS.md).

---

## Contact Information

- **Author**: Marco Julio Lopes
- **Commercial Licensing**: info@kordspace.com
- **Project Repository**: https://github.com/kordspace/carnelian
- **Website**: https://kordspace.com

---

**Last Updated**: March 3, 2026
## Relationship to OpenClaw

Carnelian was inspired by OpenClaw, an AI agent framework created by Peter Steinberger. While OpenClaw provided foundational inspiration for agent orchestration concepts, Carnelian is a fundamentally different implementation with distinct architectural choices.

For a detailed comparison, see [docs/OPENCLAW_COMPARISON.md](docs/OPENCLAW_COMPARISON.md).
