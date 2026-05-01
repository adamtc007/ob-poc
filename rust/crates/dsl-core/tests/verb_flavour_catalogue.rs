use dsl_core::config::{
    validate_verbs_config, ConfigLoader, ValidationContext, VerbFlavour, VerbsConfig,
};

fn load_real_catalogue() -> VerbsConfig {
    ConfigLoader::from_env()
        .load_verbs()
        .expect("real verb catalogue loads")
}

#[test]
fn every_catalogue_verb_has_phase7_flavour() {
    let cfg = load_real_catalogue();
    let total: usize = cfg.domains.values().map(|domain| domain.verbs.len()).sum();
    let annotated: usize = cfg
        .domains
        .values()
        .flat_map(|domain| domain.verbs.values())
        .filter(|verb| verb.flavour.is_some())
        .count();

    assert!(
        total >= 1288,
        "real verb catalogue count regressed below baseline"
    );
    assert_eq!(annotated, total, "all verbs must carry flavour");
}

#[test]
fn phase7_flavour_lints_are_clean_for_real_catalogue() {
    let cfg = load_real_catalogue();
    let report = validate_verbs_config(
        &cfg,
        &ValidationContext {
            require_flavour: true,
            ..ValidationContext::default()
        },
    );

    assert!(
        report.well_formedness.is_empty(),
        "flavour well-formedness errors: {:#?}",
        report.well_formedness
    );
}

#[test]
fn discretionary_verbs_have_authority_and_audit_metadata() {
    let cfg = load_real_catalogue();
    let mut checked = 0;
    for (domain_name, domain) in &cfg.domains {
        for (verb_name, verb) in &domain.verbs {
            if verb.flavour != Some(VerbFlavour::Discretionary) {
                continue;
            }
            checked += 1;
            let fqn = format!("{domain_name}.{verb_name}");
            let role_guard = verb
                .role_guard
                .as_ref()
                .unwrap_or_else(|| panic!("{fqn} missing role_guard"));
            assert!(
                !role_guard.any_of.is_empty() || !role_guard.all_of.is_empty(),
                "{fqn} has empty role_guard"
            );
            assert!(
                verb.audit_class
                    .as_ref()
                    .is_some_and(|value| !value.is_empty()),
                "{fqn} missing audit_class"
            );
        }
    }

    assert!(
        checked >= 166,
        "discretionary count regressed below baseline"
    );
}

#[test]
fn tollgate_flavour_is_empty_body_only() {
    let cfg = load_real_catalogue();
    for (domain_name, domain) in &cfg.domains {
        for (verb_name, verb) in &domain.verbs {
            if verb.flavour != Some(VerbFlavour::Tollgate) {
                continue;
            }
            assert!(
                verb.crud.is_none()
                    && verb.handler.is_none()
                    && verb.graph_query.is_none()
                    && verb.durable.is_none(),
                "{domain_name}.{verb_name} has tollgate flavour but a non-empty body"
            );
        }
    }
}
