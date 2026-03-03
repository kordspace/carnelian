# 🔥 Carnelian OS v1.0.0 - Security & Architecture Review

**Date:** March 3, 2026  
**Scope:** Comprehensive security audit, quantum-resistant cryptography migration, orchestration patterns, context systems, and workspace integration  
**Status:** Pre-v1.0.0 Release Review

---

## Executive Summary

This document provides a comprehensive security and architecture review of Carnelian OS v1.0.0, with specific focus on:

1. **Quantum-Resistant Cryptography Migration** - Transitioning from Ed25519 to post-quantum algorithms when MAGIC is enabled
2. **Checksum Validation & Memory Integrity** - Optimizing verification speed and security
3. **Task Scheduler Orchestration** - Alignment with OpenClaw patterns and autonomous task creation
4. **Context & Mantra Systems** - Session-driven task generation and LLM context management
5. **Workspace Skill Integration** - Tool availability and execution security

---

## 1. Quantum-Resistant Cryptography Migration

### 1.1 Current State: Ed25519 Vulnerability

**Current Implementation:**
- **Signing Keys:** Ed25519 (256-bit elliptic curve)
- **Encryption:** AES-256 derived from Ed25519 seed via blake3 HKDF
- **Key Derivation:** `derive_aes_storage_key(signing_key)` → 32-byte AES key
- **Usage:** Config encryption, memory encryption, run log encryption, API key storage

**Quantum Vulnerability:**
- **Shor's Algorithm:** Can break Ed25519 in polynomial time on a quantum computer
- **Timeline:** NIST estimates 10-30 years before large-scale quantum computers threaten current crypto
- **Risk:** All Ed25519 signatures and derived keys vulnerable to "harvest now, decrypt later" attacks

**Affected Components:**
```
crates/carnelian-core/src/crypto.rs
├── generate_ed25519_keypair()
├── generate_ed25519_keypair_with_entropy()  ← Uses MAGIC entropy
├── derive_aes_storage_key()                 ← Derives from Ed25519 seed
└── sign_bytes()                             ← Ed25519 signatures

crates/carnelian-core/src/encryption.rs
└── EncryptionHelper::new()                  ← Uses Ed25519-derived AES key

crates/carnelian-core/src/config.rs
└── owner_signing_key: Option<SigningKey>    ← Ed25519 key storage
```

### 1.2 Recommended Migration Strategy

**Phase 1: Hybrid Cryptography (v1.1.0)**

When `config.magic.enabled = true`, use **hybrid post-quantum + classical** signatures:

1. **Primary Signature:** CRYSTALS-Dilithium (NIST PQC standard)
   - **Algorithm:** Dilithium3 (recommended security level)
   - **Key Size:** 1952 bytes (public), 4000 bytes (private)
   - **Signature Size:** 3293 bytes
   - **Security:** 192-bit quantum security (equivalent to AES-192)

2. **Fallback Signature:** Ed25519 (for backward compatibility)
   - Dual-sign all critical operations
   - Verify both signatures until full migration

3. **Key Derivation:** Use MAGIC quantum entropy for both key types
   ```rust
   // Pseudo-code
   if config.magic.enabled {
       let entropy = magic_provider.get_bytes(128).await?;
       let dilithium_seed = &entropy[0..32];
       let ed25519_seed = &entropy[32..64];
       let aes_seed = &entropy[64..96];
       
       (dilithium_keypair, ed25519_keypair, aes_key)
   }
   ```

**Phase 2: Post-Quantum Encryption (v1.2.0)**

Replace AES-256 key derivation with **CRYSTALS-Kyber** (NIST PQC KEM):

1. **Key Encapsulation:** Kyber1024 (256-bit quantum security)
   - **Public Key:** 1568 bytes
   - **Ciphertext:** 1568 bytes
   - **Shared Secret:** 32 bytes (use for AES-256-GCM)

2. **Hybrid KEM:** Combine Kyber + X25519 for defense-in-depth
   ```rust
   shared_secret = KDF(kyber_shared_secret || x25519_shared_secret)
   ```

**Phase 3: Full Post-Quantum Stack (v2.0.0)**

- Remove all classical crypto dependencies
- Use Dilithium for all signatures
- Use Kyber for all key exchange
- Maintain MAGIC quantum entropy as foundation

### 1.3 Implementation Checklist

