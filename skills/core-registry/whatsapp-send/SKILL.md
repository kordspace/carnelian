---
name: whatsapp-send
description: "Send a message or reaction via WhatsApp through the Carnelian gateway message API"
metadata:
  CARNELIAN:
    emoji: "📱"
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

# whatsapp-send

Send a message or reaction via WhatsApp through the Carnelian gateway message API.

## Input

```typescript
{
  action?: "send" | "react";   // Optional: action type (default "send")
  to: string;                   // Required: JID or phone number
  content?: string;             // Required for "send": message content
  messageId?: string;           // Required for "react": message ID to react to
  emoji?: string;               // Required for "react": emoji reaction
}
```

## Output

```typescript
{
  ok: true;
  result: any;    // Gateway response data
}
```

## Notes

- **WhatsApp requires a running Carnelian gateway** with a paired WhatsApp account
- JID format: `phone@s.whatsapp.net` (e.g. "1234567890@s.whatsapp.net")
- Phone number format: international format without + (e.g. "1234567890")
- Default gateway URL: `http://localhost:18789`
- **Authentication token is required** - set `CARNELIAN_GATEWAY_TOKEN` environment variable
