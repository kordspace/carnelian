# XP System — Agent Progression & Skill Metrics

**Carnelian Core v1.0.0**

The XP System provides exponential agent progression through a 99-level curve, skill-specific metrics tracking, and ledger-backed event logging. Every XP award is immutably recorded, enabling full auditability of agent growth, skill mastery, and milestone unlocks.

---

## Table of Contents

1. [Overview](#overview)
2. [Level Curve Mathematics](#level-curve-mathematics)
3. [XP Sources](#xp-sources)
4. [Elixir XP Integration](#elixir-xp-integration)
5. [Skill Metrics Board](#skill-metrics-board)
6. [Level Milestones](#level-milestones)
7. [Desktop UI](#desktop-ui)
8. [Ledger Integration](#ledger-integration)
9. [API Reference](#api-reference)
10. [Database Schema](#database-schema)
11. [See Also](#see-also)

---

## Overview

The `XpManager` (in `crates/carnelian-core/src/xp.rs`) orchestrates agent progression through task completion, skill usage, and quality performance. All XP awards are written to an immutable event log and correlated with the BLAKE3 hash-chain ledger for tamper-resistant auditability.

### Key Features

✅ **99-level exponential curve** — `1.172^(N-1) × 100` XP formula precomputed into `level_progression`  
✅ **Six XP sources** — task completion, skill usage, ledger signing, quality bonus, elixir creation, elixir approval  
✅ **Skill-level independence** — each skill accrues its own level via `skill_metrics`  
✅ **Ledger-backed event log** — every XP award written to `xp_events`, correlated with `ledger_events`  
✅ **Desktop UI** — top-bar level badge, progress bar, level-up toast, full `XpProgression` page  
✅ **Leaderboard** — all agents ranked by total XP via `GET /v1/xp/leaderboard`  

---

## Level Curve Mathematics

### Formula

The canonical XP curve uses an exponential formula to ensure early levels are accessible while late-game progression remains challenging:

```
XP_for_level(N) = floor(100 × 1.172^(N-1))
```

**Level 1** starts at **0 XP** (no grind to begin).

### Representative Thresholds

| Level | XP Required (this level) | Cumulative XP Required |
|-------|--------------------------|------------------------|
| 1 | 0 | 0 |
| 5 | 188 | 615 |
| 10 | 406 | 2,231 |
| 25 | 3,672 | 27,241 |
| 50 | 142,724 | 1,094,299 |
| 75 | 5,433,584 | 41,653,854 |
| 99 | 155,579,927 | 1,192,598,341 |

**Values computed from migration `00000000000004_xp_curve_retune.sql` formula: `floor(100 × 1.172^(level-1))` for `xp_required`, summed cumulatively for `total_xp_required`.**

### Curve Evolution

The exponent was retuned from **1.15** (migration `00000000000002_phase1_delta.sql`) to **1.172** (migration `00000000000004_xp_curve_retune.sql`):

- **Old curve (1.15):** Topped out at ~680 M cumulative XP at level 99
- **New curve (1.172):** Reaches ~1.19 B cumulative XP at level 99, providing significantly more headroom for long-running agents

The `level_progression` table is a precomputed lookup table queried by `XpManager::award_xp` to find the agent's new level after each award.

### Exponential Growth Visualization

```
XP Required per Level
│
│                                                              ╱
│                                                          ╱
│                                                      ╱
│                                                  ╱
│                                             ╱
│                                        ╱
│                                   ╱
│                              ╱
│                         ╱
│                    ╱
│               ╱
│          ╱
│     ╱
│ ╱
└─────────────────────────────────────────────────────────────────> Level
1    10    20    30    40    50    60    70    80    90    99

Early levels: cheap (100-500 XP)
Mid-game: moderate (1K-100K XP)
Late game: steep (100K-88M XP per level)
```

---

## XP Sources

The `XpSource` enum defines six distinct sources of XP awards, each with its own triggering conditions and amounts.

| Source | `source` Column Value | XP Amount | Trigger |
|--------|----------------------|-----------|---------|
| **Task completion** | `task_completion` | 5 / 15 / 30 XP | Based on task duration: <1 min → 5 XP, 1–5 min → 15 XP, >5 min → 30 XP |
| **Skill usage (first use)** | `skill_usage` | Awarded at first use | `XpManager::is_first_skill_use` guard prevents duplicate awards |
| **Ledger signing** | `ledger_signing` | 10 / 25 / 50 XP | Risk tier: low → 10 XP, medium (capability grants, approvals) → 25 XP, high (`db.migration`, `exec.shell`, `worker.quarantined`) → 50 XP |
| **Quality bonus (daily cron)** | `quality_bonus` | Variable | >95% success rate → +5% of current XP (cap 500); zero errors in 24 h → +50 flat; faster-than-global avg → +10% of current XP (cap 500) |
| **Elixir creation** | `elixir_creation` | 50 XP | On successful `POST /v1/elixirs` |
| **Elixir approval** | `elixir_usage` | 25 XP | On `POST /v1/elixirs/drafts/{id}/approve` |

### Task Completion XP Scaling

Task duration is measured from `started_at` to `completed_at`:

```rust
let xp_amount = match duration_secs {
    0..=59 => 5,        // Quick tasks
    60..=299 => 15,     // Medium tasks (1-5 min)
    _ => 30,            // Long tasks (>5 min)
};
```

### Ledger Signing Risk Tiers

Privileged ledger events are classified by risk:

- **Low risk (10 XP):** Read-only operations, metrics collection
- **Medium risk (25 XP):** Capability grants, elixir approvals, session creation
- **High risk (50 XP):** Database migrations (`db.migration`), shell execution (`exec.shell`), worker quarantine (`worker.quarantined`)

### Quality Bonus Cron

A daily background job evaluates agent performance over the past 24 hours:

```rust
// High success rate bonus
if success_rate > 0.95 {
    bonus_xp += (current_xp as f64 * 0.05).min(500.0) as i32;
}

// Zero-error bonus
if error_count == 0 {
    bonus_xp += 50;
}

// Speed bonus (faster than global average)
if avg_duration_ms < global_avg_duration_ms {
    bonus_xp += (current_xp as f64 * 0.10).min(500.0) as i32;
}
```

---

## Elixir XP Integration

Elixirs intersect with the XP system at three points:

### 1. Direct XP Awards

Creating and approving elixirs grants immediate XP:

- **Elixir creation:** 50 XP (awarded in `crates/carnelian-core/src/elixir.rs` via `XpManager::award_xp`)
- **Elixir approval:** 25 XP (awarded when `POST /v1/elixirs/drafts/{id}/approve` succeeds)

### 2. Quality Score Influence

An elixir's `quality_score` (0–100) feeds into MAGIC's mantra category weight system. When an elixir's `avg_quality > 80.0`, MAGIC applies an **elixir quality boost** to the category weight (see `crates/carnelian-magic/src/mantra.rs`).

This indirectly drives more targeted heartbeat actions, increasing task throughput and thus XP gain rate.

### 3. Elixir XP Boost on Sub-Agent Tasks

The `sub_agent_elixirs` table allows elixirs to be auto-injected into sub-agent contexts. Higher-quality context (from relevant elixirs) correlates with higher task success rates, which feeds the quality-bonus cron:

```
High-quality elixir context
    → Higher task success rate
        → >95% success rate threshold
            → +5% XP daily bonus
```

**Current Implementation vs Requested Spec:**

- **Implemented:** Elixirs provide contextual XP boost via improved task success rates (indirect)
- **Not Implemented:** Direct "+10% XP per active elixir" multiplier does not exist in current code
- **Actual Mechanism:** Quality score > 80.0 boosts MAGIC mantra category weights, increasing task throughput and thus XP gain rate over time

**Cross-reference:** See [ELIXIR_SYSTEM.md](ELIXIR_SYSTEM.md) for elixir lifecycle and quality scoring.

---

## Skill Metrics Board

The `skill_metrics` table tracks per-skill XP, usage counts, and performance statistics. Each skill accrues its own independent level, separate from the agent's global level.

### Schema

```sql
CREATE TABLE skill_metrics (
    metric_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    skill_id        UUID NOT NULL REFERENCES skills(skill_id) ON DELETE CASCADE,
    usage_count     INTEGER NOT NULL DEFAULT 0,
    success_count   INTEGER NOT NULL DEFAULT 0,
    failure_count   INTEGER NOT NULL DEFAULT 0,
    total_duration_ms BIGINT NOT NULL DEFAULT 0,
    avg_duration_ms INTEGER NOT NULL DEFAULT 0,
    total_xp_earned BIGINT NOT NULL DEFAULT 0,
    skill_level     INTEGER NOT NULL DEFAULT 1 CHECK (skill_level >= 1 AND skill_level <= 99),
    last_used_at    TIMESTAMPTZ,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (skill_id)
);
```

### Derived Columns in Desktop UI

The **Skill Levels** tab of `XpProgression` displays:

| Column | Description |
|--------|-------------|
| **Skill Name** | From `skills.name` JOIN |
| **Level** | `skill_level` (derived from `total_xp_earned` against `level_progression`) |
| **Usage Count** | `usage_count` |
| **Success Rate** | `success_count / usage_count × 100` |
| **Avg Duration** | `avg_duration_ms` (ms) |
| **Total XP** | `total_xp_earned` (highlighted green in UI) |
| **🧪** | Present when an active elixir is linked to this skill (`elixirs.skill_id`) |

### Auto-Draft Trigger

`XpManager::update_skill_metrics` upserts this row after every task execution and triggers the **elixir auto-draft check** when `usage_count >= 100`:

```rust
if metrics.usage_count >= 100 && !has_elixir {
    // Create draft elixir from task execution patterns
    elixir_manager.create_draft_from_skill(skill_id).await?;
}
```

---

## Level Milestones

The `level_progression` table includes a `milestone_feature` column that unlocks new capabilities at specific levels.

### Milestone Unlocks

| Level | Milestone Feature | Description |
|-------|-------------------|-------------|
| 5 | `unlock_sub_agents` | Agent can spawn and delegate to sub-agents |
| 10 | `unlock_workflows` | Multi-step workflow orchestration unlocked |
| 15 | `unlock_external_channels` | Telegram and Discord channel adapters enabled |
| 20 | `unlock_voice` | ElevenLabs STT/TTS voice gateway enabled |
| 25 | `master_tier_badge` | Master tier UI badge awarded |
| 50 | `grandmaster_tier` | Grandmaster tier designation |
| 75 | `legend_tier` | Legend tier designation |
| 99 | `max_level_achieved` | Maximum level — all systems fully unlocked |

### UI Integration

The `XpWidget` component displays "Next: `<feature>`" when the agent is approaching a milestone level, providing motivation for continued progression.

---

## Desktop UI

The Desktop UI exposes XP progression through three surfaces: a dedicated page, a top-bar widget, and level-up toasts.

### 7.1 XP Progression Page

**Component:** `XpProgression` (`crates/carnelian-ui/src/pages/xp_progression.rs`)

**Features:**
- **SVG line chart** — Cumulative XP gain over the last 50 events (auto-refreshes every 30 s)
- **Level history table** — Timestamp, source badge, XP gained, task/skill reference
- **Leaderboard table** — Rank, name, level, total XP (current agent row highlighted in blue)
- **Skill Levels table** — Searchable filter input, sortable columns

**Data source:** `GET /v1/xp/agents/:id/history` + `GET /v1/xp/leaderboard` + `GET /v1/xp/skills/top`

### 7.2 Top Bar

**Component:** `TopBar` (`crates/carnelian-ui/src/components/top_bar.rs`)

**Elements (right-hand cluster):**
- `xp-level-badge` span: **"Lv. N"**
- `xp-progress-bar-container` / `xp-progress-bar-fill` — Shows `progress_pct` width
- `xp-progress-label` — Displays total XP count

**Data source:** Polling `GET /v1/xp/agents/:id` every 30 seconds (SSE integration pending)

### 7.3 Level-Up Toast

**Component:** `Toast` (`crates/carnelian-ui/src/components/toast.rs`)  
**Store:** `EventStreamStore` (`crates/carnelian-ui/src/store.rs`)

**Current Implementation:** Level-up toasts are not currently emitted. Desktop UI shows level changes on next poll cycle.

**Planned Implementation:**
- **Trigger:** `XpLevelUp` SSE event from Core
- **Rendering:** Class `toast-level-up` (gold border, bold text)
- **Message:** "🎉 Level Up! Now Level N"
- **Queue:** Up to 10 toasts; oldest drained when overflow

---

## Ledger Integration

Every XP award follows a dual-write pattern to ensure auditability:

```
XP awarded
     │
     ├── INSERT xp_events (source, xp_amount, task_id, skill_id, metadata)
     │
     └── UPDATE agent_xp (total_xp, level, xp_to_next_level)
```

**Current Implementation:** XP events are persisted to the `xp_events` table and `agent_xp` is updated atomically. Level-up detection occurs by comparing the new `total_xp` against `level_progression.total_xp_required`.

**Event Stream Integration:** WebSocket event emission (`XpAwarded`, `LevelUp`) and ledger event writes are not currently implemented in `XpManager`. Desktop UI reactivity relies on polling the `/v1/xp/agents/:id` endpoint rather than SSE events.

### XP Events Schema

```sql
CREATE TABLE xp_events (
    event_id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    identity_id     UUID NOT NULL REFERENCES identities(identity_id) ON DELETE CASCADE,
    source          TEXT NOT NULL CHECK (source IN (
        'task_completion', 'ledger_signing', 'skill_usage',
        'quality_bonus', 'elixir_creation', 'elixir_usage'
    )),
    xp_amount       INTEGER NOT NULL,
    task_id         UUID REFERENCES tasks(task_id) ON DELETE SET NULL,
    skill_id        UUID REFERENCES skills(skill_id) ON DELETE SET NULL,
    ledger_event_id BIGINT,
    elixir_id       UUID,
    metadata        JSONB NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_xp_events_identity ON xp_events(identity_id);
CREATE INDEX idx_xp_events_source ON xp_events(source);
CREATE INDEX idx_xp_events_created ON xp_events(created_at DESC);
CREATE INDEX idx_xp_events_task ON xp_events(task_id);
CREATE INDEX idx_xp_events_skill ON xp_events(skill_id);
```

### Ledger Correlation

The `ledger_event_id` column allows each XP event to be traced back to its corresponding `ledger_events` row, providing full auditability:

```sql
SELECT 
    xe.event_id,
    xe.source,
    xe.xp_amount,
    le.event_hash,
    le.prev_hash,
    le.core_signature
FROM xp_events xe
LEFT JOIN ledger_events le ON xe.ledger_event_id = le.event_id
WHERE xe.identity_id = $1
ORDER BY xe.created_at DESC;
```

**Cross-reference:** See [LEDGER_SYSTEM.md — XP Events](LEDGER_SYSTEM.md#event-types) for the `XpAwarded` and `LevelUp` ledger event type definitions.

---

## API Reference

### Quick Reference

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/xp/agents/:id` | GET | Agent XP, level, and next-level progress |
| `/v1/xp/agents/:id/history` | GET | Paginated XP event log |
| `/v1/xp/leaderboard` | GET | All agents ranked by total XP |
| `/v1/xp/skills/:id` | GET | Skill metrics and level for one skill |
| `/v1/xp/skills/top` | GET | Top skills by total XP (max 50) |
| `/v1/xp/award` | POST | Manually award XP (requires Ed25519 signature) |

**Note:** `POST /v1/xp/events` is not currently implemented. XP events are created internally by `XpManager::award_xp` and can be queried via `GET /v1/xp/agents/:id/history`.

### Get Agent XP

```bash
curl http://localhost:18789/v1/xp/agents/01936a1a-... \
  -H "X-Carnelian-Key: $KEY"
```

**Response (200 OK):**
```json
{
  "identity_id": "01936a1a-...",
  "total_xp": 12450,
  "level": 18,
  "xp_to_next_level": 1234,
  "progress_pct": 36.0,
  "milestone_feature": "unlock_voice"
}
```

### Get XP History

```bash
curl "http://localhost:18789/v1/xp/agents/01936a1a-.../history?page=1&page_size=25" \
  -H "X-Carnelian-Key: $KEY"
```

**Response (200 OK):**
```json
{
  "events": [
    {
      "event_id": 12345,
      "source": "task_completion",
      "xp_amount": 30,
      "task_id": "01936a1e-...",
      "skill_id": null,
      "ledger_event_id": null,
      "metadata": {},
      "created_at": "2026-03-04T10:30:00Z"
    },
    {
      "event_id": 12344,
      "source": "elixir_creation",
      "xp_amount": 50,
      "task_id": null,
      "skill_id": null,
      "ledger_event_id": null,
      "metadata": {},
      "created_at": "2026-03-04T09:15:00Z"
    }
  ],
  "page": 1,
  "page_size": 25,
  "total": 342
}
```

### Get Leaderboard

```bash
curl http://localhost:18789/v1/xp/leaderboard \
  -H "X-Carnelian-Key: $KEY"
```

**Response (200 OK):**
```json
{
  "entries": [
    {
      "rank": 1,
      "identity_id": "01936a20-...",
      "name": "Lian",
      "total_xp": 45230,
      "level": 25
    },
    {
      "rank": 2,
      "identity_id": "01936a21-...",
      "name": "SubAgent-Alpha",
      "total_xp": 12450,
      "level": 18
    }
  ]
}
```

### Get Skill Metrics

```bash
curl http://localhost:18789/v1/xp/skills/01936a22-... \
  -H "X-Carnelian-Key: $KEY"
```

**Response (200 OK):**
```json
{
  "skill_id": "01936a22-...",
  "skill_name": "read_file",
  "usage_count": 342,
  "success_count": 338,
  "failure_count": 4,
  "success_rate": 98.8,
  "avg_duration_ms": 125,
  "total_xp_earned": 1542,
  "skill_level": 12,
  "last_used_at": "2026-03-04T10:30:00Z"
}
```

### Get Top Skills

```bash
curl http://localhost:18789/v1/xp/skills/top \
  -H "X-Carnelian-Key: $KEY"
```

**Response (200 OK):**
```json
{
  "skills": [
    {
      "skill_id": "01936a22-...",
      "skill_name": "read_file",
      "total_xp_earned": 1542,
      "skill_level": 12,
      "usage_count": 342
    },
    {
      "skill_id": "01936a23-...",
      "skill_name": "grep_search",
      "total_xp_earned": 987,
      "skill_level": 10,
      "usage_count": 215
    }
  ]
}
```

### Award XP Manually

```bash
curl -X POST http://localhost:18789/v1/xp/award \
  -H "X-Carnelian-Key: $KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "identity_id": "01936a1a-...",
    "source": "quality_bonus",
    "xp_amount": 100,
    "signature": "<ed25519_signature_hex>",
    "metadata": {}
  }'
```

**Response (200 OK):**
```json
{
  "identity_id": "01936a1a-...",
  "xp_awarded": 100,
  "new_total_xp": 12550,
  "level_up": null
}
```

**Note:** The `signature` field is required and must be an Ed25519 signature over the canonical message `"{identity_id}:{xp_amount}:{source}"` using the owner signing key.

### WebSocket Events

**Current Implementation:** XP-related WebSocket events are not currently emitted by `XpManager`.

**Planned Events:**

| EventType | Payload | Description |
|-----------|---------|-------------|
| `XpAwarded` | `{ "identity_id": "...", "source": "task_completion", "xp_amount": 30, "new_total_xp": 12450, "new_level": 18 }` | XP awarded to agent |
| `LevelUp` | `{ "identity_id": "...", "old_level": 17, "new_level": 18, "total_xp": 12450, "milestone_feature": null }` | Agent leveled up |

**Workaround:** Desktop UI polls `GET /v1/xp/agents/:id` every 30 seconds for XP state updates.

---

## Database Schema

### 1. Level Progression (Precomputed Lookup)

```sql
CREATE TABLE level_progression (
    level               INTEGER PRIMARY KEY CHECK (level >= 1 AND level <= 99),
    xp_required         BIGINT NOT NULL,
    cumulative_xp       BIGINT NOT NULL,
    milestone_feature   TEXT
);

-- Populated by migration 00000000000004_xp_curve_retune.sql
-- Formula: xp_required = floor(100 × 1.172^(level-1))
```

### 2. Agent XP (Per-Agent Totals)

```sql
CREATE TABLE agent_xp (
    xp_id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    identity_id     UUID NOT NULL REFERENCES identities(identity_id) ON DELETE CASCADE,
    total_xp        BIGINT NOT NULL DEFAULT 0,
    level           INTEGER NOT NULL DEFAULT 1 CHECK (level >= 1 AND level <= 99),
    xp_to_next_level BIGINT NOT NULL DEFAULT 115,  -- Updated to 117 by migration 0004
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (identity_id)
);

CREATE INDEX idx_agent_xp_identity ON agent_xp(identity_id);
CREATE INDEX idx_agent_xp_level ON agent_xp(level DESC);
CREATE INDEX idx_agent_xp_total_xp ON agent_xp(total_xp DESC);
```

**Note:** Runtime code queries by `identity_id` (UNIQUE constraint), not by `xp_id` primary key.

### 3. Skill Metrics (Per-Skill Counters)

See [§ 5 — Skill Metrics Board](#skill-metrics-board) for the complete schema.

### 4. XP Events (Immutable Event Log)

See [§ 8 — Ledger Integration](#ledger-integration) for the complete schema.

---

## See Also

- **[LEDGER_SYSTEM.md](LEDGER_SYSTEM.md)** — Hash-chain audit trail; XpAwarded & LevelUp event definitions
- **[ELIXIR_SYSTEM.md](ELIXIR_SYSTEM.md)** — Elixir quality scoring and XP integration
- **[API.md](API.md)** — Complete XP endpoint reference
- **[ARCHITECTURE.md](ARCHITECTURE.md)** — System overview

---

**Last Updated:** March 4, 2026  
**Version:** 1.0.0