**Crate Dependencies:**
```toml
[dependencies]
pqcrypto-dilithium = "0.5"  # CRYSTALS-Dilithium
pqcrypto-kyber = "0.8"      # CRYSTALS-Kyber
pqcrypto-traits = "0.3"
```

**Migration Steps:**
- [ ] Add `QuantumResistantCrypto` feature flag
- [ ] Implement `HybridSigningKey` wrapper (Dilithium + Ed25519)
- [ ] Update `crypto.rs` with PQC key generation using MAGIC entropy
- [ ] Add `derive_pqc_storage_key()` using Kyber KEM
- [ ] Update `EncryptionHelper` to support both key types
- [ ] Add `key_algorithm` column to `config_store` table
- [ ] Implement key rotation workflow (Ed25519 → Hybrid → PQC)
- [ ] Add PQC signature verification to ledger integrity checks
- [ ] Update documentation with migration guide

**Security Note:**
> When MAGIC is enabled, all cryptographic key material MUST be derived from quantum entropy sources (Quantum Origin, Quantinuum H2, IBM Quantum) to ensure true quantum randomness. This prevents backdoors in classical PRNGs and provides quantum-grade security from the foundation.

---

## 2. Checksum Validation & Memory Integrity

### 2.1 Current Implementation

**Ledger Integrity:**
```rust
crates/carnelian-core/src/ledger.rs
├── verify_chain()           ← Verifies blake3 hash chain
├── compute_entry_hash()     ← blake3::hash(entry_bytes)
└── verify_signature()       ← Ed25519 signature verification
```

**Memory Checksums:**
```rust
crates/carnelian-core/src/memory.rs
├── MemoryEntry { checksum: Option<String> }
└── compute_checksum()       ← blake3::hash(content)
```

**Performance Characteristics:**
- **blake3:** ~3 GB/s on modern CPUs (SIMD-optimized)
- **Ed25519 verify:** ~70,000 verifications/sec
- **Bottleneck:** Database I/O, not crypto operations

### 2.2 Optimization Strategies

**1. Batch Verification**
```rust
// Instead of verifying one-by-one
for entry in entries {
    verify_signature(&entry.signature)?;  // 70k/sec
}

// Use batch verification (10x faster)
ed25519_dalek::verify_batch(&signatures, &messages, &public_keys)?;  // 700k/sec
```

**2. Merkle Tree for Memory Integrity**

Replace individual checksums with a Merkle tree:
```
                    Root Hash
                   /          \
            Hash(A,B)        Hash(C,D)
           /      \          /      \
      Hash(A)  Hash(B)  Hash(C)  Hash(D)
         |        |        |        |
    Memory1  Memory2  Memory3  Memory4
```

**Benefits:**
- **Incremental Updates:** Only recompute O(log n) hashes on insert
- **Proof of Inclusion:** Verify single memory in O(log n) time
- **Tamper Detection:** Any modification changes root hash
- **Storage:** Store only root hash + Merkle proofs

**Implementation:**
```rust
pub struct MemoryMerkleTree {
    root_hash: [u8; 32],
    leaves: Vec<[u8; 32]>,  // blake3 hashes of memories
}

impl MemoryMerkleTree {
    pub fn verify_memory(&self, memory_id: i64, proof: &MerkleProof) -> bool {
        // O(log n) verification
        proof.verify(self.root_hash, memory_id)
    }
    
    pub fn update_memory(&mut self, memory_id: i64, new_hash: [u8; 32]) {
        // O(log n) update
        self.leaves[memory_id as usize] = new_hash;
        self.recompute_path(memory_id);
    }
}
```

**3. Parallel Verification with Rayon**
```rust
use rayon::prelude::*;

entries.par_iter()
    .map(|entry| verify_entry(entry))
    .collect::<Result<Vec<_>>>()?;
```

### 2.3 MAGIC Integration for Checksums

**Quantum-Enhanced Hashing:**

When MAGIC is enabled, use quantum entropy to seed hash functions:
```rust
pub async fn compute_quantum_checksum(
    data: &[u8],
    entropy_provider: &dyn EntropyProvider,
) -> Result<[u8; 32]> {
    // Get 32 bytes of quantum entropy for salt
    let salt = entropy_provider.get_bytes(32).await?;
    
    // Use blake3 keyed hash with quantum salt
    let mut hasher = blake3::Hasher::new_keyed(&salt);
    hasher.update(data);
    Ok(hasher.finalize().into())
}
```

