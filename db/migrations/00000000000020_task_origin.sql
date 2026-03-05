-- Migration: Add origin column to tasks table for tracking task creation source
-- This enables distinguishing between user-created, LLM-suggested, and system-generated tasks

-- Add nullable origin column to tasks table
ALTER TABLE tasks ADD COLUMN IF NOT EXISTS origin TEXT;

-- Add index for filtering by origin
CREATE INDEX IF NOT EXISTS idx_tasks_origin ON tasks(origin);

-- Add comment documenting valid origin values
COMMENT ON COLUMN tasks.origin IS 'Task creation source: user_created, llm_suggested, system_generated, or NULL for legacy tasks';
