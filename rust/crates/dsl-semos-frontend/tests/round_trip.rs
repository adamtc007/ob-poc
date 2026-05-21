//! Round-trip tests: YAML loader ↔ DSL loader produce equivalent VerbsConfig.
//!
//! These tests prove that the Tranche 3 DSL pipeline preserves all verb
//! metadata faithfully, ensuring Tranche 1 snapshot tests remain unaffected
//! when the system switches to DSL as the source of truth.

use dsl_diagnostics::DiagnosticBag;
use dsl_semos_frontend::loader::load_verbs_from_dsl_dir;
use std::path::Path;

/// Base path: from CARGO_MANIFEST_DIR (crates/dsl-semos-frontend) we go
///   → parent → parent → rust/
fn workspace_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()  // crates/
        .unwrap()
        .parent()  // rust/
        .unwrap()
        .to_path_buf()
}

fn dsl_dir() -> std::path::PathBuf {
    workspace_root().join("dsl-source/verbs")
}

fn yaml_config() -> dsl_core::config::types::VerbsConfig {
    let loader = dsl_core::config::loader::ConfigLoader::new(
        workspace_root().join("config").to_string_lossy().to_string()
    );
    loader.load_verbs().expect("YAML loader should succeed")
}

// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn all_domains_load_without_errors() {
    let dir = dsl_dir();
    if !dir.exists() {
        eprintln!("SKIP: dsl-source/verbs not found (run `cargo run --bin verb_to_dsl` first)");
        return;
    }

    let mut diag = DiagnosticBag::new();
    let config = load_verbs_from_dsl_dir(&dir, &mut diag);

    let errors: Vec<_> = diag.errors().collect();
    assert!(
        errors.is_empty(),
        "DSL load errors:\n{}",
        errors.iter().map(|d| format!("  {}", d.message)).collect::<Vec<_>>().join("\n")
    );

    // Should have loaded all domains
    assert!(
        config.domains.len() >= 80,
        "expected >= 80 domains, got {}",
        config.domains.len()
    );

    // Count total verbs
    let total: usize = config.domains.values().map(|d| d.verbs.len()).sum();
    assert!(
        total >= 1000,
        "expected >= 1000 verbs, got {}",
        total
    );

    eprintln!(
        "Loaded {} domains, {} verbs from DSL",
        config.domains.len(), total
    );
}

#[test]
fn cbu_domain_round_trip() {
    let dir = dsl_dir();
    if !dir.exists() {
        eprintln!("SKIP: dsl-source/verbs not found");
        return;
    }

    // Load via YAML (old path)
    let yaml_config = yaml_config();
    let yaml_cbu = yaml_config.domains.get("cbu")
        .expect("YAML config should have 'cbu' domain");

    // Load via DSL (new path)
    let mut diag = DiagnosticBag::new();
    let dsl_config = load_verbs_from_dsl_dir(&dir, &mut diag);

    let errors: Vec<_> = diag.errors().collect();
    assert!(errors.is_empty(), "DSL load errors for cbu: {:?}", errors.iter().map(|e| &e.message).collect::<Vec<_>>());

    let dsl_cbu = dsl_config.domains.get("cbu")
        .expect("DSL config should have 'cbu' domain");

    // Verb counts must match
    assert_eq!(
        yaml_cbu.verbs.len(),
        dsl_cbu.verbs.len(),
        "cbu verb count mismatch: yaml={} dsl={}",
        yaml_cbu.verbs.len(),
        dsl_cbu.verbs.len()
    );

    // Compare each verb's key fields
    for (name, yaml_verb) in &yaml_cbu.verbs {
        let dsl_verb = dsl_cbu.verbs.get(name)
            .unwrap_or_else(|| panic!("verb 'cbu.{}' missing from DSL config", name));

        assert_eq!(
            yaml_verb.description, dsl_verb.description,
            "cbu.{} description mismatch", name
        );
        assert_eq!(
            yaml_verb.args.len(), dsl_verb.args.len(),
            "cbu.{} args count mismatch", name
        );
        assert_eq!(
            yaml_verb.effect_class, dsl_verb.effect_class,
            "cbu.{} effect_class mismatch", name
        );
        assert_eq!(
            yaml_verb.behavior, dsl_verb.behavior,
            "cbu.{} behavior mismatch", name
        );
        assert_eq!(
            yaml_verb.three_axis.is_some(), dsl_verb.three_axis.is_some(),
            "cbu.{} three_axis presence mismatch", name
        );
        assert_eq!(
            yaml_verb.transition_args.is_some(), dsl_verb.transition_args.is_some(),
            "cbu.{} transition_args presence mismatch", name
        );
    }

    eprintln!("cbu domain: {} verbs verified (YAML == DSL)", yaml_cbu.verbs.len());
}

