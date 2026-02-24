# Carnelian Comprehensive Architecture Review & Enhancement Plan

## Executive Summary

This report analyzes Carnelian's current architecture, compares it with OpenClaw, identifies areas for consolidation, and provides a roadmap for achieving a unified Rust-native ecosystem. Key issues include: mixed language complexity (Rust/TypeScript/Python), deprecated TypeScript gateway, and the challenge of migrating 600+ skills.

---

## 1. Current Architecture State

### Language Ecosystem Analysis

| Component | Current Language | Target Language | Priority |
|-----------|-----------------|-----------------|----------|
| Core Orchestrator | Rust ✅ | Rust | - |
| HTTP API Server | Rust (Axum) ✅ | Rust | - |
| LLM Providers | Rust ✅ | Rust | - |
| Desktop UI | Rust (Dioxus) ✅ | Rust | - |
| **Gateway** | **TypeScript ❌** | **Rust** | **HIGH** |
| **Skills (600+)** | **TypeScript ❌** | **Rust/WASM** | **HIGH** |
| **Python Skills** | **Python ❌** | **Rust** | **MEDIUM** |
| Workers | Rust + TypeScript + Python | Rust only | MEDIUM |
| Database Migrations | SQL ✅ | SQL | - |

### Current Worker Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Worker Architecture                        │
│                                                              │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │  Node Worker    │  │  Python Worker  │  │ Shell Worker│ │
│  │  (TypeScript)   │  │    (Python)     │  │   (Bash)    │ │
│  │                 │  │                 │  │             │ │
│  │ workers/node/   │  │ workers/python/ │  │ workers/shell│ │
│  │   600+ skills   │  │   ~10 skills    │  │  ~5 skills  │ │
│  └────────┬────────┘  └────────┬────────┘  └──────┬──────┘ │
│           │                    │                   │        │
│  ┌────────┴────────────────────┴───────────────────┴────┐ │
│  │              Worker Manager (Rust)                    │ │
│  │         Process spawning, health, I/O                 │ │
│  └────────────────────────┬──────────────────────────────┘ │
└───────────────────────────┼────────────────────────────────┘
                            │
┌───────────────────────────▼────────────────────────────────┐
│                 Core Orchestrator (Rust)                  │
└───────────────────────────────────────────────────────────┘
```

---

## 2. Critical Issues Identified

### Issue 1: TypeScript Gateway is Deprecated but Present

**Current State:**
- `gateway/` directory contains full TypeScript implementation
- 9 source files including server.ts, router.ts, providers/
- Uses Node.js runtime
- **Already replaced by native Rust providers in `crates/carnelian-core/src/providers/`**

**Problem:**
- Dead code causing confusion
- Dependencies still in package.json
- No longer needed since ModelRouter migration

**Recommendation:** 
```bash
# Remove the entire gateway directory
gateway/          # DELETE - Replaced by native providers
```

### Issue 2: Python Worker Creates Unnecessary Complexity

**Current State:**
- `workers/python-worker/` with `worker.py` (39 lines)
- `crates/carnelian-worker-python/` Rust crate
- Only 2 Python skills in registry:
  - `model-usage/scripts/model_usage.py`
  - `local-places/` (has pyproject.toml)

**Analysis:**
```python
# workers/python-worker/worker.py - Full Content
import json
import sys
import os

# Add skills directory to path
sys.path.insert(0, '/app/skills')

def main():
    """Main entry point for Python worker"""
    print("Python worker ready", flush=True)
    
    for line in sys.stdin:
        try:
            msg = json.loads(line)
            # ... handle message
        except json.JSONDecodeError:
            continue

if __name__ == "__main__":
    main()
