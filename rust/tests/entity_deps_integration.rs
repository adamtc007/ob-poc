//! Integration tests for unified entity dependency DAG
//!
//! These tests verify:
//! 1. EntityDependencyRegistry loads from entity_type_dependencies table
//! 2. Dependencies are correctly queried (dependencies_of, dependents_of)
//! 3. topological_sort_unified produces correct ordering
//! 4. The unified system matches behavior of legacy resource_dependencies

use ob_poc::dsl_v2::entity_deps::{
    topological_sort_unified, DependencyKind, EntityDep, EntityDependencyRegistry, EntityInstance,
    EntityTypeKey, TopoSortUnifiedError,
};
use sqlx::PgPool;
use std::collections::HashSet;

/// Helper to get test database pool
async fn get_test_pool() -> PgPool {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database")
}

#[tokio::test]
async fn test_registry_loads_from_database() {
    let pool = get_test_pool().await;
    let registry = EntityDependencyRegistry::load(&pool)
        .await
        .expect("Failed to load registry");

    // Should have loaded some dependencies
    let known_types: Vec<_> = registry.known_types().collect();
    assert!(
        !known_types.is_empty(),
        "Registry should have loaded entity types"
    );

    // Check for expected resource dependencies (migrated from resource_dependencies)
    // Note: The unified table uses "resource_instance" as the type
    let custody_deps = registry.dependencies_of("resource_instance", Some("CUSTODY_ACCT"));
    assert!(
        !custody_deps.is_empty(),
        "CUSTODY_ACCT should have dependencies"
    );

    // CUSTODY_ACCT should depend on SETTLE_ACCT
    let has_settle_dep = custody_deps
        .iter()
        .any(|d| d.to_subtype.as_deref() == Some("SETTLE_ACCT"));
    assert!(has_settle_dep, "CUSTODY_ACCT should depend on SETTLE_ACCT");
}

#[tokio::test]
async fn test_fund_hierarchy_dependencies() {
    let pool = get_test_pool().await;
    let registry = EntityDependencyRegistry::load(&pool)
        .await
        .expect("Failed to load registry");

    // Fund depends on CBU
    let fund_deps = registry.dependencies_of("fund", None);
    let has_cbu_dep = fund_deps.iter().any(|d| d.to_type == "cbu");
    assert!(has_cbu_dep, "Fund should depend on CBU");

    // KYC case depends on CBU
    let case_deps = registry.dependencies_of("kyc_case", None);
    let has_cbu_dep = case_deps.iter().any(|d| d.to_type == "cbu");
    assert!(has_cbu_dep, "KYC case should depend on CBU");
}

#[tokio::test]
async fn test_dependents_of_query() {
    let pool = get_test_pool().await;
    let registry = EntityDependencyRegistry::load(&pool)
        .await
        .expect("Failed to load registry");

    // What depends on CBU?
    let cbu_dependents = registry.dependents_of("cbu", None);

    // Should include fund and kyc_case at minimum
    let dependent_types: HashSet<_> = cbu_dependents
        .iter()
        .map(|d| d.from_type.as_str())
        .collect();
    assert!(
        dependent_types.contains("fund"),
        "Fund should be a dependent of CBU"
    );
    assert!(
        dependent_types.contains("kyc_case"),
        "KYC case should be a dependent of CBU"
    );
}

#[tokio::test]
async fn test_resource_dependency_chain() {
    let pool = get_test_pool().await;
    let registry = EntityDependencyRegistry::load(&pool)
        .await
        .expect("Failed to load registry");

    // Verify the resource dependency chain:
    // SWIFT_CONN -> CUSTODY_ACCT -> SETTLE_ACCT
    // Note: The unified table uses "resource_instance" as the type

    // SWIFT_CONN depends on CUSTODY_ACCT
    let swift_deps = registry.dependencies_of("resource_instance", Some("SWIFT_CONN"));
    let has_custody_dep = swift_deps
        .iter()
        .any(|d| d.to_subtype.as_deref() == Some("CUSTODY_ACCT"));
    assert!(has_custody_dep, "SWIFT_CONN should depend on CUSTODY_ACCT");

    // CUSTODY_ACCT depends on SETTLE_ACCT
    let custody_deps = registry.dependencies_of("resource_instance", Some("CUSTODY_ACCT"));
    let has_settle_dep = custody_deps
        .iter()
        .any(|d| d.to_subtype.as_deref() == Some("SETTLE_ACCT"));
    assert!(has_settle_dep, "CUSTODY_ACCT should depend on SETTLE_ACCT");

    // SETTLE_ACCT has no dependencies (leaf node)
    let settle_deps = registry.dependencies_of("resource_instance", Some("SETTLE_ACCT"));
    assert!(
        settle_deps.is_empty(),
        "SETTLE_ACCT should have no dependencies"
    );
}

