import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";

import type { GatewayConfig } from "./config.js";
import { Router } from "./router.js";
import type {
  CompletionChunk,
  CompletionResponse,
  ErrorResponse,
  HealthResponse,
  ProviderHealth,
  ProviderName,
} from "./types.js";
import { UsageTracker } from "./usage.js";
import { checkTokenLimit, validateCompletionRequest } from "./validation.js";
import { hrTimeMs, log, readJsonBody, sendJson, setSseHeaders, writeDone, writeSse } from "./utils.js";

// =============================================================================
// GATEWAY SERVER
// =============================================================================

const VERSION = "0.1.0";

/**
 * The main HTTP server for the Carnelian LLM Gateway.
 *
 * Exposes:
 * - `POST /v1/complete`        — non-streaming completion
 * - `POST /v1/complete/stream` — streaming completion (SSE)
 * - `GET  /health`             — provider health check
 */
export class GatewayServer {
  private readonly config: GatewayConfig;
  private readonly router: Router;
  private readonly usage: UsageTracker;
  private readonly startedAt: number;
  private server: Server | null = null;

  constructor(config: GatewayConfig) {
    this.config = config;
    this.router = new Router(config);
    this.usage = new UsageTracker({ coreApiUrl: config.coreApiUrl });
    this.startedAt = Date.now();
  }

  // ---------------------------------------------------------------------------
  // Lifecycle
  // ---------------------------------------------------------------------------

  /** Start the HTTP server and background services. */
  async start(): Promise<void> {
    // Refresh local model cache
    await this.router.refreshOllamaModels();

    // Start usage reporting
    this.usage.start();

    // Create and start HTTP server
    this.server = createServer((req, res) => {
      void this.handleRequest(req, res);
    });

    return new Promise((resolve, reject) => {
      this.server!.listen(this.config.port, () => {
        log("info", `Carnelian Gateway listening on port ${this.config.port}`, {
          version: VERSION,
          providers: this.router.getProviderNames(),
        });
        resolve();
      });

      this.server!.on("error", (err) => {
        log("error", "Server error", { error: String(err) });
        reject(err);
      });
    });
  }

  /** Gracefully shut down the server. */
  async stop(): Promise<void> {
    log("info", "Shutting down gateway...");

    // Stop accepting new connections
    if (this.server) {
      await new Promise<void>((resolve) => {
        this.server!.close(() => resolve());
      });
    }

    // Flush pending usage records
    await this.usage.stop();

    log("info", "Gateway stopped");
  }

  // ---------------------------------------------------------------------------
  // Request dispatch
  // ---------------------------------------------------------------------------

  private async handleRequest(req: IncomingMessage, res: ServerResponse): Promise<void> {
    const url = new URL(req.url ?? "/", `http://${req.headers.host ?? "localhost"}`);
    const method = req.method ?? "GET";
    const path = url.pathname;

    try {
      if (method === "GET" && path === "/health") {
        await this.handleHealth(res);
        return;
      }

      if (method === "POST" && path === "/v1/complete") {
        await this.handleComplete(req, res);
        return;
      }

      if (method === "POST" && path === "/v1/complete/stream") {
        await this.handleCompleteStream(req, res);
        return;
      }

      // 404
      sendJson(res, 404, {
        error: { message: `Not found: ${method} ${path}`, type: "invalid_request_error" },
      } satisfies ErrorResponse);
    } catch (err) {
      log("error", "Unhandled request error", { error: String(err), path });
      if (!res.headersSent) {
        sendJson(res, 500, {
          error: { message: "Internal server error", type: "internal_error" },
        } satisfies ErrorResponse);
      }
    }
  }

  // ---------------------------------------------------------------------------
  // POST /v1/complete
  // ---------------------------------------------------------------------------

  private async handleComplete(req: IncomingMessage, res: ServerResponse): Promise<void> {
    const body = await readJsonBody(req, res);
    if (body === undefined) return;

    const validation = validateCompletionRequest(body);
    if (!validation.ok) {
      sendJson(res, 400, {
        error: { message: validation.error, type: "invalid_request_error" },
      } satisfies ErrorResponse);
      return;
    }

    const request = validation.request;

    // Enforce request limits before dispatching
    const limitError = this.enforceRequestLimits(request);
    if (limitError) {
      sendJson(res, 400, {
        error: { message: limitError, type: "invalid_request_error" },
      } satisfies ErrorResponse);
      return;
    }

    // Route to provider
    const provider = await this.router.route(request);
    if (!provider) {
      sendJson(res, 503, {
        error: {
          message: `No provider available for model "${request.model}"`,
          type: "unavailable",
        },
      } satisfies ErrorResponse);
      return;
    }

    const startMs = hrTimeMs();

    try {
      const response: CompletionResponse = await provider.complete(request);

      this.router.recordSuccess(provider.name);

      // Track usage
      this.usage.trackCompletion(
        provider.name,
        request.model,
        response.usage,
        request.correlation_id,
      );

      const durationMs = Math.round(hrTimeMs() - startMs);
      log("info", "Completion", {
        model: request.model,
        provider: provider.name,
        tokens_in: response.usage.prompt_tokens,
        tokens_out: response.usage.completion_tokens,
        duration_ms: durationMs,
      });

      sendJson(res, 200, response);
    } catch (err) {
      this.router.recordFailure(provider.name);

      const durationMs = Math.round(hrTimeMs() - startMs);
      log("error", "Completion failed", {
        model: request.model,
        provider: provider.name,
        error: String(err),
        duration_ms: durationMs,
      });

      sendJson(res, 502, {
        error: {
          message: `Provider error: ${String(err)}`,
          type: "provider_error",
          provider: provider.name,
        },
      } satisfies ErrorResponse);
    }
  }

