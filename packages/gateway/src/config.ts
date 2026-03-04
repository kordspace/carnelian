import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { log } from "./utils.js";

// =============================================================================
// CONFIGURATION TYPES
// =============================================================================

/** Per-provider configuration block. */
export interface ProviderConfig {
  enabled: boolean;
  baseUrl?: string;
  apiKey?: string;
  timeoutMs?: number;
}

/** Routing behaviour configuration. */
export interface RoutingConfig {
  /** Prefer local providers when the requested model is available locally. */
  localFirst: boolean;
  /** Fall back to remote providers when local is unavailable. */
  fallbackEnabled: boolean;
}

/** Request limit configuration. */
export interface LimitsConfig {
  /** Maximum tokens a single request may generate. */
  maxTokens: number;
  /** Per-request timeout in milliseconds. */
  requestTimeoutMs: number;
  /** Estimated context window size (tokens) used to reject oversized input. */
  contextWindow: number;
}

/** Top-level gateway configuration. */
export interface GatewayConfig {
  /** HTTP port the gateway listens on. */
  port: number;
  /** Base URL of the Carnelian Rust core API. */
  coreApiUrl: string;
  /** Provider-specific settings. */
  providers: {
    ollama: ProviderConfig;
    openai: ProviderConfig;
    anthropic: ProviderConfig;
    fireworks: ProviderConfig;
  };
  /** Routing behaviour. */
  routing: RoutingConfig;
  /** Request limits. */
  limits: LimitsConfig;
}

// =============================================================================
// DEFAULTS
// =============================================================================

const DEFAULT_CONFIG: GatewayConfig = {
  port: 18790,
  coreApiUrl: "http://localhost:8080",
  providers: {
    ollama: {
      enabled: true,
      baseUrl: "http://localhost:11434",
    },
    openai: {
      enabled: false,
    },
    anthropic: {
      enabled: false,
    },
    fireworks: {
      enabled: false,
    },
  },
  routing: {
    localFirst: true,
    fallbackEnabled: true,
  },
  limits: {
    maxTokens: 8192,
    requestTimeoutMs: 60_000,
    contextWindow: 128_000,
  },
};

// =============================================================================
// LOADER
// =============================================================================

/**
 * Load gateway configuration by merging (in priority order):
 *
 * 1. Environment variables (highest)
 * 2. Config file (`gateway.config.json`)
 * 3. Built-in defaults (lowest)
 */
export function loadConfig(configPath?: string): GatewayConfig {
  // Start with defaults
  const config: GatewayConfig = structuredClone(DEFAULT_CONFIG);

  // Layer 2: config file
  const filePath = configPath ?? resolve(process.cwd(), "gateway.config.json");
  try {
    const raw = readFileSync(filePath, "utf-8");
    const file = JSON.parse(raw) as Partial<GatewayConfig>;
    mergeFileConfig(config, file);
    log("info", `Loaded config from ${filePath}`);
  } catch {
    // Config file is optional
    log("debug", `No config file at ${filePath}, using defaults + env`);
  }

  // Layer 1: environment variables (highest priority)
  applyEnvOverrides(config);

  return config;
}

// =============================================================================
// MERGE HELPERS
// =============================================================================

function mergeFileConfig(target: GatewayConfig, source: Partial<GatewayConfig>): void {
  if (source.port !== undefined) target.port = source.port;
  if (source.coreApiUrl !== undefined) target.coreApiUrl = source.coreApiUrl;

  if (source.providers) {
    for (const key of ["ollama", "openai", "anthropic", "fireworks"] as const) {
      const src = source.providers[key];
      if (src) {
        const tgt = target.providers[key];
        if (src.enabled !== undefined) tgt.enabled = src.enabled;
        if (src.baseUrl !== undefined) tgt.baseUrl = src.baseUrl;
        if (src.apiKey !== undefined) tgt.apiKey = src.apiKey;
        if (src.timeoutMs !== undefined) tgt.timeoutMs = src.timeoutMs;
      }
    }
  }

  if (source.routing) {
    if (source.routing.localFirst !== undefined) target.routing.localFirst = source.routing.localFirst;
    if (source.routing.fallbackEnabled !== undefined) target.routing.fallbackEnabled = source.routing.fallbackEnabled;
  }

  if (source.limits) {
    if (source.limits.maxTokens !== undefined) target.limits.maxTokens = source.limits.maxTokens;
    if (source.limits.requestTimeoutMs !== undefined) target.limits.requestTimeoutMs = source.limits.requestTimeoutMs;
    if (source.limits.contextWindow !== undefined) target.limits.contextWindow = source.limits.contextWindow;
  }
}

function applyEnvOverrides(config: GatewayConfig): void {
  const env = process.env;

  if (env.GATEWAY_PORT) config.port = parseInt(env.GATEWAY_PORT, 10);
  if (env.CORE_API_URL) config.coreApiUrl = env.CORE_API_URL;

  // Ollama
  if (env.OLLAMA_BASE_URL) config.providers.ollama.baseUrl = env.OLLAMA_BASE_URL;
  if (env.OLLAMA_ENABLED) config.providers.ollama.enabled = env.OLLAMA_ENABLED === "true";

  // OpenAI
  if (env.OPENAI_API_KEY) {
    config.providers.openai.apiKey = env.OPENAI_API_KEY;
    config.providers.openai.enabled = true;
  }
  if (env.OPENAI_BASE_URL) config.providers.openai.baseUrl = env.OPENAI_BASE_URL;
  if (env.OPENAI_ENABLED) config.providers.openai.enabled = env.OPENAI_ENABLED === "true";

  // Anthropic
  if (env.ANTHROPIC_API_KEY) {
    config.providers.anthropic.apiKey = env.ANTHROPIC_API_KEY;
    config.providers.anthropic.enabled = true;
  }
  if (env.ANTHROPIC_BASE_URL) config.providers.anthropic.baseUrl = env.ANTHROPIC_BASE_URL;
  if (env.ANTHROPIC_ENABLED) config.providers.anthropic.enabled = env.ANTHROPIC_ENABLED === "true";

  // Fireworks
  if (env.FIREWORKS_API_KEY) {
    config.providers.fireworks.apiKey = env.FIREWORKS_API_KEY;
    config.providers.fireworks.enabled = true;
  }
  if (env.FIREWORKS_BASE_URL) config.providers.fireworks.baseUrl = env.FIREWORKS_BASE_URL;
  if (env.FIREWORKS_ENABLED) config.providers.fireworks.enabled = env.FIREWORKS_ENABLED === "true";

  // Routing
  if (env.LOCAL_FIRST) config.routing.localFirst = env.LOCAL_FIRST === "true";
  if (env.FALLBACK_ENABLED) config.routing.fallbackEnabled = env.FALLBACK_ENABLED === "true";

  // Limits
  if (env.MAX_TOKENS) config.limits.maxTokens = parseInt(env.MAX_TOKENS, 10);
  if (env.REQUEST_TIMEOUT_MS) config.limits.requestTimeoutMs = parseInt(env.REQUEST_TIMEOUT_MS, 10);
  if (env.CONTEXT_WINDOW) config.limits.contextWindow = parseInt(env.CONTEXT_WINDOW, 10);
}
