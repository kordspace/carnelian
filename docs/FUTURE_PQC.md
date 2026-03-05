# Post-Quantum Cryptography — v1.1.0 Roadmap

**Status:** 🔄 Planned for v1.1.0  
**Last Updated:** March 3, 2026

---

## Overview

Carnelian OS v1.0.0 ships with classical Ed25519 cryptography as the default signing algorithm (`KeyAlgorithm::Ed25519`). When MAGIC quantum entropy is enabled, Ed25519 keys are seeded from quantum sources, but the underlying algorithm remains vulnerable to Shor's algorithm on future quantum computers.

This document outlines the **post-quantum cryptography (PQC) migration roadmap** for v1.1.0, v1.2.0, and v2.0.0, leveraging the **fully implemented** NIST-standardized primitives already available in `carnelian-magic`.

---

## Current v1.0.0 State

### Default Cryptography

- **Signing Algorithm:** Ed25519 (256-bit elliptic curve)
- **Encryption:** AES-256-GCM derived from Ed25519 seed via blake3 HKDF
- **Key Algorithm Enum:** `KeyAlgorithm::Ed25519` (default)
- **MAGIC Integration:** When `config.magic.enabled = true`, Ed25519 seeds are derived from quantum entropy sources (Quantum Origin, Quantinuum H2, IBM Quantum)

### Quantum Security Posture

| Component | Algorithm | Quantum-Resistant | MAGIC Quantum Seeding |
|-----------|-----------|-------------------|----------------------|
| Owner Keypair | Ed25519 | ❌ No | ✅ Yes (when enabled) |
| Ledger Signatures | Ed25519 | ❌ No | ✅ Yes (when enabled) |
| Config Encryption | AES-256 (Ed25519-derived) | ⚠️ Partial | ✅ Yes (when enabled) |
| Memory Encryption | AES-256 (Ed25519-derived) | ⚠️ Partial | ✅ Yes (when enabled) |

**Reference:** See `docs/MAGIC.md` "Quantum Security Posture" section for full entropy provider details.

---

## Implemented Primitives (Available in `carnelian-magic`)

The following **production-ready** post-quantum cryptographic primitives are fully implemented and tested in `crates/carnelian-magic/src/pqc.rs`:

### `HybridSigningKey`

**Purpose:** Dual-signature scheme combining quantum-resistant and classical algorithms for defense-in-depth.

**Components:**
- **Primary:** CRYSTALS-Dilithium3 (NIST Level 3 security, ~192-bit quantum resistance)
- **Fallback:** Ed25519 (classical elliptic curve, backward compatibility)

**Methods:**
- `generate_with_entropy(&entropy_provider)` — Generate hybrid keypair from quantum entropy (async)
- `sign(&self, message)` — Produce `HybridSignature` containing both Dilithium3 and Ed25519 signatures
- `verify(&self, message, signature)` — Verify both signatures on this key (fail if either is invalid)
- `public_keys(&self)` — Export `HybridPublicKey` containing both public keys for storage
- `derive_aes_storage_key(&self)` — Derive AES-256 key from Ed25519 component (maintains v1.0.0 compatibility)

**Key Sizes:**
- Dilithium3 Public Key: 1952 bytes
- Dilithium3 Secret Key: 4000 bytes
- Dilithium3 Signature: 3293 bytes
- Ed25519 Public Key: 32 bytes
- Ed25519 Secret Key: 32 bytes
- Ed25519 Signature: 64 bytes

### `KyberKem`

**Purpose:** Quantum-resistant key encapsulation mechanism for secure key exchange.

**Algorithm:** CRYSTALS-Kyber1024 (NIST Level 5 security, ~256-bit quantum resistance)

**Methods:**
- `generate_with_entropy(&entropy_provider)` — Generate Kyber1024 keypair from quantum entropy (async)
- `encapsulate(&self)` — Encapsulate a shared secret using this key's public key, return `(ciphertext, shared_secret)`
- `decapsulate(&self, ciphertext)` — Decapsulate ciphertext using this key's secret key to recover shared secret
- `public_key_bytes(&self)` — Export public key bytes for storage or transmission

**Key Sizes:**
- Public Key: 1568 bytes
- Secret Key: 3168 bytes
- Ciphertext: 1568 bytes
- Shared Secret: 32 bytes (suitable for AES-256-GCM)

### `KeyAlgorithm` Enum

**Purpose:** Track which cryptographic algorithm is used for stored keys.

**Variants:**
- `Ed25519` — Classical Ed25519 (v1.0.0 default)
- `HybridDilithiumEd25519` — Hybrid Dilithium3 + Ed25519 (v1.1.0 target)
- `Dilithium3` — Pure Dilithium3 (v2.0.0 target)

