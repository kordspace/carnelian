# Carnelian vs OpenClaw - Architecture Comparison

## Executive Summary

Both Carnelian and OpenClaw are autonomous AI agent platforms with similar high-level goals but different architectural approaches. **Carnelian is a Rust-native, capability-secure orchestrator** designed for robust enterprise use, while **OpenClaw is a TypeScript-based personal assistant** optimized for consumer convenience across multiple platforms.

---

## Architecture Overview

| Aspect | Carnelian | OpenClaw |
|--------|-----------|----------|
| **Core Language** | Rust (memory-safe, zero-cost abstractions) | TypeScript/JavaScript (Node.js runtime) |
| **Primary Use Case** | Enterprise orchestration with audit trails | Personal AI assistant across devices |
| **Gateway/Control Plane** | HTTP REST + WebSocket on port 18789 | WebSocket Gateway on port 18789 (same!) |
| **Database** | PostgreSQL + pgvector (vector embeddings) | Not specified (likely in-memory or local storage) |
| **Memory System** | Structured topics with capability-based access | Canvas + session-based memory |
| **Security Model** | Capability-based security + Ed25519 signatures | DM access controls + password auth |

---

## Feature Comparison

### Channels / Adapters

| Channel | Carnelian | OpenClaw |
|---------|-----------|----------|
| Telegram | ✅ Native Rust adapter | ✅ grammY-based |
| Discord | ✅ Native Rust adapter | ✅ discord.js |
| WhatsApp | ❌ Not implemented | ✅ Baileys |
| Slack | ❌ Not implemented | ✅ Bolt |
| Signal | ❌ Not implemented | ✅ signal-cli |
| iMessage | ❌ Not implemented | ✅ BlueBubbles/legacy |
| Google Chat | ❌ Not implemented | ✅ Chat API |
| Microsoft Teams | ❌ Not implemented | ✅ Extension |
| WebChat | ✅ Part of carnelian-ui | ✅ Built-in WebChat |

**Analysis**: OpenClaw has significantly broader channel support, targeting consumer messaging platforms. Carnelian focuses on developer/enterprise channels.

### LLM Integration

| Feature | Carnelian | OpenClaw |
|---------|-----------|----------|
| Local LLM (Ollama) | ✅ Native provider | ✅ Supported |
| OpenAI | ✅ Native provider | ✅ Supported |
| Anthropic | ✅ Native provider | ✅ Supported |
| Fireworks AI | ✅ Native provider | Unknown |
| Model Failover | ✅ Built-in | ✅ Supported |
| Usage Tracking | ✅ Cost estimation | ✅ Usage tracking |
| Streaming | ✅ Native streaming | ✅ Streaming/chunking |

**Analysis**: Both have comprehensive LLM support. Carnelian has native Rust providers for direct integration; OpenClaw likely uses gateway pattern or Node.js clients.

### Runtime & Execution

| Feature | Carnelian | OpenClaw |
|---------|-----------|----------|
| Skill System | TypeScript/Node.js workers | ClawHub skills platform |
| Browser Automation | ❌ Not implemented | ✅ Chrome/Chromium CDP control |
| Cron/Scheduled Tasks | ❌ Not implemented | ✅ Cron + wakeups |
| Webhooks | ✅ Supported | ✅ Webhooks |
| Voice/Talk Mode | ❌ Not implemented | ✅ Voice Wake + Talk Mode |
| Canvas/A2UI | ❌ Not implemented | ✅ Visual workspace |
| Device Nodes | ❌ Not implemented | ✅ iOS/Android/macOS nodes |
| Camera/Screen Recording | ❌ Not implemented | ✅ Node capabilities |

**Analysis**: OpenClaw is far ahead in consumer-oriented features like voice, canvas, and device integration. Carnelian focuses on backend orchestration.

### Security & Audit

