//! Fireworks Provider
//!
//! Direct integration with Fireworks AI API for serverless LLM inference.
//! Endpoint: https://api.fireworks.ai/inference/v1

use std::time::Duration;

use async_trait::async_trait;
use futures_util::stream::{BoxStream, StreamExt};
use reqwest::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap};
use serde::{Deserialize, Serialize};

use crate::model_router::{
    Choice, ChunkChoice, ChunkDelta, CompletionChunk, CompletionRequest, CompletionResponse,
    Message, UsageStats,
};
use crate::providers::Provider;
use carnelian_common::{Error, Result};

/// Fireworks AI provider for serverless LLM inference
pub struct FireworksProvider {
    name: String,
    api_key: String,
    base_url: String,
    client: Client,
}

impl FireworksProvider {
    /// Create a new Fireworks provider
    pub fn new(api_key: impl Into<String>) -> Self {
        let api_key = api_key.into();

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            format!("Bearer {}", api_key).parse().unwrap(),
        );
        headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());

        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(10))
            .default_headers(headers)
            .build()
            .expect("Failed to build Fireworks HTTP client");

        Self {
            name: "fireworks".to_string(),
            api_key,
            base_url: "https://api.fireworks.ai/inference/v1".to_string(),
            client,
        }
    }

    /// Convert messages to Fireworks format
    fn convert_messages(&self, messages: &[Message]) -> Vec<FireworksMessage> {
        messages
            .iter()
            .map(|m| FireworksMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct FireworksMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct FireworksRequest {
    model: String,
    messages: Vec<FireworksMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<i32>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct FireworksResponse {
    id: String,
    model: String,
    choices: Vec<FireworksChoice>,
    usage: FireworksUsage,
}

#[derive(Debug, Deserialize)]
struct FireworksChoice {
    index: i32,
    message: FireworksMessage,
    #[serde(rename = "finish_reason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(clippy::struct_field_names)]
struct FireworksUsage {
    #[serde(rename = "prompt_tokens")]
    prompt_tokens: i32,
    #[serde(rename = "completion_tokens")]
    completion_tokens: i32,
    #[serde(rename = "total_tokens")]
    total_tokens: i32,
}

#[derive(Debug, Deserialize)]
struct FireworksStreamResponse {
    id: String,
    model: String,
    choices: Vec<FireworksStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct FireworksStreamChoice {
    index: i32,
    delta: FireworksDelta,
    #[serde(rename = "finish_reason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FireworksDelta {
    role: Option<String>,
    content: Option<String>,
}

#[async_trait]
impl Provider for FireworksProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn provider_type(&self) -> &'static str {
        "remote"
    }

    async fn health_check(&self) -> Result<bool> {
        // Fireworks doesn't have a simple health endpoint
        // Try a simple API call to check connectivity
        let url = format!("{}/models", self.base_url);

        match self
            .client
            .get(&url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        // Fireworks supports many models - return common ones
        // In production, this could call the Fireworks models endpoint
        Ok(vec![
            "accounts/fireworks/models/llama-v3p1-405b-instruct".to_string(),
            "accounts/fireworks/models/llama-v3p1-70b-instruct".to_string(),
            "accounts/fireworks/models/llama-v3p1-8b-instruct".to_string(),
            "accounts/fireworks/models/llama-v3-70b-instruct".to_string(),
            "accounts/fireworks/models/mixtral-8x22b-instruct".to_string(),
            "accounts/fireworks/models/mixtral-8x7b-instruct".to_string(),
            "accounts/fireworks/models/qwen2p5-72b-instruct".to_string(),
            "accounts/fireworks/models/deepseek-r1".to_string(),
        ])
    }

    async fn has_model(&self, model: &str) -> Result<bool> {
        // Fireworks models start with "accounts/fireworks/models/"
        if model.starts_with("accounts/fireworks/") {
            return Ok(true);
        }

        let models = self.list_models().await?;
        Ok(models.iter().any(|m| m == model))
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let url = format!("{}/chat/completions", self.base_url);

        let fireworks_request = FireworksRequest {
            model: request.model,
            messages: self.convert_messages(&request.messages),
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            stream: false,
        };

        let resp = self
            .client
            .post(&url)
            .json(&fireworks_request)
            .send()
            .await
            .map_err(|e| Error::ModelRouting(format!("Fireworks request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(Error::ModelRouting(format!(
                "Fireworks returned {status}: {body_text}"
            )));
        }

        let data: FireworksResponse = resp
            .json()
            .await
            .map_err(|e| Error::ModelRouting(format!("Failed to parse Fireworks response: {e}")))?;

        Ok(CompletionResponse {
            id: data.id,
            model: data.model,
            choices: data
                .choices
                .into_iter()
                .map(|c| Choice {
                    index: c.index,
                    message: Message {
                        role: c.message.role,
                        content: c.message.content,
                        name: None,
                        tool_call_id: None,
                    },
                    finish_reason: c.finish_reason,
                })
                .collect(),
            usage: UsageStats {
                prompt_tokens: data.usage.prompt_tokens,
                completion_tokens: data.usage.completion_tokens,
                total_tokens: data.usage.total_tokens,
            },
            provider: self.name.clone(),
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<BoxStream<'static, Result<CompletionChunk>>> {
        let url = format!("{}/chat/completions", self.base_url);
        let _model = request.model.clone();

        let fireworks_request = FireworksRequest {
            model: request.model,
            messages: self.convert_messages(&request.messages),
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            stream: true,
        };

        let client = self.client.clone();
        let _provider_name = self.name.clone();

        let stream = async_stream::stream! {
            let resp = match client.post(&url).json(&fireworks_request).send().await {
                Ok(r) => r,
                Err(e) => {
                    yield Err(Error::ModelRouting(format!("Fireworks stream request failed: {e}")));
                    return;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let body_text = resp.text().await.unwrap_or_default();
                yield Err(Error::ModelRouting(format!(
                    "Fireworks returned {status}: {body_text}"
                )));
                return;
            }

            let mut stream = resp.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        // Process complete SSE events
                        while let Some(pos) = buffer.find("\n\n") {
                            let event = buffer[..pos].to_string();
                            buffer = buffer[pos + 2..].to_string();

                            // Parse SSE event
                            for line in event.lines() {
                                if let Some(data) = line.strip_prefix("data: ") {
                                    let data = data.trim();

                                    if data == "[DONE]" {
                                        return;
                                    }

                                    match serde_json::from_str::<FireworksStreamResponse>(data) {
                                        Ok(stream_resp) => {
                                            for choice in stream_resp.choices {
                                                let chunk = CompletionChunk {
                                                    id: stream_resp.id.clone(),
                                                    model: stream_resp.model.clone(),
                                                    choices: vec![ChunkChoice {
                                                        index: choice.index,
                                                        delta: ChunkDelta {
                                                            role: choice.delta.role.clone(),
                                                            content: choice.delta.content.clone(),
                                                        },
                                                        finish_reason: choice.finish_reason.clone(),
                                                    }],
                                                };
                                                yield Ok(chunk);
                                            }
                                        }
                                        Err(e) => {
                                            tracing::debug!(error = %e, data = %data, "Failed to parse Fireworks stream data");
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(Error::ModelRouting(format!("Fireworks stream error: {e}")));
                        return;
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}
