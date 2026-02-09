/**
 * JSON Lines protocol implementation for stdin/stdout communication.
 *
 * Provides a reader that parses incoming JSON Lines from stdin and a writer
 * that serializes outgoing messages to stdout, matching the Rust
 * ProcessJsonlTransport protocol.
 */

import { createInterface } from "node:readline";
import type { TransportMessage } from "./types.js";

// =============================================================================
// JSON LINES READER
// =============================================================================

/** Callback for handling parsed transport messages */
export type MessageHandler = (message: TransportMessage) => void;

/** Callback for handling reader errors */
export type ErrorHandler = (error: Error, rawLine: string) => void;

/**
 * Reads JSON Lines from stdin and emits parsed TransportMessage objects.
 *
 * Each line is expected to be a valid JSON object conforming to the
 * TransportMessage discriminated union. Malformed lines are reported
 * via the error handler and skipped.
 */
export class JsonLinesReader {
  private onMessage: MessageHandler;
  private onError: ErrorHandler;
  private onClose: (() => void) | null;
  private closed = false;

  constructor(
    onMessage: MessageHandler,
    onError: ErrorHandler,
    onClose?: () => void,
  ) {
    this.onMessage = onMessage;
    this.onError = onError;
    this.onClose = onClose ?? null;
  }

  /** Start reading from stdin */
  start(): void {
    const rl = createInterface({
      input: process.stdin,
      crlfDelay: Infinity,
    });

    rl.on("line", (line: string) => {
      if (this.closed) return;
      const trimmed = line.trim();
      if (trimmed.length === 0) return;

      try {
        const parsed = JSON.parse(trimmed) as TransportMessage;
        if (!parsed.type) {
          this.onError(
            new Error(`Missing 'type' field in message`),
            trimmed,
          );
          return;
        }
        this.onMessage(parsed);
      } catch (err) {
        this.onError(
          err instanceof Error ? err : new Error(String(err)),
          trimmed,
        );
      }
    });

    rl.on("close", () => {
      this.closed = true;
      this.onClose?.();
    });
  }

  /** Check if the reader has been closed */
  isClosed(): boolean {
    return this.closed;
  }
}

// =============================================================================
// JSON LINES WRITER
// =============================================================================

/** Default maximum output size: 1 MB */
const DEFAULT_MAX_OUTPUT_BYTES = 1_048_576;

/**
 * Writes TransportMessage objects as JSON Lines to stdout.
 *
 * Each message is serialized to a single JSON line followed by a newline.
 * Tracks total output size and supports truncation when limits are exceeded.
 */
export class JsonLinesWriter {
  private totalBytes = 0;
  private maxBytes: number;
  private truncated = false;

  constructor(maxOutputBytes: number = DEFAULT_MAX_OUTPUT_BYTES) {
    this.maxBytes = maxOutputBytes;
  }

  /**
   * Write a transport message to stdout.
   *
   * Returns false if the output limit has been reached and the message
   * was not written.
   */
  write(message: TransportMessage): boolean {
    if (this.truncated) return false;

    const json = JSON.stringify(message);
    const bytes = Buffer.byteLength(json, "utf-8") + 1; // +1 for newline

    if (this.totalBytes + bytes > this.maxBytes) {
      this.truncated = true;
      // Write a final truncation notice as a log stream event
      const notice: TransportMessage = {
        type: "Stream",
        message_id: "00000000-0000-0000-0000-000000000000",
        payload: {
          run_id: "",
          event_type: "Log",
          timestamp: new Date().toISOString(),
          level: "Warn",
          message: `... output truncated at ${this.maxBytes} bytes`,
          fields: {},
        },
      };
      const noticeJson = JSON.stringify(notice);
      process.stdout.write(noticeJson + "\n");
      return false;
    }

    this.totalBytes += bytes;
    process.stdout.write(json + "\n");
    return true;
  }

  /** Get total bytes written so far */
  getTotalBytes(): number {
    return this.totalBytes;
  }

  /** Check if output has been truncated */
  isTruncated(): boolean {
    return this.truncated;
  }

  /** Reset output tracking (e.g., between invocations) */
  reset(): void {
    this.totalBytes = 0;
    this.truncated = false;
  }
}