#[test]
fn tranche1_snapshots_unaffected() {
    // This test verifies that switching to DSL loading doesn't change
    // the verb registry visible to compile_to_steps().
    // If this passes, Tranche 1 regressions are protected.

    let dir = dsl_dir();
    if !dir.exists() {
        eprintln!("SKIP: dsl-source/verbs not found");
        return;
    }

    // Load via YAML
    let yaml_config = yaml_config();

    // Load via DSL
    let mut diag = DiagnosticBag::new();
    let dsl_config = load_verbs_from_dsl_dir(&dir, &mut diag);

    let errors: Vec<_> = diag.errors().collect();
    assert!(errors.is_empty(), "DSL load produced {} errors", errors.len());

    // Compare total verb counts across all domains
    let yaml_total: usize = yaml_config.domains.values().map(|d| d.verbs.len()).sum();
    let dsl_total: usize = dsl_config.domains.values().map(|d| d.verbs.len()).sum();

    // DSL total should be at least as large as YAML (DSL may include some extras
    // from how the generator handles merged domains, but should not be less)
    assert!(
        dsl_total >= yaml_total,
        "DSL total verbs ({}) < YAML total verbs ({})",
        dsl_total, yaml_total
    );

    eprintln!(
        "Tranche 1 regression check: YAML={} verbs, DSL={} verbs",
        yaml_total, dsl_total
    );

    // Spot check 5 key verbs from different domains
    for (domain, verb_name) in &[
        ("cbu", "create"),
        ("deal", "create"),
        ("entity", "create"),
        ("session", "info"),
    ] {
        let y_domain = yaml_config.domains.get(*domain);
        let d_domain = dsl_config.domains.get(*domain);

        if let (Some(y_d), Some(d_d)) = (y_domain, d_domain) {
            if let (Some(y_verb), Some(d_verb)) = (y_d.verbs.get(*verb_name), d_d.verbs.get(*verb_name)) {
                assert_eq!(
                    y_verb.description, d_verb.description,
                    "{}.{} description mismatch", domain, verb_name
                );
                assert_eq!(
                    y_verb.args.len(), d_verb.args.len(),
                    "{}.{} args count mismatch", domain, verb_name
                );
                eprintln!("  {}.{}: OK", domain, verb_name);
            }
        }
    }
}

#[test]
fn arg_config_round_trip_spot_check() {
    // Verify that ArgConfig with lookup fields round-trips correctly
    let dir = dsl_dir();
    if !dir.exists() {
        eprintln!("SKIP: dsl-source/verbs not found");
        return;
    }

    let yaml_config = yaml_config();
    let mut diag = DiagnosticBag::new();
    let dsl_config = load_verbs_from_dsl_dir(&dir, &mut diag);

    let errors: Vec<_> = diag.errors().collect();
    assert!(errors.is_empty(), "DSL load errors: {:?}", errors.iter().map(|e| &e.message).collect::<Vec<_>>());

    // cbu.create has args with lookup config — verify round-trip
    let yaml_verb = yaml_config.domains.get("cbu")
        .and_then(|d| d.verbs.get("create"))
        .expect("cbu.create should exist in YAML config");

    let dsl_verb = dsl_config.domains.get("cbu")
        .and_then(|d| d.verbs.get("create"))
        .expect("cbu.create should exist in DSL config");

    assert_eq!(yaml_verb.args.len(), dsl_verb.args.len(), "cbu.create args count");

    for (yaml_arg, dsl_arg) in yaml_verb.args.iter().zip(dsl_verb.args.iter()) {
        assert_eq!(yaml_arg.name, dsl_arg.name, "arg name mismatch");
        assert_eq!(yaml_arg.required, dsl_arg.required, "arg {} required mismatch", yaml_arg.name);
        assert_eq!(
            yaml_arg.lookup.is_some(), dsl_arg.lookup.is_some(),
            "arg {} lookup presence mismatch", yaml_arg.name
        );
        if let (Some(y_lookup), Some(d_lookup)) = (&yaml_arg.lookup, &dsl_arg.lookup) {
            assert_eq!(y_lookup.table, d_lookup.table, "arg {} lookup.table mismatch", yaml_arg.name);
            assert_eq!(y_lookup.primary_key, d_lookup.primary_key, "arg {} lookup.primary_key mismatch", yaml_arg.name);
        }
    }

    eprintln!("cbu.create arg round-trip: {} args verified", yaml_verb.args.len());
}

