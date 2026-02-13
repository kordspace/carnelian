import type { GatewayConfig } from "./config.js";
import { OllamaProvider } from "./providers/ollama.js";
import { OpenAiProvider } from "./providers/openai.js";
import { AnthropicProvider } from "./providers/anthropic.js";
import { FireworksProvider } from "./providers/fireworks.js";
import type { CompletionRequest, Provider, ProviderName } from "./types.js";
import { log } from "./utils.js";

// =============================================================================
// MODEL → PROVIDER MAPPING
// =============================================================================

/** Rules for mapping a model name to a provider. */
interface ModelRule {
  /** Substring or regex pattern to match against the model name. */
  pattern: string;
  /** Provider to route to when matched. */
  provider: ProviderName;
}

/**
 * Default model routing rules evaluated in order.
 *
 * The first matching rule wins. Models not matching any rule fall through
 * to the local-first / fallback logic.
 */
const MODEL_RULES: ModelRule[] = [
  // Anthropic models
  { pattern: "claude", provider: "anthropic" },

  // OpenAI models
  { pattern: "gpt-", provider: "openai" },
  { pattern: "o1", provider: "openai" },
  { pattern: "o3", provider: "openai" },

  // Fireworks models
  { pattern: "accounts/fireworks", provider: "fireworks" },

  // Everything else (Ollama-style tags like "deepseek-r1:7b", "llama3:8b")
  // handled by the local-first fallback below.
];

// =============================================================================
// CIRCUIT BREAKER
// =============================================================================

interface CircuitState {
  failures: number;
  lastFailure: number;
  open: boolean;
}

const CIRCUIT_FAILURE_THRESHOLD = 3;
const CIRCUIT_RESET_MS = 30_000;

// =============================================================================
// ROUTER
// =============================================================================

/**
 * Routes completion requests to the appropriate provider.
 *
 * Implements local-first routing: if the requested model is available
 * on Ollama it is served locally. Otherwise the router selects a remote
 * provider based on model name patterns and configuration.
 *
 * A simple circuit breaker prevents repeated calls to a failing provider.
 */
export class Router {
  private readonly providers: Map<ProviderName, Provider> = new Map();
  private readonly circuits: Map<ProviderName, CircuitState> = new Map();
  private readonly config: GatewayConfig;
  private ollamaModels: Set<string> = new Set();

  constructor(config: GatewayConfig) {
    this.config = config;
    this.initProviders();
  }

  // ---------------------------------------------------------------------------
  // Initialisation
  // ---------------------------------------------------------------------------

  private initProviders(): void {
    const pc = this.config.providers;

    if (pc.ollama.enabled) {
      this.providers.set(
        "ollama",
        new OllamaProvider({
          baseUrl: pc.ollama.baseUrl,
          timeoutMs: pc.ollama.timeoutMs,
        }),
      );
    }

    if (pc.openai.enabled && pc.openai.apiKey) {
      this.providers.set(
        "openai",
        new OpenAiProvider({
          apiKey: pc.openai.apiKey,
          baseUrl: pc.openai.baseUrl,
          timeoutMs: pc.openai.timeoutMs,
        }),
      );
    }

    if (pc.anthropic.enabled && pc.anthropic.apiKey) {
      this.providers.set(
        "anthropic",
        new AnthropicProvider({
          apiKey: pc.anthropic.apiKey,
          baseUrl: pc.anthropic.baseUrl,
          timeoutMs: pc.anthropic.timeoutMs,
        }),
      );
    }

    if (pc.fireworks.enabled && pc.fireworks.apiKey) {
      this.providers.set(
        "fireworks",
        new FireworksProvider({
          apiKey: pc.fireworks.apiKey,
          baseUrl: pc.fireworks.baseUrl,
          timeoutMs: pc.fireworks.timeoutMs,
        }),
      );
    }

    log("info", "Router initialised", {
      providers: [...this.providers.keys()],
    });
  }

  /**
   * Refresh the cached set of locally available Ollama models.
   *
   * Called on startup and can be called periodically.
   */
  async refreshOllamaModels(): Promise<void> {
    const ollama = this.providers.get("ollama");
    if (!ollama) return;

    try {
      const models = await ollama.listModels();
      this.ollamaModels = new Set(models);
      log("info", "Ollama models refreshed", { count: models.length, models });
    } catch (err) {
      log("warn", "Failed to refresh Ollama models", { error: String(err) });
    }
  }

  // ---------------------------------------------------------------------------
  // Routing
  // ---------------------------------------------------------------------------

