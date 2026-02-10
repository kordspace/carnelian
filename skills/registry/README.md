# Carnelian Skills Registry

This directory contains skill manifests for Carnelian OS. Each skill is a subdirectory containing a `skill.json` manifest file.

## Directory Structure

```
skills/registry/
├── healthcheck/
│   └── skill.json
├── echo/
│   └── skill.json
├── local-places/
│   └── skill.json
└── ...
```

## Manifest Format (`skill.json`)

```json
{
  "name": "skill-name",
  "description": "What this skill does",
  "runtime": "node|python|shell|wasm",
  "version": "1.0.0",
  "capabilities_required": ["fs.read", "net.http"],
  "homepage": "https://example.com",
  "sandbox": {
    "mounts": [{"host": "/tmp", "container": "/workspace", "readonly": false}],
    "network": "enabled|disabled|restricted",
    "max_memory_mb": 512,
    "max_cpu_percent": 50,
    "env": {"NODE_ENV": "production"}
  },
  "openclaw_compat": {
    "emoji": "🔧",
    "requires": {"bins": ["curl"]},
    "tags": ["utility"]
  }
}
```

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Unique skill identifier (used as database key) |
| `description` | string | Human-readable description |
| `runtime` | string | Worker runtime: `node`, `python`, `shell`, or `wasm` |

### Optional Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `version` | string | `"0.1.0"` | Semantic version |
| `capabilities_required` | string[] | `[]` | Capability keys needed for execution |
| `homepage` | string | `null` | Documentation URL |
| `sandbox` | object | `null` | Sandbox configuration |
| `openclaw_compat` | object | `null` | OpenClaw compatibility metadata |

## Discovery

Skills are discovered automatically via file watcher (2-second debounce) and on server startup. Manual refresh is also available:

```bash
# CLI
carnelian skills refresh

# REST API
curl -X POST http://localhost:18789/v1/skills/refresh
```

## Checksums

Each manifest is checksummed with blake3 for integrity verification. Skills are only updated in the database when the checksum changes.

## OpenClaw Compatibility

The `openclaw_compat` field provides a migration path from existing Thummim/OpenClaw skills. Carnelian supports all 600+ existing skills via the Node worker.
