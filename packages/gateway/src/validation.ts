import { z } from "zod";

import type { CompletionRequest } from "./types.js";

// =============================================================================
// ZOD SCHEMAS
// =============================================================================

const MessageSchema = z.object({
  role: z.enum(["system", "user", "assistant", "tool"]),
  content: z.string(),
  name: z.string().optional(),
  tool_call_id: z.string().optional(),
});

const CompletionRequestSchema = z.object({
  model: z.string().min(1, "model is required"),
  messages: z.array(MessageSchema).min(1, "messages must not be empty"),
  temperature: z.number().min(0).max(2).optional(),
  max_tokens: z.number().int().positive().optional(),
  stream: z.boolean().optional(),
  top_p: z.number().min(0).max(1).optional(),
  frequency_penalty: z.number().min(-2).max(2).optional(),
  presence_penalty: z.number().min(-2).max(2).optional(),
  stop: z.union([z.string(), z.array(z.string())]).optional(),
  user: z.string().optional(),
  correlation_id: z.string().optional(),
});

// =============================================================================
// VALIDATION
// =============================================================================

/** Result of request validation. */
export type ValidationResult =
  | { ok: true; request: CompletionRequest }
  | { ok: false; error: string };

/**
 * Validate and parse an unknown request body into a typed `CompletionRequest`.
 *
 * Returns a discriminated union so callers can branch on `ok`.
 */
export function validateCompletionRequest(body: unknown): ValidationResult {
  const result = CompletionRequestSchema.safeParse(body);

  if (!result.success) {
    const issues = result.error.issues
      .map((i) => `${i.path.join(".")}: ${i.message}`)
      .join("; ");
    return { ok: false, error: issues };
  }

  return { ok: true, request: result.data as CompletionRequest };
}

// =============================================================================
// TOKEN LIMIT CHECK
// =============================================================================

/**
 * Rough estimate of input tokens based on character count.
 *
 * Uses the ~4 characters per token heuristic. This is intentionally
 * conservative — the real count is determined by the provider.
 */
export function estimateInputTokens(request: CompletionRequest): number {
  let chars = 0;
  for (const msg of request.messages) {
    // Role overhead ≈ 4 tokens
    chars += 16;
    chars += msg.content.length;
    if (msg.name) chars += msg.name.length;
  }
  return Math.ceil(chars / 4);
}

/**
 * Check whether the estimated input tokens exceed a given context window.
 *
 * Returns `null` if within limits, or an error message string if exceeded.
 */
export function checkTokenLimit(
  request: CompletionRequest,
  contextWindow: number,
): string | null {
  const estimated = estimateInputTokens(request);
  if (estimated > contextWindow) {
    return `Estimated input tokens (${estimated}) exceed context window (${contextWindow})`;
  }
  return null;
}
