//! Skill Discovery and Registry Management
//!
//! This module provides automatic and manual skill discovery from the filesystem.
//! Skills are defined by `skill.json` manifest files in the registry directory.
//!
//! ## Discovery Modes
//!
//! - **Automatic**: File watcher monitors the registry directory for changes (2s debounce)
//! - **Manual**: CLI command or REST API triggers a full registry scan
//!
//! ## Manifest Format
//!
//! Each skill directory contains a `skill.json` file:
//!
//! ```text
//! skills/registry/
//! ├── healthcheck/
//! │   └── skill.json
//! ├── echo/
//! │   └── skill.json
//! └── ...
//! ```

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use carnelian_common::types::{EventEnvelope, EventLevel, EventType, SkillRefreshResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;

use crate::events::EventStream;

// =============================================================================
// MANIFEST SCHEMA
// =============================================================================

/// Skill manifest loaded from `skill.json`.
///
/// Contains OpenClaw-compatible base fields and Carnelian-specific extensions
/// for sandbox configuration, capability requirements, and versioning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    /// Unique skill name (used as database key)
    pub name: String,

    /// Human-readable description
    pub description: String,

    /// Worker runtime: node, python, shell, or wasm
    pub runtime: String,

    /// Semantic version string
    #[serde(default = "default_version")]
    pub version: String,

    /// Capability keys required for execution
    #[serde(default)]
    pub capabilities_required: Vec<String>,

    /// Optional homepage or documentation URL
    #[serde(default)]
    pub homepage: Option<String>,

    /// Sandbox configuration for isolated execution
    #[serde(default)]
    pub sandbox: Option<SandboxConfig>,

    /// OpenClaw compatibility metadata
    #[serde(default)]
    pub openclaw_compat: Option<OpenClawMetadata>,
}

fn default_version() -> String {
    "0.1.0".to_string()
}

/// Sandbox configuration for skill execution isolation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Filesystem mounts available to the skill
    #[serde(default)]
    pub mounts: Vec<MountConfig>,

    /// Network access policy: enabled, disabled, or restricted
    #[serde(default = "default_network_policy")]
    pub network: String,

    /// Maximum memory in MB (0 = unlimited)
    #[serde(default)]
    pub max_memory_mb: u64,

    /// Maximum CPU percentage (0 = unlimited)
    #[serde(default)]
    pub max_cpu_percent: u64,

    /// Environment variables injected into the worker
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

fn default_network_policy() -> String {
    "enabled".to_string()
}

/// A single filesystem mount for sandbox configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountConfig {
    /// Host path
    pub host: String,
    /// Container/sandbox path
    pub container: String,
    /// Whether the mount is read-only
    #[serde(default)]
    pub readonly: bool,
}

/// OpenClaw compatibility metadata for gradual migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawMetadata {
    /// Display emoji for the skill
    #[serde(default)]
    pub emoji: Option<String>,

    /// Required system binaries
    #[serde(default)]
    pub requires: Option<OpenClawRequires>,

    /// Categorization tags
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Required binaries for OpenClaw compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawRequires {
    /// Binary names that must be available on PATH
    #[serde(default)]
    pub bins: Vec<String>,
}

// =============================================================================
// MANIFEST VALIDATION
// =============================================================================

/// Validation errors for skill manifests.
#[derive(Debug, Clone)]
pub struct ManifestValidationError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ManifestValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

const VALID_RUNTIMES: &[&str] = &["node", "python", "shell", "wasm"];
const VALID_NETWORK_POLICIES: &[&str] = &["enabled", "disabled", "restricted"];

/// Validate a skill manifest, returning a list of errors (empty = valid).
pub fn validate_manifest(manifest: &SkillManifest) -> Vec<ManifestValidationError> {
    let mut errors = Vec::new();

    if manifest.name.is_empty() {
        errors.push(ManifestValidationError {
            field: "name".to_string(),
            message: "name is required".to_string(),
        });
    }

    if manifest.description.is_empty() {
        errors.push(ManifestValidationError {
            field: "description".to_string(),
            message: "description is required".to_string(),
        });
    }

    if !VALID_RUNTIMES.contains(&manifest.runtime.as_str()) {
        errors.push(ManifestValidationError {
            field: "runtime".to_string(),
            message: format!(
                "invalid runtime '{}', must be one of: {}",
                manifest.runtime,
                VALID_RUNTIMES.join(", ")
            ),
        });
    }

    if let Some(sandbox) = &manifest.sandbox {
        if !VALID_NETWORK_POLICIES.contains(&sandbox.network.as_str()) {
            errors.push(ManifestValidationError {
                field: "sandbox.network".to_string(),
                message: format!(
                    "invalid network policy '{}', must be one of: {}",
                    sandbox.network,
                    VALID_NETWORK_POLICIES.join(", ")
                ),
            });
        }
    }

    errors
}

