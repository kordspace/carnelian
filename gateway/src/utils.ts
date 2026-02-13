// =============================================================================
// STRUCTURED LOGGING
// =============================================================================

type LogLevel = "info" | "warn" | "error" | "debug";

interface LogEntry {
  level: LogLevel;
  ts: string;
  msg: string;
  [key: string]: unknown;
}

/**
 * Emit a structured JSON log line to stdout/stderr.
 *
 * All gateway logging goes through this function so the output format
 * is consistent and machine-parseable.
 */
export function log(level: LogLevel, msg: string, extra?: Record<string, unknown>): void {
  const entry: LogEntry = {
    level,
    ts: new Date().toISOString(),
    msg,
    ...extra,
  };

  const line = JSON.stringify(entry);

  if (level === "error") {
    process.stderr.write(line + "\n");
  } else {
    process.stdout.write(line + "\n");
  }
}

// =============================================================================
// HTTP HELPERS
// =============================================================================

/**
 * Write a JSON response to a `ServerResponse`.
 */
export function sendJson(
  res: import("node:http").ServerResponse,
  status: number,
  body: unknown,
): void {
  const payload = JSON.stringify(body);
  res.writeHead(status, {
    "Content-Type": "application/json",
    "Content-Length": Buffer.byteLength(payload),
  });
  res.end(payload);
}

/**
 * Set headers for a Server-Sent Events stream.
 */
export function setSseHeaders(res: import("node:http").ServerResponse): void {
  res.writeHead(200, {
    "Content-Type": "text/event-stream",
    "Cache-Control": "no-cache",
    Connection: "keep-alive",
    "X-Accel-Buffering": "no",
  });
}

/**
 * Write a single SSE data frame.
 */
export function writeSse(res: import("node:http").ServerResponse, data: unknown): void {
  res.write(`data: ${JSON.stringify(data)}\n\n`);
}

/**
 * Write the SSE termination sentinel and end the response.
 */
export function writeDone(res: import("node:http").ServerResponse): void {
  res.write("data: [DONE]\n\n");
}

// =============================================================================
// REQUEST BODY PARSING
// =============================================================================

/**
 * Read the full request body as a parsed JSON object.
 *
 * Returns `undefined` if the body exceeds `maxBytes` or is not valid JSON,
 * and sends an appropriate error response.
 */
export function readJsonBody(
  req: import("node:http").IncomingMessage,
  res: import("node:http").ServerResponse,
  maxBytes: number = 1024 * 1024,
): Promise<unknown | undefined> {
  return new Promise((resolve) => {
    const chunks: Buffer[] = [];
    let size = 0;

    req.on("data", (chunk: Buffer) => {
      size += chunk.length;
      if (size > maxBytes) {
        sendJson(res, 413, {
          error: { message: "Request body too large", type: "invalid_request_error" },
        });
        req.destroy();
        resolve(undefined);
        return;
      }
      chunks.push(chunk);
    });

    req.on("end", () => {
      const raw = Buffer.concat(chunks).toString("utf-8");
      if (!raw) {
        sendJson(res, 400, {
          error: { message: "Empty request body", type: "invalid_request_error" },
        });
        resolve(undefined);
        return;
      }
      try {
        resolve(JSON.parse(raw));
      } catch {
        sendJson(res, 400, {
          error: { message: "Invalid JSON in request body", type: "invalid_request_error" },
        });
        resolve(undefined);
      }
    });

    req.on("error", () => {
      resolve(undefined);
    });
  });
}

// =============================================================================
// TIMING
// =============================================================================

/** Return a high-resolution millisecond timestamp. */
export function hrTimeMs(): number {
  const [s, ns] = process.hrtime();
  return s * 1000 + ns / 1_000_000;
}
