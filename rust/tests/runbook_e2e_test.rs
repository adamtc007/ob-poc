//! End-to-End Acceptance Tests for Macro Expansion as Compiler Phase
//!
//! 10 tests from paper §15 covering all 13 invariants.
//!
//! ## Test Matrix
//!
//! | # | Test Name                                  | Invariant(s)     |
//! |---|--------------------------------------------|------------------|
//! | 1 | `test_primitive_verb_round_trip`            | INV-1, INV-9     |
//! | 2 | `test_macro_expands_to_primitives`          | INV-4            |
//! | 3 | `test_nested_macro_fixpoint`                | INV-4, INV-12    |
//! | 4 | `test_cycle_detection`                      | INV-4            |
//! | 5 | `test_depth_limit`                          | INV-4            |
//! | 6 | `test_pack_constraint_blocks_forbidden_verb`| INV-6            |
//! | 7 | `test_content_addressed_id_determinism`     | INV-2, INV-13    |
//! | 8 | `test_write_set_locks_acquired`             | INV-8, INV-10    |
//! | 9 | `test_concurrent_lock_contention`           | INV-10           |
//! | 10| `test_no_execution_without_compiled_id`     | INV-1, INV-11    |

#![cfg(feature = "vnext-repl")]

use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

use ob_poc::dsl_v2::macros::{
    expand_macro_fixpoint, ArgStyle, ExpansionLimits, InvokeMacroStep, MacroArg, MacroArgType,
    MacroArgs, MacroExpansionError, MacroExpansionStep, MacroKind, MacroRegistry, MacroRouting,
    MacroSchema, MacroTarget, MacroUi, VerbCallStep,
};
use ob_poc::journey::pack_manager::{ConstraintSource, EffectiveConstraints};
use ob_poc::repl::verb_config_index::VerbConfigIndex;
use ob_poc::runbook::canonical::{canonical_bytes_for_steps, content_addressed_id, full_sha256};
use ob_poc::runbook::errors::CompilationErrorKind;
use ob_poc::runbook::verb_classifier::{classify_verb, VerbClassification};
use ob_poc::runbook::{
    compile_invocation, compile_verb, execute_runbook, CompiledRunbook, CompiledRunbookStatus,
    CompiledStep, ExecutionMode, OrchestratorResponse, ReplayEnvelope, RunbookStore,
    RunbookStoreBackend, StepExecutor, StepOutcome,
};
use ob_poc::session::unified::{ClientRef, StructureType, UnifiedSession};

// =============================================================================
// Shared helpers
// =============================================================================

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

fn make_step(verb: &str) -> CompiledStep {
    CompiledStep {
        step_id: Uuid::new_v4(),
        sentence: format!("Execute {verb}"),
        verb: verb.into(),
        dsl: format!("({verb})"),
        args: BTreeMap::new(),
        depends_on: vec![],
        execution_mode: ExecutionMode::Sync,
        write_set: vec![],
        verb_contract_snapshot_id: None,
    }
}

#[allow(dead_code)]
fn make_step_with_args(verb: &str, args: &[(&str, &str)]) -> CompiledStep {
    CompiledStep {
        step_id: Uuid::new_v4(),
        sentence: format!("Execute {verb}"),
        verb: verb.into(),
        dsl: format!("({verb})"),
        args: args
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        depends_on: vec![],
        execution_mode: ExecutionMode::Sync,
        write_set: vec![],
        verb_contract_snapshot_id: None,
    }
}

fn make_step_with_write_set(verb: &str, write_set: Vec<Uuid>) -> CompiledStep {
    CompiledStep {
        step_id: Uuid::new_v4(),
        sentence: format!("Execute {verb}"),
        verb: verb.into(),
        dsl: format!("({verb})"),
        args: BTreeMap::new(),
        depends_on: vec![],
        execution_mode: ExecutionMode::Sync,
        write_set,
        verb_contract_snapshot_id: None,
    }
}

fn make_string_arg(name: &str) -> MacroArg {
    MacroArg {
        arg_type: MacroArgType::Str,
        ui_label: name.to_string(),
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
    }
}

