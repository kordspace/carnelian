-- Core Schema Migration for Carnelian OS
-- This migration creates the foundational tables for the orchestrator

-- Enable pgcrypto extension for gen_random_uuid()
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Enable pgvector extension for embedding storage
CREATE EXTENSION IF NOT EXISTS vector;

-- ============================================================================
-- IDENTITIES: Core agent (Lian) and sub-agent identities
-- ============================================================================
CREATE TABLE identities (
    identity_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    pronouns TEXT,
    identity_type TEXT NOT NULL CHECK (identity_type IN ('core', 'sub_agent')),
    soul_file_path TEXT,
    soul_file_hash TEXT,
    directives JSONB DEFAULT '[]'::jsonb,
    voice_config JSONB DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_identities_type ON identities(identity_type);

-- ============================================================================
-- CAPABILITIES: Define capability types for security model
-- ============================================================================
CREATE TABLE capabilities (
    capability_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    capability_key TEXT NOT NULL UNIQUE,
    description TEXT,
    scope_schema JSONB DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- CAPABILITY_GRANTS: Track capability assignments to subjects
-- ============================================================================
CREATE TABLE capability_grants (
    grant_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_type TEXT NOT NULL CHECK (subject_type IN ('identity', 'skill', 'channel', 'session')),
    subject_id UUID NOT NULL,
    capability_key TEXT NOT NULL REFERENCES capabilities(capability_key) ON DELETE CASCADE,
    scope JSONB DEFAULT '{}'::jsonb,
    constraints JSONB DEFAULT '{}'::jsonb,
    approved_by UUID REFERENCES identities(identity_id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ
);

CREATE INDEX idx_capability_grants_subject ON capability_grants(subject_type, subject_id);
CREATE INDEX idx_capability_grants_key ON capability_grants(capability_key);

-- ============================================================================
-- SKILLS: Skill catalog for worker execution
-- ============================================================================
CREATE TABLE skills (
    skill_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    runtime TEXT NOT NULL CHECK (runtime IN ('node', 'python', 'shell', 'wasm')),
    enabled BOOLEAN NOT NULL DEFAULT true,
    manifest JSONB DEFAULT '{}'::jsonb,
    capabilities_required TEXT[] DEFAULT '{}',
    checksum TEXT,
    signature TEXT,
    discovered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_skills_enabled ON skills(enabled);
CREATE INDEX idx_skills_runtime ON skills(runtime);

-- ============================================================================
-- TASKS: Work queue for orchestrator
-- ============================================================================
CREATE TABLE tasks (
    task_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title TEXT NOT NULL,
    description TEXT,
    created_by UUID REFERENCES identities(identity_id) ON DELETE SET NULL,
    assigned_to UUID REFERENCES identities(identity_id) ON DELETE SET NULL,
    skill_id UUID REFERENCES skills(skill_id) ON DELETE SET NULL,
    state TEXT NOT NULL DEFAULT 'pending' CHECK (state IN ('pending', 'running', 'completed', 'failed', 'canceled')),
    priority INTEGER NOT NULL DEFAULT 0,
    requires_approval BOOLEAN NOT NULL DEFAULT false,
    correlation_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_tasks_state ON tasks(state);
CREATE INDEX idx_tasks_created_by ON tasks(created_by);
CREATE INDEX idx_tasks_assigned_to ON tasks(assigned_to);
CREATE INDEX idx_tasks_correlation ON tasks(correlation_id);

-- ============================================================================
-- TASK_RUNS: Execution attempts for tasks
-- ============================================================================
CREATE TABLE task_runs (
    run_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
    attempt INTEGER NOT NULL DEFAULT 1,
    worker_id TEXT,
    state TEXT NOT NULL DEFAULT 'queued' CHECK (state IN ('queued', 'running', 'success', 'failed', 'timeout', 'canceled')),
    started_at TIMESTAMPTZ,
    ended_at TIMESTAMPTZ,
    exit_code INTEGER,
    result JSONB,
    error TEXT,
    correlation_id UUID
);

CREATE INDEX idx_task_runs_task ON task_runs(task_id);
CREATE INDEX idx_task_runs_state ON task_runs(state);
CREATE INDEX idx_task_runs_correlation ON task_runs(correlation_id);

-- ============================================================================
-- RUN_LOGS: Structured logs for task runs
-- ============================================================================
CREATE TABLE run_logs (
    log_id BIGSERIAL PRIMARY KEY,
    run_id UUID NOT NULL REFERENCES task_runs(run_id) ON DELETE CASCADE,
    ts TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    level TEXT NOT NULL CHECK (level IN ('error', 'warn', 'info', 'debug', 'trace')),
    message TEXT NOT NULL,
    fields JSONB DEFAULT '{}'::jsonb,
    truncated BOOLEAN NOT NULL DEFAULT false
);

CREATE INDEX idx_run_logs_run ON run_logs(run_id);
CREATE INDEX idx_run_logs_level ON run_logs(level);
CREATE INDEX idx_run_logs_ts ON run_logs(ts DESC);

-- ============================================================================
-- LEDGER_EVENTS: Tamper-resistant audit log with blake3 hash chain
-- ============================================================================
-- Hash-chain integrity using blake3 algorithm:
--   - payload_hash: blake3(canonical_json_payload)
--   - event_hash: blake3(timestamp || actor_id || action_type || payload_hash || prev_hash)
--   - Each event links to previous via prev_hash, creating immutable chain
--   - Verification: recompute all hashes and check chain integrity
CREATE TABLE ledger_events (
    event_id BIGSERIAL PRIMARY KEY,
    ts TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    actor_id UUID REFERENCES identities(identity_id) ON DELETE SET NULL,
    action_type TEXT NOT NULL,
    payload_hash TEXT NOT NULL,
    prev_hash TEXT,
    event_hash TEXT NOT NULL,
    core_signature TEXT,
    correlation_id UUID,
    metadata JSONB DEFAULT '{}'::jsonb
);

CREATE INDEX idx_ledger_events_ts ON ledger_events(ts DESC);
CREATE INDEX idx_ledger_events_actor ON ledger_events(actor_id);
CREATE INDEX idx_ledger_events_correlation ON ledger_events(correlation_id);

-- ============================================================================
-- MEMORIES: Memory storage with vector embeddings for RAG
-- ============================================================================
CREATE TABLE memories (
    memory_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    identity_id UUID NOT NULL REFERENCES identities(identity_id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    summary TEXT,
    source TEXT NOT NULL CHECK (source IN ('conversation', 'task', 'observation', 'reflection')),
    embedding vector(1536),
    importance REAL DEFAULT 0.5 CHECK (importance >= 0.0 AND importance <= 1.0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    accessed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    access_count INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_memories_identity ON memories(identity_id);
CREATE INDEX idx_memories_source ON memories(source);
CREATE INDEX idx_memories_importance ON memories(importance DESC);
CREATE INDEX idx_memories_embedding ON memories USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

-- ============================================================================
-- MODEL_PROVIDERS: LLM provider configurations
-- ============================================================================
CREATE TABLE model_providers (
    provider_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_type TEXT NOT NULL CHECK (provider_type IN ('local', 'remote')),
    name TEXT NOT NULL UNIQUE,
    enabled BOOLEAN NOT NULL DEFAULT true,
    config JSONB DEFAULT '{}'::jsonb,
    budget_limits JSONB DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_model_providers_enabled ON model_providers(enabled);

-- ============================================================================
-- USAGE_COSTS: Token usage and cost tracking
-- ============================================================================
CREATE TABLE usage_costs (
    usage_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_id UUID NOT NULL REFERENCES model_providers(provider_id) ON DELETE CASCADE,
    ts TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    tokens_in INTEGER NOT NULL DEFAULT 0,
    tokens_out INTEGER NOT NULL DEFAULT 0,
    cost_estimate NUMERIC(10, 6) DEFAULT 0,
    task_id UUID REFERENCES tasks(task_id) ON DELETE SET NULL,
    run_id UUID REFERENCES task_runs(run_id) ON DELETE SET NULL,
    correlation_id UUID
);

CREATE INDEX idx_usage_costs_provider ON usage_costs(provider_id);
CREATE INDEX idx_usage_costs_ts ON usage_costs(ts DESC);
CREATE INDEX idx_usage_costs_task ON usage_costs(task_id);

-- ============================================================================
-- CONFIG_STORE: Key-value configuration storage
-- ============================================================================
CREATE TABLE config_store (
    key TEXT PRIMARY KEY,
    value JSONB NOT NULL,
    encrypted BOOLEAN NOT NULL DEFAULT false,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- CONFIG_VERSIONS: Configuration change history
-- ============================================================================
CREATE TABLE config_versions (
    version_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    config_key TEXT NOT NULL,
    config_value JSONB NOT NULL,
    change_description TEXT,
    config_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_config_versions_key ON config_versions(config_key);
CREATE INDEX idx_config_versions_created ON config_versions(created_at DESC);

-- ============================================================================
-- HEARTBEAT_HISTORY: Heartbeat tracking for wake routine
-- ============================================================================
CREATE TABLE heartbeat_history (
    heartbeat_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    identity_id UUID NOT NULL REFERENCES identities(identity_id) ON DELETE CASCADE,
    ts TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    mantra TEXT,
    tasks_queued INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL CHECK (status IN ('ok', 'skipped', 'failed')),
    reason TEXT,
    duration_ms INTEGER
);

CREATE INDEX idx_heartbeat_history_identity ON heartbeat_history(identity_id);
CREATE INDEX idx_heartbeat_history_ts ON heartbeat_history(ts DESC);

-- ============================================================================
-- SEED DATA: Insert core identity for Lian
-- ============================================================================
INSERT INTO identities (name, pronouns, identity_type, soul_file_path, directives)
VALUES (
    'Lian',
    'she/her',
    'core',
    'souls/lian.md',
    '["Assist Marco with development tasks", "Maintain system integrity", "Learn and adapt"]'::jsonb
);

-- Insert default capabilities
-- Capability mapping to original requirements:
--   fs.read, fs.write     -> filesystem access
--   net.http              -> network access  
--   process.spawn         -> exec.shell equivalent (more precise naming)
--   model.inference       -> model.local + model.remote combined (more flexible)
INSERT INTO capabilities (capability_key, description) VALUES
    ('fs.read', 'Read files from the filesystem'),
    ('fs.write', 'Write files to the filesystem'),
    ('fs.delete', 'Delete files from the filesystem'),
    ('net.http', 'Make HTTP requests'),
    ('net.websocket', 'Establish WebSocket connections'),
    ('process.spawn', 'Spawn child processes'),
    ('process.kill', 'Kill running processes'),
    ('exec.shell', 'Execute shell commands'),
    ('db.read', 'Read from the database'),
    ('db.write', 'Write to the database'),
    ('model.inference', 'Request model inference'),
    ('model.local', 'Request local model inference'),
    ('model.remote', 'Request remote model inference'),
    ('skill.execute', 'Execute skills'),
    ('task.create', 'Create new tasks'),
    ('task.cancel', 'Cancel running tasks'),
    ('config.read', 'Read configuration'),
    ('config.write', 'Write configuration'),
    ('ledger.read', 'Read audit ledger'),
    ('ledger.write', 'Write to audit ledger');

-- Insert default local model provider (Ollama)
INSERT INTO model_providers (provider_type, name, config)
VALUES (
    'local',
    'ollama',
    '{"base_url": "http://localhost:11434", "default_model": "deepseek-r1:7b"}'::jsonb
);
