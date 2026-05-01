use dsl_core::resolver::{resolve_template, ManifestOptions, ResolverInputs, ResolverManifest};
use std::path::PathBuf;

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
    assert!(manifest
        .to_text()
        .contains("Slots with missing gate metadata: 0"));
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
