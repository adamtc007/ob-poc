//! Compilation pipeline — transforms a classified verb + args into a CompiledRunbook.
//!
//! ## Pipeline
//!
//! ```text
//! VerbClassification + args
//!     ↓
//! match classification:
//!     Macro  → validate args → expand_macro() → capture audit → build steps
//!     Prim   → build single step
//!     Unknown → return Clarification
//!     ↓
//! PackConstraintGate (check expanded verbs against active pack constraints)
//!     ↓
//! freeze as CompiledRunbook with ReplayEnvelope
//!     ↓
//! return OrchestratorResponse::Compiled
//! ```
//!
//! ## Feature Gate
//!
//! Gated behind `vnext-repl` because it depends on `VerbConfigIndex`.

use std::collections::HashMap;

use uuid::Uuid;

use crate::dsl_v2::macros::{expand_macro, MacroExpansionError, MacroRegistry, MacroSchema};
use crate::journey::pack_manager::EffectiveConstraints;
use crate::session::unified::UnifiedSession;

// ---------------------------------------------------------------------------
// derive_write_set — extract entity UUIDs from resolved args
// ---------------------------------------------------------------------------

/// Phase 1 write_set derivation: extract any arg value that parses as a UUID.
///
/// This catches the common case where entity IDs are passed as resolved
/// argument values (e.g., `:entity-id <uuid>`, `:cbu-id <uuid>`).
///
/// ## Future: Contract-Driven Extraction (Phase 2)
///
/// Phase 2 will use `VerbContractBody.writes_flags` or verb YAML `crud.table`
/// to determine which args represent write targets vs read-only references.
/// This requires contract integration which is deferred.
pub(crate) fn derive_write_set(args: &HashMap<String, String>) -> Vec<Uuid> {
    args.values()
        .filter_map(|v| {
            let trimmed = v.trim().trim_matches(|c| c == '<' || c == '>');
            Uuid::parse_str(trimmed).ok()
        })
        .collect()
}

use crate::plan_builder::plan_assembler::assemble_plan;

use super::constraint_gate::check_pack_constraints;
use super::envelope::{self, ReplayEnvelope};
use super::response::{
    ClarificationContext, ClarificationRequest, CompiledRunbookSummary, MissingField,
    OrchestratorResponse, StepPreview,
};
use super::types::{CompiledRunbook, CompiledStep, ExecutionMode};
use super::verb_classifier::VerbClassification;

/// Compile a classified verb invocation into a `CompiledRunbook`.
///
/// This is the core compilation function that bridges verb classification
/// (P1.2) with macro expansion (existing `expand_macro()`) and runbook
/// freezing (P0 types).
///
/// # Arguments
///
/// * `session_id` — session that owns this compilation
/// * `classification` — result from `classify_verb()`
/// * `args` — user-provided arguments (name → value)
/// * `session` — current session state (for macro context/autofill)
/// * `macro_registry` — for macro expansion
/// * `runbook_version` — monotonic version within the session
/// * `constraints` — effective constraints from PackManager (Phase 2)
///
/// # Returns
///
/// An `OrchestratorResponse`:
/// - `Compiled` — ready for `execute_runbook()`
/// - `Clarification` — missing required args or unknown verb
/// - `ConstraintViolation` — expanded verbs violate pack constraints
pub fn compile_verb(
    session_id: Uuid,
    classification: &VerbClassification<'_>,
    args: &HashMap<String, String>,
    session: &UnifiedSession,
    macro_registry: &MacroRegistry,
    runbook_version: u64,
    constraints: &EffectiveConstraints,
) -> OrchestratorResponse {
    match classification {
        VerbClassification::Macro { fqn, schema } => compile_macro(
            session_id,
            fqn,
            schema,
            args,
            session,
            macro_registry,
            runbook_version,
            constraints,
        ),
        VerbClassification::Primitive { fqn } => {
            compile_primitive(session_id, fqn, args, runbook_version, constraints)
        }
        VerbClassification::Unknown { name } => {
            OrchestratorResponse::Clarification(ClarificationRequest {
                question: format!("Unknown verb: '{}'. Did you mean something else?", name),
                missing_fields: vec![],
                context: ClarificationContext {
                    verb: Some(name.clone()),
                    is_macro: false,
                    extracted_args: args.clone(),
                },
            })
        }
    }
}

