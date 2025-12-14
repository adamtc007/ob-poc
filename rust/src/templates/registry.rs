//! Template Registry
//!
//! Loads, indexes, and provides access to template definitions.

use std::collections::HashMap;
use std::path::Path;

use super::definition::TemplateDefinition;
use super::error::TemplateError;

/// Registry of all loaded templates with multiple indexes
pub struct TemplateRegistry {
    /// All templates by ID
    templates: HashMap<String, TemplateDefinition>,

    /// Index: tag → template IDs
    by_tag: HashMap<String, Vec<String>>,

    /// Index: blocker type → template IDs
    by_blocker: HashMap<String, Vec<String>>,

    /// Index: (workflow, state) → template IDs
    by_workflow_state: HashMap<(String, String), Vec<String>>,
}

impl TemplateRegistry {
    /// Create an empty registry
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            by_tag: HashMap::new(),
            by_blocker: HashMap::new(),
            by_workflow_state: HashMap::new(),
        }
    }

    /// Load all templates from a directory (recursive)
    pub fn load_from_dir(dir: &Path) -> Result<Self, TemplateError> {
        let mut registry = Self::new();

        if dir.exists() {
            registry.load_recursive(dir)?;
        }

        Ok(registry)
    }

    fn load_recursive(&mut self, dir: &Path) -> Result<(), TemplateError> {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                self.load_recursive(&path)?;
            } else if path
                .extension()
                .map(|e| e == "yaml" || e == "yml")
                .unwrap_or(false)
            {
                let content = std::fs::read_to_string(&path)?;
                match serde_yaml::from_str::<TemplateDefinition>(&content) {
                    Ok(template) => {
                        self.index_template(template);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse template {:?}: {}", path, e);
                    }
                }
            }
        }
        Ok(())
    }

    fn index_template(&mut self, template: TemplateDefinition) {
        let id = template.template.clone();

        // Index by tags
        for tag in &template.tags {
            self.by_tag.entry(tag.clone()).or_default().push(id.clone());
        }

        // Index by blocker
        for blocker in &template.workflow_context.resolves_blockers {
            self.by_blocker
                .entry(blocker.clone())
                .or_default()
                .push(id.clone());
        }

        // Index by workflow/state combinations
        for workflow in &template.workflow_context.applicable_workflows {
            for state in &template.workflow_context.applicable_states {
                self.by_workflow_state
                    .entry((workflow.clone(), state.clone()))
                    .or_default()
                    .push(id.clone());
            }
        }

        self.templates.insert(id, template);
    }

    /// Register a template programmatically
    pub fn register(&mut self, template: TemplateDefinition) {
        self.index_template(template);
    }

    /// Get template by ID
    pub fn get(&self, id: &str) -> Option<&TemplateDefinition> {
        self.templates.get(id)
    }

    /// Find templates by tag
    pub fn find_by_tag(&self, tag: &str) -> Vec<&TemplateDefinition> {
        self.by_tag
            .get(tag)
            .map(|ids| ids.iter().filter_map(|id| self.templates.get(id)).collect())
            .unwrap_or_default()
    }

    /// Find templates that resolve a blocker type
    pub fn find_by_blocker(&self, blocker_type: &str) -> Vec<&TemplateDefinition> {
        // First try exact match
        if let Some(ids) = self.by_blocker.get(blocker_type) {
            return ids.iter().filter_map(|id| self.templates.get(id)).collect();
        }

        // Then try prefix match (e.g., "missing_role:DIRECTOR" matches "missing_role")
        let prefix = blocker_type.split(':').next().unwrap_or(blocker_type);
        self.by_blocker
            .get(prefix)
            .map(|ids| ids.iter().filter_map(|id| self.templates.get(id)).collect())
            .unwrap_or_default()
    }

    /// Find templates for a workflow state
    pub fn find_by_workflow_state(&self, workflow: &str, state: &str) -> Vec<&TemplateDefinition> {
        self.by_workflow_state
            .get(&(workflow.to_string(), state.to_string()))
            .map(|ids| ids.iter().filter_map(|id| self.templates.get(id)).collect())
            .unwrap_or_default()
    }

    /// Search templates by text (name, description, tags)
    pub fn search(&self, query: &str) -> Vec<&TemplateDefinition> {
        let query_lower = query.to_lowercase();
        self.templates
            .values()
            .filter(|t| {
                t.metadata.name.to_lowercase().contains(&query_lower)
                    || t.metadata.summary.to_lowercase().contains(&query_lower)
                    || t.metadata.description.to_lowercase().contains(&query_lower)
                    || t.template.to_lowercase().contains(&query_lower)
                    || t.tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(&query_lower))
            })
            .collect()
    }

    /// List all templates
    pub fn list(&self) -> Vec<&TemplateDefinition> {
        self.templates.values().collect()
    }

    /// List all template IDs
    pub fn list_ids(&self) -> Vec<&str> {
        self.templates.keys().map(|s| s.as_str()).collect()
    }

    /// Get count of templates
    pub fn len(&self) -> usize {
        self.templates.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    /// Get all unique tags
    pub fn all_tags(&self) -> Vec<&str> {
        self.by_tag.keys().map(|s| s.as_str()).collect()
    }

    /// Get all blockers that have resolving templates
    pub fn all_blockers(&self) -> Vec<&str> {
        self.by_blocker.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_template() -> TemplateDefinition {
        serde_yaml::from_str(
            r#"
template: test-template
version: 1
metadata:
  name: Test Template
  summary: A test template
tags:
  - test
  - example
workflow_context:
  applicable_workflows:
    - test_workflow
  applicable_states:
    - TEST_STATE
  resolves_blockers:
    - test_blocker
params: {}
body: "(test.verb)"
"#,
        )
        .unwrap()
    }

    #[test]
    fn test_register_and_get() {
        let mut registry = TemplateRegistry::new();
        registry.register(sample_template());

        assert!(registry.get("test-template").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_find_by_tag() {
        let mut registry = TemplateRegistry::new();
        registry.register(sample_template());

        let found = registry.find_by_tag("test");
        assert_eq!(found.len(), 1);

        let not_found = registry.find_by_tag("other");
        assert!(not_found.is_empty());
    }

    #[test]
    fn test_find_by_blocker() {
        let mut registry = TemplateRegistry::new();
        registry.register(sample_template());

        let found = registry.find_by_blocker("test_blocker");
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn test_find_by_workflow_state() {
        let mut registry = TemplateRegistry::new();
        registry.register(sample_template());

        let found = registry.find_by_workflow_state("test_workflow", "TEST_STATE");
        assert_eq!(found.len(), 1);

        let not_found = registry.find_by_workflow_state("other", "OTHER");
        assert!(not_found.is_empty());
    }

    #[test]
    fn test_search() {
        let mut registry = TemplateRegistry::new();
        registry.register(sample_template());

        let found = registry.search("test");
        assert_eq!(found.len(), 1);

        let found_summary = registry.search("summary");
        assert_eq!(found_summary.len(), 1);
    }
}
