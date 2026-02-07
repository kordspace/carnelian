-- =============================================================================
-- Migration 0005: Add value_blob column to config_store
-- =============================================================================
-- Adds a BYTEA column for storing binary data (e.g., Ed25519 keypairs) that
-- cannot be represented as JSONB.
-- =============================================================================

ALTER TABLE config_store ADD COLUMN value_blob BYTEA;