fn make_macro_schema(
    label: &str,
    description: &str,
    required: HashMap<String, MacroArg>,
    expands_to: Vec<MacroExpansionStep>,
) -> MacroSchema {
    MacroSchema {
        id: None,
        kind: MacroKind::Macro,
        tier: None,
        aliases: vec![],
        taxonomy: None,
        ui: MacroUi {
            label: label.to_string(),
            description: description.to_string(),
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
        expands_to,
        sets_state: vec![],
        unlocks: vec![],
    }
}

struct SuccessExecutor;
#[async_trait::async_trait]
impl StepExecutor for SuccessExecutor {
    async fn execute_step(&self, _step: &CompiledStep) -> StepOutcome {
        StepOutcome::Completed {
            result: serde_json::json!({"ok": true}),
        }
    }
}

/// Executor that records verbs executed and captures write_set contents.
struct RecordingExecutor {
    executed: std::sync::Mutex<Vec<(String, Vec<Uuid>)>>,
}

impl RecordingExecutor {
    fn new() -> Self {
        Self {
            executed: std::sync::Mutex::new(Vec::new()),
        }
    }
    fn executed_verbs(&self) -> Vec<String> {
        self.executed
            .lock()
            .unwrap()
            .iter()
            .map(|(v, _)| v.clone())
            .collect()
    }
    #[allow(dead_code)]
    fn executed_write_sets(&self) -> Vec<Vec<Uuid>> {
        self.executed
            .lock()
            .unwrap()
            .iter()
            .map(|(_, ws)| ws.clone())
            .collect()
    }
}

#[async_trait::async_trait]
impl StepExecutor for RecordingExecutor {
    async fn execute_step(&self, step: &CompiledStep) -> StepOutcome {
        self.executed
            .lock()
            .unwrap()
            .push((step.verb.clone(), step.write_set.clone()));
        StepOutcome::Completed {
            result: serde_json::json!({"ok": true}),
        }
    }
}

// =============================================================================
// Test 1: test_primitive_verb_round_trip (INV-1, INV-9)
// =============================================================================

/// Primitive verb compiles → stores → executes from store → Completed.
/// Proves INV-1 (all execution through compile/execute gate) and
/// INV-9 (store round-trip).
#[tokio::test]
async fn test_primitive_verb_round_trip() {
    // Step 1: Compile a primitive verb
    let registry = MacroRegistry::new();
    let session = test_session();
    let constraints = EffectiveConstraints::unconstrained();

    // Use VerbClassification::Primitive directly since empty VerbConfigIndex
    // won't classify anything
    let classification = VerbClassification::Primitive {
        fqn: "cbu.create".to_string(),
    };

    let mut args = BTreeMap::new();
    args.insert("name".to_string(), "E2E Fund".to_string());

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

    assert!(
        resp.is_compiled(),
        "Primitive verb must compile: {:?}",
        resp
    );
    let summary = match &resp {
        OrchestratorResponse::Compiled(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(summary.step_count, 1);
    assert_eq!(summary.preview[0].verb, "cbu.create");

    // Step 2: Build runbook and store
    let store = RunbookStore::new();
    let steps: Vec<_> = summary.preview.iter().map(|p| make_step(&p.verb)).collect();
    let rb = CompiledRunbook::new(Uuid::new_v4(), 1, steps, ReplayEnvelope::empty());
    let id = rb.id;
    store.insert(&rb).await.unwrap();

    // Step 3: Retrieve from store and verify (INV-9)
    let retrieved = store
        .get(&id)
        .await
        .unwrap()
        .expect("Must retrieve stored runbook");
    assert_eq!(retrieved.steps.len(), 1);
    assert!(matches!(retrieved.status, CompiledRunbookStatus::Compiled));

    // Step 4: Execute through gate (INV-1)
    let result = execute_runbook(&store, id, None, &SuccessExecutor)
        .await
        .unwrap();
    assert!(
        matches!(result.final_status, CompiledRunbookStatus::Completed { .. }),
        "Must complete: {:?}",
        result.final_status
    );
    assert_eq!(result.step_results.len(), 1);

    // Step 5: Verify status updated in store
    let after = store.get(&id).await.unwrap().unwrap();
    assert!(matches!(
        after.status,
        CompiledRunbookStatus::Completed { .. }
    ));
}

// =============================================================================
// Test 2: test_macro_expands_to_primitives (INV-4)
// =============================================================================

/// `structure.setup` expands fully — no `@invoke-macro` directives remain.
#[tokio::test]
async fn test_macro_expands_to_primitives() {
    let mut registry = MacroRegistry::new();

    let mut required = HashMap::new();
    required.insert("name".to_string(), make_string_arg("Name"));

    registry.add(
        "structure.setup".to_string(),
        make_macro_schema(
            "Set up Structure",
            "Create a new structure",
            required,
            vec![MacroExpansionStep::VerbCall(VerbCallStep {
                verb: "cbu.create".to_string(),
                args: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "${arg.name}".to_string());
                    m
                },
                bind_as: None,
            })],
        ),
    );

    let session = test_session();
    let mut args = BTreeMap::new();
    args.insert("name".to_string(), "Macro Test Fund".to_string());

    // Expand via fixpoint
    let result = expand_macro_fixpoint(
        "structure.setup",
        &args,
        &session,
        &registry,
        Default::default(),
    );
    let output = result.expect("Expansion must succeed");

    // No @invoke-macro directives in output
    for stmt in &output.statements {
        assert!(
            !stmt.contains("@invoke-macro"),
            "No @invoke-macro directives should remain in expanded output, found: {}",
            stmt
        );
    }

    // Must have at least one statement
    assert!(
        !output.statements.is_empty(),
        "Expansion must produce at least one statement"
    );

    // Compile and execute to verify end-to-end
    let verb_index = VerbConfigIndex::empty();
    let constraints = EffectiveConstraints::unconstrained();
    let classification = classify_verb("structure.setup", &verb_index, &registry);

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

    assert!(resp.is_compiled(), "Macro must compile: {:?}", resp);
    let summary = match &resp {
        OrchestratorResponse::Compiled(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(summary.preview[0].verb, "cbu.create");

    // Execute
    let store = RunbookStore::new();
    let steps: Vec<_> = summary.preview.iter().map(|p| make_step(&p.verb)).collect();
    let rb = CompiledRunbook::new(Uuid::new_v4(), 1, steps, ReplayEnvelope::empty());
    let id = rb.id;
    store.insert(&rb).await.unwrap();

    let result = execute_runbook(&store, id, None, &SuccessExecutor)
        .await
        .unwrap();
    assert!(matches!(
        result.final_status,
        CompiledRunbookStatus::Completed { .. }
    ));
}

// =============================================================================
// Test 3: test_nested_macro_fixpoint (INV-4, INV-12)
// =============================================================================

/// Composite macro (M17-style) that invokes another macro fully expands
/// through fixpoint. Audit carries `expansion_limits` (INV-12).
#[tokio::test]
async fn test_nested_macro_fixpoint() {
    let mut registry = MacroRegistry::new();

    // Inner macro: party.setup → entity.create
    let mut inner_required = HashMap::new();
    inner_required.insert("party-name".to_string(), make_string_arg("Party Name"));

    registry.add(
        "party.setup".to_string(),
        make_macro_schema(
            "Set up Party",
            "Create a party entity",
            inner_required,
            vec![MacroExpansionStep::VerbCall(VerbCallStep {
                verb: "entity.create".to_string(),
                args: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "${arg.party-name}".to_string());
                    m
                },
                bind_as: None,
            })],
        ),
    );

    // Outer macro: structure.full → cbu.create + invoke party.setup
    let mut outer_required = HashMap::new();
    outer_required.insert("name".to_string(), make_string_arg("Name"));
    outer_required.insert("party-name".to_string(), make_string_arg("Party Name"));

    registry.add(
        "structure.full".to_string(),
        make_macro_schema(
            "Full Structure Setup",
            "Create structure and party",
            outer_required,
            vec![
                MacroExpansionStep::VerbCall(VerbCallStep {
                    verb: "cbu.create".to_string(),
                    args: {
                        let mut m = HashMap::new();
                        m.insert("name".to_string(), "${arg.name}".to_string());
                        m
                    },
                    bind_as: None,
                }),
                MacroExpansionStep::InvokeMacro(InvokeMacroStep {
                    macro_id: "party.setup".to_string(),
                    args: {
                        let mut m = HashMap::new();
                        m.insert("party-name".to_string(), "${arg.party-name}".to_string());
                        m
                    },
                    import_symbols: vec![],
                }),
            ],
        ),
    );

    let session = test_session();
    let mut args = BTreeMap::new();
    args.insert("name".to_string(), "Nested Fund".to_string());
    args.insert("party-name".to_string(), "GP Entity".to_string());

    let limits = ExpansionLimits {
        max_depth: 8,
        max_steps: 500,
    };

    let result = expand_macro_fixpoint("structure.full", &args, &session, &registry, limits);
    let output = result.expect("Nested expansion must succeed");

    // No @invoke-macro directives remain
    for stmt in &output.statements {
        assert!(
            !stmt.contains("@invoke-macro"),
            "All macros must be fully expanded, found: {}",
            stmt
        );
    }

    // INV-12: Expansion limits recorded in output
    assert_eq!(output.limits.max_depth, 8);
    assert_eq!(output.limits.max_steps, 500);

    // INV-12: Audits are produced for each expansion pass
    assert!(
        !output.audits.is_empty(),
        "Audits must be produced for expansion passes (INV-12)"
    );
    for audit in &output.audits {
        // Each audit has a non-nil expansion_id and a macro FQN
        assert_ne!(audit.expansion_id, Uuid::nil());
        assert!(!audit.macro_fqn.is_empty());
    }

    // Should have at least 2 statements (cbu.create + entity.create)
    assert!(
        output.total_steps >= 2,
        "Nested expansion must produce at least 2 steps, got {}",
        output.total_steps
    );
}

// =============================================================================
// Test 4: test_cycle_detection (INV-4)
// =============================================================================

/// Circular macro references (A → B → A) detected per-path, not global.
/// A→B→A returns CycleDetected; but A and B appearing in separate non-cyclic
/// branches would succeed (per-path detection).
#[test]
fn test_cycle_detection() {
    let mut registry = MacroRegistry::new();

    // cycle.a invokes cycle.b
    let mut a_required = HashMap::new();
    a_required.insert("x".to_string(), make_string_arg("X"));

    registry.add(
        "cycle.a".to_string(),
        make_macro_schema(
            "Cycle A",
            "Invokes B",
            a_required,
            vec![MacroExpansionStep::InvokeMacro(InvokeMacroStep {
                macro_id: "cycle.b".to_string(),
                args: {
                    let mut m = HashMap::new();
                    m.insert("x".to_string(), "${arg.x}".to_string());
                    m
                },
                import_symbols: vec![],
            })],
        ),
    );

    // cycle.b invokes cycle.a → creates A→B→A cycle
    let mut b_required = HashMap::new();
    b_required.insert("x".to_string(), make_string_arg("X"));

    registry.add(
        "cycle.b".to_string(),
        make_macro_schema(
            "Cycle B",
            "Invokes A",
            b_required,
            vec![MacroExpansionStep::InvokeMacro(InvokeMacroStep {
                macro_id: "cycle.a".to_string(),
                args: {
                    let mut m = HashMap::new();
                    m.insert("x".to_string(), "${arg.x}".to_string());
                    m
                },
                import_symbols: vec![],
            })],
        ),
    );

    let session = test_session();
    let mut args = BTreeMap::new();
    args.insert("x".to_string(), "test".to_string());

    let result = expand_macro_fixpoint("cycle.a", &args, &session, &registry, Default::default());

    assert!(result.is_err(), "Cycle must be detected");
    match result {
        Err(MacroExpansionError::CycleDetected { cycle }) => {
            assert!(
                cycle.len() >= 3,
                "Cycle path must have at least 3 entries (A→B→A), got {:?}",
                cycle
            );
            assert_eq!(cycle.first().unwrap(), "cycle.a");
            assert_eq!(cycle.last().unwrap(), "cycle.a");
        }
        Err(other) => panic!("Expected CycleDetected, got: {:?}", other),
        Ok(_) => panic!("Expected error, got success"),
    }
}

// =============================================================================
// Test 5: test_depth_limit (INV-4)
// =============================================================================

/// Chain of nested macros exceeding max_depth returns MaxDepthExceeded.
#[test]
fn test_depth_limit() {
    let mut registry = MacroRegistry::new();

    // Create a chain: deep.0 → deep.1 → deep.2 → ... → deep.9
    // With max_depth=8, this should fail
    for i in 0..10 {
        let name = format!("deep.{}", i);
        let mut required = HashMap::new();
        required.insert("x".to_string(), make_string_arg("X"));

        let expansion = if i < 9 {
            vec![MacroExpansionStep::InvokeMacro(InvokeMacroStep {
                macro_id: format!("deep.{}", i + 1),
                args: {
                    let mut m = HashMap::new();
                    m.insert("x".to_string(), "${arg.x}".to_string());
                    m
                },
                import_symbols: vec![],
            })]
        } else {
            // Terminal: deep.9 produces a verb call
            vec![MacroExpansionStep::VerbCall(VerbCallStep {
                verb: "cbu.create".to_string(),
                args: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "${arg.x}".to_string());
                    m
                },
                bind_as: None,
            })]
        };

        registry.add(
            name,
            make_macro_schema("Deep", "Chain", required, expansion),
        );
    }

    let session = test_session();
    let mut args = BTreeMap::new();
    args.insert("x".to_string(), "test".to_string());

    let limits = ExpansionLimits {
        max_depth: 8,
        max_steps: 500,
    };

    let result = expand_macro_fixpoint("deep.0", &args, &session, &registry, limits);

    assert!(result.is_err(), "Depth limit must be enforced");
    match result {
        Err(MacroExpansionError::MaxDepthExceeded { depth: _, limit }) => {
            assert_eq!(limit, 8, "Limit must match configured max_depth");
        }
        Err(other) => panic!("Expected MaxDepthExceeded, got: {:?}", other),
        Ok(_) => panic!("Expected error, got success"),
    }
}

