//! Entity Dependency Registry
//!
//! Unified configuration-driven dependency model for all entity types.
//! This replaces the resource-specific `ResourceDependencyGraph` with a
//! generic system that handles CBUs, cases, funds, resources, and more.
//!
//! ## Architecture
//!
//! Dependencies are loaded from the `entity_type_dependencies` table and
//! cached in a registry. The registry provides:
//!
//! - `dependencies_of(type, subtype)` - What does this entity need?
//! - `dependents_of(type, subtype)` - What entities need this?
//! - `topological_sort_unified()` - Order entities for execution
//!
//! ## Dependency Kinds
//!
//! - `required`: Must exist before this entity can be created
//! - `optional`: May be linked if available
//! - `lifecycle`: State transition dependency (e.g., case must be OPEN)

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// TYPES
// =============================================================================

/// Row type for entity_type_dependencies query
#[cfg(feature = "database")]
type DepRow = (
    String,         // from_type
    Option<String>, // from_subtype
    String,         // to_type
    Option<String>, // to_subtype
    Option<String>, // via_arg
    String,         // dependency_kind
    Option<String>, // condition_expr
    i32,            // priority
);

/// Kind of dependency relationship
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DependencyKind {
    /// Must exist before this entity can be created
    Required,
    /// May be linked if available
    Optional,
    /// State transition dependency
    Lifecycle,
}

impl From<&str> for DependencyKind {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "optional" => DependencyKind::Optional,
            "lifecycle" => DependencyKind::Lifecycle,
            _ => DependencyKind::Required,
        }
    }
}

impl std::fmt::Display for DependencyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DependencyKind::Required => write!(f, "required"),
            DependencyKind::Optional => write!(f, "optional"),
            DependencyKind::Lifecycle => write!(f, "lifecycle"),
        }
    }
}

/// A single entity dependency
#[derive(Debug, Clone)]
pub struct EntityDep {
    /// Source entity type (e.g., "resource")
    pub from_type: String,
    /// Source entity subtype (e.g., "CUSTODY_ACCT")
    pub from_subtype: Option<String>,
    /// Target entity type this depends on
    pub to_type: String,
    /// Target entity subtype
    pub to_subtype: Option<String>,
    /// Argument name to inject the dependency reference
    pub via_arg: Option<String>,
    /// Kind of dependency
    pub kind: DependencyKind,
    /// Optional condition expression (for future use)
    pub condition_expr: Option<String>,
    /// Priority for ordering (lower = higher priority)
    pub priority: i32,
}

/// Entity type key (type + optional subtype)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityTypeKey {
    pub entity_type: String,
    pub subtype: Option<String>,
}

impl EntityTypeKey {
    pub fn new(entity_type: impl Into<String>, subtype: Option<impl Into<String>>) -> Self {
        Self {
            entity_type: entity_type.into(),
            subtype: subtype.map(|s| s.into()),
        }
    }

    /// Match with wildcard support (None matches any subtype)
    pub fn matches(&self, other: &EntityTypeKey) -> bool {
        if self.entity_type != other.entity_type {
            return false;
        }
        match (&self.subtype, &other.subtype) {
            (None, _) | (_, None) => true,
            (Some(a), Some(b)) => a == b,
        }
    }
}

impl std::fmt::Display for EntityTypeKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.subtype {
            Some(st) => write!(f, "{}:{}", self.entity_type, st),
            None => write!(f, "{}", self.entity_type),
        }
    }
}

/// Registry of entity dependencies loaded from database
#[derive(Debug, Clone)]
pub struct EntityDependencyRegistry {
    /// All dependencies indexed by source entity
    deps_by_source: HashMap<EntityTypeKey, Vec<EntityDep>>,
    /// Reverse index: what depends on this entity
    deps_by_target: HashMap<EntityTypeKey, Vec<EntityDep>>,
    /// All known entity types
    known_types: HashSet<EntityTypeKey>,
}

impl EntityDependencyRegistry {
    /// Create empty registry
    pub fn new() -> Self {
        Self {
            deps_by_source: HashMap::new(),
            deps_by_target: HashMap::new(),
            known_types: HashSet::new(),
        }
    }

