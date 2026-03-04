import type {
  CompletionChunk,
  CompletionRequest,
  CompletionResponse,
  ProviderName,
  ProviderType,
} from "../types.js";
import { log } from "../utils.js";
import { BaseProvider, parseSseStream } from "./base.js";

// =============================================================================
// OPENAI TYPES
// =============================================================================

interface OpenAiChatRequest {
  model: string;
  messages: Array<{ role: string; content: string; name?: string; tool_call_id?: string }>;
  temperature?: number;
  max_tokens?: number;
  top_p?: number;
  frequency_penalty?: number;
  presence_penalty?: number;
  stop?: string | string[];
  stream?: boolean;
  user?: string;
}

interface OpenAiChatResponse {
  id: string;
  object: string;
  created: number;
  model: string;
  choices: Array<{
    index: number;
    message: { role: string; content: string | null };
    finish_reason: string | null;
  }>;
  usage: {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
  };
}

interface OpenAiStreamChunk {
  id: string;
  object: string;
  created: number;
  model: string;
  choices: Array<{
    index: number;
    delta: { role?: string; content?: string };
    finish_reason: string | null;
  }>;
}

interface OpenAiModelsResponse {
  data: Array<{ id: string }>;
}

// =============================================================================
// OPENAI PROVIDER
// =============================================================================

/**
 * Provider adapter for the OpenAI API.
 *
 * Uses the standard `/v1/chat/completions` endpoint. The response format
 * is already OpenAI-compatible so mapping is minimal.
 */
export class OpenAiProvider extends BaseProvider {
  readonly name: ProviderName = "openai";
  readonly type: ProviderType = "remote";

  constructor(opts: { apiKey: string; baseUrl?: string; timeoutMs?: number }) {
    super({
      baseUrl: opts.baseUrl ?? "https://api.openai.com",
      apiKey: opts.apiKey,
      timeoutMs: opts.timeoutMs,
    });
  }

  // ---------------------------------------------------------------------------
  // Provider interface
  // ---------------------------------------------------------------------------

  async complete(request: CompletionRequest): Promise<CompletionResponse> {
    const body = this.buildOpenAiRequest(request, false);

    const response = await this.fetchWithRetry(
      `${this.baseUrl}/v1/chat/completions`,
      {
        method: "POST",
        headers: this.buildHeaders(),
        body: JSON.stringify(body),
      },
    );

    if (!response.ok) {
      const text = await response.text().catch(() => "");
      throw new Error(`OpenAI error HTTP ${response.status}: ${text}`);
    }

    const data = (await response.json()) as OpenAiChatResponse;

    return {
      id: data.id,
      object: "chat.completion",
      created: data.created,
      model: data.model,
      choices: data.choices.map((c) => ({
        index: c.index,
        message: {
          role: "assistant",
          content: c.message.content ?? "",
        },
        finish_reason: this.mapFinishReason(c.finish_reason),
      })),
      usage: {
        prompt_tokens: data.usage?.prompt_tokens ?? 0,
        completion_tokens: data.usage?.completion_tokens ?? 0,
        total_tokens: data.usage?.total_tokens ?? 0,
      },
      provider: this.name,
    };
  }

  async *completeStream(request: CompletionRequest): AsyncIterable<CompletionChunk> {
    const body = this.buildOpenAiRequest(request, true);

    const response = await this.fetchStream(
      `${this.baseUrl}/v1/chat/completions`,
      {
        method: "POST",
        headers: this.buildHeaders(),
        body: JSON.stringify(body),
      },
    );

    for await (const chunk of parseSseStream<OpenAiStreamChunk>(response)) {
      yield {
        id: chunk.id,
        object: "chat.completion.chunk",
        created: chunk.created,
        model: chunk.model,
        choices: chunk.choices.map((c) => ({
          index: c.index,
          delta: {
            role: c.delta.role as CompletionChunk["choices"][0]["delta"]["role"],
            content: c.delta.content,
          },
          finish_reason: this.mapFinishReason(c.finish_reason),
        })),
      };
    }
  }

  async isAvailable(): Promise<boolean> {
    if (!this.apiKey) return false;
    try {
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), 5_000);
      const response = await fetch(`${this.baseUrl}/v1/models`, {
        headers: this.buildHeaders(),
        signal: controller.signal,
      });
      clearTimeout(timer);
      return response.ok;
    } catch {
      return false;
    }
  }

  async listModels(): Promise<string[]> {
    if (!this.apiKey) return [];
    try {
      const response = await this.fetchWithRetry(
        `${this.baseUrl}/v1/models`,
        { method: "GET", headers: this.buildHeaders() },
        0,
      );
      if (!response.ok) return [];
      const data = (await response.json()) as OpenAiModelsResponse;
      return (data.data ?? []).map((m) => m.id);
    } catch (err) {
      log("warn", "OpenAI: failed to list models", { error: String(err) });
      return [];
    }
  }

  // ---------------------------------------------------------------------------
  // Internal
  // ---------------------------------------------------------------------------

  private buildOpenAiRequest(request: CompletionRequest, stream: boolean): OpenAiChatRequest {
    const req: OpenAiChatRequest = {
      model: request.model,
      messages: request.messages.map((m) => {
        const msg: OpenAiChatRequest["messages"][0] = { role: m.role, content: m.content };
        if (m.name) msg.name = m.name;
        if (m.tool_call_id) msg.tool_call_id = m.tool_call_id;
        return msg;
      }),
      stream,
    };

    if (request.temperature !== undefined) req.temperature = request.temperature;
    if (request.max_tokens !== undefined) req.max_tokens = request.max_tokens;
    if (request.top_p !== undefined) req.top_p = request.top_p;
    if (request.frequency_penalty !== undefined) req.frequency_penalty = request.frequency_penalty;
    if (request.presence_penalty !== undefined) req.presence_penalty = request.presence_penalty;
    if (request.stop !== undefined) req.stop = request.stop;
    if (request.user) req.user = request.user;

    return req;
  }

  private mapFinishReason(
    reason: string | null,
  ): "stop" | "length" | "tool_calls" | "content_filter" | null {
    if (!reason) return null;
    switch (reason) {
      case "stop":
        return "stop";
      case "length":
        return "length";
      case "tool_calls":
      case "function_call":
        return "tool_calls";
      case "content_filter":
        return "content_filter";
      default:
        return "stop";
    }
  }
}
