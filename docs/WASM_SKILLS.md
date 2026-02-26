# How to Write a Carnelian WASM Skill

WASM skills run inside `WasmSkillRuntime` (wasmtime 27 + WASI P1). They are sandboxed by default and are the recommended target for new skills when you need deterministic, portable execution with strong isolation guarantees.

## Skill Directory Structure

WASM skills follow the same registry layout as other runtimes:

```
skills/registry/<skill-name>/
├── skill.json          # manifest
└── <skill-name>.wasm   # compiled binary
```

Example: `skills/registry/hello-wasm/`

## `skill.json` Manifest

Every WASM skill requires a manifest that describes its identity and capabilities. **Note: Currently, only `capabilities_required` is read by the runtime.** The `sandbox` fields are not consumed (see notes below).

| Field | Type | Description |
|---|---|---|
| `name` | string | Unique skill identifier (must match directory name) |
| `description` | string | Human-readable description |
| `runtime` | `"wasm"` | Must be exactly `"wasm"` for the WASM runtime |
| `version` | string | SemVer (e.g., `"1.0.0"`) |
| `capabilities_required` | string[] | Array of capability strings — **only field consumed by runtime** |
| `sandbox.network` | `"disabled"` \| `"enabled"` | **Not consumed by runtime** — informational only |
| `sandbox.max_memory_mb` | number | **Not consumed by runtime** — memory limits not yet enforced |
| `sandbox.max_cpu_percent` | number | **Not consumed by runtime** — informational only |

**Example `skill.json` (from `hello-wasm`):**

```json
{
  "name": "hello-wasm",
  "description": "A simple WASM skill that echoes its input",
  "runtime": "wasm",
  "version": "1.0.0",
  "capabilities_required": []
}
```

**Note:** The `sandbox` configuration shown in the original `hello-wasm/skill.json` is not consumed by the runtime. Only `capabilities_required` determines sandbox permissions.

## Writing a Rust WASM Skill

### I/O Contract

The runtime communicates with your WASM module via stdin/stdout:

1. **Input**: The runtime writes a serialized `SkillInput.data` JSON object to the module's stdin pipe
2. **Output**: Your module must write a JSON value to stdout
3. **Result**: The runtime captures stdout via `MemoryOutputPipe` and parses it back as the skill's result

### Entry Points

The runtime tries entry points in order:

1. **`invoke`** (preferred) — Call this if exported
2. **`_start`** — Fallback if `invoke` is not found

Reference: `wasm_runtime.rs` lines 234–238

### Minimal Example

```rust
// src/main.rs
use std::io::{self, Read, Write};

fn main() {
    // Read input from stdin
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    
    // Parse the JSON input (it's the SkillInput.data field)
    let input_json: serde_json::Value = serde_json::from_str(&input).unwrap();
    
    // Process and produce output
    let output = serde_json::json!({
        "echo": input_json,
        "processed": true
    });
    
    // Write result to stdout
    io::stdout().write_all(output.to_string().as_bytes()).unwrap();
}
```

### Cargo.toml Setup

```toml
[package]
name = "my-wasm-skill"
version = "1.0.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[profile.release]
opt-level = 3
lto = true
strip = true
```

## Compiling to WASM

1. **Add the WASI target:**
   ```bash
   rustup target add wasm32-wasip1
   ```

2. **Build the skill:**
   ```bash
   cargo build --target wasm32-wasip1 --release
   ```

3. **Copy the output to the skill directory:**
   ```bash
   cp target/wasm32-wasip1/release/my-wasm-skill.wasm \
      skills/registry/my-wasm-skill/my-wasm-skill.wasm
   ```

## Capability Grants

The runtime currently honours **only** `capabilities_required` from `skill.json`. The following capability strings are recognized:

| Capability string | Effect |
|---|---|
| `"fs.read"` | Preopens `.` with read-only `DirPerms::READ` / `FilePerms::READ` |
| `"network"` | Calls `inherit_network()` on the WASI context |

**Important:** The runtime does **not** read `sandbox.network` or any other `sandbox.*` fields. Network access is enabled **only** when `"network"` is present in `capabilities_required`. Filesystem access is enabled **only** when `"fs.read"` is present.

All other capabilities are silently ignored (deny-by-default).

Add capabilities to your `skill.json`:

```json
{
  "capabilities_required": ["fs.read", "network"]
}
```

## Resource Limits

### Output Size

Controlled by the `CARNELIAN_MAX_OUTPUT_BYTES` environment variable (default 1 MB).

- When `stdout_pipe.contents().len() >= max_output_bytes`, output is truncated
- `SkillOutput.metadata` will contain `{ "truncated": "true" }` when this occurs

### Timeout

Epoch-based timeout enforcement:

- **Default**: 30 seconds (`default_timeout_secs`)
- **Mechanism**: A background Tokio task increments the epoch every second
- **Deadline**: The store's deadline is set to `timeout_secs` epochs
- **Interruption**: Long-running operations are interrupted when the epoch deadline is reached

### Memory

- **Default pages**: `default_max_memory_pages = 1024` (64 MB) stored on `WasmSkill` struct
- **Enforcement**: **Not currently enforced** — the `max_memory_pages` field exists in the runtime code but is not applied to the WASM linear memory limit

**Note:** The `sandbox.max_memory_mb` field in `skill.json` is **not consumed** by the runtime. Memory limits are not yet implemented despite the field existing in code.

Override in `skill.json` (informational only — not enforced):

```json
{
  "sandbox": {
    "max_memory_mb": 128
  }
}
```

## Reference Skill

The canonical smoke-test WASM skill is at `skills/registry/hello-wasm/`:

- **No capabilities required** — runs fully sandboxed with no filesystem or network access
- **Network disabled** — because `"network"` is not in `capabilities_required`, not because of any `sandbox` field
- **Simple echo** — returns its input unchanged

**Note:** The original `hello-wasm/skill.json` contains a `sandbox` section, but these fields are **not read by the runtime**. Network is disabled by default (deny-by-default) because `capabilities_required` is empty.

Use it as a template for new WASM skills:

```bash
# Copy and modify
cp -r skills/registry/hello-wasm skills/registry/my-skill
# Edit skill.json and replace the .wasm file
```

## Discovery

The `WasmSkillRuntime::discover_and_load` method scans a directory for `*.wasm` files and loads them automatically. When you place a skill in `skills/registry/<name>/`, it becomes available for invocation.

The `WorkerManager::spawn_worker` routes `WorkerRuntime::Wasm` to this runtime, creating an in-process WASM worker that can execute any discovered WASM skill.

## Runtime Implementation

The WASM runtime is implemented in:

- **`carnelian-core/src/skills/wasm_runtime.rs`** — Main runtime with epoch timeout, capability grants, and WASI context setup

Key features:
- wasmtime 27 with WASI P1 support
- Epoch-based timeout for deterministic interruption
- Capability-gated filesystem and network access
- Memory limits via linear memory configuration
- Automatic skill discovery and loading
