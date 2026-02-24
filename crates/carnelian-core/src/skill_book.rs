//! Skill Book Library — curated skill catalog with activation management.
//!
//! Provides:
//! - Loading and parsing skill-book.json manifests
//! - Checking activation status against skills registry
//! - Activating skills (copy to registry + store config)
//! - Deactivating skills

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::Config;

/// Skill Book manifest structure (skill-book.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillBookManifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub runtime: String,
    pub version: String,
    #[serde(default)]
    pub required_config: Vec<SkillConfigField>,
}

/// Configuration field in manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillConfigField {
    pub key: String,
    pub label: String,
    #[serde(default)]
    pub secret: bool,
}

/// Categories definition (categories.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoriesDefinition {
    pub categories: Vec<CategoryInfo>,
}

/// Category metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
}

/// Skill Book manager.
pub struct SkillBook {
    skill_book_path: PathBuf,
    registry_path: PathBuf,
    config: std::sync::Arc<Config>,
}

impl SkillBook {
    /// Create a new SkillBook instance.
    pub fn new(skill_book_path: PathBuf, registry_path: PathBuf, config: std::sync::Arc<Config>) -> Self {
        Self {
            skill_book_path,
            registry_path,
            config,
        }
    }

    /// Load the categories definition.
    pub fn load_categories(&self) -> carnelian_common::Result<Vec<CategoryInfo>> {
        let categories_path = self.skill_book_path.join("categories.json");
        if !categories_path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&categories_path)
            .map_err(|e| carnelian_common::Error::Config(format!("Failed to read categories.json: {}", e)))?;
        
        let definition: CategoriesDefinition = serde_json::from_str(&content)
            .map_err(|e| carnelian_common::Error::Config(format!("Failed to parse categories.json: {}", e)))?;
        