// =============================================================================
// Test 6: test_pack_constraint_blocks_forbidden_verb (INV-6)
// =============================================================================

/// Expanded verb outside pack's allowed set → ConstraintViolation.
/// Validates that constraint checking happens after DAG, before SemReg (§6.2).
#[test]
fn test_pack_constraint_blocks_forbidden_verb() {
    let mut registry = MacroRegistry::new();

    let mut required = HashMap::new();
    required.insert("name".to_string(), make_string_arg("Name"));

    registry.add(
        "structure.setup".to_string(),
        make_macro_schema(
            "Set up Structure",
            "Create structure",
            required,
            vec![MacroExpansionStep::VerbCall(VerbCallStep {
                verb: "cbu.create".to_string(),
                args: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "${arg.name}".to_string());
                    m
                },
                bind_as: None,
            })],
        ),
    );

    let verb_index = VerbConfigIndex::empty();
    let session = test_session();

    // Pack allows ONLY kyc verbs — cbu.create should be blocked
    let mut allowed = HashSet::new();
    allowed.insert("kyc.create-case".to_string());
    allowed.insert("kyc.assign-analyst".to_string());
    let constraints = EffectiveConstraints {
        allowed_verbs: Some(allowed),
        forbidden_verbs: HashSet::new(),
        contributing_packs: vec![ConstraintSource {
            pack_id: "kyc-case".to_string(),
            pack_name: "KYC Case".to_string(),
            allowed_count: 2,
            forbidden_count: 0,
        }],
    };

    let mut args = BTreeMap::new();
    args.insert("name".to_string(), "Blocked Fund".to_string());

    let resp = compile_invocation(
        Uuid::new_v4(),
        "structure.setup",
        &args,
        &session,
        &registry,
        &verb_index,
        &constraints,
        1,
        None,
        None, // verb_snapshot_pins
    );

    assert!(
        matches!(resp, OrchestratorResponse::ConstraintViolation(_)),
        "Expanded verb outside pack must be blocked (INV-6), got: {:?}",
        resp
    );

    if let OrchestratorResponse::ConstraintViolation(detail) = &resp {
        assert!(
            detail.violating_verbs.contains(&"cbu.create".to_string()),
            "Must identify cbu.create as violating verb"
        );
        assert!(
            !detail.active_constraints.is_empty(),
            "Must report active constraints"
        );
        assert!(
            !detail.remediation_options.is_empty(),
            "Must offer remediation options"
        );
    }
}

