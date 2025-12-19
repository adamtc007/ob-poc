//! AST Enrichment Pass
//!
//! Transforms raw AST (all strings are Literal::String) into enriched AST
//! (strings with lookup config become EntityRef).
//!
//! ## Pipeline Position
//!
//! ```text
//! Source → Parser v2 → Raw AST (Literals only)
//!                          ↓
//!               Enrichment Pass (this module)
//!                          ↓
//!              Enriched AST (String → EntityRef where lookup config exists)
//!                          ↓
//!               Validator Tree-Walk
//!                          ↓
//!              Resolved AST (EntityRef.resolved_key populated)
//! ```
//!
//! ## What This Pass Does
//!
//! For each verb call argument:
//! 1. Look up the verb definition in YAML
//! 2. If the arg has a `lookup` config, convert `Literal::String` → `EntityRef`
//! 3. Propagate span information for error reporting
//!
//! ## Why This Separation?
//!
//! The parser is pure syntax - it doesn't know about verb definitions.
//! The enrichment pass applies semantic knowledge from YAML to produce
//! a self-describing AST where tree-walkers can immediately identify
//! "breaks" (unresolved EntityRefs) without consulting YAML again.

use super::ast::*;
use super::config::types::LookupConfig;
use super::runtime_registry::{RuntimeArg, RuntimeVerbRegistry};

/// Errors that can occur during enrichment
#[derive(Debug, Clone)]
pub struct EnrichmentError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for EnrichmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Result of enrichment - enriched AST plus any errors/warnings
#[derive(Debug)]
pub struct EnrichmentResult {
    pub program: Program,
    pub errors: Vec<EnrichmentError>,
}

/// Enrich a raw AST using verb definitions from the registry
///
/// This transforms `Literal::String` nodes into `EntityRef` nodes
/// for arguments that have lookup configuration in their verb definition.
pub fn enrich_program(program: Program, registry: &RuntimeVerbRegistry) -> EnrichmentResult {
    let mut enricher = Enricher {
        registry,
        errors: Vec::new(),
    };

    let enriched = enricher.enrich_program(program);

    EnrichmentResult {
        program: enriched,
        errors: enricher.errors,
    }
}

struct Enricher<'a> {
    registry: &'a RuntimeVerbRegistry,
    errors: Vec<EnrichmentError>,
}

impl<'a> Enricher<'a> {
    fn enrich_program(&mut self, program: Program) -> Program {
        Program {
            statements: program
                .statements
                .into_iter()
                .map(|s| self.enrich_statement(s))
                .collect(),
        }
    }

    fn enrich_statement(&mut self, stmt: Statement) -> Statement {
        match stmt {
            Statement::VerbCall(vc) => Statement::VerbCall(self.enrich_verb_call(vc)),
            Statement::Comment(c) => Statement::Comment(c),
        }
    }

    fn enrich_verb_call(&mut self, vc: VerbCall) -> VerbCall {
        let full_name = vc.full_name();

        // Look up verb definition
        let verb_def = self.registry.get_by_name(&full_name);

        let enriched_args = vc
            .arguments
            .into_iter()
            .map(|arg| self.enrich_argument(arg, verb_def.map(|v| &v.args)))
            .collect();

        VerbCall {
            domain: vc.domain,
            verb: vc.verb,
            arguments: enriched_args,
            binding: vc.binding,
            span: vc.span,
        }
    }

    fn enrich_argument(&mut self, arg: Argument, verb_args: Option<&Vec<RuntimeArg>>) -> Argument {
        // Find the arg definition for this key
        let arg_def = verb_args.and_then(|args| args.iter().find(|a| a.name == arg.key));

        // Get lookup config if present
        let lookup_config = arg_def.and_then(|a| a.lookup.as_ref());

        let enriched_value = self.enrich_node(arg.value, lookup_config, arg.span);

        Argument {
            key: arg.key,
            value: enriched_value,
            span: arg.span,
        }
    }

