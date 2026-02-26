---
name: browser-automation
description: "Control the OpenClaw browser control server (status/start/stop/tabs/snapshot/screenshot/navigate/act). Routes to the local browser control HTTP server."
metadata:
  openclaw:
    emoji: "🌐"
    requires:
      bins:
        - curl
      env:
        - OPENCLAW_BROWSER_URL
    primaryEnv: OPENCLAW_BROWSER_URL
  carnelian:
    runtime: shell
    version: "1.0.0"
    sandbox:
      network: localhost
      resourceLimits:
        maxMemoryMB: 128
        maxCpuPercent: 25
        timeoutSecs: 30
    capabilities:
      - net.http
      - exec.shell
---

# browser-automation

Control the OpenClaw browser control server (status/start/stop/tabs/snapshot/screenshot/navigate/act).

Ported from THUMMIM `browser-automation-tool.ts`.

## Input

```typescript
{
  action: string;           // Required: action to perform
  profile?: string;         // Optional: browser profile ("chrome" | "openclaw")
  targetId?: string;        // Optional: target ID for tab operations
  targetUrl?: string;       // Optional: URL for open/navigate operations
  request?: object;         // Optional: request object for act operation
}
```

## Supported Actions

| Action | Description | Required Fields |
|--------|-------------|-----------------|
| `status` | Get server status | - |
| `start` | Start browser | `profile` |
| `stop` | Stop browser | `profile` |
| `profiles` | List available profiles | - |
| `tabs` | List open tabs | `profile` |
| `open` | Open new tab | `targetUrl` |
| `focus` | Focus tab | `targetId` |
| `close` | Close tab | `targetId` |
| `snapshot` | Get AI-formatted snapshot | `profile` |
| `screenshot` | Take screenshot | `targetId` |
| `navigate` | Navigate to URL | `targetId`, `targetUrl` |
| `console` | Get console logs | `targetId` |
| `pdf` | Generate PDF | `targetId` |
| `act` | Perform browser action | `request` |

## Output

Returns JSON response from the browser control server.

## Notes

- **THUMMIM dependency**: The browser control server is part of the OpenClaw/THUMMIM installation
- Default server URL: `http://localhost:3000`
- Playwright is managed by the external browser control server
- The wrapper never imports Playwright directly
- No additional npm packages required