    /// Load dependencies from database
    #[cfg(feature = "database")]
    pub async fn load(pool: &PgPool) -> Result<Self, sqlx::Error> {
        let rows: Vec<DepRow> = sqlx::query_as(
            r#"SELECT from_type, from_subtype, to_type, to_subtype, via_arg,
                      dependency_kind, condition_expr, priority
               FROM "ob-poc".entity_type_dependencies
               WHERE is_active = true
               ORDER BY priority ASC"#,
        )
        .fetch_all(pool)
        .await?;

        let mut registry = Self::new();

        for (from_type, from_subtype, to_type, to_subtype, via_arg, kind, cond, priority) in rows {
            let dep = EntityDep {
                from_type: from_type.clone(),
                from_subtype: from_subtype.clone(),
                to_type: to_type.clone(),
                to_subtype: to_subtype.clone(),
                via_arg,
                kind: DependencyKind::from(kind.as_str()),
                condition_expr: cond,
                priority,
            };

            let source_key = EntityTypeKey::new(&from_type, from_subtype.as_ref());
            let target_key = EntityTypeKey::new(&to_type, to_subtype.as_ref());

            registry.known_types.insert(source_key.clone());
            registry.known_types.insert(target_key.clone());

            registry
                .deps_by_source
                .entry(source_key)
                .or_default()
                .push(dep.clone());

            registry
                .deps_by_target
                .entry(target_key)
                .or_default()
                .push(dep);
        }

        Ok(registry)
    }

    /// Get dependencies for an entity type (what does it need?)
    pub fn dependencies_of(&self, entity_type: &str, subtype: Option<&str>) -> Vec<&EntityDep> {
        let key = EntityTypeKey::new(entity_type, subtype);

        // Try exact match first
        if let Some(deps) = self.deps_by_source.get(&key) {
            return deps.iter().collect();
        }

        // Fall back to type-only match (subtype=None acts as wildcard)
        let type_only_key = EntityTypeKey::new(entity_type, None::<String>);
        self.deps_by_source
            .get(&type_only_key)
            .map(|deps| deps.iter().collect())
            .unwrap_or_default()
    }

    /// Get required dependencies only
    pub fn required_dependencies_of(
        &self,
        entity_type: &str,
        subtype: Option<&str>,
    ) -> Vec<&EntityDep> {
        self.dependencies_of(entity_type, subtype)
            .into_iter()
            .filter(|d| d.kind == DependencyKind::Required)
            .collect()
    }

    /// Get dependents of an entity type (what needs it?)
    pub fn dependents_of(&self, entity_type: &str, subtype: Option<&str>) -> Vec<&EntityDep> {
        let key = EntityTypeKey::new(entity_type, subtype);

        // Try exact match first
        if let Some(deps) = self.deps_by_target.get(&key) {
            return deps.iter().collect();
        }

        // Fall back to type-only match
        let type_only_key = EntityTypeKey::new(entity_type, None::<String>);
        self.deps_by_target
            .get(&type_only_key)
            .map(|deps| deps.iter().collect())
            .unwrap_or_default()
    }

    /// Check if an entity type has any dependencies
    pub fn has_dependencies(&self, entity_type: &str, subtype: Option<&str>) -> bool {
        !self.dependencies_of(entity_type, subtype).is_empty()
    }

    /// Get all known entity types
    pub fn known_types(&self) -> impl Iterator<Item = &EntityTypeKey> {
        self.known_types.iter()
    }

    /// Add a dependency programmatically (for testing)
    pub fn add_dependency(&mut self, dep: EntityDep) {
        let source_key = EntityTypeKey::new(&dep.from_type, dep.from_subtype.as_ref());
        let target_key = EntityTypeKey::new(&dep.to_type, dep.to_subtype.as_ref());

        self.known_types.insert(source_key.clone());
        self.known_types.insert(target_key.clone());

        self.deps_by_source
            .entry(source_key)
            .or_default()
            .push(dep.clone());

        self.deps_by_target.entry(target_key).or_default().push(dep);
    }
}

impl Default for EntityDependencyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// TOPOLOGICAL SORT
// =============================================================================

