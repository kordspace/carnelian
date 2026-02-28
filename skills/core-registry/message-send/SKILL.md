---
name: message-send
description: "Send a message to any configured channel (Discord, Telegram, Slack, WhatsApp, iMessage, etc.) through the Carnelian gateway unified message API"
metadata:
  CARNELIAN:
    emoji: "💬"
    requires:
      env:
        - CARNELIAN_GATEWAY_URL
        - CARNELIAN_GATEWAY_TOKEN
    primaryEnv: CARNELIAN_GATEWAY_TOKEN
  carnelian:
    runtime: node
    version: "1.0.0"
    sandbox:
      network: full
      resourceLimits:
        maxMemoryMB: 128
        maxCpuPercent: 25
        timeoutSecs: 20
      env:
        CARNELIAN_GATEWAY_URL: "${CARNELIAN_GATEWAY_URL}"
        CARNELIAN_GATEWAY_TOKEN: "${CARNELIAN_GATEWAY_TOKEN}"
    capabilities:
      - net.http
---

# message-send

Send a message to any configured channel through the Carnelian gateway unified message API.

## Input

```typescript
{
  action: string;         // Required: action type (e.g. "send", "react", "delete")
  channel?: string;       // Optional: channel type (inferred by gateway if omitted)
  target?: string;        // Optional: recipient (channel ID, JID, phone number, etc.)
  message?: string;       // Optional: message content
  // ... any action-specific fields
}
```

## Output

```typescript
{
  ok: true;
  result: any;    // Gateway response data
}
```

## Supported Channels

- **Discord**: Channel messages, DMs, reactions
- **Telegram**: Chat messages, replies, threads
- **Slack**: Channel messages, DMs, threads, reactions
- **WhatsApp**: Messages, reactions, media
- **iMessage**: Messages (macOS only)
- **SMS**: Text messages (via gateway)

## Notes

- The gateway automatically routes messages based on the `channel` field or infers from `target` format
- Default gateway URL: `http://localhost:18789`
- **Authentication token is required** - set `CARNELIAN_GATEWAY_TOKEN` environment variable
