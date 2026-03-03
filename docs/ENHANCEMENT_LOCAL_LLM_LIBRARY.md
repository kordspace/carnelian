# Enhancement Proposal: Local LLM Library with Model Selection

**Status:** Proposed  
**Version:** 1.0  
**Date:** March 3, 2026  
**Author:** Marco Julio Lopes

---

## Executive Summary

This proposal outlines the implementation of a comprehensive local LLM library for Carnelian OS, enabling users to select and deploy reasoning-capable models during setup based on their hardware specifications. The enhancement includes support for three model tiers: **QwQ-32B** (standard), **DeepSeek-R1 7B** (current default), and **Kimi K2.5** (ultra-high-end), with automated model download, container initialization, and machine profile configuration.

---

## Motivation

### Current State
- Carnelian currently defaults to `deepseek-r1:7b` via Ollama
- Single model recommendation per machine profile (Thummim/Urim)
- No automated model selection or download during setup
- Limited reasoning model options

### Problems
1. **Limited Model Choice**: Users cannot select alternative reasoning models suited to their hardware
2. **No Ultra-Tier Support**: High-end machines (300+ GB unified memory) lack optimized model recommendations
3. **Manual Setup**: Users must manually pull models via `ollama pull`
4. **Missing Reasoning Diversity**: No access to newer reasoning models (QwQ-32B, Kimi K2.5)

### Benefits
- **Hardware-Optimized Performance**: Match model to available VRAM/RAM
- **Reasoning Model Diversity**: Access to multiple state-of-the-art reasoning models
- **Automated Setup**: One-command model download and initialization
- **Future-Proof Architecture**: Easy addition of new models as they release

---

## Research Findings

### Model Specifications

#### 1. **QwQ-32B** (Qwen Reasoning Model)
- **Parameters:** 32B
- **Architecture:** Transformer-based reasoning model
- **Context Length:** 32,768 tokens
- **VRAM Requirements:**
  - FP16: ~96 GB
  - Q4_K_M (4-bit quantization): 24 GB (fits RTX 3090/4090)
  - Q8_0 (8-bit): ~48 GB
- **Performance:** Strong reasoning capabilities, comparable to DeepSeek-R1
- **Recommended Hardware:** 
  - Minimum: RTX 3090/4090 (24 GB VRAM) with Q4_K_M
  - Optimal: RTX 6000 Ada (48 GB VRAM) with Q8_0
- **Source:** Alibaba Cloud/Qwen Team
- **License:** Apache 2.0

#### 2. **DeepSeek-R1 7B** (Current Default)
- **Parameters:** 7B
- **Architecture:** Distilled reasoning model from DeepSeek-R1 671B
- **Context Length:** 8,192 tokens
- **VRAM Requirements:**
  - FP16: ~14 GB
  - Q4_0: 4-6 GB
  - Q8_0: 8-10 GB
- **Performance:** Excellent reasoning for size, optimized for consumer GPUs
- **Recommended Hardware:**
  - Minimum: RTX 2080 Super (8 GB VRAM)
  - Optimal: RTX 2080 Ti (11 GB VRAM)
- **Source:** DeepSeek AI
- **License:** MIT

#### 3. **Kimi K2.5** (Ultra-Tier)
- **Parameters:** 1 Trillion (MoE, 32B active per inference)
- **Architecture:** Modified DeepSeek V3 MoE with MoonViT vision encoder
- **Context Length:** 128,000 tokens
- **VRAM/RAM Requirements:**
  - Full Model: 630 GB (4× H200 GPUs)
  - UD-Q2_K_XL (2-bit quant): 375 GB unified memory
  - UD-TQ1_0 (1.8-bit quant): 240 GB unified memory (~10 tokens/s)
  - UD-Q4_K_XL (4-bit): ~500 GB
- **Performance:** State-of-the-art reasoning with vision support (pending llama.cpp)
- **Recommended Hardware:**
  - Minimum: 256 GB unified memory (Mac Studio M3 Ultra × 2)
  - Optimal: 512 GB+ unified memory or 4× H200 GPUs
- **Source:** Moonshot AI
- **License:** Modified MIT

---

