---
name: canvas-render
description: "Render canvas graphics and visualizations via Carnelian gateway node.invoke API"
metadata:
  openclaw:
    emoji: "🖼️"
    requires:
      env:
        - CARNELIAN_GATEWAY_URL
        - CARNELIAN_GATEWAY_TOKEN
    primaryEnv: CARNELIAN_GATEWAY_URL
  carnelian:
    runtime: node
    version: "1.0.0"
    sandbox:
      network: full
      resourceLimits:
        maxMemoryMB: 128
        maxCpuPercent: 25
        timeoutSecs: 30
      env:
        CARNELIAN_GATEWAY_URL: "${CARNELIAN_GATEWAY_URL}"
        CARNELIAN_GATEWAY_TOKEN: "${CARNELIAN_GATEWAY_TOKEN}"
    capabilities:
      - net.http
---

# canvas-render

Render canvas graphics and visualizations via Carnelian gateway node.invoke API.

Ported from THUMMIM `canvas-tool.ts`.

## Input

```typescript
{
  action: "present" | "hide" | "navigate" | "eval" | "snapshot" | "a2ui_push" | "a2ui_reset";
  gatewayUrl?: string;        // Optional: gateway URL (default from env)
  gatewayToken?: string;      // Optional: gateway token (default from env)
  node?: string;              // Optional: node ID for routing
  target?: string;            // Optional: target selector
  x?: number;                 // Optional: X coordinate
  y?: number;                 // Optional: Y coordinate
  width?: number;             // Optional: width
  height?: number;            // Optional: height
  url?: string;               // Optional: URL to navigate to
  javaScript?: string;        // Optional: JavaScript to evaluate
  outputFormat?: string;      // Optional: snapshot format (png, jpeg, webp)
  maxWidth?: number;          // Optional: max snapshot width
  quality?: number;           // Optional: snapshot quality (0-100)
  delayMs?: number;           // Optional: delay before snapshot
  jsonl?: string;             // Optional: JSONL data for a2ui_push
}
```

## Output

Varies by action:

**present/hide/navigate/eval/a2ui_push/a2ui_reset:**
```typescript
{
  ok: true;
  result: any;  // Gateway response
}
```

**snapshot:**
```typescript
{
  base64: string;   // Base64-encoded image data
  format: string;   // Image format (png, jpeg, webp)
}
```

## Actions

- **present**: Show canvas at specified position
- **hide**: Hide canvas
- **navigate**: Navigate to URL
- **eval**: Evaluate JavaScript in canvas context
- **snapshot**: Capture canvas screenshot as base64
- **a2ui_push**: Push JSONL data to canvas
- **a2ui_reset**: Reset canvas state

## Notes

- Requires a running Carnelian gateway with canvas/node support
- Default gateway URL: `http://localhost:18790`
- Snapshots are returned as base64 since fs write is not exposed to callers
- For full action surface, reference THUMMIM `canvas-tool.ts`