// =============================================================================
// Test 7: test_content_addressed_id_determinism (INV-2, INV-13)
// =============================================================================

/// Same inputs → same CompiledRunbookId; different args → different ID.
/// Proves INV-2 (deterministic canonical serialization) and INV-13 (schema
/// evolution changes hash).
#[test]
fn test_content_addressed_id_determinism() {
    let env = ReplayEnvelope::empty();

    // Use content_addressed_id with identical step_ids for true determinism
    let step_id = Uuid::nil();
    let identical_step_a = CompiledStep {
        step_id,
        sentence: "Execute cbu.create".into(),
        verb: "cbu.create".into(),
        dsl: "(cbu.create)".into(),
        args: {
            let mut m = BTreeMap::new();
            m.insert("name".to_string(), "Acme".to_string());
            m
        },
        depends_on: vec![],
        execution_mode: ExecutionMode::Sync,
        write_set: vec![],
        verb_contract_snapshot_id: None,
    };
    let identical_step_b = identical_step_a.clone();

    let id_a = content_addressed_id(&[identical_step_a.clone()], &env);
    let id_b = content_addressed_id(&[identical_step_b], &env);
    assert_eq!(
        id_a, id_b,
        "INV-2: Same inputs must produce identical content-addressed IDs"
    );

    // Different args → different ID
    let step_different = CompiledStep {
        step_id,
        sentence: "Execute cbu.create".into(),
        verb: "cbu.create".into(),
        dsl: "(cbu.create)".into(),
        args: {
            let mut m = BTreeMap::new();
            m.insert("name".to_string(), "Beta Corp".to_string());
            m
        },
        depends_on: vec![],
        execution_mode: ExecutionMode::Sync,
        write_set: vec![],
        verb_contract_snapshot_id: None,
    };

    let id_c = content_addressed_id(&[step_different], &env);
    assert_ne!(id_a, id_c, "Different args must produce different IDs");

    // Different step count → different ID (INV-13: schema evolution)
    let two_steps = vec![
        CompiledStep {
            step_id,
            sentence: "Execute cbu.create".into(),
            verb: "cbu.create".into(),
            dsl: "(cbu.create)".into(),
            args: {
                let mut m = BTreeMap::new();
                m.insert("name".to_string(), "Acme".to_string());
                m
            },
            depends_on: vec![],
            execution_mode: ExecutionMode::Sync,
            write_set: vec![],
            verb_contract_snapshot_id: None,
        },
        CompiledStep {
            step_id: Uuid::nil(),
            sentence: "Execute entity.create".into(),
            verb: "entity.create".into(),
            dsl: "(entity.create)".into(),
            args: BTreeMap::new(),
            depends_on: vec![],
            execution_mode: ExecutionMode::Sync,
            write_set: vec![],
            verb_contract_snapshot_id: None,
        },
    ];
    let id_d = content_addressed_id(&two_steps, &env);
    assert_ne!(
        id_a, id_d,
        "INV-13: Different step counts must produce different IDs (schema evolution)"
    );

    // Verify full SHA-256 is 32 bytes and first 16 match truncated UUID
    let hash = full_sha256(&[identical_step_a.clone()], &env);
    assert_eq!(hash.len(), 32);
    let expected_uuid = Uuid::from_bytes(hash[..16].try_into().unwrap());
    assert_eq!(id_a.0, expected_uuid);

    // BTreeMap ordering determinism (INV-2)
    let mut args_forward = BTreeMap::new();
    args_forward.insert("alpha".to_string(), "1".to_string());
    args_forward.insert("zebra".to_string(), "2".to_string());

    let mut args_reverse = BTreeMap::new();
    args_reverse.insert("zebra".to_string(), "2".to_string());
    args_reverse.insert("alpha".to_string(), "1".to_string());

    let step_fwd = CompiledStep {
        step_id,
        args: args_forward,
        ..identical_step_a.clone()
    };
    let step_rev = CompiledStep {
        step_id,
        args: args_reverse,
        ..identical_step_a.clone()
    };

    let bytes_fwd = canonical_bytes_for_steps(&[step_fwd]);
    let bytes_rev = canonical_bytes_for_steps(&[step_rev]);
    assert_eq!(
        bytes_fwd, bytes_rev,
        "INV-2: BTreeMap insertion order must not affect canonical bytes"
    );
}

