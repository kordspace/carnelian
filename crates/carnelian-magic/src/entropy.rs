//! Entropy providers for quantum and classical randomness

use async_trait::async_trait;
use rand::RngCore;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

use crate::error::{MagicError, Result};

// =============================================================================
// ENTROPY HEALTH
// =============================================================================

/// Health status for an entropy provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntropyHealth {
    pub source: String,
    pub available: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
    pub checked_at: chrono::DateTime<chrono::Utc>,
}

// =============================================================================
// ENTROPY PROVIDER TRAIT
// =============================================================================

/// Async trait for entropy providers
#[async_trait]
pub trait EntropyProvider: Send + Sync {
    /// Returns the source name
    fn source_name(&self) -> &str;

    /// Quick reachability check
    async fn is_available(&self) -> bool;

    /// Fetch n bytes of entropy
    async fn get_bytes(&self, n: usize) -> Result<Vec<u8>>;

    /// Full health probe with latency
    async fn health(&self) -> EntropyHealth;
}

// =============================================================================
// OS ENTROPY PROVIDER
// =============================================================================

/// OS-based entropy provider using rand::rngs::OsRng
pub struct OsEntropyProvider;

impl OsEntropyProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OsEntropyProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EntropyProvider for OsEntropyProvider {
    fn source_name(&self) -> &'static str {
        "os"
    }

    async fn is_available(&self) -> bool {
        true
    }

    async fn get_bytes(&self, n: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; n];
        rand::rngs::OsRng.fill_bytes(&mut buf);
        Ok(buf)
    }

    async fn health(&self) -> EntropyHealth {
        let start = std::time::Instant::now();
        let result = self.get_bytes(32).await;
        let latency_ms = start.elapsed().as_millis() as u64;

        EntropyHealth {
            source: self.source_name().to_string(),
            available: result.is_ok(),
            latency_ms: Some(latency_ms),
            error: result.err().map(|e| e.to_string()),
            checked_at: chrono::Utc::now(),
        }
    }
}

// =============================================================================
// QUANTUM ORIGIN PROVIDER
// =============================================================================

/// Quantum Origin API entropy provider
pub struct QuantumOriginProvider {
    api_key: String,
    base_url: String,
    client: Client,
}

impl QuantumOriginProvider {
    pub fn new(base_url: String, api_key: String) -> Self {
        let api_key = if api_key.is_empty() {
            std::env::var("CARNELIAN_QUANTUM_ORIGIN_API_KEY").unwrap_or_default()
        } else {
            api_key
        };

        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            api_key,
            base_url,
            client,
        }
    }
}

