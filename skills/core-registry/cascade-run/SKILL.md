---
name: cascade-run
description: "Communicate with Windsurf Cascade via JSONL channel files (message/delegate/request_help/share_context/status)."
metadata:
  openclaw:
    emoji: "🌊"
  carnelian:
    runtime: node
    version: "1.0.0"
    sandbox:
      network: none
      resourceLimits:
        maxMemoryMB: 64
        maxCpuPercent: 10
        timeoutSecs: 120
    capabilities:
      - fs.read
      - fs.write
---

# cascade-run

Communicate with Windsurf Cascade via JSONL channel files (message/delegate/request_help/share_context/status).

Ported from THUMMIM `cascade-tool.ts`.

## Input

```typescript
{
  action: string;           // Required: action to perform
  text?: string;            // Optional: message text (for message/delegate/request_help/share_context)
  wait?: boolean;           // Optional: wait for response (default true)
}
```

## Supported Actions

| Action | Description | Required Fields |
|--------|-------------|-----------------|
| `message` | Send message to Cascade | `text` |
| `delegate` | Delegate task to Cascade | `text` |
| `request_help` | Request help from Cascade | `text` |
| `share_context` | Share context with Cascade | `text` |
| `status` | Get channel status | - |

## Channel Files

- **Outbound**: `$OPENCLAW_HOME/cascade-channel.jsonl` (messages to Cascade)
- **Inbound**: `$OPENCLAW_HOME/cascade-responses.jsonl` (responses from Cascade)

## Output

**With wait=true (default):**
```typescript
{
  ok: true;
  response: string;         // Response from Cascade
}
```

**With wait=false:**
```typescript
{
  ok: true;
  queued: true;
  messageId: string;        // Message ID for tracking
}
```

**Status action:**
```typescript
{
  pendingMessages: number;
  lastMessage?: string;
  lastResponse?: string;
}
```

## Notes

- **THUMMIM dependency**: Windsurf Cascade must be running with the MCP bridge configured
- Default channel directory: `~/.openclaw` or `%USERPROFILE%/.openclaw`
- `wait=true` polls for up to 120 seconds (2s intervals)
- Uses `crypto.randomUUID()` for message ID generation
- No npm packages needed in the wrapper
- No network access required (sandbox: none)
