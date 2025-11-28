//! DSL Assembler - deterministic conversion from intents to s-expressions
//!
//! This module takes validated intents and assembles them into DSL s-expressions.
//! The assembly is purely deterministic - no LLM involved.
//!
//! Uses a "virtual context" to track entities that will be created during
//! a multi-intent sequence, allowing refs like @last_cbu to resolve before execution.

use super::intent::{AssembledDsl, IntentError, IntentValidation, ParamValue, VerbIntent};
use super::session::SessionContext;
use crate::forth_engine::runtime::Runtime;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Virtual context for tracking entities during assembly
#[derive(Debug, Clone, Default)]
struct VirtualContext {
    last_cbu_id: Option<Uuid>,
    last_entity_id: Option<Uuid>,
    named_refs: HashMap<String, Uuid>,
}

impl VirtualContext {
    /// Resolve a reference, checking virtual context first
    fn resolve_ref(&self, ref_name: &str, real_context: &SessionContext) -> Option<String> {
        match ref_name {
            "@last_cbu" => self
                .last_cbu_id
                .or(real_context.last_cbu_id)
                .map(|u| format!("\"{}\"", u)),
            "@last_entity" => self
                .last_entity_id
                .or(real_context.last_entity_id)
                .map(|u| format!("\"{}\"", u)),
            _ if ref_name.starts_with('@') => {
                let name = &ref_name[1..];
                self.named_refs
                    .get(name)
                    .or_else(|| real_context.named_refs.get(name))
                    .map(|u| format!("\"{}\"", u))
            }
            _ => None,
        }
    }

    /// Track a new entity that will be created
    fn track_intent(&mut self, intent: &VerbIntent) {
        let verb = &intent.verb;
        
        // CBU creation verbs
        if verb.starts_with("cbu.") && (verb.contains("ensure") || verb.contains("create")) {
            self.last_cbu_id = Some(Uuid::new_v4());
        }
        
        // Entity creation verbs
        if verb.starts_with("entity.") && verb.contains("create") {
            self.last_entity_id = Some(Uuid::new_v4());
        }
    }
}

/// Deterministic DSL assembler
pub struct DslAssembler {
    runtime: Arc<Runtime>,
}

impl DslAssembler {
    /// Create a new DSL assembler
    pub fn new(runtime: Arc<Runtime>) -> Self {
        Self { runtime }
    }

    /// Validate a single intent against the verb registry
    pub fn validate_intent(&self, intent: &VerbIntent) -> IntentValidation {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // 1. Check verb exists
        let word = match self.runtime.get_word(&intent.verb) {
            Some(w) => w,
            None => {
                errors.push(IntentError {
                    code: "E001".to_string(),
                    message: format!("Unknown verb: {}", intent.verb),
                    param: None,
                });
                return IntentValidation {
                    valid: false,
                    intent: intent.clone(),
                    errors,
                    warnings,
                };
            }
        };

        // 2. Warn about empty params (some verbs might need them)
        if intent.params.is_empty() && intent.refs.is_empty() {
            warnings.push(format!(
                "Verb '{}' has no parameters - signature: {}",
                intent.verb, word.signature
            ));
        }

        // 3. Validate param types (basic validation)
        for (key, value) in &intent.params {
            match value {
                ParamValue::String(s) if s.is_empty() => {
                    warnings.push(format!("Parameter '{}' is empty", key));
                }
                _ => {}
            }
        }

        // 4. Validate refs format
        for (key, ref_name) in &intent.refs {
            if !ref_name.starts_with('@') {
                errors.push(IntentError {
                    code: "E002".to_string(),
                    message: format!("Invalid reference '{}' - must start with @", ref_name),
                    param: Some(key.clone()),
                });
            }
        }

        IntentValidation {
            valid: errors.is_empty(),
            intent: intent.clone(),
            errors,
            warnings,
        }
    }

    /// Validate all intents
    pub fn validate_all(&self, intents: &[VerbIntent]) -> Vec<IntentValidation> {
        intents.iter().map(|i| self.validate_intent(i)).collect()
    }

    /// Assemble DSL from validated intents
    ///
    /// Uses a virtual context to track entities that will be created,
    /// allowing refs like @last_cbu to resolve within a multi-intent sequence.
    pub fn assemble(
        &self,
        intents: &[VerbIntent],
        context: &SessionContext,
    ) -> Result<AssembledDsl, Vec<IntentError>> {
        let mut statements = Vec::new();
        let mut all_errors = Vec::new();
        let mut virtual_ctx = VirtualContext::default();

        for intent in intents {
            // Validate verb exists
            let validation = self.validate_intent(intent);
            if !validation.valid {
                all_errors.extend(validation.errors);
                continue;
            }

            // Check for unresolved refs using BOTH virtual and real context
            for (key, ref_name) in &intent.refs {
                if virtual_ctx.resolve_ref(ref_name, context).is_none() {
                    all_errors.push(IntentError {
                        code: "E003".to_string(),
                        message: format!(
                            "Cannot resolve reference '{}' for parameter '{}' - no prior entity exists in this sequence",
                            ref_name, key
                        ),
                        param: Some(key.clone()),
                    });
                }
            }

            if all_errors.is_empty() {
                // Assemble s-expression using virtual context
                let stmt = self.render_sexpr_with_virtual(intent, context, &virtual_ctx);
                statements.push(stmt);
                
                // Track what this intent will create for subsequent refs
                virtual_ctx.track_intent(intent);
            }
        }

        if !all_errors.is_empty() {
            return Err(all_errors);
        }

        let combined = statements.join("\n\n");

        Ok(AssembledDsl {
            intent_count: intents.len(),
            statements,
            combined,
        })
    }