## Proposed Solution

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Carnelian Setup Wizard                    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              Hardware Detection & Profiling                  │
│  • Detect VRAM (NVIDIA/AMD)                                 │
│  • Detect Unified Memory (Apple Silicon)                    │
│  • Detect System RAM                                        │
│  • Calculate Total Available Memory                         │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              Model Recommendation Engine                     │
│  • Map hardware → machine profile                           │
│  • Suggest optimal model + quantization                     │
│  • Show alternative models                                  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              User Model Selection                            │
│  ┌─────────────────────────────────────────────┐           │
│  │ Recommended: deepseek-r1:7b (Q4_0)          │           │
│  │ VRAM: 6 GB / Available: 8 GB                │           │
│  │ Performance: ~25 tokens/s                   │           │
│  ├─────────────────────────────────────────────┤           │
│  │ Alternative: qwq-32b:q4 (Q4_K_M)            │           │
│  │ VRAM: 24 GB / Available: 8 GB (⚠️ Too large)│           │
│  ├─────────────────────────────────────────────┤           │
│  │ Skip: Use remote API only                   │           │
│  └─────────────────────────────────────────────┘           │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              Model Download & Initialization                 │
│  • Pull model via Ollama                                    │
│  • Verify model integrity                                   │
│  • Update machine.toml with model selection                 │
│  • Test inference (health check)                            │
└─────────────────────────────────────────────────────────────┘
```

### Machine Profile Definitions

#### **Thummim** (Entry-Level)
- **Hardware:** RTX 2080 Super, 8 GB VRAM, 32 GB RAM
- **Recommended Model:** `deepseek-r1:7b` (Q4_0, ~6 GB)
- **Alternative:** `qwen2.5:7b` (Q4_0, ~4 GB)
- **Performance:** ~20-25 tokens/s

#### **Urim** (High-End)
- **Hardware:** RTX 2080 Ti, 11 GB VRAM, 64 GB RAM
- **Recommended Model:** `deepseek-r1:7b` (Q8_0, ~10 GB)
- **Alternative:** `qwq-32b:q4` (Q4_K_M, ~24 GB, requires offloading)
- **Performance:** ~30-40 tokens/s

#### **Ultra** (New - Ultra-High-End)
- **Hardware:** 
  - Option A: 512 GB+ unified memory (Mac Studio M3 Ultra × 2)
  - Option B: 4× H200 GPUs (320 GB total VRAM)
  - Option C: 256-384 GB unified memory (single Mac Studio M3 Ultra)
- **Recommended Model:** `kimi-k2.5:q2xl` (UD-Q2_K_XL, 375 GB)
- **Alternative:** `kimi-k2.5:q1` (UD-TQ1_0, 240 GB)
- **Performance:** ~10-40 tokens/s (depending on quantization)

---

## Implementation Plan

### Phase 1: Model Library Infrastructure (Week 1)

#### 1.1 Create Model Registry
**File:** `crates/carnelian-core/src/models/registry.rs`

```rust
pub struct ModelSpec {
    pub name: String,
    pub display_name: String,
    pub provider: ModelProvider, // Ollama, HuggingFace, Custom
    pub parameters: u64,
    pub context_length: u32,
    pub quantizations: Vec<QuantizationSpec>,
    pub license: String,
    pub capabilities: Vec<ModelCapability>, // Reasoning, Vision, Code
}

pub struct QuantizationSpec {
    pub name: String, // "Q4_0", "Q8_0", "UD-Q2_K_XL"
    pub vram_gb: u32,
    pub disk_gb: u32,
    pub performance_tokens_per_sec: u32,
}

