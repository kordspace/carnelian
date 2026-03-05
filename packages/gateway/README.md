# Carnelian LLM Gateway

Standalone TypeScript gateway service providing a unified completion API with local-first routing and remote fallback. Runs on port **18790** by default.

## Architecture

```
Client → Gateway (port 18790) → Router → Provider Adapter → LLM Backend
                                   │
                                   └→ Usage Tracker → Rust Core (port 8080)
```

The gateway sits between clients and LLM providers, offering:

- **Unified API** — Single endpoint for all providers (Ollama, OpenAI, Anthropic, Fireworks)
- **Local-first routing** — Prefers Ollama when the model is available locally
- **Automatic fallback** — Routes to remote providers when local is unavailable
- **Usage tracking** — Reports token usage and cost estimates to the Rust core
- **Circuit breaker** — Prevents repeated calls to failing providers

## Quick Start

```bash
cd gateway
npm install
npm run build
npm start
```

## API Endpoints

### POST /v1/complete

Non-streaming completion.

```bash
curl -X POST http://localhost:18790/v1/complete \
  -H "Content-Type: application/json" \
  -d '{
    "model": "deepseek-r1:7b",
    "messages": [
      {"role": "user", "content": "Hello, world!"}
    ]
  }'
```

**Response:**

```json
{
  "id": "chatcmpl_abc123",
  "object": "chat.completion",
  "created": 1700000000,
  "model": "deepseek-r1:7b",
  "choices": [{
    "index": 0,
    "message": {"role": "assistant", "content": "Hello! How can I help you?"},
    "finish_reason": "stop"
  }],
  "usage": {
    "prompt_tokens": 12,
    "completion_tokens": 8,
    "total_tokens": 20
  },
  "provider": "ollama"
}
```

### POST /v1/complete/stream

Streaming completion via Server-Sent Events.

```bash
curl -X POST http://localhost:18790/v1/complete/stream \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "Explain quantum computing briefly."}
    ],
    "temperature": 0.7,
    "max_tokens": 500
  }'
```

**Response (SSE):**

```
data: {"id":"chatcmpl_abc123","object":"chat.completion.chunk","created":1700000000,"model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

data: {"id":"chatcmpl_abc123","object":"chat.completion.chunk","created":1700000000,"model":"gpt-4o","choices":[{"index":0,"delta":{"content":"Quantum"},"finish_reason":null}]}

data: [DONE]
```

### GET /health

Provider health check.

```bash
curl http://localhost:18790/health
```

**Response:**

```json
{
  "status": "ok",
  "version": "0.1.0",
  "uptime_s": 3600,
  "providers": [
    {"name": "ollama", "type": "local", "available": true, "models": ["deepseek-r1:7b", "llama3:8b"]},
    {"name": "openai", "type": "remote", "available": true}
  ]
}
```

## Request Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `model` | string | Yes | Model identifier (e.g. `deepseek-r1:7b`, `gpt-4o`, `claude-3-5-sonnet-20241022`) |
| `messages` | array | Yes | Conversation messages with `role` and `content` |
| `temperature` | number | No | Sampling temperature (0–2) |
| `max_tokens` | number | No | Maximum tokens to generate |
| `stream` | boolean | No | Ignored — use the `/stream` endpoint instead |
| `top_p` | number | No | Nucleus sampling (0–1) |
| `frequency_penalty` | number | No | Frequency penalty (-2 to 2) |
| `presence_penalty` | number | No | Presence penalty (-2 to 2) |
| `stop` | string/array | No | Stop sequences |
| `user` | string | No | Opaque user identifier |
| `correlation_id` | string | No | Correlation ID from Rust core |

## Model Routing

The router selects providers based on model name patterns:

| Pattern | Provider |
|---------|----------|
| `claude*` | Anthropic |
| `gpt-*`, `o1*`, `o3*` | OpenAI |
| `accounts/fireworks/*` | Fireworks |
| Everything else | Ollama (local-first) |

If the local provider doesn't have the model, the router falls back to remote providers.

## Configuration

Configuration is loaded from three sources (highest priority first):

1. **Environment variables**
2. **Config file** (`gateway.config.json`)
3. **Built-in defaults**

