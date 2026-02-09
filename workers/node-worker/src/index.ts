/**
 * Carnelian Node Worker — main entry point.
 *
 * Implements a JSON Lines protocol worker that communicates with the Rust
 * ProcessJsonlTransport over stdin/stdout. Discovers and loads skills from
 * a configurable directory, executes them in sandboxed environments, and
 * streams events back to the orchestrator.
 */

import { randomUUID } from "node:crypto";
import { JsonLinesReader, JsonLinesWriter } from "./protocol.js";
import { SkillLoader } from "./loader.js";
import { SandboxExecutor } from "./sandbox.js";
import { EventEmitter } from "./events.js";
import type {
  TransportMessage,
  InvokeRequest,
  CancelRequest,
  InvokeResponse,
  ExecutionContext,
} from "./types.js";

// =============================================================================
// CONSTANTS
// =============================================================================

const VERSION = "0.1.0";
const DEFAULT_SKILLS_DIR = process.env.CARNELIAN_SKILLS_DIR ??
  new URL("../../skills/registry", import.meta.url).pathname.replace(/^\/([A-Z]:)/, "$1");
const MAX_OUTPUT_BYTES = Number(process.env.CARNELIAN_MAX_OUTPUT_BYTES ?? 1_048_576);

// =============================================================================
// WORKER STATE
// =============================================================================

const startTime = Date.now();
const activeExecutions = new Map<string, ExecutionContext>();
const writer = new JsonLinesWriter(MAX_OUTPUT_BYTES);
const emitter = new EventEmitter(writer);
const sandbox = new SandboxExecutor(writer, MAX_OUTPUT_BYTES);
let loader: SkillLoader;
let shuttingDown = false;

// =============================================================================
// MESSAGE HANDLERS
// =============================================================================

/**
 * Handle an Invoke message: look up the skill, create an execution context,
 * run the skill in a sandbox, and send back the result.
 */
async function handleInvoke(messageId: string, request: InvokeRequest): Promise<void> {
  const context: ExecutionContext = {
    runId: request.run_id,
    skillName: request.skill_name,
    startTime: Date.now(),
    timeoutDeadline: Date.now() + request.timeout_secs * 1000,
    abortController: new AbortController(),
    correlationId: request.correlation_id,
    outputBytes: 0,
    truncated: false,
  };

  activeExecutions.set(request.run_id, context);

  emitter.emitLog(request.run_id, "Info", `Invoking skill: ${request.skill_name}`, {
    correlation_id: request.correlation_id,
  });

  let response: InvokeResponse;

  try {
    const skill = loader.getSkill(request.skill_name);
    if (!skill) {
      response = {
        run_id: request.run_id,
        status: "Failed",
        result: {},
        error: `Skill not found: ${request.skill_name}`,
        exit_code: null,
        duration_ms: Date.now() - context.startTime,
        truncated: false,
      };
    } else {
      response = await sandbox.execute(skill, request, context, emitter);
    }
  } catch (err) {
    const errorMsg = err instanceof Error ? err.message : String(err);
    response = {
      run_id: request.run_id,
      status: "Failed",
      result: {},
      error: errorMsg,
      exit_code: null,
      duration_ms: Date.now() - context.startTime,
      truncated: context.truncated,
    };
  } finally {
    activeExecutions.delete(request.run_id);
  }

  // Propagate writer-level truncation into the response
  if (writer.isTruncated()) {
    response.truncated = true;
  }

  // Emit completion log
  emitter.emitLog(
    request.run_id,
    response.status === "Success" ? "Info" : "Error",
    `Skill ${request.skill_name} completed: ${response.status} (${response.duration_ms}ms)`,
    {
      status: response.status,
      duration_ms: response.duration_ms,
      events_emitted: emitter.getEventCount(),
    },
  );

  // Send the result
  writer.write({
    type: "InvokeResult",
    message_id: messageId,
    payload: response,
  });
}

/**
 * Handle a Cancel message: abort the running execution for the given run_id.
 */
function handleCancel(_messageId: string, request: CancelRequest): void {
  const context = activeExecutions.get(request.run_id);
  if (!context) {
    log(`Cancel requested for unknown run_id: ${request.run_id}`);
    return;
  }

  log(`Cancelling run ${request.run_id}: ${request.reason}`);
  emitter.emitLog(request.run_id, "Warn", `Cancellation requested: ${request.reason}`);
  context.abortController.abort();
}

