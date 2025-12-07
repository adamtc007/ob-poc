//! Integration tests for Intent → DSL Assembly Pipeline
//!
//! These tests verify the deterministic assembly of DSL from structured intents.
//! The flow is:
//!   1. Create DslIntent structures (simulating Claude extraction)
//!   2. Use DslAssembler with a resolver to generate DSL
//!   3. Validate the generated DSL with the parser and linter
//!
//! Run with: cargo test --features database --test intent_assembly_integration

#![cfg(feature = "database")]

use std::collections::HashMap;

use ob_poc::dsl_v2::{
    assembler::{ArgResolver, DslAssembler},
    intent::{ArgIntent, DslIntent, DslIntentBatch, ResolvedArg},
    needs_quoting, parse_program,
    validation::ValidationContext,
    CsgLinter,
};

/// Mock resolver that simulates EntityGateway lookups
struct TestResolver {
    entities: HashMap<(String, String), ResolvedArg>,
    ref_data: HashMap<(String, String), ResolvedArg>,
}

impl TestResolver {
    fn new() -> Self {
        let mut resolver = Self {
            entities: HashMap::new(),
            ref_data: HashMap::new(),
        };

        // Add some test entities
        resolver.add_entity(
            "apex capital",
            "cbu",
            "11111111-1111-1111-1111-111111111111",
            "Apex Capital",
        );
        resolver.add_entity(
            "john smith",
            "person",
            "22222222-2222-2222-2222-222222222222",
            "John Smith",
        );
        resolver.add_entity(
            "john smith",
            "entity",
            "22222222-2222-2222-2222-222222222222",
            "John Smith",
        );
        resolver.add_entity(
            "acme holdings",
            "entity",
            "33333333-3333-3333-3333-333333333333",
            "Acme Holdings Ltd",
        );

        // Add reference data
        resolver.add_ref("director", "role", "DIRECTOR");
        resolver.add_ref("beneficial owner", "role", "BENEFICIAL_OWNER");
        resolver.add_ref("luxembourg", "jurisdiction", "LU");
        resolver.add_ref("lu", "jurisdiction", "LU");
        resolver.add_ref("fund", "client_type", "FUND");
        resolver.add_ref("corporate", "client_type", "CORPORATE");
        resolver.add_ref("new client", "case_type", "NEW_CLIENT");
        resolver.add_ref("sanctions", "screening_type", "SANCTIONS");
        resolver.add_ref("pep", "screening_type", "PEP");

        resolver
    }

    fn add_entity(&mut self, search: &str, entity_type: &str, id: &str, display: &str) {
        self.entities.insert(
            (search.to_lowercase(), entity_type.to_lowercase()),
            ResolvedArg {
                value: id.to_string(),
                is_symbol_ref: false,
                needs_quotes: needs_quoting(id),
                display: Some(display.to_string()),
            },
        );
    }

    fn add_ref(&mut self, search: &str, ref_type: &str, code: &str) {
        self.ref_data.insert(
            (search.to_lowercase(), ref_type.to_lowercase()),
            ResolvedArg {
                value: code.to_string(),
                is_symbol_ref: false,
                needs_quotes: needs_quoting(code),
                display: Some(code.to_string()),
            },
        );
    }
}

impl ArgResolver for TestResolver {
    fn resolve_entity(&self, search: &str, entity_type: &str) -> Result<ResolvedArg, String> {
        self.entities
            .get(&(search.to_lowercase(), entity_type.to_lowercase()))
            .cloned()
            .ok_or_else(|| format!("No {} found for '{}'", entity_type, search))
    }

    fn resolve_ref_data(&self, search: &str, ref_type: &str) -> Result<ResolvedArg, String> {
        self.ref_data
            .get(&(search.to_lowercase(), ref_type.to_lowercase()))
            .cloned()
            .ok_or_else(|| format!("No {} found for '{}'", ref_type, search))
    }
}

