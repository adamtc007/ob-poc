use dsl_core::{
    config::dag::{ClosureType, EligibilityConstraint, PredicateBinding},
    resolver::{
        resolve_template, InsertBetween, ResolveError, ResolvedSource, ResolverInputs,
        SlotGateMetadataRefinement,
    },
};
use std::path::PathBuf;

fn inputs() -> ResolverInputs {
    ResolverInputs::from_workspace_config_dir(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config"),
    )
    .expect("resolver inputs load")
}

fn predicate_binding(
    entity: &str,
    source_entity: Option<&str>,
    replaceable: bool,
) -> PredicateBinding {
    PredicateBinding {
        entity: entity.to_string(),
        source_kind: Default::default(),
        source_entity: source_entity.map(str::to_string),
        state_column: None,
        value_column: None,
        id_column: None,
        scope: None,
        parent_key: None,
        child_key: None,
        required_universe: None,
        replaceable_by_shape: replaceable,
        extra: Default::default(),
    }
}

#[test]
fn shape_rule_composition_applies_leaf_gate_metadata() {
    let inputs = inputs();
    let template = resolve_template("struct.lux.ucits.sicav", "cbu", &inputs)
        .expect("Lux SICAV template resolves");

    assert_eq!(
        template.generated_from.shape_rule_paths.len(),
        4,
        "base, regulated, ucits, and leaf shape rules should compose"
    );
    assert_eq!(
        template.structural_facts.jurisdiction.as_deref(),
        Some("LU")
    );
    assert_eq!(
        template.structural_facts.structure_type.as_deref(),
        Some("ucits")
    );
    assert_eq!(
        template.structural_facts.allowed_structure_types,
        vec!["sicav"]
    );
    assert_eq!(
        template.structural_facts.document_bundles,
        vec!["docs.bundle.ucits-baseline"]
    );
    assert_eq!(
        template.structural_facts.trading_profile_type.as_deref(),
        Some("ucits")
    );
    assert_eq!(
        template.structural_facts.required_roles,
        vec!["management-company", "depositary"]
    );
    assert_eq!(
        template.structural_facts.optional_roles,
        vec!["investment-manager", "administrator", "auditor"]
    );
    assert_eq!(
        template.structural_facts.deferred_roles,
        vec!["domiciliation-agent"]
    );

    let management_company = template
        .slot("management_company")
        .expect("management_company resolved");
    assert_eq!(management_company.closure, Some(ClosureType::ClosedBounded));
    assert_eq!(
        management_company.eligibility,
        Some(EligibilityConstraint::EntityKinds {
            entity_kinds: vec!["company".to_string()]
        })
    );
    assert_eq!(
        management_company.provenance.field_sources.get("closure"),
        Some(&ResolvedSource::ShapeRule)
    );
}

#[test]
fn shape_rule_composition_rejects_mixed_vector_replacement_and_additive() {
    let mut inputs = inputs();
    let rule = inputs
        .shape_rules
        .get_mut("struct.lux.ucits.sicav")
        .expect("leaf shape rule loaded");
    let slot = rule
        .body
        .slots
        .get_mut("management_company")
        .expect("slot refinement loaded");
    slot.attachment_predicates = vec!["base_predicate".to_string()];
    slot.additive_attachment_predicates = vec!["extra_predicate".to_string()];

    let err = resolve_template("struct.lux.ucits.sicav", "cbu", &inputs)
        .expect_err("mixed vector composition should fail");
    assert!(matches!(
        err,
        ResolveError::AmbiguousVectorComposition {
            ref slot,
            ref field,
            ref shape
        } if slot == "management_company"
            && field == "attachment_predicates"
            && shape == "struct.lux.ucits.sicav"
    ));
}