pub enum ModelProvider {
    Ollama { model_tag: String },
    HuggingFace { repo_id: String },
    Custom { download_url: String },
}
```

#### 1.2 Define Model Catalog
**File:** `crates/carnelian-core/src/models/catalog.rs`

```rust
pub fn get_model_catalog() -> Vec<ModelSpec> {
    vec![
        ModelSpec {
            name: "deepseek-r1-7b",
            display_name: "DeepSeek-R1 7B (Reasoning)",
            provider: ModelProvider::Ollama { model_tag: "deepseek-r1:7b" },
            parameters: 7_000_000_000,
            context_length: 8192,
            quantizations: vec![
                QuantizationSpec { name: "Q4_0", vram_gb: 6, disk_gb: 4, performance_tokens_per_sec: 25 },
                QuantizationSpec { name: "Q8_0", vram_gb: 10, disk_gb: 8, performance_tokens_per_sec: 20 },
            ],
            license: "MIT",
            capabilities: vec![ModelCapability::Reasoning, ModelCapability::Code],
        },
        ModelSpec {
            name: "qwq-32b",
            display_name: "QwQ-32B (Advanced Reasoning)",
            provider: ModelProvider::Ollama { model_tag: "qwq:32b-preview-q4" },
            parameters: 32_000_000_000,
            context_length: 32768,
            quantizations: vec![
                QuantizationSpec { name: "Q4_K_M", vram_gb: 24, disk_gb: 20, performance_tokens_per_sec: 15 },
                QuantizationSpec { name: "Q8_0", vram_gb: 48, disk_gb: 40, performance_tokens_per_sec: 10 },
            ],
            license: "Apache-2.0",
            capabilities: vec![ModelCapability::Reasoning, ModelCapability::Math],
        },
        ModelSpec {
            name: "kimi-k2.5",
            display_name: "Kimi K2.5 (Ultra Reasoning + Vision)",
            provider: ModelProvider::HuggingFace { repo_id: "unsloth/Kimi-K2.5-GGUF" },
            parameters: 1_000_000_000_000,
            context_length: 128000,
            quantizations: vec![
                QuantizationSpec { name: "UD-TQ1_0", vram_gb: 240, disk_gb: 240, performance_tokens_per_sec: 10 },
                QuantizationSpec { name: "UD-Q2_K_XL", vram_gb: 375, disk_gb: 375, performance_tokens_per_sec: 15 },
                QuantizationSpec { name: "UD-Q4_K_XL", vram_gb: 500, disk_gb: 500, performance_tokens_per_sec: 25 },
            ],
            license: "Modified MIT",
            capabilities: vec![ModelCapability::Reasoning, ModelCapability::Vision, ModelCapability::LongContext],
        },
    ]
}
```

#### 1.3 Hardware Detection
**File:** `crates/carnelian-core/src/models/hardware.rs`

```rust
pub struct HardwareProfile {
    pub total_vram_gb: u32,
    pub total_ram_gb: u32,
    pub unified_memory: bool, // Apple Silicon
    pub gpu_model: Option<String>,
    pub cpu_model: String,
}

pub fn detect_hardware() -> Result<HardwareProfile> {
    // Use nvidia-smi for NVIDIA GPUs
    // Use system_profiler for Apple Silicon
    // Use /proc/meminfo for RAM
}

pub fn recommend_machine_profile(hw: &HardwareProfile) -> MachineProfile {
    if hw.unified_memory && hw.total_ram_gb >= 512 {
        MachineProfile::Ultra
    } else if hw.total_vram_gb >= 11 && hw.total_ram_gb >= 64 {
        MachineProfile::Urim
    } else {
        MachineProfile::Thummim
    }
}
```

### Phase 2: Setup Wizard Integration (Week 2)

#### 2.1 Interactive Model Selection
**File:** `crates/carnelian-bin/src/setup/model_selector.rs`

```rust
pub async fn run_model_selection_wizard() -> Result<ModelSelection> {
    // 1. Detect hardware
    let hw = hardware::detect_hardware()?;
    let profile = hardware::recommend_machine_profile(&hw);
    
    // 2. Get compatible models
    let catalog = catalog::get_model_catalog();
    let compatible = filter_compatible_models(&catalog, &hw);
    
    // 3. Present options to user
    println!("🔍 Detected Hardware:");
    println!("  VRAM: {} GB", hw.total_vram_gb);
    println!("  RAM: {} GB", hw.total_ram_gb);
    println!("  Profile: {:?}", profile);
    println!();
    
    let recommended = compatible.first().unwrap();
    println!("✨ Recommended Model: {}", recommended.display_name);
    println!("  VRAM Required: {} GB", recommended.quantizations[0].vram_gb);
    println!("  Performance: ~{} tokens/s", recommended.quantizations[0].performance_tokens_per_sec);
    println!();
    
    // 4. Get user choice
    let selection = prompt_user_selection(&compatible)?;
    
    Ok(selection)
}
```

#### 2.2 Model Download & Initialization
**File:** `crates/carnelian-bin/src/setup/model_downloader.rs`

```rust
pub async fn download_and_initialize_model(spec: &ModelSpec, quant: &QuantizationSpec) -> Result<()> {
    match &spec.provider {
        ModelProvider::Ollama { model_tag } => {
            println!("📥 Pulling {} from Ollama...", model_tag);
            run_command(&format!("ollama pull {}", model_tag)).await?;
        }
        ModelProvider::HuggingFace { repo_id } => {
            println!("📥 Downloading {} from HuggingFace...", repo_id);
            download_gguf_from_hf(repo_id, &quant.name).await?;
        }
        ModelProvider::Custom { download_url } => {
            println!("📥 Downloading from {}...", download_url);
            download_file(download_url).await?;
        }
    }
    
    // Verify model
    println!("✅ Verifying model integrity...");
    verify_model_health(spec).await?;
    
    // Update machine.toml
    update_machine_config(spec, quant)?;
    
    Ok(())
}
```

### Phase 3: Machine Profile Updates (Week 2)

#### 3.1 Add Ultra Profile
**File:** `machine-profiles/ultra.toml`

```toml
[profile]
name = "ultra"
description = "Ultra-high-end machine (512+ GB unified memory or 4× H200 GPUs)"