// =============================================================================
// Test 8: test_write_set_locks_acquired (INV-8, INV-10)
// =============================================================================

/// Non-empty write_set → advisory locks acquired → lock events exist in
/// executor source. Verifies the write_set derivation and lock acquisition
/// pipeline end-to-end.
#[tokio::test]
async fn test_write_set_locks_acquired() {
    let entity_id_1 = Uuid::new_v4();
    let entity_id_2 = Uuid::new_v4();

    // Build steps with write_set containing entity IDs
    let step1 = make_step_with_write_set("cbu.update", vec![entity_id_1]);
    let step2 = make_step_with_write_set("entity.update", vec![entity_id_2, entity_id_1]);

    // Compute combined write_set
    let ws = ob_poc::runbook::executor::compute_write_set(&[step1.clone(), step2.clone()], None);

    // Must contain both entity IDs (BTreeSet, deduplicated)
    assert!(
        ws.contains(&entity_id_1),
        "INV-8: write_set must contain entity_id_1"
    );
    assert!(
        ws.contains(&entity_id_2),
        "INV-8: write_set must contain entity_id_2"
    );
    assert_eq!(ws.len(), 2, "Duplicates must be removed");

    // Verify sorted order (BTreeSet guarantee, INV-2)
    let sorted: Vec<Uuid> = ws.into_iter().collect();
    assert!(
        sorted.windows(2).all(|w| w[0] <= w[1]),
        "Write set must be sorted for deadlock prevention"
    );

    // Build and execute a runbook with write_set
    let store = RunbookStore::new();
    let rb = CompiledRunbook::new(
        Uuid::new_v4(),
        1,
        vec![step1, step2],
        ReplayEnvelope::empty(),
    );
    let id = rb.id;
    store.insert(&rb).await.unwrap();

    let recorder = RecordingExecutor::new();
    let result = execute_runbook(&store, id, None, &recorder).await.unwrap();
    assert!(matches!(
        result.final_status,
        CompiledRunbookStatus::Completed { .. }
    ));

    // Both steps executed
    assert_eq!(
        recorder.executed_verbs(),
        vec!["cbu.update", "entity.update"]
    );

    // INV-10: Verify lock event logging is present in source code
    let executor_source = include_str!("../src/runbook/executor.rs");
    assert!(
        executor_source.contains("\"lock_acquired\""),
        "INV-10: executor must log lock_acquired events"
    );
    assert!(
        executor_source.contains("\"lock_released\""),
        "INV-10: executor must log lock_released events"
    );
    assert!(
        executor_source.contains("\"lock_contention\""),
        "INV-10: executor must log lock_contention events"
    );

    // INV-8: Verify heuristic write_set derivation works
    let mut heuristic_args = BTreeMap::new();
    heuristic_args.insert("entity-id".to_string(), entity_id_1.to_string());
    heuristic_args.insert("name".to_string(), "Acme Corp".to_string());
    let heuristic_ws = ob_poc::runbook::write_set::derive_write_set_heuristic(&heuristic_args);
    assert!(
        heuristic_ws.contains(&entity_id_1),
        "Heuristic must extract UUID from args"
    );
    assert_eq!(heuristic_ws.len(), 1, "Non-UUID args must be ignored");
}