#[test]
fn shape_rule_composition_rejects_unsupported_state_machine_directives() {
    let mut inputs = inputs();
    inputs
        .shape_rules
        .get_mut("struct.lux.ucits.sicav")
        .expect("leaf shape rule loaded")
        .body
        .insert_between
        .push(InsertBetween {
            from: "UPLOADED".to_string(),
            to: "REVIEWED".to_string(),
            via: vec!["MANUAL_REVIEW".to_string()],
            enter_verb: None,
            exit_verb: None,
        });

    let err = resolve_template("struct.lux.ucits.sicav", "cbu", &inputs)
        .expect_err("unsupported directive should fail loudly");
    assert!(matches!(
        err,
        ResolveError::UnsupportedShapeDirective {
            ref shape,
            ref directive,
        } if shape == "struct.lux.ucits.sicav" && directive == "insert_between"
    ));
}

#[test]
fn shape_rule_composition_rejects_same_level_shape_conflicts() {
    let mut inputs = inputs();
    inputs
        .shape_rules
        .get_mut("struct.lux.ucits.sicav")
        .expect("leaf shape rule loaded")
        .body
        .extends = vec!["base.cbu".to_string(), "regulated.fund".to_string()];

    let base_refinement = SlotGateMetadataRefinement {
        closure: Some(ClosureType::Open),
        ..Default::default()
    };
    inputs
        .shape_rules
        .get_mut("base.cbu")
        .expect("base rule loaded")
        .body
        .slots
        .insert("management_company".to_string(), base_refinement);

    let regulated_refinement = SlotGateMetadataRefinement {
        closure: Some(ClosureType::ClosedBounded),
        ..Default::default()
    };
    inputs
        .shape_rules
        .get_mut("regulated.fund")
        .expect("regulated rule loaded")
        .body
        .slots
        .insert("management_company".to_string(), regulated_refinement);

    let err = resolve_template("struct.lux.ucits.sicav", "cbu", &inputs)
        .expect_err("same-level conflict should fail");
    assert!(matches!(
        err,
        ResolveError::AmbiguousShapeRefinement {
            ref slot,
            ref field,
            ref sources,
        } if slot == "management_company"
            && field == "closure"
            && sources == &vec!["base.cbu".to_string(), "regulated.fund".to_string()]
    ));
}

#[test]
fn shape_rule_composition_applies_replaceable_predicate_binding_refinement() {
    let mut inputs = inputs();
    let cbu = inputs
        .dag_taxonomies
        .get_mut("cbu")
        .expect("cbu DAG loaded")
        .dag
        .slots
        .iter_mut()
        .find(|slot| slot.id == "cbu")
        .expect("cbu slot loaded");
    let dsl_core::config::dag::SlotStateMachine::Structured(machine) =
        cbu.state_machine.as_mut().expect("cbu state machine")
    else {
        panic!("cbu state machine should be structured");
    };
    machine.predicate_bindings.push(predicate_binding(
        "shape_bound_review",
        Some("base_review"),
        true,
    ));

    let refinement = inputs
        .shape_rules
        .get_mut("struct.lux.ucits.sicav")
        .expect("leaf shape rule loaded")
        .body
        .slots
        .entry("cbu".to_string())
        .or_default();
    refinement.predicate_bindings.push(predicate_binding(
        "shape_bound_review",
        Some("sicav_review"),
        false,
    ));

    let template = resolve_template("struct.lux.ucits.sicav", "cbu", &inputs)
        .expect("template resolves with predicate binding refinement");
    let cbu = template.slot("cbu").expect("cbu slot resolved");
    let binding = cbu
        .predicate_bindings
        .iter()
        .find(|binding| binding.entity == "shape_bound_review")
        .expect("shape-refined binding present");

    assert_eq!(binding.source_entity.as_deref(), Some("sicav_review"));
    assert_eq!(
        cbu.provenance.field_sources.get("predicate_bindings"),
        Some(&ResolvedSource::ShapeRule)
    );
}

#[test]
fn shape_rule_composition_rejects_shape_rule_cycles() {
    let mut inputs = inputs();
    inputs
        .shape_rules
        .get_mut("base.cbu")
        .expect("base rule loaded")
        .body
        .extends = vec!["struct.lux.ucits.sicav".to_string()];

    let err = resolve_template("struct.lux.ucits.sicav", "cbu", &inputs)
        .expect_err("shape-rule inheritance cycle should fail");
    assert!(matches!(
        err,
        ResolveError::ShapeRuleCycle { ref cycle_path }
            if cycle_path == &vec![
                "struct.lux.ucits.sicav".to_string(),
                "ucits".to_string(),
                "regulated.fund".to_string(),
                "base.cbu".to_string(),
                "struct.lux.ucits.sicav".to_string(),
            ]
    ));
}

