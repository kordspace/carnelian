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
// ANTHROPIC TYPES
// =============================================================================

interface AnthropicMessage {
  role: "user" | "assistant";
  content: string;
}

interface AnthropicRequest {
  model: string;
  messages: AnthropicMessage[];
  system?: string;
  max_tokens: number;
  temperature?: number;
  top_p?: number;
  stop_sequences?: string[];
  stream?: boolean;
}

interface AnthropicResponse {
  id: string;
  type: "message";
  role: "assistant";
  content: Array<{ type: "text"; text: string }>;
  model: string;
  stop_reason: "end_turn" | "max_tokens" | "stop_sequence" | null;
  usage: {
    input_tokens: number;
    output_tokens: number;
  };
}

interface AnthropicStreamEvent {
  type: string;
  index?: number;
  delta?: {
    type?: string;
    text?: string;
    stop_reason?: string;
  };
  message?: AnthropicResponse;
  content_block?: { type: string; text: string };
  usage?: { output_tokens: number };
}

// =============================================================================
// ANTHROPIC PROVIDER
// =============================================================================

/**
 * Provider adapter for the Anthropic Messages API.
 *
 * Uses `/v1/messages` endpoint. Maps between Anthropic's message format
 * (separate system prompt, content blocks) and the unified gateway format.
 */
export class AnthropicProvider extends BaseProvider {
  readonly name: ProviderName = "anthropic";
  readonly type: ProviderType = "remote";

  private readonly anthropicVersion: string;

  constructor(opts: { apiKey: string; baseUrl?: string; timeoutMs?: number; version?: string }) {
    super({
      baseUrl: opts.baseUrl ?? "https://api.anthropic.com",
      apiKey: opts.apiKey,
      timeoutMs: opts.timeoutMs,
    });
    this.anthropicVersion = opts.version ?? "2023-06-01";
  }

  // ---------------------------------------------------------------------------
  // Provider interface
  // ---------------------------------------------------------------------------

  protected override buildHeaders(extra?: Record<string, string>): Record<string, string> {
    const headers: Record<string, string> = {
      "Content-Type": "application/json",
      "x-api-key": this.apiKey ?? "",
      "anthropic-version": this.anthropicVersion,
    };
    return { ...headers, ...extra };
  }

  async complete(request: CompletionRequest): Promise<CompletionResponse> {
    const body = this.buildAnthropicRequest(request, false);

    const response = await this.fetchWithRetry(
      `${this.baseUrl}/v1/messages`,
      {
        method: "POST",
        headers: this.buildHeaders(),
        body: JSON.stringify(body),
      },
    );

    if (!response.ok) {
      const text = await response.text().catch(() => "");
      throw new Error(`Anthropic error HTTP ${response.status}: ${text}`);
    }

    const data = (await response.json()) as AnthropicResponse;
    const content = data.content
      .filter((b) => b.type === "text")
      .map((b) => b.text)
      .join("");

    return {
      id: data.id,
      object: "chat.completion",
      created: this.now(),
      model: data.model,
      choices: [
        {
          index: 0,
          message: { role: "assistant", content },
          finish_reason: this.mapStopReason(data.stop_reason),
        },
      ],
      usage: {
        prompt_tokens: data.usage?.input_tokens ?? 0,
        completion_tokens: data.usage?.output_tokens ?? 0,
        total_tokens: (data.usage?.input_tokens ?? 0) + (data.usage?.output_tokens ?? 0),
      },
      provider: this.name,
    };
  }

