import type {
  CompletionChunk,
  CompletionRequest,
  CompletionResponse,
  ProviderName,
  ProviderType,
} from "../types.js";
import { log } from "../utils.js";
import { BaseProvider } from "./base.js";

// =============================================================================
// OLLAMA TYPES
// =============================================================================

interface OllamaChatRequest {
  model: string;
  messages: Array<{ role: string; content: string }>;
  stream: boolean;
  options?: {
    temperature?: number;
    top_p?: number;
    num_predict?: number;
    stop?: string[];
    frequency_penalty?: number;
    presence_penalty?: number;
  };
}

interface OllamaChatResponse {
  model: string;
  message: { role: string; content: string };
  done: boolean;
  total_duration?: number;
  prompt_eval_count?: number;
  eval_count?: number;
}

interface OllamaStreamChunk {
  model: string;
  message: { role: string; content: string };
  done: boolean;
  prompt_eval_count?: number;
  eval_count?: number;
}

interface OllamaTagsResponse {
  models: Array<{ name: string; model: string; size: number }>;
}

// =============================================================================
// OLLAMA PROVIDER
// =============================================================================

/**
 * Provider adapter for Ollama local inference.
 *
 * Connects to the Ollama HTTP API (default `http://localhost:11434`).
 * Uses `/api/chat` for completions and `/api/tags` for model listing.
 */
export class OllamaProvider extends BaseProvider {
  readonly name: ProviderName = "ollama";
  readonly type: ProviderType = "local";

  constructor(opts: { baseUrl?: string; timeoutMs?: number } = {}) {
    super({
      baseUrl: opts.baseUrl ?? "http://localhost:11434",
      timeoutMs: opts.timeoutMs ?? 120_000,
    });
  }

  // ---------------------------------------------------------------------------
  // Provider interface
  // ---------------------------------------------------------------------------

  async complete(request: CompletionRequest): Promise<CompletionResponse> {
    const body = this.buildOllamaRequest(request, false);

    const response = await this.fetchWithRetry(`${this.baseUrl}/api/chat`, {
      method: "POST",
      headers: this.buildHeaders(),
      body: JSON.stringify(body),
    });

    if (!response.ok) {
      const text = await response.text().catch(() => "");
      throw new Error(`Ollama error HTTP ${response.status}: ${text}`);
    }

    const data = (await response.json()) as OllamaChatResponse;

    return {
      id: this.generateId(),
      object: "chat.completion",
      created: this.now(),
      model: request.model,
      choices: [
        {
          index: 0,
          message: {
            role: "assistant",
            content: data.message?.content ?? "",
          },
          finish_reason: "stop",
        },
      ],
      usage: {
        prompt_tokens: data.prompt_eval_count ?? 0,
        completion_tokens: data.eval_count ?? 0,
        total_tokens: (data.prompt_eval_count ?? 0) + (data.eval_count ?? 0),
      },
      provider: this.name,
    };
  }

  async *completeStream(request: CompletionRequest): AsyncIterable<CompletionChunk> {
    const body = this.buildOllamaRequest(request, true);

    const response = await this.fetchStream(`${this.baseUrl}/api/chat`, {
      method: "POST",
      headers: this.buildHeaders(),
      body: JSON.stringify(body),
    });

    const reader = response.body?.getReader();
    if (!reader) return;

    const decoder = new TextDecoder();
    let buffer = "";
    const id = this.generateId();
    let sentRole = false;

    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split("\n");
        buffer = lines.pop() ?? "";

        for (const line of lines) {
          const trimmed = line.trim();
          if (!trimmed) continue;

          let chunk: OllamaStreamChunk;
          try {
            chunk = JSON.parse(trimmed) as OllamaStreamChunk;
          } catch {
            continue;
          }

          // Emit role delta first
          if (!sentRole) {
            sentRole = true;
            yield {
              id,
              object: "chat.completion.chunk",
              created: this.now(),
              model: request.model,
              choices: [{ index: 0, delta: { role: "assistant" }, finish_reason: null }],
            };
          }

          if (chunk.done) {
            yield {
              id,
              object: "chat.completion.chunk",
              created: this.now(),
              model: request.model,
              choices: [{ index: 0, delta: {}, finish_reason: "stop" }],
            };
            return;
          }

          const content = chunk.message?.content;
          if (content) {
            yield {
              id,
              object: "chat.completion.chunk",
              created: this.now(),
              model: request.model,
              choices: [{ index: 0, delta: { content }, finish_reason: null }],
            };
          }
        }
      }
    } finally {
      reader.releaseLock();
    }
  }

  async isAvailable(): Promise<boolean> {
    try {
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), 3_000);
      const response = await fetch(`${this.baseUrl}/api/tags`, {
        signal: controller.signal,
      });
      clearTimeout(timer);
      return response.ok;
    } catch {
      return false;
    }
  }

  async listModels(): Promise<string[]> {
    try {
      const response = await this.fetchWithRetry(`${this.baseUrl}/api/tags`, {
        method: "GET",
        headers: this.buildHeaders(),
      }, 0);

      if (!response.ok) return [];

      const data = (await response.json()) as OllamaTagsResponse;
      return (data.models ?? []).map((m) => m.name);
    } catch (err) {
      log("warn", "Ollama: failed to list models", { error: String(err) });
      return [];
    }
  }

  // ---------------------------------------------------------------------------
  // Internal
  // ---------------------------------------------------------------------------

  private buildOllamaRequest(request: CompletionRequest, stream: boolean): OllamaChatRequest {
    const ollamaReq: OllamaChatRequest = {
      model: request.model,
      messages: request.messages.map((m) => ({
        role: m.role,
        content: m.content,
      })),
      stream,
    };

    const options: OllamaChatRequest["options"] = {};
    if (request.temperature !== undefined) options.temperature = request.temperature;
    if (request.top_p !== undefined) options.top_p = request.top_p;
    if (request.max_tokens !== undefined) options.num_predict = request.max_tokens;
    if (request.frequency_penalty !== undefined) options.frequency_penalty = request.frequency_penalty;
    if (request.presence_penalty !== undefined) options.presence_penalty = request.presence_penalty;
    if (request.stop !== undefined) {
      options.stop = Array.isArray(request.stop) ? request.stop : [request.stop];
    }

    if (Object.keys(options).length > 0) {
      ollamaReq.options = options;
    }

    return ollamaReq;
  }
}
