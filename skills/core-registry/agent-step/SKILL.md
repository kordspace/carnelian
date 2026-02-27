---
name: agent-step
description: "Execute a single agent step with custom system prompt and wait for completion."
metadata:
  openclaw:
    emoji: "🤖"
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
        timeoutSecs: 60
      env:
        GATEWAY_URL: "${GATEWAY_URL}"
        GATEWAY_TOKEN: "${GATEWAY_TOKEN}"
    capabilities:
      - net.ws
---

# agent-step

Execute a single agent step with custom system prompt and wait for completion.

Ported from THUMMIM `agent-step-tool.ts`.

## Input

```typescript
{
  sessionKey: string;           // Required: session key
  message: string;              // Required: message to send
  extraSystemPrompt: string;    // Required: extra system prompt
  timeoutMs?: number;           // Optional: timeout in milliseconds (default 30000, max 60000)
  channel?: string;             // Optional: channel (default "internal")
  lane?: string;                // Optional: lane (default "nested")
}
```

## Output

```typescript
{
  ok: true;
  reply: string;                // Assistant's reply text
}
```

Or on failure:

```typescript
{
  ok: false;
  status: string;               // Wait status (e.g. "timeout", "error")
}
```

## Notes

- **THUMMIM dependency**: The THUMMIM gateway must be running
- Default gateway URL: `ws://127.0.0.1:18789`
- Calls `agent` method, then polls `agent.wait` for completion
- Reads `chat.history` to extract the latest assistant reply
- Filters out tool messages from the response
- Maximum timeout: 60 seconds
- No npm packages needed in the wrapper
- Uses WebSocket JSON-RPC for communication