#[tokio::test]
async fn test_via_arg_is_preserved() {
    let pool = get_test_pool().await;
    let registry = EntityDependencyRegistry::load(&pool)
        .await
        .expect("Failed to load registry");

    // Check that via_arg is correctly loaded
    // Note: The unified table uses "resource_instance" as the type
    let custody_deps = registry.dependencies_of("resource_instance", Some("CUSTODY_ACCT"));
    let settle_dep = custody_deps
        .iter()
        .find(|d| d.to_subtype.as_deref() == Some("SETTLE_ACCT"));

    assert!(settle_dep.is_some(), "Should find SETTLE_ACCT dependency");
    let settle_dep = settle_dep.unwrap();
    assert!(settle_dep.via_arg.is_some(), "via_arg should be populated");
    assert_eq!(
        settle_dep.via_arg.as_deref(),
        Some("settlement-account-url"),
        "via_arg should be settlement-account-url"
    );
}

#[test]
fn test_topo_sort_basic_chain() {
    // A -> B -> C (C depends on B, B depends on A)
    let instances = vec![
        EntityInstance {
            id: "c".to_string(),
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
        EntityInstance {
            id: "a".to_string(),
            entity_type: "test".to_string(),
            subtype: None,
            depends_on: vec![],
        },
    ];

    let result = topological_sort_unified(&instances).expect("Sort should succeed");

    // Should be ordered: a, b, c
    assert_eq!(result.sorted, vec!["a", "b", "c"]);
    assert_eq!(result.stages.len(), 3);
    assert!(result.reordered, "Should have reordered");
}

#[test]
fn test_topo_sort_parallel_execution() {
    // Diamond pattern: D depends on B and C, B and C depend on A
    //     A
    //    / \
    //   B   C
    //    \ /
    //     D
    let instances = vec![
        EntityInstance {
            id: "a".to_string(),
            entity_type: "test".to_string(),
            subtype: None,
            depends_on: vec![],
        },
        EntityInstance {
            id: "b".to_string(),
            entity_type: "test".to_string(),
            subtype: None,
            depends_on: vec!["a".to_string()],
        },
        EntityInstance {
            id: "c".to_string(),
            entity_type: "test".to_string(),
            subtype: None,
            depends_on: vec!["a".to_string()],
        },
        EntityInstance {
            id: "d".to_string(),
            entity_type: "test".to_string(),
            subtype: None,
            depends_on: vec!["b".to_string(), "c".to_string()],
        },
    ];

    let result = topological_sort_unified(&instances).expect("Sort should succeed");

    // Stage 0: a
    // Stage 1: b, c (parallel)
    // Stage 2: d
    assert_eq!(result.stages.len(), 3);
    assert_eq!(result.stages[0], vec!["a"]);
    assert_eq!(result.stages[1].len(), 2);
    assert!(result.stages[1].contains(&"b".to_string()));
    assert!(result.stages[1].contains(&"c".to_string()));
    assert_eq!(result.stages[2], vec!["d"]);
}

#[test]
fn test_topo_sort_cycle_detection() {
    // A -> B -> C -> A (cycle)
    let instances = vec![
        EntityInstance {
            id: "a".to_string(),
            entity_type: "test".to_string(),
            subtype: None,
            depends_on: vec!["c".to_string()],
        },
        EntityInstance {
            id: "b".to_string(),
            entity_type: "test".to_string(),
            subtype: None,
            depends_on: vec!["a".to_string()],
        },
        EntityInstance {
            id: "c".to_string(),
            entity_type: "test".to_string(),
            subtype: None,
            depends_on: vec!["b".to_string()],
        },
    ];

    let result = topological_sort_unified(&instances);
    assert!(result.is_err());
    if let Err(TopoSortUnifiedError::CyclicDependency { cycle }) = result {
        assert_eq!(cycle.len(), 3);
    } else {
        panic!("Expected CyclicDependency error");
    }
}

#[test]
fn test_topo_sort_external_dependencies() {
    // B depends on "external" which is not in the instances list
    // This simulates dependencies satisfied by prior execution context
    let instances = vec![
        EntityInstance {
            id: "b".to_string(),
            entity_type: "test".to_string(),
            subtype: None,
            depends_on: vec!["external".to_string()],
        },
        EntityInstance {
            id: "c".to_string(),
            entity_type: "test".to_string(),
            subtype: None,
            depends_on: vec!["b".to_string()],
        },
    ];

    let result = topological_sort_unified(&instances).expect("Sort should succeed");

    // External dependency is ignored (already satisfied)
    assert_eq!(result.sorted, vec!["b", "c"]);
}

#[test]
fn test_topo_sort_resource_provisioning_scenario() {
    // Real-world scenario: resource provisioning order
    let instances = vec![
        EntityInstance {
            id: "res_settle_acct".to_string(),
            entity_type: "resource".to_string(),
            subtype: Some("SETTLE_ACCT".to_string()),
            depends_on: vec![],
        },
        EntityInstance {
            id: "res_custody_acct".to_string(),
            entity_type: "resource".to_string(),
            subtype: Some("CUSTODY_ACCT".to_string()),
            depends_on: vec!["res_settle_acct".to_string()],
        },
        EntityInstance {
            id: "res_swift_conn".to_string(),
            entity_type: "resource".to_string(),
            subtype: Some("SWIFT_CONN".to_string()),
            depends_on: vec!["res_custody_acct".to_string()],
        },
        EntityInstance {
            id: "res_ca_platform".to_string(),
            entity_type: "resource".to_string(),
            subtype: Some("CA_PLATFORM".to_string()),
            depends_on: vec!["res_custody_acct".to_string()],
        },
    ];

    let result = topological_sort_unified(&instances).expect("Sort should succeed");

    // Stage 0: settle_acct (no deps)
    // Stage 1: custody_acct (depends on settle)
    // Stage 2: swift_conn, ca_platform (both depend on custody, parallel)
    assert_eq!(result.stages.len(), 3);
    assert_eq!(result.stages[0], vec!["res_settle_acct"]);
    assert_eq!(result.stages[1], vec!["res_custody_acct"]);
    assert!(result.stages[2].contains(&"res_ca_platform".to_string()));
    assert!(result.stages[2].contains(&"res_swift_conn".to_string()));
}

#[test]
fn test_entity_type_key_display() {
    let key1 = EntityTypeKey::new("resource", Some("CUSTODY_ACCT"));
    assert_eq!(format!("{}", key1), "resource:CUSTODY_ACCT");

    let key2 = EntityTypeKey::new("cbu", None::<String>);
    assert_eq!(format!("{}", key2), "cbu");
}

#[test]
fn test_dependency_kind_from_str() {
    assert_eq!(DependencyKind::from("required"), DependencyKind::Required);
    assert_eq!(DependencyKind::from("optional"), DependencyKind::Optional);
    assert_eq!(DependencyKind::from("lifecycle"), DependencyKind::Lifecycle);
    assert_eq!(DependencyKind::from("unknown"), DependencyKind::Required); // default
}

#[test]
fn test_registry_programmatic_add() {
    let mut registry = EntityDependencyRegistry::new();

    registry.add_dependency(EntityDep {
        from_type: "test".to_string(),
        from_subtype: Some("A".to_string()),
        to_type: "test".to_string(),
        to_subtype: Some("B".to_string()),
        via_arg: Some("b-ref".to_string()),
        kind: DependencyKind::Required,
        condition_expr: None,
        priority: 100,
    });

    let deps = registry.dependencies_of("test", Some("A"));
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].to_subtype, Some("B".to_string()));

    let dependents = registry.dependents_of("test", Some("B"));
    assert_eq!(dependents.len(), 1);
    assert_eq!(dependents[0].from_subtype, Some("A".to_string()));
}

