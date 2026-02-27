//! OpenAI Provider
//!
//! Direct integration with OpenAI API for GPT models.
//! Endpoint: https://api.openai.com/v1

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

/// OpenAI API provider
pub struct OpenAiProvider {
    name: String,
    api_key: String,
    base_url: String,
    client: Client,
}

impl OpenAiProvider {
    /// Create a new OpenAI provider
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
            .expect("Failed to build OpenAI HTTP client");

        Self {
            name: "openai".to_string(),
            api_key,
            base_url: "https://api.openai.com/v1".to_string(),
            client,
        }
    }

    /// Create with custom base URL (for Azure, proxies, etc.)
    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        let mut provider = Self::new(api_key);
        provider.base_url = base_url.into();
        provider
    }

    /// Convert messages to OpenAI format
    fn convert_messages(&self, messages: &[Message]) -> Vec<OpenAiMessage> {
        messages
            .iter()
            .map(|m| OpenAiMessage {
                role: m.role.clone(),
                content: m.content.clone(),
                name: m.name.clone(),
            })
            .collect()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<i32>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    id: String,
    model: String,
    choices: Vec<OpenAiChoice>,
    usage: OpenAiUsage,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    index: i32,
    message: OpenAiMessage,
    #[serde(rename = "finish_reason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(clippy::struct_field_names)]
struct OpenAiUsage {
    #[serde(rename = "prompt_tokens")]
    prompt_tokens: i32,
    #[serde(rename = "completion_tokens")]
    completion_tokens: i32,
    #[serde(rename = "total_tokens")]
    total_tokens: i32,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamResponse {
    id: String,
    model: String,
    choices: Vec<OpenAiStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChoice {
    index: i32,
    delta: OpenAiDelta,
    #[serde(rename = "finish_reason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiDelta {
    role: Option<String>,
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModel {
    id: String,
}

#[async_trait]
impl Provider for OpenAiProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn provider_type(&self) -> &'static str {
        "remote"
    }

    async fn health_check(&self) -> Result<bool> {
        // Try to list models as health check
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
        let url = format!("{}/models", self.base_url);

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::ModelRouting(format!("OpenAI list models failed: {e}")))?;

        if !resp.status().is_success() {
            return Err(Error::ModelRouting(format!(
                "OpenAI returned status {}",
                resp.status()
            )));
        }

        let data: OpenAiModelsResponse = resp
            .json()
            .await
            .map_err(|e| Error::ModelRouting(format!("Failed to parse OpenAI response: {e}")))?;

        let models: Vec<String> = data.data.into_iter().map(|m| m.id).collect();
        Ok(models)
    }

    async fn has_model(&self, model: &str) -> Result<bool> {
        let models = self.list_models().await?;
        Ok(models.iter().any(|m| m == model))
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let url = format!("{}/chat/completions", self.base_url);

        let openai_request = OpenAiRequest {
            model: request.model,
            messages: self.convert_messages(&request.messages),
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            stream: false,
        };

        let resp = self
            .client
            .post(&url)
            .json(&openai_request)
            .send()
            .await
            .map_err(|e| Error::ModelRouting(format!("OpenAI request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(Error::ModelRouting(format!(
                "OpenAI returned {status}: {body_text}"
            )));
        }

        let data: OpenAiResponse = resp
            .json()
            .await
            .map_err(|e| Error::ModelRouting(format!("Failed to parse OpenAI response: {e}")))?;

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
                        name: c.message.name,
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

        let openai_request = OpenAiRequest {
            model: request.model,
            messages: self.convert_messages(&request.messages),
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            stream: true,
        };

        let client = self.client.clone();
        let _provider_name = self.name.clone();

        let stream = async_stream::stream! {
            let resp = match client.post(&url).json(&openai_request).send().await {
                Ok(r) => r,
                Err(e) => {
                    yield Err(Error::ModelRouting(format!("OpenAI stream request failed: {e}")));
                    return;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let body_text = resp.text().await.unwrap_or_default();
                yield Err(Error::ModelRouting(format!(
                    "OpenAI returned {status}: {body_text}"
                )));
                return;
            }

            let mut stream = resp.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        // Process complete lines
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

                                    match serde_json::from_str::<OpenAiStreamResponse>(data) {
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
                                            tracing::debug!(error = %e, data = %data, "Failed to parse OpenAI stream data");
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(Error::ModelRouting(format!("OpenAI stream error: {e}")));
                        return;
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}