**Benefits:**
- **Collision Resistance:** Quantum randomness prevents preimage attacks
- **Unique Per-Instance:** Each Carnelian instance has quantum-seeded hashes
- **Audit Trail:** Log quantum entropy source in ledger

---

## 3. Task Scheduler Orchestration

### 3.1 Current Architecture

**Scheduler Components:**
```rust
crates/carnelian-core/src/scheduler.rs
├── Scheduler::run_heartbeat()     ← Every 60 seconds
├── Scheduler::dispatch_task()     ← Task → Worker assignment
├── Scheduler::poll_pending_tasks() ← Check task queue
└── Scheduler::cleanup_stale_runs() ← Timeout handling
```

**Heartbeat Operations:**
1. Poll pending tasks from `tasks` table
2. Check worker availability
3. Dispatch tasks to workers
4. Update task status
5. Cleanup stale runs
6. Emit metrics

### 3.2 OpenClaw Orchestration Patterns

**Key Differences:**

| Aspect | Carnelian (Current) | OpenClaw Pattern | Recommendation |
|--------|---------------------|------------------|----------------|
| **Task Creation** | Manual via API | Autonomous from context | ✅ Implement context-driven task creation |
| **Prioritization** | FIFO queue | Capability-based scoring | ✅ Add priority scoring system |
| **Worker Selection** | Round-robin | Skill-matching | ✅ Implement skill-based routing |
| **Failure Handling** | Retry with backoff | Adaptive retry + escalation | ✅ Add escalation workflow |
| **Context Propagation** | Session-based | Thread-based context tree | ✅ Implement context threading |

### 3.3 Autonomous Task Creation from Context

**Proposed Enhancement:**

Add `ContextAnalyzer` to monitor session activity and create tasks:

```rust
pub struct ContextAnalyzer {
    session_manager: Arc<SessionManager>,
    scheduler: Arc<Mutex<Scheduler>>,
    mantra_tree: Arc<MantraTree>,
}

impl ContextAnalyzer {
    /// Analyze session context and create follow-up tasks
    pub async fn analyze_session(&self, session_id: &str) -> Result<Vec<Task>> {
        let session = self.session_manager.get_session(session_id).await?;
        let messages = session.get_recent_messages(10).await?;
        
        // Extract action items from conversation
        let action_items = self.extract_action_items(&messages).await?;
        
        // Create tasks for each action item
        let mut tasks = Vec::new();
        for item in action_items {
            let task = Task {
                title: item.title,
                description: item.description,
                context: json!({
                    "session_id": session_id,
                    "source": "context_analyzer",
                    "priority": item.priority,
                }),
                ..Default::default()
            };
            tasks.push(task);
        }
        
        Ok(tasks)
    }
    
    /// Extract action items using mantra-guided LLM analysis
    async fn extract_action_items(&self, messages: &[Message]) -> Result<Vec<ActionItem>> {
        // Use mantra system to guide extraction
        let mantra = self.mantra_tree.select_with_db(
            &self.session_manager.pool,
            "task_extraction",
            None,
        ).await?;
        
        // LLM call with mantra prompt
        // ...
    }
}
```

**Integration Points:**
1. **Scheduler Heartbeat:** Call `ContextAnalyzer::analyze_session()` for active sessions
2. **Session Events:** Trigger analysis on message milestones (every 10 messages)
3. **Mantra Cooldown:** Respect `mantra_cooldown_beats` to avoid spam

### 3.4 Capability-Based Task Routing

**Current Limitation:**
Tasks are dispatched to any available worker, regardless of capabilities.

**Proposed Enhancement:**
```rust
pub struct TaskRouter {
    worker_manager: Arc<Mutex<WorkerManager>>,
    capability_registry: Arc<CapabilityRegistry>,
}

impl TaskRouter {
    pub async fn find_best_worker(&self, task: &Task) -> Result<Option<WorkerId>> {
        let required_caps = task.required_capabilities();
        let workers = self.worker_manager.lock().await.list_workers();
        
        // Score workers by capability match
        let mut scored_workers: Vec<(WorkerId, f64)> = workers
            .iter()
            .filter_map(|w| {
                let caps = self.capability_registry.get_worker_capabilities(w.id)?;
                let score = self.compute_capability_score(&required_caps, &caps);
                Some((w.id, score))
            })
            .collect();
        
        // Sort by score descending
        scored_workers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        
        Ok(scored_workers.first().map(|(id, _)| *id))
    }
    
    fn compute_capability_score(&self, required: &[Capability], available: &[Capability]) -> f64 {
        let matches = required.iter()
            .filter(|r| available.contains(r))
            .count();
        
        matches as f64 / required.len() as f64
    }
}
```

