//! Render Policy - Controls what is visible and how verbose.
//!
//! The policy determines:
//! - LOD (Level of Detail) for field visibility
//! - Max depth for auto-expand
//! - Pagination limits
//! - Show/prune filters

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

/// Render policy controlling visualization behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderPolicy {
    /// Level of detail (0=icon, 1=short, 2=normal, 3=verbose).
    #[serde(default = "default_lod")]
    pub lod: u8,

    /// Maximum auto-expand depth from roots.
    #[serde(default = "default_max_depth")]
    pub max_depth: u8,

    /// Maximum items per list before pagination.
    #[serde(default = "default_max_items")]
    pub max_items_per_list: usize,

    /// Chambers/branches to show.
    #[serde(default)]
    pub show: ShowFilter,

    /// Paths/filters to prune.
    #[serde(default)]
    pub prune: PruneFilter,
}

fn default_lod() -> u8 {
    2
}

fn default_max_depth() -> u8 {
    3
}

fn default_max_items() -> usize {
    50
}

impl Default for RenderPolicy {
    fn default() -> Self {
        Self {
            lod: default_lod(),
            max_depth: default_max_depth(),
            max_items_per_list: default_max_items(),
            show: ShowFilter::default(),
            prune: PruneFilter::default(),
        }
    }
}

impl RenderPolicy {
    /// Create a minimal policy (LOD 0, depth 1).
    pub fn minimal() -> Self {
        Self {
            lod: 0,
            max_depth: 1,
            ..Default::default()
        }
    }

    /// Create a verbose policy (LOD 3, depth 5).
    pub fn verbose() -> Self {
        Self {
            lod: 3,
            max_depth: 5,
            ..Default::default()
        }
    }

    /// Check if a field should be visible at current LOD.
    ///
    /// | Field | LOD 0 | LOD 1 | LOD 2 | LOD 3 |
    /// |-------|-------|-------|-------|-------|
    /// | id | ✓ | ✓ | ✓ | ✓ |
    /// | kind | - | ✓ | ✓ | ✓ |
    /// | glyph | ✓ | ✓ | ✓ | ✓ |
    /// | label_short | - | ✓ | ✓ | ✓ |
    /// | label_full | - | - | - | ✓ |
    /// | tags | - | - | ✓ | ✓ |
    /// | summary | - | - | ✓ | ✓ |
    /// | branches | - | - | (c) | ✓ |
    /// | attributes | - | - | - | ✓ |
    /// | provenance | - | - | - | ✓ |
    /// | links | - | - | ✓ | ✓ |
    pub fn field_visible(&self, field: &str) -> bool {
        match field {
            "id" | "glyph" => true,
            "kind" | "label_short" => self.lod >= 1,
            "tags" | "summary" | "links" => self.lod >= 2,
            "branches" => self.lod >= 2, // collapsed at 2, expanded at 3
            "label_full" | "attributes" | "provenance" => self.lod >= 3,
            _ => self.lod >= 2,
        }
    }

    /// Check if a node at given depth should auto-expand.
    pub fn should_auto_expand(&self, depth: usize) -> bool {
        depth < self.max_depth as usize
    }

    /// Check if a path should be pruned.
    pub fn is_pruned(&self, path: &str) -> bool {
        for pattern in &self.prune.exclude_paths {
            if path_matches(path, pattern) {
                return true;
            }
        }
        false
    }

    /// Compute a hash of the policy for cache invalidation.
    pub fn policy_hash(&self) -> String {
        let mut hasher = DefaultHasher::new();
        self.lod.hash(&mut hasher);
        self.max_depth.hash(&mut hasher);
        self.max_items_per_list.hash(&mut hasher);
        // Hash show filters
        for chamber in &self.show.chambers {
            chamber.hash(&mut hasher);
        }
        for branch in &self.show.branches {
            branch.hash(&mut hasher);
        }
        // Hash prune patterns
        for path in &self.prune.exclude_paths {
            path.hash(&mut hasher);
        }
        format!("{:016x}", hasher.finish())
    }
}

/// Simple glob-like path matching.
fn path_matches(path: &str, pattern: &str) -> bool {
    if pattern.contains('*') {
        // Very simple wildcard matching
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 2 {
            let (prefix, suffix) = (parts[0], parts[1]);
            return path.starts_with(prefix) && path.ends_with(suffix);
        }
    }
    path == pattern || path.starts_with(&format!("{}:", pattern))
}

