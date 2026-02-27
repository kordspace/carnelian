---
name: openai-image-gen
description: Batch-generate images via OpenAI Images API. Random prompt sampler + `index.html` gallery.
homepage: https://platform.openai.com/docs/api-reference/images
metadata:
  openclaw:
    emoji: "\U0001F5BC"
    requires:
      bins:
        - python3
      env:
        - OPENAI_API_KEY
    primaryEnv: OPENAI_API_KEY
    install:
      - id: python-brew
        kind: brew
        formula: python
        bins:
          - python3
        label: Install Python (brew)
  carnelian:
    runtime: python
    version: "0.1.0"
    sandbox:
      network: full
      resourceLimits:
        maxMemoryMB: 1024
        maxCpuPercent: 50
        timeoutSecs: 600
      env:
        OPENAI_API_KEY: "${OPENAI_API_KEY}"
    capabilities:
      - net.http
      - fs.write
---

# OpenAI Image Gen

Generate a handful of "random but structured" prompts and render them via the OpenAI Images API.

## Run

```bash
python3 {baseDir}/scripts/gen.py
open ~/Projects/tmp/openai-image-gen-*/index.html
```

Useful flags:

```bash
python3 {baseDir}/scripts/gen.py --count 16 --model gpt-image-1
python3 {baseDir}/scripts/gen.py --prompt "ultra-detailed studio photo of a lobster astronaut" --count 4
python3 {baseDir}/scripts/gen.py --size 1536x1024 --quality high --out-dir ./out/images
python3 {baseDir}/scripts/gen.py --model dall-e-3 --quality hd --size 1792x1024 --style vivid
python3 {baseDir}/scripts/gen.py --model dall-e-2 --size 512x512 --count 4
```

## Model-Specific Parameters

### Size

- **GPT image models**: `1024x1024`, `1536x1024`, `1024x1536`, or `auto`
- **dall-e-3**: `1024x1024`, `1792x1024`, or `1024x1792`
- **dall-e-2**: `256x256`, `512x512`, or `1024x1024`

### Quality

- **GPT image models**: `auto`, `high`, `medium`, or `low`
- **dall-e-3**: `hd` or `standard`
- **dall-e-2**: `standard` only

### Other

- **dall-e-3** only supports `n=1`
- **GPT image models** support `--background` and `--output-format`
- **dall-e-3** has `--style`: `vivid` or `natural`

## Output

- Image files (png/jpeg/webp)
- `prompts.json` (prompt to file mapping)
- `index.html` (thumbnail gallery)