| Feature | Carnelian | OpenClaw |
|---------|-----------|----------|
| Audit Trail | ✅ Immutable ledger with anchors | Unknown |
| Capability Security | ✅ Fine-grained capability grants | DM access controls |
| Cryptographic Signatures | ✅ Ed25519 owner keypair | Unknown |
| Cross-Instance Sync | ✅ Revoked grants sync | Unknown |
| Safe Mode | ✅ Safe mode with approval queue | Security defaults |
| Sub-Agents | ✅ Sub-agent management | Agent-to-agent sessions |

**Analysis**: Carnelian has a more sophisticated security model designed for enterprise use with cryptographic verification. OpenClaw focuses on personal use security.

---

## Technical Architecture Deep Dive

### Carnelian Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Carnelian OS (Rust)                       │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │ HTTP Server │  │   Scheduler  │  │  Event Stream    │   │
│  │   (Axum)    │  │              │  │  (Broadcast)     │   │
│  └──────┬──────┘  └──────────────┘  └──────────────────┘   │
│         │                                                    │
│  ┌──────┴──────────────────────────────────────────┐      │
│  │              Core Orchestrator                    │      │
│  │  ┌─────────┐ ┌──────────┐ ┌─────────┐ ┌────────┐│      │
│  │  │  Ledger │ │ Memory   │ │ Policy  │ │ Skills ││      │
│  │  │ (Audit) │ │ Manager  │ │ Engine  │ │Registry││      │
│  │  └─────────┘ └──────────┘ └─────────┘ └────────┘│      │
│  └───────────────────────────────────────────────────┘      │
│         │                                                    │
│  ┌──────┴─────────────────────────────────────────────┐    │
│  │              Provider Registry (Rust-native)         │    │
│  │  ┌─────────┐ ┌──────────┐ ┌───────────┐ ┌────────┐  │    │
│  │  │ Ollama  │ │  OpenAI  │ │ Anthropic │ │Fireworks│  │    │
│  │  │Provider │ │ Provider│ │ Provider  │ │Provider│  │    │
│  │  └─────────┘ └──────────┘ └───────────┘ └────────┘  │    │
│  └───────────────────────────────────────────────────────┘    │
│         │                                                    │
│  ┌──────┴──────────┐  ┌──────────────────┐                  │
│  │  Telegram Bot   │  │   Discord Bot    │                  │
│  └──────────────────┘  └──────────────────┘                  │
└─────────────────────────────────────────────────────────────┘
         │                              │
    ┌────┴────┐                    ┌────┴────┐
    │PostgreSQL│                    │  LLM    │
│+ pgvector │                    │Services │
    └─────────┘                    └─────────┘
```

### OpenClaw Architecture

```
WhatsApp / Telegram / Slack / Discord / Google Chat / Signal / iMessage / BlueBubbles / Microsoft Teams / Matrix / Zalo / WebChat
                                    │
                                    ▼
                    ┌───────────────────────────────┐
                    │          Gateway              │
                    │    (control plane)            │
                    │      ws://127.0.0.1:18789     │
                    └──────────────┬────────────────┘
                                   │
        ┌──────────────────────────┼──────────────────────────┐
        │                          │                          │
┌───────▼──────┐        ┌──────────▼──────────┐      ┌────────▼──────┐
│  Pi Agent    │        │  CLI (openclaw ...) │      │  WebChat UI   │
│  (RPC mode)  │        │                     │      │               │
└──────────────┘        └─────────────────────┘      └───────────────┘
        │
