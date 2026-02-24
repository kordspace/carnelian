# Carnelian Rust Skill System Architecture

## Overview

Transition from TypeScript/Node.js workers to Rust-native skills while maintaining backward compatibility.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Skill Execution Layer                     │
│                                                              │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │  Rust Skills    │  │  TS/Node Skills │  │  Shell Cmds │ │
│  │  (Native/WASM)  │  │  (Workers)      │  │  (System)   │ │
│  └────────┬────────┘  └────────┬────────┘  └──────┬──────┘ │
│           │                    │                   │        │
│  ┌────────┴────────────────────┴───────────────────┴────┐ │
│  │              Skill Registry & Router                  │ │
│  │         (manifest-based skill discovery)              │ │
│  └────────────────────────┬──────────────────────────────┘ │
└───────────────────────────┼────────────────────────────────┘
                            │
┌───────────────────────────▼────────────────────────────────┐
│                 Core Orchestrator (Rust)                  │
└───────────────────────────────────────────────────────────┘
```

## Skill Types

### 1. Native Rust Skills (New)
```rust
// Compiled as dylib or static linked
#[async_trait]
pub trait Skill: Send + Sync {
    fn manifest(&self) -> &SkillManifest;
    async fn invoke(&self, ctx: &SkillContext, input: Value) -> Result<Value>;
}

// Example: File system operations
pub struct FileSystemSkill;

#[async_trait]
impl Skill for FileSystemSkill {
    fn manifest(&self) -> &SkillManifest {
        &MANIFEST
    }
    
    async fn invoke(&self, ctx: &SkillContext, input: Value) -> Result<Value> {
        // Native Rust implementation
        // No process spawn overhead
        // Memory-safe with zero-copy where possible
    }
}
```

**Pros:**
- Zero startup overhead
- Memory safety
- Direct database access
- Type-safe interfaces

**Cons:**
- Requires Rust knowledge
- Longer compile times
- Potential to crash orchestrator if buggy

### 2. WASM Skills (Sandboxed Rust)
```rust
// Compiled to WASM32-WASI
// Sandboxed with capability-based access
#[no_mangle]
pub extern "C" fn invoke(input_ptr: *mut u8, input_len: usize) -> usize {
    // WASM sandboxed execution
    // Can only access capabilities explicitly granted
}
```

**Pros:**
- Sandboxed (can't crash orchestrator)
- Near-native performance
- Small binary size
- Language-agnostic (Rust, C, TinyGo)

**Cons:**
- WASI limitations
- No direct database access
- More complex toolchain

### 3. TypeScript Workers (Existing - Keep)
```typescript
// Keep existing workers for compatibility
// Spawn Node.js process with JSON-Lines protocol
```

**Keep for:**
- Existing skill ecosystem
- npm package access
- Rapid prototyping
- Third-party integrations

## Recommended Migration Path

### Phase 1: Hybrid System (Current Priority)
1. Keep TypeScript workers working
2. Add Rust skill loader for dylibs
3. Add WASM skill loader for WASI modules
4. Skill registry discovers all types

### Phase 2: Core Skills in Rust
1. Port high-frequency skills to Rust:
   - File system operations
   - String/text processing
   - HTTP requests
   - Database queries
2. Keep complex integrations in TypeScript

### Phase 3: WASM Ecosystem
1. Create WASM skill SDK
2. Sandboxed third-party skills
3. ClawHub-style marketplace

## Implementation Plan

### 1. Skill Trait Definition
```rust
// crates/carnelian-core/src/skills/skill.rs
#[async_trait]
pub trait Skill: Send + Sync {
    /// Get skill manifest
    fn manifest(&self) -> &SkillManifest;
    
    /// Check if skill has required capabilities
    fn check_capabilities(&self, capabilities: &[String]) -> bool;
    
    /// Invoke the skill
    async fn invoke(
        &self, 
        ctx: SkillContext, 
        input: serde_json::Value
    ) -> Result<serde_json::Value>;
    