/// Entity instance for topological sorting
#[derive(Debug, Clone)]
pub struct EntityInstance {
    /// Unique identifier for this instance (e.g., binding name or temp ID)
    pub id: String,
    /// Entity type
    pub entity_type: String,
    /// Entity subtype (if applicable)
    pub subtype: Option<String>,
    /// Dependencies on other instances (by their IDs)
    pub depends_on: Vec<String>,
}

/// Result of topological sort
#[derive(Debug)]
pub struct TopoSortUnifiedResult {
    /// Sorted entity IDs
    pub sorted: Vec<String>,
    /// Parallel execution stages (if applicable)
    pub stages: Vec<Vec<String>>,
    /// Whether any reordering occurred
    pub reordered: bool,
}

/// Topological sort error
#[derive(Debug, Clone)]
pub enum TopoSortUnifiedError {
    /// Cyclic dependency detected
    CyclicDependency { cycle: Vec<String> },
    /// Missing dependency
    MissingDependency { from: String, to: String },
}

impl std::fmt::Display for TopoSortUnifiedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TopoSortUnifiedError::CyclicDependency { cycle } => {
                write!(f, "Cyclic dependency detected: {}", cycle.join(" → "))
            }
            TopoSortUnifiedError::MissingDependency { from, to } => {
                write!(f, "Missing dependency: {} requires {}", from, to)
            }
        }
    }
}

impl std::error::Error for TopoSortUnifiedError {}

/// Topologically sort entity instances using Kahn's algorithm
///
/// Returns both a linear ordering and parallel execution stages.
pub fn topological_sort_unified(
    instances: &[EntityInstance],
) -> Result<TopoSortUnifiedResult, TopoSortUnifiedError> {
    if instances.is_empty() {
        return Ok(TopoSortUnifiedResult {
            sorted: vec![],
            stages: vec![],
            reordered: false,
        });
    }

    // Build ID -> index mapping
    let id_to_idx: HashMap<&str, usize> = instances
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.as_str(), i))
        .collect();

    // Build adjacency list and in-degree count
    // Edge direction: dependency -> dependent (i.e., if A depends on B, edge B -> A)
    let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut in_degree: HashMap<usize, usize> = HashMap::new();

    for (idx, instance) in instances.iter().enumerate() {
        adj.entry(idx).or_default();
        in_degree.entry(idx).or_insert(0);

        for dep_id in &instance.depends_on {
            if let Some(&dep_idx) = id_to_idx.get(dep_id.as_str()) {
                // dep_idx -> idx (dependency must come before dependent)
                adj.entry(dep_idx).or_default().push(idx);
                *in_degree.entry(idx).or_insert(0) += 1;
            }
            // If dependency not found in instances, it's external (already satisfied)
        }
    }

    // Kahn's algorithm with stage tracking
    let mut stages: Vec<Vec<String>> = Vec::new();
    let mut sorted: Vec<String> = Vec::new();
    let mut remaining: HashSet<usize> = (0..instances.len()).collect();

    while !remaining.is_empty() {
        // Find all nodes with in_degree == 0
        let mut stage: Vec<usize> = remaining
            .iter()
            .filter(|idx| in_degree.get(idx).copied().unwrap_or(0) == 0)
            .copied()
            .collect();

        if stage.is_empty() {
            // Cycle detected
            let cycle: Vec<String> = remaining
                .iter()
                .map(|&idx| instances[idx].id.clone())
                .collect();
            return Err(TopoSortUnifiedError::CyclicDependency { cycle });
        }

        // Sort for deterministic ordering
        stage.sort_by_key(|&idx| &instances[idx].id);

        // Add to stages and sorted list
        let stage_ids: Vec<String> = stage.iter().map(|&idx| instances[idx].id.clone()).collect();
        sorted.extend(stage_ids.clone());
        stages.push(stage_ids);

        // Remove stage from remaining, update in-degrees
        for idx in &stage {
            remaining.remove(idx);
            if let Some(dependents) = adj.get(idx) {
                for &dep_idx in dependents {
                    if let Some(deg) = in_degree.get_mut(&dep_idx) {
                        *deg = deg.saturating_sub(1);
                    }
                }
            }
        }
    }

    // Check if reordering occurred (compare to original order)
    let original_order: Vec<String> = instances.iter().map(|e| e.id.clone()).collect();
    let reordered = sorted != original_order;

    Ok(TopoSortUnifiedResult {
        sorted,
        stages,
        reordered,
    })
}