/// Compile a macro invocation.
///
/// 1. Check for missing required args → return Clarification if any
/// 2. Call `expand_macro()` → get expanded DSL statements + audit
/// 3. **PackConstraintGate** — check expanded verbs against constraints
/// 4. Build CompiledSteps from expanded statements
/// 5. Capture MacroExpansionAudit into ReplayEnvelope
/// 6. Freeze as CompiledRunbook → return Compiled
#[allow(clippy::too_many_arguments)]
fn compile_macro(
    session_id: Uuid,
    fqn: &str,
    schema: &MacroSchema,
    args: &HashMap<String, String>,
    session: &UnifiedSession,
    macro_registry: &MacroRegistry,
    runbook_version: u64,
    constraints: &EffectiveConstraints,
) -> OrchestratorResponse {
    // 1. Pre-check required args — give a user-friendly clarification
    //    before attempting expansion (which also checks, but with less context)
    let missing = find_missing_required_args(schema, args);
    if !missing.is_empty() {
        return OrchestratorResponse::Clarification(ClarificationRequest {
            question: format!("To run '{}', I need a few more details:", schema.ui.label),
            missing_fields: missing,
            context: ClarificationContext {
                verb: Some(fqn.to_string()),
                is_macro: true,
                extracted_args: args.clone(),
            },
        });
    }

    // 2. Expand macro
    let expansion = match expand_macro(fqn, args, session, macro_registry) {
        Ok(output) => output,
        Err(MacroExpansionError::MissingRequired(field)) => {
            // Autofill or conditional requirement not met — surface as clarification
            return OrchestratorResponse::Clarification(ClarificationRequest {
                question: format!(
                    "Missing required field for '{}': {}",
                    schema.ui.label, field
                ),
                missing_fields: vec![MissingField {
                    field_name: field.clone(),
                    reason: format!("Required by {}", fqn),
                    suggestions: vec![],
                    required: true,
                }],
                context: ClarificationContext {
                    verb: Some(fqn.to_string()),
                    is_macro: true,
                    extracted_args: args.clone(),
                },
            });
        }
        Err(e) => {
            // Other expansion errors → clarification with error detail
            return OrchestratorResponse::Clarification(ClarificationRequest {
                question: format!("Cannot expand '{}': {}", schema.ui.label, e),
                missing_fields: vec![],
                context: ClarificationContext {
                    verb: Some(fqn.to_string()),
                    is_macro: true,
                    extracted_args: args.clone(),
                },
            });
        }
    };

    // 3. PackConstraintGate — check expanded verbs against active pack constraints
    let expanded_verbs: Vec<String> = expansion
        .statements
        .iter()
        .map(|dsl| extract_verb_from_dsl(dsl))
        .collect();

    if let Err(violation) = check_pack_constraints(&expanded_verbs, constraints) {
        return OrchestratorResponse::ConstraintViolation(violation);
    }

    // 4. Build CompiledSteps from expanded DSL statements, then run through
    //    PlanAssembler to resolve binding dependencies and compute phases.
    // Derive write_set from top-level args as a conservative approximation.
    // Macro expansion doesn't carry per-step args, so we use the parent args
    // for all expanded steps. Phase 2 will use contract-driven extraction.
    let macro_write_set = derive_write_set(args);

    let raw_steps: Vec<CompiledStep> = expansion
        .statements
        .iter()
        .enumerate()
        .map(|(i, dsl)| {
            let verb = extract_verb_from_dsl(dsl);
            CompiledStep {
                step_id: Uuid::new_v4(),
                sentence: format!("Step {}: {}", i + 1, verb),
                verb: verb.clone(),
                dsl: dsl.clone(),
                args: HashMap::new(),
                depends_on: vec![],
                execution_mode: ExecutionMode::Sync,
                write_set: macro_write_set.clone(),
            }
        })
        .collect();

    // Run PlanAssembler to resolve dependencies and reorder if needed.
    let steps = match assemble_plan(raw_steps) {
        Ok(assembly) => assembly.steps,
        Err(assembly_err) => {
            return assembly_err.into_response();
        }
    };

    // 5. Capture audit into ReplayEnvelope
    let macro_audit = envelope::MacroExpansionAudit {
        expansion_id: expansion.audit.expansion_id,
        macro_name: expansion.audit.macro_fqn.clone(),
        params: args
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect(),
        resolved_autofill: HashMap::new(), // TODO: capture autofill in Phase 2
        expansion_digest: expansion.audit.output_digest.clone(),
        expanded_at: expansion.audit.expanded_at,
    };

    let envelope = ReplayEnvelope {
        session_cursor: runbook_version,
        entity_bindings: HashMap::new(),
        external_lookups: vec![],
        macro_audits: vec![macro_audit],
        sealed_at: chrono::Utc::now(),
    };

    // 6. Freeze as CompiledRunbook
    let runbook = CompiledRunbook::new(session_id, runbook_version, steps.clone(), envelope);

    let preview: Vec<StepPreview> = steps
        .iter()
        .take(5) // Show up to 5 steps in preview
        .map(|s| StepPreview {
            step_id: s.step_id,
            verb: s.verb.clone(),
            sentence: s.sentence.clone(),
        })
        .collect();

    // FUTURE: capture CompilationAudit for learning — feed macro expansion
    // outcomes into FeedbackService so the promotion pipeline can learn which
    // macro invocations succeed/fail and improve verb discovery accuracy.
    OrchestratorResponse::Compiled(CompiledRunbookSummary {
        compiled_runbook_id: runbook.id,
        runbook_version: runbook.version,
        step_count: runbook.step_count(),
        envelope_entity_count: runbook.envelope.entity_bindings.len(),
        preview,
        compiled_runbook: Some(runbook),
    })
}

