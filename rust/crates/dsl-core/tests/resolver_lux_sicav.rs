use dsl_core::{
    config::dag::{ClosureType, EligibilityConstraint},
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
fn resolver_lux_sicav_composes_pilot_template() {
    let inputs = inputs();
    let template = resolve_template("struct.lux.ucits.sicav", "cbu", &inputs)
        .expect("Lux SICAV template resolves");

    assert_eq!(template.workspace, "cbu");
    assert_eq!(template.composite_shape, "struct.lux.ucits.sicav");
    assert!(template.slot("cbu").is_some());
    assert!(template.slot("administrator").is_some());
    assert!(template.slot("auditor").is_some());
    assert!(template.slot("domiciliation_agent").is_none());
    assert!(!template.transitions.is_empty());

    let management_company = template
        .slot("management_company")
        .expect("management company resolved");
    assert_eq!(management_company.closure, Some(ClosureType::ClosedBounded));
    assert_eq!(management_company.cardinality_max, Some(1));
    assert_eq!(
        management_company.eligibility,
        Some(EligibilityConstraint::EntityKinds {
            entity_kinds: vec!["company".to_string()]
        })
    );

    let cbu = template.slot("cbu").expect("cbu resolved");
    assert_eq!(cbu.closure, Some(ClosureType::ClosedBounded));
    assert_eq!(cbu.cardinality_max, Some(1));
    assert_eq!(cbu.entry_state.as_deref(), Some("DISCOVERED"));
}

#[test]
fn resolver_lux_sicav_provenance_preserves_legacy_constellation_stack() {
    let inputs = inputs();
    let template = resolve_template("struct.lux.ucits.sicav", "cbu", &inputs)
        .expect("Lux SICAV template resolves");
    let stack = template
        .generated_from
        .legacy_constellation_stack
        .iter()
        .map(|map| map.constellation.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        stack,
        vec![
            "group.ownership",
            "struct.lux.ucits.sicav",
            "kyc.onboarding"
        ]
    );
}
