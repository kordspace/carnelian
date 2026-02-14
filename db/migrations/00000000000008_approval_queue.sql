-- Approval queue for privileged actions requiring owner authorization
CREATE TABLE approval_queue (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    action_type     TEXT NOT NULL,
    payload         JSONB NOT NULL,
    status          TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'denied')) DEFAULT 'pending',
    requested_by    UUID REFERENCES identities(identity_id) ON DELETE SET NULL,
    requested_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at     TIMESTAMPTZ,
    resolved_by     UUID REFERENCES identities(identity_id) ON DELETE SET NULL,
    signature       TEXT,
    correlation_id  UUID
);

CREATE INDEX idx_approval_queue_status ON approval_queue(status);
CREATE INDEX idx_approval_queue_action_type ON approval_queue(action_type);
CREATE INDEX idx_approval_queue_correlation ON approval_queue(correlation_id);
CREATE INDEX idx_approval_queue_requested_at ON approval_queue(requested_at DESC);