/// Compile a primitive verb invocation as a single-step runbook.
fn compile_primitive(
    session_id: Uuid,
    fqn: &str,
    args: &HashMap<String, String>,
    runbook_version: u64,
    constraints: &EffectiveConstraints,
) -> OrchestratorResponse {
    // Check the verb against pack constraints
    if let Err(violation) = check_pack_constraints(&[fqn.to_string()], constraints) {
        return OrchestratorResponse::ConstraintViolation(violation);
    }

    // Build DSL from fqn + args
    let dsl = build_dsl_from_args(fqn, args);

    let step = CompiledStep {
        step_id: Uuid::new_v4(),
        sentence: format!("Execute {}", fqn),
        verb: fqn.to_string(),
        dsl: dsl.clone(),
        args: args.clone(),
        depends_on: vec![],
        execution_mode: ExecutionMode::Sync,
        write_set: derive_write_set(args),
    };

    let runbook = CompiledRunbook::new(
        session_id,
        runbook_version,
        vec![step.clone()],
        ReplayEnvelope::empty(),
    );

    // FUTURE: capture CompilationAudit for learning (primitive verb path)
    OrchestratorResponse::Compiled(CompiledRunbookSummary {
        compiled_runbook_id: runbook.id,
        runbook_version: runbook.version,
        step_count: 1,
        envelope_entity_count: 0,
        preview: vec![StepPreview {
            step_id: step.step_id,
            verb: step.verb,
            sentence: step.sentence,
        }],
        compiled_runbook: Some(runbook),
    })
}

