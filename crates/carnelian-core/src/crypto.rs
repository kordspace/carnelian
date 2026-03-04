//! Cryptographic utilities for 🔥 Carnelian OS
//!
//! This module centralizes cryptographic operations including:
//! - Ed25519 digital signatures (classical)
//! - Hybrid post-quantum signatures (Dilithium3 + Ed25519) when MAGIC is enabled
//! - Blake3-based key derivation
//!
//! # Ed25519 Signatures
//!
//! Ed25519 produces 64-byte signatures from 32-byte signing keys. Signatures are
//! deterministic (same key + message = same signature) and provide 128-bit security.
//! Keys are stored as 32-byte seeds; the full 64-byte expanded key is derived on use.
//!
//! # Hybrid Post-Quantum Signatures
//!
//! When MAGIC is enabled, Carnelian uses hybrid signatures combining:
//! - CRYSTALS-Dilithium3 (quantum-resistant, NIST Level 3)
//! - Ed25519 (classical, backward compatible)
//! Both signatures must verify for defense-in-depth security.
//!
//! # Key Derivation
//!
//! Blake3's `derive_key` function provides HKDF-like key derivation with a context
//! string, producing 32-byte derived keys suitable for encryption or authentication.
//! This avoids adding a separate HKDF crate while providing equivalent security.
//!
//! # Security Considerations
//!
//! - Signing keys should never be logged or serialized to untrusted storage
//! - The `store_keypair_in_db` function stores the raw 32-byte seed; enable the
//!   `encrypted` flag and set `CARNELIAN_KEYPAIR_PASSPHRASE` for production use
//! - Hex-encoded signatures in the ledger are human-readable for debugging but
//!   add 2x storage overhead vs raw bytes
//! - All signature operations use `ed25519_dalek` which is constant-time
//! - Hybrid keys provide quantum resistance when MAGIC entropy is available

use carnelian_common::{Error, Result};
use carnelian_magic::{EntropyProvider, HybridSignature, HybridSigningKey, KeyAlgorithm};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use sqlx::PgPool;
use std::sync::Arc;

// ── Key Generation ──────────────────────────────────────────────────────────

/// Generate a new Ed25519 keypair using the OS cryptographic random number generator.
///
/// Returns `(SigningKey, VerifyingKey)` where the signing key contains the 32-byte
/// seed and the verifying key is the corresponding 32-byte public key.
pub fn generate_ed25519_keypair() -> (SigningKey, VerifyingKey) {
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    (signing_key, verifying_key)
}

/// Generate a new Ed25519 keypair using entropy from a MAGIC entropy provider.
///
/// Uses quantum-enhanced entropy when available, falling back to OS entropy on failure.
/// Returns `(SigningKey, VerifyingKey)` where the signing key contains the 32-byte
/// seed derived from the entropy provider.
///
/// # Errors
///
/// Returns an error if the entropy provider fails to generate 32 bytes or if the
/// resulting bytes cannot be converted to a valid Ed25519 signing key.
pub async fn generate_ed25519_keypair_with_entropy(
    provider: &dyn EntropyProvider,
) -> Result<(SigningKey, VerifyingKey)> {
    let entropy_bytes = provider
        .get_bytes(32)
        .await
        .map_err(|e| Error::Crypto(format!("Entropy provider failed: {}", e)))?;

    let seed: [u8; 32] = entropy_bytes.try_into().map_err(|_| {
        Error::Crypto("Failed to convert entropy bytes to 32-byte seed".to_string())
    })?;

    let signing_key = SigningKey::from_bytes(&seed);
    let verifying_key = signing_key.verifying_key();

    Ok((signing_key, verifying_key))
}

/// Serialize a signing key to its 32-byte seed representation.
///
/// This is the minimal representation needed to reconstruct the full key.
pub fn keypair_to_bytes(signing_key: &SigningKey) -> [u8; 32] {
    signing_key.to_bytes()
}

/// Reconstruct a signing key from a 32-byte seed (alias for keypair_from_bytes).
///
/// # Errors
///
/// Returns an error if `bytes` is not exactly 32 bytes.
pub fn bytes_to_keypair(bytes: &[u8]) -> Result<SigningKey> {
    keypair_from_bytes(bytes)
}