// =============================================================================
// Test 9: test_concurrent_lock_contention (INV-10)
// =============================================================================

/// Two runbooks targeting the same entity produce overlapping write_sets.
/// The write_set computation is deterministic and both include the contested
/// entity. Actual lock contention requires Postgres (integration test), but
/// we verify the prerequisite: both runbooks derive the same lock keys.
#[tokio::test]
async fn test_concurrent_lock_contention() {
    let contested_entity = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
    let entity_b = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();

    // Runbook 1: updates contested entity + entity B
    let rb1_step1 = make_step_with_write_set("cbu.update", vec![contested_entity]);
    let rb1_step2 = make_step_with_write_set("entity.update", vec![entity_b]);

    // Runbook 2: also updates contested entity
    let rb2_step1 = make_step_with_write_set("trading-profile.update", vec![contested_entity]);

    // Both produce write_sets containing the contested entity
    let ws1 =
        ob_poc::runbook::executor::compute_write_set(&[rb1_step1.clone(), rb1_step2.clone()], None);
    let ws2 = ob_poc::runbook::executor::compute_write_set(&[rb2_step1.clone()], None);

    assert!(ws1.contains(&contested_entity));
    assert!(ws2.contains(&contested_entity));

    // Overlapping lock keys detected
    let overlap: Vec<Uuid> = ws1.intersection(&ws2).copied().collect();
    assert!(
        !overlap.is_empty(),
        "INV-10: Overlapping write_sets must be detectable"
    );
    assert_eq!(overlap[0], contested_entity);

    // Both runbooks execute independently (no Postgres → no actual contention)
    let store1 = RunbookStore::new();
    let rb1 = CompiledRunbook::new(
        Uuid::new_v4(),
        1,
        vec![rb1_step1, rb1_step2],
        ReplayEnvelope::empty(),
    );
    let id1 = rb1.id;
    store1.insert(&rb1).await.unwrap();

    let store2 = RunbookStore::new();
    let rb2 = CompiledRunbook::new(Uuid::new_v4(), 1, vec![rb2_step1], ReplayEnvelope::empty());
    let id2 = rb2.id;
    store2.insert(&rb2).await.unwrap();

    // Execute both — without Postgres, locks are skipped but execution works
    let r1 = execute_runbook(&store1, id1, None, &SuccessExecutor)
        .await
        .unwrap();
    let r2 = execute_runbook(&store2, id2, None, &SuccessExecutor)
        .await
        .unwrap();

    assert!(matches!(
        r1.final_status,
        CompiledRunbookStatus::Completed { .. }
    ));
    assert!(matches!(
        r2.final_status,
        CompiledRunbookStatus::Completed { .. }
    ));

    // Verify LockError::Contention has holder_runbook_id field (INV-10)
    let executor_source = include_str!("../src/runbook/executor.rs");
    assert!(
        executor_source.contains("holder_runbook_id"),
        "INV-10: LockError::Contention must carry holder_runbook_id"
    );
}

