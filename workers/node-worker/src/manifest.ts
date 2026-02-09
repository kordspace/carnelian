/**
 * Skill manifest schema and validation.
 *
 * Parses YAML frontmatter from SKILL.md files and validates the manifest
 * structure. Supports both OpenClaw-compatible metadata and Carnelian
 * extensions.
 */

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { parse as parseYaml } from "yaml";

// =============================================================================
// MANIFEST TYPES
// =============================================================================

/** OpenClaw installation instruction */
export interface InstallInstruction {
  id: string;
  kind: string;
  formula?: string;
  cask?: string;
  bins?: string[];
  label?: string;
}

/** OpenClaw metadata section */
export interface OpenClawMetadata {
  emoji?: string;
  requires?: {
    bins?: string[];
    env?: string[];
  };
  primaryEnv?: string;
  install?: InstallInstruction[];
  os?: string[];
}

/** Sandbox mount configuration */
export interface SandboxMount {
  host: string;
  container: string;
  readonly: boolean;
}

/** Sandbox resource limits */
export interface ResourceLimits {
  maxMemoryMB: number;
  maxCpuPercent: number;
  timeoutSecs: number;
}

/** Sandbox configuration */
export interface SandboxConfig {
  mounts?: SandboxMount[];
  network: "none" | "localhost" | "full";
  resourceLimits: ResourceLimits;
  env?: Record<string, string>;
}

/** Carnelian metadata extensions */
export interface CarnelianMetadata {
  runtime: "node" | "python" | "shell";
  version: string;
  sandbox: SandboxConfig;
  capabilities: string[];
}

/** Complete skill manifest */
export interface SkillManifest {
  name: string;
  description: string;
  homepage?: string;
  metadata: {
    openclaw?: OpenClawMetadata;
    carnelian?: CarnelianMetadata;
  };
}

/** Validation error detail */
export interface ValidationError {
  field: string;
  message: string;
}

/** Validation result */
export interface ValidationResult {
  valid: boolean;
  errors: ValidationError[];
  manifest: SkillManifest | null;
  checksum: string | null;
}

// =============================================================================
// YAML FRONTMATTER PARSER
// =============================================================================

/**
 * Parse YAML frontmatter from a SKILL.md file.
 *
 * Extracts the content between `---` delimiters at the start of the file
 * and parses it using a full YAML parser that supports nested objects,
 * arrays of objects, and all standard YAML features.
 */
export function parseFrontmatter(content: string): Record<string, unknown> {
  const lines = content.split("\n");
  if (lines[0]?.trim() !== "---") {
    return {};
  }

  let endIndex = -1;
  for (let i = 1; i < lines.length; i++) {
    if (lines[i]?.trim() === "---") {
      endIndex = i;
      break;
    }
  }

  if (endIndex === -1) return {};

  const yamlContent = lines.slice(1, endIndex).join("\n");
  try {
    const parsed = parseYaml(yamlContent);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      return parsed as Record<string, unknown>;
    }
    return {};
  } catch {
    return {};
  }
}

// =============================================================================
// MANIFEST VALIDATION
// =============================================================================

/**
 * Validate a parsed manifest object.
 *
 * Checks required fields, validates metadata structure, and computes
 * a SHA-256 checksum of the canonical manifest JSON.
 */