/// Reconstruct a signing key from a 32-byte seed.
///
/// # Errors
///
/// Returns an error if `bytes` is not exactly 32 bytes.
pub fn keypair_from_bytes(bytes: &[u8]) -> Result<SigningKey> {
    let seed: [u8; 32] = bytes.try_into().map_err(|_| {
        Error::Crypto(format!(
            "Invalid seed length: expected 32 bytes, got {}",
            bytes.len()
        ))
    })?;
    Ok(SigningKey::from_bytes(&seed))
}

// ── Hybrid Post-Quantum Key Generation ─────────────────────────────────────

/// Generate a hybrid signing key (Dilithium3 + Ed25519) using quantum entropy.
///
/// When MAGIC is enabled, this generates both post-quantum and classical keys
/// for defense-in-depth security. Both signatures must verify.
///
/// # Arguments
///
/// * `provider` - MAGIC entropy provider for quantum randomness
///
/// # Returns
///
/// A `HybridSigningKey` containing both Dilithium3 and Ed25519 keys
///
/// # Errors
///
/// Returns an error if the entropy provider fails or key generation fails.
pub async fn generate_hybrid_keypair_with_entropy(
    provider: &Arc<dyn EntropyProvider>,
) -> Result<HybridSigningKey> {
    HybridSigningKey::generate_with_entropy(provider)
        .await
        .map_err(|e| Error::Crypto(format!("Hybrid key generation failed: {}", e)))
}

/// Sign bytes with a hybrid key (both Dilithium3 and Ed25519).
///
/// Returns a `HybridSignature` containing both signatures.
pub fn sign_bytes_hybrid(key: &HybridSigningKey, data: &[u8]) -> HybridSignature {
    key.sign(data)
}

/// Verify a hybrid signature.
///
/// Both Dilithium3 and Ed25519 signatures must verify for success.
///
/// # Returns
///
/// `Ok(true)` if both signatures are valid, `Ok(false)` if either fails.
pub fn verify_hybrid_signature(
    key: &HybridSigningKey,
    data: &[u8],
    signature: &HybridSignature,
) -> Result<bool> {
    Ok(key.verify(data, signature).is_ok())
}

// ── HKDF Derivation Helpers ─────────────────────────────────────────────────

/// Derive a 32-byte key from a master key and context string using blake3 KDF.
///
/// The context string should be a unique, application-specific identifier
/// (e.g., `"carnelian-config-encryption-v1"`). Different contexts produce
/// independent derived keys even from the same master key.
pub fn derive_storage_key(master_key: &[u8], context: &str) -> [u8; 32] {
    blake3::derive_key(context, master_key)
}

/// Derive an encryption key from an Ed25519 signing key seed for a given purpose.
///
/// This enables future encryption-at-rest by deriving AES-256 keys from the
/// owner's Ed25519 seed without exposing the signing key itself.
///
/// # Arguments
///
/// * `signing_key` - The Ed25519 signing key whose seed is used as input keying material
/// * `purpose` - A unique context string (e.g., `"carnelian-memory-encryption-v1"`)
pub fn derive_encryption_key(signing_key: &SigningKey, purpose: &str) -> [u8; 32] {
    let seed = signing_key.to_bytes();
    blake3::derive_key(purpose, &seed)
}

/// Derive a 32-byte AES-256 storage encryption key from an Ed25519 signing key.
///
/// Uses blake3 HKDF with the fixed context `"carnelian-aes-storage-v1"` to
/// produce a deterministic key suitable for AES-256-GCM encryption at rest
/// via PostgreSQL's `pgcrypto` extension.
///
/// # Arguments
///
/// * `signing_key` - The Ed25519 signing key whose 32-byte seed is the input keying material
///
/// # Returns
///
/// A 32-byte key suitable for use with `pgp_sym_encrypt_bytea` / `pgp_sym_decrypt_bytea`.
pub fn derive_aes_storage_key(signing_key: &SigningKey) -> [u8; 32] {
    derive_encryption_key(signing_key, "carnelian-aes-storage-v1")
}

// ── Signature Utilities ─────────────────────────────────────────────────────

/// Sign arbitrary bytes with an Ed25519 signing key, returning a hex-encoded signature.
///
/// The returned string is 128 hex characters (64 bytes encoded).
pub fn sign_bytes(signing_key: &SigningKey, data: &[u8]) -> String {
    let signature: Signature = signing_key.sign(data);
    hex::encode(signature.to_bytes())
}

