---
name: cron-schedule
description: "Manage Gateway cron jobs (status/list/add/update/remove/run/runs) and send wake events."
metadata:
  CARNELIAN:
    emoji: "⏰"
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

# cron-schedule

Manage Gateway cron jobs (status/list/add/update/remove/run/runs) and send wake events.

Ported from CARNELIAN `cron-tool.ts`.

## Input

```typescript
{
  action: string;           // Required: action to perform
  job?: object;             // Optional: job definition for add action
  jobId?: string;           // Optional: job ID for update/remove/run/runs actions
  patch?: object;           // Optional: patch object for update action
  text?: string;            // Optional: wake message text
  mode?: string;            // Optional: wake mode
}
```

## Supported Actions

| Action | Description | Required Fields |
|--------|-------------|-----------------|
| `status` | Get cron system status | - |
| `list` | List all cron jobs | - |
| `add` | Add new cron job | `job` |
| `update` | Update existing job | `jobId`, `patch` |
| `remove` | Remove cron job | `jobId` |
| `run` | Trigger job execution | `jobId` |
| `runs` | Get job execution history | `jobId` |
| `wake` | Send wake event | `text` |

## Job Schema

```typescript
{
  schedule: {
    kind: "at" | "every" | "cron";
    value: string;           // ISO timestamp, interval string, or cron expression
  };
  payload: {
    kind: "systemEvent" | "agentTurn";
    // ... kind-specific fields
  };
  sessionTarget: "main" | "isolated";
}
```

### Session Target Constraints

- `main` session → `systemEvent` payload only
- `isolated` session → `agentTurn` payload only

## Output

Returns JSON response from the gateway.

## Notes

- **CARNELIAN dependency**: The CARNELIAN gateway must be running
- Default gateway URL: `ws://127.0.0.1:18789`
- No npm packages needed in the wrapper
- Uses WebSocket JSON-RPC for communication