// =============================================================================
// CHECKSUM COMPUTATION
// =============================================================================

/// Compute a blake3 checksum of a skill manifest.
///
/// The manifest is serialized to canonical JSON (sorted keys via `BTreeMap`)
/// before hashing to ensure deterministic output regardless of field order.
pub fn compute_manifest_checksum(manifest: &SkillManifest) -> String {
    // Serialize to serde_json::Value, then to sorted string for canonical form
    let value = serde_json::to_value(manifest).unwrap_or_default();
    let canonical = canonical_json(&value);
    let hash = blake3::hash(canonical.as_bytes());
    hash.to_hex().to_string()
}

/// Produce canonical JSON with sorted keys for deterministic hashing.
fn canonical_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let sorted: BTreeMap<_, _> = map.iter().collect();
            let entries: Vec<String> = sorted
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(k).unwrap(),
                        canonical_json(v)
                    )
                })
                .collect();
            format!("{{{}}}", entries.join(","))
        }
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(canonical_json).collect();
            format!("[{}]", items.join(","))
        }
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

// =============================================================================
// SKILL DISCOVERY
// =============================================================================

/// Skill discovery engine that scans the registry directory, validates manifests,
/// and synchronizes the database.
pub struct SkillDiscovery {
    pool: PgPool,
    event_stream: Option<Arc<EventStream>>,
    registry_path: PathBuf,
}

impl SkillDiscovery {
    /// Create a new discovery instance.
    ///
    /// `event_stream` is optional — CLI usage may not have one.
    pub fn new(
        pool: PgPool,
        event_stream: Option<Arc<EventStream>>,
        registry_path: PathBuf,
    ) -> Self {
        Self {
            pool,
            event_stream,
            registry_path,
        }
    }

    /// Perform a full registry scan and return refresh counts.
    ///
    /// 1. Walk the registry directory for `skill.json` files
    /// 2. Parse and validate each manifest
    /// 3. Upsert valid skills into the database
    /// 4. Remove skills whose manifests no longer exist on disk
    /// 5. Emit events for each change
    pub async fn refresh(&self) -> carnelian_common::Result<SkillRefreshResponse> {
        let scan = self.scan_registry().await?;

        let mut discovered: u32 = 0;
        let mut updated: u32 = 0;
        // Collect names of all valid manifests found on disk
        let mut found_names: HashSet<String> = HashSet::new();

        for (path, manifest) in &scan.manifests {
            found_names.insert(manifest.name.clone());

            match self.upsert_skill(manifest).await {
                Ok(UpsertResult::Inserted) => {
                    tracing::info!(
                        skill = %manifest.name,
                        runtime = %manifest.runtime,
                        path = %path.display(),
                        "Skill discovered"
                    );
                    discovered += 1;
                }
                Ok(UpsertResult::Updated) => {
                    tracing::info!(
                        skill = %manifest.name,
                        runtime = %manifest.runtime,
                        path = %path.display(),
                        "Skill updated"
                    );
                    updated += 1;
                }
                Ok(UpsertResult::Unchanged) => {
                    tracing::debug!(skill = %manifest.name, "Skill unchanged");
                }
                Err(e) => {
                    tracing::warn!(
                        skill = %manifest.name,
                        error = %e,
                        path = %path.display(),
                        "Failed to upsert skill"
                    );
                }
            }
        }

        // Only remove stale skills when the scan actually ran successfully.
        // If the registry directory was missing or unreadable the scan is
        // skipped and we must NOT delete existing skills from the database.
        let removed = if scan.skipped {
            tracing::debug!("Scan was skipped, not removing stale skills");
            0
        } else {
            self.remove_stale_skills(&found_names).await?
        };

        tracing::info!(
            discovered = discovered,
            updated = updated,
            removed = removed,
            total_manifests = scan.manifests.len(),
            "Skill registry refresh complete"
        );

        Ok(SkillRefreshResponse {
            discovered,
            updated,
            removed,
        })
    }