---

## 4. Context & Mantra Systems

### 4.1 Current Mantra Architecture

**Components:**
```rust
crates/carnelian-magic/src/mantra.rs
├── MantraTree::select()           ← Select mantra by category
├── MantraTree::select_with_db()   ← DB-backed selection
├── MantraTree::build_context()    ← Build LLM context from mantras
└── MantraTree::preload()          ← Load mantras from DB
```

**Mantra Categories:**
- `system_prompt` - Core system instructions
- `task_execution` - Task-specific guidance
- `skill_invocation` - Skill execution patterns
- `error_recovery` - Failure handling
- `context_compression` - Memory optimization

### 4.2 Session-Driven Context Management

**Current Flow:**
```
User Message → Session → LLM Call → Response
                  ↓
            Context Window (fixed size)
```

**Proposed Enhancement:**
```
User Message → Session → Context Analyzer → Mantra Selection → LLM Call
                  ↓              ↓                ↓
            Message History   Action Items    Dynamic Context
                  ↓              ↓                ↓
            Compression    Task Creation    Skill Suggestions
```

**Implementation:**
```rust
pub struct ContextManager {
    session_manager: Arc<SessionManager>,
    mantra_tree: Arc<MantraTree>,
    memory_manager: Arc<MemoryManager>,
}

impl ContextManager {
    /// Build optimized context for LLM call
    pub async fn build_context(&self, session_id: &str) -> Result<String> {
        let session = self.session_manager.get_session(session_id).await?;
        let messages = session.get_messages().await?;
        
        // 1. Compress old messages using mantra-guided summarization
        let compressed = self.compress_history(&messages[..messages.len()-10]).await?;
        
        // 2. Keep recent messages verbatim
        let recent = &messages[messages.len()-10..];
        
        // 3. Retrieve relevant memories
        let memories = self.memory_manager
            .search_memories(&session.last_message_content(), 5)
            .await?;
        
        // 4. Select relevant mantras
        let mantras = self.mantra_tree
            .select_with_db(&self.session_manager.pool, "context_building", None)
            .await?;
        
        // 5. Assemble final context
        Ok(format!(
            "{}\n\n# Compressed History\n{}\n\n# Recent Messages\n{}\n\n# Relevant Memories\n{}",
            mantras.template,
            compressed,
            Self::format_messages(recent),
            Self::format_memories(&memories),
        ))
    }
}
```

### 4.3 Mantra-Driven Task Triggers

**Concept:** Use mantras to define triggers for automatic task creation.

**Example Mantra:**
```toml
[mantra.task_triggers.code_review]
category = "task_creation"
template = """
When the user commits code changes, automatically create a task to:
1. Run static analysis (clippy, cargo check)
2. Run test suite
3. Generate documentation
4. Update CHANGELOG.md
"""
trigger_pattern = "git commit|push changes|deploy"
priority = "high"
```

**Implementation:**
```rust
pub struct MantraTriggerEngine {
    mantra_tree: Arc<MantraTree>,
    scheduler: Arc<Mutex<Scheduler>>,
}

impl MantraTriggerEngine {
    pub async fn check_triggers(&self, session_id: &str, message: &str) -> Result<()> {
        let triggers = self.mantra_tree.get_triggers("task_creation").await?;
        
        for trigger in triggers {
            if trigger.pattern_matches(message) {
                let task = self.create_task_from_mantra(&trigger, session_id).await?;
                self.scheduler.lock().await.enqueue_task(task).await?;
            }
        }
        
        Ok(())
    }
}
```

---

## 5. Workspace Skill Integration

### 5.1 Current Skill Architecture

**Skill Discovery:**
```rust
crates/carnelian-core/src/skills/discovery.rs
├── SkillDiscovery::refresh()      ← Scan registry directories
├── SkillDiscovery::register()     ← Add skill to DB
└── start_file_watcher()           ← Auto-discovery on file changes
```