// =============================================================================
// GLOBAL REGISTRY (lazy-loaded)
// =============================================================================

static ENTITY_DEPS_REGISTRY: OnceLock<EntityDependencyRegistry> = OnceLock::new();

/// Get or initialize the global entity dependency registry
///
/// This is a compile-time fallback that returns an empty registry.
/// In production, use `init_entity_deps()` to load from database.
pub fn entity_deps() -> &'static EntityDependencyRegistry {
    ENTITY_DEPS_REGISTRY.get_or_init(EntityDependencyRegistry::new)
}

/// Initialize the global entity dependency registry from database
#[cfg(feature = "database")]
pub async fn init_entity_deps(pool: &PgPool) -> Result<(), sqlx::Error> {
    let registry = EntityDependencyRegistry::load(pool).await?;
    let _ = ENTITY_DEPS_REGISTRY.set(registry);
    Ok(())
}

/// Reset the global registry (for testing)
#[cfg(test)]
pub fn reset_entity_deps() {
    // OnceLock doesn't support reset, but we can work around this in tests
    // by using a separate test registry
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_registry() -> EntityDependencyRegistry {
        let mut registry = EntityDependencyRegistry::new();

        // CBU hierarchy: fund depends on cbu
        registry.add_dependency(EntityDep {
            from_type: "fund".to_string(),
            from_subtype: None,
            to_type: "cbu".to_string(),
            to_subtype: None,
            via_arg: Some("cbu-id".to_string()),
            kind: DependencyKind::Required,
            condition_expr: None,
            priority: 100,
        });

        // KYC case depends on cbu
        registry.add_dependency(EntityDep {
            from_type: "kyc_case".to_string(),
            from_subtype: None,
            to_type: "cbu".to_string(),
            to_subtype: None,
            via_arg: Some("cbu-id".to_string()),
            kind: DependencyKind::Required,
            condition_expr: None,
            priority: 100,
        });

        // Resource dependencies: CUSTODY_ACCT depends on SETTLE_ACCT
        registry.add_dependency(EntityDep {
            from_type: "resource".to_string(),
            from_subtype: Some("CUSTODY_ACCT".to_string()),
            to_type: "resource".to_string(),
            to_subtype: Some("SETTLE_ACCT".to_string()),
            via_arg: Some("settlement-account-url".to_string()),
            kind: DependencyKind::Required,
            condition_expr: None,
            priority: 100,
        });

        // SWIFT_CONN depends on CUSTODY_ACCT
        registry.add_dependency(EntityDep {
            from_type: "resource".to_string(),
            from_subtype: Some("SWIFT_CONN".to_string()),
            to_type: "resource".to_string(),
            to_subtype: Some("CUSTODY_ACCT".to_string()),
            via_arg: Some("custody-account-url".to_string()),
            kind: DependencyKind::Required,
            condition_expr: None,
            priority: 100,
        });

        registry
    }

    #[test]
    fn test_dependencies_of() {
        let registry = create_test_registry();

        // Fund depends on CBU
        let fund_deps = registry.dependencies_of("fund", None);
        assert_eq!(fund_deps.len(), 1);
        assert_eq!(fund_deps[0].to_type, "cbu");

        // CUSTODY_ACCT depends on SETTLE_ACCT
        let custody_deps = registry.dependencies_of("resource", Some("CUSTODY_ACCT"));
        assert_eq!(custody_deps.len(), 1);
        assert_eq!(custody_deps[0].to_subtype, Some("SETTLE_ACCT".to_string()));
    }

    #[test]
    fn test_dependents_of() {
        let registry = create_test_registry();

        // What depends on CBU?
        let cbu_dependents = registry.dependents_of("cbu", None);
        assert_eq!(cbu_dependents.len(), 2); // fund and kyc_case

        // What depends on CUSTODY_ACCT?
        let custody_dependents = registry.dependents_of("resource", Some("CUSTODY_ACCT"));
        assert_eq!(custody_dependents.len(), 1);
        assert_eq!(
            custody_dependents[0].from_subtype,
            Some("SWIFT_CONN".to_string())
        );
    }

    #[test]
    fn test_topo_sort_simple() {
        let instances = vec![
            EntityInstance {
                id: "cbu1".to_string(),
                entity_type: "cbu".to_string(),
                subtype: None,
                depends_on: vec![],
            },
            EntityInstance {
                id: "fund1".to_string(),
                entity_type: "fund".to_string(),
                subtype: None,
                depends_on: vec!["cbu1".to_string()],
            },
        ];

        let result = topological_sort_unified(&instances).unwrap();

        // CBU should come before fund
        assert_eq!(result.sorted, vec!["cbu1", "fund1"]);
        assert_eq!(result.stages.len(), 2);
        assert_eq!(result.stages[0], vec!["cbu1"]);
        assert_eq!(result.stages[1], vec!["fund1"]);
    }

    #[test]
    fn test_topo_sort_parallel_stages() {
        let instances = vec![
            EntityInstance {
                id: "settle".to_string(),
                entity_type: "resource".to_string(),
                subtype: Some("SETTLE_ACCT".to_string()),
                depends_on: vec![],
            },
            EntityInstance {
                id: "custody".to_string(),
                entity_type: "resource".to_string(),
                subtype: Some("CUSTODY_ACCT".to_string()),
                depends_on: vec!["settle".to_string()],
            },
            EntityInstance {
                id: "swift".to_string(),
                entity_type: "resource".to_string(),
                subtype: Some("SWIFT_CONN".to_string()),
                depends_on: vec!["custody".to_string()],
            },
            EntityInstance {
                id: "ca".to_string(),
                entity_type: "resource".to_string(),
                subtype: Some("CA_PLATFORM".to_string()),
                depends_on: vec!["custody".to_string()],
            },
        ];

        let result = topological_sort_unified(&instances).unwrap();

        // Should have 3 stages: settle -> custody -> (swift, ca)
        assert_eq!(result.stages.len(), 3);
        assert_eq!(result.stages[0], vec!["settle"]);
        assert_eq!(result.stages[1], vec!["custody"]);
        assert!(result.stages[2].contains(&"swift".to_string()));
        assert!(result.stages[2].contains(&"ca".to_string()));
    }

    #[test]
    fn test_topo_sort_cycle_detection() {
        let instances = vec![
            EntityInstance {
                id: "a".to_string(),
                entity_type: "test".to_string(),
                subtype: None,
                depends_on: vec!["b".to_string()],
            },
            EntityInstance {
                id: "b".to_string(),
                entity_type: "test".to_string(),
                subtype: None,
                depends_on: vec!["a".to_string()],
            },
        ];

        let result = topological_sort_unified(&instances);
        assert!(result.is_err());
        if let Err(TopoSortUnifiedError::CyclicDependency { cycle }) = result {
            assert!(cycle.contains(&"a".to_string()));
            assert!(cycle.contains(&"b".to_string()));
        }
    }

    #[test]
    fn test_topo_sort_external_dependency() {
        // When a dependency references an ID not in the instances list,
        // it's treated as external (already satisfied)
        let instances = vec![EntityInstance {
            id: "fund1".to_string(),
            entity_type: "fund".to_string(),
            subtype: None,
            depends_on: vec!["external_cbu".to_string()], // Not in instances
        }];

        let result = topological_sort_unified(&instances).unwrap();
        assert_eq!(result.sorted, vec!["fund1"]);
    }

    #[test]
    fn test_entity_type_key_matching() {
        let key1 = EntityTypeKey::new("resource", Some("CUSTODY_ACCT"));
        let key2 = EntityTypeKey::new("resource", Some("CUSTODY_ACCT"));
        let key3 = EntityTypeKey::new("resource", None::<String>);
        let key4 = EntityTypeKey::new("resource", Some("SWIFT_CONN"));

        assert!(key1.matches(&key2)); // Exact match
        assert!(key1.matches(&key3)); // Wildcard match
        assert!(key3.matches(&key1)); // Wildcard match (symmetric)
        assert!(!key1.matches(&key4)); // Different subtypes
    }
}