        Ok(definition.categories)
    }

    /// Load the full skill catalog with activation status.
    pub fn load_catalog(&self) -> carnelian_common::Result<carnelian_common::types::SkillBookCatalog> {
        let categories = self.load_categories()?;
        let category_ids: Vec<String> = categories.iter().map(|c| c.id.clone()).collect();
        
        let mut skills = Vec::new();
        
        // Iterate through skill directories
        if let Ok(entries) = std::fs::read_dir(&self.skill_book_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let manifest_path = path.join("skill-book.json");
                    if let Ok(manifest) = self.load_manifest(&manifest_path) {
                        let activated = self.check_activated(&manifest.id);
                        
                        let required_config = manifest.required_config.iter().map(|f| {
                            carnelian_common::types::SkillConfigField {
                                key: f.key.clone(),
                                label: f.label.clone(),
                                secret: f.secret,
                            }
                        }).collect();
                        
                        skills.push(carnelian_common::types::SkillBookEntry {
                            id: manifest.id,
                            name: manifest.name,
                            description: manifest.description,
                            category: manifest.category,
                            runtime: manifest.runtime,
                            version: manifest.version,
                            required_config,
                            activated,
                        });
                    }
                }
            }
        }
        
        Ok(carnelian_common::types::SkillBookCatalog {
            skills,
            categories: category_ids,
        })
    }

    /// Load a single skill manifest.
    fn load_manifest(&self, path: &Path) -> carnelian_common::Result<SkillBookManifest> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| carnelian_common::Error::Config(format!("Failed to read manifest: {}", e)))?;
        
        let manifest: SkillBookManifest = serde_json::from_str(&content)
            .map_err(|e| carnelian_common::Error::Config(format!("Failed to parse manifest: {}", e)))?;
        
        Ok(manifest)
    }

    /// Check if a skill is activated (exists in registry).
    fn check_activated(&self, skill_id: &str) -> bool {
        let registry_skill_path = self.registry_path.join(skill_id);
        registry_skill_path.exists()
    }

    /// Get a single skill entry.
    pub fn get_entry(&self, skill_id: &str) -> carnelian_common::Result<carnelian_common::types::SkillBookEntry> {
        let skill_path = self.skill_book_path.join(skill_id);
        let manifest_path = skill_path.join("skill-book.json");
        
        if !manifest_path.exists() {
            return Err(carnelian_common::Error::Validation(
                format!("Skill '{}' not found in Skill Book", skill_id)
            ));
        }
        
        let manifest = self.load_manifest(&manifest_path)?;
        let activated = self.check_activated(skill_id);
        
        let required_config = manifest.required_config.iter().map(|f| {
            carnelian_common::types::SkillConfigField {
                key: f.key.clone(),
                label: f.label.clone(),
                secret: f.secret,
            }
        }).collect();
        
        Ok(carnelian_common::types::SkillBookEntry {
            id: manifest.id,
            name: manifest.name,
            description: manifest.description,
            category: manifest.category,
            runtime: manifest.runtime,
            version: manifest.version,
            required_config,
            activated,
        })
    }

    /// Activate a skill — copy to registry and store config.
    pub async fn activate(
        &self,
        skill_id: &str,
        config_values: HashMap<String, String>,
    ) -> carnelian_common::Result<carnelian_common::types::ActivateSkillResponse> {
        let skill_path = self.skill_book_path.join(skill_id);
        let manifest_path = skill_path.join("skill-book.json");
        
        if !manifest_path.exists() {
            return Err(carnelian_common::Error::Validation(
                format!("Skill '{}' not found in Skill Book", skill_id)
            ));
        }
        
        let manifest = self.load_manifest(&manifest_path)?;
        
        // Verify all required config is provided
        for field in &manifest.required_config {
            if !config_values.contains_key(&field.key) {
                return Err(carnelian_common::Error::Validation(
                    format!("Missing required config: {}", field.key)
                ));
            }
        }
        
        // Create target directory in registry
        let target_path = self.registry_path.join(skill_id);
        if !target_path.exists() {
            std::fs::create_dir_all(&target_path)
                .map_err(|e| carnelian_common::Error::Config(format!("Failed to create registry dir: {}", e)))?;
        }
        
        // Copy manifest as skill.json
        let skill_json_path = target_path.join("skill.json");
        let skill_content = serde_json::json!({
            "id": manifest.id,
            "name": manifest.name,
            "description": manifest.description,
            "runtime": manifest.runtime,
            "version": manifest.version,
            "capabilities": ["*"], // Full capabilities for skill-book skills
        });
        
        std::fs::write(&skill_json_path, skill_content.to_string())
            .map_err(|e| carnelian_common::Error::Config(format!("Failed to write skill.json: {}", e)))?;
        
        // Store config values in database
        if let Ok(pool) = self.config.pool() {
            for (key, value) in &config_values {
                let full_key = format!("skill.{}.config.{}", skill_id, key);
                let value_json = serde_json::json!(value);
                
                sqlx::query(
                    "INSERT INTO config_store (key, value) VALUES ($1, $2) ON CONFLICT (key) DO UPDATE SET value = $2",
                )
                .bind(&full_key)
                .bind(&value_json)
                .execute(pool)
                .await
                .map_err(|e| carnelian_common::Error::Database(e))?;
            }
        }
        
        Ok(carnelian_common::types::ActivateSkillResponse {
            skill_id: skill_id.to_string(),
            activated: true,
        })
    }

    /// Deactivate a skill — remove from registry and optionally purge config.
    pub async fn deactivate(&self, skill_id: &str) -> carnelian_common::Result<carnelian_common::types::DeactivateSkillResponse> {
        let target_path = self.registry_path.join(skill_id);
        
        if target_path.exists() {
            std::fs::remove_dir_all(&target_path)
                .map_err(|e| carnelian_common::Error::Config(format!("Failed to remove skill dir: {}", e)))?;
        }
        
        // Purge config keys from database
        if let Ok(pool) = self.config.pool() {
            let pattern = format!("skill.{}.config.%", skill_id);
            sqlx::query("DELETE FROM config_store WHERE key LIKE $1")
                .bind(&pattern)
                .execute(pool)
                .await
                .map_err(|e| carnelian_common::Error::Database(e))?;
        }
        
        Ok(carnelian_common::types::DeactivateSkillResponse {
            skill_id: skill_id.to_string(),
            deactivated: true,
        })
    }
}
