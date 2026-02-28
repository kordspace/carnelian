---
name: memory-write
description: "Write memories to persistent storage using the file system"
metadata:
  CARNELIAN:
    emoji: "💾"
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
      - fs.write
---

# memory-write

Write memories to persistent storage using the file system.

Ported from CARNELIAN `memory-tool.ts`.

## Input

```typescript
{
  path: string;       // Required: file path (relative to memory directory)
  content: string;    // Required: content to write
  append?: boolean;   // Optional: append to file instead of overwriting (default false)
}
```

## Output

```typescript
{
  ok: true;
  path: string;           // Absolute path where content was written
  bytesWritten: number;   // Number of bytes written
}
```

## Notes

- Path is relative to the agent's memory directory (`CARNELIAN_MEMORY_DIR` or current working directory)
- Parent directories are created automatically if they don't exist
- Use `append: true` to add content to an existing file without overwriting
- No network access required (sandbox network: none)
- Uses `fs.writeFile` / `fs.appendFile` exposed in the node sandbox

## Example

```javascript
{
  "path": "notes/meeting.txt",
  "content": "Meeting notes from 2026-02-25\n",
  "append": true
}
```
