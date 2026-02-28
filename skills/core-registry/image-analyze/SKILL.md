---
name: image-analyze
description: "Analyze images using vision models (GPT-4o or Claude Opus)"
metadata:
  CARNELIAN:
    emoji: "🔬"
    requires:
      env:
        - OPENAI_API_KEY
        - ANTHROPIC_API_KEY
    primaryEnv: OPENAI_API_KEY
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
        OPENAI_API_KEY: "${OPENAI_API_KEY}"
        ANTHROPIC_API_KEY: "${ANTHROPIC_API_KEY}"
    capabilities:
      - net.http
---

# image-analyze

Analyze images using vision models (GPT-4o or Claude Opus).

Ported from CARNELIAN `image-tool.ts`.

## Input

```typescript
{
  image: string;           // Required: HTTP(S) URL or data: URL
  prompt?: string;         // Optional: analysis prompt (default "Describe the image.")
  model?: string;          // Optional: model to use (default based on provider)
  provider?: "openai" | "anthropic";  // Optional: provider (default based on available API keys)
}
```

## Output

```typescript
{
  text: string;      // Analysis result
  model: string;     // Model used
  provider: string;  // Provider used
}
```

## Notes

- **Image input must be an HTTP(S) URL or data: URL** - file paths are not supported in the sandbox
- Provider selection: defaults to `openai` if `OPENAI_API_KEY` is set, otherwise `anthropic`
- OpenAI models: `gpt-4o`, `gpt-4o-mini`, `gpt-4-turbo`
- Anthropic models: `claude-opus-4-5`, `claude-sonnet-4`, `claude-3-5-sonnet-20241022`
- For local file analysis, convert to data URL first or upload to a public URL

## Example

```javascript
{
  "image": "https://example.com/photo.jpg",
  "prompt": "What objects are in this image?",
  "provider": "openai"
}
```