export function validateManifest(raw: Record<string, unknown>): ValidationResult {
  const errors: ValidationError[] = [];

  // Required fields
  if (typeof raw.name !== "string" || raw.name.trim() === "") {
    errors.push({ field: "name", message: "name is required and must be a non-empty string" });
  }
  if (typeof raw.description !== "string" || raw.description.trim() === "") {
    errors.push({
      field: "description",
      message: "description is required and must be a non-empty string",
    });
  }

  // Build manifest
  const manifest: SkillManifest = {
    name: String(raw.name ?? ""),
    description: String(raw.description ?? ""),
    homepage: typeof raw.homepage === "string" ? raw.homepage : undefined,
    metadata: {
      openclaw: undefined,
      carnelian: undefined,
    },
  };

  // Parse metadata
  const metadata = raw.metadata as Record<string, unknown> | undefined;
  if (metadata && typeof metadata === "object") {
    // OpenClaw metadata
    const oc = metadata.openclaw as Record<string, unknown> | undefined;
    if (oc && typeof oc === "object") {
      manifest.metadata.openclaw = {
        emoji: typeof oc.emoji === "string" ? oc.emoji : undefined,
        requires: parseRequires(oc.requires),
        primaryEnv: typeof oc.primaryEnv === "string" ? oc.primaryEnv : undefined,
        install: Array.isArray(oc.install)
          ? (oc.install as InstallInstruction[])
          : undefined,
        os: Array.isArray(oc.os) ? (oc.os as string[]) : undefined,
      };
    }

    // Carnelian metadata
    const cn = metadata.carnelian as Record<string, unknown> | undefined;
    if (cn && typeof cn === "object") {
      const sandbox = cn.sandbox as Record<string, unknown> | undefined;
      const resourceLimits = (sandbox?.resourceLimits as Record<string, unknown>) ?? {};

      manifest.metadata.carnelian = {
        runtime: validateRuntime(cn.runtime),
        version: typeof cn.version === "string" ? cn.version : "0.0.0",
        sandbox: {
          mounts: Array.isArray(sandbox?.mounts)
            ? (sandbox.mounts as SandboxMount[])
            : undefined,
          network: validateNetwork(sandbox?.network),
          resourceLimits: {
            maxMemoryMB: Number(resourceLimits.maxMemoryMB ?? 512),
            maxCpuPercent: Number(resourceLimits.maxCpuPercent ?? 50),
            timeoutSecs: Number(resourceLimits.timeoutSecs ?? 300),
          },
          env:
            sandbox?.env && typeof sandbox.env === "object"
              ? (sandbox.env as Record<string, string>)
              : undefined,
        },
        capabilities: Array.isArray(cn.capabilities)
          ? (cn.capabilities as string[])
          : [],
      };
    }
  }

  // Compute checksum
  let checksum: string | null = null;
  if (errors.length === 0) {
    const canonical = JSON.stringify(manifest);
    checksum = createHash("sha256").update(canonical).digest("hex");
  }

  return {
    valid: errors.length === 0,
    errors,
    manifest: errors.length === 0 ? manifest : null,
    checksum,
  };
}

function parseRequires(
  raw: unknown,
): { bins?: string[]; env?: string[] } | undefined {
  if (!raw || typeof raw !== "object") return undefined;
  const r = raw as Record<string, unknown>;
  return {
    bins: Array.isArray(r.bins) ? (r.bins as string[]) : undefined,
    env: Array.isArray(r.env) ? (r.env as string[]) : undefined,
  };
}

function validateRuntime(raw: unknown): "node" | "python" | "shell" {
  if (raw === "node" || raw === "python" || raw === "shell") return raw;
  return "node";
}

function validateNetwork(raw: unknown): "none" | "localhost" | "full" {
  if (raw === "none" || raw === "localhost" || raw === "full") return raw;
  return "none";
}

// =============================================================================
// MANIFEST LOADING
// =============================================================================

/**
 * Load and validate a skill manifest from a SKILL.md file.
 *
 * Reads the file, parses YAML frontmatter, validates the manifest,
 * and returns the validation result.
 */
export async function loadManifest(skillMdPath: string): Promise<ValidationResult> {
  const content = await readFile(skillMdPath, "utf-8");
  const raw = parseFrontmatter(content);
  return validateManifest(raw);
}

/**
 * Compute SHA-256 checksum of a manifest in canonical JSON form.
 */
export function computeChecksum(manifest: SkillManifest): string {
  const canonical = JSON.stringify(manifest);
  return createHash("sha256").update(canonical).digest("hex");
}
