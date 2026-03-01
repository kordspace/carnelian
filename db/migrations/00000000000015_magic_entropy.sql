-- Migration: Create magic_entropy_log table and extend ledger/config for MAGIC entropy
-- Timestamp: 00000000000015

-- =============================================================================
-- Table for storing entropy request audit logs from the MAGIC subsystem
-- =============================================================================

CREATE TABLE magic_entropy_log (
    log_id UUID PRIMARY KEY,
    ts TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    source TEXT NOT NULL,
    bytes_requested INT NOT NULL,
    quantum_available BOOL NOT NULL DEFAULT false,
    latency_ms INT,
    error TEXT,
    correlation_id UUID
);

-- =============================================================================
-- Indexes on magic_entropy_log
-- =============================================================================

-- Index for time-ordered audit queries
CREATE INDEX idx_magic_entropy_log_ts ON magic_entropy_log(ts DESC);

-- Index for correlation-based trace lookups
CREATE INDEX idx_magic_entropy_log_correlation_id ON magic_entropy_log(correlation_id);

-- =============================================================================
-- Extend ledger_events for MAGIC-enhanced hash chain
-- =============================================================================

-- Add quantum_salt column to ledger_events for MAGIC-enhanced hash chain
ALTER TABLE ledger_events ADD COLUMN quantum_salt BYTEA;

-- =============================================================================
-- Seed config_store with Quantinuum defaults
-- =============================================================================

-- Seed default Quantinuum configuration keys into config_store (no-op if already present)
INSERT INTO config_store (key, value_text, encrypted) VALUES ('quantinuum_token_expiry', 'null', false) ON CONFLICT (key) DO NOTHING;
INSERT INTO config_store (key, value_text, encrypted) VALUES ('quantinuum_device', '"H2-1"', false) ON CONFLICT (key) DO NOTHING;
