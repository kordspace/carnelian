-- =============================================================================
-- Channel Sessions Migration
-- =============================================================================
--
-- Ensures the `channel_sessions` table exists with all required columns,
-- constraints, and indexes for channel adapter integrations.
--
-- This migration is idempotent: it uses IF NOT EXISTS so it can be safely
-- applied even if the table was already created in phase1_delta.
-- =============================================================================

CREATE TABLE IF NOT EXISTS channel_sessions (
    session_id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_type    TEXT NOT NULL CHECK (channel_type IN ('telegram', 'discord', 'whatsapp', 'slack', 'ui')),
    channel_user_id TEXT NOT NULL,
    trust_level     TEXT NOT NULL DEFAULT 'untrusted' CHECK (trust_level IN ('untrusted', 'conversational', 'owner')),
    identity_id     UUID NULL REFERENCES identities(identity_id) ON DELETE SET NULL,
    metadata        JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (channel_type, channel_user_id)
);

CREATE INDEX IF NOT EXISTS idx_channel_sessions_type ON channel_sessions(channel_type);
CREATE INDEX IF NOT EXISTS idx_channel_sessions_trust ON channel_sessions(trust_level);
CREATE INDEX IF NOT EXISTS idx_channel_sessions_identity ON channel_sessions(identity_id);