  // ---------------------------------------------------------------------------
  // POST /v1/complete/stream
  // ---------------------------------------------------------------------------

  private async handleCompleteStream(req: IncomingMessage, res: ServerResponse): Promise<void> {
    const body = await readJsonBody(req, res);
    if (body === undefined) return;

    const validation = validateCompletionRequest(body);
    if (!validation.ok) {
      sendJson(res, 400, {
        error: { message: validation.error, type: "invalid_request_error" },
      } satisfies ErrorResponse);
      return;
    }

    const request = validation.request;

    // Enforce request limits before dispatching
    const limitError = this.enforceRequestLimits(request);
    if (limitError) {
      sendJson(res, 400, {
        error: { message: limitError, type: "invalid_request_error" },
      } satisfies ErrorResponse);
      return;
    }

    // Route to provider
    const provider = await this.router.route(request);
    if (!provider) {
      sendJson(res, 503, {
        error: {
          message: `No provider available for model "${request.model}"`,
          type: "unavailable",
        },
      } satisfies ErrorResponse);
      return;
    }

    setSseHeaders(res);

    const startMs = hrTimeMs();
    let totalContent = "";
    let closed = false;

    req.on("close", () => {
      closed = true;
    });

    try {
      for await (const chunk of provider.completeStream(request) as AsyncIterable<CompletionChunk>) {
        if (closed) break;

        // Accumulate content for usage estimation
        const content = chunk.choices[0]?.delta?.content;
        if (content) totalContent += content;

        writeSse(res, chunk);
      }

      if (!closed) {
        writeDone(res);
        res.end();
      }

      this.router.recordSuccess(provider.name);

      // Estimate usage for streaming (providers don't always report tokens in chunks)
      const estimatedPromptTokens = Math.ceil(
        request.messages.reduce((acc, m) => acc + m.content.length, 0) / 4,
      );
      const estimatedCompletionTokens = Math.ceil(totalContent.length / 4);

      this.usage.trackCompletion(
        provider.name,
        request.model,
        {
          prompt_tokens: estimatedPromptTokens,
          completion_tokens: estimatedCompletionTokens,
          total_tokens: estimatedPromptTokens + estimatedCompletionTokens,
        },
        request.correlation_id,
      );

      const durationMs = Math.round(hrTimeMs() - startMs);
      log("info", "Stream completion", {
        model: request.model,
        provider: provider.name,
        est_tokens_in: estimatedPromptTokens,
        est_tokens_out: estimatedCompletionTokens,
        duration_ms: durationMs,
      });
    } catch (err) {
      this.router.recordFailure(provider.name);

      const durationMs = Math.round(hrTimeMs() - startMs);
      log("error", "Stream completion failed", {
        model: request.model,
        provider: provider.name,
        error: String(err),
        duration_ms: durationMs,
      });

      if (!closed && !res.headersSent) {
        sendJson(res, 502, {
          error: {
            message: `Provider error: ${String(err)}`,
            type: "provider_error",
            provider: provider.name,
          },
        } satisfies ErrorResponse);
      } else if (!closed) {
        // Already streaming — send error as final SSE event
        writeSse(res, {
          error: { message: String(err), type: "provider_error" },
        });
        writeDone(res);
        res.end();
      }
    }
  }

  // ---------------------------------------------------------------------------
  // Request limit enforcement
  // ---------------------------------------------------------------------------

  /**
   * Enforce configured request limits before dispatching to a provider.
   *
   * - Rejects requests whose estimated input tokens exceed the context window.
   * - Clamps `max_tokens` to `config.limits.maxTokens` if it exceeds the limit.
   *
   * Returns an error message string if the request should be rejected, or `null` if OK.
   */
  private enforceRequestLimits(request: import("./types.js").CompletionRequest): string | null {
    const limits = this.config.limits;

    // Check estimated input tokens against context window
    const tokenError = checkTokenLimit(request, limits.contextWindow);
    if (tokenError) return tokenError;

    // Reject if max_tokens exceeds the configured ceiling
    if (request.max_tokens !== undefined && request.max_tokens > limits.maxTokens) {
      request.max_tokens = limits.maxTokens;
      log("debug", "Clamped max_tokens to configured limit", {
        original: request.max_tokens,
        clamped: limits.maxTokens,
      });
    }

    return null;
  }

  // ---------------------------------------------------------------------------
  // GET /health
  // ---------------------------------------------------------------------------

  private async handleHealth(res: ServerResponse): Promise<void> {
    const availability = await this.router.checkProviders();
    const providers: ProviderHealth[] = [];

    for (const name of this.router.getProviderNames()) {
      const provider = this.router.getProvider(name);
      const available = availability.get(name) ?? false;

      let models: string[] | undefined;
      if (available && provider) {
        try {
          models = await provider.listModels();
        } catch {
          // Ignore
        }
      }

      providers.push({
        name,
        type: provider?.type ?? "remote",
        available,
        models,
      });
    }

    const anyAvailable = providers.some((p) => p.available);
    const allAvailable = providers.every((p) => p.available);

    const health: HealthResponse = {
      status: allAvailable ? "ok" : anyAvailable ? "degraded" : "unavailable",
      version: VERSION,
      uptime_s: Math.floor((Date.now() - this.startedAt) / 1000),
      providers,
    };

    sendJson(res, anyAvailable ? 200 : 503, health);
  }
}
