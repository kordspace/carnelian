//! Native Skill Loader
//!
//! Loads Rust skills compiled as dynamic libraries (.so/.dll/.dylib)
//! using the `libloading` crate for safe dynamic linking.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use libloading::{Library, Symbol};
use tracing::{info, warn, error};

use crate::skills::skill_trait::{BoxedSkill, SkillFactory, SkillMetadata};
use crate::skills::SkillManifest;
use carnelian_common::{Error, Result};

/// Handle to a loaded native skill library
pub struct NativeSkillHandle {
    /// The loaded library (kept alive for the duration)
    #[allow(dead_code)]
    library: Library,
    
    /// The skill instance
    skill: BoxedSkill,
    
    /// Path to the library file
    path: PathBuf,
    
    /// Last modified time for hot-reload detection
    modified: std::time::SystemTime,
}

/// Loader for native Rust skills compiled as dynamic libraries
pub struct NativeSkillLoader {
    /// Registry of loaded skills
    registry: Arc<RwLock<HashMap<String, NativeSkillHandle>>>,
    
    /// Search paths for native skills
    search_paths: Vec<PathBuf>,
}

impl NativeSkillLoader {
    /// Create a new native skill loader
    pub fn new() -> Self {
        Self {
            registry: Arc::new(RwLock::new(HashMap::new())),
            search_paths: vec![PathBuf::from("./skills/native")],
        }
    }
    
    /// Add a search path for native skills
    pub fn add_search_path(&mut self, path: impl Into<PathBuf>) {
        self.search_paths.push(path.into());
    }
    
    /// Load a skill from a library file
    ///
    /// # Safety
    /// This uses unsafe code internally for FFI. The skill library
    /// must be compiled with a compatible ABI.
    #[allow(unsafe_code)]
    pub fn load(&self, path: &Path) -> Result<String> {
        let skill_id = path.file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| Error::Config("Invalid skill path".to_string()))?
            .to_string();
        
        // Check if already loaded
        {
            let registry = self.registry.read().unwrap();
            if registry.contains_key(&skill_id) {
                info!(skill_id = %skill_id, "Skill already loaded, skipping");
                return Ok(skill_id);
            }
        }
        
        // Get file modified time for hot-reload tracking
        let modified = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| std::time::SystemTime::now());
        
        // Load the library
        info!(path = %path.display(), "Loading native skill library");
        
        unsafe {
            let library = Library::new(path)
                .map_err(|e| Error::Worker(format!("Failed to load skill library: {}", e)))?;
            
            // Look for the skill factory symbol
            let factory: Symbol<SkillFactory> = library
                .get(b"_carnelian_skill_create")
                .map_err(|e| Error::Worker(format!("Missing skill factory symbol: {}", e)))?;
            
            // Create the skill instance
            let skill = factory();
            let manifest = skill.manifest();
            let loaded_skill_id = manifest.name.clone();
            
            // Verify skill ID matches filename
            if loaded_skill_id != skill_id {
                warn!(expected = %skill_id, actual = %loaded_skill_id, "Skill ID mismatch");
            }
            
            // Store the handle
            let handle = NativeSkillHandle {
                library,
                skill,
                path: path.to_path_buf(),
                modified,
            };
            
            let mut registry = self.registry.write().unwrap();
            registry.insert(loaded_skill_id.clone(), handle);
            
            info!(skill_id = %loaded_skill_id, "Native skill loaded successfully");
            Ok(loaded_skill_id)
        }
    }
    
    /// Unload a skill
    pub fn unload(&self, skill_id: &str) -> Result<()> {
        let mut registry = self.registry.write().unwrap();
        
        if let Some(mut handle) = registry.remove(skill_id) {
            // Call shutdown on the skill
            // Note: We can't easily call async shutdown here
            info!(skill_id = %skill_id, "Native skill unloaded");
        }
        
        Ok(())
    }
    
    /// Get a reference to a loaded skill
    pub fn get(&self, skill_id: &str) -> Result<&BoxedSkill> {
        let registry = self.registry.read().unwrap();
        
        let handle = registry.get(skill_id)
            .ok_or_else(|| Error::Worker(format!("Skill not loaded: {}", skill_id)))?;
        
        // Return reference to the boxed skill
        Ok(&handle.skill)
    }
    
    /// Scan search paths and load all native skills
    pub fn discover_and_load(&self) -> Result<Vec<String>> {
        let mut loaded = Vec::new();
        
        for search_path in &self.search_paths {
            if !search_path.exists() {
                continue;
            }
            
            let entries = std::fs::read_dir(search_path)
                .map_err(|e| Error::Io(e))?;
            
            for entry in entries {
                let entry = entry.map_err(|e| Error::Io(e))?;
                let path = entry.path();
                
                // Look for library files
                if path.is_file() {
                    let ext = path.extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    
                    #[cfg(target_os = "linux")]
                    let is_lib = ext == "so";
                    #[cfg(target_os = "macos")]
                    let is_lib = ext == "dylib";
                    #[cfg(target_os = "windows")]
                    let is_lib = ext == "dll";
                    
                    if is_lib {
                        match self.load(&path) {
                            Ok(id) => loaded.push(id),
                            Err(e) => {
                                warn!(path = %path.display(), error = %e, "Failed to load skill");
                            }
                        }
                    }
                }
            }
        }
        
        Ok(loaded)
    }
    
    /// List all loaded native skills
    pub fn list_loaded(&self) -> Vec<String> {
        let registry = self.registry.read().unwrap();
        registry.keys().cloned().collect()
    }
    
    /// Check if a skill is loaded
    pub fn is_loaded(&self, skill_id: &str) -> bool {
        let registry = self.registry.read().unwrap();
        registry.contains_key(skill_id)
    }
    
    /// Hot-reload modified skills
    ///
    /// Scans loaded skills and reloads any that have been modified
    pub fn hot_reload(&self) -> Result<Vec<String>> {
        let mut reloaded = Vec::new();
        let registry = self.registry.read().unwrap();
        
        for (skill_id, handle) in registry.iter() {
            let current_modified = std::fs::metadata(&handle.path)
                .and_then(|m| m.modified())
                .unwrap_or_else(|_| std::time::SystemTime::now());
            
            if current_modified > handle.modified {
                info!(skill_id = %skill_id, "Skill modified, needs reload");
                reloaded.push(skill_id.clone());
            }
        }
        
        drop(registry);
        
        // Reload identified skills
        for skill_id in &reloaded {
            if let Err(e) = self.reload(skill_id) {
                warn!(skill_id = %skill_id, error = %e, "Hot-reload failed");
            }
        }
        
        Ok(reloaded)
    }
    
    /// Reload a skill
    fn reload(&self, skill_id: &str) -> Result<()> {
        // Get the path before unloading
        let path = {
            let registry = self.registry.read().unwrap();
            registry.get(skill_id)
                .map(|h| h.path.clone())
                .ok_or_else(|| Error::Worker(format!("Skill not loaded: {}", skill_id)))?
        };
        
        // Unload and reload
        self.unload(skill_id)?;
        self.load(&path)?;
        
        info!(skill_id = %skill_id, "Skill reloaded successfully");
        Ok(())
    }
}

impl Default for NativeSkillLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_native_loader_new() {
        let loader = NativeSkillLoader::new();
        assert!(loader.list_loaded().is_empty());
    }
    
    #[test]
    fn test_add_search_path() {
        let mut loader = NativeSkillLoader::new();
        loader.add_search_path("/test/path");
        assert_eq!(loader.search_paths.len(), 2);
    }
}