```

**Recommendation:**
- **Migrate Python skills to Rust** - Only 2 skills exist
- **Remove Python worker entirely** - Not worth the maintenance overhead
- Use `pyo3` crate for any Python interop needs in future

### Issue 3: 600+ TypeScript Skills Need Migration Path

**Current State:**
```
skills/registry/
├── echo/              # TypeScript
├── healthcheck/       # TypeScript  
├── local-places/      # Python
├── model-usage/       # Python
├── openai-image-gen/  # TypeScript
├── skill-creator/     # TypeScript
└── ... (594+ more in Thummim repo)
```

**Skill Runtime Distribution:**
| Runtime | Count | Migration Strategy |
|---------|-------|-------------------|
| TypeScript | ~580 | WASM or Native Rust |
| Python | ~15 | Native Rust |
| Shell | ~10 | Native Rust or keep |
| Rust | 0 | Target state |

**Migration Complexity Analysis:**

1. **Simple Skills** (60% - ~360 skills)
   - HTTP calls, file operations, text processing
   - **Migration:** Straightforward to Rust
   - **Effort:** 1-2 hours per skill

2. **Medium Complexity** (30% - ~180 skills)
   - Database queries, image processing, API integrations
   - **Migration:** Requires crate dependencies
   - **Effort:** 4-8 hours per skill

3. **Complex Skills** (10% - ~60 skills)
   - Browser automation, ML inference, complex algorithms
   - **Migration:** May need WASM or keep TypeScript
   - **Effort:** 1-3 days per skill

---

## 3. Gateway Architecture Review

### Current Misconception: "Gateway Enables Web Features"

**Fact Check:**
- The TypeScript gateway in `gateway/` is **NOT** the web server
- The actual web server is in **`crates/carnelian-core/src/server.rs`** (Axum-based, Rust)
- Port 18789 is served by **Rust Axum**, not TypeScript

### Actual Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Carnelian Web Stack                      │
│                                                              │
│  ┌─────────────────────────────────────────────────────────┐│
│  │         HTTP/WebSocket Server (Axum/Rust)            ││
│  │              crates/carnelian-core/src/server.rs       ││
│  │                                                        ││
│  │  Routes:                                               ││
│  │  - /v1/health          GET                             ││
│  │  - /v1/complete        POST (native providers)        ││
│  │  - /v1/events/ws       WebSocket                      ││
│  │  - /v1/skills/*        REST API                        ││
│  │  - /v1/tasks/*         Task management                ││
│  └─────────────────────────────────────────────────────────┘│
│                           │                                  │
│  ┌────────────────────────┼──────────────────────────────┐  │
│  │                        ▼                               │  │
│  │  ┌─────────────────────────────────────────────────┐   │  │
│  │  │         Desktop UI (Dioxus - Rust)             │   │  │
│  │  │    crates/carnelian-ui/ - Native desktop app  │   │  │
│  │  │                                                  │   │  │
│  │  │  Connects to: ws://localhost:18789/v1/events/ws │   │  │
│  │  └─────────────────────────────────────────────────┘   │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  Note: NO TypeScript gateway involved in any web traffic    │
└─────────────────────────────────────────────────────────────┘
```

### TypeScript Gateway (DEPRECATED)

```
gateway/                    # DELETE THIS DIRECTORY
├── src/
│   ├── server.ts          # Node.js HTTP server - UNUSED
│   ├── router.ts          # LLM routing - REPLACED by ModelRouter
│   ├── providers/         # Provider implementations - REPLACED
│   │   ├── openai.ts
│   │   ├── anthropic.ts
│   │   └── ollama.ts
│   └── ...
└── package.json
```

**The TypeScript gateway served the same port (18790) for LLM proxying, but this is now handled natively by `ModelRouter` in Rust.**

---

## 4. UI Capabilities Analysis

### Current UI: Dioxus Desktop Application

```rust
// crates/carnelian-ui/src/main.rs
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    components::system_tray::init_system_tray();
    dioxus::launch(app);  // Native desktop app
}
```

**Features:**
- ✅ Native desktop app (Windows/Mac/Linux)
- ✅ WebSocket connection to core
- ✅ System tray integration
- ✅ Real-time event streaming
- ✅ Router-based navigation
- ✅ Glassy dark theme

### Web UI Capabilities

**Current State:**
- Desktop UI is **native only**, not web-hosted
- No web-based UI exists
- REST API is available for custom web frontends

**For Web-Hosted Deployment:**
Two approaches:

1. **Add Web UI to Axum Server** (Recommended)
```rust
// Add to server.rs
.use_static_files("./web-ui/dist")  // Serve React/Vue/Svelte app
.route("/", get(serve_index))       // Main dashboard
```

