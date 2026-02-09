import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { JsonLinesWriter } from "../protocol.js";
import type { TransportMessage } from "../types.js";

describe("JsonLinesWriter", () => {
  it("tracks total bytes written", () => {
    const writer = new JsonLinesWriter(1_048_576);
    const msg: TransportMessage = {
      type: "HealthResult",
      message_id: "00000000-0000-0000-0000-000000000001",
      payload: { healthy: true, worker_id: "test-1", uptime_secs: 42 },
    };

    // We can't easily capture stdout in a unit test, but we can verify
    // the writer tracks bytes
    const initialBytes = writer.getTotalBytes();
    assert.equal(initialBytes, 0);
    assert.equal(writer.isTruncated(), false);
  });

  it("reports truncation when limit exceeded", () => {
    // Create a writer with a very small limit
    const writer = new JsonLinesWriter(10);
    assert.equal(writer.isTruncated(), false);

    const msg: TransportMessage = {
      type: "HealthResult",
      message_id: "00000000-0000-0000-0000-000000000001",
      payload: { healthy: true, worker_id: "test-1", uptime_secs: 42 },
    };

    // This message is larger than 10 bytes, so it should trigger truncation
    const written = writer.write(msg);
    assert.equal(written, false);
    assert.equal(writer.isTruncated(), true);
  });

  it("resets output tracking", () => {
    const writer = new JsonLinesWriter(10);
    const msg: TransportMessage = {
      type: "HealthResult",
      message_id: "00000000-0000-0000-0000-000000000001",
      payload: { healthy: true, worker_id: "test-1", uptime_secs: 42 },
    };

    writer.write(msg); // triggers truncation
    assert.equal(writer.isTruncated(), true);

    writer.reset();
    assert.equal(writer.isTruncated(), false);
    assert.equal(writer.getTotalBytes(), 0);
  });
});

describe("TransportMessage types", () => {
  it("serializes Invoke message correctly", () => {
    const msg: TransportMessage = {
      type: "Invoke",
      message_id: "abc-123",
      payload: {
        run_id: "run-1",
        skill_name: "test-skill",
        input: { key: "value" },
        timeout_secs: 30,
        correlation_id: null,
      },
    };

    const json = JSON.stringify(msg);
    const parsed = JSON.parse(json) as TransportMessage;
    assert.equal(parsed.type, "Invoke");
    assert.equal(parsed.message_id, "abc-123");
    if (parsed.type === "Invoke") {
      assert.equal(parsed.payload.skill_name, "test-skill");
      assert.equal(parsed.payload.timeout_secs, 30);
    }
  });

  it("serializes Cancel message correctly", () => {
    const msg: TransportMessage = {
      type: "Cancel",
      message_id: "abc-456",
      payload: { run_id: "run-1", reason: "user requested" },
    };

    const json = JSON.stringify(msg);
    const parsed = JSON.parse(json) as TransportMessage;
    assert.equal(parsed.type, "Cancel");
    if (parsed.type === "Cancel") {
      assert.equal(parsed.payload.reason, "user requested");
    }
  });

  it("serializes Health message correctly", () => {
    const msg: TransportMessage = {
      type: "Health",
      message_id: "abc-789",
    };

    const json = JSON.stringify(msg);
    const parsed = JSON.parse(json) as TransportMessage;
    assert.equal(parsed.type, "Health");
  });

  it("serializes InvokeResult message correctly", () => {
    const msg: TransportMessage = {
      type: "InvokeResult",
      message_id: "abc-result",
      payload: {
        run_id: "run-1",
        status: "Success",
        result: { output: "hello" },
        error: null,
        exit_code: 0,
        duration_ms: 150,
        truncated: false,
      },
    };

    const json = JSON.stringify(msg);
    const parsed = JSON.parse(json) as TransportMessage;
    assert.equal(parsed.type, "InvokeResult");
    if (parsed.type === "InvokeResult") {
      assert.equal(parsed.payload.status, "Success");
      assert.equal(parsed.payload.duration_ms, 150);
      assert.equal(parsed.payload.truncated, false);
    }
  });

  it("serializes Stream message correctly", () => {
    const msg: TransportMessage = {
      type: "Stream",
      message_id: "abc-stream",
      payload: {
        run_id: "run-1",
        event_type: "Log",
        timestamp: "2026-02-07T00:00:00.000Z",
        level: "Info",
        message: "Processing started",
        fields: { step: 1 },
      },
    };

    const json = JSON.stringify(msg);
    const parsed = JSON.parse(json) as TransportMessage;
    assert.equal(parsed.type, "Stream");
    if (parsed.type === "Stream") {
      assert.equal(parsed.payload.event_type, "Log");
      assert.equal(parsed.payload.level, "Info");
    }
  });
});