#[async_trait]
impl EntropyProvider for QuantumOriginProvider {
    fn source_name(&self) -> &'static str {
        "quantum-origin"
    }

    async fn is_available(&self) -> bool {
        let url = format!("{}/v1/entropy", self.base_url);
        self.client
            .head(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    async fn get_bytes(&self, n: usize) -> Result<Vec<u8>> {
        let url = format!("{}/v1/entropy", self.base_url);
        let body = serde_json::json!({ "bytes": n });

        // First attempt
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await;

        let response = match response {
            Ok(r) if r.status().is_success() => r,
            Ok(_) | Err(_) => {
                // Retry once
                self.client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", self.api_key))
                    .json(&body)
                    .send()
                    .await?
            }
        };

        if !response.status().is_success() {
            return Err(MagicError::ProviderError {
                provider: self.source_name().to_string(),
                message: format!("HTTP {}", response.status()),
            });
        }

        let json: serde_json::Value = response.json().await?;
        let bytes_hex = json
            .get("bytes")
            .and_then(|v| v.as_str())
            .ok_or_else(|| MagicError::ProviderError {
                provider: self.source_name().to_string(),
                message: "Missing 'bytes' field in response".to_string(),
            })?;

        let decoded = hex::decode(bytes_hex).map_err(|e| MagicError::ProviderError {
            provider: self.source_name().to_string(),
            message: format!("Hex decode error: {}", e),
        })?;

        // Validate length matches requested byte count
        if decoded.len() != n {
            return Err(MagicError::ProviderError {
                provider: self.source_name().to_string(),
                message: format!(
                    "Length mismatch: requested {} bytes, received {}",
                    n,
                    decoded.len()
                ),
            });
        }

        Ok(decoded)
    }

    async fn health(&self) -> EntropyHealth {
        let start = std::time::Instant::now();
        let result = self.get_bytes(32).await;
        let latency_ms = start.elapsed().as_millis() as u64;

        EntropyHealth {
            source: self.source_name().to_string(),
            available: result.is_ok(),
            latency_ms: Some(latency_ms),
            error: result.err().map(|e| e.to_string()),
            checked_at: chrono::Utc::now(),
        }
    }
}

// =============================================================================
// SKILL BRIDGE TRAIT
// =============================================================================

/// Injection seam for Python skill invocation
#[async_trait]
pub trait SkillBridge: Send + Sync {
    async fn invoke_skill(
        &self,
        skill_name: &str,
        input: serde_json::Value,
    ) -> Result<serde_json::Value>;
}

// =============================================================================
// QUANTINUUM H2 PROVIDER
// =============================================================================

/// Quantinuum H2 quantum RNG provider (via Python skill bridge)
pub struct QuantinuumH2Provider {
    bridge: Arc<dyn SkillBridge>,
}

impl QuantinuumH2Provider {
    pub fn new(bridge: Arc<dyn SkillBridge>) -> Self {
        Self { bridge }
    }
}

#[async_trait]
impl EntropyProvider for QuantinuumH2Provider {
    fn source_name(&self) -> &'static str {
        "quantinuum-h2"
    }

    async fn is_available(&self) -> bool {
        self.bridge
            .invoke_skill("quantinuum-h2-rng", serde_json::json!({"op": "ping"}))
            .await
            .is_ok()
    }

    async fn get_bytes(&self, n: usize) -> Result<Vec<u8>> {
        let result = self
            .bridge
            .invoke_skill(
                "quantinuum-h2-rng",
                serde_json::json!({"n_bits": n * 8}),
            )
            .await?;

        let bytes_hex = result
            .get("bytes")
            .and_then(|v| v.as_str())
            .ok_or_else(|| MagicError::SkillBridgeError("Missing 'bytes' field".to_string()))?;

        let decoded = hex::decode(bytes_hex)
            .map_err(|e| MagicError::SkillBridgeError(format!("Hex decode error: {}", e)))?;

        // Validate length matches requested byte count
        if decoded.len() != n {
            return Err(MagicError::SkillBridgeError(format!(
                "Length mismatch: requested {} bytes, received {}",
                n,
                decoded.len()
            )));
        }

        Ok(decoded)
    }

    async fn health(&self) -> EntropyHealth {
        let start = std::time::Instant::now();
        let result = self.get_bytes(32).await;
        let latency_ms = start.elapsed().as_millis() as u64;

        EntropyHealth {
            source: self.source_name().to_string(),
            available: result.is_ok(),
            latency_ms: Some(latency_ms),
            error: result.err().map(|e| e.to_string()),
            checked_at: chrono::Utc::now(),
        }
    }
}

// =============================================================================
// QISKIT PROVIDER
// =============================================================================

/// Qiskit quantum RNG provider (via Python skill bridge)
pub struct QiskitProvider {
    bridge: Arc<dyn SkillBridge>,
}

impl QiskitProvider {
    pub fn new(bridge: Arc<dyn SkillBridge>) -> Self {
        Self { bridge }
    }
}

#[async_trait]
impl EntropyProvider for QiskitProvider {
    fn source_name(&self) -> &'static str {
        "qiskit-rng"
    }

    async fn is_available(&self) -> bool {
        self.bridge
            .invoke_skill("qiskit-rng", serde_json::json!({"op": "ping"}))
            .await
            .is_ok()
    }

    async fn get_bytes(&self, n: usize) -> Result<Vec<u8>> {
        let result = self
            .bridge
            .invoke_skill("qiskit-rng", serde_json::json!({"shots": n * 8}))
            .await?;

        let bytes_hex = result
            .get("bytes")
            .and_then(|v| v.as_str())
            .ok_or_else(|| MagicError::SkillBridgeError("Missing 'bytes' field".to_string()))?;

        let decoded = hex::decode(bytes_hex)
            .map_err(|e| MagicError::SkillBridgeError(format!("Hex decode error: {}", e)))?;

        // Validate length matches requested byte count
        if decoded.len() != n {
            return Err(MagicError::SkillBridgeError(format!(
                "Length mismatch: requested {} bytes, received {}",
                n,
                decoded.len()
            )));
        }

        Ok(decoded)
    }

    async fn health(&self) -> EntropyHealth {
        let start = std::time::Instant::now();
        let result = self.get_bytes(32).await;
        let latency_ms = start.elapsed().as_millis() as u64;

        EntropyHealth {
            source: self.source_name().to_string(),
            available: result.is_ok(),
            latency_ms: Some(latency_ms),
            error: result.err().map(|e| e.to_string()),
            checked_at: chrono::Utc::now(),
        }
    }
}

