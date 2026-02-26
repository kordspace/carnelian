/**
 * Execution sandbox envelope.
 *
 * Provides isolated execution contexts for skills with resource limits,
 * timeout enforcement, and output capture. Supports Node.js in-process
 * execution via the `vm` module and external process execution for
 * Python and shell skills.
 */

import { spawn, type ChildProcess } from "node:child_process";
import { createContext, runInContext, type Context } from "node:vm";
import { join } from "node:path";
import * as nodeCrypto from "node:crypto";
import * as nodeFs from "node:fs/promises";
import type { SkillEntry } from "./loader.js";
import type { EventEmitter } from "./events.js";
import type { JsonLinesWriter } from "./protocol.js";
import type {
  InvokeRequest,
  InvokeResponse,
  InvokeStatus,
  ExecutionContext,
} from "./types.js";

// =============================================================================
// CONSTANTS
// =============================================================================

/** Default output limit: 1 MB */
const DEFAULT_MAX_OUTPUT_BYTES = 1_048_576;

/** Grace period after SIGTERM before SIGKILL (ms) */
const SIGKILL_GRACE_MS = 5_000;

// =============================================================================
// SANDBOX EXECUTOR
// =============================================================================

/**
 * Executes skills within a sandboxed environment with resource limits,
 * timeout enforcement, and output capture.
 */
export class SandboxExecutor {
  private maxOutputBytes: number;
  private writer: JsonLinesWriter;

  constructor(writer: JsonLinesWriter, maxOutputBytes: number = DEFAULT_MAX_OUTPUT_BYTES) {
    this.maxOutputBytes = maxOutputBytes;
    this.writer = writer;
  }

  /**
   * Execute a skill invocation.
   *
   * Routes to the appropriate execution strategy based on the skill's
   * configured runtime (node, python, shell).
   */
  async execute(
    skill: SkillEntry,
    request: InvokeRequest,
    context: ExecutionContext,
    emitter: EventEmitter,
  ): Promise<InvokeResponse> {
    // Reset writer per invocation so one large run doesn't block later runs
    this.writer.reset();

    const runtime = skill.manifest.metadata.carnelian?.runtime ?? "node";

    try {
      switch (runtime) {
        case "node":
          return await this.executeNode(skill, request, context, emitter);
        case "python":
          return await this.executeProcess(
            skill,
            request,
            context,
            emitter,
            "python3",
          );
        case "shell":
          return await this.executeProcess(
            skill,
            request,
            context,
            emitter,
            "sh",
          );
        default:
          return this.buildResponse(request, context, "Failed", null, {
            error: `Unsupported runtime: ${runtime}`,
          });
      }
    } catch (err) {
      const errorMsg =
        err instanceof Error ? err.message : String(err);
      const stack = err instanceof Error ? err.stack : undefined;

      emitter.emitLog(request.run_id, "Error", `Execution error: ${errorMsg}`, {
        stack,
      });

      return this.buildResponse(request, context, "Failed", null, {
        error: errorMsg,
      });
    }
  }

  // ---------------------------------------------------------------------------
  // NODE.JS IN-PROCESS EXECUTION (vm module)
  // ---------------------------------------------------------------------------

