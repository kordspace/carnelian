//! WASM Skill Runtime
//!
//! Sandboxed skill execution using wasmtime and WASI.
//! Provides capability-based security and resource limits.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use tracing::{info, warn};
use wasmtime::{Config, Engine, Linker, Module, Store};
use wasmtime_wasi::{
    WasiCtxBuilder,
    pipe::{MemoryInputPipe, MemoryOutputPipe},
    preview1::WasiP1Ctx,
};

use crate::skills::skill_trait::{SkillInput, SkillOutput};
use carnelian_common::{Error, Result};

/// State for WASM skill execution
pub struct WasmState {
    /// WASI context for system access
    wasi: WasiP1Ctx,

    /// Granted capabilities
    capabilities: Vec<String>,
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

        // Create linker with WASI
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::preview1::add_to_linker_async(&mut linker, |s: &mut WasmState| &mut s.wasi)
            .map_err(|e| Error::Worker(format!("Failed to add WASI to linker: {}", e)))?;

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
        skill_id: &str,
        input: SkillInput,
        capabilities: Vec<String>,
    ) -> Result<SkillOutput> {
        // Step 1: Look up the skill
        let skill = {
            let skills = self.skills.lock().unwrap();
            skills
                .get(skill_id)
                .cloned()
                .ok_or_else(|| Error::Worker(format!("WASM skill '{}' not found", skill_id)))?
        };

        // Step 2: Read output limit
        let max_output_bytes: usize = std::env::var("CARNELIAN_MAX_OUTPUT_BYTES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1_048_576); // 1 MB default

        // Step 3: Create I/O pipes
        let stdout_pipe = MemoryOutputPipe::new(max_output_bytes);
        let input_bytes = serde_json::to_vec(&input.params)
            .map_err(|e| Error::Worker(format!("Failed to serialize input: {}", e)))?;
        let stdin_pipe = MemoryInputPipe::new(input_bytes);

        // Step 4: Build WasiCtx with capability-based access
        let mut wasi_builder = WasiCtxBuilder::new();
        wasi_builder.stdin(stdin_pipe);
        wasi_builder.stdout(stdout_pipe.clone());

        // Network: deny by default, allow if capability granted
        if capabilities.contains(&"network".to_string()) {
            wasi_builder.inherit_network();
        }

        // Filesystem: allow read if capability granted
        if capabilities.contains(&"fs.read".to_string()) {
            wasi_builder
                .preopened_dir(
                    std::path::PathBuf::from("."),
                    ".",
                    wasmtime_wasi::DirPerms::READ,
                    wasmtime_wasi::FilePerms::READ,
                )
                .map_err(|e| Error::Worker(format!("Failed to preopen dir: {}", e)))?;
        }

        let wasi = wasi_builder.build_p1();

        // Step 5: Create Store with WasmState
        let mut store = Store::new(&skill.engine, WasmState { wasi, capabilities });

        // Step 6: Set epoch deadline for timeout
        store.set_epoch_deadline(skill.timeout_secs);

        // Step 7: Spawn epoch ticker for timeout enforcement
        let engine_clone = skill.engine.clone();
        let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel();
        let epoch_ticker = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        engine_clone.increment_epoch();
                    }
                    _ = &mut cancel_rx => {
                        break;
                    }
                }
            }
        });

        // Step 8: Instantiate and call the module
        let result: Result<()> = async {
            let instance = self
                .linker
                .instantiate_async(&mut store, &skill.module)
                .await
                .map_err(|e| Error::Worker(format!("Failed to instantiate WASM: {}", e)))?;

            // Try "invoke" first, fall back to "_start"
            let func = instance
                .get_typed_func::<(), ()>(&mut store, "invoke")
                .or_else(|_| instance.get_typed_func::<(), ()>(&mut store, "_start"))
                .map_err(|e| {
                    Error::Worker(format!("No 'invoke' or '_start' function found: {}", e))
                })?;

            func.call_async(&mut store, ())
                .await
                .map_err(|e| Error::Worker(format!("WASM execution failed: {}", e)))?;

            Ok(())
        }
        .await;

        // Step 9: Cancel epoch ticker
        let _ = cancel_tx.send(());
        let _ = epoch_ticker.await;

        // Step 10: Capture stdout and check truncation
        let output_bytes = stdout_pipe.contents();
        let truncated = output_bytes.len() >= max_output_bytes;

        // Step 11: Parse output
        let output_str = String::from_utf8_lossy(&output_bytes);
        let data = serde_json::from_slice(&output_bytes).unwrap_or_else(|_| {
            serde_json::json!({
                "output": output_str.to_string()
            })
        });

        // Step 12: Build result
        let mut metadata = HashMap::new();
        if truncated {
            metadata.insert("truncated".to_string(), "true".to_string());
        }

        match result {
            Ok(_) => Ok(SkillOutput {
                success: true,
                data,
                error: None,
                metadata: metadata.clone(),
            }),
            Err(e) => Ok(SkillOutput {
                success: false,
                data: serde_json::json!({}),
                error: Some(e.to_string()),
                metadata,
            }),
        }
    }

    /// Unload a WASM skill
    pub fn unload(&self, skill_id: &str) -> Result<()> {
        let mut skills = self.skills.lock().unwrap();
        skills.remove(skill_id);
        info!(skill_id = %skill_id, "WASM skill unloaded");
        Ok(())
    }

    /// Get a loaded skill by ID
    pub fn get_skill(&self, skill_id: &str) -> Option<WasmSkill> {
        let skills = self.skills.lock().unwrap();
        skills.get(skill_id).cloned()
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

// Clone implementation for WasmSkill
impl Clone for WasmSkill {
    fn clone(&self) -> Self {
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