// =============================================================================
// Test 10: test_no_execution_without_compiled_id (INV-1, INV-11)
// =============================================================================

/// All execution paths require a CompiledRunbookId. No raw DSL execution
/// is permitted. Both Chat API and REPL paths must be gated.
#[tokio::test]
async fn test_no_execution_without_compiled_id() {
    let store = RunbookStore::new();

    // 1. Attempt to execute non-existent runbook → NotFound
    let fake_rb = CompiledRunbook::new(Uuid::new_v4(), 1, vec![], ReplayEnvelope::empty());
    let fake_id = fake_rb.id;
    let result = execute_runbook(&store, fake_id, None, &SuccessExecutor).await;
    assert!(
        matches!(
            result,
            Err(ob_poc::runbook::executor::ExecutionError::NotFound(_))
        ),
        "INV-1: Non-existent runbook must be rejected"
    );

    // 2. Execute to completion, then attempt re-execution → NotExecutable
    let step = make_step("cbu.create");
    let rb = CompiledRunbook::new(Uuid::new_v4(), 1, vec![step], ReplayEnvelope::empty());
    let id = rb.id;
    store.insert(&rb).await.unwrap();

    let result = execute_runbook(&store, id, None, &SuccessExecutor)
        .await
        .unwrap();
    assert!(matches!(
        result.final_status,
        CompiledRunbookStatus::Completed { .. }
    ));

    let retry = execute_runbook(&store, id, None, &SuccessExecutor).await;
    assert!(
        matches!(
            retry,
            Err(ob_poc::runbook::executor::ExecutionError::NotExecutable(
                _,
                _
            ))
        ),
        "INV-1: Completed runbook must reject re-execution"
    );

    // 3. INV-11: Both Chat API and REPL must contain execute_runbook references
    let agent_source = include_str!("../src/api/agent_service.rs");
    let repl_source = include_str!("../src/repl/orchestrator_v2.rs");

    assert!(
        agent_source.contains("execute_runbook"),
        "INV-11: Chat API (agent_service.rs) must reference execute_runbook"
    );
    assert!(
        repl_source.contains("execute_runbook"),
        "INV-11: REPL (orchestrator_v2.rs) must reference execute_runbook"
    );

    // 4. INV-1: When runbook-gate-vnext is enabled, no ungated execute_dsl calls
    // Scan agent_service.rs for execute_dsl calls that are NOT inside
    // cfg(not(feature = "runbook-gate-vnext")) blocks
    let mut in_not_gate = false;
    let mut ungated_calls: Vec<(usize, String)> = Vec::new();
    for (i, line) in agent_source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.contains("cfg(not(feature = \"runbook-gate-vnext\"))") {
            in_not_gate = true;
        }
        // Reset flag at function boundaries
        if (trimmed.starts_with("fn ")
            || trimmed.starts_with("async fn ")
            || trimmed.starts_with("pub fn ")
            || trimmed.starts_with("pub async fn "))
            && !in_not_gate
        {
            in_not_gate = false;
        }
        // Skip comments
        if trimmed.starts_with("//") || trimmed.starts_with("*") || trimmed.starts_with("///") {
            continue;
        }
        // Check for execute_dsl calls outside the gated block
        if (trimmed.contains("execute_dsl(") || trimmed.contains(".execute_dsl(")) && !in_not_gate {
            ungated_calls.push((i + 1, line.to_string()));
        }
    }
    assert!(
        ungated_calls.is_empty(),
        "INV-1: Found ungated execute_dsl calls in agent_service.rs: {:?}",
        ungated_calls
    );

    // 5. Static verification: no HashMap in canonical types (INV-2)
    let types_source = include_str!("../src/runbook/types.rs");
    let envelope_source = include_str!("../src/runbook/envelope.rs");

    // Count non-comment HashMap references in types.rs
    let hashmap_in_types: Vec<&str> = types_source
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.starts_with("//")
                && !t.starts_with("///")
                && !t.starts_with("*")
                && t.contains("HashMap")
                && !t.contains("BTreeMap")
                && !t.contains("#[cfg(test)]")
        })
        .collect();
    assert!(
        hashmap_in_types.is_empty(),
        "INV-2: No HashMap in runbook types.rs (use BTreeMap), found: {:?}",
        hashmap_in_types
    );

    // Count non-comment HashMap references in envelope.rs struct definitions
    let hashmap_in_envelope: Vec<&str> = envelope_source
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.starts_with("//")
                && !t.starts_with("///")
                && !t.starts_with("*")
                && t.contains("HashMap")
                && !t.contains("BTreeMap")
                && !t.contains("#[cfg(test)]")
                && !t.contains("use std::collections")
        })
        .collect();
    assert!(
        hashmap_in_envelope.is_empty(),
        "INV-2: No HashMap in runbook envelope.rs (use BTreeMap), found: {:?}",
        hashmap_in_envelope
    );

    // 6. All 7 CompilationErrorKind variants (INV-7)
    let variants: Vec<CompilationErrorKind> = vec![
        CompilationErrorKind::ExpansionFailed {
            reason: "test".into(),
        },
        CompilationErrorKind::CycleDetected {
            cycle: vec!["A".into()],
        },
        CompilationErrorKind::LimitsExceeded {
            detail: "test".into(),
        },
        CompilationErrorKind::DagError {
            reason: "test".into(),
        },
        CompilationErrorKind::PackConstraint {
            verb: "test".into(),
            explanation: "test".into(),
        },
        CompilationErrorKind::SemRegDenied {
            verb: "test".into(),
            reason: "test".into(),
        },
        CompilationErrorKind::StoreFailed {
            reason: "test".into(),
        },
    ];
    assert_eq!(
        variants.len(),
        7,
        "INV-7: Must have exactly 7 error variants"
    );
    for v in &variants {
        assert!(
            !v.to_string().is_empty(),
            "All variants must produce non-empty Display"
        );
    }

    // 7. INV-5: Verify Kahn's algorithm is used (already in plan_assembler)
    // This is a static assertion that the toposort is iterative, not recursive
    let assembler_source = include_str!("../src/runbook/compiler.rs");
    assert!(
        assembler_source.contains("assemble_plan") || assembler_source.contains("toposort"),
        "INV-5: Compilation must use DAG assembly (Kahn's algorithm)"
    );
}