/// Find missing required arguments for a macro schema.
fn find_missing_required_args(
    schema: &MacroSchema,
    args: &HashMap<String, String>,
) -> Vec<MissingField> {
    let mut missing = Vec::new();

    for (name, arg_spec) in schema.required_args() {
        if !args.contains_key(name) {
            let suggestions: Vec<String> = if arg_spec.is_enum() {
                arg_spec.values.iter().map(|v| v.label.clone()).collect()
            } else {
                vec![]
            };

            missing.push(MissingField {
                field_name: name.clone(),
                reason: if arg_spec.ui_label.is_empty() {
                    name.clone()
                } else {
                    arg_spec.ui_label.clone()
                },
                suggestions,
                required: true,
            });
        }
    }

    missing
}

/// Extract verb FQN from a DSL s-expression.
///
/// Simple heuristic: verb is the first token after the opening paren.
fn extract_verb_from_dsl(dsl: &str) -> String {
    let trimmed = dsl.trim();
    if let Some(after_paren) = trimmed.strip_prefix('(') {
        if let Some(end) = after_paren.find(|c: char| c.is_whitespace() || c == ')') {
            return after_paren[..end].to_string();
        }
        return after_paren.trim_end_matches(')').to_string();
    }
    // Comment or other non-DSL line
    trimmed.to_string()
}