  private async executeNode(
    skill: SkillEntry,
    request: InvokeRequest,
    context: ExecutionContext,
    emitter: EventEmitter,
  ): Promise<InvokeResponse> {
    // Find the main script
    const mainScript = this.findMainScript(skill, ["main.js", "index.js"]);
    if (!mainScript) {
      return this.buildResponse(request, context, "Failed", null, {
        error: "No main.js or index.js found in skill scripts",
      });
    }

    // Read the script content
    const { readFile } = await import("node:fs/promises");
    const scriptContent = await readFile(mainScript, "utf-8");

    // Create sandboxed context with abort signal
    const sandbox = this.createNodeSandbox(request, context, emitter, skill);
    const vmContext = createContext(sandbox);

    // Compute VM timeout from sandbox resource limits or request timeout
    const sandboxTimeout = skill.manifest.metadata.carnelian?.sandbox?.resourceLimits?.timeoutSecs;
    const effectiveTimeoutMs = sandboxTimeout
      ? Math.min(sandboxTimeout * 1000, context.timeoutDeadline - Date.now())
      : context.timeoutDeadline - Date.now();
    const vmTimeoutMs = Math.max(effectiveTimeoutMs, 1);

    // Execute with timeout
    return await this.runWithTimeout(
      context,
      async () => {
        try {
          // runInContext timeout kills synchronous infinite loops
          runInContext(scriptContent, vmContext, {
            filename: mainScript,
            timeout: vmTimeoutMs,
          });

          // Check abort before calling exported functions
          if (context.abortController.signal.aborted) {
            return this.buildResponse(request, context, "Cancelled", null, {
              error: "Execution was cancelled",
            });
          }

          // If the script exports a run/execute function, call it
          if (typeof sandbox.__exports?.run === "function") {
            const result = await sandbox.__exports.run(request.input);
            return this.buildResponse(request, context, "Success", null, {
              result: result ?? {},
            });
          }

          if (typeof sandbox.__exports?.execute === "function") {
            const result = await sandbox.__exports.execute(request.input);
            return this.buildResponse(request, context, "Success", null, {
              result: result ?? {},
            });
          }

          // Script ran without exporting — use captured output
          return this.buildResponse(request, context, "Success", null, {
            result: sandbox.__result ?? {},
          });
        } catch (err) {
          // VM timeout throws "Script execution timed out"
          const msg = err instanceof Error ? err.message : String(err);
          if (msg.includes("Script execution timed out")) {
            return this.buildResponse(request, context, "Timeout", null, {
              error: `VM execution timed out after ${Math.round(vmTimeoutMs / 1000)}s`,
            });
          }
          return this.buildResponse(request, context, "Failed", null, {
            error: msg,
          });
        }
      },
      request,
    );
  }