  async *completeStream(request: CompletionRequest): AsyncIterable<CompletionChunk> {
    const body = this.buildAnthropicRequest(request, true);

    const response = await this.fetchStream(
      `${this.baseUrl}/v1/messages`,
      {
        method: "POST",
        headers: this.buildHeaders(),
        body: JSON.stringify(body),
      },
    );

    const id = this.generateId();
    let sentRole = false;

    const reader = response.body?.getReader();
    if (!reader) return;

    const decoder = new TextDecoder();
    let buffer = "";

    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split("\n");
        buffer = lines.pop() ?? "";

        for (const line of lines) {
          const trimmed = line.trim();

          if (trimmed.startsWith("data:")) {
            const data = trimmed.slice(5).trim();
            if (!data || data === "[DONE]") continue;

            let event: AnthropicStreamEvent;
            try {
              event = JSON.parse(data) as AnthropicStreamEvent;
            } catch {
              continue;
            }

            // Emit role delta on first content
            if (!sentRole && (event.type === "content_block_start" || event.type === "content_block_delta")) {
              sentRole = true;
              yield {
                id,
                object: "chat.completion.chunk",
                created: this.now(),
                model: request.model,
                choices: [{ index: 0, delta: { role: "assistant" }, finish_reason: null }],
              };
            }

            if (event.type === "content_block_delta" && event.delta?.text) {
              yield {
                id,
                object: "chat.completion.chunk",
                created: this.now(),
                model: request.model,
                choices: [
                  {
                    index: 0,
                    delta: { content: event.delta.text },
                    finish_reason: null,
                  },
                ],
              };
            }

            if (event.type === "message_delta") {
              const stopReason = event.delta?.stop_reason;
              yield {
                id,
                object: "chat.completion.chunk",
                created: this.now(),
                model: request.model,
                choices: [
                  {
                    index: 0,
                    delta: {},
                    finish_reason: this.mapStopReason(stopReason ?? null),
                  },
                ],
              };
            }

            if (event.type === "message_stop") {
              return;
            }
          }
        }
      }
    } finally {
      reader.releaseLock();
    }
  }

  async isAvailable(): Promise<boolean> {
    if (!this.apiKey) return false;
    // Anthropic doesn't have a lightweight health endpoint; we verify the key
    // is configured and non-empty. A real availability check would cost tokens.
    return true;
  }

  async listModels(): Promise<string[]> {
    // Anthropic doesn't expose a model listing endpoint.
    // Return the well-known model identifiers.
    return [
      "claude-3-5-sonnet-20241022",
      "claude-3-5-haiku-20241022",
      "claude-3-opus-20240229",
      "claude-3-sonnet-20240229",
      "claude-3-haiku-20240307",
    ];
  }

  // ---------------------------------------------------------------------------
  // Internal
  // ---------------------------------------------------------------------------

  private buildAnthropicRequest(request: CompletionRequest, stream: boolean): AnthropicRequest {
    // Anthropic requires system messages to be passed separately
    const systemParts: string[] = [];
    const messages: AnthropicMessage[] = [];

    for (const msg of request.messages) {
      if (msg.role === "system") {
        systemParts.push(msg.content);
        continue;
      }
      // Anthropic only accepts "user" and "assistant" roles
      const role = msg.role === "tool" ? "user" : msg.role === "user" ? "user" : "assistant";
      messages.push({ role, content: msg.content });
    }

    // Anthropic requires alternating user/assistant messages.
    // Merge consecutive same-role messages.
    const merged = this.mergeConsecutiveMessages(messages);

    const req: AnthropicRequest = {
      model: request.model,
      messages: merged,
      max_tokens: request.max_tokens ?? 4096,
      stream,
    };

    if (systemParts.length > 0) {
      req.system = systemParts.join("\n\n");
    }
    if (request.temperature !== undefined) req.temperature = request.temperature;
    if (request.top_p !== undefined) req.top_p = request.top_p;
    if (request.stop !== undefined) {
      req.stop_sequences = Array.isArray(request.stop) ? request.stop : [request.stop];
    }

    return req;
  }

  private mergeConsecutiveMessages(messages: AnthropicMessage[]): AnthropicMessage[] {
    if (messages.length === 0) return [];

    const merged: AnthropicMessage[] = [{ ...messages[0]! }];
    for (let i = 1; i < messages.length; i++) {
      const prev = merged[merged.length - 1]!;
      const curr = messages[i]!;
      if (curr.role === prev.role) {
        prev.content += "\n\n" + curr.content;
      } else {
        merged.push({ ...curr });
      }
    }
    return merged;
  }

  private mapStopReason(
    reason: string | null,
  ): "stop" | "length" | "tool_calls" | "content_filter" | null {
    if (!reason) return null;
    switch (reason) {
      case "end_turn":
      case "stop_sequence":
        return "stop";
      case "max_tokens":
        return "length";
      default:
        return "stop";
    }
  }
}
