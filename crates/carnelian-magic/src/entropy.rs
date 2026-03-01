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
    fn source_name(&self) -> &str {
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
    fn source_name(&self) -> &str {
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
    fn source_name(&self) -> &str {
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
    fn source_name(&self) -> &str {
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

    /// Get health status from all configured providers
    pub async fn all_health(&self) -> Vec<EntropyHealth> {
        let mut health = vec![self.os.health().await];

        if let Some(ref qo) = self.quantum_origin {
            health.push(qo.health().await);
        }
        if let Some(ref qh) = self.quantinuum {
            health.push(qh.health().await);
        }
        if let Some(ref qk) = self.qiskit {
            health.push(qk.health().await);
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
    fn source_name(&self) -> &str {
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
