//! Operator macro registry
//!
//! Loads and indexes operator macro definitions from YAML files.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use tracing::{debug, info, warn};

use super::definition::OperatorMacroDef;

/// Registry of operator macro definitions
#[derive(Debug, Clone)]
pub struct OperatorMacroRegistry {
    /// Macros indexed by FQN (e.g., "structure.setup")
    macros: HashMap<String, OperatorMacroDef>,

    /// Index by domain (e.g., "structure" -> ["structure.setup", "structure.assign-role"])
    by_domain: HashMap<String, Vec<String>>,

    /// Index by mode tag (e.g., "onboarding" -> ["structure.setup", "case.open"])
    by_mode_tag: HashMap<String, Vec<String>>,
}

impl Default for OperatorMacroRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl OperatorMacroRegistry {
    /// Create an empty registry
    pub fn new() -> Self {
        Self {
            macros: HashMap::new(),
            by_domain: HashMap::new(),
            by_mode_tag: HashMap::new(),
        }
    }

    /// Load macros from a directory (recursive)
    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        let mut registry = Self::new();

        if !dir.exists() {
            warn!("Operator macros directory does not exist: {:?}", dir);
            return Ok(registry);
        }

        registry.load_dir_recursive(dir)?;

        info!(
            "Loaded {} operator macros from {:?}",
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
                    Ok(count) => debug!("Loaded {} macros from {:?}", count, path),
                    Err(e) => warn!("Failed to load macros from {:?}: {}", path, e),
                }
            }
        }

        Ok(())
    }

    fn load_file(&mut self, path: &Path) -> Result<usize> {
        let content =
            std::fs::read_to_string(path).with_context(|| format!("Failed to read {:?}", path))?;

        // Parse as a map of FQN -> MacroDef (the YAML format has FQN as key)
        let macros: HashMap<String, OperatorMacroDef> = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse {:?}", path))?;

        let count = macros.len();

        for (fqn, mut macro_def) in macros {
            macro_def.fqn = fqn.clone();
            self.register(macro_def);
        }

        Ok(count)
    }

    /// Register a macro definition
    pub fn register(&mut self, macro_def: OperatorMacroDef) {
        let fqn = macro_def.fqn.clone();
        let domain = macro_def.domain().to_string();

        // Build domain index
        self.by_domain.entry(domain).or_default().push(fqn.clone());

        // Build mode tag index
        for tag in &macro_def.routing.mode_tags {
            self.by_mode_tag
                .entry(tag.clone())
                .or_default()
                .push(fqn.clone());
        }

        self.macros.insert(fqn, macro_def);
    }

    /// Get a macro by FQN
    pub fn get(&self, fqn: &str) -> Option<&OperatorMacroDef> {
        self.macros.get(fqn)
    }

    /// List all macros, optionally filtered
    pub fn list(&self, filter: Option<MacroFilter>) -> Vec<&OperatorMacroDef> {
        self.macros
            .values()
            .filter(|m| {
                if let Some(ref f) = filter {
                    f.matches(m)
                } else {
                    true
                }
            })
            .collect()
    }

    /// Get macros by domain
    pub fn by_domain(&self, domain: &str) -> Vec<&OperatorMacroDef> {
        self.by_domain
            .get(domain)
            .map(|fqns| fqns.iter().filter_map(|f| self.macros.get(f)).collect())
            .unwrap_or_default()
    }

    /// Get macros by mode tag
    pub fn by_mode_tag(&self, tag: &str) -> Vec<&OperatorMacroDef> {
        self.by_mode_tag
            .get(tag)
            .map(|fqns| fqns.iter().filter_map(|f| self.macros.get(f)).collect())
            .unwrap_or_default()
    }

    /// Get all macro FQNs
    pub fn fqns(&self) -> Vec<&str> {
        self.macros.keys().map(|s| s.as_str()).collect()
    }

    /// Get all domains
    pub fn domains(&self) -> Vec<&str> {
        self.by_domain.keys().map(|s| s.as_str()).collect()
    }

    /// Get all mode tags
    pub fn mode_tags(&self) -> Vec<&str> {
        self.by_mode_tag.keys().map(|s| s.as_str()).collect()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.macros.is_empty()
    }

    /// Get count of registered macros
    pub fn len(&self) -> usize {
        self.macros.len()
    }

    /// Build taxonomy tree for UI
    pub fn build_taxonomy(&self) -> MacroTaxonomy {
        let mut domains: HashMap<String, DomainNode> = HashMap::new();

        for macro_def in self.macros.values() {
            let domain = macro_def.domain().to_string();

            let domain_node = domains.entry(domain.clone()).or_insert_with(|| DomainNode {
                domain: domain.clone(),
                label: domain_label(&domain),
                macros: Vec::new(),
            });

            domain_node.macros.push(MacroNode {
                fqn: macro_def.fqn.clone(),
                label: macro_def.ui.label.clone(),
                description: macro_def.ui.description.clone(),
            });
        }

        // Sort macros within each domain
        for node in domains.values_mut() {
            node.macros.sort_by(|a, b| a.label.cmp(&b.label));
        }

        // Sort domains
        let mut domain_list: Vec<_> = domains.into_values().collect();
        domain_list.sort_by(|a, b| a.label.cmp(&b.label));

        MacroTaxonomy {
            domains: domain_list,
        }
    }
}

/// Filter for listing macros
#[derive(Debug, Clone, Default)]
pub struct MacroFilter {
    /// Filter by domain
    pub domain: Option<String>,

    /// Filter by mode tag
    pub mode_tag: Option<String>,

    /// Filter by search term (searches FQN, label, description)
    pub search: Option<String>,
}