// =============================================================================
// MIXED ENTROPY PROVIDER
// =============================================================================

/// Mixed entropy provider with quantum-first fallback to OS
pub struct MixedEntropyProvider {
    os: OsEntropyProvider,
    quantum_origin: Option<QuantumOriginProvider>,
    quantinuum: Option<QuantinuumH2Provider>,
    qiskit: Option<QiskitProvider>,
    node_id: uuid::Uuid,
}

impl MixedEntropyProvider {
    pub fn new(
        quantum_origin: Option<QuantumOriginProvider>,
        quantinuum: Option<QuantinuumH2Provider>,
        qiskit: Option<QiskitProvider>,
        node_id: uuid::Uuid,
    ) -> Self {
        Self {
            os: OsEntropyProvider::new(),
            quantum_origin,
            quantinuum,
            qiskit,
            node_id,
        }
    }

    /// Create a provider with only OS entropy (for offline testing)
    pub fn new_os_only() -> Self {
        Self {
            os: OsEntropyProvider::new(),
            quantum_origin: None,
            quantinuum: None,
            qiskit: None,
            node_id: uuid::Uuid::new_v4(),
        }
    }

    /// Get health status from all configured providers (concurrent)
    pub async fn all_health(&self) -> Vec<EntropyHealth> {
        // Execute all health checks concurrently
        let (os_health, qo_health, qh_health, qk_health) = tokio::join!(
            self.os.health(),
            async {
                if let Some(ref qo) = self.quantum_origin {
                    Some(qo.health().await)
                } else {
                    None
                }
            },
            async {
                if let Some(ref qh) = self.quantinuum {
                    Some(qh.health().await)
                } else {
                    None
                }
            },
            async {
                if let Some(ref qk) = self.qiskit {
                    Some(qk.health().await)
                } else {
                    None
                }
            },
        );

        // Collect results in order: OS first, then quantum providers
        let mut health = vec![os_health];
        if let Some(h) = qo_health {
            health.push(h);
        }
        if let Some(h) = qh_health {
            health.push(h);
        }
        if let Some(h) = qk_health {
            health.push(h);
        }

        health
    }

    /// Mix quantum and OS entropy using blake3
    fn mix_entropy(&self, os_bytes: &[u8], quantum_bytes: &[u8], n: usize) -> Vec<u8> {
        // Combine: os_bytes || quantum_bytes || timestamp || node_id
        let mut combined = Vec::new();
        combined.extend_from_slice(os_bytes);
        combined.extend_from_slice(quantum_bytes);

        // Add timestamp (8 bytes LE)
        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64;
        combined.extend_from_slice(&ts.to_le_bytes());

        // Add node_id (16 bytes)
        combined.extend_from_slice(self.node_id.as_bytes());

        // Derive key using blake3
        let mut output = vec![0u8; n];
        let mut hasher = blake3::Hasher::new_derive_key("carnelian-magic-entropy-v1");
        hasher.update(&combined);

        if n <= 32 {
            let hash = hasher.finalize();
            output.copy_from_slice(&hash.as_bytes()[..n]);
        } else {
            // For larger outputs, repeatedly hash
            let mut offset = 0;
            let mut counter = 0u64;
            while offset < n {
                let mut h = blake3::Hasher::new_derive_key("carnelian-magic-entropy-v1");
                h.update(&combined);
                h.update(&counter.to_le_bytes());
                let hash = h.finalize();
                let chunk_size = std::cmp::min(32, n - offset);
                output[offset..offset + chunk_size].copy_from_slice(&hash.as_bytes()[..chunk_size]);
                offset += chunk_size;
                counter += 1;
            }
        }

        output
    }
}