    /// Walk the registry directory and parse all valid `skill.json` files.
    ///
    /// Returns a `ScanResult` whose `skipped` flag is `true` when the
    /// registry directory does not exist or is unreadable.  Callers must
    /// check this flag before removing stale skills from the database.
    async fn scan_registry(&self) -> carnelian_common::Result<ScanResult> {
        let mut results = Vec::new();

        if !self.registry_path.exists() {
            tracing::debug!(
                path = %self.registry_path.display(),
                "Skills registry directory does not exist, skipping scan"
            );
            return Ok(ScanResult {
                manifests: results,
                skipped: true,
            });
        }

        let entries = match std::fs::read_dir(&self.registry_path) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!(
                    path = %self.registry_path.display(),
                    error = %e,
                    "Failed to read skills registry directory"
                );
                return Ok(ScanResult {
                    manifests: results,
                    skipped: true,
                });
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to read directory entry");
                    continue;
                }
            };

            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("skill.json");
            if !manifest_path.exists() {
                continue;
            }

            match self.parse_manifest(&manifest_path).await {
                Ok(manifest) => {
                    let errors = validate_manifest(&manifest);
                    if errors.is_empty() {
                        results.push((manifest_path, manifest));
                    } else {
                        for err in &errors {
                            tracing::warn!(
                                path = %manifest_path.display(),
                                field = %err.field,
                                error = %err.message,
                                "Skill manifest validation failed"
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        path = %manifest_path.display(),
                        error = %e,
                        "Failed to parse skill manifest"
                    );
                }
            }
        }

        Ok(ScanResult {
            manifests: results,
            skipped: false,
        })
    }

    /// Parse a single `skill.json` file.
    async fn parse_manifest(&self, path: &Path) -> carnelian_common::Result<SkillManifest> {
        let content = tokio::fs::read_to_string(path).await.map_err(|e| {
            carnelian_common::Error::Config(format!("Failed to read {}: {}", path.display(), e))
        })?;

        serde_json::from_str(&content).map_err(|e| {
            carnelian_common::Error::Config(format!("Failed to parse {}: {}", path.display(), e))
        })
    }

    /// Insert or update a skill in the database.
    ///
    /// Returns whether the skill was inserted, updated, or unchanged.
    async fn upsert_skill(
        &self,
        manifest: &SkillManifest,
    ) -> carnelian_common::Result<UpsertResult> {
        let checksum = compute_manifest_checksum(manifest);
        let manifest_json = serde_json::to_value(manifest).unwrap_or_default();
        let capabilities: Vec<String> = manifest.capabilities_required.clone();

        // Check if skill already exists
        let existing: Option<(String, Option<String>)> =
            sqlx::query_as("SELECT runtime, checksum FROM skills WHERE name = $1")
                .bind(&manifest.name)
                .fetch_optional(&self.pool)
                .await?;

        match existing {
            Some((_, Some(ref existing_checksum))) if existing_checksum == &checksum => {
                // Checksum unchanged — no update needed
                Ok(UpsertResult::Unchanged)
            }
            Some(_) => {
                // Skill exists but checksum differs — update
                sqlx::query(
                    r"UPDATE skills
                      SET description = $1,
                          runtime = $2,
                          manifest = $3,
                          capabilities_required = $4,
                          checksum = $5,
                          enabled = true,
                          updated_at = NOW()
                      WHERE name = $6",
                )
                .bind(&manifest.description)
                .bind(&manifest.runtime)
                .bind(&manifest_json)
                .bind(&capabilities)
                .bind(&checksum)
                .bind(&manifest.name)
                .execute(&self.pool)
                .await?;

                self.emit_event(
                    EventType::SkillUpdated,
                    json!({
                        "name": manifest.name,
                        "runtime": manifest.runtime,
                        "version": manifest.version,
                        "checksum": checksum,
                    }),
                );

                Ok(UpsertResult::Updated)
            }
            None => {
                // New skill — insert
                sqlx::query(
                    r"INSERT INTO skills (name, description, runtime, manifest, capabilities_required, checksum)
                      VALUES ($1, $2, $3, $4, $5, $6)",
                )
                .bind(&manifest.name)
                .bind(&manifest.description)
                .bind(&manifest.runtime)
                .bind(&manifest_json)
                .bind(&capabilities)
                .bind(&checksum)
                .execute(&self.pool)
                .await?;

                self.emit_event(
                    EventType::SkillDiscovered,
                    json!({
                        "name": manifest.name,
                        "runtime": manifest.runtime,
                        "version": manifest.version,
                        "checksum": checksum,
                    }),
                );

                Ok(UpsertResult::Inserted)
            }
        }
    }

    /// Remove skills from the database that are no longer present on disk.
    async fn remove_stale_skills(
        &self,
        found_names: &HashSet<String>,
    ) -> carnelian_common::Result<u32> {
        // Fetch all skill names currently in the database
        let db_names: Vec<String> = sqlx::query_scalar("SELECT name FROM skills")
            .fetch_all(&self.pool)
            .await?;

        let mut removed: u32 = 0;

        for name in &db_names {
            if !found_names.contains(name) {
                sqlx::query("DELETE FROM skills WHERE name = $1")
                    .bind(name)
                    .execute(&self.pool)
                    .await?;

                self.emit_event(EventType::SkillRemoved, json!({ "name": name }));

                tracing::info!(skill = %name, "Skill removed (manifest no longer on disk)");
                removed += 1;
            }
        }

        Ok(removed)
    }

    /// Emit an event to the event stream (if available).
    fn emit_event(&self, event_type: EventType, payload: serde_json::Value) {
        if let Some(ref es) = self.event_stream {
            es.publish(EventEnvelope::new(EventLevel::Info, event_type, payload));
        }
    }
}

