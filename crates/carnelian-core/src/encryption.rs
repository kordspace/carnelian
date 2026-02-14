//! Encryption at Rest for 🔥 Carnelian OS
//!
//! This module provides transparent encryption/decryption of sensitive data
//! stored in PostgreSQL using the `pgcrypto` extension's symmetric PGP
//! functions (`pgp_sym_encrypt_bytea` / `pgp_sym_decrypt_bytea`).
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐     derive_aes_storage_key()     ┌──────────────┐
//! │ Ed25519 Seed │ ──────────────────────────────► │ 32-byte AES  │
//! │ (SigningKey)  │   blake3 HKDF context:          │ Storage Key  │
//! └─────────────┘   "carnelian-aes-storage-v1"     └──────┬───────┘
//!                                                         │
//!                                                         ▼
//!                                              ┌──────────────────┐
//!                                              │ EncryptionHelper │
//!                                              │   pool + key     │
//!                                              └────────┬─────────┘
//!                                                       │
//!                              ┌────────────────────────┼────────────────────────┐
//!                              ▼                        ▼                        ▼
//!                     encrypt_text()           encrypt_bytes()          decrypt_text()
//!                     decrypt_text()           decrypt_bytes()          decrypt_bytes()
//!                              │                        │                        │
//!                              ▼                        ▼                        ▼
//!                     pgp_sym_encrypt_bytea    pgp_sym_encrypt_bytea   pgp_sym_decrypt_bytea
//!                     (PostgreSQL pgcrypto)    (PostgreSQL pgcrypto)   (PostgreSQL pgcrypto)
//! ```
//!
//! # Key Derivation
//!
//! The AES-256 key is derived from the owner's Ed25519 signing key seed via
//! [`crate::crypto::derive_aes_storage_key`], which uses blake3's `derive_key`
//! with context `"carnelian-aes-storage-v1"`. This produces a deterministic
//! 32-byte key without exposing the signing key itself.
//!
//! # Key Rotation
//!
//! Key rotation is supported via the `key_version` column in `config_store`:
//!
//! 1. Generate a new Ed25519 keypair with [`crate::crypto::generate_ed25519_keypair`]
//! 2. Derive a new AES key — the version is tracked in `config_store.key_version`
//! 3. Re-encrypt all sensitive data (memories, run_logs, config blobs) with the new key
//! 4. Update `key_version` in `config_store` rows
//! 5. Key rotation should go through the approval workflow (Phase 2 `ApprovalQueue`)
//!
//! ## Key Versioning Strategy
//!
//! Each encrypted row can be associated with a `key_version`. During rotation,
//! a background task reads rows with the old version, decrypts with the old key,
//! re-encrypts with the new key, and updates the version atomically.
//!
//! ```ignore
//! // Example key rotation workflow
//! let (new_signing_key, _) = generate_ed25519_keypair();
//! let new_helper = EncryptionHelper::new(pool, &new_signing_key);
//! let old_helper = EncryptionHelper::new(pool, &old_signing_key);
//!
//! // For each encrypted row:
//! let plaintext = old_helper.decrypt_bytes(&row.ciphertext).await?;
//! let new_ciphertext = new_helper.encrypt_bytes(&plaintext).await?;
//! // UPDATE row SET content = new_ciphertext, key_version = 2
//! ```
//!
//! # Security Notes
//!
//! - The AES key is passed to PostgreSQL as a hex-encoded query parameter;
//!   ensure TLS is enabled on the database connection in production.
//! - `pgp_sym_encrypt_bytea` uses OpenPGP symmetric encryption (AES-256 + SHA-256)
//!   which includes integrity verification on decryption.
//! - Embeddings (`memories.embedding`) are intentionally left unencrypted to
//!   preserve pgvector cosine similarity search functionality.

use carnelian_common::{Error, Result};
use ed25519_dalek::SigningKey;
use sqlx::PgPool;

use crate::crypto::derive_aes_storage_key;

// =============================================================================
// ENCRYPTION HELPER
// =============================================================================

