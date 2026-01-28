//! Macro Registry
//!
//! Loads and stores macro definitions from YAML files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::{debug, info, warn};

use super::schema::{MacroKind, MacroSchema};

/// Registry of all loaded macros
#[derive(Debug, Clone)]
pub struct MacroRegistry {
    /// Macros by fully qualified name (e.g., "structure.setup")
    macros: HashMap<String, MacroSchema>,

    /// Index by operator domain (e.g., "structure" → ["structure.setup", "structure.assign-role"])
    by_domain: HashMap<String, Vec<String>>,

    /// Index by mode tag (e.g., "kyc" → ["case.open", "case.approve"])
    by_mode_tag: HashMap<String, Vec<String>>,

    /// Source files loaded
    source_files: Vec<PathBuf>,
}

impl MacroRegistry {
    /// Create empty registry
    pub fn new() -> Self {
        Self {
            macros: HashMap::new(),
            by_domain: HashMap::new(),
            by_mode_tag: HashMap::new(),
            source_files: Vec::new(),
        }
    }

    /// Get macro by FQN (e.g., "structure.setup")
    pub fn get(&self, fqn: &str) -> Option<&MacroSchema> {
        self.macros.get(fqn)
    }

    /// Check if a macro exists
    pub fn has(&self, fqn: &str) -> bool {
        self.macros.contains_key(fqn)
    }

    /// List all macro FQNs
    pub fn all_fqns(&self) -> impl Iterator<Item = &String> {
        self.macros.keys()
    }

    /// List all macros
    pub fn all(&self) -> impl Iterator<Item = (&String, &MacroSchema)> {
        self.macros.iter()
    }

    /// Count of macros
    pub fn len(&self) -> usize {
        self.macros.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.macros.is_empty()
    }

    /// Get macros by operator domain
    pub fn by_domain(&self, domain: &str) -> Vec<&MacroSchema> {
        self.by_domain
            .get(domain)
            .map(|fqns| fqns.iter().filter_map(|fqn| self.macros.get(fqn)).collect())
            .unwrap_or_default()
    }

    /// Get macros by mode tag
    pub fn by_mode_tag(&self, tag: &str) -> Vec<&MacroSchema> {
        self.by_mode_tag
            .get(tag)
            .map(|fqns| fqns.iter().filter_map(|fqn| self.macros.get(fqn)).collect())
            .unwrap_or_default()
    }

    /// Get all operator domains
    pub fn domains(&self) -> impl Iterator<Item = &String> {
        self.by_domain.keys()
    }

    /// Get all mode tags
    pub fn mode_tags(&self) -> impl Iterator<Item = &String> {
        self.by_mode_tag.keys()
    }

    /// Add a macro to the registry
    pub fn add(&mut self, fqn: String, schema: MacroSchema) {
        // Index by domain
        if let Some(domain) = &schema.routing.operator_domain {
            self.by_domain
                .entry(domain.clone())
                .or_default()
                .push(fqn.clone());
        }

        // Index by mode tags
        for tag in &schema.routing.mode_tags {
            self.by_mode_tag
                .entry(tag.clone())
                .or_default()
                .push(fqn.clone());
        }

        self.macros.insert(fqn, schema);
    }

    /// Merge another registry into this one
    pub fn merge(&mut self, other: MacroRegistry) {
        for (fqn, schema) in other.macros {
            self.add(fqn, schema);
        }
        self.source_files.extend(other.source_files);
    }

    /// Get source files that were loaded
    pub fn source_files(&self) -> &[PathBuf] {
        &self.source_files
    }
}

impl Default for MacroRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Load macro registry from default location
///
/// Looks for macro YAML files in `config/verb_schemas/macros/`
pub fn load_macro_registry() -> Result<MacroRegistry> {
    // Find config directory
    let config_dir = find_config_dir()?;
    let macros_dir = config_dir.join("verb_schemas").join("macros");

    if !macros_dir.exists() {
        warn!("Macros directory not found: {:?}", macros_dir);
        return Ok(MacroRegistry::new());
    }

    load_macro_registry_from_dir(&macros_dir)
}

