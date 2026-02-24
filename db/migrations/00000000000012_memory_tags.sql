-- Add tags column to memories table for topic-based filtering
ALTER TABLE memories ADD COLUMN IF NOT EXISTS tags JSONB DEFAULT '[]'::jsonb;

-- Add GIN index for efficient tag queries
CREATE INDEX IF NOT EXISTS idx_memories_tags ON memories USING GIN (tags);

-- Add comment
COMMENT ON COLUMN memories.tags IS 'Topic tags for selective disclosure during export';
