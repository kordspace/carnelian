//! LLM Provider Adapters
//!
//! Native Rust implementations for LLM provider APIs.
//! Replaces the TypeScript gateway with direct provider communication.

use crate::model_router::{CompletionChunk, CompletionRequest, CompletionResponse};
use carnelian_common::Result;
use futures_util::stream::BoxStream;

pub mod anthropic;
pub mod fireworks;
pub mod ollama;
pub mod openai;

pub use anthropic::AnthropicProvider;
pub use fireworks::FireworksProvider;
pub use ollama::OllamaProvider;
pub use openai::OpenAiProvider;

/// Trait for LLM provider implementations
#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    /// Provider name (e.g., "ollama", "openai")
    fn name(&self) -> &str;

    /// Provider type ("local" or "remote")
    fn provider_type(&self) -> &'static str;

    /// Check if provider is available
    async fn health_check(&self) -> Result<bool>;

    /// List available models
    async fn list_models(&self) -> Result<Vec<String>>;

    /// Check if a specific model is available
    async fn has_model(&self, model: &str) -> Result<bool>;

    /// Non-streaming completion
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse>;

    /// Streaming completion
    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<BoxStream<'static, Result<CompletionChunk>>>;
}

/// Provider registry for managing multiple providers
pub struct ProviderRegistry {
    providers: Vec<Box<dyn Provider>>,
}

impl ProviderRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Add a provider to the registry
    pub fn add_provider(&mut self, provider: Box<dyn Provider>) {
        self.providers.push(provider);
    }

    /// Get all providers
    pub fn providers(&self) -> &[Box<dyn Provider>] {
        &self.providers
    }

    /// Get a provider by name
    pub fn get_provider(&self, name: &str) -> Option<&dyn Provider> {
        self.providers
            .iter()
            .find(|p| p.name() == name)
            .map(|p| p.as_ref())
    }

    /// Get all local providers
    pub fn local_providers(&self) -> impl Iterator<Item = &dyn Provider> {
        self.providers
            .iter()
            .filter(|p| p.provider_type() == "local")
            .map(|p| p.as_ref())
    }

    /// Get all remote providers
    pub fn remote_providers(&self) -> impl Iterator<Item = &dyn Provider> {
        self.providers
            .iter()
            .filter(|p| p.provider_type() == "remote")
            .map(|p| p.as_ref())
    }

    /// Run health checks on all providers
    pub async fn health_check_all(&self) -> Vec<(String, bool)> {
        let mut results = Vec::new();
        for provider in &self.providers {
            let provider_name = provider.name().to_string();
            let healthy = provider.health_check().await.unwrap_or(false);
            results.push((provider_name, healthy));
        }
        results
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