**Storage:** Persisted in `config_store.key_algorithm` column (added in migration `00000000000018_pqc_key_algorithm.sql`)

---

## v1.1.0 Activation Plan (Hybrid Phase)

### Goal

Enable **opt-in hybrid post-quantum cryptography** when MAGIC is enabled, while maintaining full backward compatibility with v1.0.0 Ed25519 deployments.

### Activation Steps

#### 1. Feature Flag: `QuantumResistantCrypto`

Add a new feature flag to `Cargo.toml`:

```toml
[features]
default = ["desktop-ui"]
quantum-resistant-crypto = ["carnelian-magic/pqc"]
```

When enabled, `carnelian-core` will use `HybridSigningKey` instead of Ed25519 for new key generation.

#### 2. Wire `HybridSigningKey` into `crypto.rs`

Update `crates/carnelian-core/src/crypto.rs`:

```rust
#[cfg(feature = "quantum-resistant-crypto")]
pub async fn generate_owner_keypair_with_entropy(
    entropy_provider: Arc<dyn EntropyProvider>,
) -> Result<HybridSigningKey> {
    HybridSigningKey::generate_with_entropy(entropy_provider).await
}

#[cfg(not(feature = "quantum-resistant-crypto"))]
pub async fn generate_owner_keypair_with_entropy(
    entropy_provider: Arc<dyn EntropyProvider>,
) -> Result<SigningKey> {
    generate_ed25519_keypair_with_entropy(entropy_provider).await
}
```

#### 3. Database Schema: `key_algorithm` Column

**Migration:** `db/migrations/00000000000018_pqc_key_algorithm.sql` (already exists)

Adds `key_algorithm TEXT NOT NULL DEFAULT 'ed25519'` to `config_store` table to track which algorithm is in use.

#### 4. Dual-Sign Ledger Entries

Update `crates/carnelian-core/src/ledger.rs` to sign all privileged events with both Dilithium3 and Ed25519:

```rust
let signature = if let Some(hybrid_key) = &config.hybrid_signing_key {
    hybrid_key.sign(event_bytes).await?
} else {
    // Fallback to Ed25519 for v1.0.0 compatibility
    config.owner_signing_key.sign(event_bytes)
};
```

#### 5. Runtime Configuration

When `config.magic.enabled = true` **and** `quantum-resistant-crypto` feature is enabled:
- New instances generate `HybridSigningKey` on first run
- Existing instances continue using Ed25519 until manual migration
- `key_algorithm` column reflects current state

### Prerequisites

- MAGIC must be enabled (`config.magic.enabled = true`)
- Valid quantum entropy provider configured (Quantum Origin API key or Quantinuum H2 credentials)
- `quantum-resistant-crypto` feature flag enabled at compile time

---

## Migration CLI Command

### Planned Command: `carnelian crypto migrate --to hybrid`

**Purpose:** Migrate an existing v1.0.0 Ed25519 deployment to v1.1.0 hybrid PQC.

**Prerequisites:**
1. MAGIC enabled with valid quantum entropy provider
2. `quantum-resistant-crypto` feature compiled in
3. Database backup completed (migration is irreversible)

**Migration Process:**

1. **Generate New Hybrid Key:**
   - Derive 128 bytes of quantum entropy
   - Generate `HybridSigningKey` (Dilithium3 + Ed25519)
   - Store in `config_store` with `key_algorithm = 'hybrid_dilithium_ed25519'`

2. **Re-Sign Critical Data:**
   - Re-sign all ledger events with hybrid signature
   - Update `ledger_events.signature` column to store both signatures
   - Verify all historical signatures remain valid

3. **Update Encryption Keys:**
   - Derive new AES-256 key from hybrid key's Ed25519 component
   - Re-encrypt all `config_store` encrypted values
   - Re-encrypt memory snapshots and run logs

4. **Verification:**
   - Verify all ledger signatures with both algorithms
   - Confirm config decryption works
   - Run integrity checks on encrypted data

**Rollback:** Not supported — migration is one-way. Backup before migrating.

**Example:**

```bash
# Backup database
carnelian db backup --output carnelian-v1.0.0-backup.sql

# Migrate to hybrid PQC
carnelian crypto migrate --to hybrid

# Verify migration
carnelian crypto verify --algorithm hybrid
```

---

## v1.2.0 Encryption Upgrade

### Goal

Replace AES-256 key derivation with **Kyber1024 key encapsulation** for quantum-resistant encryption-at-rest.

### Changes

#### 1. Kyber KEM for Encryption Keys

Replace `derive_aes_storage_key()` with Kyber-based key exchange:

```rust
// Current (v1.0.0 - v1.1.0)
let aes_key = signing_key.derive_aes_storage_key();

// Future (v1.2.0)
let kyber_kem = KyberKem::generate_with_entropy(entropy_provider).await?;
let (ciphertext, shared_secret) = kyber_kem.encapsulate(&public_key)?;
let aes_key = blake3::derive_key("carnelian-kyber-aes-v1", &shared_secret);
```

#### 2. Store Kyber Public Key

Add `kyber_public_key BYTEA` column to `config_store` to persist the Kyber public key for future key rotations.

#### 3. CLI Command: `carnelian crypto rotate --algorithm kyber`

**Purpose:** Rotate encryption keys to use Kyber1024 KEM.

**Process:**
1. Generate new `KyberKem` keypair
2. Encapsulate new shared secret
3. Derive new AES-256 key from shared secret
4. Re-encrypt all stored data with new key
5. Store Kyber ciphertext in database

### Backward Compatibility

v1.2.0 will support **dual-mode decryption**:
- Attempt Kyber decapsulation first
- Fall back to Ed25519-derived key if Kyber fails
- Allow gradual migration via `carnelian crypto rotate`

---

## v2.0.0 Full PQC

### Goal

Remove all classical cryptographic dependencies and operate in **pure post-quantum mode**.

### Breaking Changes

1. **Remove Ed25519:**
   - `KeyAlgorithm::Ed25519` no longer supported
   - All keys must be `Dilithium3` or hybrid
   - Migration from v1.x required before upgrading

2. **Remove X25519:**
   - Replace all ECDH key exchange with Kyber1024
   - Update WebSocket and API authentication

3. **Dilithium-Only Signatures:**
   - `HybridSigningKey` deprecated in favor of pure `Dilithium3`
   - Ledger signatures use only Dilithium3
   - Smaller signature size (3293 bytes vs 3357 bytes for hybrid)

4. **Kyber-Only Key Exchange:**
   - All encryption uses Kyber1024 KEM
   - No fallback to classical key derivation

### Migration Path

```
v1.0.0 (Ed25519)
    ↓ carnelian crypto migrate --to hybrid
v1.1.0 (Hybrid Dilithium3 + Ed25519)
    ↓ carnelian crypto rotate --algorithm kyber
v1.2.0 (Hybrid + Kyber KEM)
    ↓ carnelian crypto migrate --to dilithium3
v2.0.0 (Pure PQC: Dilithium3 + Kyber1024)
```

---

## Security Note

**Quantum Entropy Requirement:**

When MAGIC is enabled, all cryptographic key material **MUST** be derived from quantum entropy sources to achieve the full security guarantees of post-quantum algorithms. Using OS CSPRNG as a fallback reduces security to classical levels.

**Recommended Providers (in priority order):**
1. **Quantum Origin** (Quantinuum) — REST API, easiest setup
2. **Quantinuum H2** — Hardware quantum computer or emulator
3. **IBM Quantum** (via Qiskit) — Requires IBM Quantum account

**Threat Model:**

Post-quantum cryptography protects against:
- **Shor's Algorithm:** Breaks RSA, ECDSA, ECDH in polynomial time
- **Grover's Algorithm:** Reduces symmetric key security by half (AES-256 → AES-128 equivalent)
- **Harvest Now, Decrypt Later:** Adversaries storing encrypted data for future quantum decryption

PQC does **not** protect against:
- Side-channel attacks (timing, power analysis)
- Implementation bugs in cryptographic libraries
- Compromised entropy sources
- Social engineering or credential theft

---

## Cross-References

- **Architecture Review:** `docs/SECURITY_ARCHITECTURE_REVIEW_V1.md` — Comprehensive PQC migration analysis (§1, §6.2, §7)
- **MAGIC Subsystem:** `docs/MAGIC.md` — Quantum entropy provider setup and configuration
- **Security Policy:** `SECURITY.md` — Supported versions and vulnerability reporting
- **Implementation:** `crates/carnelian-magic/src/pqc.rs` — Production-ready PQC primitives
- **Database Schema:** `db/migrations/00000000000018_pqc_key_algorithm.sql` — Key algorithm tracking

---

## Timeline

| Version | Target Date | Status | Key Features |
|---------|-------------|--------|--------------|
| v1.0.0 | March 2026 | ✅ Released | Ed25519 + MAGIC quantum seeding |
| v1.1.0 | Q2 2026 | 🔄 Planned | Hybrid Dilithium3 + Ed25519 signatures |
| v1.2.0 | Q3 2026 | 📋 Roadmap | Kyber1024 KEM for encryption |
| v2.0.0 | Q4 2026 | 📋 Roadmap | Pure PQC (Dilithium3 + Kyber1024) |

---

**For questions or contributions related to PQC migration, see `CONTRIBUTING.md` or contact the Carnelian security team.**
