---
name: slack-send
description: "Send a message to a Slack channel or DM via the Slack Web API"
metadata:
  CARNELIAN:
    emoji: "💼"
    requires:
      env:
        - SLACK_BOT_TOKEN
    primaryEnv: SLACK_BOT_TOKEN
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
        SLACK_BOT_TOKEN: "${SLACK_BOT_TOKEN}"
    capabilities:
      - net.http
---

# slack-send

Send a message to a Slack channel or DM via the Slack Web API.

Ported from CARNELIAN `slack-actions.ts`.

## Input

```typescript
{
  to: string;           // Required: channel ID or name (e.g. "#general" or "C1234567890")
  content: string;      // Required: message content
  threadTs?: string;    // Optional: thread timestamp to reply in thread
  mediaUrl?: string;    // Optional: media URL to attach
}
```

## Output

```typescript
{
  ok: true;
  ts: string;       // Message timestamp (unique ID)
  channel: string;  // Channel ID where message was sent
}
```

## Notes

- Supports channel IDs (e.g. "C1234567890") or channel names (e.g. "#general")
- For full action surface (edit, delete, reactions, etc.), reference CARNELIAN `slack-actions.ts`
- Bot must have `chat:write` scope and access to the target channel
- Media attachments are added via the `attachments` field