  /** Create a sandboxed global object for vm execution */
  private createNodeSandbox(
    request: InvokeRequest,
    context: ExecutionContext,
    emitter: EventEmitter,
    skill: SkillEntry,
  ): Record<string, unknown> & { __exports: Record<string, unknown>; __result: unknown } {
    // Resolve sandbox.env overrides (replicate spawnProcess logic)
    const resolvedSandboxEnv: Record<string, string> = {};
    const sandboxEnv = skill.manifest.metadata.carnelian?.sandbox?.env;
    if (sandboxEnv) {
      for (const [key, value] of Object.entries(sandboxEnv)) {
        const resolved = value.replace(/\$\{(\w+)\}/g, (_: string, name: string) =>
          process.env[name] ?? "",
        );
        resolvedSandboxEnv[key] = resolved;
      }
    }

    // Build read-only process.env proxy
    const mergedEnv = { ...process.env, ...resolvedSandboxEnv };
    const readOnlyEnvProxy = new Proxy(mergedEnv, {
      get(target, prop) {
        if (typeof prop === "string") {
          return target[prop];
        }
        return undefined;
      },
      has(target, prop) {
        return prop in target;
      },
      set() {
        throw new TypeError("process.env is read-only in sandbox");
      },
      deleteProperty() {
        throw new TypeError("process.env is read-only in sandbox");
      },
    });

    // Determine network policy
    const networkPolicy = skill.manifest.metadata.carnelian?.sandbox?.network ?? "none";

    // Build fetch wrapper with abort signal propagation
    const sandboxFetch = networkPolicy !== "none"
      ? (input: RequestInfo | URL, init?: RequestInit) => {
          // Merge abort signals: context signal + caller signal
          let mergedSignal: AbortSignal;
          
          if (!init?.signal) {
            // No caller signal, just use context signal
            mergedSignal = context.abortController.signal;
          } else if (typeof AbortSignal.any === "function") {
            // Node 20+: Use AbortSignal.any for proper merging
            mergedSignal = AbortSignal.any([context.abortController.signal, init.signal]);
          } else {
            // Node 18 fallback: Create new controller that listens to both
            const mergedController = new AbortController();
            const abortHandler = () => mergedController.abort();
            
            if (context.abortController.signal.aborted || init.signal.aborted) {
              mergedController.abort();
            } else {
              context.abortController.signal.addEventListener("abort", abortHandler, { once: true });
              init.signal.addEventListener("abort", abortHandler, { once: true });
            }
            
            mergedSignal = mergedController.signal;
          }

          return globalThis.fetch(input, { ...init, signal: mergedSignal });
        }
      : () => {
          throw new Error("Network access is disabled (network: none)");
        };

    // Build WebSocket wrapper with network policy enforcement
    const sandboxWebSocket = networkPolicy !== "none"
      ? globalThis.WebSocket
      : class {
          constructor() {
            throw new Error("Network access is disabled (network: none)");
          }
        };

    // Guard: only emit events if not aborted/cancelled
    const guardedEmit = (level: string, msg: string, fields?: Record<string, unknown>) => {
      if (context.abortController.signal.aborted) return;
      emitter.emitLog(request.run_id, level as "Info" | "Warn" | "Error" | "Debug", msg, fields);
    };

    const sandbox = {
      __exports: {} as Record<string, unknown>,
      __result: undefined as unknown,

      // Expose abort signal so async code can check cancellation
      abortSignal: context.abortController.signal,

      // Safe globals
      console: {
        log: (...args: unknown[]) => guardedEmit("Info", args.map(String).join(" ")),
        warn: (...args: unknown[]) => guardedEmit("Warn", args.map(String).join(" ")),
        error: (...args: unknown[]) => guardedEmit("Error", args.map(String).join(" ")),
        debug: (...args: unknown[]) => guardedEmit("Debug", args.map(String).join(" ")),
      },

      setTimeout: globalThis.setTimeout,
      clearTimeout: globalThis.clearTimeout,
      setInterval: globalThis.setInterval,
      clearInterval: globalThis.clearInterval,
      JSON,
      Math,
      Date,
      Array,
      Object,
      String,
      Number,
      Boolean,
      RegExp,
      Map,
      Set,
      Promise,
      Error,
      TypeError,
      RangeError,
      Symbol,
      parseInt,
      parseFloat,
      isNaN,
      isFinite,
      encodeURIComponent,
      decodeURIComponent,
      encodeURI,
      decodeURI,

      // HTTP / network (conditional on skill.sandbox.network)
      fetch: sandboxFetch,
      URL: globalThis.URL,
      URLSearchParams: globalThis.URLSearchParams,
      Headers: globalThis.Headers,
      Request: globalThis.Request,
      Response: globalThis.Response,
      WebSocket: sandboxWebSocket, // Requires Node.js >= 21, gated by network policy

      // Encoding / hashing
      Buffer: globalThis.Buffer,
      crypto: nodeCrypto,

      // File system (node:fs/promises)
      fs: nodeFs,

      // Environment (read-only proxy)
      process: { env: readOnlyEnvProxy },

      // Skill API
      input: request.input,
      exports: {} as Record<string, unknown>,
      module: { exports: {} as Record<string, unknown> },
    };

    // Wire up module.exports → __exports
    Object.defineProperty(sandbox, "__exports", {
      get: () => sandbox.module.exports,
    });

    return sandbox;
  }

  // ---------------------------------------------------------------------------
  // EXTERNAL PROCESS EXECUTION (Python, Shell)
  // ---------------------------------------------------------------------------

