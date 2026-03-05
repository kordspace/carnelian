-- Migration: Add key_algorithm tracking for post-quantum cryptography
-- Date: 2026-03-03
-- Purpose: Track which cryptographic algorithm is used for each key in config_store
--          to support hybrid PQC migration (Ed25519 → Dilithium + Ed25519 → Dilithium)

-- Add key_algorithm column to config_store
ALTER TABLE config_store
ADD COLUMN key_algorithm TEXT NOT NULL DEFAULT 'ed25519'
CHECK (key_algorithm IN ('ed25519', 'hybrid_dilithium_ed25519', 'dilithium3'));

-- Add index for querying by algorithm
CREATE INDEX idx_config_store_key_algorithm ON config_store(key_algorithm);

-- Add comment explaining the column
COMMENT ON COLUMN config_store.key_algorithm IS 
'Cryptographic algorithm used for this key: ed25519 (classical), hybrid_dilithium_ed25519 (v1.0.0+), dilithium3 (v2.0.0+)';

-- Migration path:
-- 1. All existing keys default to 'ed25519'
-- 2. New keys created with MAGIC enabled use 'hybrid_dilithium_ed25519'
-- 3. Future migration to 'dilithium3' only (post-quantum only)
