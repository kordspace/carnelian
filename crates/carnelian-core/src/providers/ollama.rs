//! Ollama Provider
//!
//! Direct integration with Ollama API for local model inference.
//! Default endpoint: http://localhost:11434

use std::time::Duration;

use async_trait::async_trait;
use futures_util::stream::{BoxStream, StreamExt};
use reqwest::Client;
use serde_json::json;

use crate::model_router::{
    Choice, ChunkChoice, ChunkDelta, CompletionChunk, CompletionRequest, CompletionResponse,
    Message, UsageStats,
};
use crate::providers::Provider;
use carnelian_common::{Error, Result};

/// Ollama API provider for local model inference
pub struct OllamaProvider {
    name: String,
    base_url: String,
    client: Client,
}

impl OllamaProvider {
    /// Create a new Ollama provider
    pub fn new(base_url: impl Into<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to build Ollama HTTP client");

        Self {
            name: "ollama".to_string(),
            base_url: base_url.into(),
            client,
        }
    }

    /// Create with default localhost URL
    pub fn default_localhost() -> Self {
        Self::new("http://localhost:11434")
    }

    /// Convert generic messages to Ollama format
    fn convert_messages(&self, messages: &[Message]) -> (Option<String>, Vec<String>) {
        // Ollama uses a simple prompt format
        // System message becomes system prompt, rest are concatenated
        let mut system = None;
        let mut conversation = Vec::new();

        for msg in messages {
            match msg.role.as_str() {
                "system" => {
                    system = Some(msg.content.clone());
                }
                "user" => {
                    conversation.push(format!("User: {}", msg.content));
                }
                "assistant" => {
                    conversation.push(format!("Assistant: {}", msg.content));
                }
                _ => {
                    conversation.push(format!("{}: {}", msg.role, msg.content));
                }
            }
        }

        (system, conversation)
    }

    /// Build the full prompt string
    fn build_prompt(&self, system: Option<String>, conversation: Vec<String>) -> String {
        let mut prompt = String::new();

        if let Some(sys) = system {
            prompt.push_str(&sys);
            prompt.push_str("\n\n");
        }

        for line in conversation {
            prompt.push_str(&line);
            prompt.push('\n');
        }

        prompt.push_str("Assistant: ");
        prompt
    }
}

#[async_trait]
impl Provider for OllamaProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn provider_type(&self) -> &'static str {
        "local"
    }

    async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/api/tags", self.base_url);

        (self
            .client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await)
            .map_or_else(|_| Ok(false), |resp| Ok(resp.status().is_success()))
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/api/tags", self.base_url);

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::ModelRouting(format!("Ollama list models failed: {e}")))?;

        if !resp.status().is_success() {
            return Err(Error::ModelRouting(format!(
                "Ollama returned status {}",
                resp.status()
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| Error::ModelRouting(format!("Failed to parse Ollama response: {e}")))?;

        let models: Vec<String> = data["models"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["name"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(models)
    }

    async fn has_model(&self, model: &str) -> Result<bool> {
        let models = self.list_models().await?;
        Ok(models
            .iter()
            .any(|m| m == model || m.starts_with(&model.to_string())))
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let (system, conversation) = self.convert_messages(&request.messages);
        let prompt = self.build_prompt(system, conversation);

        let url = format!("{}/api/generate", self.base_url);

        let body = json!({
            "model": request.model,
            "prompt": prompt,
            "stream": false,
            "options": {
                "temperature": request.temperature.unwrap_or(0.7) as f64,
                "num_predict": request.max_tokens.unwrap_or(2048),
            }
        });

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::ModelRouting(format!("Ollama request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(Error::ModelRouting(format!(
                "Ollama returned {status}: {body_text}"
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| Error::ModelRouting(format!("Failed to parse Ollama response: {e}")))?;

        let content = data["response"]
            .as_str()
            .map(String::from)
            .unwrap_or_default();

        // Estimate token counts (Ollama doesn't return them directly)
        let prompt_tokens = i32::try_from(prompt.len() / 4).unwrap_or(i32::MAX);
        let completion_tokens = i32::try_from(content.len() / 4).unwrap_or(i32::MAX);

        Ok(CompletionResponse {
            id: format!("ollama-{}-{}", request.model, uuid::Uuid::now_v7()),
            model: request.model.clone(),
            choices: vec![Choice {
                index: 0,
                message: Message {
                    role: "assistant".to_string(),
                    content,
                    name: None,
                    tool_call_id: None,
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: UsageStats {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
            },
            provider: self.name.clone(),
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<BoxStream<'static, Result<CompletionChunk>>> {
        let (system, conversation) = self.convert_messages(&request.messages);
        let prompt = self.build_prompt(system, conversation);
        let _prompt_chars = prompt.len();

        let url = format!("{}/api/generate", self.base_url);
        let model = request.model.clone();

        let body = json!({
            "model": request.model,
            "prompt": prompt,
            "stream": true,
            "options": {
                "temperature": request.temperature.unwrap_or(0.7) as f64,
                "num_predict": request.max_tokens.unwrap_or(2048),
            }
        });

        let client = self.client.clone();
        let _provider_name = self.name.clone();

        let stream = async_stream::stream! {
            let resp = match client.post(&url).json(&body).send().await {
                Ok(r) => r,
                Err(e) => {
                    yield Err(Error::ModelRouting(format!("Ollama stream request failed: {e}")));
                    return;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let body_text = resp.text().await.unwrap_or_default();
                yield Err(Error::ModelRouting(format!(
                    "Ollama returned {status}: {body_text}"
                )));
                return;
            }

            let mut stream = resp.bytes_stream();
            let chunk_id = format!("ollama-stream-{}-{}", model, uuid::Uuid::now_v7());
            let mut chunk_index = 0;

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);

                        for line in text.lines() {
                            if line.is_empty() {
                                continue;
                            }

                            // Each line is a JSON object
                            match serde_json::from_str::<serde_json::Value>(line) {
                                Ok(data) => {
                                    if let Some(response_text) = data["response"].as_str() {

                                        let chunk = CompletionChunk {
                                            id: chunk_id.clone(),
                                            model: model.clone(),
                                            choices: vec![ChunkChoice {
                                                index: 0,
                                                delta: ChunkDelta {
                                                    role: if chunk_index == 0 { Some("assistant".to_string()) } else { None },
                                                    content: Some(response_text.to_string()),
                                                },
                                                finish_reason: None,
                                            }],
                                        };

                                        yield Ok(chunk);
                                        chunk_index += 1;
                                    }

                                    // Check if done
                                    if data["done"].as_bool() == Some(true) {
                                        let final_chunk = CompletionChunk {
                                            id: chunk_id.clone(),
                                            model: model.clone(),
                                            choices: vec![ChunkChoice {
                                                index: 0,
                                                delta: ChunkDelta {
                                                    role: None,
                                                    content: None,
                                                },
                                                finish_reason: Some("stop".to_string()),
                                            }],
                                        };
                                        yield Ok(final_chunk);
                                        return;
                                    }
                                }
                                Err(e) => {
                                    use tracing::warn;
                                    warn!(error = %e, line = %line, "Failed to parse Ollama stream line");
                                }
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(Error::ModelRouting(format!("Ollama stream error: {e}")));
                        return;
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}