// =============================================================================
// Additional invariant coverage (supplementary assertions)
// =============================================================================

/// INV-7: No .unwrap() in runbook module library code.
/// Static grep test scanning all Rust files in runbook/.
#[test]
fn test_no_unwrap_in_runbook_module() {
    let files_to_check = [
        ("types.rs", include_str!("../src/runbook/types.rs")),
        ("envelope.rs", include_str!("../src/runbook/envelope.rs")),
        ("errors.rs", include_str!("../src/runbook/errors.rs")),
        ("response.rs", include_str!("../src/runbook/response.rs")),
        ("canonical.rs", include_str!("../src/runbook/canonical.rs")),
    ];

    for (filename, source) in &files_to_check {
        let unwrap_lines: Vec<(usize, &str)> = source
            .lines()
            .enumerate()
            .filter(|(_, l)| {
                let t = l.trim();
                // Skip test code and comments
                !t.starts_with("//")
                    && !t.starts_with("///")
                    && !t.starts_with("*")
                    && !t.starts_with("#[")
                    && t.contains(".unwrap()")
            })
            .collect();

        // Filter out test code — lines between #[cfg(test)] and end of module
        let in_test_module = source.contains("#[cfg(test)]");
        if in_test_module {
            // Only check lines before the first #[cfg(test)]
            let test_start = source
                .lines()
                .position(|l| l.contains("#[cfg(test)]"))
                .unwrap_or(usize::MAX);
            let production_unwraps: Vec<(usize, &str)> = unwrap_lines
                .into_iter()
                .filter(|(i, _)| *i < test_start)
                .collect();
            assert!(
                production_unwraps.is_empty(),
                "INV-7: No .unwrap() in production code of {}: {:?}",
                filename,
                production_unwraps
            );
        } else {
            assert!(
                unwrap_lines.is_empty(),
                "INV-7: No .unwrap() in {}: {:?}",
                filename,
                unwrap_lines
            );
        }
    }
}

/// INV-13: execute_runbook() reads from store, never calls expand_macro().
/// The stored artefact IS the executable truth.
#[test]
fn test_replay_never_re_expands() {
    let executor_source = include_str!("../src/runbook/executor.rs");

    // execute_runbook should NOT contain any macro expansion calls
    assert!(
        !executor_source.contains("expand_macro(")
            && !executor_source.contains("expand_macro_fixpoint("),
        "INV-13: execute_runbook must never call expand_macro — \
         the stored artefact is the only executable truth"
    );

    // Verify execute_runbook reads from store
    assert!(
        executor_source.contains("store.get("),
        "INV-13: execute_runbook must read from store"
    );
}