### Environment Variables

```bash
# Server
GATEWAY_PORT=18790
CORE_API_URL=http://localhost:8080

# Ollama (local)
OLLAMA_BASE_URL=http://localhost:11434
OLLAMA_ENABLED=true

# OpenAI (remote)
OPENAI_API_KEY=sk-...
OPENAI_BASE_URL=https://api.openai.com
OPENAI_ENABLED=true

# Anthropic (remote)
ANTHROPIC_API_KEY=sk-ant-...
ANTHROPIC_BASE_URL=https://api.anthropic.com
ANTHROPIC_ENABLED=true

# Fireworks (remote)
FIREWORKS_API_KEY=...
FIREWORKS_BASE_URL=https://api.fireworks.ai/inference
FIREWORKS_ENABLED=true

# Routing
LOCAL_FIRST=true
FALLBACK_ENABLED=true

# Limits
MAX_TOKENS=8192
REQUEST_TIMEOUT_MS=60000
```

### Config File

Create `gateway.config.json` in the gateway directory:

```json
{
  "port": 18790,
  "coreApiUrl": "http://localhost:8080",
  "providers": {
    "ollama": {
      "enabled": true,
      "baseUrl": "http://localhost:11434"
    },
    "openai": {
      "enabled": true,
      "apiKey": "sk-..."
    },
    "anthropic": {
      "enabled": false
    },
    "fireworks": {
      "enabled": false
    }
  },
  "routing": {
    "localFirst": true,
    "fallbackEnabled": true
  },
  "limits": {
    "maxTokens": 8192,
    "requestTimeoutMs": 60000
  }
}
```

## Integration with Rust Core

The gateway reports usage to the Rust core via `POST {coreApiUrl}/api/usage`:

```json
{
  "records": [{
    "provider": "openai",
    "timestamp": "2025-01-15T10:30:00.000Z",
    "model": "gpt-4o",
    "tokens_in": 150,
    "tokens_out": 200,
    "estimated_cost": 0.002375,
    "correlation_id": "abc-123"
  }]
}
```

Records are buffered and flushed every 10 seconds. The Rust core inserts them into the `usage_costs` table.

## Project Structure

```
packages/gateway/
├── package.json
├── tsconfig.json
├── README.md
└── src/
    ├── index.ts              # Entry point & graceful shutdown
    ├── server.ts             # HTTP server & route dispatch
    ├── config.ts             # Configuration loading (env + file + defaults)
    ├── types.ts              # Shared TypeScript types
    ├── router.ts             # Local-first routing & circuit breaker
    ├── usage.ts              # Usage tracking & cost estimation
    ├── validation.ts         # Zod request validation
    ├── utils.ts              # Logging, HTTP helpers, timing
    └── providers/
        ├── base.ts           # Abstract provider with retry & SSE parsing
        ├── ollama.ts         # Ollama adapter (/api/chat)
        ├── openai.ts         # OpenAI adapter (/v1/chat/completions)
        ├── anthropic.ts      # Anthropic adapter (/v1/messages)
        └── fireworks.ts      # Fireworks adapter (OpenAI-compatible)
```

## Error Handling

| Status | Meaning |
|--------|---------|
| 400 | Invalid request (validation failed) |
| 404 | Unknown endpoint |
| 413 | Request body too large |
| 502 | Provider error (upstream failure) |
| 503 | No provider available |
| 500 | Internal server error |

All errors return:

```json
{
  "error": {
    "message": "Description of the error",
    "type": "invalid_request_error | provider_error | internal_error | unavailable",
    "provider": "openai"
  }
}
```

## Troubleshooting

**Gateway starts but Ollama shows unavailable:**
- Ensure Ollama is running: `ollama serve`
- Check the base URL: `curl http://localhost:11434/api/tags`

**OpenAI/Anthropic requests fail with 401:**
- Verify API keys are set in environment or config file

**Circuit breaker opens for a provider:**
- The provider failed 3+ times consecutively
- Resets automatically after 30 seconds
- Check logs for the underlying error

**Usage records not reaching Rust core:**
- Ensure the core is running on the configured `coreApiUrl`
- Records are buffered and retried on next flush cycle
