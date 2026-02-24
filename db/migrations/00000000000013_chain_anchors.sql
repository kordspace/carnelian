-- Migration: Create chain_anchors table for ledger anchoring
-- Timestamp: 00000000000013

-- Table for storing ledger slice anchors (hashes published to external verifiers)
CREATE TABLE chain_anchors (
    anchor_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    hash TEXT NOT NULL,
    ledger_event_from BIGINT NOT NULL,
    ledger_event_to BIGINT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    published_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    verified BOOLEAN NOT NULL DEFAULT FALSE
);

-- Index for fast lookup by hash (for external verifiers)
CREATE INDEX idx_chain_anchors_hash ON chain_anchors(hash);

-- Index for querying by event range
CREATE INDEX idx_chain_anchors_event_range ON chain_anchors(ledger_event_from, ledger_event_to);

-- Index for querying recent anchors
CREATE INDEX idx_chain_anchors_published_at ON chain_anchors(published_at DESC);
