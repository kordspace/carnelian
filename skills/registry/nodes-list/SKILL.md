---
name: nodes-list
description: "Manage paired nodes (status/describe/pending/approve/reject/notify/camera/screen/location/run)."
metadata:
  openclaw:
    emoji: "📱"
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
        maxMemoryMB: 128
        maxCpuPercent: 25
        timeoutSecs: 30
      env:
        GATEWAY_URL: "${GATEWAY_URL}"
        GATEWAY_TOKEN: "${GATEWAY_TOKEN}"
    capabilities:
      - net.ws
---

# nodes-list

Manage paired nodes (status/describe/pending/approve/reject/notify/camera/screen/location/run).

Ported from THUMMIM `nodes-tool.ts`.

## Input

```typescript
{
  action: string;           // Required: action to perform
  node?: string;            // Optional: node identifier (nodeId, displayName, or remoteIp)
  requestId?: string;       // Optional: pairing request ID
  title?: string;           // Optional: notification title
  body?: string;            // Optional: notification body
  command?: string[];       // Optional: command array for run action
  // ... additional action-specific fields
}
```

## Supported Actions

| Action | Description | Required Fields |
|--------|-------------|-----------------|
| `status` | List all paired nodes | - |
| `describe` | Get node details | `node` |
| `pending` | List pending pairing requests | - |
| `approve` | Approve pairing request | `requestId` |
| `reject` | Reject pairing request | `requestId` |
| `notify` | Send system notification | `node`, `title`, `body` |
| `camera_snap` | Take camera snapshot | `node` |
| `camera_list` | List available cameras | `node` |
| `camera_clip` | Record camera clip | `node` |
| `screen_record` | Record screen | `node` |
| `location_get` | Get device location | `node` |
| `run` | Execute system command | `node`, `command` |

## Output

Varies by action. Camera/screen actions return base64-encoded media:

```typescript
{
  base64: string;           // Base64-encoded media data
  width?: number;           // Image width (camera_snap)
  height?: number;          // Image height (camera_snap)
  facing?: string;          // Camera facing (camera_snap)
  durationMs?: number;      // Duration (camera_clip, screen_record)
  hasAudio?: boolean;       // Audio flag (camera_clip)
  fps?: number;             // Frame rate (screen_record)
}
```

## Notes

- **THUMMIM dependency**: Gateway and paired node required for camera/screen/location/run actions
- Default gateway URL: `ws://127.0.0.1:18789`
- Media is returned as base64 since the CARNELIAN worker does not have `saveMediaBuffer`
- Node identifier can be `nodeId`, `displayName`, or `remoteIp`
- All `node.invoke` calls include idempotency keys
- No npm packages needed in the wrapper
