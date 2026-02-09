/**
 * Event streaming utilities.
 *
 * Provides an EventEmitter class that emits StreamEvent messages via the
 * JSON Lines writer during skill execution. Supports Log, Progress, and
 * Artifact event types.
 */

import { randomUUID } from "node:crypto";
import { stat } from "node:fs/promises";
import { JsonLinesWriter } from "./protocol.js";
import type { EventLevel, StreamEvent, StreamEventType } from "./types.js";

// =============================================================================
// EVENT EMITTER
// =============================================================================

/**
 * Emits streaming events to the Rust transport layer via JSON Lines.
 *
 * Events are written immediately (no buffering) to ensure real-time
 * visibility of skill execution progress.
 */
export class EventEmitter {
  private writer: JsonLinesWriter;
  private eventCount = 0;

  constructor(writer: JsonLinesWriter) {
    this.writer = writer;
  }

  // ---------------------------------------------------------------------------
  // LOG EVENTS
  // ---------------------------------------------------------------------------

  /**
   * Emit a log event.
   *
   * Captures console output, error messages, and diagnostic information
   * from skill execution.
   */
  emitLog(
    runId: string,
    level: EventLevel,
    message: string,
    fields: Record<string, unknown> = {},
  ): void {
    this.emit(runId, "Log", message, { ...fields, level_str: level }, level);
  }

  // ---------------------------------------------------------------------------
  // PROGRESS EVENTS
  // ---------------------------------------------------------------------------

  /**
   * Emit a progress event.
   *
   * Reports execution progress as a percentage with optional stage/step
   * information.
   */
  emitProgress(
    runId: string,
    percentage: number,
    message: string,
    stage?: string,
    step?: string,
  ): void {
    const fields: Record<string, unknown> = {
      percentage: Math.max(0, Math.min(100, percentage)),
    };
    if (stage) fields.stage = stage;
    if (step) fields.step = step;

    this.emit(runId, "Progress", message, fields, "Info");
  }

  // ---------------------------------------------------------------------------
  // ARTIFACT EVENTS
  // ---------------------------------------------------------------------------

  /**
   * Emit an artifact event.
   *
   * Reports a file produced during skill execution. Validates that the
   * file exists and includes metadata (size, type).
   */
  async emitArtifact(
    runId: string,
    filePath: string,
    fileType?: string,
  ): Promise<void> {
    const fields: Record<string, unknown> = {
      path: filePath,
    };

    try {
      const s = await stat(filePath);
      fields.size = s.size;
      fields.type = fileType ?? inferFileType(filePath);
      fields.exists = true;
    } catch {
      fields.exists = false;
    }

    this.emit(
      runId,
      "Artifact",
      `Artifact: ${filePath}`,
      fields,
      "Info",
    );
  }

  // ---------------------------------------------------------------------------
  // CORE EMIT
  // ---------------------------------------------------------------------------

  /** Get the total number of events emitted */
  getEventCount(): number {
    return this.eventCount;
  }

  /** Emit a stream event via the JSON Lines writer */
  private emit(
    runId: string,
    eventType: StreamEventType,
    message: string,
    fields: Record<string, unknown>,
    level: EventLevel | null,
  ): void {
    const event: StreamEvent = {
      run_id: runId,
      event_type: eventType,
      timestamp: new Date().toISOString(),
      level,
      message,
      fields,
    };

    this.writer.write({
      type: "Stream",
      message_id: randomUUID(),
      payload: event,
    });

    this.eventCount++;
  }
}

// =============================================================================
// HELPERS
// =============================================================================

/** Infer file type from extension */
function inferFileType(filePath: string): string {
  const ext = filePath.split(".").pop()?.toLowerCase() ?? "";
  const typeMap: Record<string, string> = {
    png: "image/png",
    jpg: "image/jpeg",
    jpeg: "image/jpeg",
    gif: "image/gif",
    webp: "image/webp",
    svg: "image/svg+xml",
    json: "application/json",
    txt: "text/plain",
    md: "text/markdown",
    csv: "text/csv",
    html: "text/html",
    pdf: "application/pdf",
    zip: "application/zip",
  };
  return typeMap[ext] ?? "application/octet-stream";
}