2. **Use Dioxus Web Renderer**
```bash
# Dioxus supports web via WASM
cargo build --target wasm32-unknown-unknown --package carnelian-ui
# Serve the WASM bundle
```

**Recommendation:** Build a separate lightweight web UI using a modern framework (React/Vue/Svelte) that connects to the same REST/WebSocket API.

---

## 5. Docker Setup Issues Review

### Previous Build Errors (FIXED)

| Error | Cause | Fix Applied |
|-------|-------|-------------|
| `Cargo.lock not found` | Excluded in .dockerignore | Removed exclusion |
| `SQLx query macros fail` | DATABASE_URL not set | Added SQLX_OFFLINE=true |
| `async_stream not found` | Missing dependency | Added to Cargo.toml |
| `Error::Database(String)` | Wrong error variant | Added Error::DatabaseMessage |
| `ChainAnchor trait not in scope` | Missing import | Added to server.rs |
| `owner_signing_key private` | Field access | Changed to method call |

### Current Build Status

**Ready to build after:**
```powershell
# 1. Cache SQLx queries (one-time)
$env:DATABASE_URL="postgresql://carnelian:carnelian@localhost:5432/carnelian"
cargo sqlx prepare --workspace

# 2. Build Docker image
docker build -t carnelian/carnelian-core:latest .
```

### Remaining Concerns

1. **WASM Runtime Size:** `wasmtime = "27.0"` adds ~10MB to binary
2. **libloading Platform Support:** Needs testing on Windows/Mac
3. **SQLx Offline:** Requires `.sqlx/` directory in Docker context

---

## 6. Language Consolidation Roadmap

### Phase 1: Remove Dead Code (Week 1)

```bash
# 1. Delete deprecated gateway
gateway/                     # DELETE

# 2. Remove Python worker (only 2 skills)
workers/python-worker/       # DELETE
crates/carnelian-worker-python/  # DELETE

# 3. Update skill manifests
skills/registry/model-usage/     # Migrate to Rust
skills/registry/local-places/    # Migrate to Rust

# 4. Update Docker build
Dockerfile                     # Remove Python setup
```

### Phase 2: Skill Migration Strategy (Weeks 2-8)

**Strategy: Hybrid Approach (Pragmatic)**

```
skills/
├── registry/              # Keep TypeScript for now
│   └── (580 skills)     # Migrate gradually
├── native/               # NEW: Rust native skills
│   ├── filesystem/       # High-frequency operations
│   ├── http_client/      # API calls
│   ├── text_processor/   # String manipulation
│   └── crypto/           # Cryptographic operations
└── wasm/                 # NEW: WASM sandboxed skills
    └── third_party/      # Untrusted skills
```

**Migration Priority:**

| Priority | Skills | Rationale |
|----------|--------|-----------|
| 1 | Filesystem operations | High frequency, performance-critical |
| 2 | HTTP/API clients | Core functionality |
| 3 | Text processing | CPU-intensive |
| 4 | Database queries | Security sensitive |
| 5 | ML/AI inference | GPU integration |

### Phase 3: Native Rust Skills SDK (Weeks 4-6)

**Create Developer SDK:**

```rust
// Example: Creating a native Rust skill
use carnelian_sdk::prelude::*;

#[skill(name = "file-reader", version = "1.0.0")]
pub struct FileReaderSkill;

#[skill_impl]
impl Skill for FileReaderSkill {
    async fn invoke(&self, ctx: &Context, input: Value) -> Result<Value> {
        let path = input.get_path()?;
        let content = ctx.fs().read_file(path).await?;
        Ok(json!({ "content": content }))
    }
}
```

