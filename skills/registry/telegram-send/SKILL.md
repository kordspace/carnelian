---
name: telegram-send
description: "Send a message to a Telegram chat via the Telegram Bot API"
metadata:
  openclaw:
    emoji: "✈️"
    requires:
      env:
        - TELEGRAM_BOT_TOKEN
    primaryEnv: TELEGRAM_BOT_TOKEN
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
        TELEGRAM_BOT_TOKEN: "${TELEGRAM_BOT_TOKEN}"
    capabilities:
      - net.http
---

# telegram-send

Send a message to a Telegram chat via the Telegram Bot API.

Ported from THUMMIM `telegram-actions.ts`.

## Input

```typescript
{
  to: string;                  // Required: chat ID or username
  content: string;             // Required: message content
  replyToMessageId?: number;   // Optional: message ID to reply to
  messageThreadId?: number;    // Optional: forum topic thread ID
  silent?: boolean;            // Optional: send silently (no notification)
}
```

## Output

```typescript
{
  ok: true;
  messageId: number;    // ID of the sent message
  chatId: number;       // Chat ID where message was sent
}
```

## Notes

- Supports chat IDs (numeric) or usernames (string starting with @)
- Messages are sent with HTML parse mode enabled
- For full action surface (edit, delete, pin, etc.), reference THUMMIM `telegram-actions.ts`
- Bot must have permission to send messages in the target chat
