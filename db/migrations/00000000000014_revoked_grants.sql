-- Migration: Create revoked_capability_grants table for cross-instance revocation propagation
-- Timestamp: 00000000000014

-- Table for storing revoked capability grants (persisted even after deletion from capability_grants)
-- This enables cross-instance sync of revocations.
CREATE TABLE revoked_capability_grants (
    grant_id UUID PRIMARY KEY,
    revoked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_by UUID,
    reason TEXT
);

-- Index for querying revocations by timestamp (for sync)
CREATE INDEX idx_revoked_capability_grants_revoked_at ON revoked_capability_grants(revoked_at DESC);

-- Index for querying revocations by who revoked
CREATE INDEX idx_revoked_capability_grants_revoked_by ON revoked_capability_grants(revoked_by);
