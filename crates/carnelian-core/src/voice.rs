//! Voice Gateway for 🔥 Carnelian OS
//!
//! Integrates with ElevenLabs for speech-to-text and text-to-speech capabilities.
//! Uses a semaphore-based rate limiter to cap concurrent requests.

use std::sync::Arc;

use carnelian_common::{Error, Result};
use carnelian_common::types::VoiceInfo;
use sqlx::PgPool;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::encryption::EncryptionHelper;

// =============================================================================
// CONSTANTS
// =============================================================================

const ELEVENLABS_BASE_URL: &str = "https://api.elevenlabs.io/v1";
const CONFIG_KEY_API_KEY: &str = "voice.elevenlabs_api_key";
const MAX_CONCURRENT_REQUESTS: usize = 5;

// =============================================================================
// VOICE GATEWAY
// =============================================================================

/// Gateway for ElevenLabs voice services (STT, TTS, voice listing).
///
/// Wraps a `reqwest::Client` with a semaphore-based rate limiter to cap
/// concurrent outbound requests at [`MAX_CONCURRENT_REQUESTS`].
pub struct VoiceGateway {
    http_client: reqwest::Client,
    rate_limiter: Arc<Semaphore>,
    pool: PgPool,
}

impl VoiceGateway {
    /// Create a new `VoiceGateway` backed by the given connection pool.
    pub fn new(pool: PgPool) -> Self {
        let http_client = reqwest::ClientBuilder::new()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build reqwest client");

        Self {
            http_client,
            rate_limiter: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
            pool,
        }
    }

    // =========================================================================
    // API KEY MANAGEMENT
    // =========================================================================

    /// Persist the ElevenLabs API key in `config_store`.
    ///
    /// The key is always encrypted via the provided `EncryptionHelper`.
    /// Returns an error if no helper is available (owner signing key not loaded).
    pub async fn save_api_key(
        &self,
        api_key: &str,
        encryption: Option<&EncryptionHelper>,
    ) -> Result<()> {
        let enc = encryption.ok_or_else(|| {
            Error::Config(
                "Cannot store voice API key: owner signing key not loaded (encryption required)"
                    .to_string(),
            )
        })?;

        let ciphertext = enc.encrypt_text(api_key).await?;
        sqlx::query(
            r"INSERT INTO config_store (key, value, value_blob, encrypted, updated_at)
              VALUES ($1, '{}'::jsonb, $2, true, NOW())
              ON CONFLICT (key) DO UPDATE
                SET value = '{}'::jsonb, value_blob = $2, encrypted = true, updated_at = NOW()",
        )
        .bind(CONFIG_KEY_API_KEY)
        .bind(&ciphertext)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Config(format!("Failed to store voice API key: {}", e)))?;

