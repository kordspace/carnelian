/**
 * Skill discovery and loading.
 *
 * Scans skill directories for SKILL.md files, parses and validates
 * manifests, and maintains a registry of available skills.
 */

import { readdir, stat, access, constants } from "node:fs/promises";
import { join, resolve } from "node:path";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { loadManifest, computeChecksum } from "./manifest.js";
import type { SkillManifest } from "./manifest.js";

const execFileAsync = promisify(execFile);

// =============================================================================
// SKILL ENTRY
// =============================================================================

/** A loaded and validated skill in the registry */
export interface SkillEntry {
  /** Validated skill manifest */
  manifest: SkillManifest;
  /** Absolute path to the skill directory */
  basePath: string;
  /** Map of script names to absolute file paths */
  scriptPaths: Map<string, string>;
  /** SHA-256 checksum of the manifest */
  checksum: string;
  /** When the skill was loaded (epoch ms) */
  loadedAt: number;
}

/** Validation issue found during skill loading */
export interface SkillValidationIssue {
  skillName: string;
  level: "error" | "warning";
  message: string;
}

// =============================================================================
// SKILL LOADER
// =============================================================================

/**
 * Discovers, loads, and validates skills from a directory tree.
 *
 * Each skill is expected to live in its own subdirectory containing a
 * SKILL.md file with YAML frontmatter describing the manifest.
 */
export class SkillLoader {
  private registry: Map<string, SkillEntry> = new Map();
  private skillsDir: string;

  constructor(skillsDir: string) {
    this.skillsDir = resolve(skillsDir);
  }

  /**
   * Scan the skills directory and load all valid skill manifests.
   *
   * Returns an array of validation issues encountered during loading.
   */
  async scanAndLoad(): Promise<SkillValidationIssue[]> {
    const issues: SkillValidationIssue[] = [];

    let entries: string[];
    try {
      entries = await readdir(this.skillsDir);
    } catch {
      issues.push({
        skillName: "*",
        level: "error",
        message: `Skills directory not found: ${this.skillsDir}`,
      });
      return issues;
    }

    for (const entry of entries) {
      const skillDir = join(this.skillsDir, entry);
      const skillMdPath = join(skillDir, "SKILL.md");

      // Check if it's a directory
      try {
        const s = await stat(skillDir);
        if (!s.isDirectory()) continue;
      } catch {
        continue;
      }

      // Check if SKILL.md exists
      try {
        await access(skillMdPath, constants.R_OK);
      } catch {
        continue; // Not a skill directory
      }

      // Load and validate manifest
      try {
        const result = await loadManifest(skillMdPath);

        if (!result.valid || !result.manifest) {
          for (const err of result.errors) {
            issues.push({
              skillName: entry,
              level: "error",
              message: `${err.field}: ${err.message}`,
            });
          }
          continue;
        }

        // Discover script files
        const scriptPaths = await this.discoverScripts(skillDir);

        // Build skill entry
        const skillEntry: SkillEntry = {
          manifest: result.manifest,
          basePath: skillDir,
          scriptPaths,
          checksum: result.checksum ?? computeChecksum(result.manifest),
          loadedAt: Date.now(),
        };

        this.registry.set(result.manifest.name, skillEntry);

        // Run optional validation checks
        const validationIssues = await this.validateSkillDependencies(
          result.manifest,
          entry,
        );
        issues.push(...validationIssues);
      } catch (err) {
        issues.push({
          skillName: entry,
          level: "error",
          message: `Failed to load manifest: ${err instanceof Error ? err.message : String(err)}`,
        });
      }
    }

    return issues;
  }

  /** Get a skill from the registry by name */
  getSkill(name: string): SkillEntry | undefined {
    return this.registry.get(name);
  }

  /** Get all loaded skills */
  getAllSkills(): Map<string, SkillEntry> {
    return new Map(this.registry);
  }

  /** Get the number of loaded skills */
  getSkillCount(): number {
    return this.registry.size;
  }

  /** Reload a specific skill by name */
  async reloadSkill(name: string): Promise<SkillValidationIssue[]> {
    const existing = this.registry.get(name);
    if (!existing) {
      return [{ skillName: name, level: "error", message: "Skill not found in registry" }];
    }

    const skillMdPath = join(existing.basePath, "SKILL.md");
    const result = await loadManifest(skillMdPath);

    if (!result.valid || !result.manifest) {
      return result.errors.map((err) => ({
        skillName: name,
        level: "error" as const,
        message: `${err.field}: ${err.message}`,
      }));
    }

    const scriptPaths = await this.discoverScripts(existing.basePath);
    this.registry.set(name, {
      manifest: result.manifest,
      basePath: existing.basePath,
      scriptPaths,
      checksum: result.checksum ?? computeChecksum(result.manifest),
      loadedAt: Date.now(),
    });

    return [];
  }

  // ---------------------------------------------------------------------------
  // PRIVATE HELPERS
  // ---------------------------------------------------------------------------

  /** Discover script files in a skill directory */
  private async discoverScripts(skillDir: string): Promise<Map<string, string>> {
    const scripts = new Map<string, string>();
    const scriptsDir = join(skillDir, "scripts");

    try {
      const entries = await readdir(scriptsDir);
      for (const entry of entries) {
        const fullPath = join(scriptsDir, entry);
        const s = await stat(fullPath);
        if (s.isFile()) {
          scripts.set(entry, fullPath);
        }
      }
    } catch {
      // No scripts directory — that's fine for reference-only skills
    }

    return scripts;
  }

  /** Validate that required binaries and env vars are available */
  private async validateSkillDependencies(
    manifest: SkillManifest,
    dirName: string,
  ): Promise<SkillValidationIssue[]> {
    const issues: SkillValidationIssue[] = [];
    const requires = manifest.metadata.openclaw?.requires;

    // Check required binaries
    if (requires?.bins) {
      for (const bin of requires.bins) {
        const found = await this.checkBinaryExists(bin);
        if (!found) {
          issues.push({
            skillName: dirName,
            level: "warning",
            message: `Required binary not found in PATH: ${bin}`,
          });
        }
      }
    }

    // Check required environment variables
    if (requires?.env) {
      for (const envVar of requires.env) {
        if (!process.env[envVar]) {
          issues.push({
            skillName: dirName,
            level: "warning",
            message: `Required environment variable not set: ${envVar}`,
          });
        }
      }
    }

    return issues;
  }

  /** Check if a binary exists in PATH */
  private async checkBinaryExists(bin: string): Promise<boolean> {
    const cmd = process.platform === "win32" ? "where" : "which";
    try {
      await execFileAsync(cmd, [bin]);
      return true;
    } catch {
      return false;
    }
  }
}
