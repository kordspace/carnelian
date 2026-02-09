import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "../events.js";
import { JsonLinesWriter } from "../protocol.js";

describe("EventEmitter", () => {
  it("tracks event count", () => {
    const writer = new JsonLinesWriter(1_048_576);
    const emitter = new EventEmitter(writer);

    assert.equal(emitter.getEventCount(), 0);

    emitter.emitLog("run-1", "Info", "test message");
    assert.equal(emitter.getEventCount(), 1);

    emitter.emitLog("run-1", "Warn", "warning message");
    assert.equal(emitter.getEventCount(), 2);
  });

  it("emits progress events with clamped percentage", () => {
    const writer = new JsonLinesWriter(1_048_576);
    const emitter = new EventEmitter(writer);

    // Should not throw for out-of-range percentages
    emitter.emitProgress("run-1", -10, "below zero");
    emitter.emitProgress("run-1", 150, "above 100");
    emitter.emitProgress("run-1", 50, "normal", "stage-1", "step-a");

    assert.equal(emitter.getEventCount(), 3);
  });

  it("emits log events at different levels", () => {
    const writer = new JsonLinesWriter(1_048_576);
    const emitter = new EventEmitter(writer);

    emitter.emitLog("run-1", "Trace", "trace msg");
    emitter.emitLog("run-1", "Debug", "debug msg");
    emitter.emitLog("run-1", "Info", "info msg");
    emitter.emitLog("run-1", "Warn", "warn msg");
    emitter.emitLog("run-1", "Error", "error msg");

    assert.equal(emitter.getEventCount(), 5);
  });
});
