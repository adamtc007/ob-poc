//! Configuration loader
//!
//! Loads and validates YAML configuration files.

use anyhow::{anyhow, Context, Result};
use std::path::Path;
use tracing::info;

use super::phrase_gen::generate_phrases;
use super::types::{ArgType, CsgRulesConfig, VerbBehavior, VerbsConfig};

pub struct ConfigLoader {
    config_dir: String,
}

impl ConfigLoader {
    pub fn new(config_dir: impl Into<String>) -> Self {
        Self {
            config_dir: config_dir.into(),
        }
    }

    /// Create loader from DSL_CONFIG_DIR env var or default to "config"
    ///
    /// Path resolution order:
    /// 1. DSL_CONFIG_DIR environment variable (explicit override)
    /// 2. Relative "config" path (works when running from workspace root)
    /// 3. CARGO_MANIFEST_DIR/config (works if config is in crate directory)
    /// 4. Workspace root config (traverse up from CARGO_MANIFEST_DIR to find rust/config)
    /// 5. Compile-time workspace detection (for test binaries)
    pub fn from_env() -> Self {
        // Try explicit override first
        if let Ok(dir) = std::env::var("DSL_CONFIG_DIR") {
            return Self::new(dir);
        }

        // Try relative "config" (works when running from workspace root)
        if Path::new("config/verbs").exists() {
            return Self::new("config");
        }

        // Try runtime CARGO_MANIFEST_DIR (set by cargo during build/test)
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            // First try config in the crate itself
            let config_path = format!("{}/config", manifest_dir);
            if Path::new(&config_path).join("verbs").exists() {
                return Self::new(config_path);
            }

            // Try workspace root config (traverse up to find rust/config)
            if let Some(workspace_config) = Self::find_workspace_config(&manifest_dir) {
                return Self::new(workspace_config);
            }
        }

        // Use compile-time path - for workspace crates, traverse up to find config
        // CARGO_MANIFEST_DIR at compile time points to the crate directory (e.g., crates/dsl-core)
        let compile_time_manifest = env!("CARGO_MANIFEST_DIR");

        // Try crate-local config first
        let compile_time_config = concat!(env!("CARGO_MANIFEST_DIR"), "/config");
        if Path::new(compile_time_config).join("verbs").exists() {
            return Self::new(compile_time_config);
        }

        // Try workspace root config (traverse up from crate directory)
        if let Some(workspace_config) = Self::find_workspace_config(compile_time_manifest) {
            return Self::new(workspace_config);
        }

