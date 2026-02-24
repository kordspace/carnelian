# Gateway-to-Rust Migration Implementation Plan

## Overview
This document details the step-by-step implementation for migrating the TypeScript gateway service into native Rust within the `model_router.rs` module.

## Current Architecture
```
model_router.rs → HTTP → gateway (TypeScript) → HTTP → Providers (Ollama/OpenAI/Anthropic/Fireworks)
```

## Target Architecture
```
model_router.rs → HTTP → Providers (Ollama/OpenAI/Anthropic/Fireworks)
```

## Implementation Phases

### Phase 1: Create Provider Adapters (1-2 days)

#### 1.1 Provider Trait Definition
Create a new file: `crates/carnelian-core/src/providers/mod.rs`

```rust
/// Trait for LLM provider implementations
#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    /// Provider name (e.g., "ollama", "openai")
    fn name(&self) -> &str;
    
    /// Provider type ("local" or "remote")
    fn provider_type(&self) -> &str;
    
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
```

#### 1.2 Ollama Provider
Create: `crates/carnelian-core/src/providers/ollama.rs`
- Direct HTTP calls to Ollama API (localhost:11434)
- Support for `/api/generate` and `/api/tags`
- Handle both streaming and non-streaming

#### 1.3 OpenAI Provider  
Create: `crates/carnelian-core/src/providers/openai.rs`
- Direct HTTP calls to OpenAI API
- Handle authentication via API key from config
- Support chat completions endpoint

#### 1.4 Anthropic Provider
Create: `crates/carnelian-core/src/providers/anthropic.rs`
- Direct HTTP calls to Anthropic API
- Handle authentication
- Support messages endpoint

#### 1.5 Fireworks Provider
Create: `crates/carnelian-core/src/providers/fireworks.rs`
- Direct HTTP calls to Fireworks API
- Handle authentication

### Phase 2: Refactor ModelRouter (1 day)

#### 2.1 Update ModelRouter to Use Native Providers
Modify `model_router.rs`:
- Replace `gateway_url` with provider registry
- Add provider instances directly
- Remove HTTP calls to gateway
- Route directly to provider implementations

#### 2.2 Configuration Integration
- Load provider configs from `model_providers` table
- Initialize provider instances at startup
- Support dynamic provider enable/disable

### Phase 3: API Endpoint Migration (1 day)

#### 3.1 Add Gateway Endpoints to Server
Add to `server.rs`:
- `POST /v1/complete` → model_router.complete()
- `POST /v1/complete/stream` → model_router.complete_stream()
- `GET /health` → provider health checks

#### 3.2 Update AppState
- Add provider registry to AppState
- Initialize providers during server startup

### Phase 4: Cleanup (0.5 day)

#### 4.1 Remove TypeScript Gateway
Delete:
- `gateway/` directory
- `gateway/package.json`
- `gateway/src/` (all TypeScript files)
- `gateway/README.md`

#### 4.2 Update docker-compose.yml
Remove gateway service definition

#### 4.3 Update Documentation
- Update README.md
- Update API.md
- Update architecture diagrams

### Phase 5: Testing & Validation (1 day)

#### 5.1 Unit Tests
- Provider adapter tests
- Routing logic tests
- Error handling tests

#### 5.2 Integration Tests
- End-to-end completion tests
- Streaming tests
- Health check tests

#### 5.3 Manual Validation
- Test each provider (Ollama, OpenAI, etc.)
- Verify capability checks still work
- Verify budget enforcement still works
- Verify audit logging still works

## File Changes

### New Files
```
crates/carnelian-core/src/
├── providers/
│   ├── mod.rs          # Provider trait and registry
│   ├── ollama.rs       # Ollama provider
│   ├── openai.rs       # OpenAI provider
│   ├── anthropic.rs    # Anthropic provider
│   └── fireworks.rs    # Fireworks provider
```

### Modified Files
```
crates/carnelian-core/src/
├── model_router.rs     # Refactor to use native providers
├── server.rs           # Add gateway endpoints
└── lib.rs              # Add providers module

docker-compose.yml      # Remove gateway service
```

### Deleted Files
```
gateway/                # Entire directory
```

## Risk Mitigation

1. **Rollback Plan**: Keep gateway code in git history for easy rollback
2. **Feature Parity**: Ensure all gateway features are migrated:
   - Request validation
   - Token limit enforcement
   - Error handling
   - Usage tracking
   - Streaming support
3. **Testing**: Comprehensive test coverage before merge

## Success Criteria

- [ ] All TypeScript gateway code removed
- [ ] Direct provider communication working
- [ ] All existing tests passing
- [ ] New integration tests added
- [ ] Documentation updated
- [ ] No performance regression
- [ ] Feature parity verified

## Timeline

Total estimated time: **4-5 days**

- Phase 1: 1-2 days
- Phase 2: 1 day
- Phase 3: 1 day
- Phase 4: 0.5 day
- Phase 5: 1 day
