-- Delta migration for the memories table:
-- 1. Make importance NOT NULL (backfill existing NULLs with the default 0.5)
-- 2. Add btree index on created_at for temporal queries
--    (load_recent_memories "today + yesterday" heuristic, MemoryQuery time-range filters)

UPDATE memories SET importance = 0.5 WHERE importance IS NULL;
ALTER TABLE memories ALTER COLUMN importance SET NOT NULL;

CREATE INDEX IF NOT EXISTS idx_memories_created_at ON memories(created_at DESC);