/**
 * Handle a Health message: report worker status.
 */
function handleHealth(messageId: string): void {
  const uptimeSecs = Math.floor((Date.now() - startTime) / 1000);

  writer.write({
    type: "HealthResult",
    message_id: messageId,
    payload: {
      healthy: !shuttingDown,
      worker_id: `node-worker-${process.pid}`,
      uptime_secs: uptimeSecs,
    },
  });
}

// =============================================================================
// MESSAGE ROUTING
// =============================================================================

function onMessage(message: TransportMessage): void {
  switch (message.type) {
    case "Invoke":
      // Fire and forget — the response is sent asynchronously
      handleInvoke(message.message_id, message.payload).catch((err) => {
        log(`Unhandled error in invoke handler: ${err}`);
      });
      break;

    case "Cancel":
      handleCancel(message.message_id, message.payload);
      break;

    case "Health":
      handleHealth(message.message_id);
      break;

    default:
      // Ignore outbound message types (InvokeResult, Stream, HealthResult)
      break;
  }
}

function onProtocolError(error: Error, rawLine: string): void {
  log(`Protocol error: ${error.message} — raw: ${rawLine.substring(0, 200)}`);
}

function onStdinClose(): void {
  log("stdin closed — shutting down");
  shutdown();
}

// =============================================================================
// LIFECYCLE
// =============================================================================

/** Log to stderr (stdout is reserved for JSON Lines protocol) */
function log(message: string): void {
  const ts = new Date().toISOString();
  process.stderr.write(`[${ts}] [node-worker] ${message}\n`);
}

/** Graceful shutdown: cancel active executions and exit */
function shutdown(): void {
  if (shuttingDown) return;
  shuttingDown = true;

  log(`Shutting down — ${activeExecutions.size} active executions`);

  // Cancel all active executions
  for (const [runId, context] of activeExecutions) {
    log(`Cancelling active run: ${runId}`);
    context.abortController.abort();
  }

  // Give a brief window for cleanup, then exit
  setTimeout(() => {
    log("Shutdown complete");
    process.exit(0);
  }, 1000);
}

// =============================================================================
// MAIN
// =============================================================================

async function main(): Promise<void> {
  log(`Carnelian Node Worker v${VERSION} starting`);
  log(`PID: ${process.pid}`);
  log(`Node.js: ${process.version}`);
  log(`Skills directory: ${DEFAULT_SKILLS_DIR}`);
  log(`Max output bytes: ${MAX_OUTPUT_BYTES}`);

  // Initialize skill loader
  loader = new SkillLoader(DEFAULT_SKILLS_DIR);
  const issues = await loader.scanAndLoad();

  for (const issue of issues) {
    const prefix = issue.level === "error" ? "ERROR" : "WARN";
    log(`[${prefix}] ${issue.skillName}: ${issue.message}`);
  }

  const skillCount = loader.getSkillCount();
  log(`Loaded ${skillCount} skills`);

  if (skillCount === 0) {
    log(`WARNING: No skills found in ${DEFAULT_SKILLS_DIR}. Check CARNELIAN_SKILLS_DIR or skill registry path.`);
  }

  // Register signal handlers
  process.on("SIGTERM", () => {
    log("Received SIGTERM");
    shutdown();
  });

  process.on("SIGINT", () => {
    log("Received SIGINT");
    shutdown();
  });

  // Catch uncaught exceptions
  process.on("uncaughtException", (err: Error) => {
    log(`Uncaught exception: ${err.message}\n${err.stack}`);
    shutdown();
  });

  process.on("unhandledRejection", (reason: unknown) => {
    log(`Unhandled rejection: ${reason}`);
    // Don't shutdown — just log and continue
  });

  // Start the JSON Lines message loop
  const reader = new JsonLinesReader(onMessage, onProtocolError, onStdinClose);
  reader.start();

  log("Ready — listening for messages on stdin");
}

main().catch((err) => {
  const msg = err instanceof Error ? err.message : String(err);
  process.stderr.write(`[FATAL] ${msg}\n`);
  process.exit(1);
});
