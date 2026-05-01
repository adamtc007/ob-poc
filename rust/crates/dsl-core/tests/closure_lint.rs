use dsl_core::{
    config::{validate_resolved_template_gate_metadata, DagError, DagValidationContext},
    resolver::{resolve_template, ResolverInputs},
};
use std::path::PathBuf;

fn inputs() -> ResolverInputs {
    ResolverInputs::from_workspace_config_dir(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config"),
    )
    .expect("resolver inputs load")
}

#[test]
fn closure_lint_rejects_universal_quantifier_over_closed_unbounded_slot() {
    let inputs = inputs();
    let mut template = resolve_template("struct.lux.ucits.sicav", "cbu", &inputs)
        .expect("Lux SICAV template resolves");
    let cbu = template.slot_mut("cbu").expect("cbu slot resolved");
    cbu.attachment_predicates
        .push("every investment_manager.state = APPROVED".to_string());

    let report =
        validate_resolved_template_gate_metadata(&template, &DagValidationContext::default());

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

    let report =
        validate_resolved_template_gate_metadata(&template, &DagValidationContext::default());

    assert!(
        report.errors.iter().all(|error| !matches!(
            error,
            DagError::ResolvedClosureUniversalQuantifierInvalid { .. }
        )),
        "{:#?}",
        report.errors
    );
}