**Skill Execution:**
```rust
crates/carnelian-core/src/skills/bridge.rs
├── SkillBridge::invoke_skill()    ← Execute skill with params
├── SkillBridge::list_skills()     ← Query available skills
└── SkillBridge::validate_params() ← Type checking
```

**Registries:**
- `skills/node-registry/` - TypeScript/JavaScript skills
- `skills/python-registry/` - Python skills
- `skills/rust-registry/` - Rust skills (future)

### 5.2 Tool Availability & Security

**Current Security Model:**
1. **Skill Isolation:** Each skill runs in separate process
2. **Capability Checks:** Skills declare required capabilities in `skill.json`
3. **Timeout Enforcement:** Skills killed after timeout (default: 30s)
4. **Output Validation:** JSON schema validation on skill output

**Gaps:**
- ❌ No sandboxing (skills can access filesystem)
- ❌ No network isolation
- ❌ No resource limits (CPU, memory)
- ❌ No audit logging of skill execution

**Recommended Enhancements:**

**1. Skill Sandboxing with Bubblewrap (Linux)**
```rust
pub async fn invoke_skill_sandboxed(&self, skill_id: &str, params: Value) -> Result<Value> {
    let skill = self.get_skill(skill_id).await?;
    
    // Build bwrap command
    let mut cmd = Command::new("bwrap");
    cmd.args(&[
        "--ro-bind", "/usr", "/usr",
        "--ro-bind", "/lib", "/lib",
        "--ro-bind", "/lib64", "/lib64",
        "--tmpfs", "/tmp",
        "--proc", "/proc",
        "--dev", "/dev",
        "--unshare-all",
        "--die-with-parent",
        "--",
        &skill.runtime_path(),
        &skill.entry_point(),
    ]);
    
    // Execute with timeout
    let output = timeout(Duration::from_secs(skill.timeout_secs), cmd.output()).await??;
    
    // Parse and validate output
    serde_json::from_slice(&output.stdout)
}
```

**2. Resource Limits with cgroups**
```rust
pub struct SkillResourceLimits {
    max_memory_mb: u64,
    max_cpu_percent: u32,
    max_processes: u32,
}

impl SkillBridge {
    fn apply_cgroup_limits(&self, pid: u32, limits: &SkillResourceLimits) -> Result<()> {
        // Create cgroup
        let cgroup_path = format!("/sys/fs/cgroup/carnelian/skill-{}", pid);
        std::fs::create_dir_all(&cgroup_path)?;
        
        // Set memory limit
        std::fs::write(
            format!("{}/memory.max", cgroup_path),
            format!("{}", limits.max_memory_mb * 1024 * 1024),
        )?;
        
        // Set CPU limit
        std::fs::write(
            format!("{}/cpu.max", cgroup_path),
            format!("{} 100000", limits.max_cpu_percent * 1000),
        )?;
        
        // Add process to cgroup
        std::fs::write(format!("{}/cgroup.procs", cgroup_path), format!("{}", pid))?;
        
        Ok(())
    }
}
```

**3. Skill Execution Audit Log**
```rust
pub async fn log_skill_execution(&self, execution: &SkillExecution) -> Result<()> {
    sqlx::query(
        r"INSERT INTO skill_execution_log 
          (skill_id, session_id, params, output, duration_ms, success, error)
          VALUES ($1, $2, $3, $4, $5, $6, $7)"
    )
    .bind(&execution.skill_id)
    .bind(&execution.session_id)
    .bind(&execution.params)
    .bind(&execution.output)
    .bind(execution.duration_ms)
    .bind(execution.success)
    .bind(&execution.error)
    .execute(&self.pool)
    .await?;
    
    Ok(())
}
```

### 5.3 Workspace Tool Integration

**Current Tools:**
- **SkillBridge:** Execute registered skills
- **WorkerManager:** Manage background workers
- **SessionManager:** Conversation persistence
- **MemoryManager:** Knowledge graph
- **ElixirManager:** Knowledge artifacts

**Missing Tools:**
- ❌ **FileSystemTool:** Safe file operations
- ❌ **GitTool:** Version control operations
- ❌ **DatabaseTool:** SQL query execution
- ❌ **HTTPTool:** External API calls
- ❌ **ShellTool:** Command execution (high-risk)