#[test]
fn shape_rule_composition_extracts_lux_aif_raif_macro_facts() {
    let inputs = inputs();
    let template = resolve_template("struct.lux.aif.raif", "cbu", &inputs)
        .expect("Lux AIF RAIF template resolves");

    assert_eq!(
        template.structural_facts.jurisdiction.as_deref(),
        Some("LU")
    );
    assert_eq!(
        template.structural_facts.structure_type.as_deref(),
        Some("aif")
    );
    assert_eq!(
        template.structural_facts.allowed_structure_types,
        vec!["raif"]
    );
    assert_eq!(
        template.structural_facts.document_bundles,
        vec!["docs.bundle.aif-baseline"]
    );
    assert_eq!(
        template.structural_facts.trading_profile_type.as_deref(),
        Some("aif")
    );
    assert_eq!(
        template.structural_facts.required_roles,
        vec!["aifm", "depositary"]
    );
    assert_eq!(
        template.structural_facts.optional_roles,
        vec![
            "investment-manager",
            "administrator",
            "auditor",
            "prime-broker"
        ]
    );
}

#[test]
fn shape_rule_composition_extracts_lux_pe_scsp_macro_facts() {
    let inputs = inputs();
    let template = resolve_template("struct.lux.pe.scsp", "cbu", &inputs)
        .expect("Lux PE SCSp template resolves");

    assert_eq!(
        template.structural_facts.jurisdiction.as_deref(),
        Some("LU")
    );
    assert_eq!(
        template.structural_facts.structure_type.as_deref(),
        Some("pe")
    );
    assert_eq!(
        template.structural_facts.allowed_structure_types,
        vec!["scsp", "pe"]
    );
    assert_eq!(
        template.structural_facts.document_bundles,
        vec!["docs.bundle.private-equity-baseline"]
    );
    assert_eq!(
        template.structural_facts.trading_profile_type.as_deref(),
        Some("pe")
    );
    assert_eq!(
        template.structural_facts.required_roles,
        vec!["general-partner"]
    );
    assert_eq!(
        template.structural_facts.optional_roles,
        vec![
            "aifm",
            "depositary",
            "administrator",
            "auditor",
            "legal-counsel"
        ]
    );
}

#[test]
fn shape_rule_composition_extracts_ie_ucits_icav_macro_facts() {
    let inputs = inputs();
    let template = resolve_template("struct.ie.ucits.icav", "cbu", &inputs)
        .expect("IE UCITS ICAV template resolves");

    assert_eq!(
        template.structural_facts.jurisdiction.as_deref(),
        Some("IE")
    );
    assert_eq!(
        template.structural_facts.structure_type.as_deref(),
        Some("ucits")
    );
    assert_eq!(
        template.structural_facts.allowed_structure_types,
        vec!["icav", "ucits"]
    );
    assert_eq!(
        template.structural_facts.document_bundles,
        vec!["docs.bundle.ucits-baseline"]
    );
    assert_eq!(
        template.structural_facts.trading_profile_type.as_deref(),
        Some("ucits")
    );
    assert_eq!(
        template.structural_facts.required_roles,
        vec!["management-company", "depositary"]
    );
    assert_eq!(
        template.structural_facts.optional_roles,
        vec![
            "investment-manager",
            "administrator",
            "auditor",
            "company-secretary",
            "legal-counsel"
        ]
    );
}

