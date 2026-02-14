-- =============================================================================
-- Migration 0009: Encryption at Rest Support
-- =============================================================================
-- Adds schema changes to support AES-256-GCM encryption at rest using
-- PostgreSQL's pgcrypto extension (already enabled in migration 0001).
--
-- Changes:
--   1. Add value_text TEXT column to config_store (addresses existing code usage)
--   2. Add key_version INTEGER column to config_store for key rotation support
--   3. Add sensitive BOOLEAN column to run_logs for flagging encrypted messages
--   4. Alter memories.content from TEXT to BYTEA for encrypted storage
--   5. Alter run_logs.message from TEXT to BYTEA for encrypted storage
--   6. Create index on run_logs(sensitive) for efficient filtering
--
-- Note: pgcrypto extension is already enabled in migration 0001 via
--       CREATE EXTENSION IF NOT EXISTS pgcrypto;
-- =============================================================================

-- ============================================================================
-- CONFIG_STORE: Add value_text and key_version columns
-- ============================================================================

ALTER TABLE config_store ADD COLUMN IF NOT EXISTS value_text TEXT;
ALTER TABLE config_store ADD COLUMN IF NOT EXISTS key_version INTEGER NOT NULL DEFAULT 1;

-- ============================================================================
-- RUN_LOGS: Add sensitive flag for encrypted messages
-- ============================================================================

ALTER TABLE run_logs ADD COLUMN sensitive BOOLEAN NOT NULL DEFAULT false;
CREATE INDEX idx_run_logs_sensitive ON run_logs(sensitive) WHERE sensitive = true;

-- ============================================================================
-- MEMORIES: Convert content from TEXT to BYTEA for encrypted storage
-- ============================================================================
-- Existing plaintext content is cast to BYTEA via convert_to(). New rows will
-- store pgcrypto-encrypted bytes directly. The embedding column remains
-- unencrypted to preserve pgvector cosine similarity search.

ALTER TABLE memories ALTER COLUMN content TYPE BYTEA USING convert_to(content, 'UTF8');

-- Remove LZ4 compression on memories.content — encryption output is
-- incompressible and the TOAST overhead is wasted.
ALTER TABLE memories ALTER COLUMN content SET COMPRESSION default;

-- ============================================================================
-- RUN_LOGS: Convert message from TEXT to BYTEA for encrypted storage
-- ============================================================================
-- Existing plaintext messages are cast to BYTEA. New sensitive rows will
-- store pgcrypto-encrypted bytes; non-sensitive rows store raw UTF-8 bytes.

ALTER TABLE run_logs ALTER COLUMN message TYPE BYTEA USING convert_to(message, 'UTF8');

-- Remove LZ4 compression on run_logs.message for the same reason.
ALTER TABLE run_logs ALTER COLUMN message SET COMPRESSION default;
