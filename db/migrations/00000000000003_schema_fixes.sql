-- =============================================================================
-- Schema Fixes - Pronouns, Subject ID Type, TOAST Compression
-- =============================================================================
--
-- This migration corrects schema inconsistencies identified during review:
--   - Updates Lian's pronouns from "she/her" to "he/him"
--   - Converts capability_grants.subject_id from UUID to TEXT for external refs
--   - Expands subject_type enum to include 'external_key'
--   - Enables LZ4 TOAST compression on large text/JSONB columns
-- =============================================================================


-- =============================================================================
-- UPDATE LIAN'S PRONOUNS
-- =============================================================================

UPDATE identities SET pronouns = 'he/him' WHERE name = 'Lian' AND identity_type = 'core';


-- =============================================================================
-- ALTER capability_grants.subject_id TYPE
-- =============================================================================
-- Convert from UUID to TEXT to allow external system references
-- (e.g., "telegram:12345", "discord:user#1234")

ALTER TABLE capability_grants ALTER COLUMN subject_id TYPE TEXT USING subject_id::TEXT;

ALTER TABLE capability_grants ADD CONSTRAINT chk_subject_id_format
    CHECK (subject_id ~ '^[a-zA-Z0-9_:.\-]+$');


-- =============================================================================
-- EXPAND subject_type ENUM
-- =============================================================================
-- Add 'external_key' to support API key-based subjects

ALTER TABLE capability_grants DROP CONSTRAINT capability_grants_subject_type_check;

ALTER TABLE capability_grants ADD CONSTRAINT capability_grants_subject_type_check
    CHECK (subject_type IN ('identity', 'skill', 'channel', 'session', 'external_key'));


-- =============================================================================
-- ENABLE LZ4 TOAST COMPRESSION
-- =============================================================================
-- LZ4 compression reduces storage for large text/JSONB columns while
-- maintaining fast decompression. Existing rows will be recompressed on
-- next UPDATE; new rows use LZ4 immediately.

ALTER TABLE memories ALTER COLUMN content SET COMPRESSION lz4;
ALTER TABLE run_logs ALTER COLUMN message SET COMPRESSION lz4;
ALTER TABLE ledger_events ALTER COLUMN metadata SET COMPRESSION lz4;
