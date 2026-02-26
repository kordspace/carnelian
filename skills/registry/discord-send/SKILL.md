---
name: discord-send
description: "Send a message to a Discord channel via the Discord REST API"
metadata:
  openclaw:
    emoji: "🎮"
    requires:
      env:
        - DISCORD_TOKEN
    primaryEnv: DISCORD_TOKEN
  carnelian:
    runtime: node
    version: "1.0.0"
    sandbox:
      network: full
      resourceLimits:
        maxMemoryMB: 128
        maxCpuPercent: 25
        timeoutSecs: 15
      env:
        DISCORD_TOKEN: "${DISCORD_TOKEN}"
    capabilities:
      - net.http
---

# discord-send

Send a message to a Discord channel via the Discord REST API.

Ported from THUMMIM `discord-actions.ts`.

## Input

```typescript
{
  channelId: string;           // Required: Discord channel ID
  content: string;             // Required: message content
  replyToMessageId?: string;   // Optional: message ID to reply to
}
```

## Output

```typescript
{
  ok: true;
  messageId: string;    // ID of the sent message
  channelId: string;    // Channel ID where message was sent
}
```

## Notes

- Only `sendMessage` is implemented in this wrapper
- For guild/moderation actions, use the full THUMMIM tool
- Requires a Discord bot token with appropriate permissions
- Bot must have access to the target channel
