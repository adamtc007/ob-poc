//! Effect declaration health check — Tranche 1 regression baseline.
//!
//! Loads all verb YAML files via the `ConfigLoader` and checks every declared
//! `effect_class` is a valid `EffectClass` variant.
//!
//! Type A (extra effects, future concept): log only.
//! Type B (missing effect_class): log only — many verbs legitimately omit it.
//! Type C (YAML deserialization error for effect_class): **test fails**.
//!
//! Since `EffectClass` is deserialized by serde from YAML during
//! `ConfigLoader::load_verbs()`, a Type C error would prevent the loader
//! from loading the file at all. This test therefore catches Type C errors
//! by asserting the full YAML load succeeds and collecting statistics.

use dsl_core::ConfigLoader;
use std::path::PathBuf;

/// Locate the workspace config directory relative to CARGO_MANIFEST_DIR.
fn workspace_config_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR points to crates/dsl-core during tests.
    // The verb config is at rust/config/ (3 levels up).
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .expect("parent of crates/dsl-core")
        .parent() // rust/
        .expect("parent of crates/")
        .join("config")
}

/// Load all verb configs from the workspace config directory.
/// Panics (Type C error) if any YAML file fails to parse.
fn load_all_verbs() -> dsl_core::VerbsConfig {
    let config_dir = workspace_config_dir();
    assert!(
        config_dir.exists(),
        "verb config dir not found at {}",
        config_dir.display()
    );

    let loader = ConfigLoader::new(config_dir.to_string_lossy());
    loader
        .load_verbs()
        .expect("all verb YAML files must parse without Type C errors")
}

#[test]
fn all_verb_yaml_files_parse_without_type_c_errors() {
    // This test exercises the full YAML load pipeline.
    // If any `effect_class:` value is not a valid EffectClass variant,
    // serde_yaml will return an error here → test fails (Type C).
    let verbs_config = load_all_verbs();

    let domain_count = verbs_config.domains.len();
    let total_verbs: usize = verbs_config.domains.values().map(|d| d.verbs.len()).sum();

    println!(
        "Loaded {} domains, {} verbs — no Type C errors",
        domain_count, total_verbs
    );

    assert!(domain_count > 0, "expected at least one domain to be loaded");
    assert!(total_verbs > 0, "expected at least one verb to be loaded");
}

#[test]
fn effect_class_coverage_statistics() {
    let verbs_config = load_all_verbs();

    let mut with_effect_class: Vec<(String, String, String)> = Vec::new(); // (domain, verb, effect_class)
    let mut without_effect_class: Vec<(String, String)> = Vec::new(); // (domain, verb)

    for (domain_name, domain_config) in &verbs_config.domains {
        for (verb_name, verb_config) in &domain_config.verbs {
            match &verb_config.effect_class {
                Some(ec) => {
                    with_effect_class.push((
                        domain_name.clone(),
                        verb_name.clone(),
                        format!("{:?}", ec),
                    ));
                }
                None => {
                    without_effect_class.push((domain_name.clone(), verb_name.clone()));
                }
            }
        }
    }

    let total = with_effect_class.len() + without_effect_class.len();
    let coverage_pct = if total > 0 {
        (with_effect_class.len() * 100) / total
    } else {
        0
    };

    println!(
        "Effect class coverage: {}/{} verbs ({coverage_pct}%)",
        with_effect_class.len(),
        total
    );
    println!(
        "Type B (missing effect_class, logged only): {} verbs",
        without_effect_class.len()
    );

    // Print all Type B for human review — do not fail, just inform.
    if !without_effect_class.is_empty() {
        println!("Verbs without effect_class declaration (Type B):");
        for (domain, verb) in &without_effect_class {
            println!("  {}.{}", domain, verb);
        }
    }

    // Structural assertion: we must have loaded something meaningful.
    assert!(
        total >= 100,
        "expected ≥100 verbs across all YAML files, got {total} — possible load failure"
    );
}

#[test]
fn known_effect_class_values_are_valid_variants() {
    use dsl_core::EffectClass;

    let verbs_config = load_all_verbs();

    let mut all_effect_classes: Vec<(String, String, EffectClass)> = Vec::new();

    for (domain_name, domain_config) in &verbs_config.domains {
        for (verb_name, verb_config) in &domain_config.verbs {
            if let Some(ec) = verb_config.effect_class {
                all_effect_classes.push((domain_name.clone(), verb_name.clone(), ec));
            }
        }
    }

    println!(
        "Verbs with effect_class declared: {}",
        all_effect_classes.len()
    );

    // Verify each declared EffectClass matches one of the known variants.
    // This is redundant with serde deserialization but makes the assertion
    // explicit and future-proof if new variants are added.
    let known_variants = [
        EffectClass::Pure,
        EffectClass::ReadSnapshot,
        EffectClass::IdempotentEnsure,
        EffectClass::AppendFact,
        EffectClass::AppendTransitionSnapshot,
        EffectClass::CommutativeAccumulate,
        EffectClass::ReadModifyWrite,
        EffectClass::CrossResourceInvariant,
        EffectClass::ExternalEffect,
        EffectClass::AdminOverride,
    ];

    let mut invalid_entries: Vec<String> = Vec::new();

    for (domain, verb, ec) in &all_effect_classes {
        if !known_variants.contains(ec) {
            invalid_entries.push(format!(
                "{}.{}: unexpected EffectClass variant {:?}",
                domain, verb, ec
            ));
        }
    }

    if !invalid_entries.is_empty() {
        panic!(
            "Type C errors — effect_class values that are not known EffectClass variants:\n{}",
            invalid_entries.join("\n")
        );
    }

    // Snapshot the per-variant distribution for regression tracking.
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for (_, _, ec) in &all_effect_classes {
        *counts.entry(format!("{:?}", ec)).or_insert(0) += 1;
    }
    println!("Effect class distribution:");
    for (variant, count) in &counts {
        println!("  {:.<40} {}", variant, count);
    }

    // Sanity check: must have at least one ReadModifyWrite (most common)
    let rmw_count = all_effect_classes
        .iter()
        .filter(|(_, _, ec)| matches!(ec, EffectClass::ReadModifyWrite))
        .count();
    assert!(
        rmw_count > 0,
        "expected at least one ReadModifyWrite effect_class — likely a load failure"
    );
}