#[test]
fn shape_rule_composition_extracts_ie_aif_icav_macro_facts() {
    let inputs = inputs();
    let template = resolve_template("struct.ie.aif.icav", "cbu", &inputs)
        .expect("IE AIF ICAV template resolves");

    assert_eq!(
        template.structural_facts.jurisdiction.as_deref(),
        Some("IE")
    );
    assert_eq!(
        template.structural_facts.structure_type.as_deref(),
        Some("aif")
    );
    assert_eq!(
        template.structural_facts.allowed_structure_types,
        vec!["icav", "aif"]
    );
    assert_eq!(
        template.structural_facts.document_bundles,
        vec!["docs.bundle.aif-baseline"]
    );
    assert_eq!(
        template.structural_facts.trading_profile_type.as_deref(),
        Some("aif")
    );
    assert_eq!(
        template.structural_facts.required_roles,
        vec!["aifm", "depositary"]
    );
    assert_eq!(
        template.structural_facts.optional_roles,
        vec![
            "investment-manager",
            "administrator",
            "auditor",
            "prime-broker",
            "company-secretary"
        ]
    );
}

#[test]
fn shape_rule_composition_extracts_ie_hedge_icav_macro_facts() {
    let inputs = inputs();
    let template = resolve_template("struct.ie.hedge.icav", "cbu", &inputs)
        .expect("IE Hedge ICAV template resolves");

    assert_eq!(
        template.structural_facts.jurisdiction.as_deref(),
        Some("IE")
    );
    assert_eq!(
        template.structural_facts.structure_type.as_deref(),
        Some("aif")
    );
    assert_eq!(
        template.structural_facts.allowed_structure_types,
        vec!["icav", "hedge", "qiaif"]
    );
    assert_eq!(
        template.structural_facts.document_bundles,
        vec!["docs.bundle.aif-baseline", "docs.bundle.hedge-baseline"]
    );
    assert_eq!(
        template.structural_facts.trading_profile_type.as_deref(),
        Some("hedge")
    );
    assert_eq!(
        template.structural_facts.required_roles,
        vec!["aifm", "depositary"]
    );
    assert_eq!(
        template.structural_facts.optional_roles,
        vec![
            "investment-manager",
            "administrator",
            "auditor",
            "prime-broker",
            "executing-broker"
        ]
    );
}

#[test]
fn shape_rule_composition_extracts_uk_macro_facts() {
    struct Expected<'a> {
        shape: &'a str,
        structure_type: &'a str,
        allowed_structure_types: &'a [&'a str],
        document_bundles: &'a [&'a str],
        trading_profile_type: Option<&'a str>,
        required_roles: &'a [&'a str],
        optional_roles: &'a [&'a str],
    }

    let cases = [
        Expected {
            shape: "struct.uk.authorised.oeic",
            structure_type: "uk-authorised",
            allowed_structure_types: &["oeic", "uk-authorised"],
            document_bundles: &["docs.bundle.uk-authorised-baseline"],
            trading_profile_type: Some("uk-authorised"),
            required_roles: &["authorised-corporate-director", "depositary"],
            optional_roles: &[
                "investment-manager",
                "administrator",
                "auditor",
                "registrar",
            ],
        },
        Expected {
            shape: "struct.uk.authorised.aut",
            structure_type: "uk-authorised",
            allowed_structure_types: &["aut", "uk-authorised"],
            document_bundles: &["docs.bundle.uk-authorised-baseline"],
            trading_profile_type: Some("uk-authorised"),
            required_roles: &["authorised-fund-manager", "trustee"],
            optional_roles: &["investment-manager", "administrator", "auditor"],
        },
        Expected {
            shape: "struct.uk.authorised.acs",
            structure_type: "uk-authorised",
            allowed_structure_types: &["acs", "uk-authorised"],
            document_bundles: &["docs.bundle.uk-authorised-baseline"],
            trading_profile_type: Some("uk-authorised"),
            required_roles: &["acs-operator", "depositary"],
            optional_roles: &["investment-manager", "administrator", "auditor"],
        },
        Expected {
            shape: "struct.uk.authorised.ltaf",
            structure_type: "uk-authorised",
            allowed_structure_types: &["ltaf", "uk-authorised"],
            document_bundles: &[
                "docs.bundle.uk-authorised-baseline",
                "docs.bundle.ltaf-baseline",
            ],
            trading_profile_type: Some("ltaf"),
            required_roles: &["authorised-corporate-director", "depositary"],
            optional_roles: &[
                "investment-manager",
                "administrator",
                "auditor",
                "valuation-agent",
            ],
        },
        Expected {
            shape: "struct.uk.manager.llp",
            structure_type: "manager",
            allowed_structure_types: &["llp", "manager"],
            document_bundles: &["docs.bundle.manager-baseline"],
            trading_profile_type: None,
            required_roles: &["designated-member"],
            optional_roles: &["compliance-officer", "mlro", "auditor"],
        },
        Expected {
            shape: "struct.uk.private-equity.lp",
            structure_type: "pe",
            allowed_structure_types: &["lp", "pe"],
            document_bundles: &["docs.bundle.private-equity-baseline"],
            trading_profile_type: Some("pe"),
            required_roles: &["general-partner"],
            optional_roles: &[
                "aifm",
                "depositary",
                "administrator",
                "auditor",
                "legal-counsel",
            ],
        },
    ];

    let inputs = inputs();
    for expected in cases {
        let template =
            resolve_template(expected.shape, "cbu", &inputs).expect("UK template resolves");
        assert_eq!(
            template.structural_facts.jurisdiction.as_deref(),
            Some("UK"),
            "{}",
            expected.shape
        );
        assert_eq!(
            template.structural_facts.structure_type.as_deref(),
            Some(expected.structure_type),
            "{}",
            expected.shape
        );
        assert_eq!(
            template.structural_facts.allowed_structure_types, expected.allowed_structure_types,
            "{}",
            expected.shape
        );
        assert_eq!(
            template.structural_facts.document_bundles, expected.document_bundles,
            "{}",
            expected.shape
        );
        assert_eq!(
            template.structural_facts.trading_profile_type.as_deref(),
            expected.trading_profile_type,
            "{}",
            expected.shape
        );
        assert_eq!(
            template.structural_facts.required_roles, expected.required_roles,
            "{}",
            expected.shape
        );
        assert_eq!(
            template.structural_facts.optional_roles, expected.optional_roles,
            "{}",
            expected.shape
        );
    }
}

