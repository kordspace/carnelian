//! Anthropic Provider
//!
//! Direct integration with Anthropic API for Claude models.
//! Endpoint: https://api.anthropic.com/v1

use std::time::Duration;

use async_trait::async_trait;
use futures_util::stream::{BoxStream, StreamExt};
use reqwest::Client;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};

use crate::model_router::{
    Choice, ChunkChoice, ChunkDelta, CompletionChunk, CompletionRequest, CompletionResponse,
    Message, UsageStats,
};
use crate::providers::Provider;
use carnelian_common::{Error, Result};

/// Anthropic API provider for Claude models
pub struct AnthropicProvider {
    name: String,
    api_key: String,
    base_url: String,
    client: Client,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider
    pub fn new(api_key: impl Into<String>) -> Self {
        let api_key = api_key.into();

        let mut headers = HeaderMap::new();
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(&api_key).expect("Invalid API key format"),
        );
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(10))
            .default_headers(headers)
            .build()
            .expect("Failed to build Anthropic HTTP client");

        Self {
            name: "anthropic".to_string(),
            api_key,
            base_url: "https://api.anthropic.com/v1".to_string(),
            client,
        }
    }

    /// Convert messages to Anthropic format
    fn convert_messages(&self, messages: &[Message]) -> (Option<String>, Vec<AnthropicMessage>) {
        let mut system = None;
        let mut anthropic_messages = Vec::new();

        for msg in messages {
            match msg.role.as_str() {
                "system" => {
                    system = Some(msg.content.clone());
                }
                _ => {
                    anthropic_messages.push(AnthropicMessage {
                        role: if msg.role == "assistant" {
                            "assistant"
                        } else {
                            "user"
                        }
                        .to_string(),
                        content: msg.content.clone(),
                    });
                }
            }
        }

        (system, anthropic_messages)
    }
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    id: String,
    model: String,
    #[serde(rename = "type")]
    response_type: String,
    content: Vec<AnthropicContent>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    #[serde(rename = "input_tokens")]
    input_tokens: i32,
    #[serde(rename = "output_tokens")]
    output_tokens: i32,
}

#[derive(Debug, Deserialize)]
struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delta: Option<AnthropicDelta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_block: Option<AnthropicContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
struct AnthropicDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicModelsResponse {
    data: Vec<AnthropicModel>,
}