    fn enrich_node(
        &mut self,
        node: AstNode,
        lookup_config: Option<&LookupConfig>,
        arg_span: Span,
    ) -> AstNode {
        match node {
            // String literal - potentially convert to EntityRef
            AstNode::Literal(Literal::String(s)) => {
                if let Some(config) = lookup_config {
                    // Convert to EntityRef
                    let entity_type = config
                        .entity_type
                        .clone()
                        .unwrap_or_else(|| config.table.clone());

                    AstNode::EntityRef {
                        entity_type,
                        search_column: config.search_key.primary_column().to_string(),
                        value: s,
                        resolved_key: None,
                        span: arg_span, // Preserve span for LSP diagnostics
                    }
                } else {
                    // Keep as string literal
                    AstNode::Literal(Literal::String(s))
                }
            }

            // UUID literal - if this arg has lookup config, it's already resolved
            AstNode::Literal(Literal::Uuid(uuid)) => {
                if let Some(config) = lookup_config {
                    let entity_type = config
                        .entity_type
                        .clone()
                        .unwrap_or_else(|| config.table.clone());

                    // UUID is already the resolved key
                    AstNode::EntityRef {
                        entity_type,
                        search_column: config.search_key.primary_column().to_string(),
                        value: uuid.to_string(), // Use UUID as display value
                        resolved_key: Some(uuid.to_string()),
                        span: arg_span, // Preserve span for LSP diagnostics
                    }
                } else {
                    AstNode::Literal(Literal::Uuid(uuid))
                }
            }

            // Symbol refs pass through - resolved at execution time
            AstNode::SymbolRef { name, span } => AstNode::SymbolRef { name, span },

            // Entity refs pass through (already enriched)
            AstNode::EntityRef { .. } => node,

            // Other literals pass through
            AstNode::Literal(lit) => AstNode::Literal(lit),

            // Lists - enrich each item (with same lookup config for homogeneous lists)
            AstNode::List { items, span } => AstNode::List {
                items: items
                    .into_iter()
                    .map(|item| self.enrich_node(item, lookup_config, span))
                    .collect(),
                span,
            },

            // Maps - enrich each value (no lookup config for map values)
            AstNode::Map { entries, span } => AstNode::Map {
                entries: entries
                    .into_iter()
                    .map(|(k, v)| (k, self.enrich_node(v, None, span)))
                    .collect(),
                span,
            },

            // Nested verb calls - recursive enrichment
            AstNode::Nested(vc) => AstNode::Nested(Box::new(self.enrich_verb_call(*vc))),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::super::config::types::{
        ArgConfig, ArgType, CrudConfig, CrudOperation, DomainConfig, LookupConfig, ResolutionMode,
        SearchKeyConfig, VerbBehavior, VerbConfig, VerbsConfig,
    };
    use super::*;
    use std::collections::HashMap;

    // Helper to create a minimal test registry
    fn test_registry() -> RuntimeVerbRegistry {
        let mut domains = HashMap::new();

        let mut cbu_verbs = HashMap::new();

        // cbu.ensure verb
        cbu_verbs.insert(
            "ensure".to_string(),
            VerbConfig {
                description: "Create or update CBU".to_string(),
                behavior: VerbBehavior::Crud,
                produces: None,
                consumes: vec![],
                lifecycle: None,
                graph_query: None,
                crud: Some(CrudConfig {
                    operation: CrudOperation::Upsert,
                    table: Some("cbus".to_string()),
                    schema: None,
                    key: None,
                    returning: None,
                    conflict_keys: None,
                    junction: None,
                    from_col: None,
                    to_col: None,
                    role_table: None,
                    role_col: None,
                    fk_col: None,
                    filter_col: None,
                    primary_table: None,
                    join_table: None,
                    join_col: None,
                    base_table: None,
                    extension_table: None,
                    extension_table_column: None,
                    type_id_column: None,
                    type_code: None,
                    order_by: None,
                    set_values: None,
                }),
                handler: None,
                args: vec![
                    ArgConfig {
                        name: "name".to_string(),
                        arg_type: ArgType::String,
                        required: true,
                        maps_to: None,
                        lookup: None,
                        valid_values: None,
                        default: None,
                        description: None,
                        validation: None,
                        fuzzy_check: None,
                    },
                    ArgConfig {
                        name: "jurisdiction".to_string(),
                        arg_type: ArgType::Lookup,
                        required: false,
                        maps_to: None,
                        lookup: Some(LookupConfig {
                            table: "jurisdictions".to_string(),
                            schema: None,
                            entity_type: Some("jurisdiction".to_string()),
                            search_key: SearchKeyConfig::Simple("code".to_string()),
                            primary_key: "code".to_string(),
                            resolution_mode: None, // reference data - autocomplete
                        }),
                        valid_values: None,
                        default: None,
                        description: None,
                        validation: None,
                        fuzzy_check: None,
                    },
                    ArgConfig {
                        name: "client-type".to_string(),
                        arg_type: ArgType::String,
                        required: false,
                        maps_to: None,
                        lookup: None,
                        valid_values: None,
                        default: None,
                        description: None,
                        validation: None,
                        fuzzy_check: None,
                    },
                ],
                returns: None,
            },
        );

        // cbu.assign-role verb
        cbu_verbs.insert(
            "assign-role".to_string(),
            VerbConfig {
                description: "Assign role to entity".to_string(),
                behavior: VerbBehavior::Crud,
                produces: None,
                consumes: vec![],
                lifecycle: None,
                graph_query: None,
                crud: Some(CrudConfig {
                    operation: CrudOperation::RoleLink,
                    table: None,
                    schema: None,
                    key: None,
                    returning: None,
                    conflict_keys: None,
                    junction: None,
                    from_col: None,
                    to_col: None,
                    role_table: None,
                    role_col: None,
                    fk_col: None,
                    filter_col: None,
                    primary_table: None,
                    join_table: None,
                    join_col: None,
                    base_table: None,
                    extension_table: None,
                    extension_table_column: None,
                    type_id_column: None,
                    type_code: None,
                    order_by: None,
                    set_values: None,
                }),
                handler: None,
                args: vec![
                    ArgConfig {
                        name: "cbu-id".to_string(),
                        arg_type: ArgType::Uuid,
                        required: true,
                        maps_to: None,
                        lookup: None,
                        valid_values: None,
                        default: None,
                        description: None,
                        validation: None,
                        fuzzy_check: None,
                    },
                    ArgConfig {
                        name: "entity-id".to_string(),
                        arg_type: ArgType::Lookup,
                        required: true,
                        maps_to: None,
                        lookup: Some(LookupConfig {
                            table: "entities".to_string(),
                            schema: None,
                            entity_type: Some("entity".to_string()),
                            search_key: SearchKeyConfig::Simple("name".to_string()),
                            primary_key: "entity_id".to_string(),
                            resolution_mode: Some(ResolutionMode::Entity), // entity - search modal
                        }),
                        valid_values: None,
                        default: None,
                        description: None,
                        validation: None,
                        fuzzy_check: None,
                    },
                    ArgConfig {
                        name: "role".to_string(),
                        arg_type: ArgType::Lookup,
                        required: true,
                        maps_to: None,
                        lookup: Some(LookupConfig {
                            table: "roles".to_string(),
                            schema: None,
                            entity_type: Some("role".to_string()),
                            search_key: SearchKeyConfig::Simple("name".to_string()),
                            primary_key: "name".to_string(),
                            resolution_mode: None, // reference data - autocomplete
                        }),
                        valid_values: None,
                        default: None,
                        description: None,
                        validation: None,
                        fuzzy_check: None,
                    },
                ],
                returns: None,
            },
        );

        domains.insert(
            "cbu".to_string(),
            DomainConfig {
                description: "CBU operations".to_string(),
                verbs: cbu_verbs,
                dynamic_verbs: vec![],
            },
        );

        let config = VerbsConfig {
            version: "1.0".to_string(),
            domains,
        };

        RuntimeVerbRegistry::from_config(&config)
    }

    #[test]
    fn test_enrich_string_to_entity_ref() {
        let registry = test_registry();

        // Raw AST with string literals
        let raw = Program {
            statements: vec![Statement::VerbCall(VerbCall {
                domain: "cbu".to_string(),
                verb: "ensure".to_string(),
                arguments: vec![
                    Argument {
                        key: "name".to_string(),
                        value: AstNode::Literal(Literal::String("Test Fund".to_string())),
                        span: Span::default(),
                    },
                    Argument {
                        key: "jurisdiction".to_string(),
                        value: AstNode::Literal(Literal::String("LU".to_string())),
                        span: Span::default(),
                    },
                ],
                binding: None,
                span: Span::default(),
            })],
        };

        let result = enrich_program(raw, &registry);
        assert!(result.errors.is_empty());

        // Check that jurisdiction was converted to EntityRef
        if let Statement::VerbCall(vc) = &result.program.statements[0] {
            // name should stay as string (no lookup config)
            let name_arg = vc.get_arg("name").unwrap();
            assert!(name_arg.value.is_literal());
            assert_eq!(name_arg.value.as_string(), Some("Test Fund"));

            // jurisdiction should be EntityRef
            let juris_arg = vc.get_arg("jurisdiction").unwrap();
            assert!(juris_arg.value.is_entity_ref());
            assert!(juris_arg.value.is_unresolved_entity_ref());

            if let AstNode::EntityRef {
                entity_type,
                search_column,
                value,
                resolved_key,
                ..
            } = &juris_arg.value
            {
                assert_eq!(entity_type, "jurisdiction");
                assert_eq!(search_column, "code");
                assert_eq!(value, "LU");
                assert!(resolved_key.is_none());
            } else {
                panic!("Expected EntityRef");
            }
        }
    }

    #[test]
    fn test_enrich_with_symbol_ref() {
        let registry = test_registry();

        // Raw AST with symbol reference
        let raw = Program {
            statements: vec![Statement::VerbCall(VerbCall {
                domain: "cbu".to_string(),
                verb: "assign-role".to_string(),
                arguments: vec![
                    Argument {
                        key: "cbu-id".to_string(),
                        value: AstNode::SymbolRef {
                            name: "fund".to_string(),
                            span: Span::default(),
                        },
                        span: Span::default(),
                    },
                    Argument {
                        key: "entity-id".to_string(),
                        value: AstNode::Literal(Literal::String("John Smith".to_string())),
                        span: Span::default(),
                    },
                    Argument {
                        key: "role".to_string(),
                        value: AstNode::Literal(Literal::String("DIRECTOR".to_string())),
                        span: Span::default(),
                    },
                ],
                binding: None,
                span: Span::default(),
            })],
        };

        let result = enrich_program(raw, &registry);

        if let Statement::VerbCall(vc) = &result.program.statements[0] {
            // cbu-id should stay as SymbolRef
            let cbu_arg = vc.get_arg("cbu-id").unwrap();
            assert!(cbu_arg.value.is_symbol_ref());

            // entity-id should be EntityRef
            let entity_arg = vc.get_arg("entity-id").unwrap();
            assert!(entity_arg.value.is_entity_ref());
            if let AstNode::EntityRef {
                entity_type, value, ..
            } = &entity_arg.value
            {
                assert_eq!(entity_type, "entity");
                assert_eq!(value, "John Smith");
            }

            // role should be EntityRef
            let role_arg = vc.get_arg("role").unwrap();
            assert!(role_arg.value.is_entity_ref());
            if let AstNode::EntityRef {
                entity_type, value, ..
            } = &role_arg.value
            {
                assert_eq!(entity_type, "role");
                assert_eq!(value, "DIRECTOR");
            }
        }
    }

    #[test]
    fn test_enrich_list_with_lookup() {
        let registry = test_registry();

        // Raw AST with list of strings that should be entity refs
        let raw = Program {
            statements: vec![Statement::VerbCall(VerbCall {
                domain: "cbu".to_string(),
                verb: "assign-role".to_string(),
                arguments: vec![
                    Argument {
                        key: "cbu-id".to_string(),
                        value: AstNode::SymbolRef {
                            name: "fund".to_string(),
                            span: Span::default(),
                        },
                        span: Span::default(),
                    },
                    // entity-id as a list (hypothetical - tests list enrichment)
                    Argument {
                        key: "entity-id".to_string(),
                        value: AstNode::List {
                            items: vec![
                                AstNode::Literal(Literal::String("Alice".to_string())),
                                AstNode::Literal(Literal::String("Bob".to_string())),
                            ],
                            span: Span::default(),
                        },
                        span: Span::default(),
                    },
                    Argument {
                        key: "role".to_string(),
                        value: AstNode::Literal(Literal::String("DIRECTOR".to_string())),
                        span: Span::default(),
                    },
                ],
                binding: None,
                span: Span::default(),
            })],
        };

        let result = enrich_program(raw, &registry);

        if let Statement::VerbCall(vc) = &result.program.statements[0] {
            let entity_arg = vc.get_arg("entity-id").unwrap();
            if let AstNode::List { items, .. } = &entity_arg.value {
                // Each item should be EntityRef
                assert_eq!(items.len(), 2);
                assert!(items[0].is_entity_ref());
                assert!(items[1].is_entity_ref());

                if let AstNode::EntityRef { value, .. } = &items[0] {
                    assert_eq!(value, "Alice");
                }
                if let AstNode::EntityRef { value, .. } = &items[1] {
                    assert_eq!(value, "Bob");
                }
            } else {
                panic!("Expected List");
            }
        }
    }

    #[test]
    fn test_enrich_unknown_verb() {
        let registry = test_registry();

        // Raw AST with unknown verb - should pass through unchanged
        let raw = Program {
            statements: vec![Statement::VerbCall(VerbCall {
                domain: "unknown".to_string(),
                verb: "verb".to_string(),
                arguments: vec![Argument {
                    key: "name".to_string(),
                    value: AstNode::Literal(Literal::String("Test".to_string())),
                    span: Span::default(),
                }],
                binding: None,
                span: Span::default(),
            })],
        };

        let result = enrich_program(raw, &registry);

        // No errors - unknown verbs just pass through
        // (validation will catch unknown verbs later)
        if let Statement::VerbCall(vc) = &result.program.statements[0] {
            let name_arg = vc.get_arg("name").unwrap();
            // Should stay as literal string since no lookup config
            assert!(name_arg.value.is_literal());
        }
    }
}