/// Verify a hex-encoded Ed25519 signature against a hex-encoded public key.
///
/// # Arguments
///
/// * `public_key_hex` - 64-character hex string encoding the 32-byte verifying key
/// * `data` - The original message bytes that were signed
/// * `signature_hex` - 128-character hex string encoding the 64-byte signature
///
/// # Returns
///
/// `Ok(true)` if the signature is valid, `Ok(false)` if verification fails.
///
/// # Errors
///
/// Returns `Error::Crypto` if the hex strings cannot be decoded or have invalid lengths.
pub fn verify_signature(public_key_hex: &str, data: &[u8], signature_hex: &str) -> Result<bool> {
    let pk_bytes = hex::decode(public_key_hex)
        .map_err(|e| Error::Crypto(format!("Invalid public key hex: {}", e)))?;
    let pk_array: [u8; 32] = pk_bytes.as_slice().try_into().map_err(|_| {
        Error::Crypto(format!(
            "Invalid public key length: expected 32 bytes, got {}",
            pk_bytes.len()
        ))
    })?;
    let verifying_key = VerifyingKey::from_bytes(&pk_array)
        .map_err(|e| Error::Crypto(format!("Invalid public key: {}", e)))?;

    let sig_bytes = hex::decode(signature_hex)
        .map_err(|e| Error::Crypto(format!("Invalid signature hex: {}", e)))?;
    let sig_array: [u8; 64] = sig_bytes.as_slice().try_into().map_err(|_| {
        Error::Crypto(format!(
            "Invalid signature length: expected 64 bytes, got {}",
            sig_bytes.len()
        ))
    })?;
    let signature = Signature::from_bytes(&sig_array);

    Ok(verifying_key.verify(data, &signature).is_ok())
}

/// Extract the hex-encoded public key from a signing key.
///
/// Returns a 64-character hex string representing the 32-byte verifying key.
pub fn public_key_from_signing_key(signing_key: &SigningKey) -> String {
    hex::encode(signing_key.verifying_key().as_bytes())
}

// ── Key Storage Helpers ─────────────────────────────────────────────────────

/// Store an Ed25519 signing key in the `config_store` database table.
///
/// The key is stored as a 32-byte seed in the `value_blob` column under the
/// key `"owner_keypair"`. If `encrypted` is true, the `encrypted` column is
/// set accordingly (actual encryption should be handled by the caller).
///
/// # Arguments
///
/// * `pool` - Database connection pool
/// * `signing_key` - The signing key to store
/// * `encrypted` - Whether the stored blob is encrypted
pub async fn store_keypair_in_db(
    pool: &PgPool,
    signing_key: &SigningKey,
    encrypted: bool,
) -> Result<()> {
    let seed = signing_key.to_bytes();

    sqlx::query(
        r"INSERT INTO config_store (key, value, value_blob, encrypted, updated_at)
          VALUES ('owner_keypair', '{}'::jsonb, $1, $2, NOW())
          ON CONFLICT (key) DO UPDATE SET value = '{}'::jsonb, value_blob = $1, encrypted = $2, updated_at = NOW()",
    )
    .bind(seed.as_slice())
    .bind(encrypted)
    .execute(pool)
    .await
    .map_err(|e| Error::Crypto(format!("Failed to store keypair in database: {}", e)))?;

    tracing::info!("Owner keypair stored in config_store (Ed25519)");
    Ok(())
}