/// Debug test for parsing
#[test]
fn test_parse_debug() {
    // Test parsing a simple DSL statement directly - all string values must be quoted
    let dsl1 = r#"(cbu.ensure :name "Test" :jurisdiction "LU" :as @fund)"#;
    let result1 = parse_program(dsl1);
    println!("DSL1: {}", dsl1);
    println!("Result1: {:?}", result1);
    assert!(
        result1.is_ok(),
        "Simple DSL should parse: {:?}",
        result1.err()
    );

    // Test with quoted role code
    let dsl2 = r#"(cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")"#;
    let result2 = parse_program(dsl2);
    println!("DSL2: {}", dsl2);
    println!("Result2: {:?}", result2);
    assert!(
        result2.is_ok(),
        "Role DSL should parse: {:?}",
        result2.err()
    );
}

/// Test assembling a simple CBU creation intent
#[test]
fn test_assemble_cbu_creation() {
    let resolver = TestResolver::new();
    let assembler = DslAssembler::new();

    let intent = DslIntent {
        verb: Some("cbu.ensure".to_string()),
        action: "create".to_string(),
        domain: "cbu".to_string(),
        args: HashMap::from([
            (
                "name".to_string(),
                ArgIntent::Literal {
                    value: serde_json::json!("Pacific Growth Fund"),
                },
            ),
            (
                "jurisdiction".to_string(),
                ArgIntent::RefDataLookup {
                    search_text: "Luxembourg".to_string(),
                    ref_type: "jurisdiction".to_string(),
                },
            ),
            (
                "client-type".to_string(),
                ArgIntent::RefDataLookup {
                    search_text: "fund".to_string(),
                    ref_type: "client_type".to_string(),
                },
            ),
        ]),
        bind_as: Some("fund".to_string()),
        source_text: Some("Create a fund called Pacific Growth in Luxembourg".to_string()),
    };

    let result = assembler.assemble_one(&intent, &resolver).unwrap();

    // Verify DSL structure - all string values should be quoted
    assert!(result.dsl.starts_with("(cbu.ensure"));
    assert!(result.dsl.contains(":name \"Pacific Growth Fund\""));
    assert!(result.dsl.contains(":jurisdiction \"LU\""));
    assert!(result.dsl.contains(":client-type \"FUND\""));
    assert!(result.dsl.contains(":as @fund"));

    // Verify it parses correctly
    let parsed = parse_program(&result.dsl);
    assert!(parsed.is_ok(), "Generated DSL should parse: {}", result.dsl);
}

/// Test assembling an assign-role intent with symbol references
#[test]
fn test_assemble_assign_role_with_symbols() {
    let resolver = TestResolver::new();
    let assembler = DslAssembler::new();

    let intent = DslIntent {
        verb: Some("cbu.assign-role".to_string()),
        action: "assign".to_string(),
        domain: "cbu".to_string(),
        args: HashMap::from([
            (
                "cbu-id".to_string(),
                ArgIntent::SymbolRef {
                    symbol: "fund".to_string(),
                },
            ),
            (
                "entity-id".to_string(),
                ArgIntent::SymbolRef {
                    symbol: "john".to_string(),
                },
            ),
            (
                "role".to_string(),
                ArgIntent::RefDataLookup {
                    search_text: "director".to_string(),
                    ref_type: "role".to_string(),
                },
            ),
        ]),
        bind_as: None,
        source_text: Some("Assign John as director".to_string()),
    };

    let result = assembler.assemble_one(&intent, &resolver).unwrap();

    assert!(result.dsl.contains(":cbu-id @fund"));
    assert!(result.dsl.contains(":entity-id @john"));
    assert!(result.dsl.contains(":role \"DIRECTOR\""));

    let parsed = parse_program(&result.dsl);
    assert!(parsed.is_ok(), "Generated DSL should parse: {}", result.dsl);
}

