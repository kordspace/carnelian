---
name: session-spawn
description: "Spawn sub-agent sessions via the Gateway sessions_spawn method."
metadata:
  openclaw:
    emoji: "🔀"
    requires:
      env:
        - GATEWAY_URL
        - GATEWAY_TOKEN
    primaryEnv: GATEWAY_URL
  carnelian:
    runtime: node
    version: "1.0.0"
    sandbox:
      network: localhost
      resourceLimits:
        maxMemoryMB: 64
        maxCpuPercent: 10
        timeoutSecs: 15
      env:
        GATEWAY_URL: "${GATEWAY_URL}"
        GATEWAY_TOKEN: "${GATEWAY_TOKEN}"
    capabilities:
      - net.ws
---

# session-spawn

Spawn sub-agent sessions via the Gateway sessions_spawn method.

Ported from THUMMIM `session-spawn-tool.ts`.

## Input

```typescript
{
  task: string;                  // Required: task for the sub-agent
  label?: string;                // Optional: session label
  agentId?: string;              // Optional: agent ID
  model?: string;                // Optional: model to use
  thinking?: boolean;            // Optional: enable thinking mode
  runTimeoutSeconds?: number;    // Optional: run timeout
  cleanup?: "keep" | "delete";   // Optional: cleanup mode
}
```

## Output

```typescript
{
  status: "accepted";
  childSessionKey: string;    // Generated session key for the sub-agent
  runId: string;              // Run ID for the spawned task
}
```

## Notes

- **THUMMIM dependency**: The THUMMIM gateway must be running
- Default gateway URL: `ws://127.0.0.1:18789`
- `sessions_spawn` is forbidden from sub-agent sessions
- Child session key format: `agent:default:subagent:<uuid>`
- If `model` is provided, the skill calls `sessions.patch` to set the model before spawning
- No npm packages needed in the wrapper
- Uses WebSocket JSON-RPC for communication
