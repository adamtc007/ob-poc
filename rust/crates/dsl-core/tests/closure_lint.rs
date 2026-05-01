use dsl_core::{
    config::{
        dag::PredicateBinding, validate_resolved_template_gate_metadata, DagError,
        DagValidationContext,
    },
    resolver::{resolve_template, ResolverInputs},
};
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

fn inputs() -> ResolverInputs {
    ResolverInputs::from_workspace_config_dir(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config"),
    )
    .expect("resolver inputs load")
}

fn validation_context() -> DagValidationContext {
    DagValidationContext {
        known_entity_kinds: HashSet::from([
            "company".to_string(),
            "cbu".to_string(),
            "person".to_string(),
        ]),
    }
}

#[test]
fn closure_lint_rejects_universal_quantifier_over_closed_unbounded_slot() {
    let inputs = inputs();
    let mut template = resolve_template("struct.lux.ucits.sicav", "cbu", &inputs)
        .expect("Lux SICAV template resolves");
    let cbu = template.slot_mut("cbu").expect("cbu slot resolved");
    cbu.attachment_predicates
        .push("every investment_manager.state = APPROVED".to_string());

    let report = validate_resolved_template_gate_metadata(&template, &validation_context());

    assert!(report.errors.iter().any(|error| matches!(
        error,
        DagError::ResolvedClosureUniversalQuantifierInvalid {
            slot_id,
            field,
            quantified_slot,
            ..
        } if slot_id == "cbu"
            && field == "attachment_predicates"
            && quantified_slot == "investment_manager"
    )));
}

#[test]
fn closure_lint_allows_aggregate_count_over_closed_unbounded_slot() {
    let inputs = inputs();
    let mut template = resolve_template("struct.lux.ucits.sicav", "cbu", &inputs)
        .expect("Lux SICAV template resolves");
    let cbu = template.slot_mut("cbu").expect("cbu slot resolved");
    cbu.attachment_predicates
        .push("at least one investment_manager.state = APPROVED".to_string());

    let report = validate_resolved_template_gate_metadata(&template, &validation_context());

    assert!(
        report.errors.iter().all(|error| !matches!(
            error,
            DagError::ResolvedClosureUniversalQuantifierInvalid { .. }
        )),
        "{:#?}",
        report.errors
    );
}

#[test]
fn closure_lint_uses_predicate_binding_entity_aliases() {
    let inputs = inputs();
    let mut template = resolve_template("struct.lux.ucits.sicav", "cbu", &inputs)
        .expect("Lux SICAV template resolves");
    let investment_manager = template
        .slot_mut("investment_manager")
        .expect("investment_manager slot resolved");
    investment_manager.id = "investment_manager_slot".to_string();
    investment_manager
        .predicate_bindings
        .push(PredicateBinding {
            entity: "investment_manager".to_string(),
            source_kind: Default::default(),
            source_entity: None,
            state_column: None,
            value_column: None,
            id_column: None,
            scope: None,
            parent_key: None,
            child_key: None,
            required_universe: None,
            replaceable_by_shape: false,
            extra: BTreeMap::new(),
        });
    let cbu = template.slot_mut("cbu").expect("cbu slot resolved");
    cbu.attachment_predicates
        .push("every investment_manager.state = APPROVED".to_string());

    let report = validate_resolved_template_gate_metadata(&template, &validation_context());

    assert!(report.errors.iter().any(|error| matches!(
        error,
        DagError::ResolvedClosureUniversalQuantifierInvalid {
            slot_id,
            quantified_slot,
            ..
        } if slot_id == "cbu" && quantified_slot == "investment_manager"
    )));
}