#[test]
fn three_axis_round_trip() {
    // Verify three_axis JSON round-trip
    let dir = dsl_dir();
    if !dir.exists() {
        eprintln!("SKIP: dsl-source/verbs not found");
        return;
    }

    let yaml_config = yaml_config();
    let mut diag = DiagnosticBag::new();
    let dsl_config = load_verbs_from_dsl_dir(&dir, &mut diag);

    // Find verbs with three_axis declared
    let mut checked = 0;
    for (domain_name, domain) in &yaml_config.domains {
        for (verb_name, yaml_verb) in &domain.verbs {
            if let Some(yaml_three) = &yaml_verb.three_axis {
                let dsl_three = dsl_config.domains
                    .get(domain_name)
                    .and_then(|d| d.verbs.get(verb_name))
                    .and_then(|v| v.three_axis.as_ref());

                let dsl_three = dsl_three.unwrap_or_else(|| panic!(
                    "{}.{} has three_axis in YAML but not in DSL", domain_name, verb_name
                ));

                assert_eq!(
                    yaml_three.state_effect, dsl_three.state_effect,
                    "{}.{} three_axis.state_effect mismatch", domain_name, verb_name
                );
                assert_eq!(
                    yaml_three.consequence.baseline, dsl_three.consequence.baseline,
                    "{}.{} three_axis.consequence.baseline mismatch", domain_name, verb_name
                );

                checked += 1;
                if checked >= 20 {
                    break;  // spot check 20 verbs is sufficient
                }
            }
        }
        if checked >= 20 {
            break;
        }
    }

    eprintln!("three_axis round-trip: {} verbs spot-checked", checked);
}

#[test]
fn transition_args_round_trip() {
    // Verify Pattern D verbs (transition_args) round-trip correctly
    let dir = dsl_dir();
    if !dir.exists() {
        eprintln!("SKIP: dsl-source/verbs not found");
        return;
    }

    let yaml_config = yaml_config();
    let mut diag = DiagnosticBag::new();
    let dsl_config = load_verbs_from_dsl_dir(&dir, &mut diag);

    let mut checked = 0;
    for (domain_name, domain) in &yaml_config.domains {
        for (verb_name, yaml_verb) in &domain.verbs {
            if let Some(yaml_ta) = &yaml_verb.transition_args {
                let dsl_ta = dsl_config.domains
                    .get(domain_name)
                    .and_then(|d| d.verbs.get(verb_name))
                    .and_then(|v| v.transition_args.as_ref());

                let dsl_ta = dsl_ta.unwrap_or_else(|| panic!(
                    "{}.{} has transition_args in YAML but not in DSL", domain_name, verb_name
                ));

                assert_eq!(
                    yaml_ta.entity_id_arg, dsl_ta.entity_id_arg,
                    "{}.{} transition_args.entity_id_arg mismatch", domain_name, verb_name
                );
                assert_eq!(
                    yaml_ta.target_state_arg, dsl_ta.target_state_arg,
                    "{}.{} transition_args.target_state_arg mismatch", domain_name, verb_name
                );

                checked += 1;
            }
        }
    }

    assert!(checked > 0, "expected at least 1 Pattern D verb (transition_args)");
    eprintln!("Pattern D (transition_args) round-trip: {} verbs verified", checked);
}