/// Test assembling with entity lookups (not symbol refs)
#[test]
fn test_assemble_with_entity_lookup() {
    let resolver = TestResolver::new();
    let assembler = DslAssembler::new();

    let intent = DslIntent {
        verb: Some("cbu.assign-role".to_string()),
        action: "assign".to_string(),
        domain: "cbu".to_string(),
        args: HashMap::from([
            (
                "cbu-id".to_string(),
                ArgIntent::EntityLookup {
                    search_text: "Apex Capital".to_string(),
                    entity_type: Some("cbu".to_string()),
                },
            ),
            (
                "entity-id".to_string(),
                ArgIntent::EntityLookup {
                    search_text: "John Smith".to_string(),
                    entity_type: Some("person".to_string()),
                },
            ),
            (
                "role".to_string(),
                ArgIntent::RefDataLookup {
                    search_text: "beneficial owner".to_string(),
                    ref_type: "role".to_string(),
                },
            ),
        ]),
        bind_as: None,
        source_text: Some("Add John Smith as beneficial owner of Apex Capital".to_string()),
    };

    let result = assembler.assemble_one(&intent, &resolver).unwrap();

    // Should have resolved UUIDs (quoted), not search text
    assert!(result
        .dsl
        .contains("\"11111111-1111-1111-1111-111111111111\"")); // CBU UUID quoted
    assert!(result
        .dsl
        .contains("\"22222222-2222-2222-2222-222222222222\"")); // Person UUID quoted
    assert!(result.dsl.contains(":role \"BENEFICIAL_OWNER\""));

    let parsed = parse_program(&result.dsl);
    assert!(parsed.is_ok(), "Generated DSL should parse: {}", result.dsl);
}

/// Test assembling a batch of intents
#[test]
fn test_assemble_batch() {
    let resolver = TestResolver::new();
    let mut assembler = DslAssembler::new();

    let batch = DslIntentBatch {
        actions: vec![
            DslIntent {
                verb: Some("cbu.ensure".to_string()),
                action: "create".to_string(),
                domain: "cbu".to_string(),
                args: HashMap::from([
                    (
                        "name".to_string(),
                        ArgIntent::Literal {
                            value: serde_json::json!("Test Fund"),
                        },
                    ),
                    (
                        "jurisdiction".to_string(),
                        ArgIntent::RefDataLookup {
                            search_text: "LU".to_string(),
                            ref_type: "jurisdiction".to_string(),
                        },
                    ),
                ]),
                bind_as: Some("fund".to_string()),
                source_text: None,
            },
            DslIntent {
                verb: Some("entity.create-proper-person".to_string()),
                action: "create".to_string(),
                domain: "entity".to_string(),
                args: HashMap::from([
                    (
                        "first-name".to_string(),
                        ArgIntent::Literal {
                            value: serde_json::json!("Jane"),
                        },
                    ),
                    (
                        "last-name".to_string(),
                        ArgIntent::Literal {
                            value: serde_json::json!("Doe"),
                        },
                    ),
                ]),
                bind_as: Some("jane".to_string()),
                source_text: None,
            },
            DslIntent {
                verb: Some("cbu.assign-role".to_string()),
                action: "assign".to_string(),
                domain: "cbu".to_string(),
                args: HashMap::from([
                    (
                        "cbu-id".to_string(),
                        ArgIntent::SymbolRef {
                            symbol: "fund".to_string(),
                        },
                    ),
                    (
                        "entity-id".to_string(),
                        ArgIntent::SymbolRef {
                            symbol: "jane".to_string(),
                        },
                    ),
                    (
                        "role".to_string(),
                        ArgIntent::RefDataLookup {
                            search_text: "director".to_string(),
                            ref_type: "role".to_string(),
                        },
                    ),
                ]),
                bind_as: None,
                source_text: None,
            },
        ],
        context: Some("Create fund with director".to_string()),
        original_request: "Create a fund in Luxembourg and add Jane Doe as director".to_string(),
    };

    let result = assembler.assemble_batch(&batch, &resolver).unwrap();

    // Should have 3 statements
    let lines: Vec<&str> = result.lines().filter(|l| l.starts_with('(')).collect();
    assert_eq!(lines.len(), 3);

    // First should be cbu.ensure with @fund binding
    assert!(lines[0].contains("cbu.ensure"));
    assert!(lines[0].contains(":as @fund"));

    // Second should be entity.create-proper-person with @jane binding
    assert!(lines[1].contains("entity.create-proper-person"));
    assert!(lines[1].contains(":as @jane"));

    // Third should reference @fund and @jane
    assert!(lines[2].contains("cbu.assign-role"));
    assert!(lines[2].contains(":cbu-id @fund"));
    assert!(lines[2].contains(":entity-id @jane"));

    // Full batch should parse
    let parsed = parse_program(&result);
    assert!(
        parsed.is_ok(),
        "Generated DSL batch should parse:\n{}",
        result
    );
}