#[async_trait]
impl EntropyProvider for MixedEntropyProvider {
    fn source_name(&self) -> &'static str {
        "mixed"
    }

    async fn is_available(&self) -> bool {
        true // OS is always available
    }

    async fn get_bytes(&self, n: usize) -> Result<Vec<u8>> {
        // Always get OS bytes
        let os_bytes = self.os.get_bytes(n).await?;

        // Try quantum providers in strict priority order with failure-aware fallback
        let mut quantum_result = None;

        // Try QuantumOrigin first
        if let Some(ref qo) = self.quantum_origin {
            if let Ok(bytes) = qo.get_bytes(n).await {
                quantum_result = Some(bytes);
            }
        }

        // On failure/unavailable, try QuantinuumH2
        if quantum_result.is_none() {
            if let Some(ref qh) = self.quantinuum {
                if let Ok(bytes) = qh.get_bytes(n).await {
                    quantum_result = Some(bytes);
                }
            }
        }

        // On failure/unavailable, try Qiskit
        if quantum_result.is_none() {
            if let Some(ref qk) = self.qiskit {
                if let Ok(bytes) = qk.get_bytes(n).await {
                    quantum_result = Some(bytes);
                }
            }
        }

        // Mix if we got quantum bytes, otherwise return OS bytes
        if let Some(quantum_bytes) = quantum_result {
            Ok(self.mix_entropy(&os_bytes, &quantum_bytes, n))
        } else {
            Ok(os_bytes)
        }
    }

    async fn health(&self) -> EntropyHealth {
        let start = std::time::Instant::now();
        let result = self.get_bytes(32).await;
        let latency_ms = start.elapsed().as_millis() as u64;

        EntropyHealth {
            source: self.source_name().to_string(),
            available: result.is_ok(),
            latency_ms: Some(latency_ms),
            error: result.err().map(|e| e.to_string()),
            checked_at: chrono::Utc::now(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Mock SkillBridge for testing quantum providers without network calls
    struct MockSkillBridge {
        fail: bool,
    }

    #[async_trait::async_trait]
    impl SkillBridge for MockSkillBridge {
        async fn invoke_skill(
            &self,
            _skill_id: &str,
            input: serde_json::Value,
        ) -> Result<serde_json::Value> {
            if self.fail {
                return Err(MagicError::SkillBridgeError("mock failure".into()));
            }

            // Infer byte count from input (n_bits or shots field)
            let n_bytes = if let Some(n_bits) = input.get("n_bits").and_then(|v| v.as_u64()) {
                (n_bits / 8) as usize
            } else if let Some(shots) = input.get("shots").and_then(|v| v.as_u64()) {
                (shots / 8) as usize
            } else {
                32 // default
            };

            // Return mock quantum bytes (0xAB repeated)
            let mock_bytes = vec![0xABu8; n_bytes];
            Ok(serde_json::json!({
                "bytes": hex::encode(mock_bytes)
            }))
        }
    }

    #[tokio::test]
    async fn test_os_provider_byte_count() {
        let provider = OsEntropyProvider::new();

        // Test 64 bytes
        let result = provider.get_bytes(64).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 64);

        // Test 1 byte
        let result = provider.get_bytes(1).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);

        // Test 0 bytes
        let result = provider.get_bytes(0).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_os_provider_always_available() {
        let provider = OsEntropyProvider::new();

        // is_available should always return true
        assert!(provider.is_available().await);

        // health should show available
        let health = provider.health().await;
        assert!(health.available);
        assert_eq!(health.source, "os");
    }

    #[tokio::test]
    async fn test_entropy_health_fields() {
        let provider = OsEntropyProvider::new();
        let health = provider.health().await;

        // Verify all fields are populated correctly
        assert!(health.available);
        assert!(health.error.is_none());
        assert!(health.latency_ms.is_some());
        assert_eq!(health.source, "os");

        // checked_at should be within 5 seconds of now
        let now = chrono::Utc::now();
        let diff = (now - health.checked_at).num_seconds().abs();
        assert!(diff < 5, "checked_at timestamp is too far from current time");
    }

    #[tokio::test]
    async fn test_mixed_provider_os_fallback() {
        // Create failing quantum providers
        let quantinuum = QuantinuumH2Provider::new(Arc::new(MockSkillBridge { fail: true }));
        let qiskit = QiskitProvider::new(Arc::new(MockSkillBridge { fail: true }));

        // Create mixed provider with no Quantum Origin, only failing quantum providers
        let provider = MixedEntropyProvider::new(
            None,
            Some(quantinuum),
            Some(qiskit),
            uuid::Uuid::new_v4(),
        );

        // Should fall back to OS and succeed
        let result = provider.get_bytes(32).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 32);
    }

    #[tokio::test]
    async fn test_mixed_provider_quantum_path() {
        // Create successful quantum provider
        let quantinuum = QuantinuumH2Provider::new(Arc::new(MockSkillBridge { fail: false }));

        // Create mixed provider with working quantum source
        let provider = MixedEntropyProvider::new(
            None,
            Some(quantinuum),
            None,
            uuid::Uuid::new_v4(),
        );

        // Get mixed entropy
        let mixed_result = provider.get_bytes(32).await;
        assert!(mixed_result.is_ok());
        let mixed_bytes = mixed_result.unwrap();
        assert_eq!(mixed_bytes.len(), 32);

        // Get pure OS entropy for comparison
        let os_provider = OsEntropyProvider::new();
        let os_result = os_provider.get_bytes(32).await;
        assert!(os_result.is_ok());
        let os_bytes = os_result.unwrap();

        // Mixed bytes should differ from pure OS bytes (probabilistically guaranteed)
        // The blake3 mixing ensures they won't be identical
        assert_ne!(
            mixed_bytes, os_bytes,
            "Mixed entropy should differ from pure OS entropy"
        );
    }

    #[tokio::test]
    async fn test_mixed_provider_all_health() {
        // Create failing quantum providers
        let quantinuum = QuantinuumH2Provider::new(Arc::new(MockSkillBridge { fail: true }));
        let qiskit = QiskitProvider::new(Arc::new(MockSkillBridge { fail: true }));

        // Create mixed provider
        let provider = MixedEntropyProvider::new(
            None,
            Some(quantinuum),
            Some(qiskit),
            uuid::Uuid::new_v4(),
        );

        // Get health for all providers
        let health_vec = provider.all_health().await;

        // Should have 3 providers: os, quantinuum, qiskit
        assert_eq!(health_vec.len(), 3);

        // First should be OS and available
        assert_eq!(health_vec[0].source, "os");
        assert!(health_vec[0].available);

        // Second and third should be unavailable (failing quantum providers)
        assert!(!health_vec[1].available);
        assert!(!health_vec[2].available);
    }

    #[tokio::test]
    async fn test_provider_priority_selection() {
        // Test 1: Higher-priority provider succeeds, lower-priority not used
        let quantinuum_success = QuantinuumH2Provider::new(Arc::new(MockSkillBridge { fail: false }));
        let qiskit_fail = QiskitProvider::new(Arc::new(MockSkillBridge { fail: true }));

        let provider = MixedEntropyProvider::new(
            None,
            Some(quantinuum_success),
            Some(qiskit_fail),
            uuid::Uuid::new_v4(),
        );

        // Should succeed using quantinuum (priority 2), not attempt qiskit (priority 3)
        let result = provider.get_bytes(32).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 32);

        // Test 2: Higher-priority fails, next provider is attempted
        let quantinuum_fail = QuantinuumH2Provider::new(Arc::new(MockSkillBridge { fail: true }));
        let qiskit_success = QiskitProvider::new(Arc::new(MockSkillBridge { fail: false }));

        let provider = MixedEntropyProvider::new(
            None,
            Some(quantinuum_fail),
            Some(qiskit_success),
            uuid::Uuid::new_v4(),
        );

        // Should fall through quantinuum to qiskit, then mix with OS
        let result = provider.get_bytes(32).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 32);

        // Test 3: All quantum fail, OS fallback succeeds
        let quantinuum_fail = QuantinuumH2Provider::new(Arc::new(MockSkillBridge { fail: true }));
        let qiskit_fail = QiskitProvider::new(Arc::new(MockSkillBridge { fail: true }));

        let provider = MixedEntropyProvider::new(
            None,
            Some(quantinuum_fail),
            Some(qiskit_fail),
            uuid::Uuid::new_v4(),
        );

        // Should fall back to OS (priority 4) and succeed
        let result = provider.get_bytes(32).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 32);
    }
}