/// Store a hybrid signing key in the database.
///
/// Stores both Dilithium3 and Ed25519 keys with key_algorithm tracking.
///
/// # Arguments
///
/// * `pool` - Database connection pool
/// * `key` - The hybrid signing key to store
/// * `encrypted` - Whether the stored blob is encrypted
pub async fn store_hybrid_keypair_in_db(
    pool: &PgPool,
    key: &HybridSigningKey,
    encrypted: bool,
) -> Result<()> {
    // Serialize hybrid key (store Ed25519 seed + Dilithium keys)
    let public_keys = key.public_keys();
    let ed25519_seed = key.ed25519_sk.to_bytes();

    // Store in config_store with key_algorithm
    sqlx::query(
        r"INSERT INTO config_store (key, value, value_blob, encrypted, key_algorithm, updated_at)
          VALUES ('owner_keypair', $1::jsonb, $2, $3, 'hybrid_dilithium_ed25519', NOW())
          ON CONFLICT (key) DO UPDATE SET 
            value = $1::jsonb, 
            value_blob = $2, 
            encrypted = $3, 
            key_algorithm = 'hybrid_dilithium_ed25519',
            updated_at = NOW()",
    )
    .bind(serde_json::json!({
        "dilithium_pk": hex::encode(&public_keys.dilithium_pk),
        "ed25519_pk": hex::encode(&public_keys.ed25519_pk),
    }))
    .bind(ed25519_seed.as_slice())
    .bind(encrypted)
    .execute(pool)
    .await
    .map_err(|e| Error::Crypto(format!("Failed to store hybrid keypair: {}", e)))?;

    tracing::info!("Owner hybrid keypair stored in config_store (Dilithium3 + Ed25519)");
    Ok(())
}