/// Filter for what to show.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShowFilter {
    /// Chambers to include (empty = all).
    #[serde(default)]
    pub chambers: Vec<String>,

    /// Branch names to include (empty = all).
    #[serde(default)]
    pub branches: Vec<String>,

    /// Node kinds to include (empty = all).
    #[serde(default)]
    pub node_kinds: Vec<String>,
}

impl ShowFilter {
    /// Check if a chamber should be shown.
    pub fn chamber_visible(&self, chamber: &str) -> bool {
        self.chambers.is_empty() || self.chambers.iter().any(|c| c == chamber)
    }

    /// Check if a branch should be shown.
    pub fn branch_visible(&self, branch: &str) -> bool {
        self.branches.is_empty() || self.branches.iter().any(|b| b == branch)
    }
}

/// Filter for what to prune.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PruneFilter {
    /// Glob patterns for paths to exclude.
    #[serde(default)]
    pub exclude_paths: Vec<String>,

    /// Kind-specific filters.
    #[serde(default)]
    pub filters: BTreeMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy() {
        let policy = RenderPolicy::default();
        assert_eq!(policy.lod, 2);
        assert_eq!(policy.max_depth, 3);
        assert_eq!(policy.max_items_per_list, 50);
    }

    #[test]
    fn test_field_visibility() {
        let mut policy = RenderPolicy::default();

        // LOD 0 - minimal
        policy.lod = 0;
        assert!(policy.field_visible("id"));
        assert!(policy.field_visible("glyph"));
        assert!(!policy.field_visible("label_short"));
        assert!(!policy.field_visible("provenance"));

        // LOD 1 - short
        policy.lod = 1;
        assert!(policy.field_visible("label_short"));
        assert!(policy.field_visible("kind"));
        assert!(!policy.field_visible("tags"));

        // LOD 2 - normal
        policy.lod = 2;
        assert!(policy.field_visible("tags"));
        assert!(policy.field_visible("summary"));
        assert!(policy.field_visible("links"));
        assert!(!policy.field_visible("provenance"));

        // LOD 3 - verbose
        policy.lod = 3;
        assert!(policy.field_visible("provenance"));
        assert!(policy.field_visible("attributes"));
        assert!(policy.field_visible("label_full"));
    }

    #[test]
    fn test_auto_expand() {
        let policy = RenderPolicy {
            max_depth: 3,
            ..Default::default()
        };

        assert!(policy.should_auto_expand(0));
        assert!(policy.should_auto_expand(1));
        assert!(policy.should_auto_expand(2));
        assert!(!policy.should_auto_expand(3));
        assert!(!policy.should_auto_expand(5));
    }

    #[test]
    fn test_path_pruning() {
        let policy = RenderPolicy {
            prune: PruneFilter {
                exclude_paths: vec![
                    "cbu:*.documents".to_string(),
                    "register:control".to_string(),
                ],
                filters: BTreeMap::new(),
            },
            ..Default::default()
        };

        assert!(policy.is_pruned("cbu:allianz.documents"));
        assert!(policy.is_pruned("register:control"));
        assert!(policy.is_pruned("register:control:edge:001"));
        assert!(!policy.is_pruned("cbu:allianz:members"));
        assert!(!policy.is_pruned("register:economic"));
    }

    #[test]
    fn test_policy_hash_determinism() {
        let policy1 = RenderPolicy::default();
        let policy2 = RenderPolicy::default();

        assert_eq!(policy1.policy_hash(), policy2.policy_hash());

        let policy3 = RenderPolicy {
            lod: 3,
            ..Default::default()
        };
        assert_ne!(policy1.policy_hash(), policy3.policy_hash());
    }

    #[test]
    fn test_show_filter() {
        let filter = ShowFilter {
            chambers: vec!["cbu".to_string(), "matrix".to_string()],
            branches: vec!["members".to_string()],
            node_kinds: vec![],
        };

        assert!(filter.chamber_visible("cbu"));
        assert!(filter.chamber_visible("matrix"));
        assert!(!filter.chamber_visible("registers"));

        assert!(filter.branch_visible("members"));
        assert!(!filter.branch_visible("products"));
    }

    #[test]
    fn test_empty_show_filter_shows_all() {
        let filter = ShowFilter::default();

        assert!(filter.chamber_visible("anything"));
        assert!(filter.branch_visible("anything"));
    }
}
