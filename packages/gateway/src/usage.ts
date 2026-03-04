import type { ProviderName, UsageRecord, UsageStats } from "./types.js";
import { log } from "./utils.js";

// =============================================================================
// PRICING TABLE
// =============================================================================

/** Per-million-token pricing for known models. */
interface ModelPricing {
  inputPerMillion: number;
  outputPerMillion: number;
}

const PRICING: Record<string, ModelPricing> = {
  // OpenAI
  "gpt-4o": { inputPerMillion: 2.5, outputPerMillion: 10.0 },
  "gpt-4o-mini": { inputPerMillion: 0.15, outputPerMillion: 0.6 },
  "gpt-4-turbo": { inputPerMillion: 10.0, outputPerMillion: 30.0 },
  "gpt-4": { inputPerMillion: 30.0, outputPerMillion: 60.0 },
  "gpt-3.5-turbo": { inputPerMillion: 0.5, outputPerMillion: 1.5 },

  // Anthropic
  "claude-3-5-sonnet-20241022": { inputPerMillion: 3.0, outputPerMillion: 15.0 },
  "claude-3-5-haiku-20241022": { inputPerMillion: 0.8, outputPerMillion: 4.0 },
  "claude-3-opus-20240229": { inputPerMillion: 15.0, outputPerMillion: 75.0 },
  "claude-3-sonnet-20240229": { inputPerMillion: 3.0, outputPerMillion: 15.0 },
  "claude-3-haiku-20240307": { inputPerMillion: 0.25, outputPerMillion: 1.25 },

  // Fireworks (representative pricing)
  "accounts/fireworks/models/llama-v3p1-70b-instruct": { inputPerMillion: 0.9, outputPerMillion: 0.9 },
  "accounts/fireworks/models/llama-v3p1-8b-instruct": { inputPerMillion: 0.2, outputPerMillion: 0.2 },
};

// =============================================================================
// COST ESTIMATION
// =============================================================================

/**
 * Estimate the USD cost of a completion based on model pricing.
 *
 * Returns 0 for local models (Ollama) and unknown models.
 */
export function estimateCost(
  model: string,
  provider: ProviderName,
  usage: UsageStats,
): number {
  // Local models have no API cost
  if (provider === "ollama") return 0;

  const pricing = PRICING[model];
  if (!pricing) return 0;

  const inputCost = (usage.prompt_tokens * pricing.inputPerMillion) / 1_000_000;
  const outputCost = (usage.completion_tokens * pricing.outputPerMillion) / 1_000_000;
  return inputCost + outputCost;
}

// =============================================================================
// USAGE TRACKER
// =============================================================================

/**
 * Buffers usage records and periodically flushes them to the Rust core.
 *
 * Records are sent via HTTP POST to `{coreApiUrl}/api/usage`. If the
 * core is unreachable, records are kept in the buffer and retried on
 * the next flush cycle.
 */
export class UsageTracker {
  private readonly coreApiUrl: string;
  private buffer: UsageRecord[] = [];
  private flushTimer: ReturnType<typeof setInterval> | null = null;
  private readonly flushIntervalMs: number;

  constructor(opts: { coreApiUrl: string; flushIntervalMs?: number }) {
    this.coreApiUrl = opts.coreApiUrl.replace(/\/+$/, "");
    this.flushIntervalMs = opts.flushIntervalMs ?? 10_000;
  }

  /** Start the periodic flush timer. */
  start(): void {
    if (this.flushTimer) return;
    this.flushTimer = setInterval(() => {
      void this.flush();
    }, this.flushIntervalMs);
    // Allow the process to exit even if the timer is running
    if (this.flushTimer) {
      (this.flushTimer as { unref?: () => void }).unref?.();
    }
    log("info", "Usage tracker started", { flushIntervalMs: this.flushIntervalMs });
  }

  /** Stop the periodic flush timer and perform a final flush. */
  async stop(): Promise<void> {
    if (this.flushTimer) {
      clearInterval(this.flushTimer);
      this.flushTimer = null;
    }
    await this.flush();
    log("info", "Usage tracker stopped");
  }

  /** Record a usage event. It will be flushed to the core on the next cycle. */
  record(record: UsageRecord): void {
    this.buffer.push(record);
    log("debug", "Usage recorded", {
      provider: record.provider,
      model: record.model,
      tokens_in: record.tokens_in,
      tokens_out: record.tokens_out,
      estimated_cost: record.estimated_cost,
    });
  }

  /** Convenience: build and record a usage event from completion data. */
  trackCompletion(
    provider: ProviderName,
    model: string,
    usage: UsageStats,
    correlationId?: string,
  ): void {
    const cost = estimateCost(model, provider, usage);
    this.record({
      provider,
      timestamp: new Date().toISOString(),
      model,
      tokens_in: usage.prompt_tokens,
      tokens_out: usage.completion_tokens,
      estimated_cost: cost,
      correlation_id: correlationId,
    });
  }

  /** Flush buffered records to the Rust core. */
  async flush(): Promise<void> {
    if (this.buffer.length === 0) return;

    const batch = this.buffer.splice(0);
    try {
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), 5_000);

      const response = await fetch(`${this.coreApiUrl}/api/usage`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ records: batch }),
        signal: controller.signal,
      });

      clearTimeout(timer);

      if (!response.ok) {
        log("warn", "Usage flush failed, re-queuing records", {
          status: response.status,
          count: batch.length,
        });
        // Put records back at the front of the buffer
        this.buffer.unshift(...batch);
      } else {
        log("debug", "Usage flush succeeded", { count: batch.length });
      }
    } catch (err) {
      log("warn", "Usage flush error, re-queuing records", {
        error: String(err),
        count: batch.length,
      });
      this.buffer.unshift(...batch);
    }
  }

  /** Number of records currently buffered. */
  get pendingCount(): number {
    return this.buffer.length;
  }
}