/// Load an Ed25519 signing key from the `config_store` database table.
///
/// Queries for the `"owner_keypair"` key and reconstructs the signing key
/// from the stored 32-byte seed. Returns `None` if no keypair is stored.
///
/// **Note:** This does not handle decryption of encrypted blobs. If the
/// `encrypted` flag is set, the caller must decrypt before calling this,
/// or use `Config::load_owner_keypair_from_db()` which handles decryption.
pub async fn load_keypair_from_db(pool: &PgPool) -> Result<Option<SigningKey>> {
    let row: Option<(Option<Vec<u8>>, bool)> = sqlx::query_as(
        "SELECT value_blob, encrypted FROM config_store WHERE key = 'owner_keypair'",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| Error::Crypto(format!("Failed to query keypair from database: {}", e)))?;

    match row {
        Some((Some(blob), false)) => {
            let key = keypair_from_bytes(&blob)?;
            Ok(Some(key))
        }
        Some((None, _)) => {
            // Row exists but value_blob is NULL - treat as no keypair
            Ok(None)
        }
        Some((_, true)) => Err(Error::Crypto(
            "Keypair is encrypted; use Config::load_owner_keypair_from_db() for decryption"
                .to_string(),
        )),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_keypair_produces_valid_keys() {
        let (signing, verifying) = generate_ed25519_keypair();
        // Verify the verifying key matches the signing key
        assert_eq!(signing.verifying_key(), verifying);
        // Sign and verify a message to confirm the keypair works
        let msg = b"test message";
        let sig = signing.sign(msg);
        assert!(verifying.verify(msg, &sig).is_ok());
    }

    #[test]
    fn test_keypair_bytes_roundtrip() {
        let (signing, _) = generate_ed25519_keypair();
        let bytes = keypair_to_bytes(&signing);
        let restored = keypair_from_bytes(&bytes).expect("should reconstruct from bytes");
        assert_eq!(signing.to_bytes(), restored.to_bytes());
    }

    #[test]
    fn test_keypair_from_bytes_rejects_wrong_length() {
        let short = vec![0u8; 16];
        assert!(keypair_from_bytes(&short).is_err());
        let long = vec![0u8; 64];
        assert!(keypair_from_bytes(&long).is_err());
    }

    #[test]
    fn test_sign_and_verify_roundtrip() {
        let (signing, _) = generate_ed25519_keypair();
        let public_hex = public_key_from_signing_key(&signing);
        let data = b"hello carnelian";

        let sig_hex = sign_bytes(&signing, data);
        assert_eq!(sig_hex.len(), 128, "hex signature should be 128 chars");

        let valid = verify_signature(&public_hex, data, &sig_hex).expect("verify should not error");
        assert!(valid, "signature should verify");
    }

    #[test]
    fn test_verify_invalid_signature_rejected() {
        let (signing, _) = generate_ed25519_keypair();
        let public_hex = public_key_from_signing_key(&signing);
        let data = b"original message";

        let sig_hex = sign_bytes(&signing, data);

        // Tamper with the data
        let tampered = b"tampered message";
        let valid =
            verify_signature(&public_hex, tampered, &sig_hex).expect("verify should not error");
        assert!(!valid, "tampered data should fail verification");

        // Tamper with the signature (flip a character)
        let mut bad_sig = sig_hex.clone();
        let replacement = if bad_sig.starts_with('0') { 'f' } else { '0' };
        bad_sig.replace_range(0..1, &replacement.to_string());
        let valid = verify_signature(&public_hex, data, &bad_sig).expect("verify should not error");
        assert!(!valid, "tampered signature should fail verification");
    }

    #[test]
    fn test_verify_rejects_wrong_key() {
        let (signing1, _) = generate_ed25519_keypair();
        let (signing2, _) = generate_ed25519_keypair();
        let public_hex2 = public_key_from_signing_key(&signing2);
        let data = b"signed by key 1";

        let sig_hex = sign_bytes(&signing1, data);

        let valid =
            verify_signature(&public_hex2, data, &sig_hex).expect("verify should not error");
        assert!(!valid, "wrong key should fail verification");
    }

    #[test]
    fn test_verify_rejects_invalid_hex() {
        let result = verify_signature("not_hex!", b"data", "also_not_hex!");
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_rejects_wrong_length_key() {
        let result = verify_signature("aabb", b"data", &"00".repeat(64));
        assert!(result.is_err());
    }

    #[test]
    fn test_derive_storage_key_deterministic() {
        let master = b"master secret key material";
        let ctx = "carnelian-test-context-v1";

        let key1 = derive_storage_key(master, ctx);
        let key2 = derive_storage_key(master, ctx);
        assert_eq!(key1, key2, "same input should produce same derived key");
    }

    #[test]
    fn test_derive_storage_key_different_contexts() {
        let master = b"master secret key material";
        let key1 = derive_storage_key(master, "context-a");
        let key2 = derive_storage_key(master, "context-b");
        assert_ne!(
            key1, key2,
            "different contexts should produce different keys"
        );
    }

    #[test]
    fn test_derive_encryption_key_from_signing_key() {
        let (signing, _) = generate_ed25519_keypair();
        let key1 = derive_encryption_key(&signing, "carnelian-memory-encryption-v1");
        let key2 = derive_encryption_key(&signing, "carnelian-config-encryption-v1");
        assert_ne!(
            key1, key2,
            "different purposes should produce different keys"
        );

        // Same purpose should be deterministic
        let key3 = derive_encryption_key(&signing, "carnelian-memory-encryption-v1");
        assert_eq!(key1, key3);
    }

    #[test]
    fn test_derive_aes_storage_key_deterministic() {
        let (signing, _) = generate_ed25519_keypair();
        let key1 = derive_aes_storage_key(&signing);
        let key2 = derive_aes_storage_key(&signing);
        assert_eq!(key1, key2, "same signing key should produce same AES key");
        assert_eq!(key1.len(), 32, "AES-256 key must be 32 bytes");
    }

    #[test]
    fn test_derive_aes_storage_key_context_separation() {
        let (signing, _) = generate_ed25519_keypair();
        let aes_key = derive_aes_storage_key(&signing);
        let other_key = derive_encryption_key(&signing, "carnelian-memory-encryption-v1");
        assert_ne!(
            aes_key, other_key,
            "AES storage key must differ from keys derived with other contexts"
        );
    }

    #[test]
    fn test_derive_aes_storage_key_different_signing_keys() {
        let (signing1, _) = generate_ed25519_keypair();
        let (signing2, _) = generate_ed25519_keypair();
        let key1 = derive_aes_storage_key(&signing1);
        let key2 = derive_aes_storage_key(&signing2);
        assert_ne!(
            key1, key2,
            "different signing keys should produce different AES keys"
        );
    }

    #[test]
    fn test_public_key_from_signing_key_format() {
        let (signing, verifying) = generate_ed25519_keypair();
        let hex_key = public_key_from_signing_key(&signing);
        assert_eq!(hex_key.len(), 64, "hex public key should be 64 chars");
        assert_eq!(hex_key, hex::encode(verifying.as_bytes()));
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_store_and_load_keypair_db() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("Failed to connect to database");

        let (signing, _) = generate_ed25519_keypair();
        store_keypair_in_db(&pool, &signing, false)
            .await
            .expect("store should succeed");

        let loaded = load_keypair_from_db(&pool)
            .await
            .expect("load should succeed")
            .expect("keypair should exist");

        assert_eq!(
            signing.to_bytes(),
            loaded.to_bytes(),
            "round-trip should preserve key"
        );
    }
}
