//! Schema module tests
//!
//! Tests for the schema infrastructure including:
//! - Type definitions
//! - Registry loading
//! - Tokenization
//! - Parsing
//! - Canonicalization
//! - Round-trip verification

#[cfg(test)]
mod type_tests {
    use crate::mcp::schema::types::*;

    #[test]
    fn test_arg_shape_type_names() {
        assert_eq!(ArgShape::Str.type_name(), "string");
        assert_eq!(ArgShape::Int.type_name(), "integer");
        assert_eq!(ArgShape::Bool.type_name(), "boolean");
        assert_eq!(ArgShape::Uuid.type_name(), "uuid");
        assert_eq!(ArgShape::Enum { values: vec![] }.type_name(), "enum");
        assert_eq!(
            ArgShape::EntityRef {
                allowed_kinds: vec![]
            }
            .type_name(),
            "entity"
        );
    }

    #[test]
    fn test_verb_content_to_spec() {
        let content = VerbContent {
            description: "Test verb".to_string(),
            invocation_phrases: vec!["test".to_string(), "multi word phrase".to_string()],
            behavior: "plugin".to_string(),
            metadata: VerbMetadata {
                tier: "intent".to_string(),
                ..Default::default()
            },
            args: vec![ArgContent {
                name: "id".to_string(),
                arg_type: "uuid".to_string(),
                required: true,
                description: "The ID".to_string(),
                default: None,
                valid_values: None,
                maps_to: None,
                lookup: None,
            }],
            returns: None,
        };

        let spec = content.to_spec("test", "create");

        assert_eq!(spec.name, "test.create");
        assert_eq!(spec.domain, "test");
        assert!(spec.aliases.contains(&"test".to_string())); // Single-word phrase
        assert!(!spec.aliases.contains(&"multi word phrase".to_string())); // Multi-word excluded
        assert_eq!(spec.args.required.len(), 1);
        assert_eq!(spec.args.required[0].name, "id");
    }
}

#[cfg(test)]
mod registry_tests {
    use crate::mcp::schema::registry::*;
    use crate::mcp::schema::types::*;
    use std::collections::HashMap;

    fn create_test_registry() -> VerbRegistry {
        let mut registry = VerbRegistry::new();

        // View verbs
        registry.register(VerbSpec {
            name: "view.drill".to_string(),
            domain: "view".to_string(),
            action: "drill".to_string(),
            aliases: vec![
                "drill".to_string(),
                "dive".to_string(),
                "expand".to_string(),
            ],
            args: ArgSchema::default(),
            positional_sugar: vec![],
            keyword_aliases: HashMap::new(),
            doc: "Drill into entity".to_string(),
            tier: "intent".to_string(),
            tags: vec!["navigation".to_string()],
            ..Default::default()
        });

        registry.register(VerbSpec {
            name: "view.surface".to_string(),
            domain: "view".to_string(),
            action: "surface".to_string(),
            aliases: vec!["surface".to_string(), "back".to_string(), "up".to_string()],
            args: ArgSchema::default(),
            positional_sugar: vec![],
            keyword_aliases: HashMap::new(),
            doc: "Surface up".to_string(),
            tier: "intent".to_string(),
            tags: vec!["navigation".to_string()],
            ..Default::default()
        });

        // CBU verbs (to test collisions)
        registry.register(VerbSpec {
            name: "cbu.create".to_string(),
            domain: "cbu".to_string(),
            action: "create".to_string(),
            aliases: vec!["create".to_string(), "add".to_string()],
            args: ArgSchema::default(),
            positional_sugar: vec![],
            keyword_aliases: HashMap::new(),
            doc: "Create CBU".to_string(),
            tier: "intent".to_string(),
            tags: vec![],
            ..Default::default()
        });

        registry.register(VerbSpec {
            name: "entity.create".to_string(),
            domain: "entity".to_string(),
            action: "create".to_string(),
            aliases: vec!["create".to_string(), "add".to_string()],
            args: ArgSchema::default(),
            positional_sugar: vec![],
            keyword_aliases: HashMap::new(),
            doc: "Create entity".to_string(),
            tier: "intent".to_string(),
            tags: vec![],
            ..Default::default()
        });

        registry
    }

    #[test]
    fn test_exact_fqn_resolution() {
        let registry = create_test_registry();

        match registry.resolve_head("view.drill") {
            HeadResolution::Exact(spec) => {
                assert_eq!(spec.name, "view.drill");
            }
            other => panic!("Expected Exact, got {:?}", other),
        }
    }

    #[test]
    fn test_unique_alias_resolution() {
        let registry = create_test_registry();

        match registry.resolve_head("dive") {
            HeadResolution::Alias { alias, spec } => {
                assert_eq!(alias, "dive");
                assert_eq!(spec.name, "view.drill");
            }
            other => panic!("Expected Alias, got {:?}", other),
        }
    }

    #[test]
    fn test_ambiguous_alias() {
        let registry = create_test_registry();

        // "create" is used by both cbu.create and entity.create
        match registry.resolve_head("create") {
            HeadResolution::Ambiguous { alias, candidates } => {
                assert_eq!(alias, "create");
                assert!(candidates.len() >= 2);
                let names: Vec<_> = candidates.iter().map(|c| c.name.as_str()).collect();
                assert!(names.contains(&"cbu.create"));
                assert!(names.contains(&"entity.create"));
            }
            other => panic!("Expected Ambiguous, got {:?}", other),
        }
    }

