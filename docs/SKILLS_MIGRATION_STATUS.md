# Skills Migration Status — Carnelian Core v1.0.0

This document tracks the migration of skills from the legacy skill library into Carnelian's curated Skill Book.

## Status Summary

| Category | Legacy Skills | Migrated | Status |
|----------|---------------|----------|--------|
| **Quantum** | 3 | 3 | ✅ Complete |
| **Core Utilities** | 50+ | 15 | 🚧 In Progress |
| **GCP Integration** | 12 | 12 | ✅ Complete |
| **Native Operations** | 10 | 10 | ✅ Complete |
| **Python ML/Data** | 30+ | 5 | 🚧 In Progress |
| **Node.js Platform APIs** | 500+ | 20 | 🚧 In Progress |
| **WASM Skills** | 10+ | 3 | 🚧 In Progress |

**Total:** 50+ curated skills migrated, 600+ in migration queue

## Migration Notes

### Completed Categories

#### Quantum (Phase 10)
- ✅ `quantinuum-h2-rng` — Hadamard circuit entropy via pytket
- ✅ `qiskit-rng` — IBM Quantum backend entropy via Qiskit
- ✅ `quantum-optimize` — Quantum-seeded simulated annealing

#### GCP Integration (Phase 8)
- ✅ `gcp-bigquery-query` — Execute SQL queries on BigQuery
- ✅ `gcp-pubsub-publish` — Publish messages to Pub/Sub topics
- ✅ `gcp-storage-upload` — Upload files to Cloud Storage
- ✅ `gcp-compute-list-instances` — List Compute Engine instances
- ✅ `gcp-firestore-query` — Query Firestore collections
- ✅ `gcp-cloud-run-deploy` — Deploy Cloud Run services
- ✅ `gcp-cloud-functions-deploy` — Deploy Cloud Functions
- ✅ `gcp-iam-list-roles` — List IAM roles
- ✅ `gcp-logging-query` — Query Cloud Logging
- ✅ `gcp-monitoring-metrics` — Fetch Cloud Monitoring metrics
- ✅ `gcp-secret-manager-get` — Retrieve secrets from Secret Manager
- ✅ `gcp-vertex-ai-predict` — Vertex AI model predictions

#### Native Operations (Phase 8)
- ✅ `git_status` — Git repository status
- ✅ `file_hash` — BLAKE3 file hashing
- ✅ `docker_ps` — Docker container listing
- ✅ `dir_list` — Directory traversal with walkdir
- ✅ `process_info` — System process information
- ✅ `network_info` — Network interface information
- ✅ `disk_usage` — Disk space usage
- ✅ `memory_stats` — System memory statistics
- ✅ `cpu_info` — CPU information
- ✅ `env_vars` — Environment variable access

### In Progress Categories

#### Core Utilities (15/50+)
Migrated: `uuid-generate`, `hash-compute`, `time-format`, `json-parse`, `csv-to-json`, `base64-encode`, `base64-decode`, `url-parse`, `regex-match`, `string-transform`, `number-format`, `date-parse`, `file-read`, `file-write`, `http-request`

Pending: File system operations, text processing, data validation, encryption utilities, compression utilities, and more.

#### Python ML/Data (5/30+)
Migrated: `pandas-dataframe`, `numpy-array`, `matplotlib-plot`, `scikit-learn-train`, `tensorflow-predict`

Pending: Advanced ML models, data visualization, statistical analysis, NLP processing, computer vision, and more.

#### Node.js Platform APIs (20/500+)
Migrated: Core HTTP, file system, process management, and basic integrations.

Pending: Extensive platform API integrations (Stripe, GitHub, Slack, AWS, Azure, etc.)

#### WASM Skills (3/10+)
Migrated: `hello-wasm`, `blake3-hash-wasm`, `fibonacci-wasm`

Pending: Additional computational and cryptographic WASM skills.

## Known Limitations

The v1.0.0 release ships with **50+ curated skills** in `skills/skill-book/` plus **bulk import tooling** for migrating existing skill libraries. Full migration of the 600-skill legacy library is tracked as an ongoing effort in this document.

### Bulk Import Tooling

The `carnelian skills import` command (planned for v1.1.0) will enable automated migration of compatible skills from the legacy library. Current manual migration process:

1. Copy skill directory to `skills/skill-book/<category>/`
2. Update `skill.json` manifest with category and required_config
3. Run `carnelian skills refresh` to sync to database
4. Test skill execution via CLI or UI
5. Update this document with migration status

## Migration Roadmap

### v1.1.0 (Q2 2026)
- Bulk import CLI command
- Automated manifest validation
- Category auto-detection
- 100+ additional skills migrated

### v1.2.0 (Q3 2026)
- 200+ additional skills migrated
- Advanced skill chaining
- Skill marketplace integration

### v2.0.0 (Q4 2026)
- Full 600-skill parity
- Custom skill SDK
- Community skill submissions

## Contributing

To contribute skill migrations:

1. Fork the repository
2. Migrate skills following the Skill Book structure
3. Update this document with migration status
4. Submit a pull request with CLA signature

See [CONTRIBUTING.md](../CONTRIBUTING.md) for details.
