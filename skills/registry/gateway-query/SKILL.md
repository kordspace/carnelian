---
name: gateway-query
description: "Query and manage Gateway configuration (restart/config.get/config.schema/config.apply/config.patch/update.run)."
metadata:
  openclaw:
    emoji: "⚙️"
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

# gateway-query

Query and manage Gateway configuration (restart/config.get/config.schema/config.apply/config.patch/update.run).

Ported from THUMMIM `gateway-tool.ts`.

## Input

```typescript
{
  action: string;           // Required: action to perform
  raw?: string;             // Optional: raw config for apply/patch
  baseHash?: string;        // Optional: base hash for apply/patch
  sessionKey?: string;      // Optional: session key
  note?: string;            // Optional: note for config changes
  restartDelayMs?: number;  // Optional: restart delay
  delayMs?: number;         // Optional: delay for restart action
  reason?: string;          // Optional: reason for restart
}
```

## Supported Actions

| Action | Description | Required Fields |
|--------|-------------|-----------------|
| `config.get` | Get current configuration | - |
| `config.schema` | Get configuration schema | - |
| `config.apply` | Apply new configuration | `raw` |
| `config.patch` | Patch configuration | `raw` |
| `update.run` | Run gateway update | - |
| `restart` | Restart gateway | - |

## Output

Returns JSON response from the gateway.

## Notes

- **THUMMIM dependency**: The THUMMIM gateway must be running
- Default gateway URL: `ws://127.0.0.1:18789`
- `restart` requires `commands.restart=true` in the gateway config
- `restart` triggers SIGUSR1 and is fire-and-forget (WebSocket will close)
- `config.apply` and `config.patch` obtain `baseHash` from `config.get` if not provided
- No npm packages needed in the wrapper
- Uses WebSocket JSON-RPC for communication
