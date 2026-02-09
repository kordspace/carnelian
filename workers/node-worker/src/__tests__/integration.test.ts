import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import type { TransportMessage, InvokeResponse, HealthResponse } from "../types.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const WORKER_ENTRY = join(__dirname, "..", "index.js");

/**
 * Spawn the worker process and exchange JSON Lines messages.
 *
 * Sends the given messages to stdin, collects all stdout lines as parsed
 * TransportMessages, and returns them after the worker exits.
 */
function spawnWorker(
  messages: TransportMessage[],
  timeoutMs = 10_000,
): Promise<{ responses: TransportMessage[]; stderr: string; exitCode: number | null }> {
  return new Promise((resolve) => {
    const child = spawn("node", [WORKER_ENTRY], {
      env: {
        ...process.env,
        CARNELIAN_SKILLS_DIR: join(__dirname, "..", "..", "__test_skills__"),
      },
      stdio: ["pipe", "pipe", "pipe"],
    });

    const responses: TransportMessage[] = [];
    let stderr = "";
    let stdoutBuf = "";

    child.stdout!.on("data", (chunk: Buffer) => {
      stdoutBuf += chunk.toString();
      const lines = stdoutBuf.split("\n");
      stdoutBuf = lines.pop()!; // keep incomplete line in buffer
      for (const line of lines) {
        if (line.trim()) {
          try {
            responses.push(JSON.parse(line) as TransportMessage);
          } catch {
            // skip malformed lines
          }
        }
      }
    });

    child.stderr!.on("data", (chunk: Buffer) => {
      stderr += chunk.toString();
    });

    child.on("close", (code: number | null) => {
      resolve({ responses, stderr, exitCode: code });
    });

    // Send messages with a small delay to let the worker initialize
    setTimeout(() => {
      for (const msg of messages) {
        child.stdin!.write(JSON.stringify(msg) + "\n");
      }
      // Close stdin to trigger shutdown
      child.stdin!.end();
    }, 500);

    // Safety timeout
    setTimeout(() => {
      child.kill("SIGTERM");
    }, timeoutMs);
  });
}

describe("Integration: Worker Process", () => {
  it("responds to Health message", async () => {
    const healthMsg: TransportMessage = {
      type: "Health",
      message_id: "health-test-1",
    };

    const { responses, exitCode } = await spawnWorker([healthMsg]);

    const healthResult = responses.find(
      (r) => r.type === "HealthResult" && r.message_id === "health-test-1",
    );
    assert.ok(healthResult, "Should receive HealthResult response");
    if (healthResult && healthResult.type === "HealthResult") {
      assert.equal(healthResult.payload.healthy, true);
      assert.ok(healthResult.payload.worker_id.startsWith("node-worker-"));
      assert.ok(healthResult.payload.uptime_secs >= 0);
    }
  });

  it("returns Failed for unknown skill", async () => {
    const invokeMsg: TransportMessage = {
      type: "Invoke",
      message_id: "invoke-unknown-1",
      payload: {
        run_id: "run-unknown-1",
        skill_name: "nonexistent-skill",
        input: {},
        timeout_secs: 10,
        correlation_id: null,
      },
    };

    const { responses } = await spawnWorker([invokeMsg]);

    const result = responses.find(
      (r) => r.type === "InvokeResult" && r.message_id === "invoke-unknown-1",
    );
    assert.ok(result, "Should receive InvokeResult response");
    if (result && result.type === "InvokeResult") {
      assert.equal(result.payload.status, "Failed");
      assert.ok(result.payload.error?.includes("not found"));
      assert.equal(result.payload.run_id, "run-unknown-1");
    }
  });

  it("handles multiple messages in sequence", async () => {
    const messages: TransportMessage[] = [
      { type: "Health", message_id: "multi-health" },
      {
        type: "Invoke",
        message_id: "multi-invoke",
        payload: {
          run_id: "run-multi",
          skill_name: "missing",
          input: {},
          timeout_secs: 5,
          correlation_id: null,
        },
      },
      { type: "Health", message_id: "multi-health-2" },
    ];

    const { responses } = await spawnWorker(messages);

    const healthResults = responses.filter((r) => r.type === "HealthResult");
    const invokeResults = responses.filter((r) => r.type === "InvokeResult");

    assert.ok(healthResults.length >= 2, "Should receive at least 2 HealthResult responses");
    assert.ok(invokeResults.length >= 1, "Should receive at least 1 InvokeResult response");
  });

  it("shuts down cleanly when stdin closes", async () => {
    const { exitCode, stderr } = await spawnWorker([]);

    assert.equal(exitCode, 0, "Worker should exit with code 0");
    assert.ok(stderr.includes("Shutting down"), "Should log shutdown message");
  });
});