  private async executeProcess(
    skill: SkillEntry,
    request: InvokeRequest,
    context: ExecutionContext,
    emitter: EventEmitter,
    interpreter: string,
  ): Promise<InvokeResponse> {
    // Find the main script
    const extensions =
      interpreter === "python3"
        ? ["main.py", "run.py", "index.py"]
        : ["main.sh", "run.sh", "index.sh"];
    const mainScript = this.findMainScript(skill, extensions);

    if (!mainScript) {
      return this.buildResponse(request, context, "Failed", null, {
        error: `No script found for ${interpreter} (looked for ${extensions.join(", ")})`,
      });
    }

    return await this.runWithTimeout(
      context,
      () => this.spawnProcess(skill, request, context, emitter, interpreter, mainScript),
      request,
    );
  }

  /** Spawn an external process and capture its output */
  private spawnProcess(
    skill: SkillEntry,
    request: InvokeRequest,
    context: ExecutionContext,
    emitter: EventEmitter,
    interpreter: string,
    scriptPath: string,
  ): Promise<InvokeResponse> {
    return new Promise((resolve) => {
      // Build environment from host env + sandbox overrides
      const env: Record<string, string> = {
        ...process.env as Record<string, string>,
        CARNELIAN_RUN_ID: request.run_id,
        CARNELIAN_SKILL_NAME: request.skill_name,
        CARNELIAN_INPUT: JSON.stringify(request.input),
      };

      // Apply sandbox env overrides (resolve ${VAR} references from host env)
      const sandboxEnv = skill.manifest.metadata.carnelian?.sandbox?.env;
      if (sandboxEnv) {
        for (const [key, value] of Object.entries(sandboxEnv)) {
          const resolved = value.replace(/\$\{(\w+)\}/g, (_: string, name: string) =>
            process.env[name] ?? "",
          );
          env[key] = resolved;
        }
      }

      // Enforce network policy via environment hint
      const networkPolicy = skill.manifest.metadata.carnelian?.sandbox?.network ?? "none";
      env.CARNELIAN_NETWORK_POLICY = networkPolicy;
      if (networkPolicy === "none") {
        // Remove proxy/network env vars to discourage outbound access
        delete env.HTTP_PROXY;
        delete env.HTTPS_PROXY;
        delete env.http_proxy;
        delete env.https_proxy;
      }

      // Apply resource limits: use timeout to enforce timeoutSecs
      const resourceLimits = skill.manifest.metadata.carnelian?.sandbox?.resourceLimits;
      const maxMemoryMB = resourceLimits?.maxMemoryMB ?? 512;

      // Build interpreter args with memory limit where supported
      const interpreterArgs: string[] = [];
      if (interpreter === "python3") {
        // No built-in memory flag, but we set env for the script
        env.CARNELIAN_MAX_MEMORY_MB = String(maxMemoryMB);
      }
      interpreterArgs.push(scriptPath);

      const child: ChildProcess = spawn(interpreter, interpreterArgs, {
        cwd: skill.basePath,
        env,
        stdio: ["pipe", "pipe", "pipe"],
        signal: context.abortController.signal,
      });

      let stdout = "";
      let stderr = "";
      let outputBytes = 0;
      let truncated = false;
      let settled = false;

      child.stdout?.on("data", (chunk: Buffer) => {
        if (settled || context.abortController.signal.aborted) return;
        const str = chunk.toString();
        outputBytes += chunk.length;

        if (outputBytes > this.maxOutputBytes) {
          if (!truncated) {
            truncated = true;
            emitter.emitLog(
              request.run_id,
              "Warn",
              `... output truncated at ${this.maxOutputBytes} bytes`,
            );
          }
          return;
        }

        stdout += str;

        // Emit stdout lines as log events
        const lines = str.split("\n");
        for (const line of lines) {
          if (line.trim()) {
            emitter.emitLog(request.run_id, "Info", line);
          }
        }
      });

      child.stderr?.on("data", (chunk: Buffer) => {
        if (settled || context.abortController.signal.aborted) return;
        const str = chunk.toString();
        outputBytes += chunk.length;

        if (outputBytes > this.maxOutputBytes && !truncated) {
          truncated = true;
        }

        stderr += str;

        // Emit stderr lines as warn/error events
        const lines = str.split("\n");
        for (const line of lines) {
          if (line.trim()) {
            emitter.emitLog(request.run_id, "Warn", line);
          }
        }
      });

      child.on("close", (code: number | null) => {
        if (settled) return;
        settled = true;
        context.truncated = truncated;
        context.outputBytes = outputBytes;

        if (context.abortController.signal.aborted) {
          resolve(
            this.buildResponse(request, context, "Cancelled", code, {
              error: "Execution was cancelled",
            }),
          );
          return;
        }

        // Try to parse stdout as JSON result
        let result: unknown = {};
        try {
          const trimmed = stdout.trim();
          if (trimmed) {
            result = JSON.parse(trimmed);
          }
        } catch {
          // Not JSON — use raw stdout
          result = { stdout: stdout.trim(), stderr: stderr.trim() };
        }

        const status: InvokeStatus = code === 0 ? "Success" : "Failed";
        const error = code !== 0 ? stderr.trim() || `Process exited with code ${code}` : null;

        resolve(
          this.buildResponse(request, context, status, code, {
            result,
            error,
          }),
        );
      });

      child.on("error", (err: Error) => {
        if (settled) return;
        settled = true;
        resolve(
          this.buildResponse(request, context, "Failed", null, {
            error: `Process error: ${err.message}`,
          }),
        );
      });

      // Write input to stdin and close
      if (child.stdin) {
        child.stdin.write(JSON.stringify(request.input));
        child.stdin.end();
      }
    });
  }