#[derive(Debug, Deserialize)]
struct AnthropicModel {
    id: String,
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn provider_type(&self) -> &'static str {
        "remote"
    }

    async fn health_check(&self) -> Result<bool> {
        // Anthropic doesn't have a simple health endpoint, so check if we can list models
        let url = format!("{}/models", self.base_url);

        (self
            .client
            .get(&url)
            .timeout(Duration::from_secs(10))
            .send()
            .await)
            .map_or_else(|_| Ok(false), |resp| Ok(resp.status().is_success()))
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/models", self.base_url);

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::ModelRouting(format!("Anthropic list models failed: {e}")))?;

        if !resp.status().is_success() {
            // Anthropic may not support model listing in all API versions
            // Return common Claude models as fallback
            return Ok(vec![
                "claude-3-5-sonnet-20241022".to_string(),
                "claude-3-5-haiku-20241022".to_string(),
                "claude-3-opus-20240229".to_string(),
                "claude-3-sonnet-20240229".to_string(),
                "claude-3-haiku-20240307".to_string(),
            ]);
        }

        let data: AnthropicModelsResponse = resp
            .json()
            .await
            .map_err(|e| Error::ModelRouting(format!("Failed to parse Anthropic response: {e}")))?;

        let models: Vec<String> = data.data.into_iter().map(|m| m.id).collect();
        Ok(models)
    }

    async fn has_model(&self, model: &str) -> Result<bool> {
        // Anthropic models follow pattern "claude-*"
        if model.to_lowercase().starts_with("claude") {
            return Ok(true);
        }

        let models = self.list_models().await?;
        Ok(models.iter().any(|m| m == model))
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let url = format!("{}/messages", self.base_url);

        let (system, messages) = self.convert_messages(&request.messages);

        let anthropic_request = AnthropicRequest {
            model: request.model,
            messages,
            system,
            max_tokens: request.max_tokens.or(Some(4096)),
            temperature: request.temperature,
            stream: false,
        };

        let resp = self
            .client
            .post(&url)
            .json(&anthropic_request)
            .send()
            .await
            .map_err(|e| Error::ModelRouting(format!("Anthropic request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(Error::ModelRouting(format!(
                "Anthropic returned {status}: {body_text}"
            )));
        }

        let data: AnthropicResponse = resp
            .json()
            .await
            .map_err(|e| Error::ModelRouting(format!("Failed to parse Anthropic response: {e}")))?;

        // Combine all content blocks into single content
        let content: String = data
            .content
            .iter()
            .filter(|c| c.content_type == "text")
            .map(|c| c.text.clone())
            .collect();

        Ok(CompletionResponse {
            id: data.id,
            model: data.model,
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
                prompt_tokens: data.usage.input_tokens,
                completion_tokens: data.usage.output_tokens,
                total_tokens: data.usage.input_tokens + data.usage.output_tokens,
            },
            provider: self.name.clone(),
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<BoxStream<'static, Result<CompletionChunk>>> {
        let url = format!("{}/messages", self.base_url);
        let model = request.model.clone();

        let (system, messages) = self.convert_messages(&request.messages);

        let anthropic_request = AnthropicRequest {
            model: request.model,
            messages,
            system,
            max_tokens: request.max_tokens.or(Some(4096)),
            temperature: request.temperature,
            stream: true,
        };

        let client = self.client.clone();
        let _provider_name = self.name.clone();

        let stream = async_stream::stream! {
            let resp = match client.post(&url).json(&anthropic_request).send().await {
                Ok(r) => r,
                Err(e) => {
                    yield Err(Error::ModelRouting(format!("Anthropic stream request failed: {e}")));
                    return;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let body_text = resp.text().await.unwrap_or_default();
                yield Err(Error::ModelRouting(format!(
                    "Anthropic returned {status}: {body_text}"
                )));
                return;
            }

            let mut stream = resp.bytes_stream();
            let mut buffer = String::new();
            let stream_id = format!("anthropic-stream-{}-{}", model, uuid::Uuid::now_v7());

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        // Process complete SSE events
                        while let Some(pos) = buffer.find("\n\n") {
                            let event_text = buffer[..pos].to_string();
                            buffer = buffer[pos + 2..].to_string();

                            // Parse SSE event
                            let mut event_data = String::new();

                            for line in event_text.lines() {
                                if line.strip_prefix("event: ").is_some() {
                                    // Event type ignored - we process based on data content
                                } else if let Some(data) = line.strip_prefix("data: ") {
                                    event_data = data.to_string();
                                }
                            }

                            if event_data.is_empty() {
                                continue;
                            }

                            match serde_json::from_str::<AnthropicStreamEvent>(&event_data) {
                                Ok(event) => {
                                    match event.event_type.as_str() {
                                        "content_block_delta" => {
                                            if let Some(delta) = event.delta {
                                                if let Some(text) = delta.text {
                                                    let chunk = CompletionChunk {
                                                        id: stream_id.clone(),
                                                        model: model.clone(),
                                                        choices: vec![ChunkChoice {
                                                            index: event.index.unwrap_or(0),
                                                            delta: ChunkDelta {
                                                                role: None,
                                                                content: Some(text),
                                                            },
                                                            finish_reason: None,
                                                        }],
                                                    };
                                                    yield Ok(chunk);
                                                }
                                            }
                                        }
                                        "message_stop" => {
                                            // Stream complete
                                            let final_chunk = CompletionChunk {
                                                id: stream_id.clone(),
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
                                        _ => {}
                                    }
                                }
                                Err(e) => {
                                    use tracing::warn;
                                    warn!(error = %e, data = %event_data, "Failed to parse Anthropic stream event");
                                }
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(Error::ModelRouting(format!("Anthropic stream error: {e}")));
                        return;
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}
