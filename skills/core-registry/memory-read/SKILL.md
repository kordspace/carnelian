---
name: memory-read
description: "Read memories from persistent storage using the file system"
metadata:
  openclaw:
    emoji: "📖"
  carnelian:
    runtime: node
    version: "1.0.0"
    sandbox:
      network: none
      resourceLimits:
        maxMemoryMB: 64
        maxCpuPercent: 10
        timeoutSecs: 10
    capabilities:
      - fs.read
---

# memory-read

Read memories from persistent storage using the file system.

Ported from THUMMIM `memory-tool.ts`.

## Input

```typescript
{
  path: string;       // Required: file path (relative to memory directory)
  from?: number;      // Optional: 1-indexed line number to start reading from
  lines?: number;     // Optional: number of lines to read
}
```

## Output

```typescript
{
  path: string;   // Absolute path that was read
  text: string;   // File content (full or partial based on from/lines)
}
```

## Notes

- Path is relative to the agent's memory directory (`CARNELIAN_MEMORY_DIR` or current working directory)
- `from` is 1-indexed (first line is 1, not 0)
- If `from` and `lines` are specified, only that range is returned
- No network access required (sandbox network: none)
- Uses `fs.readFile` exposed in the node sandbox

## Example

**Read full file:**
```javascript
{
  "path": "notes/meeting.txt"
}
```

**Read lines 10-20:**
```javascript
{
  "path": "notes/meeting.txt",
  "from": 10,
  "lines": 11
}
```
