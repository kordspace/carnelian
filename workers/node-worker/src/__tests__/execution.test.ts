import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type {
  InvokeRequest,
  InvokeResponse,
  ExecutionContext,
} from "../types.js";

describe("InvokeRequest", () => {
  it("has correct structure", () => {
    const request: InvokeRequest = {
      run_id: "run-abc-123",
      skill_name: "test-skill",
      input: { prompt: "hello" },
      timeout_secs: 30,
      correlation_id: "corr-456",
    };

    assert.equal(request.run_id, "run-abc-123");
    assert.equal(request.skill_name, "test-skill");
    assert.equal(request.timeout_secs, 30);
    assert.equal(request.correlation_id, "corr-456");
  });

  it("allows null correlation_id", () => {
    const request: InvokeRequest = {
      run_id: "run-1",
      skill_name: "test",
      input: {},
      timeout_secs: 60,
      correlation_id: null,
    };

    assert.equal(request.correlation_id, null);
  });
});

describe("InvokeResponse", () => {
  it("represents successful execution", () => {
    const response: InvokeResponse = {
      run_id: "run-1",
      status: "Success",
      result: { output: "done" },
      error: null,
      exit_code: 0,
      duration_ms: 250,
      truncated: false,
    };

    assert.equal(response.status, "Success");
    assert.equal(response.error, null);
    assert.equal(response.exit_code, 0);
    assert.equal(response.truncated, false);
  });

  it("represents failed execution", () => {
    const response: InvokeResponse = {
      run_id: "run-2",
      status: "Failed",
      result: {},
      error: "Script crashed",
      exit_code: 1,
      duration_ms: 100,
      truncated: false,
    };

    assert.equal(response.status, "Failed");
    assert.equal(response.error, "Script crashed");
    assert.equal(response.exit_code, 1);
  });

  it("represents timeout", () => {
    const response: InvokeResponse = {
      run_id: "run-3",
      status: "Timeout",
      result: {},
      error: "Execution timed out after 30s",
      exit_code: null,
      duration_ms: 30000,
      truncated: false,
    };

    assert.equal(response.status, "Timeout");
    assert.equal(response.exit_code, null);
  });

  it("represents cancellation", () => {
    const response: InvokeResponse = {
      run_id: "run-4",
      status: "Cancelled",
      result: {},
      error: "Execution was cancelled",
      exit_code: null,
      duration_ms: 5000,
      truncated: false,
    };

    assert.equal(response.status, "Cancelled");
  });

  it("represents truncated output", () => {
    const response: InvokeResponse = {
      run_id: "run-5",
      status: "Success",
      result: { partial: true },
      error: null,
      exit_code: 0,
      duration_ms: 1500,
      truncated: true,
    };

    assert.equal(response.truncated, true);
  });

  it("round-trips through JSON serialization", () => {
    const original: InvokeResponse = {
      run_id: "run-rt",
      status: "Success",
      result: { nested: { data: [1, 2, 3] } },
      error: null,
      exit_code: 0,
      duration_ms: 42,
      truncated: false,
    };

    const json = JSON.stringify(original);
    const parsed = JSON.parse(json) as InvokeResponse;

    assert.deepEqual(parsed, original);
  });
});

describe("ExecutionContext", () => {
  it("creates valid context for tracking", () => {
    const now = Date.now();
    const ctx: ExecutionContext = {
      runId: "run-ctx-1",
      skillName: "test-skill",
      startTime: now,
      timeoutDeadline: now + 30_000,
      abortController: new AbortController(),
      correlationId: null,
      outputBytes: 0,
      truncated: false,
    };

    assert.equal(ctx.runId, "run-ctx-1");
    assert.equal(ctx.skillName, "test-skill");
    assert.equal(ctx.timeoutDeadline - ctx.startTime, 30_000);
    assert.equal(ctx.abortController.signal.aborted, false);
  });

  it("supports cancellation via AbortController", () => {
    const ctx: ExecutionContext = {
      runId: "run-cancel",
      skillName: "test",
      startTime: Date.now(),
      timeoutDeadline: Date.now() + 60_000,
      abortController: new AbortController(),
      correlationId: null,
      outputBytes: 0,
      truncated: false,
    };

    assert.equal(ctx.abortController.signal.aborted, false);
    ctx.abortController.abort();
    assert.equal(ctx.abortController.signal.aborted, true);
  });
});
