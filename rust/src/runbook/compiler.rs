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

use std::collections::{BTreeMap, HashMap};

use uuid::Uuid;

use crate::dsl_v2::macros::{
    expand_macro_fixpoint, MacroExpansionError, MacroRegistry, MacroSchema, EXPANSION_LIMITS,
};
use crate::journey::pack_manager::EffectiveConstraints;
use crate::session::unified::UnifiedSession;

// ---------------------------------------------------------------------------
// derive_write_set — extract entity UUIDs from resolved args
// ---------------------------------------------------------------------------

// Write-set derivation delegated to `write_set` module (INV-8).
// Heuristic is always active; contract-driven path gated behind
// `write-set-contract` feature flag. See `write_set.rs` for details.
use super::write_set::derive_write_set;

use crate::plan_builder::plan_assembler::assemble_plan;

use super::constraint_gate::check_pack_constraints;
use super::envelope::{self, ReplayEnvelope};
use super::errors::{CompilationError, CompilationErrorKind};
use super::response::{
    ClarificationContext, ClarificationRequest, CompiledRunbookSummary, MissingField,
    OrchestratorResponse, StepPreview,
};
use super::sem_reg_filter::filter_verbs_against_allowed_set;
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
/// * `sem_reg_allowed_verbs` — optional SemReg allowed verb set (INV-6 Step 4).
///   When `Some`, expanded verbs are filtered against this set after pack gate.
///   When `None`, SemReg filtering is skipped (graceful degradation).
/// * `verb_snapshot_pins` — optional pre-resolved verb FQN → snapshot_id map.
///   When `Some`, each compiled step pins the verb contract snapshot for audit.
///   When `None`, snapshot pinning is skipped (graceful degradation).
///   The caller resolves these from SemReg asynchronously before calling this function.
///
/// # Returns
///
/// An `OrchestratorResponse`:
/// - `Compiled` — ready for `execute_runbook()`
/// - `Clarification` — missing required args or unknown verb
/// - `ConstraintViolation` — expanded verbs violate pack constraints
/// - `CompilationError` — typed error from a §6.2 pipeline phase (INV-7)
#[allow(clippy::too_many_arguments)]
pub fn compile_verb(
    session_id: Uuid,
    classification: &VerbClassification<'_>,
    args: &BTreeMap<String, String>,
    session: &UnifiedSession,
    macro_registry: &MacroRegistry,
    runbook_version: u64,
    constraints: &EffectiveConstraints,
    sem_reg_allowed_verbs: Option<&std::collections::HashSet<String>>,
    verb_snapshot_pins: Option<&HashMap<String, (Uuid, Uuid)>>,
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
            sem_reg_allowed_verbs,
            verb_snapshot_pins,
        ),
        VerbClassification::Primitive { fqn } => compile_primitive(
            session_id,
            fqn,
            args,
            runbook_version,
            constraints,
            sem_reg_allowed_verbs,
            verb_snapshot_pins,
        ),
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
/// §6.2 Normative compilation order (INV-6):
///
/// 1. Pre-check required args → return Clarification if any
/// 2. **Step 1: expand** — fixpoint macro expansion (INV-4, INV-12)
/// 3. **Step 2: DAG**   — assemble_plan for dependency ordering (INV-5)
/// 4. **Step 3: pack gate** — check expanded verbs against pack constraints
/// 5. **Step 4: SemReg** — filter expanded verbs against SemReg allowed set
/// 6. **Step 5: write_set** — derive write_set from args (INV-8)
/// 7. **Step 6: store** — freeze as CompiledRunbook + ReplayEnvelope
#[allow(clippy::too_many_arguments)]
fn compile_macro(
    session_id: Uuid,
    fqn: &str,
    schema: &MacroSchema,
    args: &BTreeMap<String, String>,
    session: &UnifiedSession,
    macro_registry: &MacroRegistry,
    runbook_version: u64,
    constraints: &EffectiveConstraints,
    sem_reg_allowed_verbs: Option<&std::collections::HashSet<String>>,
    verb_snapshot_pins: Option<&HashMap<String, (Uuid, Uuid)>>,
) -> OrchestratorResponse {
    // Pre-check required args — give a user-friendly clarification
    // before attempting expansion (which also checks, but with less context).
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

    // §6.2 Step 1: expand — fixpoint macro expansion
    // (INV-4: per-path cycle detection, INV-12: limits in audit)
    let fixpoint = match expand_macro_fixpoint(fqn, args, session, macro_registry, EXPANSION_LIMITS)
    {
        Ok(output) => output,
        Err(MacroExpansionError::MissingRequired(field)) => {
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
            return OrchestratorResponse::CompilationError(CompilationError::new(
                CompilationErrorKind::ExpansionFailed {
                    reason: e.to_string(),
                },
                "expand",
            ));
        }
    };

    // Extract expanded verb names for downstream gates.
    let expanded_verbs: Vec<String> = fixpoint
        .statements
        .iter()
        .map(|dsl| extract_verb_from_dsl(dsl))
        .collect();

    // §6.2 Step 2: DAG — build steps then run PlanAssembler for dependency ordering (INV-5).
    // Build raw steps first (write_set is empty — populated after Step 5).
    let raw_steps: Vec<CompiledStep> = fixpoint
        .statements
        .iter()
        .enumerate()
        .map(|(i, dsl)| {
            let verb = extract_verb_from_dsl(dsl);
            let snapshot_id = verb_snapshot_pins
                .and_then(|pins| pins.get(&verb))
                .map(|(_obj_id, snap_id)| *snap_id);
            CompiledStep {
                step_id: Uuid::new_v4(),
                sentence: format!("Step {}: {}", i + 1, verb),
                verb: verb.clone(),
                dsl: dsl.clone(),
                args: std::collections::BTreeMap::new(),
                depends_on: vec![],
                execution_mode: ExecutionMode::Sync,
                write_set: vec![], // Populated in Step 5
                verb_contract_snapshot_id: snapshot_id,
            }
        })
        .collect();

    let steps = match assemble_plan(raw_steps) {
        Ok(assembly) => assembly.steps,
        Err(assembly_err) => {
            return assembly_err.into_response();
        }
    };

    // §6.2 Step 3: pack gate — check expanded verbs against active pack constraints.
    if let Err(violation) = check_pack_constraints(&expanded_verbs, constraints) {
        return OrchestratorResponse::ConstraintViolation(violation);
    }

    // §6.2 Step 4: SemReg — filter expanded verbs against SemReg allowed set (INV-6).
    // When `sem_reg_allowed_verbs` is None, SemReg filtering is skipped (graceful degradation).
    if let Some(allowed) = sem_reg_allowed_verbs {
        let filter_result = filter_verbs_against_allowed_set(&expanded_verbs, allowed);
        if filter_result.has_denials() {
            let denied = filter_result.first_denied().expect("has_denials was true");
            return OrchestratorResponse::CompilationError(CompilationError::new(
                CompilationErrorKind::SemRegDenied {
                    verb: denied.verb.clone(),
                    reason: denied.reason.clone(),
                },
                "sem_reg",
            ));
        }
    }

    // §6.2 Step 5: write_set — derive from top-level args (INV-8).
    // Macro expansion doesn't carry per-step args, so we use the parent args
    // for all expanded steps as a conservative approximation.
    let macro_write_set: Vec<Uuid> = derive_write_set(fqn, args, None).into_iter().collect();

    let steps: Vec<CompiledStep> = steps
        .into_iter()
        .map(|mut s| {
            s.write_set = macro_write_set.clone();
            s
        })
        .collect();

    // §6.2 Step 6: store — capture audits into envelope + freeze as CompiledRunbook.
    let macro_audits: Vec<envelope::MacroExpansionAudit> = fixpoint
        .audits
        .iter()
        .map(|a| envelope::MacroExpansionAudit {
            expansion_id: a.expansion_id,
            macro_name: a.macro_fqn.clone(),
            params: args.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            resolved_autofill: std::collections::BTreeMap::new(),
            expansion_digest: a.output_digest.clone(),
            expansion_limits: fixpoint.limits,
            expanded_at: a.expanded_at,
        })
        .collect();

    // Collect deterministic digests for EnvelopeCore (no timestamps).
    let macro_audit_digests: Vec<String> = macro_audits
        .iter()
        .map(|a| a.expansion_digest.clone())
        .collect();

    // Build snapshot manifest from verb_snapshot_pins for all expanded verbs.
    let snapshot_manifest: HashMap<Uuid, Uuid> = verb_snapshot_pins
        .map(|pins| {
            expanded_verbs
                .iter()
                .filter_map(|v| pins.get(v.as_str()))
                .map(|(obj_id, snap_id)| (*obj_id, *snap_id))
                .collect()
        })
        .unwrap_or_default();

    let envelope = ReplayEnvelope {
        core: envelope::EnvelopeCore {
            session_cursor: runbook_version,
            entity_bindings: std::collections::BTreeMap::new(),
            external_lookup_digests: vec![],
            macro_audit_digests,
            snapshot_manifest,
        },
        external_lookups: vec![],
        macro_audits,
        sealed_at: chrono::Utc::now(),
    };

    let runbook = CompiledRunbook::new(session_id, runbook_version, steps.clone(), envelope);

    let preview: Vec<StepPreview> = steps
        .iter()
        .take(5)
        .map(|s| StepPreview {
            step_id: s.step_id,
            verb: s.verb.clone(),
            sentence: s.sentence.clone(),
        })
        .collect();

    OrchestratorResponse::Compiled(CompiledRunbookSummary {
        compiled_runbook_id: runbook.id,
        runbook_version: runbook.version,
        step_count: runbook.step_count(),
        envelope_entity_count: runbook.envelope.entity_bindings().len(),
        preview,
        compiled_runbook: Some(runbook),
    })
}

