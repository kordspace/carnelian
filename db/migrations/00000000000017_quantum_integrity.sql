-- Migration: Add quantum_checksum column to core tables for MAGIC integrity verification
-- This migration adds a TEXT column to store quantum-derived checksums for data integrity.
-- Existing rows remain NULL; checksums are populated incrementally via application logic.

-- ============================================================================
-- Section 1: DDL - Add quantum_checksum TEXT column
-- ============================================================================

-- Add quantum_checksum to memories table
ALTER TABLE memories
ADD COLUMN quantum_checksum TEXT;

-- Add quantum_checksum to task_runs table
ALTER TABLE task_runs
ADD COLUMN quantum_checksum TEXT;

-- Add quantum_checksum to session_messages table
ALTER TABLE session_messages
ADD COLUMN quantum_checksum TEXT;

-- Add quantum_checksum to elixirs table
ALTER TABLE elixirs
ADD COLUMN quantum_checksum TEXT;

-- ============================================================================
-- Section 2: Indexes - Partial indexes on populated rows only
-- ============================================================================

-- Index for memories table (partial: only rows with checksums)
CREATE INDEX idx_memories_quantum_checksum
ON memories (quantum_checksum)
WHERE quantum_checksum IS NOT NULL;

-- Index for task_runs table (partial: only rows with checksums)
CREATE INDEX idx_task_runs_quantum_checksum
ON task_runs (quantum_checksum)
WHERE quantum_checksum IS NOT NULL;

-- Index for session_messages table (partial: only rows with checksums)
CREATE INDEX idx_session_messages_quantum_checksum
ON session_messages (quantum_checksum)
WHERE quantum_checksum IS NOT NULL;

-- Index for elixirs table (partial: only rows with checksums)
CREATE INDEX idx_elixirs_quantum_checksum
ON elixirs (quantum_checksum)
WHERE quantum_checksum IS NOT NULL;