#[test]
fn shape_rule_composition_extracts_us_macro_facts() {
    struct Expected<'a> {
        shape: &'a str,
        structure_type: &'a str,
        allowed_structure_types: &'a [&'a str],
        document_bundles: &'a [&'a str],
        trading_profile_type: &'a str,
        required_roles: &'a [&'a str],
        optional_roles: &'a [&'a str],
    }

    let cases = [
        Expected {
            shape: "struct.us.40act.open-end",
            structure_type: "40act",
            allowed_structure_types: &["40act", "open-end", "mutual-fund"],
            document_bundles: &["docs.bundle.us-40act-baseline"],
            trading_profile_type: "40act",
            required_roles: &["investment-adviser", "custodian"],
            optional_roles: &[
                "sub-adviser",
                "administrator",
                "transfer-agent",
                "distributor",
                "auditor",
                "legal-counsel",
            ],
        },
        Expected {
            shape: "struct.us.40act.closed-end",
            structure_type: "40act",
            allowed_structure_types: &["40act", "closed-end"],
            document_bundles: &["docs.bundle.us-40act-baseline"],
            trading_profile_type: "40act",
            required_roles: &["investment-adviser", "custodian"],
            optional_roles: &[
                "sub-adviser",
                "administrator",
                "transfer-agent",
                "auditor",
                "legal-counsel",
            ],
        },
        Expected {
            shape: "struct.us.etf.40act",
            structure_type: "40act",
            allowed_structure_types: &["etf", "40act"],
            document_bundles: &["docs.bundle.etf-baseline"],
            trading_profile_type: "etf",
            required_roles: &["investment-adviser", "custodian", "authorized-participant"],
            optional_roles: &[
                "sub-adviser",
                "administrator",
                "transfer-agent",
                "distributor",
                "auditor",
                "market-maker",
            ],
        },
        Expected {
            shape: "struct.us.private-fund.delaware-lp",
            structure_type: "private-fund",
            allowed_structure_types: &["delaware-lp", "private-fund", "pe", "hedge"],
            document_bundles: &["docs.bundle.private-equity-baseline"],
            trading_profile_type: "private-fund",
            required_roles: &["general-partner", "investment-manager"],
            optional_roles: &[
                "custodian",
                "administrator",
                "prime-broker",
                "auditor",
                "legal-counsel",
                "tax-advisor",
            ],
        },
    ];

    let inputs = inputs();
    for expected in cases {
        let template =
            resolve_template(expected.shape, "cbu", &inputs).expect("US template resolves");
        assert_eq!(
            template.structural_facts.jurisdiction.as_deref(),
            Some("US"),
            "{}",
            expected.shape
        );
        assert_eq!(
            template.structural_facts.structure_type.as_deref(),
            Some(expected.structure_type),
            "{}",
            expected.shape
        );
        assert_eq!(
            template.structural_facts.allowed_structure_types, expected.allowed_structure_types,
            "{}",
            expected.shape
        );
        assert_eq!(
            template.structural_facts.document_bundles, expected.document_bundles,
            "{}",
            expected.shape
        );
        assert_eq!(
            template.structural_facts.trading_profile_type.as_deref(),
            Some(expected.trading_profile_type),
            "{}",
            expected.shape
        );
        assert_eq!(
            template.structural_facts.required_roles, expected.required_roles,
            "{}",
            expected.shape
        );
        assert_eq!(
            template.structural_facts.optional_roles, expected.optional_roles,
            "{}",
            expected.shape
        );
    }
}

