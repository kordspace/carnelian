//! WASM Skill Runtime
//!
//! Sandboxed skill execution using wasmtime and WASI.
//! Provides capability-based security and resource limits.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use tracing::{error, info, warn};
use wasmtime::{Config, Engine, Linker, Module, Store, TypedFunc};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder};

use crate::skills::skill_trait::{HealthStatus, SkillInput, SkillOutput};
use carnelian_common::{Error, Result};

/// State for WASM skill execution
pub struct WasmState {
    /// WASI context for system access
    wasi: WasiCtx,

    /// Granted capabilities
    capabilities: Vec<String>,

    /// Output buffer for results
    output_buffer: Arc<Mutex<Vec<u8>>>,
}

/// A WASM-based skill instance
pub struct WasmSkill {
    /// Skill ID
    id: String,

    /// Skill name
    name: String,

    /// WASM module
    module: Module,

    /// WASM engine (shared)
    engine: Arc<Engine>,

    /// Maximum memory allowed (in pages of 64KB)
    max_memory_pages: u32,

    /// Timeout for execution (seconds)
    timeout_secs: u64,
}

/// Runtime for executing WASM skills
pub struct WasmSkillRuntime {
    /// WASM engine
    engine: Arc<Engine>,

    /// Module linker
    linker: Linker<WasmState>,

    /// Loaded skills
    skills: Arc<Mutex<HashMap<String, WasmSkill>>>,

    /// Default resource limits
    default_max_memory_pages: u32,
    default_timeout_secs: u64,
}

impl WasmSkillRuntime {
    /// Create a new WASM runtime with default settings
    pub fn new() -> Result<Self> {
        // Configure WASM engine
        let mut config = Config::new();
        config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);
        config.async_support(true);
        config.epoch_interruption(true);

        let engine = Arc::new(
            Engine::new(&config)
                .map_err(|e| Error::Worker(format!("Failed to create WASM engine: {}", e)))?,
        );

        // Create linker with WASI - disabled for now due to API changes in v27
        let linker = Linker::new(&engine);
        // TODO: Update to wasmtime-wasi v27 API when ready
        // wasmtime_wasi::add_to_linker(&mut linker, |state: &mut WasmState| &mut state.wasi)

        Ok(Self {
            engine,
            linker,
            skills: Arc::new(Mutex::new(HashMap::new())),
            default_max_memory_pages: 1024, // 64MB
            default_timeout_secs: 30,
        })
    }

    /// Load a WASM skill from file
    pub fn load(&self, path: &Path, id: impl Into<String>) -> Result<String> {
        let id = id.into();
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&id)
            .to_string();

        info!(skill_id = %id, path = %path.display(), "Loading WASM skill");

        // Read WASM bytes
        let wasm_bytes = std::fs::read(path).map_err(|e| Error::Io(e))?;

        // Compile module
        let module = Module::new(&self.engine, wasm_bytes)
            .map_err(|e| Error::Worker(format!("Failed to compile WASM: {}", e)))?;

        let skill = WasmSkill {
            id: id.clone(),
            name,
            module,
            engine: self.engine.clone(),
            max_memory_pages: self.default_max_memory_pages,
            timeout_secs: self.default_timeout_secs,
        };

        // Store skill
        let mut skills = self.skills.lock().unwrap();
        skills.insert(id.clone(), skill);

        info!(skill_id = %id, "WASM skill loaded successfully");
        Ok(id)
    }

    /// Load WASM from bytes (for embedded skills)
    pub fn load_bytes(&self, id: impl Into<String>, wasm_bytes: &[u8]) -> Result<String> {
        let id = id.into();

        info!(skill_id = %id, "Loading WASM skill from bytes");

        let module = Module::new(&self.engine, wasm_bytes)
            .map_err(|e| Error::Worker(format!("Failed to compile WASM: {}", e)))?;

        let skill = WasmSkill {
            id: id.clone(),
            name: id.clone(),
            module,
            engine: self.engine.clone(),
            max_memory_pages: self.default_max_memory_pages,
            timeout_secs: self.default_timeout_secs,
        };

        let mut skills = self.skills.lock().unwrap();
        skills.insert(id.clone(), skill);

        Ok(id)
    }

    /// Invoke a WASM skill
    pub async fn invoke(
        &self,
        _skill_id: &str,
        _input: SkillInput,
        _capabilities: Vec<String>,
    ) -> Result<SkillOutput> {
        // WASM skill execution temporarily disabled due to wasmtime-wasi v27 API changes
        // This will be re-enabled when the API is stabilized
        Err(Error::Worker(
            "WASM skill execution is not yet implemented".to_string(),
        ))
    }

    /// Unload a WASM skill
    pub fn unload(&self, skill_id: &str) -> Result<()> {
        let mut skills = self.skills.lock().unwrap();
        skills.remove(skill_id);
        info!(skill_id = %skill_id, "WASM skill unloaded");
        Ok(())
    }

    /// List loaded WASM skills
    pub fn list_loaded(&self) -> Vec<String> {
        let skills = self.skills.lock().unwrap();
        skills.keys().cloned().collect()
    }

    /// Check if a skill is loaded
    pub fn is_loaded(&self, skill_id: &str) -> bool {
        let skills = self.skills.lock().unwrap();
        skills.contains_key(skill_id)
    }

    /// Scan directory and load all WASM skills
    pub fn discover_and_load(&self, dir: &Path) -> Result<Vec<String>> {
        let mut loaded = Vec::new();

        if !dir.exists() {
            return Ok(loaded);
        }

        let entries = std::fs::read_dir(dir).map_err(|e| Error::Io(e))?;

        for entry in entries {
            let entry = entry.map_err(|e| Error::Io(e))?;
            let path = entry.path();

            if path.extension().map(|e| e == "wasm").unwrap_or(false) {
                let id = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                match self.load(&path, &id) {
                    Ok(_) => loaded.push(id),
                    Err(e) => {
                        warn!(path = %path.display(), error = %e, "Failed to load WASM skill");
                    }
                }
            }
        }

        Ok(loaded)
    }
}

impl Default for WasmSkillRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to create default WASM runtime")
    }
}

// Clone implementation for WasmSkill (without engine)
impl WasmSkill {
    fn clone_skill(&self) -> Self {
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            module: self.module.clone(),
            engine: self.engine.clone(),
            max_memory_pages: self.max_memory_pages,
            timeout_secs: self.timeout_secs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_wasm_runtime_new() {
        let runtime = WasmSkillRuntime::new();
        assert!(runtime.is_ok());
    }

    #[test]
    fn test_list_empty() {
        let runtime = WasmSkillRuntime::new().unwrap();
        assert!(runtime.list_loaded().is_empty());
    }
}
