use dsl_core::resolver::{resolve_template, ManifestOptions, ResolverInputs, ResolverManifest};
use std::{fs, path::PathBuf};

fn inputs() -> ResolverInputs {
    ResolverInputs::from_workspace_config_dir(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config"),
    )
    .expect("resolver inputs load")
}

#[test]
fn manifest_reports_zero_missing_for_lux_sicav_pilot_inventory() {
    let inputs = inputs();
    let template = resolve_template("struct.lux.ucits.sicav", "cbu", &inputs)
        .expect("Lux SICAV template resolves");
    let manifest = ResolverManifest::from_template(&template, &ManifestOptions::pilot_lux_sicav());

    assert_eq!(manifest.slots_with_missing_gate_metadata, 0);
    assert_eq!(
        manifest.deferred_roles,
        vec!["domiciliation-agent".to_string()]
    );
    assert!(manifest
        .to_text()
        .contains("Slots with missing gate metadata: 0"));
    assert!(manifest
        .to_text()
        .contains("Deferred roles: domiciliation-agent"));
}

#[test]
fn version_hash_is_stable_for_identical_inputs() {
    let inputs = inputs();
    let first =
        resolve_template("struct.lux.ucits.sicav", "cbu", &inputs).expect("first resolve succeeds");
    let second = resolve_template("struct.lux.ucits.sicav", "cbu", &inputs)
        .expect("second resolve succeeds");

    assert_eq!(first.version, second.version);
}

#[test]
fn version_hash_includes_state_machine_seed_inputs() {
    let inputs = inputs();
    let baseline = resolve_template("struct.lux.ucits.sicav", "cbu", &inputs)
        .expect("baseline resolve succeeds");
    let dir = tempfile::tempdir().expect("tempdir");
    let extra_state_machine = dir.path().join("synthetic_state_machine.yaml");
    fs::write(
        &extra_state_machine,
        "state_machine: synthetic\nstates: [draft]\n",
    )
    .expect("write synthetic state machine");

    let mut changed_inputs = inputs;
    changed_inputs.state_machine_paths.push(extra_state_machine);
    let changed = resolve_template("struct.lux.ucits.sicav", "cbu", &changed_inputs)
        .expect("changed resolve succeeds");

    assert_ne!(baseline.version, changed.version);
}
