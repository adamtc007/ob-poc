//! Document Bundle Registry
//!
//! In-memory registry of document bundles loaded from YAML files.
//! Handles inheritance resolution.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use tracing::{debug, info, warn};

use super::types::{DocsBundleDef, ResolvedBundleDocument};

/// Registry of document bundles loaded from YAML
#[derive(Debug, Clone)]
pub struct DocsBundleRegistry {
    /// All bundles indexed by ID
    bundles: HashMap<String, DocsBundleDef>,

    /// Pre-resolved bundles (with inheritance applied)
    resolved_cache: HashMap<String, Vec<ResolvedBundleDocument>>,
}

impl DocsBundleRegistry {
    /// Create an empty registry
    pub fn new() -> Self {
        Self {
            bundles: HashMap::new(),
            resolved_cache: HashMap::new(),
        }
    }

    /// Load bundles from a directory containing YAML files
    pub fn load_from_dir(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        let mut registry = Self::new();

        if !dir.exists() {
            warn!("Document bundles directory does not exist: {:?}", dir);
            return Ok(registry);
        }

        for entry in std::fs::read_dir(dir)
            .with_context(|| format!("Failed to read document bundles directory: {:?}", dir))?
        {
            let entry = entry?;
            let path = entry.path();

            if path
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml")
            {
                debug!("Loading document bundles from {:?}", path);
                registry.load_file(&path)?;
            }
        }

        // Build resolved cache after all bundles loaded
        registry.build_resolved_cache()?;

        info!(
            "Loaded {} document bundles from {:?}",
            registry.bundles.len(),
            dir
        );

        Ok(registry)
    }

    /// Load bundles from a single YAML file
    pub fn load_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read bundle file: {:?}", path))?;

        let bundles: BundleFile = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse bundle file: {:?}", path))?;

        for bundle in bundles.bundles {
            debug!("Loaded bundle: {}", bundle.id);
            self.bundles.insert(bundle.id.clone(), bundle);
        }

        Ok(())
    }

    /// Get a bundle by ID (raw, without inheritance resolution)
    pub fn get(&self, bundle_id: &str) -> Option<&DocsBundleDef> {
        self.bundles.get(bundle_id)
    }

    /// Get a bundle with inheritance resolved
    pub fn get_resolved(&self, bundle_id: &str) -> Option<&[ResolvedBundleDocument]> {
        self.resolved_cache.get(bundle_id).map(|v| v.as_slice())
    }

    /// List all bundle IDs
    pub fn list_bundle_ids(&self) -> Vec<&str> {
        self.bundles.keys().map(|s| s.as_str()).collect()
    }

    /// List all currently effective bundles
    pub fn list_effective(&self) -> Vec<&DocsBundleDef> {
        self.bundles.values().filter(|b| b.is_effective()).collect()
    }

    /// Build the resolved cache for all bundles
    fn build_resolved_cache(&mut self) -> Result<()> {
        let bundle_ids: Vec<String> = self.bundles.keys().cloned().collect();

        for bundle_id in bundle_ids {
            let resolved = self.resolve_inheritance(&bundle_id)?;
            self.resolved_cache.insert(bundle_id, resolved);
        }

        Ok(())
    }

    /// Resolve inheritance for a bundle
    fn resolve_inheritance(&self, bundle_id: &str) -> Result<Vec<ResolvedBundleDocument>> {
        let mut chain = Vec::new();
        let mut current_id = Some(bundle_id.to_string());
        let mut visited = std::collections::HashSet::new();

        // Walk up the inheritance chain
        while let Some(id) = current_id {
            if visited.contains(&id) {
                return Err(anyhow!(
                    "Circular inheritance detected in bundle: {}",
                    bundle_id
                ));
            }
            visited.insert(id.clone());

            let bundle = self
                .bundles
                .get(&id)
                .ok_or_else(|| anyhow!("Bundle not found: {} (referenced by {})", id, bundle_id))?;

            chain.push(bundle);
            current_id = bundle.extends.clone();

            // Safety limit
            if chain.len() > 10 {
                return Err(anyhow!(
                    "Inheritance chain too deep for bundle: {}",
                    bundle_id
                ));
            }
        }

        // Build resolved documents (child overrides parent)
        // Process from root (oldest ancestor) to leaf (the requested bundle)
        let mut docs: HashMap<String, ResolvedBundleDocument> = HashMap::new();

        for bundle in chain.iter().rev() {
            for doc in &bundle.documents {
                let resolved = ResolvedBundleDocument {
                    bundle_id: bundle_id.to_string(),
                    document_id: doc.id.clone(),
                    document_name: doc.name.clone(),
                    description: doc.description.clone(),
                    required: doc.required,
                    required_if: doc.required_if.clone(),
                    template_ref: doc.template_ref.clone(),
                    sort_order: doc.sort_order,
                    source_bundle_id: bundle.id.clone(),
                };
                docs.insert(doc.id.clone(), resolved);
            }
        }

        // Sort by sort_order then document_id
        let mut result: Vec<_> = docs.into_values().collect();
        result.sort_by(|a, b| {
            a.sort_order
                .cmp(&b.sort_order)
                .then_with(|| a.document_id.cmp(&b.document_id))
        });

        Ok(result)
    }
}

