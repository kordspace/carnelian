import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { HealthResponse, TransportMessage } from "../types.js";

describe("HealthResponse", () => {
  it("has correct structure", () => {
    const response: HealthResponse = {
      healthy: true,
      worker_id: "node-worker-12345",
      uptime_secs: 120,
    };

    assert.equal(response.healthy, true);
    assert.equal(response.worker_id, "node-worker-12345");
    assert.equal(response.uptime_secs, 120);
  });

  it("serializes as HealthResult transport message", () => {
    const msg: TransportMessage = {
      type: "HealthResult",
      message_id: "health-msg-1",
      payload: {
        healthy: true,
        worker_id: "node-worker-1",
        uptime_secs: 60,
      },
    };

    const json = JSON.stringify(msg);
    const parsed = JSON.parse(json);

    assert.equal(parsed.type, "HealthResult");
    assert.equal(parsed.payload.healthy, true);
    assert.equal(parsed.payload.uptime_secs, 60);
  });

  it("represents unhealthy state during shutdown", () => {
    const response: HealthResponse = {
      healthy: false,
      worker_id: "node-worker-99",
      uptime_secs: 3600,
    };

    assert.equal(response.healthy, false);
  });
});
