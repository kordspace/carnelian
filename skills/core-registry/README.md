# CARNELIAN Core Registry

This directory contains **core system skills** that provide foundational capabilities for CARNELIAN.

> **Note**: Renamed from `registry` to `core-registry` (2026-02-26) for clarity and to distinguish from other skill registries.

## Purpose

The `core-registry` contains self-contained, essential skills that:
- Provide basic system operations and utilities
- Offer algorithms and data processing functions
- Serve as reference implementations
- Support multiple runtimes (WASM, Node.js, Python, Shell)

## Directory Structure

```
skills/core-registry/
├── README.md              # This file
├── hello-wasm/           # Example WASM skill
├── echo/                # Basic echo functionality
├── skill-creator/       # Skill creation utility
├── crypto-*/            # Cryptographic operations
├── json-*/              # JSON utilities
├── array-*/             # Array manipulation
├── string-*/            # String operations
├── math-*/              # Mathematical functions
├── file-*/              # File system operations
├── git-*/               # Git operations
├── http-*/              # HTTP utilities
├── browser-*/           # Browser automation
├── slack-*/             # Slack integration
├── discord-*/           # Discord integration
├── telegram-*/          # Telegram integration
├── cron-*/              # Cron/scheduling
├── gateway-*/           # Gateway operations
├── session-*/           # Session management
└── ...and 150+ more core utilities
```

## Naming Convention

Core skills follow the pattern: `{category}-{action}`

| Prefix | Category | Examples |
|--------|----------|----------|
| `array-*` | Array manipulation | `array-chunk`, `array-sort`, `array-unique` |
| `string-*` | String operations | `string-reverse`, `string-search`, `string-trim` |
| `json-*` | JSON processing | `json-parse`, `json-transform`, `json-validate` |
| `crypto-*` | Cryptography | `crypto-hash`, `crypto-encrypt`, `crypto-sign` |
| `math-*` | Mathematics | `math-eval`, `math-calculate`, `math-pow` |
| `file-*` | File operations | `file-read`, `file-write`, `file-delete` |
| `git-*` | Git operations | `git-branch`, `git-commit`, `git-diff` |
| `http-*` | HTTP utilities | `http-request`, `http-webhook` |
| `cron-*` | Scheduling | `cron-add`, `cron-list`, `cron-run` |
| `browser-*` | Browser automation | `browser-click`, `browser-navigate`, `browser-screenshot` |

## Runtime Distribution

| Runtime | Count | Percentage | Purpose |
|---------|-------|------------|---------|
| **WASM** | ~150 | 65% | Self-contained algorithms, data processing |
| **Node.js** | ~60 | 26% | I/O operations, platform integrations |
| **Python** | ~15 | 6% | Data analysis, code parsing |
| **Shell** | ~6 | 3% | System operations |
| **Total** | **231** | 100% | |

## Relationship to Other Registries

```
skills/
├── node-registry/       434 skills - Platform integrations (Stripe, AWS, etc.)
├── python-registry/      25 skills - ML/Data Science
├── core-registry/       231 skills - Core utilities (THIS DIRECTORY)
└──
    Total: 690 skills
    Unique: 624 skills (66 shared between core and node)
```

### Registry Responsibilities

| Registry | Purpose | Example Skills |
|----------|---------|----------------|
| **node-registry** | Third-party platform integrations | `stripe-payment`, `aws-s3-upload`, `github-create-issue` |
| **python-registry** | Machine learning & data science | `scikit-train`, `pandas-analyze`, `numpy-calculate` |
| **core-registry** | Foundational utilities & algorithms | `json-parse`, `array-sort`, `crypto-hash`, `math-eval` |

## Duplicate Skills

**66 skills exist in both core-registry and node-registry:**

These include browser automation, gateway operations, session management, communication integrations (Discord, Slack, Telegram), file operations, and various utilities.

**Why duplicates exist:**
1. **Core versions**: Simple, focused, dependency-free implementations
2. **Node versions**: Full-featured with platform-specific enhancements
3. **Different use cases**: Core for embedded use, Node for full integrations

**Examples:**
- `core-registry/discord-send` - Basic webhook sender
- `node-registry/discord-send` - Full Discord API integration with rate limiting

Both serve different needs and can coexist.

## Skill Count History

| Date | Count | Milestone |
|------|-------|-----------|
| 2026-02-26 | 231 | Renamed from `registry` to `core-registry` |
| Pre-2026 | 230 | As `registry` |
| Original | 200+ | Initial WASM-focused skills |

## Comparison to Target

Previous commits claimed **698 total skills** with breakdown:
- Node.js: 433 skills
- WASM/Rust: 230 skills (now core-registry: 231)
- Python: 25 skills
- Native: 10 skills

**Current state:** 624 unique skills (66 duplicates across registries)

## Creating Core Skills

Core skills should be:
- **Self-contained**: No external API dependencies
- **Well-tested**: Unit tests in `tests/unit/`
- **Documented**: Purpose, parameters, return values
- **Efficient**: Optimized for frequent use
- **Focused**: Single responsibility per skill

### Example skill.json

```json
{
  "name": "array-flatten",
  "description": "Flatten nested arrays to single level",
  "runtime": "wasm",
  "version": "1.0.0",
  "capabilities_required": [],
  "sandbox": {
    "max_memory_mb": 64,
    "max_cpu_percent": 5
  }
}
```

## Maintenance

When modifying core skills:
1. Check if duplicate exists in node-registry
2. Update both if behavior changes
3. Run tests: `make test-unit`
4. Update skill version
5. Document changes

## Related Documentation

- [SKILL_GAP_ANALYSIS.md](../../docs/SKILL_GAP_ANALYSIS.md) - Migration tracking
- [RUST_SKILL_SYSTEM.md](../../docs/RUST_SKILL_SYSTEM.md) - WASM skills
- [TESTING_GUIDE.md](../../documentation/TESTING_GUIDE.md) - Test procedures

---

**Part of CARNELIAN Skill Ecosystem**  
**Registry**: core-registry (231 skills)  
**Total Ecosystem**: 690 skills | 624 unique  
**Last Updated**: 2026-02-26