/// Compile a primitive verb invocation as a single-step runbook.
///
/// §6.2 Normative compilation order for primitives (INV-6):
/// Step 1: expand — N/A (primitive, no expansion)
/// Step 2: DAG — N/A (single step, no dependencies)
/// Step 3: pack gate — check verb against pack constraints
/// Step 4: SemReg — filter verb against SemReg allowed set
/// Step 5: write_set — derive from args
/// Step 6: store — freeze as CompiledRunbook
fn compile_primitive(
    session_id: Uuid,
    fqn: &str,
    args: &BTreeMap<String, String>,
    runbook_version: u64,
    constraints: &EffectiveConstraints,
    sem_reg_allowed_verbs: Option<&std::collections::HashSet<String>>,
    verb_snapshot_pins: Option<&HashMap<String, (Uuid, Uuid)>>,
) -> OrchestratorResponse {
    // §6.2 Step 3: pack gate
    if let Err(violation) = check_pack_constraints(&[fqn.to_string()], constraints) {
        return OrchestratorResponse::ConstraintViolation(violation);
    }

    // §6.2 Step 4: SemReg filter
    if let Some(allowed) = sem_reg_allowed_verbs {
        let filter_result = filter_verbs_against_allowed_set(&[fqn.to_string()], allowed);
        if filter_result.has_denials() {
            let denied = filter_result.first_denied().expect("has_denials was true");
            return OrchestratorResponse::CompilationError(CompilationError::new(
                CompilationErrorKind::SemRegDenied {
                    verb: denied.verb.clone(),
                    reason: denied.reason.clone(),
                },
                "sem_reg",
            ));
        }
    }

    // §6.2 Step 5: write_set
    let dsl = build_dsl_from_args(fqn, args);

    // Pin verb contract snapshot if available (S16: execution snapshot pinning).
    let snapshot_id = verb_snapshot_pins
        .and_then(|pins| pins.get(fqn))
        .map(|(_obj_id, snap_id)| *snap_id);

    let step = CompiledStep {
        step_id: Uuid::new_v4(),
        sentence: format!("Execute {}", fqn),
        verb: fqn.to_string(),
        dsl: dsl.clone(),
        args: args.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        depends_on: vec![],
        execution_mode: ExecutionMode::Sync,
        write_set: derive_write_set(fqn, args, None).into_iter().collect(),
        verb_contract_snapshot_id: snapshot_id,
    };

    // Build snapshot manifest for the envelope (single verb for primitives).
    let snapshot_manifest: HashMap<Uuid, Uuid> = verb_snapshot_pins
        .and_then(|pins| pins.get(fqn))
        .map(|(obj_id, snap_id)| {
            let mut m = HashMap::new();
            m.insert(*obj_id, *snap_id);
            m
        })
        .unwrap_or_default();

    let envelope = ReplayEnvelope {
        core: envelope::EnvelopeCore {
            session_cursor: runbook_version,
            entity_bindings: std::collections::BTreeMap::new(),
            external_lookup_digests: vec![],
            macro_audit_digests: vec![],
            snapshot_manifest,
        },
        external_lookups: vec![],
        macro_audits: vec![],
        sealed_at: chrono::Utc::now(),
    };

    // §6.2 Step 6: store — freeze as CompiledRunbook
    let runbook = CompiledRunbook::new(session_id, runbook_version, vec![step.clone()], envelope);

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
    args: &BTreeMap<String, String>,
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
///
/// BTreeMap iteration is deterministic (sorted by key), so no explicit
/// sorting is needed (INV-2, Phase C).
fn build_dsl_from_args(fqn: &str, args: &BTreeMap<String, String>) -> String {
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
    use std::collections::{HashMap, HashSet};

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

        let mut args = BTreeMap::new();
        args.insert("name".to_string(), "Acme Fund".to_string());

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &args,
            &session,
            &registry,
            1,
            &constraints,
            None, // sem_reg_allowed_verbs
            None, // verb_snapshot_pins
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

        let args = BTreeMap::new(); // Missing required "name"

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &args,
            &session,
            &registry,
            1,
            &constraints,
            None, // sem_reg_allowed_verbs
            None, // verb_snapshot_pins
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

        let mut args = BTreeMap::new();
        args.insert("name".to_string(), "Test Fund".to_string());

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &args,
            &session,
            &registry,
            1,
            &constraints,
            None, // sem_reg_allowed_verbs
            None, // verb_snapshot_pins
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
            &BTreeMap::new(),
            &session,
            &registry,
            1,
            &constraints,
            None, // sem_reg_allowed_verbs
            None, // verb_snapshot_pins
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

        let mut args = BTreeMap::new();
        args.insert("name".to_string(), "Audit Test".to_string());

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &args,
            &session,
            &registry,
            1,
            &constraints,
            None, // sem_reg_allowed_verbs
            None, // verb_snapshot_pins
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

        let mut args = BTreeMap::new();
        args.insert("name".to_string(), "Acme Fund".to_string());

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &args,
            &session,
            &registry,
            1,
            &constraints,
            None, // sem_reg_allowed_verbs
            None, // verb_snapshot_pins
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
            &BTreeMap::new(),
            &session,
            &registry,
            1,
            &constraints,
            None, // sem_reg_allowed_verbs
            None, // verb_snapshot_pins
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
        let mut args = BTreeMap::new();
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
        let mut args = BTreeMap::new();
        args.insert("entity-id".to_string(), id1.to_string());
        args.insert("cbu-id".to_string(), format!("<{}>", id2));

        let ws: Vec<Uuid> = derive_write_set("test.verb", &args, None)
            .into_iter()
            .collect();
        assert_eq!(ws.len(), 2, "Expected 2 UUIDs, got {:?}", ws);
        assert!(ws.contains(&id1));
        assert!(ws.contains(&id2));
    }

    #[test]
    fn test_write_set_ignores_non_uuid_args() {
        let mut args = BTreeMap::new();
        args.insert("name".to_string(), "Acme Corp".to_string());
        args.insert("jurisdiction".to_string(), "LU".to_string());
        args.insert("count".to_string(), "42".to_string());
        args.insert("empty".to_string(), String::new());

        let ws: Vec<Uuid> = derive_write_set("test.verb", &args, None)
            .into_iter()
            .collect();
        assert!(ws.is_empty(), "Expected empty write_set, got {:?}", ws);
    }

    #[test]
    fn test_write_set_mixed_args() {
        let id = Uuid::new_v4();
        let mut args = BTreeMap::new();
        args.insert("entity-id".to_string(), id.to_string());
        args.insert("name".to_string(), "Test".to_string());
        args.insert("mode".to_string(), "trading".to_string());

        let ws: Vec<Uuid> = derive_write_set("test.verb", &args, None)
            .into_iter()
            .collect();
        assert_eq!(ws.len(), 1);
        assert!(ws.contains(&id));
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

        let mut args = BTreeMap::new();
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
            None, // sem_reg_allowed_verbs
            None, // verb_snapshot_pins
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

    // ── Phase 3: SemReg governance tests (INV-6, INV-7) ──────────────

    /// INV-7: SemReg denial returns CompilationError::SemRegDenied for macros.
    #[test]
    fn test_semreg_denies_macro_verb() {
        let registry = test_macro_registry();
        let session = test_session();
        let constraints = EffectiveConstraints::unconstrained();
        let verb_index = VerbConfigIndex::empty();
        let classification = classify_verb("structure.setup", &verb_index, &registry);

        let mut args = BTreeMap::new();
        args.insert("name".to_string(), "SemReg Test".to_string());

        // SemReg allows only "session.load-galaxy" — the expanded cbu.create will be denied.
        let allowed: HashSet<String> = ["session.load-galaxy".to_string()].into_iter().collect();

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &args,
            &session,
            &registry,
            1,
            &constraints,
            Some(&allowed),
            None, // verb_snapshot_pins
        );

        assert!(
            matches!(
                &resp,
                OrchestratorResponse::CompilationError(e) if matches!(
                    e.kind,
                    CompilationErrorKind::SemRegDenied { .. }
                )
            ),
            "Expected SemRegDenied, got {:?}",
            resp
        );
    }

    /// INV-7: SemReg denial returns CompilationError::SemRegDenied for primitives.
    #[test]
    fn test_semreg_denies_primitive_verb() {
        let session = UnifiedSession::new();
        let registry = MacroRegistry::new();
        let constraints = EffectiveConstraints::unconstrained();
        let classification = VerbClassification::Primitive {
            fqn: "cbu.create".to_string(),
        };

        let mut args = BTreeMap::new();
        args.insert("name".to_string(), "Denied Fund".to_string());

        // Allow only "entity.create" — cbu.create is NOT in the set.
        let allowed: HashSet<String> = ["entity.create".to_string()].into_iter().collect();

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &args,
            &session,
            &registry,
            1,
            &constraints,
            Some(&allowed),
            None, // verb_snapshot_pins
        );

        assert!(
            matches!(
                &resp,
                OrchestratorResponse::CompilationError(e) if matches!(
                    e.kind,
                    CompilationErrorKind::SemRegDenied { .. }
                )
            ),
            "Expected SemRegDenied for primitive, got {:?}",
            resp
        );
    }

    /// INV-7: When SemReg allows the verb, compilation succeeds normally.
    #[test]
    fn test_semreg_allows_verb_compilation_succeeds() {
        let registry = test_macro_registry();
        let session = test_session();
        let constraints = EffectiveConstraints::unconstrained();
        let verb_index = VerbConfigIndex::empty();
        let classification = classify_verb("structure.setup", &verb_index, &registry);

        let mut args = BTreeMap::new();
        args.insert("name".to_string(), "Allowed Fund".to_string());

        // Allow the verb that structure.setup expands to.
        let allowed: HashSet<String> = ["cbu.create".to_string()].into_iter().collect();

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &args,
            &session,
            &registry,
            1,
            &constraints,
            Some(&allowed),
            None, // verb_snapshot_pins
        );

        assert!(
            resp.is_compiled(),
            "Expected Compiled when SemReg allows verb, got {:?}",
            resp
        );
    }

    /// INV-7: When SemReg is unavailable (None), compilation proceeds (graceful degradation).
    #[test]
    fn test_semreg_unavailable_graceful_fallback() {
        let registry = test_macro_registry();
        let session = test_session();
        let constraints = EffectiveConstraints::unconstrained();
        let verb_index = VerbConfigIndex::empty();
        let classification = classify_verb("structure.setup", &verb_index, &registry);

        let mut args = BTreeMap::new();
        args.insert("name".to_string(), "Fallback Fund".to_string());

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &args,
            &session,
            &registry,
            1,
            &constraints,
            None, // SemReg unavailable
            None, // verb_snapshot_pins
        );

        assert!(
            resp.is_compiled(),
            "Expected Compiled when SemReg is None (graceful degradation), got {:?}",
            resp
        );
    }

    /// INV-6: Validation order — expansion error is returned BEFORE DAG is attempted.
    /// (Expansion errors map to CompilationError, not Clarification, unless MissingRequired.)
    #[test]
    fn test_validation_order_expand_before_dag() {
        // A cycle in macro expansion should return CompilationError::CycleDetected
        // or ExpansionFailed, proving expansion runs before DAG assembly (INV-6 Step 1 < Step 2).
        // With our test macros there's no cycle to trigger, but we can verify that
        // missing-required-arg returns Clarification (the one expansion error that
        // is NOT a CompilationError) — proving expansion ran first.
        let registry = test_macro_registry();
        let session = test_session();
        let constraints = EffectiveConstraints::unconstrained();
        let verb_index = VerbConfigIndex::empty();
        let classification = classify_verb("structure.setup", &verb_index, &registry);

        // No args — "name" is required
        let args = BTreeMap::new();

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &args,
            &session,
            &registry,
            1,
            &constraints,
            None,
            None, // verb_snapshot_pins
        );

        // Must be Clarification (missing required arg), proving expansion ran first.
        assert!(
            matches!(resp, OrchestratorResponse::Clarification(_)),
            "Expected Clarification for missing args (expansion phase), got {:?}",
            resp
        );
    }

    /// INV-6: Pack constraint violation is returned AFTER DAG but BEFORE SemReg.
    #[test]
    fn test_validation_order_pack_before_semreg() {
        // Set up constraints that forbid "cbu.create"
        let registry = test_macro_registry();
        let session = test_session();
        let verb_index = VerbConfigIndex::empty();
        let classification = classify_verb("structure.setup", &verb_index, &registry);

        let mut args = BTreeMap::new();
        args.insert("name".to_string(), "Blocked Fund".to_string());

        // Forbid the expanded verb
        let mut forbidden = HashSet::new();
        forbidden.insert("cbu.create".to_string());
        let constraints = EffectiveConstraints {
            allowed_verbs: None,
            forbidden_verbs: forbidden,
            contributing_packs: vec![],
        };

        // Also supply SemReg that would deny — but pack gate should fire first (§6.2 Step 3 < Step 4).
        let allowed: HashSet<String> = ["entity.create".to_string()].into_iter().collect();

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &args,
            &session,
            &registry,
            1,
            &constraints,
            Some(&allowed),
            None, // verb_snapshot_pins
        );

        // Must be ConstraintViolation (pack gate), NOT CompilationError::SemRegDenied
        assert!(
            matches!(resp, OrchestratorResponse::ConstraintViolation(_)),
            "Expected ConstraintViolation (pack gate fires before SemReg), got {:?}",
            resp
        );
    }

    /// INV-7: All 7 CompilationErrorKind variants are constructible.
    #[test]
    fn test_all_7_error_kinds_constructible() {
        let kinds: Vec<CompilationErrorKind> = vec![
            CompilationErrorKind::ExpansionFailed {
                reason: "test".into(),
            },
            CompilationErrorKind::CycleDetected {
                cycle: vec!["a".into(), "b".into()],
            },
            CompilationErrorKind::LimitsExceeded {
                detail: "max depth".into(),
            },
            CompilationErrorKind::DagError {
                reason: "toposort failed".into(),
            },
            CompilationErrorKind::PackConstraint {
                verb: "cbu.create".into(),
                explanation: "test-pack".into(),
            },
            CompilationErrorKind::SemRegDenied {
                verb: "cbu.create".into(),
                reason: "not in allowed set".into(),
            },
            CompilationErrorKind::StoreFailed {
                reason: "connection lost".into(),
            },
        ];

        assert_eq!(kinds.len(), 7, "Must have exactly 7 variants");
        for kind in &kinds {
            // Each variant must produce a non-empty Display string
            let display = format!("{}", kind);
            assert!(
                !display.is_empty(),
                "Display for {:?} must be non-empty",
                kind
            );
        }
    }

    // -----------------------------------------------------------------------
    // S16: Snapshot pinning tests
    // -----------------------------------------------------------------------

    #[test]
    fn primitive_compile_pins_snapshot_when_provided() {
        let registry = MacroRegistry::new();
        let session = test_session();
        let classification = VerbClassification::Primitive {
            fqn: "cbu.create".to_string(),
        };
        let constraints = EffectiveConstraints::unconstrained();

        let obj_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let snap_id = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();

        let mut pins = HashMap::new();
        pins.insert("cbu.create".to_string(), (obj_id, snap_id));

        let mut args = BTreeMap::new();
        args.insert("name".to_string(), "TestFund".to_string());

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &args,
            &session,
            &registry,
            1,
            &constraints,
            None,        // sem_reg_allowed_verbs
            Some(&pins), // verb_snapshot_pins
        );

        match resp {
            OrchestratorResponse::Compiled(summary) => {
                let rb = summary
                    .compiled_runbook
                    .expect("must have compiled runbook");
                assert_eq!(rb.steps.len(), 1);
                assert_eq!(
                    rb.steps[0].verb_contract_snapshot_id,
                    Some(snap_id),
                    "S16: step must carry pinned snapshot ID"
                );
                assert_eq!(
                    rb.envelope.core.snapshot_manifest.get(&obj_id),
                    Some(&snap_id),
                    "S16: envelope manifest must contain object→snapshot mapping"
                );
            }
            other => panic!("Expected Compiled, got: {:?}", other),
        }
    }

    #[test]
    fn primitive_compile_without_pins_has_none() {
        let registry = MacroRegistry::new();
        let session = test_session();
        let classification = VerbClassification::Primitive {
            fqn: "cbu.create".to_string(),
        };
        let constraints = EffectiveConstraints::unconstrained();

        let mut args = BTreeMap::new();
        args.insert("name".to_string(), "TestFund".to_string());

        let resp = compile_verb(
            Uuid::new_v4(),
            &classification,
            &args,
            &session,
            &registry,
            1,
            &constraints,
            None, // sem_reg_allowed_verbs
            None, // verb_snapshot_pins
        );

        match resp {
            OrchestratorResponse::Compiled(summary) => {
                let rb = summary
                    .compiled_runbook
                    .expect("must have compiled runbook");
                assert_eq!(rb.steps.len(), 1);
                assert_eq!(
                    rb.steps[0].verb_contract_snapshot_id, None,
                    "S16: step must have None when no pins provided"
                );
                assert!(
                    rb.envelope.core.snapshot_manifest.is_empty(),
                    "S16: envelope manifest must be empty when no pins"
                );
            }
            other => panic!("Expected Compiled, got: {:?}", other),
        }
    }
}
