-- =============================================================================
-- Phase 1 Delta Migration - Sessions, Workflows, XP System, and Elixirs
-- =============================================================================
--
-- This migration adds missing tables from the roadmap specifications to complete
-- the Phase 1 foundation. It introduces:
--   - Unified session management (sessions, session_messages)
--   - Skill versioning (skill_versions)
--   - Channel sessions for external integrations (channel_sessions)
--   - Workflow definitions (workflows)
--   - Sub-agent lifecycle (sub_agents)
--   - XP progression system with 99-level exponential curve
--   - Elixirs RAG system for knowledge persistence
--
-- Follows established patterns: UUID primary keys, TIMESTAMPTZ timestamps,
-- JSONB for flexible data, CHECK constraints for enums, pgvector for embeddings.
-- =============================================================================


-- =============================================================================
-- SESSIONS
-- =============================================================================

CREATE TABLE sessions (
    session_id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_key     TEXT UNIQUE NOT NULL,
    agent_id        UUID NOT NULL REFERENCES identities(identity_id) ON DELETE CASCADE,
    channel         TEXT NOT NULL DEFAULT 'local',
    transcript_path TEXT,
    token_counters  JSONB NOT NULL DEFAULT '{"total": 0, "user": 0, "assistant": 0, "tool": 0}',
    compaction_count    INTEGER NOT NULL DEFAULT 0,
    context_window_limit INTEGER,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_activity_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ
);

CREATE INDEX idx_sessions_agent ON sessions(agent_id);
CREATE INDEX idx_sessions_channel ON sessions(channel);
CREATE INDEX idx_sessions_key ON sessions(session_key);
CREATE INDEX idx_sessions_activity ON sessions(last_activity_at DESC);