/// Test that missing required args produce clear errors
#[test]
fn test_missing_required_arg_error() {
    let resolver = TestResolver::new();
    let assembler = DslAssembler::new();

    let intent = DslIntent {
        verb: Some("cbu.ensure".to_string()),
        action: "create".to_string(),
        domain: "cbu".to_string(),
        args: HashMap::new(), // Missing required :name
        bind_as: None,
        source_text: None,
    };

    let result = assembler.assemble_one(&intent, &resolver);

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(
        error.to_string().contains("name"),
        "Error should mention missing 'name' arg"
    );
}

/// Test that lookup failures produce clear errors
#[test]
fn test_lookup_failure_error() {
    let resolver = TestResolver::new();
    let assembler = DslAssembler::new();

    let intent = DslIntent {
        verb: Some("cbu.assign-role".to_string()),
        action: "assign".to_string(),
        domain: "cbu".to_string(),
        args: HashMap::from([
            (
                "cbu-id".to_string(),
                ArgIntent::EntityLookup {
                    search_text: "NonExistent Corp".to_string(),
                    entity_type: Some("cbu".to_string()),
                },
            ),
            (
                "entity-id".to_string(),
                ArgIntent::SymbolRef {
                    symbol: "john".to_string(),
                },
            ),
            (
                "role".to_string(),
                ArgIntent::RefDataLookup {
                    search_text: "director".to_string(),
                    ref_type: "role".to_string(),
                },
            ),
        ]),
        bind_as: None,
        source_text: None,
    };

    let result = assembler.assemble_one(&intent, &resolver);

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("NonExistent Corp") || error.to_string().contains("cbu-id"));
}

/// Test CSG linting of assembled DSL (requires database)
#[tokio::test]
#[ignore] // Requires database
async fn test_assembled_dsl_passes_linting() {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/data_designer".to_string());
    let pool = sqlx::PgPool::connect(&url)
        .await
        .expect("Failed to connect");

    let resolver = TestResolver::new();
    let mut assembler = DslAssembler::new();

    let batch = DslIntentBatch {
        actions: vec![DslIntent {
            verb: Some("cbu.ensure".to_string()),
            action: "create".to_string(),
            domain: "cbu".to_string(),
            args: HashMap::from([
                (
                    "name".to_string(),
                    ArgIntent::Literal {
                        value: serde_json::json!("Test Fund"),
                    },
                ),
                (
                    "jurisdiction".to_string(),
                    ArgIntent::Literal {
                        value: serde_json::json!("LU"),
                    },
                ),
                (
                    "client-type".to_string(),
                    ArgIntent::Literal {
                        value: serde_json::json!("FUND"),
                    },
                ),
            ]),
            bind_as: Some("fund".to_string()),
            source_text: None,
        }],
        context: None,
        original_request: "Create a test fund".to_string(),
    };

    let dsl = assembler.assemble_batch(&batch, &resolver).unwrap();

    // Parse
    let ast = parse_program(&dsl).expect("Should parse");

    // Lint
    let mut linter = CsgLinter::new(pool);
    linter.initialize().await.expect("Failed to init linter");

    let result = linter.lint(ast, &ValidationContext::default(), &dsl).await;

    assert!(
        !result.has_errors(),
        "Assembled DSL should pass linting. Errors: {:?}",
        result.diagnostics
    );
}