/// Helper for encrypting and decrypting data at rest using PostgreSQL's pgcrypto.
///
/// Holds a database pool and a hex-encoded AES-256 key derived from the owner's
/// Ed25519 signing key. All encryption/decryption is performed server-side via
/// `pgp_sym_encrypt_bytea` and `pgp_sym_decrypt_bytea` SQL functions.
#[derive(Clone)]
pub struct EncryptionHelper {
    pool: PgPool,
    /// Hex-encoded 32-byte AES key for pgcrypto symmetric functions
    key_hex: String,
}

impl EncryptionHelper {
    /// Create a new `EncryptionHelper` by deriving the AES storage key from
    /// the provided Ed25519 signing key.
    ///
    /// # Arguments
    ///
    /// * `pool` - PostgreSQL connection pool (must have pgcrypto extension)
    /// * `signing_key` - Owner's Ed25519 signing key for key derivation
    pub fn new(pool: &PgPool, signing_key: &SigningKey) -> Self {
        let aes_key = derive_aes_storage_key(signing_key);
        Self {
            pool: pool.clone(),
            key_hex: hex::encode(aes_key),
        }
    }

    /// Return the hex-encoded AES key for use in raw SQL queries.
    ///
    /// This is useful when callers need to embed encryption/decryption
    /// directly in their own SQL statements rather than using the helper methods.
    #[must_use]
    pub fn key_hex(&self) -> &str {
        &self.key_hex
    }

    // =========================================================================
    // TEXT ENCRYPTION
    // =========================================================================

    /// Encrypt a UTF-8 string, returning the pgcrypto ciphertext as raw bytes.
    ///
    /// The plaintext is converted to bytes and encrypted server-side via
    /// `pgp_sym_encrypt_bytea`. The returned `Vec<u8>` should be stored in a
    /// BYTEA column.
    ///
    /// # Errors
    ///
    /// Returns `Error::Crypto` if the SQL encryption call fails.
    pub async fn encrypt_text(&self, plaintext: &str) -> Result<Vec<u8>> {
        self.encrypt_bytes(plaintext.as_bytes()).await
    }

    /// Decrypt pgcrypto ciphertext back to a UTF-8 string.
    ///
    /// # Errors
    ///
    /// Returns `Error::Crypto` if decryption fails (wrong key, corrupted data)
    /// or if the decrypted bytes are not valid UTF-8.
    pub async fn decrypt_text(&self, ciphertext: &[u8]) -> Result<String> {
        let bytes = self.decrypt_bytes(ciphertext).await?;
        String::from_utf8(bytes)
            .map_err(|e| Error::Crypto(format!("Decrypted data is not valid UTF-8: {}", e)))
    }

    // =========================================================================
    // BINARY ENCRYPTION
    // =========================================================================

    /// Encrypt arbitrary bytes, returning the pgcrypto ciphertext.
    ///
    /// Uses `pgp_sym_encrypt_bytea($1, $2)` where `$1` is the plaintext bytes
    /// and `$2` is the hex-encoded AES key.
    ///
    /// # Errors
    ///
    /// Returns `Error::Crypto` if the SQL encryption call fails.
    pub async fn encrypt_bytes(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let row: (Vec<u8>,) = sqlx::query_as("SELECT pgp_sym_encrypt_bytea($1, $2)")
            .bind(plaintext)
            .bind(&self.key_hex)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| Error::Crypto(format!("Encryption failed: {}", e)))?;