        Ok(())
    }

    /// Load the ElevenLabs API key from `config_store`.
    ///
    /// Returns `None` if no key has been configured yet.
    pub async fn load_api_key(
        &self,
        encryption: Option<&EncryptionHelper>,
    ) -> Result<Option<String>> {
        let row: Option<(Option<Vec<u8>>, Option<String>, bool)> = sqlx::query_as(
            "SELECT value_blob, value_text, encrypted FROM config_store WHERE key = $1",
        )
        .bind(CONFIG_KEY_API_KEY)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Config(format!("Failed to load voice API key: {}", e)))?;

        match row {
            None => Ok(None),
            Some((blob, text, encrypted)) => {
                if encrypted {
                    let enc = encryption.ok_or_else(|| {
                        Error::Config(
                            "Voice API key is encrypted but no EncryptionHelper provided"
                                .to_string(),
                        )
                    })?;
                    let blob = blob.ok_or_else(|| {
                        Error::Config("Encrypted voice API key has no blob data".to_string())
                    })?;
                    let plaintext = enc.decrypt_text(&blob).await?;
                    Ok(Some(plaintext))
                } else {
                    Ok(text)
                }
            }
        }
    }

    // =========================================================================
    // ELEVENLABS API METHODS
    // =========================================================================

    /// Transcribe audio bytes to text via ElevenLabs speech-to-text.
    pub async fn speech_to_text(
        &self,
        audio: bytes::Bytes,
        encryption: Option<&EncryptionHelper>,
    ) -> Result<String> {
        let _permit = self
            .rate_limiter
            .acquire()
            .await
            .map_err(|e| Error::Config(format!("Rate limiter closed: {}", e)))?;

        let api_key = self
            .load_api_key(encryption)
            .await?
            .ok_or_else(|| Error::Config("ElevenLabs API key not configured".to_string()))?;

        let part = reqwest::multipart::Part::bytes(audio.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| Error::Config(format!("Failed to build multipart: {}", e)))?;

        let form = reqwest::multipart::Form::new().part("audio", part);

        let response = self
            .http_client
            .post(format!("{}/speech-to-text", ELEVENLABS_BASE_URL))
            .header("xi-api-key", &api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| Error::Config(format!("ElevenLabs STT request failed: {}", e)))?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(Error::RateLimitExceeded(
                "ElevenLabs rate limit exceeded".to_string(),
            ));
        }
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(Error::Security(
                "ElevenLabs API key is invalid or expired".to_string(),
            ));
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Config(format!(
                "ElevenLabs STT error ({}): {}",
                status, body
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| Error::Config(format!("Failed to parse STT response: {}", e)))?;

        json["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| Error::Config("STT response missing 'text' field".to_string()))
    }

    /// Synthesise speech from text via ElevenLabs text-to-speech.
    ///
    /// Returns raw audio bytes (audio/mpeg).
    pub async fn text_to_speech(
        &self,
        text: &str,
        voice_id: &str,
        encryption: Option<&EncryptionHelper>,
    ) -> Result<bytes::Bytes> {
        let _permit = self
            .rate_limiter
            .acquire()
            .await
            .map_err(|e| Error::Config(format!("Rate limiter closed: {}", e)))?;

        let api_key = self
            .load_api_key(encryption)
            .await?
            .ok_or_else(|| Error::Config("ElevenLabs API key not configured".to_string()))?;

        let response = self
            .http_client
            .post(format!(
                "{}/text-to-speech/{}",
                ELEVENLABS_BASE_URL, voice_id
            ))
            .header("xi-api-key", &api_key)
            .json(&serde_json::json!({
                "text": text,
                "model_id": "eleven_monolingual_v1"
            }))
            .send()
            .await
            .map_err(|e| Error::Config(format!("ElevenLabs TTS request failed: {}", e)))?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(Error::RateLimitExceeded(
                "ElevenLabs rate limit exceeded".to_string(),
            ));
        }
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(Error::Security(
                "ElevenLabs API key is invalid or expired".to_string(),
            ));
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Config(format!(
                "ElevenLabs TTS error ({}): {}",
                status, body
            )));
        }

        response
            .bytes()
            .await
            .map_err(|e| Error::Config(format!("Failed to read TTS audio bytes: {}", e)))
    }

    /// List available voices from ElevenLabs.
    pub async fn list_voices(
        &self,
        encryption: Option<&EncryptionHelper>,
    ) -> Result<Vec<VoiceInfo>> {
        let _permit = self
            .rate_limiter
            .acquire()
            .await
            .map_err(|e| Error::Config(format!("Rate limiter closed: {}", e)))?;

        let api_key = self
            .load_api_key(encryption)
            .await?
            .ok_or_else(|| Error::Config("ElevenLabs API key not configured".to_string()))?;

        let response = self
            .http_client
            .get(format!("{}/voices", ELEVENLABS_BASE_URL))
            .header("xi-api-key", &api_key)
            .send()
            .await
            .map_err(|e| Error::Config(format!("ElevenLabs voices request failed: {}", e)))?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(Error::RateLimitExceeded(
                "ElevenLabs rate limit exceeded".to_string(),
            ));
        }
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(Error::Security(
                "ElevenLabs API key is invalid or expired".to_string(),
            ));
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Config(format!(
                "ElevenLabs voices error ({}): {}",
                status, body
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| Error::Config(format!("Failed to parse voices response: {}", e)))?;

        let voices_array = json["voices"]
            .as_array()
            .ok_or_else(|| Error::Config("Voices response missing 'voices' array".to_string()))?;

        let mut voices = Vec::with_capacity(voices_array.len());
        for v in voices_array {
            voices.push(VoiceInfo {
                voice_id: v["voice_id"].as_str().unwrap_or_default().to_string(),
                name: v["name"].as_str().unwrap_or_default().to_string(),
                description: v["description"].as_str().map(|s| s.to_string()),
                preview_url: v["preview_url"].as_str().map(|s| s.to_string()),
                labels: v["labels"]
                    .as_object()
                    .map(|obj| {
                        obj.iter()
                            .filter_map(|(k, val)| val.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    })
                    .unwrap_or_default(),
            });
        }

        Ok(voices)
    }

    // =========================================================================
    // IDENTITY VOICE CONFIG
    // =========================================================================

    /// Update the `voice_config` JSONB column on an identity row.
    pub async fn update_identity_voice_config(
        &self,
        identity_id: Uuid,
        config: serde_json::Value,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE identities SET voice_config = $2, updated_at = NOW() WHERE identity_id = $1",
        )
        .bind(identity_id)
        .bind(&config)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Config(format!("Failed to update voice config: {}", e)))?;

        Ok(())
    }
}
