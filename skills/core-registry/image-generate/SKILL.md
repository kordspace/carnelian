---
name: image-generate
description: "Generate images using DALL-E via OpenAI API"
metadata:
  openclaw:
    emoji: "🎨"
    requires:
      env:
        - OPENAI_API_KEY
    primaryEnv: OPENAI_API_KEY
  carnelian:
    runtime: node
    version: "1.0.0"
    sandbox:
      network: full
      resourceLimits:
        maxMemoryMB: 128
        maxCpuPercent: 25
        timeoutSecs: 60
      env:
        OPENAI_API_KEY: "${OPENAI_API_KEY}"
    capabilities:
      - net.http
---

# image-generate

Generate images using DALL-E via OpenAI API.

Ported from THUMMIM `image-tool.ts`.

## Input

```typescript
{
  prompt: string;                    // Required: text description of the image
  model?: string;                    // Optional: model to use (default "dall-e-3")
  size?: string;                     // Optional: image size (default "1024x1024")
  quality?: "standard" | "hd";       // Optional: image quality (default "standard")
  n?: number;                        // Optional: number of images (default 1)
  response_format?: "url" | "b64_json";  // Optional: response format (default "url")
}
```

### Supported Sizes
- DALL-E 3: `1024x1024`, `1024x1792`, `1792x1024`
- DALL-E 2: `256x256`, `512x512`, `1024x1024`

## Output

**URL format:**
```typescript
{
  url: string;              // Image URL (expires after 1 hour)
  revised_prompt?: string;  // DALL-E 3 revised prompt
}
```

**Base64 format:**
```typescript
{
  b64_json: string;         // Base64-encoded image data
  revised_prompt?: string;  // DALL-E 3 revised prompt
}
```

## Notes

- Image generation can take 10-30 seconds depending on model and quality
- DALL-E 3 may revise prompts for safety and quality
- URLs expire after 1 hour; use `b64_json` for persistent storage
- Requires `OPENAI_API_KEY` environment variable
