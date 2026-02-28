---
name: text-to-speech
description: "Convert text to speech using OpenAI TTS API"
metadata:
  CARNELIAN:
    emoji: "🔊"
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
        maxMemoryMB: 256
        maxCpuPercent: 25
        timeoutSecs: 30
      env:
        OPENAI_API_KEY: "${OPENAI_API_KEY}"
    capabilities:
      - net.http
---

# text-to-speech

Convert text to speech using OpenAI TTS API.

Ported from CARNELIAN `tts-tool.ts`.

## Input

```typescript
{
  text: string;                      // Required: text to convert to speech
  voice?: string;                    // Optional: voice to use (default "alloy")
  model?: string;                    // Optional: model to use (default "tts-1")
  format?: string;                   // Optional: audio format (default "mp3")
}
```

### Available Voices
- `alloy` - Neutral and balanced
- `echo` - Male, clear and expressive
- `fable` - British accent, warm
- `onyx` - Deep male voice
- `nova` - Female, energetic
- `shimmer` - Female, soft and warm

### Available Models
- `tts-1` - Standard quality, faster
- `tts-1-hd` - High definition quality, slower

### Available Formats
- `mp3` - MP3 audio (default)
- `opus` - Opus audio
- `aac` - AAC audio
- `flac` - FLAC audio

## Output

```typescript
{
  audio_base64: string;   // Base64-encoded audio data
  format: string;         // Audio format used
  provider: string;       // Provider used ("openai")
}
```

## Notes

- Audio is returned as base64-encoded data since the sandbox has no file-write path exposed to callers
- To save the audio, decode the base64 string and write to a file outside the sandbox
- Maximum text length: ~4096 characters (model-dependent)
- Requires `OPENAI_API_KEY` environment variable
