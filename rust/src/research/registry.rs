//! Research macro registry
//!
//! Loads and indexes research macro definitions from YAML files.

use std::collections::HashMap;
use std::path::Path;

use tracing::{debug, info, warn};

use super::definition::{ResearchMacroDef, ResearchMacroWrapper};
use super::error::Result;

/// Registry of research macro definitions
#[derive(Debug, Clone)]
pub struct ResearchMacroRegistry {
    /// Macros indexed by name
    macros: HashMap<String, ResearchMacroDef>,

    /// Tag index for search
    by_tag: HashMap<String, Vec<String>>,
}

impl Default for ResearchMacroRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ResearchMacroRegistry {
    /// Create an empty registry
    pub fn new() -> Self {
        Self {
            macros: HashMap::new(),
            by_tag: HashMap::new(),
        }
    }

    /// Load macros from a directory (recursive)
    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        let mut registry = Self::new();

        if !dir.exists() {
            warn!("Research macros directory does not exist: {:?}", dir);
            return Ok(registry);
        }

        registry.load_dir_recursive(dir)?;

        info!(
            "Loaded {} research macros from {:?}",
            registry.macros.len(),
            dir
        );

        Ok(registry)
    }

    fn load_dir_recursive(&mut self, dir: &Path) -> Result<()> {
        let entries = std::fs::read_dir(dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.load_dir_recursive(&path)?;
            } else if path
                .extension()
                .map(|e| e == "yaml" || e == "yml")
                .unwrap_or(false)
            {
                match self.load_file(&path) {
                    Ok(()) => debug!("Loaded research macro from {:?}", path),
                    Err(e) => warn!("Failed to load research macro from {:?}: {}", path, e),
                }
            }
        }

        Ok(())
    }

    fn load_file(&mut self, path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(path)?;
        let wrapper: ResearchMacroWrapper = serde_yaml::from_str(&content)?;
        self.register(wrapper.macro_def);
        Ok(())
    }

    /// Register a macro definition
    pub fn register(&mut self, macro_def: ResearchMacroDef) {
        let name = macro_def.name.clone();

        // Build tag index
        for tag in &macro_def.tags {
            self.by_tag
                .entry(tag.clone())
                .or_default()
                .push(name.clone());
        }

        self.macros.insert(name, macro_def);
    }

    /// Get a macro by name
    pub fn get(&self, name: &str) -> Option<&ResearchMacroDef> {
        self.macros.get(name)
    }

    /// List all macros, optionally filtered by search term
    pub fn list(&self, search: Option<&str>) -> Vec<&ResearchMacroDef> {
        let search_lower = search.map(|s| s.to_lowercase());

        self.macros
            .values()
            .filter(|m| {
                search_lower.as_ref().is_none_or(|term| {
                    m.name.to_lowercase().contains(term)
                        || m.description.to_lowercase().contains(term)
                        || m.tags.iter().any(|t| t.to_lowercase().contains(term))
                })
            })
            .collect()
    }

    /// Get macros by tag
    pub fn by_tag(&self, tag: &str) -> Vec<&ResearchMacroDef> {
        self.by_tag
            .get(tag)
            .map(|names| names.iter().filter_map(|n| self.macros.get(n)).collect())
            .unwrap_or_default()
    }

    /// Get all macro names
    pub fn names(&self) -> Vec<&str> {
        self.macros.keys().map(|s| s.as_str()).collect()
    }

    /// Get all tags
    pub fn tags(&self) -> Vec<&str> {
        self.by_tag.keys().map(|s| s.as_str()).collect()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.macros.is_empty()
    }

    /// Get count of registered macros
    pub fn len(&self) -> usize {
        self.macros.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_yaml(dir: &Path, name: &str, content: &str) {
        let path = dir.join(format!("{}.yaml", name));
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn test_load_from_dir() {
        let temp = TempDir::new().unwrap();

        create_test_yaml(
            temp.path(),
            "test-macro",
            r#"
macro:
  name: test-macro
  description: A test macro
  parameters: []
  tools: []
  prompt: "Test prompt"
  output:
    schema_name: test
    schema: {}
  tags:
    - test
"#,
        );

        let registry = ResearchMacroRegistry::load_from_dir(temp.path()).unwrap();
        assert_eq!(registry.len(), 1);
        assert!(registry.get("test-macro").is_some());
    }

    #[test]
    fn test_list_with_search() {
        let mut registry = ResearchMacroRegistry::new();

        registry.register(ResearchMacroDef {
            name: "client-discovery".into(),
            version: "1.0".into(),
            description: "Discover client structure".into(),
            parameters: vec![],
            tools: vec![],
            prompt: String::new(),
            output: super::super::definition::ResearchOutput {
                schema_name: "client".into(),
                schema: serde_json::json!({}),
                review: super::super::definition::ReviewRequirement::Required,
            },
            suggested_verbs: None,
            tags: vec!["client".into(), "discovery".into()],
        });

        registry.register(ResearchMacroDef {
            name: "ubo-investigation".into(),
            version: "1.0".into(),
            description: "Investigate UBO chain".into(),
            parameters: vec![],
            tools: vec![],
            prompt: String::new(),
            output: super::super::definition::ResearchOutput {
                schema_name: "ubo".into(),
                schema: serde_json::json!({}),
                review: super::super::definition::ReviewRequirement::Required,
            },
            suggested_verbs: None,
            tags: vec!["ubo".into(), "investigation".into()],
        });

        // Search by name
        let results = registry.list(Some("client"));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "client-discovery");

        // Search by description
        let results = registry.list(Some("chain"));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "ubo-investigation");

        // List all
        let results = registry.list(None);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_by_tag() {
        let mut registry = ResearchMacroRegistry::new();

        registry.register(ResearchMacroDef {
            name: "macro1".into(),
            version: "1.0".into(),
            description: String::new(),
            parameters: vec![],
            tools: vec![],
            prompt: String::new(),
            output: super::super::definition::ResearchOutput {
                schema_name: "test".into(),
                schema: serde_json::json!({}),
                review: super::super::definition::ReviewRequirement::Required,
            },
            suggested_verbs: None,
            tags: vec!["shared-tag".into(), "tag1".into()],
        });

        registry.register(ResearchMacroDef {
            name: "macro2".into(),
            version: "1.0".into(),
            description: String::new(),
            parameters: vec![],
            tools: vec![],
            prompt: String::new(),
            output: super::super::definition::ResearchOutput {
                schema_name: "test".into(),
                schema: serde_json::json!({}),
                review: super::super::definition::ReviewRequirement::Required,
            },
            suggested_verbs: None,
            tags: vec!["shared-tag".into(), "tag2".into()],
        });

        let shared = registry.by_tag("shared-tag");
        assert_eq!(shared.len(), 2);

        let tag1 = registry.by_tag("tag1");
        assert_eq!(tag1.len(), 1);
    }
}