/// Result of an upsert operation.
enum UpsertResult {
    Inserted,
    Updated,
    Unchanged,
}

/// Result of a registry scan.
///
/// When `skipped` is `true` the registry directory was missing or unreadable
/// and `manifests` will be empty.  Callers must not use an empty manifest list
/// from a skipped scan to remove existing skills from the database.
struct ScanResult {
    manifests: Vec<(PathBuf, SkillManifest)>,
    skipped: bool,
}

// =============================================================================
// FILE WATCHER
// =============================================================================

/// Start a background file watcher on the skills registry directory.
///
/// Uses `notify-debouncer-mini` with a 2-second debounce to batch rapid
/// filesystem changes. When changes are detected, triggers a full registry
/// scan via `SkillDiscovery::refresh()`.
///
/// Returns a `JoinHandle` for the background task. The watcher runs until
/// the handle is aborted or the process exits.
pub fn start_file_watcher(
    pool: PgPool,
    event_stream: Arc<EventStream>,
    registry_path: PathBuf,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        use notify_debouncer_mini::{DebouncedEventKind, new_debouncer};

        let (tx, mut rx) = tokio::sync::mpsc::channel(16);

        let debounce_duration = Duration::from_secs(2);

        // Create debouncer that sends events to our async channel
        let mut debouncer = match new_debouncer(
            debounce_duration,
            move |events: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
                match events {
                    Ok(evts) => {
                        // Filter for skill.json changes only
                        let dominated_by_skill_json = evts.iter().any(|e| {
                            e.path.file_name().is_some_and(|f| f == "skill.json")
                                || e.kind == DebouncedEventKind::Any
                        });
                        if dominated_by_skill_json {
                            let _ = tx.blocking_send(());
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "File watcher error");
                    }
                }
            },
        ) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!(error = %e, "Failed to create file watcher");
                return;
            }
        };

        // Watch the registry path recursively
        let watch_path = registry_path.clone();
        if let Err(e) = debouncer
            .watcher()
            .watch(&watch_path, notify::RecursiveMode::Recursive)
        {
            tracing::error!(
                path = %watch_path.display(),
                error = %e,
                "Failed to watch skills registry directory"
            );
            return;
        }

        tracing::info!(
            path = %registry_path.display(),
            "File watcher started for skills registry"
        );

        // Process debounced events
        while rx.recv().await.is_some() {
            tracing::debug!("File watcher triggered, refreshing skill registry");
            let discovery = SkillDiscovery::new(
                pool.clone(),
                Some(event_stream.clone()),
                registry_path.clone(),
            );
            match discovery.refresh().await {
                Ok(result) => {
                    if result.discovered > 0 || result.updated > 0 || result.removed > 0 {
                        tracing::info!(
                            discovered = result.discovered,
                            updated = result.updated,
                            removed = result.removed,
                            "File watcher: skill registry updated"
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "File watcher: skill refresh failed");
                }
            }
        }

        tracing::debug!("File watcher stopped");
    })
}