        // Last resort - return "config" and let it fail with clear error
        Self::new("config")
    }

    /// Find config directory by traversing up from a manifest directory
    /// Looks for a parent directory containing "config/verbs"
    fn find_workspace_config(start_dir: &str) -> Option<String> {
        let mut current = Path::new(start_dir);

        // Traverse up to 5 levels looking for config/verbs
        for _ in 0..5 {
            let config_path = current.join("config");
            if config_path.join("verbs").exists() {
                return Some(config_path.to_string_lossy().into_owned());
            }

            // Move up one directory
            current = current.parent()?;
        }

        None
    }

    /// Get the config directory path as string slice
    pub fn config_dir_str(&self) -> &str {
        &self.config_dir
    }

    /// Get the config directory as PathBuf
    pub fn config_dir(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.config_dir)
    }

    /// Load verb configuration
    ///
    /// Supports two modes:
    /// 1. Single file: config/verbs.yaml (legacy)
    /// 2. Split directory: config/verbs/*.yaml (preferred)
    ///
    /// If verbs/ directory exists, loads all .yaml files recursively and merges domains.
    /// Otherwise falls back to verbs.yaml.
    pub fn load_verbs(&self) -> Result<VerbsConfig> {
        let verbs_dir = Path::new(&self.config_dir).join("verbs");

        if verbs_dir.exists() && verbs_dir.is_dir() {
            self.load_verbs_from_directory(&verbs_dir)
        } else {
            self.load_verbs_from_file()
        }
    }

    /// Load verbs from single verbs.yaml file (legacy mode)
    fn load_verbs_from_file(&self) -> Result<VerbsConfig> {
        let path = Path::new(&self.config_dir).join("verbs.yaml");
        info!("Loading verb configuration from {}", path.display());

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let mut config: VerbsConfig = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        self.validate_verbs(&config)?;

        // Auto-generate invocation phrases for verbs without them
        self.enrich_with_generated_phrases(&mut config);

        info!(
            "Loaded {} domains with {} total verbs",
            config.domains.len(),
            config
                .domains
                .values()
                .map(|d| d.verbs.len())
                .sum::<usize>()
        );

        Ok(config)
    }

    /// Load verbs from split directory (config/verbs/*.yaml)
    fn load_verbs_from_directory(&self, verbs_dir: &Path) -> Result<VerbsConfig> {
        info!(
            "Loading verb configuration from directory {}",
            verbs_dir.display()
        );

        let mut merged_config = VerbsConfig {
            version: "1.0".to_string(),
            domains: std::collections::HashMap::new(),
        };

        // Recursively find all .yaml files
        let yaml_files = self.find_yaml_files(verbs_dir)?;

        for path in yaml_files {
            // Skip _meta.yaml (contains version info, not domains)
            if path
                .file_name()
                .map(|n| n.to_str().unwrap_or(""))
                .unwrap_or("")
                .starts_with('_')
            {
                // Check for version in _meta.yaml
                if path.file_name().map(|n| n == "_meta.yaml").unwrap_or(false) {
                    let content = std::fs::read_to_string(&path)
                        .with_context(|| format!("Failed to read {}", path.display()))?;
                    if let Ok(meta) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                        if let Some(version) = meta.get("version").and_then(|v| v.as_str()) {
                            merged_config.version = version.to_string();
                        }
                    }
                }
                continue;
            }

            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;

            let partial: VerbsConfig = serde_yaml::from_str(&content)
                .with_context(|| format!("Failed to parse {}", path.display()))?;

            // Merge domains
            for (domain_name, domain_config) in partial.domains {
                merged_config.domains.insert(domain_name, domain_config);
            }
        }

        self.validate_verbs(&merged_config)?;

        // Auto-generate invocation phrases for verbs without them
        self.enrich_with_generated_phrases(&mut merged_config);

        info!(
            "Loaded {} domains with {} total verbs from split config",
            merged_config.domains.len(),
            merged_config
                .domains
                .values()
                .map(|d| d.verbs.len())
                .sum::<usize>()
        );

        Ok(merged_config)
    }

    /// Recursively find all .yaml files in a directory
    fn find_yaml_files(&self, dir: &Path) -> Result<Vec<std::path::PathBuf>> {
        #![allow(clippy::only_used_in_recursion)]
        let mut files = Vec::new();

        for entry in std::fs::read_dir(dir)
            .with_context(|| format!("Failed to read directory {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                files.extend(self.find_yaml_files(&path)?);
            } else if path
                .extension()
                .map(|e| e == "yaml" || e == "yml")
                .unwrap_or(false)
            {
                files.push(path);
            }
        }

        // Sort for deterministic loading order
        files.sort();
        Ok(files)
    }

    /// Load CSG rules configuration
    pub fn load_csg_rules(&self) -> Result<CsgRulesConfig> {
        let path = Path::new(&self.config_dir).join("csg_rules.yaml");
        info!("Loading CSG rules from {}", path.display());

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let config: CsgRulesConfig = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        self.validate_csg_rules(&config)?;

        info!(
            "Loaded {} constraints, {} warnings, {} jurisdiction rules",
            config.constraints.len(),
            config.warnings.len(),
            config.jurisdiction_rules.len()
        );

        Ok(config)
    }

    fn validate_verbs(&self, config: &VerbsConfig) -> Result<()> {
        for (domain, domain_config) in &config.domains {
            for (verb, verb_config) in &domain_config.verbs {
                let full_name = format!("{}.{}", domain, verb);

                // Validate CRUD verbs have crud config
                if verb_config.behavior == VerbBehavior::Crud && verb_config.crud.is_none() {
                    return Err(anyhow!("{}: crud behavior requires crud config", full_name));
                }

                // Validate lookup args have lookup config
                for arg in &verb_config.args {
                    if arg.arg_type == ArgType::Lookup && arg.lookup.is_none() {
                        return Err(anyhow!(
                            "{} arg '{}': lookup type requires lookup config",
                            full_name,
                            arg.name
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    /// Auto-generate invocation phrases for verbs that don't have any.
    ///
    /// This ensures all verbs are discoverable via semantic search without
    /// requiring manual phrase curation. Existing phrases are preserved.
    fn enrich_with_generated_phrases(&self, config: &mut VerbsConfig) {
        let mut generated_count = 0;

        for (domain_name, domain_config) in &mut config.domains {
            for (verb_name, verb_config) in &mut domain_config.verbs {
                // Get existing phrases (if any)
                let existing: Vec<String> = verb_config.invocation_phrases.clone();

                // Generate phrases combining action + domain
                let generated = generate_phrases(domain_name, verb_name, &existing);

                // Only update if we generated more phrases than existing, or if existing was empty
                let should_update = generated.len() > existing.len()
                    || (verb_config.invocation_phrases.is_empty() && !generated.is_empty());
                if should_update {
                    verb_config.invocation_phrases = generated;
                    generated_count += 1;
                }
            }
        }

        if generated_count > 0 {
            info!(
                "Auto-generated invocation phrases for {} verbs",
                generated_count
            );
        }
    }

    fn validate_csg_rules(&self, config: &CsgRulesConfig) -> Result<()> {
        let mut ids = std::collections::HashSet::new();

        // Check for duplicate rule IDs
        for rule in &config.constraints {
            if !ids.insert(&rule.id) {
                return Err(anyhow!("Duplicate rule ID: {}", rule.id));
            }
        }
        for rule in &config.warnings {
            if !ids.insert(&rule.id) {
                return Err(anyhow!("Duplicate rule ID: {}", rule.id));
            }
        }
        for rule in &config.jurisdiction_rules {
            if !ids.insert(&rule.id) {
                return Err(anyhow!("Duplicate rule ID: {}", rule.id));
            }
        }
        for rule in &config.composite_rules {
            if !ids.insert(&rule.id) {
                return Err(anyhow!("Duplicate rule ID: {}", rule.id));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loader_creation() {
        let loader = ConfigLoader::new("config");
        assert_eq!(loader.config_dir, "config");
    }

    #[test]
    fn test_from_env_finds_workspace_config() {
        std::env::remove_var("DSL_CONFIG_DIR");
        let loader = ConfigLoader::from_env();

        // Debug output
        println!("Resolved config_dir: {}", loader.config_dir);
        println!(
            "Compile-time CARGO_MANIFEST_DIR: {}",
            env!("CARGO_MANIFEST_DIR")
        );

        // Should find workspace config (rust/config/verbs exists)
        let verbs_path = Path::new(&loader.config_dir).join("verbs");
        println!(
            "Checking verbs path: {} -> exists: {}",
            verbs_path.display(),
            verbs_path.exists()
        );

        assert!(
            verbs_path.exists(),
            "ConfigLoader should find workspace config/verbs directory. Got config_dir: {}",
            loader.config_dir
        );
    }
}

#[test]
#[ignore = "requires config files - run from workspace root"]
fn test_load_verbs_yaml() {
    // This test loads the actual verbs.yaml file
    let loader = ConfigLoader::new("config");
    let result = loader.load_verbs();

    match result {
        Ok(config) => {
            assert_eq!(config.version, "1.0");
            assert!(config.domains.contains_key("cbu"), "Should have cbu domain");
            assert!(
                config.domains.contains_key("entity"),
                "Should have entity domain"
            );
            assert!(
                config.domains.contains_key("product"),
                "Should have product domain"
            );
            assert!(
                config.domains.contains_key("service"),
                "Should have service domain"
            );
            assert!(
                config.domains.contains_key("service-resource"),
                "Should have service-resource domain"
            );

            // Check CBU verbs
            let cbu = config.domains.get("cbu").unwrap();
            assert!(
                cbu.verbs.contains_key("create"),
                "CBU should have create verb"
            );
            assert!(cbu.verbs.contains_key("read"), "CBU should have read verb");
            assert!(
                cbu.verbs.contains_key("ensure"),
                "CBU should have ensure verb"
            );
            assert!(
                cbu.verbs.contains_key("assign-role"),
                "CBU should have assign-role verb"
            );

            println!("Loaded {} domains", config.domains.len());
            for (name, domain) in &config.domains {
                println!("  {}: {} verbs", name, domain.verbs.len());
            }
        }
        Err(e) => {
            panic!("Failed to load verbs.yaml: {:?}", e);
        }
    }
}

#[test]
#[ignore = "requires config files - run from workspace root"]
fn test_load_csg_rules_yaml() {
    // This test loads the actual csg_rules.yaml file
    let loader = ConfigLoader::new("config");
    let result = loader.load_csg_rules();

    match result {
        Ok(config) => {
            assert_eq!(config.version, "1.0");
            assert!(!config.constraints.is_empty(), "Should have constraints");
            assert!(!config.warnings.is_empty(), "Should have warnings");

            println!("Loaded {} constraints", config.constraints.len());
            println!("Loaded {} warnings", config.warnings.len());
            println!(
                "Loaded {} jurisdiction rules",
                config.jurisdiction_rules.len()
            );
            println!("Loaded {} composite rules", config.composite_rules.len());
        }
        Err(e) => {
            panic!("Failed to load csg_rules.yaml: {}", e);
        }
    }
}