[hardware]
min_vram_gb = 320  # 4× H200
min_ram_gb = 512   # or unified memory
min_disk_gb = 500

[ollama]
model = "kimi-k2.5:q2xl"
keep_alive = -1
num_gpu = 999

[resources]
max_concurrent_tasks = 32
worker_pool_size = 16
embedding_batch_size = 128
```

#### 3.2 Update Existing Profiles
**Files:** `machine-profiles/thummim.toml`, `machine-profiles/urim.toml`

Add model selection field:
```toml
[ollama]
model = "deepseek-r1:7b"  # User-selected during setup
model_alternatives = ["qwen2.5:7b", "llama3.1:8b"]
```

### Phase 4: Documentation & User Experience (Week 3)

#### 4.1 Update GETTING_STARTED.md
Add section:
```markdown
## 5. Select Your Local LLM

Carnelian will automatically detect your hardware and recommend an optimal reasoning model:

**Detected Hardware:**
- VRAM: 8 GB (RTX 2080 Super)
- RAM: 32 GB
- Profile: Thummim

**Recommended Model:** DeepSeek-R1 7B (Q4_0)
- VRAM Required: 6 GB
- Performance: ~25 tokens/s
- License: MIT

**Alternative Models:**
- QwQ-32B (Q4_K_M) - Requires 24 GB VRAM ⚠️
- Qwen2.5 7B (Q4_0) - Requires 4 GB VRAM ✅

**Select model:**
1. deepseek-r1:7b (Recommended)
2. qwen2.5:7b
3. Skip (use remote API only)

Choice [1]: _
```

#### 4.2 Create docs/MODELS.md
Comprehensive model comparison table, hardware requirements, performance benchmarks.

#### 4.3 Update Docker Compose
**File:** `docker-compose.yml`

```yaml
services:
  carnelian-ollama:
    image: ollama/ollama:latest
    volumes:
      - ./models:/root/.ollama  # Pre-downloaded models
    environment:
      - OLLAMA_MODELS=/root/.ollama
      - OLLAMA_KEEP_ALIVE=${OLLAMA_KEEP_ALIVE:--1}
      - OLLAMA_NUM_GPU=${OLLAMA_NUM_GPU:-999}
```

Add model pre-loading script:
**File:** `scripts/preload-models.sh`

```bash
#!/bin/bash
# Pre-download models for offline setup
ollama pull deepseek-r1:7b
ollama pull qwq:32b-preview-q4
# Kimi K2.5 requires manual GGUF download from HuggingFace
```

---

## OpenClaw Disclaimer

### LICENSE.md Addition

Add new section after "Acknowledgments":

```markdown
## Relationship to OpenClaw

