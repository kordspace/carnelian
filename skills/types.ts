/**
 * Shared type definitions for Carnelian skills.
 * 
 * These types define the contract between skills and the skill execution runtime.
 */

/** Context provided to a skill during execution */
export interface SkillContext {
  /** Unique identifier for this execution run */
  run_id: string;
  /** Name of the skill being executed */
  skill_name: string;
  /** Input parameters for the skill */
  parameters: Record<string, unknown>;
  /** Gateway URL for making API calls back to Carnelian */
  gateway: string;
  /** Correlation ID for request tracing */
  correlation_id: string | null;
  /** Timeout deadline (epoch ms) */
  timeout_deadline: number;
  /** AbortSignal for cancellation support */
  signal?: AbortSignal;
}

/** Result returned by a skill execution */
export interface SkillResult {
  /** Whether the skill executed successfully */
  success: boolean;
  /** Result data (present on success) */
  data?: unknown;
  /** Error message (present on failure) */
  error?: string;
  /** Additional metadata */
  metadata?: Record<string, unknown>;
}