**Proposed Tool Framework:**
```rust
#[async_trait]
pub trait WorkspaceTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn required_capabilities(&self) -> Vec<Capability>;
    
    async fn execute(&self, params: Value) -> Result<Value>;
    async fn validate_params(&self, params: &Value) -> Result<()>;
}

pub struct FileSystemTool {
    allowed_paths: Vec<PathBuf>,
}

#[async_trait]
impl WorkspaceTool for FileSystemTool {
    fn name(&self) -> &str { "filesystem" }
    
    fn required_capabilities(&self) -> Vec<Capability> {
        vec![Capability::FileRead, Capability::FileWrite]
    }
    
    async fn execute(&self, params: Value) -> Result<Value> {
        let operation = params["operation"].as_str()
            .ok_or_else(|| Error::InvalidInput("Missing operation".into()))?;
        
        match operation {
            "read" => self.read_file(&params["path"]).await,
            "write" => self.write_file(&params["path"], &params["content"]).await,
            "list" => self.list_directory(&params["path"]).await,
            _ => Err(Error::InvalidInput(format!("Unknown operation: {}", operation))),
        }
    }
}
```

---

## 6. Recommendations Summary

### 6.1 Critical (v1.0.0)
- ✅ **entropy_timeout_secs alias** - COMPLETED
- ✅ **Nested MAGIC config** - COMPLETED
- ✅ **Machine profile names** - COMPLETED
- ✅ **Test compilation fixes** - COMPLETED

### 6.2 High Priority (v1.1.0)
- [ ] **Hybrid PQC Signatures** - Dilithium + Ed25519 when MAGIC enabled
- [ ] **Merkle Tree Memory Integrity** - Replace individual checksums
- [ ] **Context-Driven Task Creation** - Autonomous task generation from sessions
- [ ] **Skill Sandboxing** - Bubblewrap isolation for skill execution
- [ ] **Capability-Based Routing** - Match tasks to workers by skills

### 6.3 Medium Priority (v1.2.0)
- [ ] **Post-Quantum Encryption** - Kyber KEM for key exchange
- [ ] **Batch Signature Verification** - 10x performance improvement
- [ ] **Mantra Trigger Engine** - Automatic task creation from patterns
- [ ] **Workspace Tool Framework** - Standardized tool interface
- [ ] **Resource Limits** - cgroups for skill execution

### 6.4 Long-Term (v2.0.0)
- [ ] **Full PQC Stack** - Remove all classical crypto
- [ ] **Distributed Merkle Forest** - Multi-node memory integrity
- [ ] **Advanced Orchestration** - OpenClaw-style context threading
- [ ] **Zero-Trust Skill Execution** - Complete isolation + attestation

---

## 7. Migration Path

### v1.0.0 → v1.1.0 (Quantum Hardening)
1. Add `pqcrypto-dilithium` dependency
2. Implement `HybridSigningKey` wrapper
3. Update key generation to use MAGIC entropy for both key types
4. Add `key_algorithm` column to database
5. Dual-sign all ledger entries
6. Provide migration tool: `carnelian crypto migrate --to hybrid`

### v1.1.0 → v1.2.0 (Encryption Upgrade)
1. Add `pqcrypto-kyber` dependency
2. Implement Kyber KEM for key exchange
3. Update `EncryptionHelper` to support Kyber-derived keys
4. Re-encrypt all sensitive data with new keys
5. Provide migration tool: `carnelian crypto rotate --algorithm kyber`

### v1.2.0 → v2.0.0 (Full PQC)
1. Remove Ed25519 dependencies
2. Remove X25519 dependencies
3. Update all signature verification to Dilithium-only
4. Update all encryption to Kyber-only
5. Audit all cryptographic operations for quantum resistance

---

## 8. Conclusion

Carnelian OS v1.0.0 has a solid foundation for security and orchestration. The primary recommendations focus on:

1. **Quantum Resistance:** Migrate to PQC when MAGIC is enabled to future-proof against quantum attacks
2. **Performance:** Optimize checksum validation with Merkle trees and batch verification
3. **Autonomy:** Implement context-driven task creation for true agentic behavior
4. **Security:** Add skill sandboxing and resource limits to prevent abuse
5. **Alignment:** Adopt OpenClaw orchestration patterns for better task routing

These enhancements will position Carnelian as a quantum-resistant, autonomous agent operating system ready for the post-quantum era.

---

**Next Steps:**
1. Review and approve this document
2. Create GitHub issues for each recommendation
3. Prioritize v1.1.0 roadmap
4. Begin PQC library evaluation and testing
5. Update security documentation with migration guides