Carnelian Core was inspired by [OpenClaw](https://github.com/openclaw), an AI agent framework created by Peter Steinberger. While OpenClaw provided foundational inspiration for agent orchestration concepts, **Carnelian Core is a fundamentally different and unique implementation**.

### Key Architectural Differences

| Aspect | OpenClaw | Carnelian Core |
|--------|----------|----------------|
| **Language** | Python | Rust (core), TypeScript (UI), Python (workers) |
| **Architecture** | Monolithic agent framework | Multi-runtime worker orchestration with JSONL transport |
| **Security Model** | Traditional permissions | Capability-based deny-by-default with Ed25519 signatures |
| **State Management** | In-memory/file-based | PostgreSQL with pgvector, ledger-backed event sourcing |
| **Quantum Integration** | None | MAGIC subsystem with quantum entropy providers (Quantum Origin, Quantinuum H2, Qiskit) |
| **Knowledge Persistence** | RAG with vector DB | Elixir system with approval workflow, quality scoring, version control |
| **Skill System** | Python plugins | Multi-runtime (Node.js, Python, WASM, Rust) with 50+ curated skills |
| **Mantra System** | None | Quantum-seeded weighted context injection with cooldowns |
| **XP Progression** | None | Ledger-backed XP with automatic event sourcing and BLAKE3 hash-chaining |
| **Desktop UI** | CLI/Web | Dioxus native desktop application |
| **License** | MIT | Open source (free personal use), commercial licensing via Kordspace LLC |

### Novel Contributions

Carnelian Core introduces several innovations not present in OpenClaw or other agent frameworks:

1. **Quantum-Enhanced Entropy Generation**: First-of-its-kind multi-provider quantum entropy chain with cryptographic mixing for key generation, ledger salting, and mantra scheduling.

2. **Mantra Matrix System**: Weighted, cooldown-enforced context injection using quantum-seeded selection across 18 categories with 100+ mantras.

3. **Capability-Based Security**: Deny-by-default security model with Ed25519-signed authority chains, eliminating ambient authority vulnerabilities.

4. **Ledger-Backed XP Progression**: Immutable event sourcing for agent progression with BLAKE3 hash-chaining and quantum integrity verification.

5. **Multi-Runtime Worker Orchestration**: Unified orchestration of Node.js, Python, WASM, and native Rust workers via JSONL transport protocol.

6. **Elixir Knowledge System**: RAG-based knowledge persistence with pgvector embeddings, approval workflow, quality scoring, and version control.

### Acknowledgment

We acknowledge OpenClaw as an inspirational source that demonstrated the potential of AI agent frameworks. Carnelian Core builds upon these concepts while introducing a completely new architecture, security model, and feature set designed for production deployment and commercial use.

For a detailed comparison of agent frameworks, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#comparison-with-other-frameworks).
```

---

## Success Metrics

### Technical Metrics
- ✅ Model download success rate > 95%
- ✅ Hardware detection accuracy > 98%
- ✅ Model inference health check pass rate > 99%
- ✅ Setup wizard completion time < 10 minutes

### User Experience Metrics
- ✅ User satisfaction with model selection process > 4.5/5
- ✅ Percentage of users selecting recommended model > 70%
- ✅ Support tickets related to model setup < 5%

### Performance Metrics
- ✅ Thummim: 20-25 tokens/s (DeepSeek-R1 7B Q4_0)
- ✅ Urim: 30-40 tokens/s (DeepSeek-R1 7B Q8_0)
- ✅ Ultra: 10-40 tokens/s (Kimi K2.5 Q2_K_XL)

---

## Risks & Mitigation

### Risk 1: Model Download Failures
**Mitigation:** Implement retry logic, fallback to alternative models, offline model bundles

### Risk 2: Hardware Detection Inaccuracy
**Mitigation:** Manual override option, conservative recommendations, extensive testing

### Risk 3: Kimi K2.5 Availability
**Mitigation:** Provide clear HuggingFace download instructions, automated GGUF downloader

### Risk 4: Disk Space Constraints
**Mitigation:** Pre-flight disk space check, warn users, offer quantization options

---

## Timeline

- **Week 1:** Model registry, catalog, hardware detection
- **Week 2:** Setup wizard, model downloader, Ultra profile
- **Week 3:** Documentation, Docker updates, testing
- **Week 4:** User testing, refinement, v1.1.0 release

---

## Conclusion

This enhancement transforms Carnelian OS from a single-model system to a flexible, hardware-aware LLM platform. By supporting QwQ-32B, DeepSeek-R1 7B, and Kimi K2.5, we enable users across all hardware tiers to leverage state-of-the-art reasoning models optimized for their specific configurations.

The automated setup wizard, combined with the new Ultra machine profile, positions Carnelian OS as a premier choice for both consumer and enterprise AI deployments.
