//! Integration tests for PQC hybrid sign/verify flow

use carnelian_magic::{EntropyProvider, HybridSigningKey, KyberKem, MixedEntropyProvider};
use std::sync::Arc;

#[tokio::test]
async fn test_pqc_hybrid_sign_verify_e2e() {
    // Create OS-only entropy provider for offline testing
    let provider: Arc<dyn EntropyProvider> = Arc::new(MixedEntropyProvider::new_os_only());

    // Generate hybrid signing key with quantum entropy
    let key = HybridSigningKey::generate_with_entropy(&provider)
        .await
        .expect("Failed to generate hybrid signing key");

    // Sign a message
    let message = b"End-to-end PQC test message for Carnelian v1.0.0";
    let signature = key.sign(message);

    // Verify both Dilithium3 and Ed25519 signatures
    key.verify(message, &signature)
        .expect("Hybrid signature verification should succeed");

    // Verify signature fails on tampered message
    let tampered_message = b"Tampered message";
    assert!(
        key.verify(tampered_message, &signature).is_err(),
        "Signature should fail on tampered message"
    );

    // Verify public key export
    let public_keys = key.public_keys();
    assert_eq!(
        public_keys.ed25519_pk.len(),
        32,
        "Ed25519 public key should be 32 bytes"
    );
    assert!(
        public_keys.dilithium_pk.len() > 0,
        "Dilithium public key should exist"
    );
}

#[tokio::test]
async fn test_kyber_kem_e2e() {
    let provider: Arc<dyn EntropyProvider> = Arc::new(MixedEntropyProvider::new_os_only());

    // Generate Kyber KEM keypair
    let kem = KyberKem::generate_with_entropy(&provider)
        .await
        .expect("Failed to generate Kyber KEM");

    // Encapsulate to create shared secret
    let (ciphertext, shared_secret_1) = kem.encapsulate();
    assert!(!ciphertext.is_empty(), "Ciphertext should not be empty");

    // Decapsulate to recover shared secret
    let shared_secret_2 = kem
        .decapsulate(&ciphertext)
        .expect("Decapsulation should succeed");

    // Shared secrets must match
    assert_eq!(
        shared_secret_1, shared_secret_2,
        "Encapsulated and decapsulated secrets must match"
    );

    // Verify decapsulation fails on invalid ciphertext
    let invalid_ct = vec![0u8; 10];
    assert!(
        kem.decapsulate(&invalid_ct).is_err(),
        "Decapsulation should fail on invalid ciphertext"
    );
}
