---
name: web-fetch
description: "Fetch and extract readable content from a URL; returns markdown or plain text"
metadata:
  CARNELIAN:
    emoji: "🌐"
    requires:
      env:
        - FIRECRAWL_API_KEY
    primaryEnv: FIRECRAWL_API_KEY
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
        FIRECRAWL_API_KEY: "${FIRECRAWL_API_KEY}"
    capabilities:
      - net.http
---

# web-fetch

Fetch and extract readable content from a URL.

Ported from CARNELIAN `web-fetch.ts`.

## Input

```typescript
{
  url: string;                          // Required: URL to fetch
  extractMode?: "markdown" | "text";    // Optional: extraction mode (default "markdown")
  maxChars?: number;                    // Optional: max characters to return (default 50000)
}
```

## Output

```typescript
{
  url: string;              // Original URL
  finalUrl: string;         // Final URL after redirects
  status: number;           // HTTP status code
  contentType: string;      // Content-Type header
  title?: string;           // Page title (when available)
  extractMode: string;      // Extraction mode used
  extractor: string;        // Extractor used ("firecrawl" or "direct")
  truncated: boolean;       // Whether content was truncated
  length: number;           // Content length
  fetchedAt: string;        // ISO timestamp
  tookMs: number;           // Time taken in milliseconds
  text: string;             // Extracted content
}
```

## Notes

- **Firecrawl**: When `FIRECRAWL_API_KEY` is set, uses Firecrawl API for high-quality markdown extraction
- **Direct fetch**: Falls back to direct HTTP fetch with regex-based HTML tag stripping
- **Readability**: Not available in sandbox; HTML is stripped with regex fallback
- **Truncation**: Content is truncated to `maxChars` to prevent memory issues