#[tokio::test]
async fn test_unified_dag_matches_legacy_behavior() {
    // This test verifies that the unified entity_type_dependencies produces
    // the same dependency graph as the legacy resource_dependencies table
    let pool = get_test_pool().await;
    let registry = EntityDependencyRegistry::load(&pool)
        .await
        .expect("Failed to load registry");

    // Load legacy dependencies for comparison
    let legacy_deps: Vec<(String, String, String)> = sqlx::query_as(
        r#"SELECT rt1.resource_code as from_code, rt2.resource_code as to_code, rd.inject_arg
           FROM "ob-poc".resource_dependencies rd
           JOIN "ob-poc".service_resource_types rt1 ON rt1.resource_id = rd.resource_type_id
           JOIN "ob-poc".service_resource_types rt2 ON rt2.resource_id = rd.depends_on_type_id
           WHERE rd.is_active = true"#,
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to load legacy deps");

    // For each legacy dependency, verify it exists in unified registry
    // Note: The unified table uses "resource_instance" as the type
    for (from_code, to_code, inject_arg) in &legacy_deps {
        let deps = registry.dependencies_of("resource_instance", Some(from_code));
        let found = deps.iter().any(|d| {
            d.to_subtype.as_deref() == Some(to_code.as_str())
                && d.via_arg.as_deref() == Some(inject_arg.as_str())
        });

        assert!(
            found,
            "Legacy dependency {} -> {} (via {}) should exist in unified registry",
            from_code, to_code, inject_arg
        );
    }
}
