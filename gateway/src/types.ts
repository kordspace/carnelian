// =============================================================================
// PROVIDER TYPES
// =============================================================================

/** Provider classification matching the database `model_providers.provider_type` column. */
export type ProviderType = "local" | "remote";

/** Identifies a known provider backend. */
export type ProviderName = "ollama" | "openai" | "anthropic" | "fireworks";

// =============================================================================
// MESSAGE TYPES
// =============================================================================

/** Role of a chat message participant. */
export type MessageRole = "system" | "user" | "assistant" | "tool";

/** A single message in a chat conversation. */
export interface Message {
  role: MessageRole;
  content: string;
  name?: string;
  tool_call_id?: string;
}

// =============================================================================
// COMPLETION REQUEST
// =============================================================================

/** Unified completion request accepted by the gateway. */
export interface CompletionRequest {
  /** Model identifier (e.g. "deepseek-r1:7b", "gpt-4o", "claude-3-5-sonnet"). */
  model: string;
  /** Conversation messages. */
  messages: Message[];
  /** Sampling temperature (0–2). */
  temperature?: number;
  /** Maximum tokens to generate. */
  max_tokens?: number;
  /** Whether to stream the response via SSE. */
  stream?: boolean;
  /** Top-p nucleus sampling. */
  top_p?: number;
  /** Frequency penalty (-2 to 2). */
  frequency_penalty?: number;
  /** Presence penalty (-2 to 2). */
  presence_penalty?: number;
  /** Stop sequences. */
  stop?: string | string[];
  /** Opaque user identifier for abuse tracking. */
  user?: string;
  /** Correlation ID propagated from the Rust core. */
  correlation_id?: string;
}

// =============================================================================
// COMPLETION RESPONSE
// =============================================================================

/** A single completion choice. */
export interface Choice {
  index: number;
  message: Message;
  finish_reason: "stop" | "length" | "tool_calls" | "content_filter" | null;
}

/** Token usage statistics for a completion. */
export interface UsageStats {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
}

/** Unified non-streaming completion response. */
export interface CompletionResponse {
  id: string;
  object: "chat.completion";
  created: number;
  model: string;
  choices: Choice[];
  usage: UsageStats;
  /** Which provider actually served the request. */
  provider: ProviderName;
}

// =============================================================================
// STREAMING TYPES
// =============================================================================

/** Delta content in a streaming chunk. */
export interface ChunkDelta {
  role?: MessageRole;
  content?: string;
}

/** A single streaming chunk choice. */
export interface ChunkChoice {
  index: number;
  delta: ChunkDelta;
  finish_reason: "stop" | "length" | "tool_calls" | "content_filter" | null;
}

/** A single SSE chunk during streaming completion. */
export interface CompletionChunk {
  id: string;
  object: "chat.completion.chunk";
  created: number;
  model: string;
  choices: ChunkChoice[];
}

// =============================================================================
// PROVIDER INTERFACE
// =============================================================================

/** Contract that every provider adapter must implement. */
export interface Provider {
  /** Human-readable provider name. */
  readonly name: ProviderName;
  /** Whether this is a local or remote provider. */
  readonly type: ProviderType;
  /** Execute a non-streaming completion. */
  complete(request: CompletionRequest): Promise<CompletionResponse>;
  /** Execute a streaming completion, yielding SSE chunks. */
  completeStream(request: CompletionRequest): AsyncIterable<CompletionChunk>;
  /** Check whether the provider is reachable and ready. */
  isAvailable(): Promise<boolean>;
  /** List model identifiers this provider can serve. */
  listModels(): Promise<string[]>;
}

// =============================================================================
// USAGE REPORTING
// =============================================================================

/** Usage record sent to the Rust core for persistence in `usage_costs`. */
export interface UsageRecord {
  /** Provider name (maps to `model_providers.name`). */
  provider: ProviderName;
  /** ISO-8601 timestamp. */
  timestamp: string;
  /** Model used. */
  model: string;
  /** Prompt / input tokens. */
  tokens_in: number;
  /** Completion / output tokens. */
  tokens_out: number;
  /** Estimated cost in USD. */
  estimated_cost: number;
  /** Optional correlation ID from the originating request. */
  correlation_id?: string;
}

// =============================================================================
// HEALTH
// =============================================================================

/** Per-provider health status. */
export interface ProviderHealth {
  name: ProviderName;
  type: ProviderType;
  available: boolean;
  latency_ms?: number;
  models?: string[];
  error?: string;
}

/** Response shape for GET /health. */
export interface HealthResponse {
  status: "ok" | "degraded" | "unavailable";
  version: string;
  uptime_s: number;
  providers: ProviderHealth[];
}

// =============================================================================
// ERROR
// =============================================================================

/** Standard error response body. */
export interface ErrorResponse {
  error: {
    message: string;
    type: "invalid_request_error" | "provider_error" | "internal_error" | "unavailable";
    provider?: ProviderName;
    code?: string;
  };
}
