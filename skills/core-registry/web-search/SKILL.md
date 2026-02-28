---
name: web-search
description: "Search the web via Brave Search API or Perplexity Sonar; returns titles, URLs, and snippets (Brave) or an AI-synthesised answer with citations (Perplexity)"
metadata:
  CARNELIAN:
    emoji: "🔍"
    requires:
      env:
        - BRAVE_API_KEY
        - PERPLEXITY_API_KEY
        - OPENROUTER_API_KEY
    primaryEnv: BRAVE_API_KEY
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
        BRAVE_API_KEY: "${BRAVE_API_KEY}"
        PERPLEXITY_API_KEY: "${PERPLEXITY_API_KEY}"
        OPENROUTER_API_KEY: "${OPENROUTER_API_KEY}"
    capabilities:
      - net.http
---

# web-search

Search the web using Brave Search API or Perplexity Sonar.

Ported from CARNELIAN `web-search.ts`.

## Input

```typescript
{
  query: string;           // Required: search query
  count?: number;          // Optional: number of results (default 5, max 10)
  country?: string;        // Optional: country code (e.g. "us", "uk")
  search_lang?: string;    // Optional: search language (e.g. "en")
  ui_lang?: string;        // Optional: UI language (e.g. "en-US")
  freshness?: string;      // Optional: Brave freshness filter ("pd", "pw", "pm", "py", "YYYY-MM-DDtoYYYY-MM-DD")
  provider?: "brave" | "perplexity";  // Optional: search provider (default "brave")
}
```

### Brave Freshness Values
- `"pd"` - Past day
- `"pw"` - Past week
- `"pm"` - Past month
- `"py"` - Past year
- `"YYYY-MM-DDtoYYYY-MM-DD"` - Custom date range

### Environment Variable Resolution Order

**Brave provider:**
- `BRAVE_API_KEY` (required)

**Perplexity provider:**
- `PERPLEXITY_API_KEY` → `OPENROUTER_API_KEY` (tries in order)

## Output

**Brave:**
```typescript
{
  query: string;
  provider: "brave";
  count: number;
  tookMs: number;
  results: Array<{
    title: string;
    url: string;
    description: string;
    published?: string;
    siteName?: string;
  }>;
}
```

**Perplexity:**
```typescript
{
  query: string;
  provider: "perplexity";
  model: string;
  tookMs: number;
  content: string;
  citations: string[];
}
```