  /**
   * Select the provider for a given completion request.
   *
   * Algorithm:
   * 1. Check model name against known routing rules.
   * 2. If `localFirst` and the model is available on Ollama → use Ollama.
   * 3. If a rule matched a remote provider and it's available → use it.
   * 4. If `fallbackEnabled`, try any available remote provider.
   * 5. Return `null` if no provider can serve the request.
   */
  async route(request: CompletionRequest): Promise<Provider | null> {
    const model = request.model;

    // Step 1: Check if model matches a specific provider rule
    const ruleMatch = this.matchModelRule(model);

    // Step 2: Local-first — check Ollama
    if (this.config.routing.localFirst) {
      const ollama = this.providers.get("ollama");
      if (ollama && !this.isCircuitOpen("ollama")) {
        // Check if the model is available locally (exact match or tag match)
        if (this.isModelLocal(model)) {
          log("debug", "Routing to Ollama (local-first)", { model });
          return ollama;
        }
      }
    }

    // Step 3: Use the rule-matched provider if available
    if (ruleMatch) {
      const provider = this.providers.get(ruleMatch);
      if (provider && !this.isCircuitOpen(ruleMatch)) {
        log("debug", "Routing to rule-matched provider", { model, provider: ruleMatch });
        return provider;
      }
    }

    // Step 4: Fallback — try any available remote provider
    if (this.config.routing.fallbackEnabled) {
      for (const [name, provider] of this.providers) {
        if (name === "ollama") continue; // Already tried
        if (this.isCircuitOpen(name)) continue;
        log("debug", "Routing to fallback provider", { model, provider: name });
        return provider;
      }
    }

    // Step 5: Last resort — try Ollama even if model isn't listed
    const ollama = this.providers.get("ollama");
    if (ollama && !this.isCircuitOpen("ollama")) {
      log("debug", "Routing to Ollama (last resort)", { model });
      return ollama;
    }

    log("warn", "No provider available for model", { model });
    return null;
  }

  /** Record a successful call to a provider (resets circuit breaker). */
  recordSuccess(name: ProviderName): void {
    this.circuits.delete(name);
  }

  /** Record a failed call to a provider (increments circuit breaker). */
  recordFailure(name: ProviderName): void {
    const state = this.circuits.get(name) ?? { failures: 0, lastFailure: 0, open: false };
    state.failures += 1;
    state.lastFailure = Date.now();
    if (state.failures >= CIRCUIT_FAILURE_THRESHOLD) {
      state.open = true;
      log("warn", "Circuit breaker opened", { provider: name, failures: state.failures });
    }
    this.circuits.set(name, state);
  }

  // ---------------------------------------------------------------------------
  // Health
  // ---------------------------------------------------------------------------

  /** Check availability of all configured providers. */
  async checkProviders(): Promise<Map<ProviderName, boolean>> {
    const results = new Map<ProviderName, boolean>();
    const checks = [...this.providers.entries()].map(async ([name, provider]) => {
      try {
        const available = await provider.isAvailable();
        results.set(name, available);
      } catch {
        results.set(name, false);
      }
    });
    await Promise.all(checks);
    return results;
  }

  /** Get the provider instance by name. */
  getProvider(name: ProviderName): Provider | undefined {
    return this.providers.get(name);
  }

  /** Get all configured provider names. */
  getProviderNames(): ProviderName[] {
    return [...this.providers.keys()];
  }

  // ---------------------------------------------------------------------------
  // Internal
  // ---------------------------------------------------------------------------

  private matchModelRule(model: string): ProviderName | null {
    const lower = model.toLowerCase();
    for (const rule of MODEL_RULES) {
      if (lower.includes(rule.pattern)) {
        return rule.provider;
      }
    }
    return null;
  }

  private isModelLocal(model: string): boolean {
    if (this.ollamaModels.has(model)) return true;
    // Also check without tag (e.g. "deepseek-r1" matches "deepseek-r1:7b")
    for (const local of this.ollamaModels) {
      const base = local.split(":")[0];
      if (base && model.startsWith(base)) return true;
    }
    return false;
  }

  private isCircuitOpen(name: ProviderName): boolean {
    const state = this.circuits.get(name);
    if (!state || !state.open) return false;

    // Check if enough time has passed to half-open the circuit
    if (Date.now() - state.lastFailure > CIRCUIT_RESET_MS) {
      state.open = false;
      state.failures = 0;
      log("info", "Circuit breaker reset", { provider: name });
      return false;
    }

    return true;
  }
}