/// Load macro registry from a specific directory
pub fn load_macro_registry_from_dir(dir: &Path) -> Result<MacroRegistry> {
    let mut registry = MacroRegistry::new();

    if !dir.exists() {
        return Ok(registry);
    }

    // Find all YAML files
    let yaml_files: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .map(|ext| ext == "yaml" || ext == "yml")
                .unwrap_or(false)
        })
        .map(|entry| entry.path())
        .collect();

    info!(
        "Loading macros from {} files in {:?}",
        yaml_files.len(),
        dir
    );

    for path in yaml_files {
        match load_macro_file(&path) {
            Ok(file_registry) => {
                debug!(
                    "Loaded {} macros from {:?}",
                    file_registry.len(),
                    path.file_name()
                );
                registry.merge(file_registry);
            }
            Err(e) => {
                warn!("Failed to load macro file {:?}: {}", path, e);
            }
        }
    }

    info!(
        "Macro registry loaded: {} macros, {} domains, {} mode tags",
        registry.len(),
        registry.by_domain.len(),
        registry.by_mode_tag.len()
    );

    Ok(registry)
}

/// Load macros from a single YAML file
fn load_macro_file(path: &Path) -> Result<MacroRegistry> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read macro file: {:?}", path))?;

    let raw: HashMap<String, MacroSchema> = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse macro file: {:?}", path))?;

    let mut registry = MacroRegistry::new();
    registry.source_files.push(path.to_path_buf());

    for (fqn, schema) in raw {
        // Only load macros, not primitives
        if schema.kind == MacroKind::Macro {
            registry.add(fqn, schema);
        }
    }

    Ok(registry)
}

/// Find the config directory (searches up from current dir)
fn find_config_dir() -> Result<PathBuf> {
    // Check environment variable first
    if let Ok(dir) = std::env::var("DSL_CONFIG_DIR") {
        let path = PathBuf::from(dir);
        if path.exists() {
            return Ok(path);
        }
    }

    // Search up from current directory
    let mut current = std::env::current_dir()?;
    loop {
        let config = current.join("config");
        if config.exists() {
            return Ok(config);
        }

        // Also check rust/config (for when running from repo root)
        let rust_config = current.join("rust").join("config");
        if rust_config.exists() {
            return Ok(rust_config);
        }

        if !current.pop() {
            break;
        }
    }

    // Fallback to relative path
    Ok(PathBuf::from("config"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_registry() {
        let registry = MacroRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert!(registry.get("structure.setup").is_none());
    }

    #[test]
    fn test_load_from_yaml() {
        let yaml = r#"
structure.setup:
  kind: macro
  ui:
    label: "Set up Structure"
    description: "Create a new fund"
    target_label: "Structure"
  routing:
    mode_tags: [onboarding, kyc]
    operator_domain: structure
  target:
    operates_on: client_ref
    produces: structure_ref
  args:
    style: keyworded
    required:
      name:
        type: str
        ui_label: "Name"
    optional: {}
  prereqs: []
  expands_to:
    - verb: cbu.create
      args:
        name: "${arg.name}"
  unlocks: []
"#;

        let raw: HashMap<String, MacroSchema> = serde_yaml::from_str(yaml).unwrap();
        let mut registry = MacroRegistry::new();

        for (fqn, schema) in raw {
            if schema.kind == MacroKind::Macro {
                registry.add(fqn, schema);
            }
        }

        assert_eq!(registry.len(), 1);
        assert!(registry.has("structure.setup"));

        let schema = registry.get("structure.setup").unwrap();
        assert_eq!(schema.ui.label, "Set up Structure");
        assert_eq!(schema.routing.mode_tags, vec!["onboarding", "kyc"]);

        // Check indexes
        let by_domain = registry.by_domain("structure");
        assert_eq!(by_domain.len(), 1);

        let by_tag = registry.by_mode_tag("kyc");
        assert_eq!(by_tag.len(), 1);
    }
}