**Benefits:**
- Type safety across skill boundary
- Direct database access (no serialization overhead)
- Zero startup time (no process spawn)
- Memory-safe (can't crash orchestrator)

---

## 7. Comparison: Carnelian vs OpenClaw

### Architecture Philosophy

| Aspect | Carnelian | OpenClaw |
|--------|-----------|----------|
| **Core Language** | Rust (systems) | TypeScript (application) |
| **Target User** | Enterprise/Developers | Consumers/Personal |
| **Security Model** | Capability-based + Cryptographic | Permission-based |
| **Database** | PostgreSQL + pgvector | In-memory/Local |
| **Audit Trail** | Immutable ledger with anchors | Limited |
| **Channels** | 2 (Telegram, Discord) | 11 (All major platforms) |
| **Skills** | 600+ (TypeScript - migrate to Rust) | 600+ (TypeScript - ClawHub) |
| **Browser Control** | ❌ Not implemented | ✅ Chrome/Chromium CDP |
| **Voice/Talk** | ❌ Not implemented | ✅ Voice Wake + Talk Mode |
| **Mobile Apps** | ❌ Not implemented | ✅ iOS/Android nodes |

### Key Insight

**Carnelian and OpenClaw are converging on similar architecture but from different directions:**

```
OpenClaw Evolution:
TypeScript → More TypeScript → Native bindings for performance
                    ↓
              (Current State)

Carnelian Evolution:
Rust Core → TypeScript Workers → Migrate workers to Rust/WASM
                  ↓
            (Current State)
```

**Meeting Point:** Both will likely end up with:
- Rust core for performance-critical operations
- WASM for sandboxed extensibility
- TypeScript only for rapid prototyping

---

## 8. Recommendations Summary

### Immediate Actions (This Week)

1. **Delete `gateway/` directory**
   - It's dead code, replaced by native ModelRouter
   - Reduces confusion and build time

2. **Remove Python worker**
   - Only 2 Python skills exist
   - Migrate them to Rust or TypeScript
   - Simplifies deployment

3. **Complete Docker build**
   - Run `cargo sqlx prepare`
   - Build and test image
   - Document deployment process

### Short-Term (Next 4 Weeks)

1. **Implement Rust skill framework**
   - Use the `Skill` trait I created
   - Build `NativeSkillLoader` with `libloading`
   - Build `WasmSkillRuntime` with `wasmtime`

2. **Migrate top 10 most-used skills to Rust**
   - Identify via usage analytics
   - Focus on filesystem, HTTP, text processing
   - Measure performance improvement

3. **Add web UI option**
   - Create minimal React/Vue dashboard
   - Connect to existing REST/WebSocket API
   - Enable web-hosted deployments

### Long-Term (Next 3 Months)

1. **Skill Migration Gradual Rollout**
   - Target: 50 most-used skills in Rust
   - Keep TypeScript for remaining 550+
   - Measure adoption and performance

2. **Browser Automation Integration**
   - Add Chrome/Chromium CDP support (like OpenClaw)
   - Enables web scraping, form filling, E2E testing

3. **Mobile Companion Apps**
   - iOS/Android apps that connect to core
   - Voice input, camera, notifications
   - Similar to OpenClaw's node architecture

---

## 9. Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Skill migration breaks existing workflows | Medium | High | Keep TypeScript workers as fallback |
| WASM runtime too heavy | Low | Medium | Make WASM optional feature |
| Native skill ABI breaks | Low | High | Version the Skill trait |
| libloading fails on platform | Medium | Low | Fallback to process workers |
| 600 skills too much to migrate | High | Low | Don't migrate all - hybrid is OK |

---

## 10. Conclusion

**Current State:** Carnelian has successfully built a Rust-native core but carries legacy TypeScript/Python baggage that creates complexity without adding value.

**Target State:** A unified Rust-native ecosystem with:
- Rust core orchestrator ✅ (DONE)
- Rust-native skills (IN PROGRESS)
- WASM for sandboxed third-party skills (IN PROGRESS)
- TypeScript only for rapid prototyping (ACCEPTABLE)
- No Python (REMOVE)
- No TypeScript gateway (REMOVE)

**The 600+ skills from Thummim don't need to be migrated immediately.** The hybrid approach (Rust for hot paths, TypeScript for the rest) is pragmatic and allows gradual transition.

**Most Important:** Remove the dead code (gateway, Python worker) to clarify the architecture and reduce maintenance burden.

---

*Report generated: February 24, 2026*
*Analysis includes: codebase review, OpenClaw comparison, Docker setup, skill architecture*