┌───────┴──────────┐
│  Browser Control │  Chrome/Chromium with CDP
│  (Canvas/A2UI)   │
└──────────────────┘
```

---

## Strengths & Weaknesses

### Carnelian Strengths

1. **Memory Safety**: Rust eliminates entire classes of runtime errors
2. **Performance**: Native compilation, no GC pauses
3. **Security**: Capability-based model with cryptographic verification
4. **Auditability**: Immutable ledger with chain anchoring
5. **Type Safety**: Strong typing across the entire codebase
6. **Database Integration**: PostgreSQL with vector search for semantic memory

### Carnelian Weaknesses

1. **Channel Coverage**: Limited to Telegram/Discord
2. **Consumer Features**: No voice, canvas, or device integration
3. **Skill Ecosystem**: Smaller ecosystem than OpenClaw's ClawHub
4. **Browser Automation**: No built-in browser control
5. **Mobile Support**: No iOS/Android companion apps

### OpenClaw Strengths

1. **Platform Coverage**: Every major messaging platform
2. **Consumer Features**: Voice, canvas, camera, screen recording
3. **Browser Control**: Full Chrome/Chromium automation
4. **Mobile Apps**: iOS/Android nodes for device integration
5. **Ease of Use**: Simple installation, works on any OS
6. **Skills Ecosystem**: ClawHub with install gating

### OpenClaw Weaknesses

1. **Type Safety**: TypeScript has runtime type holes
2. **Performance**: Node.js GC and event loop limitations
3. **Memory Safety**: JavaScript's memory model is less strict
4. **Audit Trail**: Less sophisticated than Carnelian's ledger
5. **Enterprise Security**: No capability-based security model

---

## Skill System Comparison

### Carnelian Skills

**Current State**: TypeScript/Node.js workers with SKILL.md manifests

```typescript
// Carnelian skill structure
skills/registry/
  echo/
    SKILL.md          // Manifest with capabilities
    index.ts         // TypeScript implementation
  healthcheck/
    SKILL.md
    index.ts
```

**Execution Model**: Workers spawn Node.js processes for sandboxed execution

**Pros**: Sandboxed, language-agnostic workers (Node, Python, Shell)
**Cons**: Process overhead, requires Node.js runtime

### OpenClaw Skills

**Current State**: ClawHub skills platform with bundled/managed/workspace skills

**Execution Model**: Unknown (likely direct function calls or containerized)

**Pros**: Install gating, UI integration, larger ecosystem
**Cons**: Unknown execution isolation

---

## Migration Path: TypeScript to Rust Skills

To achieve the "Carnelian skill book index" with pure Rust skills:

### Recommended Approach

1. **Keep TypeScript Workers** for existing skills (backward compatibility)
2. **Add Rust-native skills** using WASM or dylib loading
3. **Create a Rust skill SDK** for native skill development

### Rust Skill Architecture

```rust
// Example Rust skill trait
#[async_trait]
pub trait Skill: Send + Sync {
    fn manifest(&self) -> &SkillManifest;
    async fn invoke(&self, ctx: &SkillContext, input: Value) -> Result<Value>;
}

// Native Rust skills compiled as dylibs
// Loaded dynamically at runtime
// Sandboxed using WASI or process isolation
```

### Benefits of Rust Skills

1. **Performance**: No process spawn overhead
2. **Safety**: Memory safety without GC
3. **Type Safety**: Compile-time verification
4. **Binary Size**: Smaller than Node.js + dependencies
5. **Startup Time**: Near-instant vs Node.js boot

---

## Recommendations

### For Carnelian

1. **Keep TypeScript Workers** for the foreseeable future
2. **Add Rust-native skill support** as an optimization path
3. **Focus on differentiators**: Security, audit, database integration
4. **Add browser automation** to match OpenClaw's capabilities
5. **Consider voice/Talk Mode** integration

### For OpenClaw Users Considering Carnelian

1. **Choose Carnelian if**: You need enterprise security, audit trails, database-backed memory
2. **Stay with OpenClaw if**: You need consumer features, voice, mobile apps, broad channel support

---

## Conclusion

**Carnelian** and **OpenClaw** serve different but overlapping use cases:

- **Carnelian** = Enterprise-grade Rust orchestrator with security focus
- **OpenClaw** = Consumer-friendly TypeScript assistant with broad platform support

The port 18789 coincidence suggests shared heritage or inspiration. Both can coexist: Carnelian for backend orchestration, OpenClaw for frontend interaction.

For a pure Rust skill ecosystem, Carnelian should:
1. Keep TypeScript workers for compatibility
2. Add WASM-based Rust skills for performance-critical operations
3. Maintain the security and audit advantages