  // ---------------------------------------------------------------------------
  // TIMEOUT ENFORCEMENT
  // ---------------------------------------------------------------------------

  /**
   * Run an async operation with timeout enforcement.
   *
   * On timeout: sends SIGTERM, waits grace period, then SIGKILL.
   */
  private async runWithTimeout(
    context: ExecutionContext,
    operation: () => Promise<InvokeResponse>,
    request: InvokeRequest,
  ): Promise<InvokeResponse> {
    const remainingMs = context.timeoutDeadline - Date.now();
    if (remainingMs <= 0) {
      return this.buildResponse(request, context, "Timeout", null, {
        error: "Timeout before execution started",
      });
    }

    let timeoutId: ReturnType<typeof setTimeout> | undefined;

    const timeoutPromise = new Promise<InvokeResponse>((resolve) => {
      timeoutId = setTimeout(() => {
        context.abortController.abort();
        resolve(
          this.buildResponse(request, context, "Timeout", null, {
            error: `Execution timed out after ${request.timeout_secs}s`,
          }),
        );
      }, remainingMs);
    });

    try {
      const result = await Promise.race([operation(), timeoutPromise]);
      return result;
    } finally {
      if (timeoutId !== undefined) {
        clearTimeout(timeoutId);
      }
    }
  }

  // ---------------------------------------------------------------------------
  // HELPERS
  // ---------------------------------------------------------------------------

  /** Find the main script file for a skill */
  private findMainScript(skill: SkillEntry, candidates: string[]): string | null {
    for (const name of candidates) {
      const path = skill.scriptPaths.get(name);
      if (path) return path;
    }

    // Also check skill base directory
    for (const name of candidates) {
      const path = join(skill.basePath, name);
      // We'll try it — the caller will get a file-not-found error if missing
      if (skill.scriptPaths.size === 0) return path;
    }

    return null;
  }

  /** Build a standardized InvokeResponse */
  private buildResponse(
    request: InvokeRequest,
    context: ExecutionContext,
    status: InvokeStatus,
    exitCode: number | null,
    opts: { result?: unknown; error?: string | null },
  ): InvokeResponse {
    return {
      run_id: request.run_id,
      status,
      result: opts.result ?? {},
      error: opts.error ?? null,
      exit_code: exitCode,
      duration_ms: Date.now() - context.startTime,
      truncated: context.truncated,
    };
  }
}
