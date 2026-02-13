-- Add correlation_id column to heartbeat_history for end-to-end tracing
ALTER TABLE heartbeat_history ADD COLUMN correlation_id UUID;

-- Add index for correlation-based queries
CREATE INDEX idx_heartbeat_history_correlation ON heartbeat_history(correlation_id);