/// Build a DSL s-expression from verb FQN and args.
fn build_dsl_from_args(fqn: &str, args: &HashMap<String, String>) -> String {
    let mut parts = vec![format!("({}", fqn)];
    for (name, value) in args {
        if value.contains(' ') {
            parts.push(format!(" :{} \"{}\"", name, value.replace('"', "\\\"")));
        } else {
            parts.push(format!(" :{} {}", name, value));
        }
    }
    parts.push(")".to_string());
    parts.join("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl_v2::macros::{
        ArgStyle, MacroArgs, MacroExpansionStep, MacroKind, MacroRouting, MacroSchema, MacroTarget,
        MacroUi, VerbCallStep,
    };
    use crate::journey::pack_manager::EffectiveConstraints;
    use crate::repl::verb_config_index::VerbConfigIndex;
    use crate::runbook::verb_classifier::classify_verb;
    use crate::session::unified::{ClientRef, StructureType};

    fn test_session() -> UnifiedSession {
        UnifiedSession {
            client: Some(ClientRef {
                client_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
                display_name: "Test Client".to_string(),
            }),
            structure_type: Some(StructureType::Pe),
            ..Default::default()
        }
    }

    fn test_macro_registry() -> MacroRegistry {
        let mut registry = MacroRegistry::new();
        let mut required = HashMap::new();
        required.insert(
            "name".to_string(),
            crate::dsl_v2::macros::MacroArg {
                arg_type: crate::dsl_v2::macros::MacroArgType::Str,
                ui_label: "Name".to_string(),
                autofill_from: None,
                picker: None,
                default: None,
                valid_values: vec![],
                values: vec![],
                default_key: None,
                item_type: None,
                internal: None,
                required_if: None,
                placeholder_if_missing: false,
            },
        );

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
                    required,
                    optional: Default::default(),
                },
                required_roles: vec![],
                optional_roles: vec![],
                docs_bundle: None,
                prereqs: vec![],
                expands_to: vec![MacroExpansionStep::VerbCall(VerbCallStep {
                    verb: "cbu.create".to_string(),
                    args: {
                        let mut m = HashMap::new();
                        m.insert("name".to_string(), "${arg.name}".to_string());
                        m.insert("client-id".to_string(), "${scope.client_id}".to_string());
                        m
                    },
                    bind_as: None,
                })],
                sets_state: vec![],
                unlocks: vec![],
            },
        );
        registry
    }

    #[test]
    fn test_compile_macro_success() {
        let registry = test_macro_registry();
        let session = test_session();
        let verb_index = VerbConfigIndex::empty();
        let classification = classify_verb("structure.setup", &verb_index, &registry);
        let constraints = EffectiveConstraints::unconstrained();

        let mut args = HashMap::new();
        args.insert("name".to_string(), "Acme Fund".to_string());

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &args,
            &session,
            &registry,
            1,
            &constraints,
        );

        assert!(resp.is_compiled(), "Expected Compiled, got {:?}", resp);
        if let OrchestratorResponse::Compiled(summary) = resp {
            assert_eq!(summary.step_count, 1);
            assert_eq!(summary.runbook_version, 1);
            assert_eq!(summary.preview[0].verb, "cbu.create");
        }
    }

    #[test]
    fn test_compile_macro_missing_args() {
        let registry = test_macro_registry();
        let session = test_session();
        let verb_index = VerbConfigIndex::empty();
        let classification = classify_verb("structure.setup", &verb_index, &registry);
        let constraints = EffectiveConstraints::unconstrained();

        let args = HashMap::new(); // Missing required "name"

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &args,
            &session,
            &registry,
            1,
            &constraints,
        );

        assert!(
            matches!(resp, OrchestratorResponse::Clarification(_)),
            "Expected Clarification for missing args, got {:?}",
            resp
        );
        if let OrchestratorResponse::Clarification(req) = resp {
            assert_eq!(req.missing_fields.len(), 1);
            assert_eq!(req.missing_fields[0].field_name, "name");
            assert!(req.context.is_macro);
        }
    }

    #[test]
    fn test_compile_primitive() {
        let registry = MacroRegistry::new();
        let session = test_session();
        let constraints = EffectiveConstraints::unconstrained();

        let classification = VerbClassification::Primitive {
            fqn: "cbu.create".to_string(),
        };

        let mut args = HashMap::new();
        args.insert("name".to_string(), "Test Fund".to_string());

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &args,
            &session,
            &registry,
            1,
            &constraints,
        );

        assert!(resp.is_compiled());
        if let OrchestratorResponse::Compiled(summary) = resp {
            assert_eq!(summary.step_count, 1);
            assert_eq!(summary.preview[0].verb, "cbu.create");
        }
    }

    #[test]
    fn test_compile_unknown_verb() {
        let registry = MacroRegistry::new();
        let session = test_session();
        let constraints = EffectiveConstraints::unconstrained();

        let classification = VerbClassification::Unknown {
            name: "nonexistent.verb".to_string(),
        };

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &HashMap::new(),
            &session,
            &registry,
            1,
            &constraints,
        );

        assert!(matches!(resp, OrchestratorResponse::Clarification(_)));
        if let OrchestratorResponse::Clarification(req) = resp {
            assert!(req.question.contains("nonexistent.verb"));
        }
    }

    #[test]
    fn test_macro_expansion_audit_captured() {
        let registry = test_macro_registry();
        let session = test_session();
        let verb_index = VerbConfigIndex::empty();
        let classification = classify_verb("structure.setup", &verb_index, &registry);
        let constraints = EffectiveConstraints::unconstrained();

        let mut args = HashMap::new();
        args.insert("name".to_string(), "Audit Test".to_string());

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &args,
            &session,
            &registry,
            1,
            &constraints,
        );

        assert!(resp.is_compiled());
    }

    #[test]
    fn test_compile_macro_constraint_violation() {
        let registry = test_macro_registry();
        let session = test_session();
        let verb_index = VerbConfigIndex::empty();
        let classification = classify_verb("structure.setup", &verb_index, &registry);

        // Constrain to kyc verbs only — cbu.create (expanded from macro) should fail
        let mut allowed = std::collections::HashSet::new();
        allowed.insert("kyc.create-case".to_string());
        let constraints = EffectiveConstraints {
            allowed_verbs: Some(allowed),
            forbidden_verbs: std::collections::HashSet::new(),
            contributing_packs: vec![],
        };

        let mut args = HashMap::new();
        args.insert("name".to_string(), "Acme Fund".to_string());

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &args,
            &session,
            &registry,
            1,
            &constraints,
        );

        assert!(
            matches!(resp, OrchestratorResponse::ConstraintViolation(_)),
            "Expected ConstraintViolation, got {:?}",
            resp
        );
        if let OrchestratorResponse::ConstraintViolation(detail) = resp {
            assert!(detail.violating_verbs.contains(&"cbu.create".to_string()));
        }
    }

    #[test]
    fn test_compile_primitive_constraint_violation() {
        let registry = MacroRegistry::new();
        let session = test_session();

        // Forbid cbu.delete
        let mut forbidden = std::collections::HashSet::new();
        forbidden.insert("cbu.delete".to_string());
        let constraints = EffectiveConstraints {
            allowed_verbs: None,
            forbidden_verbs: forbidden,
            contributing_packs: vec![],
        };

        let classification = VerbClassification::Primitive {
            fqn: "cbu.delete".to_string(),
        };

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &HashMap::new(),
            &session,
            &registry,
            1,
            &constraints,
        );

        assert!(
            matches!(resp, OrchestratorResponse::ConstraintViolation(_)),
            "Expected ConstraintViolation, got {:?}",
            resp
        );
    }

    #[test]
    fn test_extract_verb_from_dsl() {
        assert_eq!(
            extract_verb_from_dsl("(cbu.create :name \"Test\")"),
            "cbu.create"
        );
        assert_eq!(
            extract_verb_from_dsl("(session.load-galaxy :apex \"x\")"),
            "session.load-galaxy"
        );
        assert_eq!(extract_verb_from_dsl("(entity.create)"), "entity.create");
    }

    #[test]
    fn test_build_dsl_from_args() {
        let mut args = HashMap::new();
        args.insert("name".to_string(), "Acme".to_string());
        let dsl = build_dsl_from_args("cbu.create", &args);
        assert!(dsl.starts_with("(cbu.create"));
        assert!(dsl.contains(":name Acme"));
        assert!(dsl.ends_with(')'));
    }

    // -----------------------------------------------------------------------
    // R4 / R7: write_set derivation behavioral tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_set_derived_from_args() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let mut args = HashMap::new();
        args.insert("entity-id".to_string(), id1.to_string());
        args.insert("cbu-id".to_string(), format!("<{}>", id2));

        let ws = derive_write_set(&args);
        assert_eq!(ws.len(), 2, "Expected 2 UUIDs, got {:?}", ws);
        assert!(ws.contains(&id1));
        assert!(ws.contains(&id2));
    }

    #[test]
    fn test_write_set_ignores_non_uuid_args() {
        let mut args = HashMap::new();
        args.insert("name".to_string(), "Acme Corp".to_string());
        args.insert("jurisdiction".to_string(), "LU".to_string());
        args.insert("count".to_string(), "42".to_string());
        args.insert("empty".to_string(), String::new());

        let ws = derive_write_set(&args);
        assert!(ws.is_empty(), "Expected empty write_set, got {:?}", ws);
    }

    #[test]
    fn test_write_set_mixed_args() {
        let id = Uuid::new_v4();
        let mut args = HashMap::new();
        args.insert("entity-id".to_string(), id.to_string());
        args.insert("name".to_string(), "Test".to_string());
        args.insert("mode".to_string(), "trading".to_string());

        let ws = derive_write_set(&args);
        assert_eq!(ws.len(), 1);
        assert_eq!(ws[0], id);
    }

    #[test]
    fn test_compile_primitive_populates_write_set() {
        let registry = MacroRegistry::new();
        let session = test_session();
        let constraints = EffectiveConstraints::unconstrained();
        let entity_id = Uuid::new_v4();

        let classification = VerbClassification::Primitive {
            fqn: "entity.update".to_string(),
        };

        let mut args = HashMap::new();
        args.insert("entity-id".to_string(), entity_id.to_string());
        args.insert("name".to_string(), "Updated Name".to_string());

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &args,
            &session,
            &registry,
            1,
            &constraints,
        );

        assert!(resp.is_compiled());
        if let OrchestratorResponse::Compiled(summary) = resp {
            let runbook = summary
                .compiled_runbook
                .expect("Should have compiled_runbook");
            assert!(
                !runbook.steps[0].write_set.is_empty(),
                "write_set should contain the entity UUID"
            );
            assert!(
                runbook.steps[0].write_set.contains(&entity_id),
                "write_set should contain {:?}",
                entity_id
            );
        }
    }
}
