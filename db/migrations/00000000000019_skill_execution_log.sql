-- Migration: Add skill execution audit log
-- Date: 2026-03-03
-- Purpose: Track all skill executions for security auditing and debugging

CREATE TABLE skill_execution_log (
    execution_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    skill_id TEXT NOT NULL,
    session_id UUID REFERENCES sessions(session_id) ON DELETE SET NULL,
    identity_id UUID REFERENCES identities(identity_id) ON DELETE SET NULL,
    
    -- Execution details
    params JSONB NOT NULL DEFAULT '{}'::jsonb,
    output JSONB,
    error TEXT,
    
    -- Performance metrics
    duration_ms BIGINT NOT NULL,
    exit_code INTEGER,
    timed_out BOOLEAN NOT NULL DEFAULT false,
    
    -- Resource usage
    memory_used_mb BIGINT,
    cpu_time_ms BIGINT,
    
    -- Security
    sandboxed BOOLEAN NOT NULL DEFAULT true,
    resource_limits JSONB,
    
    -- Timestamps
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Audit trail
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for common queries
CREATE INDEX idx_skill_execution_log_skill_id ON skill_execution_log(skill_id);
CREATE INDEX idx_skill_execution_log_session_id ON skill_execution_log(session_id);
CREATE INDEX idx_skill_execution_log_identity_id ON skill_execution_log(identity_id);
CREATE INDEX idx_skill_execution_log_started_at ON skill_execution_log(started_at DESC);
CREATE INDEX idx_skill_execution_log_timed_out ON skill_execution_log(timed_out) WHERE timed_out = true;

-- Index for failed executions
CREATE INDEX idx_skill_execution_log_errors ON skill_execution_log(skill_id, started_at DESC) WHERE error IS NOT NULL;

COMMENT ON TABLE skill_execution_log IS 'Audit log of all skill executions for security and debugging';
COMMENT ON COLUMN skill_execution_log.sandboxed IS 'Whether execution used resource limits and sandboxing';
COMMENT ON COLUMN skill_execution_log.timed_out IS 'Whether execution was terminated due to timeout';
