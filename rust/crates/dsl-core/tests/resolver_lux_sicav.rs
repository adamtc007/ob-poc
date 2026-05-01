use dsl_core::{
    config::dag::{ClosureType, EligibilityConstraint},
    resolver::{resolve_template, ResolvedSource, ResolverInputs},
};
use sem_os_core::constellation_map_def as core_map;
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
    assert!(template.transitions.iter().any(|transition| {
        transition.slot_id == "cbu"
            && transition.from == "VALIDATION_PENDING"
            && transition.to == "VALIDATED"
            && transition.via.as_deref() == Some("cbu.decide")
            && transition
                .destination_green_when
                .as_deref()
                .is_some_and(|predicate| predicate.contains("cbu_evidence.state = APPROVED"))
    }));
    assert!(template.transitions.iter().any(|transition| {
        transition.slot_id == "cbu_evidence"
            && transition.from == "UPLOADED"
            && transition.to == "REVIEWED"
            && transition.via.as_deref() == Some("cbu.review-evidence")
    }));

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
fn resolver_constellation_gate_metadata_beats_dag_taxonomy() {
    let mut inputs = inputs();
    inputs
        .constellation_maps
        .get_mut("struct.lux.ucits.sicav")
        .expect("Lux SICAV map")
        .body
        .slots
        .get_mut("cbu")
        .expect("cbu slot")
        .closure = Some(core_map::ClosureType::Open);

    let template = resolve_template("struct.lux.ucits.sicav", "cbu", &inputs)
        .expect("Lux SICAV template resolves");
    let cbu = template.slot("cbu").expect("cbu resolved");

    assert_eq!(cbu.closure, Some(ClosureType::Open));
    assert_eq!(
        cbu.provenance.field_sources.get("closure"),
        Some(&ResolvedSource::ConstellationMap)
    );
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
