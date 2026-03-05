# Elixir System — Knowledge Persistence & Dataset Injection

**Carnelian Core v1.0.0**

The Elixir System is Carnelian's RAG-based knowledge persistence layer, providing long-term memory, dataset injection capabilities, and automated knowledge capture from successful task patterns.

---

## Table of Contents

1. [Overview](#overview)
2. [Core Concepts](#core-concepts)
3. [Elixir Types](#elixir-types)
4. [Dataset Structure](#dataset-structure)
5. [Brewing System — Auto-Draft Generation](#brewing-system--auto-draft-generation)
6. [Quality Scoring](#quality-scoring)
7. [Injection Capabilities](#injection-capabilities)
8. [Quantum Integrity](#quantum-integrity)
9. [API Reference](#api-reference)
10. [Database Schema](#database-schema)
11. [Usage Examples](#usage-examples)
12. [Best Practices](#best-practices)

---

## Overview

Elixirs are **knowledge artifacts** that capture domain expertise, skill patterns, context caches, and training data. They enable:

- **Long-term memory persistence** — Store knowledge beyond session boundaries
- **Context injection** — Dynamically inject relevant knowledge into LLM prompts
- **Pattern capture** — Auto-generate elixirs from successful task execution patterns
- **Semantic search** — pgvector-powered similarity search across knowledge base
- **Quality tracking** — Scoring system with XP integration for effectiveness measurement
- **Version control** — Full version history with rollback capabilities

### Key Features

✅ **Four elixir types** — skill_backup, domain_knowledge, context_cache, training_data  
✅ **Auto-brewing** — Automatic draft generation from high-usage skills (100+ executions)  
✅ **Approval workflow** — Human-in-the-loop review for auto-generated drafts  
✅ **pgvector embeddings** — 1536-dimensional semantic search  
✅ **Quantum checksums** — BLAKE3 + MAGIC entropy for tamper-resistant integrity  
✅ **XP integration** — Quality scores influence XP awards and skill effectiveness  
✅ **Mantra linking** — Elixirs can be referenced by mantras for context weighting  

---

## Core Concepts

### What is an Elixir?

An **elixir** is a structured knowledge container with:

- **Metadata** — Name, description, type, icon, creator, skill association
- **Dataset** — JSON payload containing the actual knowledge content
- **Embedding** — 1536-dimensional pgvector for semantic search
- **Quality Score** — 0-100 effectiveness rating
- **Integrity Hash** — BLAKE3 checksum (optionally quantum-enhanced)
- **Version History** — Full audit trail of changes

### Lifecycle

```
┌─────────────────┐
│  Task Execution │
│   (100+ uses)   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Auto-Draft     │◄─── Brewing System
│  Generation     │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Human Review   │◄─── Approval Queue
│  (Approve/Reject)│
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Active Elixir  │◄─── Knowledge Base
│  (Searchable)   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Context        │◄─── LLM Prompt Assembly
│  Injection      │
└─────────────────┘
```

---

## Elixir Types

### 1. `skill_backup`

**Purpose:** Preserve skill implementation details, configuration, and execution patterns.

**Use Cases:**
- Backup skill code before major refactoring
- Document skill-specific configuration patterns
- Capture successful execution workflows

**Dataset Structure:**
```json
{
  "skill_name": "git-commit",
  "implementation": "...",
  "config_schema": {...},
  "execution_patterns": [...]
}
```

### 2. `domain_knowledge`

**Purpose:** Store domain-specific expertise, best practices, and reference material.

**Use Cases:**
- Programming language patterns (e.g., "Rust async patterns")
- API documentation and usage examples
- Architecture decision records
- Security best practices

**Dataset Structure:**
```json
{
  "domain": "rust-async",
  "content": "Comprehensive guide to async patterns in Rust...",
  "examples": [...],
  "references": [...]
}
```

### 3. `context_cache`

**Purpose:** Cache frequently-used context snippets for faster prompt assembly.

**Use Cases:**
- Common error resolution patterns
- Frequently referenced code snippets
- Standard operating procedures
- Troubleshooting guides

**Dataset Structure:**
```json
{
  "context_type": "error_resolution",
  "trigger_patterns": ["ECONNREFUSED", "connection refused"],
  "resolution_steps": [...],
  "related_skills": [...]
}
```

### 4. `training_data`

**Purpose:** Capture training examples for fine-tuning or few-shot learning.

**Use Cases:**
- Successful task completion examples
- Error recovery patterns
- User interaction patterns
- Skill execution traces

**Dataset Structure:**
```json
{
  "task_type": "code_review",
  "input_examples": [...],
  "output_examples": [...],
  "success_metrics": {...}
}
```

---

## Dataset Structure

Elixir datasets are **flexible JSON payloads** stored in the `dataset` JSONB column. While structure varies by type, common patterns include:

### Minimal Dataset

```json
{
  "content": "Knowledge content goes here",
  "metadata": {
    "source": "manual",
    "created_by": "marco",
    "tags": ["rust", "async", "patterns"]
  }
}
```

### Rich Dataset

```json
{
  "content": "Comprehensive guide to Rust async patterns",
  "sections": [
    {
      "title": "Futures and Polling",
      "content": "...",
      "code_examples": [...]
    },
    {
      "title": "Tokio Runtime",
      "content": "...",
      "code_examples": [...]
    }
  ],
  "metadata": {
    "source": "auto_generated",
    "skill_id": "01936a1a-...",
    "execution_count": 150,
    "success_rate": 0.94,
    "tags": ["rust", "async", "tokio", "futures"]
  },
  "references": [
    {"url": "https://tokio.rs", "title": "Tokio Documentation"},
    {"url": "https://rust-lang.github.io/async-book/", "title": "Async Book"}
  ]
}
```

### Builder Pattern Dataset

For auto-generated elixirs from task patterns:

```json
{
  "pattern_type": "successful_execution",
  "skill_id": "01936a1a-...",
  "execution_samples": [
    {
      "task_id": "01936a1b-...",
      "input": {...},
      "output": {...},
      "duration_ms": 1250,
      "success": true
    }
  ],
  "aggregated_insights": {
    "common_inputs": [...],
    "success_factors": [...],
    "failure_modes": [...]
  },
  "suggested_improvements": [...]
}
```

---

## Brewing System — Auto-Draft Generation

The **brewing system** automatically generates elixir drafts from successful task patterns, reducing manual knowledge capture overhead.

### Trigger Conditions

Auto-draft generation occurs when:

1. **Skill reaches 100+ executions** (configurable threshold)
2. **No pending draft exists** for that skill
3. **No active elixir exists** for that skill
4. **Success rate ≥ 70%** (quality gate)

### Draft Generation Process

```rust
// Simplified brewing logic
pub async fn auto_brew_draft(
    pool: &PgPool,
    skill_id: Uuid,
    execution_samples: Vec<TaskRun>,
) -> Result<Uuid> {
    // 1. Analyze execution patterns
    let patterns = analyze_execution_patterns(&execution_samples);
    
    // 2. Compute quality score
    let quality_score = compute_quality_score(&patterns);
    
    // 3. Build dataset
    let dataset = build_dataset_from_patterns(patterns);
    
    // 4. Generate embedding
    let embedding = generate_embedding(&dataset).await?;
    
    // 5. Create draft record
    let draft_id = sqlx::query_scalar(
        "INSERT INTO elixir_drafts (skill_id, proposed_name, elixir_type, 
         dataset, quality_score, auto_generated) 
         VALUES ($1, $2, $3, $4, $5, true) 
         RETURNING draft_id"
    )
    .bind(skill_id)
    .bind(format!("{}-patterns", skill_name))
    .bind("skill_backup")
    .bind(dataset)
    .bind(quality_score)
    .fetch_one(pool)
    .await?;
    
    Ok(draft_id)
}
```

### Approval Workflow

1. **Draft appears in approval queue** — `GET /v1/elixirs/drafts`
2. **Human reviews draft** — Inspect dataset, quality score, metadata
3. **Approve or reject:**
   - **Approve** → `POST /v1/elixirs/drafts/{id}/approve` — Creates active elixir
   - **Reject** → `POST /v1/elixirs/drafts/{id}/reject` — Marks draft as rejected

### Quality Score Calculation

```rust
fn compute_quality_score(patterns: &ExecutionPatterns) -> f64 {
    let success_rate = patterns.success_count as f64 / patterns.total_count as f64;
    let consistency = 1.0 - patterns.variance;
    let coverage = patterns.unique_inputs as f64 / patterns.total_count as f64;
    
    // Weighted average
    (success_rate * 0.5) + (consistency * 0.3) + (coverage * 0.2) * 100.0
}
```

---

## Quality Scoring

Quality scores (0-100) influence:

- **XP awards** — Higher quality elixirs grant more XP when used
- **Search ranking** — Better scores boost semantic search results
- **Auto-approval** — Scores ≥ 85 may auto-approve in future versions
- **Retention** — Low-quality elixirs (<40) flagged for review

### Score Components

| Component | Weight | Description |
|-----------|--------|-------------|
| **Success Rate** | 50% | Percentage of successful task executions |
| **Consistency** | 30% | Low variance in execution patterns |
| **Coverage** | 20% | Diversity of input/output examples |

### Score Decay

Quality scores decay over time if elixirs aren't used:

- **30 days unused** → -5 points
- **90 days unused** → -15 points
- **180 days unused** → -30 points (flagged for archival)

---

## Injection Capabilities

Elixirs are injected into LLM context through multiple pathways:

### 1. Semantic Search Injection

```rust
// Context assembler automatically retrieves relevant elixirs
let relevant_elixirs = elixir_manager
    .search_elixirs(query_text, limit: 3)
    .await?;

for elixir in relevant_elixirs {
    context.add_section(format!(
        "## Knowledge: {}\n\n{}",
        elixir.name,
        elixir.dataset["content"]
    ));
}
```

### 2. Skill-Linked Injection

When a skill executes, its linked elixir is automatically injected:

```sql
SELECT e.* FROM elixirs e
WHERE e.skill_id = $1 AND e.active = true
ORDER BY e.quality_score DESC
LIMIT 1
```

### 3. Mantra-Referenced Injection

Mantras can reference elixirs for weighted context:

```rust
// Mantra entry with elixir reference
{
    "text": "What patterns emerge from recent errors?",
    "category": "Reflection",
    "elixir_id": "01936a20-..."  // Links to error-patterns elixir
}
```

When this mantra is selected, its linked elixir is injected with higher weight.

### 4. Explicit API Injection

Manual injection via API:

```bash
curl -X POST http://localhost:18789/v1/context/inject \
  -H "X-Carnelian-Key: $KEY" \
  -d '{"elixir_ids": ["01936a20-...", "01936a21-..."]}'
```

---

## Quantum Integrity

Elixirs use **quantum-enhanced checksums** for tamper-resistant integrity verification.

### Checksum Computation

```rust
use carnelian_magic::QuantumHasher;

let quantum_salt = entropy_provider.sample_bytes(16).await?;
let hasher = QuantumHasher::new(quantum_salt);
let checksum = hasher.hash_elixir_dataset(&dataset);

// Store in security_integrity_hash column
sqlx::query(
    "UPDATE elixirs SET security_integrity_hash = $1 WHERE elixir_id = $2"
)
.bind(checksum)
.bind(elixir_id)
.execute(pool)
.await?;
```

### Verification

```bash
# Verify all elixirs
curl -X POST http://localhost:18789/v1/magic/integrity/verify \
  -H "X-Carnelian-Key: $KEY" \
  -d '{"tables": ["elixirs"]}'

# Response
{
  "results": [
    {
      "table": "elixirs",
      "total_rows": 42,
      "verified": 42,
      "tampered": 0,
      "missing_checksums": 0
    }
  ]
}
```

### Rehashing

Periodically rehash elixirs with fresh quantum entropy:

```bash
curl -X POST http://localhost:18789/v1/magic/elixirs/rehash \
  -H "X-Carnelian-Key: $KEY"

# Response
{
  "message": "Rehashed all elixirs with fresh entropy",
  "rehashed": 42
}
```

---

## API Reference

See [API.md](API.md) for complete endpoint documentation.

### Quick Reference

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/elixirs` | GET | List elixirs with pagination and filtering |
| `/v1/elixirs` | POST | Create new elixir |
| `/v1/elixirs/{id}` | GET | Get single elixir by ID |
| `/v1/elixirs/search` | GET | Semantic search using pgvector |
| `/v1/elixirs/drafts` | GET | List pending drafts |
| `/v1/elixirs/drafts/{id}/approve` | POST | Approve draft and create elixir |
| `/v1/elixirs/drafts/{id}/reject` | POST | Reject draft |

---

## Database Schema

### `elixirs` Table

```sql
CREATE TABLE elixirs (
    elixir_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    description TEXT,
    elixir_type TEXT NOT NULL CHECK (elixir_type IN (
        'skill_backup', 'domain_knowledge', 'context_cache', 'training_data'
    )),
    icon TEXT DEFAULT '🧪',
    created_by UUID REFERENCES identities(identity_id),
    skill_id UUID REFERENCES skills(skill_id),
    dataset JSONB NOT NULL,
    embedding vector(1536),
    size_bytes BIGINT DEFAULT 0,
    version INTEGER DEFAULT 1,
    quality_score REAL DEFAULT 0.0,
    security_integrity_hash TEXT,
    quantum_checksum TEXT,
    active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_elixirs_embedding ON elixirs USING ivfflat (embedding vector_cosine_ops);
CREATE INDEX idx_elixirs_skill_id ON elixirs(skill_id);
CREATE INDEX idx_elixirs_type ON elixirs(elixir_type);
CREATE INDEX idx_elixirs_active ON elixirs(active);
```

### `elixir_versions` Table

```sql
CREATE TABLE elixir_versions (
    version_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    elixir_id UUID NOT NULL REFERENCES elixirs(elixir_id) ON DELETE CASCADE,
    version_number INTEGER NOT NULL,
    dataset JSONB NOT NULL,
    quality_score REAL,
    changed_by UUID REFERENCES identities(identity_id),
    change_reason TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

### `elixir_usage` Table

```sql
CREATE TABLE elixir_usage (
    usage_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    elixir_id UUID NOT NULL REFERENCES elixirs(elixir_id) ON DELETE CASCADE,
    task_id UUID REFERENCES tasks(task_id),
    context_type TEXT, -- 'semantic_search', 'skill_linked', 'mantra_referenced', 'explicit'
    effectiveness_score REAL, -- 0.0-1.0 rating of how helpful it was
    used_at TIMESTAMPTZ DEFAULT NOW()
);
```

### `elixir_drafts` Table

```sql
CREATE TABLE elixir_drafts (
    draft_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    skill_id UUID REFERENCES skills(skill_id),
    proposed_name TEXT NOT NULL,
    elixir_type TEXT NOT NULL,
    dataset JSONB NOT NULL,
    quality_score REAL,
    auto_generated BOOLEAN DEFAULT false,
    status TEXT DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected')),
    reviewed_by UUID REFERENCES identities(identity_id),
    reviewed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

---

## Usage Examples

### Create Domain Knowledge Elixir

```bash
curl -X POST http://localhost:18789/v1/elixirs \
  -H "X-Carnelian-Key: $KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "rust-async-patterns",
    "description": "Comprehensive guide to async patterns in Rust",
    "elixir_type": "domain_knowledge",
    "dataset": {
      "content": "Async patterns in Rust revolve around Futures, async/await syntax, and the Tokio runtime...",
      "sections": [
        {
          "title": "Futures and Polling",
          "content": "...",
          "code_examples": ["..."]
        }
      ],
      "metadata": {
        "tags": ["rust", "async", "tokio"],
        "difficulty": "intermediate"
      }
    }
  }'
```

### Search for Relevant Elixirs

```bash
curl -X GET "http://localhost:18789/v1/elixirs/search?query=async+error+handling&limit=5" \
  -H "X-Carnelian-Key: $KEY"
```

### Review and Approve Draft

```bash
# List drafts
curl -X GET http://localhost:18789/v1/elixirs/drafts \
  -H "X-Carnelian-Key: $KEY"

# Approve specific draft
curl -X POST http://localhost:18789/v1/elixirs/drafts/01936a1c-.../approve \
  -H "X-Carnelian-Key: $KEY"
```

---

## Best Practices

### 1. Dataset Design

✅ **DO:**
- Use structured JSON with clear sections
- Include metadata for filtering and search
- Add tags for categorization
- Provide code examples where applicable
- Include references and sources

❌ **DON'T:**
- Store binary data (use external storage + reference)
- Exceed 1MB dataset size (performance impact)
- Duplicate content across multiple elixirs
- Omit metadata (reduces searchability)

### 2. Quality Management

✅ **DO:**
- Review auto-generated drafts before approval
- Update quality scores based on usage feedback
- Archive unused elixirs after 180 days
- Version elixirs when making significant changes

❌ **DON'T:**
- Auto-approve low-quality drafts (<60 score)
- Keep duplicate elixirs active
- Ignore quality decay warnings

### 3. Injection Strategy

✅ **DO:**
- Use semantic search for dynamic context
- Link elixirs to related skills
- Reference high-quality elixirs in mantras
- Monitor injection effectiveness via usage tracking

❌ **DON'T:**
- Inject too many elixirs (context bloat)
- Inject irrelevant knowledge (noise)
- Ignore injection performance metrics

### 4. Security

✅ **DO:**
- Enable quantum checksums for sensitive elixirs
- Verify integrity periodically
- Rehash with fresh entropy quarterly
- Audit elixir access patterns

❌ **DON'T:**
- Store credentials in elixir datasets
- Disable integrity verification
- Share elixirs across trust boundaries without review

---

## See Also

- **[MAGIC.md](MAGIC.md)** — Quantum entropy system for checksums
- **[API.md](API.md)** — Complete API reference
- **[ARCHITECTURE.md](ARCHITECTURE.md)** — System architecture overview
- **[SECURITY.md](SECURITY.md)** — Security best practices

---

**Last Updated:** March 3, 2026  
**Version:** 1.0.0