impl MacroFilter {
    /// Check if a macro matches this filter
    pub fn matches(&self, macro_def: &OperatorMacroDef) -> bool {
        // Domain filter
        if let Some(ref domain) = self.domain {
            if macro_def.domain() != domain {
                return false;
            }
        }

        // Mode tag filter
        if let Some(ref tag) = self.mode_tag {
            if !macro_def.is_available_for_mode(tag) {
                return false;
            }
        }

        // Search filter
        if let Some(ref search) = self.search {
            let search_lower = search.to_lowercase();
            let matches_fqn = macro_def.fqn.to_lowercase().contains(&search_lower);
            let matches_label = macro_def.ui.label.to_lowercase().contains(&search_lower);
            let matches_desc = macro_def
                .ui
                .description
                .to_lowercase()
                .contains(&search_lower);

            if !matches_fqn && !matches_label && !matches_desc {
                return false;
            }
        }

        true
    }
}

/// Taxonomy tree for UI
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MacroTaxonomy {
    pub domains: Vec<DomainNode>,
}

/// Domain node in taxonomy
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DomainNode {
    pub domain: String,
    pub label: String,
    pub macros: Vec<MacroNode>,
}

/// Macro node in taxonomy
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MacroNode {
    pub fqn: String,
    pub label: String,
    pub description: String,
}

/// Get display label for a domain
fn domain_label(domain: &str) -> String {
    match domain {
        "structure" => "Structure".to_string(),
        "case" => "KYC Case".to_string(),
        "mandate" => "Mandate".to_string(),
        other => {
            // Title case
            let mut chars = other.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().chain(chars).collect(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_yaml(dir: &std::path::Path, name: &str, content: &str) {
        let path = dir.join(format!("{}.yaml", name));
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn test_load_from_dir() {
        let temp = TempDir::new().unwrap();

        create_test_yaml(
            temp.path(),
            "structure",
            r#"
structure.setup:
  kind: macro
  ui:
    label: "Set up Structure"
    description: "Create a new fund or mandate structure"
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
        ui_label: "Structure name"
    optional: {}
  prereqs: []
  expands_to:
    - verb: cbu.create
      args:
        name: "${arg.name}"
  unlocks:
    - structure.assign-role
"#,
        );

        let registry = OperatorMacroRegistry::load_from_dir(temp.path()).unwrap();
        assert_eq!(registry.len(), 1);
        assert!(registry.get("structure.setup").is_some());

        let macro_def = registry.get("structure.setup").unwrap();
        assert_eq!(macro_def.ui.label, "Set up Structure");
        assert_eq!(macro_def.domain(), "structure");
        assert_eq!(macro_def.action(), "setup");
    }

    #[test]
    fn test_by_domain() {
        let temp = TempDir::new().unwrap();

        create_test_yaml(
            temp.path(),
            "mixed",
            r#"
structure.setup:
  kind: macro
  ui:
    label: "Set up Structure"
    description: "Create structure"
  routing:
    mode_tags: [onboarding]
    operator_domain: structure
  target:
    operates_on: client_ref
    produces: structure_ref
  args:
    style: keyworded
    required: {}
    optional: {}
  prereqs: []
  expands_to:
    - verb: cbu.create
      args: {}
  unlocks: []

case.open:
  kind: macro
  ui:
    label: "Open Case"
    description: "Open KYC case"
  routing:
    mode_tags: [kyc]
    operator_domain: case
  target:
    operates_on: structure_ref
    produces: case_ref
  args:
    style: keyworded
    required: {}
    optional: {}
  prereqs: []
  expands_to:
    - verb: kyc-case.create
      args: {}
  unlocks: []
"#,
        );

        let registry = OperatorMacroRegistry::load_from_dir(temp.path()).unwrap();
        assert_eq!(registry.len(), 2);

        let structure_macros = registry.by_domain("structure");
        assert_eq!(structure_macros.len(), 1);
        assert_eq!(structure_macros[0].fqn, "structure.setup");

        let case_macros = registry.by_domain("case");
        assert_eq!(case_macros.len(), 1);
        assert_eq!(case_macros[0].fqn, "case.open");
    }

    #[test]
    fn test_filter() {
        let filter = MacroFilter {
            domain: Some("structure".to_string()),
            mode_tag: None,
            search: None,
        };

        // Would need a macro_def to test fully
        // Just testing filter construction
        assert_eq!(filter.domain, Some("structure".to_string()));
    }

    #[test]
    fn test_taxonomy() {
        let temp = TempDir::new().unwrap();

        create_test_yaml(
            temp.path(),
            "test",
            r#"
structure.setup:
  kind: macro
  ui:
    label: "Set up Structure"
    description: "Create"
  routing:
    mode_tags: [onboarding]
    operator_domain: structure
  target:
    operates_on: client_ref
    produces: structure_ref
  args:
    style: keyworded
    required: {}
    optional: {}
  prereqs: []
  expands_to: []
  unlocks: []

structure.list:
  kind: macro
  ui:
    label: "List Structures"
    description: "List"
  routing:
    mode_tags: [onboarding]
    operator_domain: structure
  target:
    operates_on: client_ref
    produces: null
  args:
    style: keyworded
    required: {}
    optional: {}
  prereqs: []
  expands_to: []
  unlocks: []
"#,
        );

        let registry = OperatorMacroRegistry::load_from_dir(temp.path()).unwrap();
        let taxonomy = registry.build_taxonomy();

        assert_eq!(taxonomy.domains.len(), 1);
        assert_eq!(taxonomy.domains[0].domain, "structure");
        assert_eq!(taxonomy.domains[0].macros.len(), 2);
    }
}
