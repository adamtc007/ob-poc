use dsl_core::{
    config::dag::EligibilityConstraint,
    config::{validate_resolved_template_gate_metadata, DagError, DagValidationContext},
    resolver::{resolve_template, ResolverInputs},
};
use std::{collections::HashSet, path::PathBuf};

fn inputs() -> ResolverInputs {
    ResolverInputs::from_workspace_config_dir(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config"),
    )
    .expect("resolver inputs load")
}

#[test]
fn eligibility_lint_rejects_unknown_entity_kind_after_shape_rule_composition() {
    let inputs = inputs();
    let mut template = resolve_template("struct.lux.ucits.sicav", "cbu", &inputs)
        .expect("Lux SICAV template resolves");
    let slot = template
        .slot_mut("management_company")
        .expect("management_company slot resolved");
    slot.eligibility = Some(EligibilityConstraint::EntityKinds {
        entity_kinds: vec!["company".to_string(), "invented_kind".to_string()],
    });

    let context = DagValidationContext {
        known_entity_kinds: HashSet::from(["company".to_string()]),
    };
    let report = validate_resolved_template_gate_metadata(&template, &context);

    assert!(report.errors.iter().any(|error| matches!(
        error,
        DagError::EligibilityEntityKindUnknown {
            slot_id,
            entity_kind,
            ..
        } if slot_id == "management_company" && entity_kind == "invented_kind"
    )));
}

#[test]
fn eligibility_lint_accepts_known_entity_kind_after_shape_rule_composition() {
    let inputs = inputs();
    let template = resolve_template("struct.lux.ucits.sicav", "cbu", &inputs)
        .expect("Lux SICAV template resolves");
    let context = DagValidationContext {
        known_entity_kinds: HashSet::from([
            "company".to_string(),
            "cbu".to_string(),
            "person".to_string(),
        ]),
    };
    let report = validate_resolved_template_gate_metadata(&template, &context);

    assert!(
        report
            .errors
            .iter()
            .all(|error| !matches!(error, DagError::EligibilityEntityKindUnknown { .. })),
        "{:#?}",
        report.errors
    );
}
