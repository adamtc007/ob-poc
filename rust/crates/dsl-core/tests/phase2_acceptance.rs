use dsl_core::{
    config::{validate_resolved_template_gate_metadata, DagValidationContext},
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
fn authored_shape_rules_pass_resolved_template_gate_metadata_lints() {
    let inputs = inputs();
    let context = DagValidationContext {
        known_entity_kinds: HashSet::from([
            "cbu".to_string(),
            "company".to_string(),
            "limited_company".to_string(),
            "proper_person".to_string(),
        ]),
    };

    let mut checked = Vec::new();
    for shape in inputs.shape_rules.keys() {
        if !inputs.constellation_maps.contains_key(shape) {
            continue;
        }
        let template = resolve_template(shape, "cbu", &inputs)
            .unwrap_or_else(|err| panic!("{shape} should resolve: {err}"));
        let report = validate_resolved_template_gate_metadata(&template, &context);
        assert!(
            report.errors.is_empty(),
            "{shape} resolved gate metadata should pass lints: {:#?}",
            report.errors
        );
        checked.push(shape.clone());
    }

    assert_eq!(
        checked.len(),
        18,
        "expected all authored leaf shape rules with constellation maps to be linted: {checked:?}"
    );
}