    /// Render a single intent as an s-expression using virtual context
    fn render_sexpr_with_virtual(
        &self,
        intent: &VerbIntent,
        context: &SessionContext,
        virtual_ctx: &VirtualContext,
    ) -> String {
        let mut parts = Vec::new();

        // Start with verb
        parts.push(format!("({}", intent.verb));

        // Add literal params (sorted for determinism)
        let mut param_keys: Vec<_> = intent.params.keys().collect();
        param_keys.sort();

        for key in param_keys {
            if let Some(value) = intent.params.get(key) {
                parts.push(format!(":{} {}", key, value.to_dsl_string()));
            }
        }

        // Add references (resolve from virtual context first, then real context)
        let mut ref_keys: Vec<_> = intent.refs.keys().collect();
        ref_keys.sort();

        for key in ref_keys {
            if let Some(ref_name) = intent.refs.get(key) {
                if let Some(resolved) = virtual_ctx.resolve_ref(ref_name, context) {
                    parts.push(format!(":{} {}", key, resolved));
                }
                // If ref can't be resolved, skip it (validation should have caught this)
            }
        }

        // Close the s-expression
        parts.push(")".to_string());

        parts.join(" ")
    }

    /// Render a single intent as an s-expression
    ///
    /// Returns None if there are unresolved references that cannot be rendered.
    pub fn render_sexpr(&self, intent: &VerbIntent, context: &SessionContext) -> String {
        let mut parts = Vec::new();

        // Start with verb
        parts.push(format!("({}", intent.verb));

        // Add literal params (sorted for determinism)
        let mut param_keys: Vec<_> = intent.params.keys().collect();
        param_keys.sort();

        for key in param_keys {
            if let Some(value) = intent.params.get(key) {
                parts.push(format!(":{} {}", key, value.to_dsl_string()));
            }
        }

        // Add references (resolve from context, sorted for determinism)
        // Skip refs that can't be resolved - they would cause parse errors
        let mut ref_keys: Vec<_> = intent.refs.keys().collect();
        ref_keys.sort();

        for key in ref_keys {
            if let Some(ref_name) = intent.refs.get(key) {
                if let Some(resolved) = context.resolve_ref(ref_name) {
                    parts.push(format!(":{} {}", key, resolved));
                }
                // If ref can't be resolved, skip it - better than outputting @last_cbu
            }
        }

        // Close the s-expression
        parts.push(")".to_string());

        parts.join(" ")
    }