#[test]
fn shape_rule_composition_extracts_cross_border_macro_facts() {
    struct Expected<'a> {
        shape: &'a str,
        jurisdiction: &'a str,
        structure_type: &'a str,
        allowed_structure_types: &'a [&'a str],
        document_bundles: &'a [&'a str],
        trading_profile_type: &'a str,
        required_roles: &'a [&'a str],
        optional_roles: &'a [&'a str],
    }

    let cases = [
        Expected {
            shape: "struct.hedge.cross-border",
            jurisdiction: "cross-border",
            structure_type: "hedge",
            allowed_structure_types: &["hedge", "cross-border", "master-feeder"],
            document_bundles: &["docs.bundle.hedge-baseline"],
            trading_profile_type: "hedge",
            required_roles: &["aifm", "depositary"],
            optional_roles: &[
                "investment-manager",
                "administrator",
                "auditor",
                "prime-broker",
            ],
        },
        Expected {
            shape: "struct.pe.cross-border",
            jurisdiction: "cross-border",
            structure_type: "pe",
            allowed_structure_types: &["pe", "cross-border", "parallel"],
            document_bundles: &["docs.bundle.private-equity-baseline"],
            trading_profile_type: "pe",
            required_roles: &["general-partner"],
            optional_roles: &[
                "aifm",
                "depositary",
                "administrator",
                "auditor",
                "legal-counsel",
            ],
        },
    ];

    let inputs = inputs();
    for expected in cases {
        let template = resolve_template(expected.shape, "cbu", &inputs)
            .expect("cross-border template resolves");
        assert_eq!(
            template.structural_facts.jurisdiction.as_deref(),
            Some(expected.jurisdiction),
            "{}",
            expected.shape
        );
        assert_eq!(
            template.structural_facts.structure_type.as_deref(),
            Some(expected.structure_type),
            "{}",
            expected.shape
        );
        assert_eq!(
            template.structural_facts.allowed_structure_types, expected.allowed_structure_types,
            "{}",
            expected.shape
        );
        assert_eq!(
            template.structural_facts.document_bundles, expected.document_bundles,
            "{}",
            expected.shape
        );
        assert_eq!(
            template.structural_facts.trading_profile_type.as_deref(),
            Some(expected.trading_profile_type),
            "{}",
            expected.shape
        );
        assert_eq!(
            template.structural_facts.required_roles, expected.required_roles,
            "{}",
            expected.shape
        );
        assert_eq!(
            template.structural_facts.optional_roles, expected.optional_roles,
            "{}",
            expected.shape
        );
    }
}
