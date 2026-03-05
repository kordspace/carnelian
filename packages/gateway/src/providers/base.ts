import { randomUUID } from "node:crypto";

import type {
  CompletionChunk,
  CompletionRequest,
  CompletionResponse,
  Provider,
  ProviderName,
  ProviderType,
} from "../types.js";
import { log } from "../utils.js";

// =============================================================================
// CONSTANTS
// =============================================================================

/** Default HTTP request timeout in milliseconds. */
const DEFAULT_TIMEOUT_MS = 60_000;

/** Maximum number of retry attempts for transient failures. */
const MAX_RETRIES = 2;

/** Base delay between retries in milliseconds (exponential backoff). */
const RETRY_BASE_DELAY_MS = 500;

// =============================================================================
// BASE PROVIDER
// =============================================================================

/**
 * Abstract base class for all provider adapters.
 *
 * Provides common functionality: HTTP fetching with timeout and retries,
 * unique ID generation, and structured logging.
 */
export abstract class BaseProvider implements Provider {
  abstract readonly name: ProviderName;
  abstract readonly type: ProviderType;

  protected readonly baseUrl: string;
  protected readonly apiKey: string | undefined;
  protected readonly timeoutMs: number;

  constructor(opts: { baseUrl: string; apiKey?: string; timeoutMs?: number }) {
    this.baseUrl = opts.baseUrl.replace(/\/+$/, "");
    this.apiKey = opts.apiKey;
    this.timeoutMs = opts.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  }

  abstract complete(request: CompletionRequest): Promise<CompletionResponse>;
  abstract completeStream(request: CompletionRequest): AsyncIterable<CompletionChunk>;
  abstract isAvailable(): Promise<boolean>;
  abstract listModels(): Promise<string[]>;

  // ---------------------------------------------------------------------------
  // HTTP helpers
  // ---------------------------------------------------------------------------

  /** Build default headers for provider API calls. */
  protected buildHeaders(extra?: Record<string, string>): Record<string, string> {
    const headers: Record<string, string> = {
      "Content-Type": "application/json",
      Accept: "application/json",
    };
    if (this.apiKey) {
      headers["Authorization"] = `Bearer ${this.apiKey}`;
    }
    return { ...headers, ...extra };
  }

  /**
   * Perform an HTTP fetch with timeout and optional retries.
   *
   * Retries are only attempted for network errors and 5xx responses.
   */
  protected async fetchWithRetry(
    url: string,
    init: RequestInit,
    retries: number = MAX_RETRIES,
  ): Promise<Response> {
    let lastError: unknown;

    for (let attempt = 0; attempt <= retries; attempt++) {
      try {
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), this.timeoutMs);

        const response = await fetch(url, {
          ...init,
          signal: controller.signal,
        });

        clearTimeout(timer);

        // Don't retry client errors (4xx)
        if (response.status >= 400 && response.status < 500) {
          return response;
        }

        // Retry on server errors (5xx)
        if (response.status >= 500 && attempt < retries) {
          lastError = new Error(`HTTP ${response.status}: ${response.statusText}`);
          await this.backoff(attempt);
          continue;
        }

        return response;
      } catch (err) {
        lastError = err;
        if (attempt < retries) {
          log("warn", `${this.name}: request attempt ${attempt + 1} failed, retrying...`, {
            error: String(err),
          });
          await this.backoff(attempt);
        }
      }
    }

    throw lastError;
  }

  /** Perform an HTTP fetch for streaming (no retry, returns raw Response). */
  protected async fetchStream(url: string, init: RequestInit): Promise<Response> {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeoutMs);

    const response = await fetch(url, {
      ...init,
      signal: controller.signal,
    });

    clearTimeout(timer);

    if (!response.ok) {
      const body = await response.text().catch(() => "");
      throw new Error(`${this.name} stream error HTTP ${response.status}: ${body}`);
    }

    return response;
  }

  // ---------------------------------------------------------------------------
  // Utilities
  // ---------------------------------------------------------------------------

  /** Generate a unique completion ID. */
  protected generateId(prefix: string = "chatcmpl"): string {
    return `${prefix}_${randomUUID().replace(/-/g, "").slice(0, 24)}`;
  }

  /** Current Unix timestamp in seconds. */
  protected now(): number {
    return Math.floor(Date.now() / 1000);
  }

  /** Exponential backoff sleep. */
  private async backoff(attempt: number): Promise<void> {
    const delay = RETRY_BASE_DELAY_MS * Math.pow(2, attempt);
    await new Promise((resolve) => setTimeout(resolve, delay));
  }
}

// =============================================================================
// SSE PARSER
// =============================================================================

/**
 * Parse a Server-Sent Events stream from a fetch Response body.
 *
 * Yields parsed JSON objects from `data:` lines. Terminates on `[DONE]`.
 */
export async function* parseSseStream<T>(response: Response): AsyncIterable<T> {
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
        if (!trimmed || trimmed.startsWith(":")) continue;

        if (trimmed.startsWith("data:")) {
          const data = trimmed.slice(5).trim();
          if (data === "[DONE]") return;

          try {
            yield JSON.parse(data) as T;
          } catch {
            // Skip malformed JSON lines
          }
        }
      }
    }
  } finally {
    reader.releaseLock();
  }
}
