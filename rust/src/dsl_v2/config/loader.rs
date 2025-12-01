//! Configuration loader
//!
//! Loads and validates YAML configuration files.

use anyhow::{anyhow, Context, Result};
use std::path::Path;
use tracing::info;

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
    pub fn from_env() -> Self {
        let dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        Self::new(dir)
    }

    /// Get the config directory path
    pub fn config_dir(&self) -> &str {
        &self.config_dir
    }

    /// Load verb configuration
    pub fn load_verbs(&self) -> Result<VerbsConfig> {
        let path = Path::new(&self.config_dir).join("verbs.yaml");
        info!("Loading verb configuration from {}", path.display());

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let config: VerbsConfig = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        self.validate_verbs(&config)?;

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

        // Validate plugins
        for (plugin_name, plugin_config) in &config.plugins {
            for arg in &plugin_config.args {
                if arg.arg_type == ArgType::Lookup && arg.lookup.is_none() {
                    return Err(anyhow!(
                        "plugin {} arg '{}': lookup type requires lookup config",
                        plugin_name,
                        arg.name
                    ));
                }
            }
        }

        Ok(())
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
    fn test_from_env_default() {
        std::env::remove_var("DSL_CONFIG_DIR");
        let loader = ConfigLoader::from_env();
        assert_eq!(loader.config_dir, "config");
    }
}

    #[test]
    fn test_load_verbs_yaml() {
        // This test loads the actual verbs.yaml file
        let loader = ConfigLoader::new("config");
        let result = loader.load_verbs();
        
        match result {
            Ok(config) => {
                assert_eq!(config.version, "1.0");
                assert!(config.domains.contains_key("cbu"), "Should have cbu domain");
                assert!(config.domains.contains_key("entity"), "Should have entity domain");
                assert!(config.domains.contains_key("product"), "Should have product domain");
                assert!(config.domains.contains_key("service"), "Should have service domain");
                assert!(config.domains.contains_key("lifecycle-resource"), "Should have lifecycle-resource domain");
                
                // Check CBU verbs
                let cbu = config.domains.get("cbu").unwrap();
                assert!(cbu.verbs.contains_key("create"), "CBU should have create verb");
                assert!(cbu.verbs.contains_key("read"), "CBU should have read verb");
                assert!(cbu.verbs.contains_key("ensure"), "CBU should have ensure verb");
                assert!(cbu.verbs.contains_key("assign-role"), "CBU should have assign-role verb");
                
                println!("Loaded {} domains", config.domains.len());
                for (name, domain) in &config.domains {
                    println!("  {}: {} verbs", name, domain.verbs.len());
                }
                println!("Loaded {} plugins", config.plugins.len());
            }
            Err(e) => {
                panic!("Failed to load verbs.yaml: {}", e);
            }
        }
    }

    #[test]
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
                println!("Loaded {} jurisdiction rules", config.jurisdiction_rules.len());
                println!("Loaded {} composite rules", config.composite_rules.len());
            }
            Err(e) => {
                panic!("Failed to load csg_rules.yaml: {}", e);
            }
        }
    }