CREATE TABLE session_messages (
    message_id      BIGSERIAL PRIMARY KEY,
    session_id      UUID NOT NULL REFERENCES sessions(session_id) ON DELETE CASCADE,
    ts              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    role            TEXT NOT NULL CHECK (role IN ('system', 'user', 'assistant', 'tool')),
    content         TEXT NOT NULL,
    tool_name       TEXT,
    tool_call_id    TEXT,
    correlation_id  UUID,
    token_estimate  INTEGER,
    metadata        JSONB NOT NULL DEFAULT '{}',
    tool_metadata   JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX idx_session_messages_session_ts ON session_messages(session_id, ts DESC);
CREATE INDEX idx_session_messages_correlation ON session_messages(correlation_id);


-- =============================================================================
-- SKILL VERSIONING
-- =============================================================================

CREATE TABLE skill_versions (
    version_id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    skill_id        UUID NOT NULL REFERENCES skills(skill_id) ON DELETE CASCADE,
    version         TEXT NOT NULL,
    manifest        JSONB NOT NULL,
    checksum        TEXT NOT NULL,
    signature       TEXT,
    published_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (skill_id, version)
);

CREATE INDEX idx_skill_versions_skill ON skill_versions(skill_id);
CREATE INDEX idx_skill_versions_published ON skill_versions(published_at DESC);


-- =============================================================================
-- CHANNEL SESSIONS
-- =============================================================================

CREATE TABLE channel_sessions (
    session_id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_type    TEXT NOT NULL CHECK (channel_type IN ('telegram', 'discord', 'whatsapp', 'slack', 'ui')),
    channel_user_id TEXT NOT NULL,
    trust_level     TEXT NOT NULL DEFAULT 'untrusted' CHECK (trust_level IN ('conversational', 'untrusted', 'owner')),
    identity_id     UUID REFERENCES identities(identity_id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata        JSONB NOT NULL DEFAULT '{}',
    UNIQUE (channel_type, channel_user_id)
);

CREATE INDEX idx_channel_sessions_type ON channel_sessions(channel_type);
CREATE INDEX idx_channel_sessions_trust ON channel_sessions(trust_level);
CREATE INDEX idx_channel_sessions_identity ON channel_sessions(identity_id);


-- =============================================================================
-- WORKFLOWS
-- =============================================================================

CREATE TABLE workflows (
    workflow_id     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            TEXT UNIQUE NOT NULL,
    description     TEXT,
    created_by      UUID REFERENCES identities(identity_id) ON DELETE SET NULL,
    skill_chain     JSONB NOT NULL DEFAULT '[]',
    enabled         BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_workflows_enabled ON workflows(enabled);
CREATE INDEX idx_workflows_created_by ON workflows(created_by);


-- =============================================================================
-- SUB-AGENTS
-- =============================================================================

CREATE TABLE sub_agents (
    sub_agent_id    UUID PRIMARY KEY REFERENCES identities(identity_id) ON DELETE CASCADE,
    parent_id       UUID NOT NULL REFERENCES identities(identity_id) ON DELETE CASCADE,
    created_by      UUID NOT NULL REFERENCES identities(identity_id) ON DELETE CASCADE,
    model_provider  UUID REFERENCES model_providers(provider_id) ON DELETE SET NULL,
    name            TEXT NOT NULL,
    role            TEXT NOT NULL,
    directives      JSONB,
    ephemeral       BOOLEAN NOT NULL DEFAULT false,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_active_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    terminated_at   TIMESTAMPTZ
);

CREATE INDEX idx_sub_agents_parent ON sub_agents(parent_id);
CREATE INDEX idx_sub_agents_created_by ON sub_agents(created_by);
CREATE INDEX idx_sub_agents_active ON sub_agents(sub_agent_id) WHERE terminated_at IS NULL;
CREATE INDEX idx_sub_agents_role ON sub_agents(role);


-- =============================================================================
-- XP SYSTEM
-- =============================================================================

-- Precomputed level progression lookup table (levels 1-99)
-- Formula: XP_for_level(N) = floor(100 * (1.15^(N-1))), Level 1 = 0
CREATE TABLE level_progression (
    level               INTEGER PRIMARY KEY CHECK (level >= 1 AND level <= 99),
    xp_required         BIGINT NOT NULL,
    total_xp_required   BIGINT NOT NULL,
    milestone_feature   TEXT
);

CREATE INDEX idx_level_progression_total_xp ON level_progression(total_xp_required);

-- Per-agent XP tracking
CREATE TABLE agent_xp (
    xp_id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    identity_id     UUID NOT NULL REFERENCES identities(identity_id) ON DELETE CASCADE,
    total_xp        BIGINT NOT NULL DEFAULT 0,
    level           INTEGER NOT NULL DEFAULT 1 CHECK (level >= 1 AND level <= 99),
    xp_to_next_level BIGINT NOT NULL DEFAULT 115,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (identity_id)
);

CREATE INDEX idx_agent_xp_identity ON agent_xp(identity_id);
CREATE INDEX idx_agent_xp_level ON agent_xp(level DESC);
CREATE INDEX idx_agent_xp_total_xp ON agent_xp(total_xp DESC);

-- Per-skill metrics with XP tracking
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

CREATE INDEX idx_skill_metrics_skill ON skill_metrics(skill_id);
CREATE INDEX idx_skill_metrics_usage ON skill_metrics(usage_count DESC);
CREATE INDEX idx_skill_metrics_success_rate ON skill_metrics(((success_count::REAL / NULLIF(usage_count, 0)::REAL)));
CREATE INDEX idx_skill_metrics_xp ON skill_metrics(total_xp_earned DESC);
CREATE INDEX idx_skill_metrics_level ON skill_metrics(skill_level DESC);

-- XP event log
CREATE TABLE xp_events (
    event_id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    identity_id     UUID NOT NULL REFERENCES identities(identity_id) ON DELETE CASCADE,
    source          TEXT NOT NULL CHECK (source IN ('task_completion', 'ledger_signing', 'skill_usage', 'quality_bonus', 'elixir_creation', 'elixir_usage')),
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


-- =============================================================================
-- ELIXIRS SYSTEM
-- =============================================================================

-- Core elixirs table for RAG-based knowledge persistence
CREATE TABLE elixirs (
    elixir_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            TEXT UNIQUE NOT NULL,
    description     TEXT,
    elixir_type     TEXT NOT NULL CHECK (elixir_type IN ('skill_backup', 'domain_knowledge', 'context_cache', 'training_data')),
    icon            TEXT NOT NULL DEFAULT '🧪',
    created_by      UUID REFERENCES identities(identity_id) ON DELETE SET NULL,
    skill_id        UUID REFERENCES skills(skill_id) ON DELETE SET NULL,
    dataset         JSONB NOT NULL,
    embedding       vector(1536),
    size_bytes      BIGINT NOT NULL DEFAULT 0,
    version         INTEGER NOT NULL DEFAULT 1,
    quality_score   REAL NOT NULL DEFAULT 0.0 CHECK (quality_score >= 0.0 AND quality_score <= 100.0),
    security_integrity_hash TEXT,
    active          BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_elixirs_type ON elixirs(elixir_type);
CREATE INDEX idx_elixirs_skill ON elixirs(skill_id);
CREATE INDEX idx_elixirs_active ON elixirs(active);
CREATE INDEX idx_elixirs_quality ON elixirs(quality_score DESC);
CREATE INDEX idx_elixirs_embedding ON elixirs USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

-- Elixir version history
CREATE TABLE elixir_versions (
    version_id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    elixir_id       UUID NOT NULL REFERENCES elixirs(elixir_id) ON DELETE CASCADE,
    version_number  INTEGER NOT NULL,
    dataset         JSONB NOT NULL,
    embedding       vector(1536),
    change_description TEXT,
    created_by      UUID REFERENCES identities(identity_id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (elixir_id, version_number)
);

CREATE INDEX idx_elixir_versions_elixir ON elixir_versions(elixir_id);
CREATE INDEX idx_elixir_versions_created ON elixir_versions(created_at DESC);

-- Elixir usage tracking
CREATE TABLE elixir_usage (
    usage_id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    elixir_id       UUID NOT NULL REFERENCES elixirs(elixir_id) ON DELETE CASCADE,
    used_by         UUID NOT NULL REFERENCES identities(identity_id) ON DELETE CASCADE,
    task_id         UUID REFERENCES tasks(task_id) ON DELETE SET NULL,
    effectiveness_score REAL CHECK (effectiveness_score >= 0.0 AND effectiveness_score <= 1.0),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_elixir_usage_elixir ON elixir_usage(elixir_id);
CREATE INDEX idx_elixir_usage_agent ON elixir_usage(used_by);
CREATE INDEX idx_elixir_usage_task ON elixir_usage(task_id);
CREATE INDEX idx_elixir_usage_created ON elixir_usage(created_at DESC);

-- Sub-agent elixir assignments
CREATE TABLE sub_agent_elixirs (
    assignment_id   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sub_agent_id    UUID NOT NULL REFERENCES sub_agents(sub_agent_id) ON DELETE CASCADE,
    elixir_id       UUID NOT NULL REFERENCES elixirs(elixir_id) ON DELETE CASCADE,
    auto_inject     BOOLEAN NOT NULL DEFAULT true,
    priority        INTEGER NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (sub_agent_id, elixir_id)
);

CREATE INDEX idx_sub_agent_elixirs_sub_agent ON sub_agent_elixirs(sub_agent_id);
CREATE INDEX idx_sub_agent_elixirs_elixir ON sub_agent_elixirs(elixir_id);
CREATE INDEX idx_sub_agent_elixirs_priority ON sub_agent_elixirs(sub_agent_id, priority DESC);

-- Elixir drafts (auto-generated proposals awaiting review)
CREATE TABLE elixir_drafts (
    draft_id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    skill_id        UUID NOT NULL REFERENCES skills(skill_id) ON DELETE CASCADE,
    proposed_name   TEXT NOT NULL,
    proposed_description TEXT,
    dataset         JSONB NOT NULL,
    embedding       vector(1536),
    auto_created_reason TEXT,
    status          TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected')),
    reviewed_by     UUID REFERENCES identities(identity_id) ON DELETE SET NULL,
    reviewed_at     TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_elixir_drafts_skill ON elixir_drafts(skill_id);
CREATE INDEX idx_elixir_drafts_status ON elixir_drafts(status);
CREATE INDEX idx_elixir_drafts_created ON elixir_drafts(created_at DESC);


-- =============================================================================
-- FOREIGN KEY CONSTRAINT FOR XP EVENTS → ELIXIRS
-- =============================================================================
-- Added after elixirs table is created to avoid forward reference

ALTER TABLE xp_events
    ADD CONSTRAINT fk_xp_events_elixir
    FOREIGN KEY (elixir_id) REFERENCES elixirs(elixir_id) ON DELETE SET NULL;


-- =============================================================================
-- SEED DATA: Level Progression (99 levels, exponential curve)
-- =============================================================================
-- Formula: XP_for_level(N) = floor(100 * (1.15^(N-1))), Level 1 = 0
-- BIGINT fields support level 99 reaching ~680M cumulative XP

INSERT INTO level_progression (level, xp_required, total_xp_required, milestone_feature) VALUES
    (1,   0,           0,           NULL),
    (2,   115,         115,         NULL),
    (3,   132,         247,         NULL),
    (4,   152,         399,         NULL),
    (5,   174,         573,         'unlock_sub_agents'),
    (6,   201,         774,         NULL),
    (7,   231,         1005,        NULL),
    (8,   266,         1271,        NULL),
    (9,   305,         1576,        NULL),
    (10,  351,         1927,        'unlock_workflows'),
    (11,  404,         2331,        NULL),
    (12,  465,         2796,        NULL),
    (13,  535,         3331,        NULL),
    (14,  615,         3946,        NULL),
    (15,  707,         4653,        'unlock_external_channels'),
    (16,  813,         5466,        NULL),
    (17,  935,         6401,        NULL),
    (18,  1076,        7477,        NULL),
    (19,  1237,        8714,        NULL),
    (20,  1423,        10137,       'unlock_voice'),
    (21,  1636,        11773,       NULL),
    (22,  1882,        13655,       NULL),
    (23,  2164,        15819,       NULL),
    (24,  2489,        18308,       NULL),
    (25,  2862,        21170,       'master_tier_badge'),
    (26,  3291,        24461,       NULL),
    (27,  3785,        28246,       NULL),
    (28,  4353,        32599,       NULL),
    (29,  5006,        37605,       NULL),
    (30,  5757,        43362,       NULL),
    (31,  6621,        49983,       NULL),
    (32,  7614,        57597,       NULL),
    (33,  8756,        66353,       NULL),
    (34,  10069,       76422,       NULL),
    (35,  11580,       88002,       NULL),
    (36,  13317,       101319,      NULL),
    (37,  15315,       116634,      NULL),
    (38,  17612,       134246,      NULL),
    (39,  20254,       154500,      NULL),
    (40,  23292,       177792,      NULL),
    (41,  26786,       204578,      NULL),
    (42,  30804,       235382,      NULL),
    (43,  35424,       270806,      NULL),
    (44,  40738,       311544,      NULL),
    (45,  46849,       358393,      NULL),
    (46,  53876,       412269,      NULL),
    (47,  61958,       474227,      NULL),
    (48,  71252,       545479,      NULL),
    (49,  81940,       627419,      NULL),
    (50,  94231,       721650,      'grandmaster_tier'),
    (51,  108365,      830015,      NULL),
    (52,  124620,      954635,      NULL),
    (53,  143313,      1097948,     NULL),
    (54,  164810,      1262758,     NULL),
    (55,  189532,      1452290,     NULL),
    (56,  217962,      1670252,     NULL),
    (57,  250656,      1920908,     NULL),
    (58,  288255,      2209163,     NULL),
    (59,  331493,      2540656,     NULL),
    (60,  381217,      2921873,     NULL),
    (61,  438399,      3360272,     NULL),
    (62,  504159,      3864431,     NULL),
    (63,  579783,      4444214,     NULL),
    (64,  666751,      5110965,     NULL),
    (65,  766764,      5877729,     NULL),
    (66,  881778,      6759507,     NULL),
    (67,  1014045,     7773552,     NULL),
    (68,  1166152,     8939704,     NULL),
    (69,  1341075,     10280779,    NULL),
    (70,  1542236,     11823015,    NULL),
    (71,  1773572,     13596587,    NULL),
    (72,  2039607,     15636194,    NULL),
    (73,  2345548,     17981742,    NULL),
    (74,  2697381,     20679123,    NULL),
    (75,  3101988,     23781111,    'legend_tier'),
    (76,  3567286,     27348397,    NULL),
    (77,  4102379,     31450776,    NULL),
    (78,  4717736,     36168512,    NULL),
    (79,  5425397,     41593909,    NULL),
    (80,  6239206,     47833115,    NULL),
    (81,  7175087,     55008202,    NULL),
    (82,  8251351,     63259553,    NULL),
    (83,  9489053,     72748606,    NULL),
    (84,  10912411,    83661017,    NULL),
    (85,  12549273,    96210290,    NULL),
    (86,  14431664,    110641954,   NULL),
    (87,  16596414,    127238368,   NULL),
    (88,  19085876,    146324244,   NULL),
    (89,  21948758,    168273002,   NULL),
    (90,  25241071,    193514073,   NULL),
    (91,  29027232,    222541305,   NULL),
    (92,  33381317,    255922622,   NULL),
    (93,  38388515,    294311137,   NULL),
    (94,  44146792,    338457929,   NULL),
    (95,  50768811,    389226740,   NULL),
    (96,  58384132,    447610872,   NULL),
    (97,  67141752,    514752624,   NULL),
    (98,  77213015,    591965639,   NULL),
    (99,  88794967,    680760606,   'max_level_achieved');


-- =============================================================================
-- SEED DATA: Machine Profile Elixir Limits
-- =============================================================================
-- Insert machine profile configs with elixir limits into config_store

INSERT INTO config_store (key, value) VALUES
    ('machine_profile.urim', '{
        "cpu_cores": 16,
        "memory_gb": 64,
        "gpu": true,
        "worker_concurrency": 8,
        "event_buffer_capacity": 50000,
        "event_broadcast_capacity": 4096,
        "model_context_window": 128000,
        "elixir_max_count": 200,
        "elixir_max_size_mb": 50,
        "elixir_max_per_sub_agent": 10
    }'::JSONB),
    ('machine_profile.thummim', '{
        "cpu_cores": 8,
        "memory_gb": 32,
        "gpu": false,
        "worker_concurrency": 4,
        "event_buffer_capacity": 10000,
        "event_broadcast_capacity": 1024,
        "model_context_window": 32000,
        "elixir_max_count": 100,
        "elixir_max_size_mb": 20,
        "elixir_max_per_sub_agent": 5
    }'::JSONB)
ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW();


-- =============================================================================
-- SEED DATA: Initialize XP for Lian
-- =============================================================================

INSERT INTO agent_xp (identity_id, total_xp, level, xp_to_next_level)
SELECT identity_id, 0, 1, 115
FROM identities
WHERE identity_type = 'core' AND name = 'Lian';