    /// Health check
    async fn health(&self) -> HealthStatus;
}
```

### 2. Native Skill Loader
```rust
// Load .so/.dll/.dylib files from skills/native/
// Use libloading crate for dynamic loading
pub struct NativeSkillLoader {
    registry: Arc<RwLock<HashMap<String, Box<dyn Skill>>>>,
}

impl NativeSkillLoader {
    pub fn load(&self, path: &Path) -> Result<Box<dyn Skill>> {
        // Use libloading to load dylib
        // Extract Skill trait implementation
        // Register in registry
    }
}
```

### 3. WASM Skill Runtime
```rust
// Use wasmtime for WASM execution
pub struct WasmSkillRuntime {
    engine: wasmtime::Engine,
    linker: wasmtime::Linker<WasmState>,
}

impl WasmSkillRuntime {
    pub fn load(&self, wasm_bytes: &[u8]) -> Result<WasmSkill> {
        // Compile WASM module
        // Set up WASI capabilities
        // Create sandboxed execution context
    }
}
```

### 4. Skill Registry Enhancement
```rust
pub struct SkillRegistry {
    // Existing TS workers
    node_workers: WorkerManager,
    
    // New native skills
    native_skills: NativeSkillLoader,
    
    // New WASM skills  
    wasm_runtime: WasmSkillRuntime,
    
    // Unified routing
    router: SkillRouter,
}

impl SkillRegistry {
    pub async fn discover_skills(&self) -> Result<()> {
        // Scan skills/registry/ - existing TS skills
        // Scan skills/native/ - new Rust dylibs
        // Scan skills/wasm/ - new WASM modules
        // Build unified manifest index
    }
    
    pub async fn invoke(&self, skill_id: &str, input: Value) -> Result<Value> {
        // Route to appropriate runtime based on skill type
    }
}
```

## File Structure

```
skills/
├── registry/              # Existing TypeScript skills
│   ├── echo/
│   ├── healthcheck/
│   └── ...
├── native/               # New Rust dylib skills
│   ├── filesystem/
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   └── SKILL.md
│   ├── http_client/
│   └── ...
├── wasm/                 # New WASM skills
│   ├── text_processor.wasm
│   └── calculator.wasm
└── manifests/            # Unified skill index
    ├── index.json
    └── checksums.json
```

## Skill Manifest (Unified)

```toml
# SKILL.md for all skill types
[skill]
id = "filesystem"
name = "File System Operations"
version = "1.0.0"
type = "native"  # native | wasm | node

[capabilities]
required = ["fs:read", "fs:write"]

[runtime]
# Type-specific config
# native: { lib = "libfilesystem.so" }
# wasm: { module = "filesystem.wasm", max_memory = "64MB" }
# node: { script = "index.js", timeout = 30 }
```

## Security Model

### Native Skills
- Same process (high trust required)
- Capability-checked at invocation
- Code review required for built-in skills

### WASM Skills
- Sandboxed with WASI
- Capability-based access control
- Memory and CPU limits enforced
- Suitable for third-party skills

### Node Workers
- Process isolation (existing security)
- JSON-Lines protocol
- Timeout and resource limits

## Migration Checklist

- [ ] Define Skill trait interface
- [ ] Implement NativeSkillLoader with libloading
- [ ] Implement WasmSkillRuntime with wasmtime
- [ ] Enhance SkillRegistry to support all types
- [ ] Create example native skill (filesystem)
- [ ] Create example WASM skill (calculator)
- [ ] Update skill discovery to scan all directories
- [ ] Add skill type to manifest schema
- [ ] Document skill development SDK
- [ ] Create skill template generator

## Conclusion

**Recommendation:** 
1. **Keep TypeScript workers** for existing skills and rapid development
2. **Add Native Rust skills** for performance-critical core operations
3. **Add WASM skills** for sandboxed third-party ecosystem

This gives Carnelian the best of both worlds:
- Enterprise security with Rust core
- Performance for hot paths
- Ecosystem compatibility
- Future extensibility