    #[test]
    fn test_not_found() {
        let registry = create_test_registry();

        match registry.resolve_head("nonexistent") {
            HeadResolution::NotFound { input, .. } => {
                assert_eq!(input, "nonexistent");
            }
            other => panic!("Expected NotFound, got {:?}", other),
        }
    }

    #[test]
    fn test_alias_collisions_report() {
        let registry = create_test_registry();
        let collisions = registry.alias_collisions();

        // Should detect "create" and "add" as collisions
        assert!(!collisions.is_empty());

        let create_collision = collisions
            .iter()
            .find(|(alias, _)| alias.as_str() == "create");
        assert!(create_collision.is_some());
        assert!(create_collision.unwrap().1.len() >= 2);
    }

    #[test]
    fn test_domain_verbs() {
        let registry = create_test_registry();

        let view_verbs = registry.domain_verbs("view");
        assert_eq!(view_verbs.len(), 2);

        let cbu_verbs = registry.domain_verbs("cbu");
        assert_eq!(cbu_verbs.len(), 1);
    }
}

#[cfg(test)]
mod integration_tests {
    use crate::mcp::schema::*;
    use std::collections::HashMap;

    fn test_registry() -> VerbRegistry {
        let mut registry = VerbRegistry::new();

        registry.register(VerbSpec {
            name: "session.load-galaxy".to_string(),
            domain: "session".to_string(),
            action: "load-galaxy".to_string(),
            aliases: vec!["load".to_string(), "open".to_string()],
            args: ArgSchema {
                style: "keyworded".to_string(),
                required: vec![ArgDef {
                    name: "client".to_string(),
                    shape: ArgShape::EntityRef {
                        allowed_kinds: vec!["client".to_string()],
                    },
                    default: None,
                    doc: "Client to load".to_string(),
                    maps_to: None,
                    lookup: None,
                }],
                optional: vec![ArgDef {
                    name: "jurisdiction".to_string(),
                    shape: ArgShape::Str,
                    default: None,
                    doc: "Filter by jurisdiction".to_string(),
                    maps_to: None,
                    lookup: None,
                }],
            },
            positional_sugar: vec!["client".to_string()],
            keyword_aliases: HashMap::from([("j".to_string(), "jurisdiction".to_string())]),
            doc: "Load client galaxy".to_string(),
            tier: "intent".to_string(),
            tags: vec!["session".to_string()],
            ..Default::default()
        });

        registry.register(VerbSpec {
            name: "view.drill".to_string(),
            domain: "view".to_string(),
            action: "drill".to_string(),
            aliases: vec!["drill".to_string()],
            args: ArgSchema {
                style: "keyworded".to_string(),
                required: vec![ArgDef {
                    name: "entity".to_string(),
                    shape: ArgShape::EntityRef {
                        allowed_kinds: vec![],
                    },
                    default: None,
                    doc: "Entity".to_string(),
                    maps_to: None,
                    lookup: None,
                }],
                optional: vec![],
            },
            positional_sugar: vec!["entity".to_string()],
            keyword_aliases: HashMap::new(),
            doc: "Drill".to_string(),
            tier: "intent".to_string(),
            tags: vec![],
            ..Default::default()
        });

        registry
    }

    #[test]
    fn test_full_pipeline_keyword_form() {
        let registry = test_registry();
        let input = "(session.load-galaxy :client <Allianz>)";

        let parsed = parser::parse(input, &registry).unwrap();
        assert_eq!(parsed.verb_fqn, "session.load-galaxy");

        let canonical = canonicalize(&parsed, &registry).unwrap();
        assert_eq!(canonical.verb, "session.load-galaxy");
        assert_eq!(canonical.unresolved_entities.len(), 1);
        assert_eq!(canonical.unresolved_entities[0].name, "Allianz");
    }

    #[test]
    fn test_full_pipeline_positional_sugar() {
        let registry = test_registry();
        let input = "(drill \"some-uuid\")";

        let parsed = parser::parse(input, &registry).unwrap();
        assert_eq!(parsed.verb_fqn, "view.drill");

        // Should have feedback about positional sugar
        assert!(parsed
            .feedback
            .iter()
            .any(|f| matches!(f.kind, parser::FeedbackKind::PositionalSugar)));

        let canonical = canonicalize(&parsed, &registry).unwrap();
        assert!(canonical.args.iter().any(|a| a.name == "entity"));
    }

    #[test]
    fn test_round_trip() {
        let registry = test_registry();
        let input = "(session.load-galaxy :client \"uuid-123\" :jurisdiction \"LU\")";

        // Parse
        let parsed = parser::parse(input, &registry).unwrap();

        // Canonicalize
        let canonical = canonicalize(&parsed, &registry).unwrap();

        // Convert to s-expr
        let sexpr = canonical.to_sexpr();

        // Parse again
        let reparsed = parser::parse(&sexpr, &registry).unwrap();

        // Canonicalize again
        let recanonical = canonicalize(&reparsed, &registry).unwrap();

        // Verify
        assert_eq!(canonical.verb, recanonical.verb);
        assert_eq!(canonical.args.len(), recanonical.args.len());

        for orig_arg in &canonical.args {
            let reparsed_arg = recanonical.args.iter().find(|a| a.name == orig_arg.name);
            assert!(reparsed_arg.is_some(), "Missing arg: {}", orig_arg.name);
        }
    }
}