impl Default for DocsBundleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// YAML file structure for document bundles
#[derive(Debug, Deserialize)]
struct BundleFile {
    #[serde(default)]
    bundles: Vec<DocsBundleDef>,
}

use serde::Deserialize;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_bundles::types::BundleDocumentDef;
    use chrono::NaiveDate;

    fn make_test_registry() -> DocsBundleRegistry {
        let mut registry = DocsBundleRegistry::new();

        // Parent bundle
        let parent = DocsBundleDef {
            id: "docs.bundle.aif-baseline".to_string(),
            display_name: "AIF Baseline".to_string(),
            description: Some("Base AIF documents".to_string()),
            version: "2024-03".to_string(),
            effective_from: NaiveDate::from_ymd_opt(2024, 3, 1).unwrap(),
            effective_to: None,
            extends: None,
            documents: vec![
                BundleDocumentDef {
                    id: "ppm".to_string(),
                    name: "Private Placement Memo".to_string(),
                    description: None,
                    required: true,
                    required_if: None,
                    template_ref: None,
                    sort_order: 0,
                },
                BundleDocumentDef {
                    id: "lpa".to_string(),
                    name: "Limited Partnership Agreement".to_string(),
                    description: None,
                    required: true,
                    required_if: None,
                    template_ref: None,
                    sort_order: 1,
                },
            ],
        };

        // Child bundle extends parent
        let child = DocsBundleDef {
            id: "docs.bundle.hedge-baseline".to_string(),
            display_name: "Hedge Fund Baseline".to_string(),
            description: Some("Hedge fund documents".to_string()),
            version: "2024-03".to_string(),
            effective_from: NaiveDate::from_ymd_opt(2024, 3, 1).unwrap(),
            effective_to: None,
            extends: Some("docs.bundle.aif-baseline".to_string()),
            documents: vec![
                BundleDocumentDef {
                    id: "pba".to_string(),
                    name: "Prime Brokerage Agreement".to_string(),
                    description: None,
                    required: false,
                    required_if: Some("has-prime-broker".to_string()),
                    template_ref: None,
                    sort_order: 2,
                },
                // Override PPM with different template
                BundleDocumentDef {
                    id: "ppm".to_string(),
                    name: "Private Placement Memo (Hedge)".to_string(),
                    description: None,
                    required: true,
                    required_if: None,
                    template_ref: Some("hedge-ppm-template".to_string()),
                    sort_order: 0,
                },
            ],
        };

        registry.bundles.insert(parent.id.clone(), parent);
        registry.bundles.insert(child.id.clone(), child);
        registry.build_resolved_cache().unwrap();

        registry
    }

    #[test]
    fn test_get_bundle() {
        let registry = make_test_registry();

        let bundle = registry.get("docs.bundle.aif-baseline").unwrap();
        assert_eq!(bundle.display_name, "AIF Baseline");
        assert_eq!(bundle.documents.len(), 2);
    }

    #[test]
    fn test_inheritance_resolution() {
        let registry = make_test_registry();

        let resolved = registry.get_resolved("docs.bundle.hedge-baseline").unwrap();

        // Should have 3 documents: ppm (overridden), lpa (inherited), pba (new)
        assert_eq!(resolved.len(), 3);

        // PPM should be overridden by child
        let ppm = resolved.iter().find(|d| d.document_id == "ppm").unwrap();
        assert_eq!(ppm.document_name, "Private Placement Memo (Hedge)");
        assert_eq!(ppm.template_ref, Some("hedge-ppm-template".to_string()));
        assert_eq!(ppm.source_bundle_id, "docs.bundle.hedge-baseline");

        // LPA should be inherited from parent
        let lpa = resolved.iter().find(|d| d.document_id == "lpa").unwrap();
        assert_eq!(lpa.document_name, "Limited Partnership Agreement");
        assert_eq!(lpa.source_bundle_id, "docs.bundle.aif-baseline");

        // PBA should be from child
        let pba = resolved.iter().find(|d| d.document_id == "pba").unwrap();
        assert_eq!(pba.required_if, Some("has-prime-broker".to_string()));
    }

    #[test]
    fn test_parent_bundle_unchanged() {
        let registry = make_test_registry();

        let resolved = registry.get_resolved("docs.bundle.aif-baseline").unwrap();

        // Parent should have only its own 2 documents
        assert_eq!(resolved.len(), 2);
    }
}