        Ok(row.0)
    }

    /// Decrypt pgcrypto ciphertext back to raw bytes.
    ///
    /// Uses `pgp_sym_decrypt_bytea($1, $2)` where `$1` is the ciphertext and
    /// `$2` is the hex-encoded AES key.
    ///
    /// # Errors
    ///
    /// Returns `Error::Crypto` if decryption fails due to wrong key,
    /// corrupted data, or a database error.
    pub async fn decrypt_bytes(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let row: (Vec<u8>,) = sqlx::query_as("SELECT pgp_sym_decrypt_bytea($1, $2)")
            .bind(ciphertext)
            .bind(&self.key_hex)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                Error::Crypto(format!(
                    "Decryption failed (wrong key or corrupted data): {}",
                    e
                ))
            })?;

        Ok(row.0)
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::generate_ed25519_keypair;

    #[test]
    fn test_encryption_helper_key_derivation_deterministic() {
        let (signing, _) = generate_ed25519_keypair();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgresql://test:test@localhost:5432/test")
            .expect("lazy pool");

        let helper1 = EncryptionHelper::new(&pool, &signing);
        let helper2 = EncryptionHelper::new(&pool, &signing);
        assert_eq!(
            helper1.key_hex(),
            helper2.key_hex(),
            "same signing key must produce same AES key"
        );
    }

    #[test]
    fn test_encryption_helper_different_keys() {
        let (signing1, _) = generate_ed25519_keypair();
        let (signing2, _) = generate_ed25519_keypair();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgresql://test:test@localhost:5432/test")
            .expect("lazy pool");

        let helper1 = EncryptionHelper::new(&pool, &signing1);
        let helper2 = EncryptionHelper::new(&pool, &signing2);
        assert_ne!(
            helper1.key_hex(),
            helper2.key_hex(),
            "different signing keys must produce different AES keys"
        );
    }

    #[test]
    fn test_key_hex_length() {
        let (signing, _) = generate_ed25519_keypair();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgresql://test:test@localhost:5432/test")
            .expect("lazy pool");

        let helper = EncryptionHelper::new(&pool, &signing);
        assert_eq!(
            helper.key_hex().len(),
            64,
            "hex-encoded 32-byte key = 64 chars"
        );
    }

    #[tokio::test]
    #[ignore = "requires database connection with pgcrypto"]
    async fn test_encrypt_decrypt_text_roundtrip() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("connect");

        let (signing, _) = generate_ed25519_keypair();
        let helper = EncryptionHelper::new(&pool, &signing);

        let plaintext = "Hello, Carnelian! 🔥";
        let ciphertext = helper.encrypt_text(plaintext).await.expect("encrypt");
        assert_ne!(
            ciphertext,
            plaintext.as_bytes(),
            "ciphertext must differ from plaintext"
        );

        let decrypted = helper.decrypt_text(&ciphertext).await.expect("decrypt");
        assert_eq!(decrypted, plaintext, "round-trip must preserve content");
    }

    #[tokio::test]
    #[ignore = "requires database connection with pgcrypto"]
    async fn test_encrypt_decrypt_bytes_roundtrip() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("connect");

        let (signing, _) = generate_ed25519_keypair();
        let helper = EncryptionHelper::new(&pool, &signing);

        let data = vec![0u8, 1, 2, 255, 128, 64];
        let ciphertext = helper.encrypt_bytes(&data).await.expect("encrypt");
        let decrypted = helper.decrypt_bytes(&ciphertext).await.expect("decrypt");
        assert_eq!(decrypted, data);
    }

    #[tokio::test]
    #[ignore = "requires database connection with pgcrypto"]
    async fn test_decrypt_with_wrong_key_fails() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("connect");

        let (signing1, _) = generate_ed25519_keypair();
        let (signing2, _) = generate_ed25519_keypair();
        let helper1 = EncryptionHelper::new(&pool, &signing1);
        let helper2 = EncryptionHelper::new(&pool, &signing2);

        let ciphertext = helper1.encrypt_text("secret").await.expect("encrypt");
        let result = helper2.decrypt_text(&ciphertext).await;
        assert!(result.is_err(), "decryption with wrong key must fail");
    }

    #[tokio::test]
    #[ignore = "requires database connection with pgcrypto"]
    async fn test_encrypt_various_sizes() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("connect");

        let (signing, _) = generate_ed25519_keypair();
        let helper = EncryptionHelper::new(&pool, &signing);

        // 10 bytes
        let small = "0123456789";
        let ct = helper.encrypt_text(small).await.expect("encrypt small");
        assert_eq!(
            helper.decrypt_text(&ct).await.expect("decrypt small"),
            small
        );

        // 1 KB
        let medium = "A".repeat(1024);
        let ct = helper.encrypt_text(&medium).await.expect("encrypt 1KB");
        assert_eq!(helper.decrypt_text(&ct).await.expect("decrypt 1KB"), medium);

        // 100 KB
        let large = "B".repeat(100 * 1024);
        let ct = helper.encrypt_text(&large).await.expect("encrypt 100KB");
        assert_eq!(
            helper.decrypt_text(&ct).await.expect("decrypt 100KB"),
            large
        );
    }
}