    /// Check if all references in an intent can be resolved
    pub fn can_resolve_refs(&self, intent: &VerbIntent, context: &SessionContext) -> bool {
        intent
            .refs
            .values()
            .all(|ref_name| context.resolve_ref(ref_name).is_some())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forth_engine::vocab_registry::create_standard_runtime;
    use std::collections::HashMap;

    fn create_assembler() -> DslAssembler {
        let runtime = Arc::new(create_standard_runtime());
        DslAssembler::new(runtime)
    }

    #[test]
    fn test_validate_valid_intent() {
        let assembler = create_assembler();

        let intent = VerbIntent {
            verb: "cbu.ensure".to_string(),
            params: HashMap::from([
                (
                    "cbu-name".to_string(),
                    ParamValue::String("Test Corp".to_string()),
                ),
                (
                    "client-type".to_string(),
                    ParamValue::String("COMPANY".to_string()),
                ),
            ]),
            refs: HashMap::new(),
            sequence: None,
        };

        let validation = assembler.validate_intent(&intent);
        assert!(validation.valid);
        assert!(validation.errors.is_empty());
    }

    #[test]
    fn test_validate_unknown_verb() {
        let assembler = create_assembler();

        let intent = VerbIntent {
            verb: "unknown.verb".to_string(),
            params: HashMap::new(),
            refs: HashMap::new(),
            sequence: None,
        };

        let validation = assembler.validate_intent(&intent);
        assert!(!validation.valid);
        assert_eq!(validation.errors.len(), 1);
        assert_eq!(validation.errors[0].code, "E001");
    }

    #[test]
    fn test_validate_invalid_ref() {
        let assembler = create_assembler();

        let intent = VerbIntent {
            verb: "cbu.ensure".to_string(),
            params: HashMap::new(),
            refs: HashMap::from([("cbu-id".to_string(), "not_a_ref".to_string())]),
            sequence: None,
        };

        let validation = assembler.validate_intent(&intent);
        assert!(!validation.valid);
        assert!(validation.errors.iter().any(|e| e.code == "E002"));
    }

    #[test]
    fn test_render_simple_intent() {
        let assembler = create_assembler();
        let context = SessionContext::default();

        let intent = VerbIntent {
            verb: "cbu.ensure".to_string(),
            params: HashMap::from([
                (
                    "cbu-name".to_string(),
                    ParamValue::String("Test Corp".to_string()),
                ),
                (
                    "client-type".to_string(),
                    ParamValue::String("COMPANY".to_string()),
                ),
            ]),
            refs: HashMap::new(),
            sequence: None,
        };

        let dsl = assembler.render_sexpr(&intent, &context);
        assert!(dsl.starts_with("(cbu.ensure"));
        assert!(dsl.contains(":cbu-name \"Test Corp\""));
        assert!(dsl.contains(":client-type \"COMPANY\""));
        assert!(dsl.ends_with(')'));
    }

    #[test]
    fn test_render_with_refs() {
        let assembler = create_assembler();

        let cbu_id = uuid::Uuid::new_v4();
        let entity_id = uuid::Uuid::new_v4();

        let mut context = SessionContext::default();
        context.last_cbu_id = Some(cbu_id);
        context.last_entity_id = Some(entity_id);

        let intent = VerbIntent {
            verb: "cbu.attach-entity".to_string(),
            params: HashMap::from([(
                "role".to_string(),
                ParamValue::String("DIRECTOR".to_string()),
            )]),
            refs: HashMap::from([
                ("cbu-id".to_string(), "@last_cbu".to_string()),
                ("entity-id".to_string(), "@last_entity".to_string()),
            ]),
            sequence: None,
        };

        let dsl = assembler.render_sexpr(&intent, &context);
        assert!(dsl.starts_with("(cbu.attach-entity"));
        assert!(dsl.contains(":role \"DIRECTOR\""));
        // Should have resolved UUIDs
        assert!(dsl.contains(&format!(":cbu-id \"{}\"", cbu_id)));
        assert!(dsl.contains(&format!(":entity-id \"{}\"", entity_id)));
    }

    #[test]
    fn test_assemble_multiple_intents() {
        let assembler = create_assembler();
        let context = SessionContext::default();

        let intents = vec![
            VerbIntent {
                verb: "cbu.ensure".to_string(),
                params: HashMap::from([(
                    "cbu-name".to_string(),
                    ParamValue::String("Test Corp".to_string()),
                )]),
                refs: HashMap::new(),
                sequence: None,
            },
            VerbIntent {
                verb: "entity.create-proper-person".to_string(),
                params: HashMap::from([
                    (
                        "given-name".to_string(),
                        ParamValue::String("John".to_string()),
                    ),
                    (
                        "family-name".to_string(),
                        ParamValue::String("Smith".to_string()),
                    ),
                ]),
                refs: HashMap::new(),
                sequence: None,
            },
        ];

        let result = assembler.assemble(&intents, &context);
        assert!(result.is_ok());

        let assembled = result.unwrap();
        assert_eq!(assembled.intent_count, 2);
        assert_eq!(assembled.statements.len(), 2);
        assert!(assembled.combined.contains("cbu.ensure"));
        assert!(assembled.combined.contains("entity.create-proper-person"));
    }

    #[test]
    fn test_assemble_with_invalid_intent() {
        let assembler = create_assembler();
        let context = SessionContext::default();

        let intents = vec![
            VerbIntent {
                verb: "cbu.ensure".to_string(),
                params: HashMap::from([(
                    "cbu-name".to_string(),
                    ParamValue::String("Test Corp".to_string()),
                )]),
                refs: HashMap::new(),
                sequence: None,
            },
            VerbIntent {
                verb: "unknown.verb".to_string(),
                params: HashMap::new(),
                refs: HashMap::new(),
                sequence: None,
            },
        ];

        let result = assembler.assemble(&intents, &context);
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == "E001"));
    }

    #[test]
    fn test_deterministic_output() {
        let assembler = create_assembler();
        let context = SessionContext::default();

        let intent = VerbIntent {
            verb: "cbu.ensure".to_string(),
            params: HashMap::from([
                (
                    "cbu-name".to_string(),
                    ParamValue::String("Test".to_string()),
                ),
                (
                    "client-type".to_string(),
                    ParamValue::String("COMPANY".to_string()),
                ),
                (
                    "jurisdiction".to_string(),
                    ParamValue::String("GB".to_string()),
                ),
            ]),
            refs: HashMap::new(),
            sequence: None,
        };

        // Run multiple times - should get same output
        let dsl1 = assembler.render_sexpr(&intent, &context);
        let dsl2 = assembler.render_sexpr(&intent, &context);
        let dsl3 = assembler.render_sexpr(&intent, &context);

        assert_eq!(dsl1, dsl2);
        assert_eq!(dsl2, dsl3);
    }
}
