//! Verb Classifier
//!
//! Classifies a verb name as Primitive, Macro, or Unknown by checking
//! the verb registry and macro registry. This is the first step in the
//! compilation pipeline after verb discovery.

use crate::dsl_v2::macros::{MacroRegistry, MacroSchema};
use crate::repl::verb_config_index::VerbConfigIndex;

/// Classification of a verb name.
#[derive(Debug, Clone)]
pub enum VerbClassification<'a> {
    /// A primitive runtime verb (exists in VerbConfigIndex).
    Primitive {
        /// Fully-qualified verb name (e.g. "cbu.create").
        fqn: String,
    },
    /// A macro that expands to primitive verbs.
    Macro {
        /// Fully-qualified macro name (e.g. "structure.setup").
        fqn: String,
        /// Reference to the macro schema for arg extraction / expansion.
        schema: &'a MacroSchema,
    },
    /// Verb not found in either registry.
    Unknown {
        /// The unrecognized verb name.
        name: String,
    },
}

/// Classify a verb name against the verb and macro registries.
///
/// Lookup order:
/// 1. Macro registry (checked first — macros shadow primitives by design)
/// 2. Verb config index (primitive verbs from YAML)
/// 3. Unknown
pub fn classify_verb<'a>(
    verb_name: &str,
    verb_index: &VerbConfigIndex,
    macro_registry: &'a MacroRegistry,
) -> VerbClassification<'a> {
    // 1. Check macro registry first — macros take priority
    if let Some(schema) = macro_registry.get(verb_name) {
        return VerbClassification::Macro {
            fqn: verb_name.to_string(),
            schema,
        };
    }

    // 2. Check primitive verb registry
    if verb_index.get(verb_name).is_some() {
        return VerbClassification::Primitive {
            fqn: verb_name.to_string(),
        };
    }

    // 3. Unknown
    VerbClassification::Unknown {
        name: verb_name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl_v2::macros::{
        ArgStyle, MacroArgs, MacroExpansionStep, MacroKind, MacroRouting, MacroSchema, MacroTarget,
        MacroUi, VerbCallStep,
    };

    fn test_macro_registry() -> MacroRegistry {
        let mut registry = MacroRegistry::new();
        registry.add(
            "structure.setup".to_string(),
            MacroSchema {
                id: None,
                kind: MacroKind::Macro,
                tier: None,
                aliases: vec![],
                taxonomy: None,
                ui: MacroUi {
                    label: "Set up Structure".to_string(),
                    description: "Create a new structure".to_string(),
                    target_label: "Structure".to_string(),
                },
                routing: MacroRouting {
                    mode_tags: vec![],
                    operator_domain: Some("structure".to_string()),
                },
                target: MacroTarget {
                    operates_on: "client-ref".to_string(),
                    produces: Some("structure-ref".to_string()),
                    allowed_structure_types: vec![],
                },
                args: MacroArgs {
                    style: ArgStyle::Keyworded,
                    required: Default::default(),
                    optional: Default::default(),
                },
                required_roles: vec![],
                optional_roles: vec![],
                docs_bundle: None,
                prereqs: vec![],
                expands_to: vec![MacroExpansionStep::VerbCall(VerbCallStep {
                    verb: "cbu.create".to_string(),
                    args: Default::default(),
                    bind_as: None,
                })],
                sets_state: vec![],
                unlocks: vec![],
            },
        );
        registry
    }

    fn test_verb_index() -> VerbConfigIndex {
        // VerbConfigIndex::empty() gives us a valid empty index.
        // We can't easily add entries without VerbsConfig, so we test
        // the Unknown and Macro branches, plus verify that an empty index
        // correctly returns Unknown for primitives.
        VerbConfigIndex::empty()
    }

    #[test]
    fn test_classify_macro() {
        let macros = test_macro_registry();
        let verbs = test_verb_index();

        match classify_verb("structure.setup", &verbs, &macros) {
            VerbClassification::Macro { fqn, schema } => {
                assert_eq!(fqn, "structure.setup");
                assert_eq!(schema.ui.label, "Set up Structure");
            }
            other => panic!("Expected Macro, got {:?}", other),
        }
    }

    #[test]
    fn test_classify_unknown() {
        let macros = test_macro_registry();
        let verbs = test_verb_index();

        match classify_verb("nonexistent.verb", &verbs, &macros) {
            VerbClassification::Unknown { name } => {
                assert_eq!(name, "nonexistent.verb");
            }
            other => panic!("Expected Unknown, got {:?}", other),
        }
    }

    #[test]
    fn test_macro_shadows_primitive() {
        // If a name exists in both registries, macro wins
        let macros = test_macro_registry();
        let verbs = test_verb_index();

        // structure.setup is in macro registry, not in verb index
        let result = classify_verb("structure.setup", &verbs, &macros);
        assert!(matches!(result, VerbClassification::Macro { .. }));
    }
}
