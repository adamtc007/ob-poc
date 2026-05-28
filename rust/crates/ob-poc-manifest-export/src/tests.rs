//! Unit tests for the manifest exporter.

use super::*;

const CBU_YAML: &str = r#"
domains:
  cbu:
    description: CBU domain
    verbs:
      create:
        description: Create a new CBU
        effect_class: read_modify_write
        args:
          - name: name
            type: string
            required: true
          - name: client-type
            type: string
            required: false
        produces:
          type: cbu
      add-product:
        description: Add a product to a CBU
        effect_class: read_modify_write
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              entity_type: cbu
              resolution_mode: entity
          - name: product-type
            type: string
            required: true
"#;

const IM_YAML: &str = r#"
domains:
  instrument-matrix:
    description: IM domain
    verbs:
      attach:
        description: Attach an instrument matrix to a CBU
        effect_class: idempotent_ensure
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              entity_type: cbu
              resolution_mode: entity
"#;

const ALLOWLIST: &str = r#"
public_verbs:
  - cbu.create
  - cbu.add-product
  - instrument-matrix.attach
public_decisions: []
"#;

fn write_fixtures(dir: &std::path::Path) {
    std::fs::write(dir.join("cbu.yaml"), CBU_YAML).unwrap();
    std::fs::write(dir.join("instrument-matrix.yaml"), IM_YAML).unwrap();
}

#[test]
fn export_demo_verbs_round_trips_through_dsl_manifest() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixtures(tmp.path());

    let allow: Allowlist = serde_yaml::from_str(ALLOWLIST).unwrap();
    let cfg = ExporterConfig::new("ob-poc", "v1.0.0");

    let yaml = export_to_yaml(tmp.path(), &allow, &cfg).expect("export");
    let manifest = dsl_manifest::Manifest::load_from_yaml(&yaml).expect("re-parse");

    assert_eq!(manifest.domain, "ob-poc");
    assert_eq!(manifest.catalogue_version, "v1.0.0");
    let ids: Vec<_> = manifest.verb_ids().collect();
    assert!(ids.contains(&"cbu.create"));
    assert!(ids.contains(&"cbu.add-product"));
    assert!(ids.contains(&"instrument-matrix.attach"));
    assert_eq!(ids.len(), 3);
}

#[test]
fn export_cbu_create_carries_signature_and_authority() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixtures(tmp.path());
    let allow: Allowlist = serde_yaml::from_str(ALLOWLIST).unwrap();
    let cfg = ExporterConfig::new("ob-poc", "v1.0.0");

    let yaml = export_to_yaml(tmp.path(), &allow, &cfg).unwrap();
    let manifest = dsl_manifest::Manifest::load_from_yaml(&yaml).unwrap();
    let v = manifest.lookup_verb("cbu.create").expect("present");
    assert_eq!(v.effect_class, "read_modify_write");
    assert_eq!(v.authority_required, "cbu.write");
    assert_eq!(
        v.signature
            .output
            .as_ref()
            .and_then(|o| o.produces.as_deref()),
        Some("CBU")
    );
    assert_eq!(v.signature.inputs.len(), 2);
    assert_eq!(v.signature.inputs[0].name, "name");
    assert_eq!(v.signature.inputs[0].type_name, "string");
    assert_eq!(v.signature.inputs[1].name, "client-type");
    assert!(!v.signature.inputs[1].required);
}

#[test]
fn export_add_product_records_entity_uuid_resource_dependency() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixtures(tmp.path());
    let allow: Allowlist = serde_yaml::from_str(ALLOWLIST).unwrap();
    let cfg = ExporterConfig::new("ob-poc", "v1.0.0");

    let yaml = export_to_yaml(tmp.path(), &allow, &cfg).unwrap();
    let manifest = dsl_manifest::Manifest::load_from_yaml(&yaml).unwrap();
    let v = manifest.lookup_verb("cbu.add-product").expect("present");
    assert_eq!(v.resource_dependencies.len(), 1);
    assert_eq!(v.resource_dependencies[0].kind, "EntityUuid");
    assert_eq!(v.resource_dependencies[0].from_input, "cbu-id");
    assert_eq!(
        v.resource_dependencies[0].entity_type.as_deref(),
        Some("CBU")
    );
}

#[test]
fn export_emits_entity_types_for_referenced_entities() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixtures(tmp.path());
    let allow: Allowlist = serde_yaml::from_str(ALLOWLIST).unwrap();
    let cfg = ExporterConfig::new("ob-poc", "v1.0.0");

    let yaml = export_to_yaml(tmp.path(), &allow, &cfg).unwrap();
    let manifest = dsl_manifest::Manifest::load_from_yaml(&yaml).unwrap();
    let cbu = manifest.lookup_type("CBU").expect("CBU type present");
    assert_eq!(cbu.kind, "entity");
    assert_eq!(cbu.uuid_type.as_deref(), Some("UUIDv7"));

    let string_t = manifest.lookup_type("string").expect("primitive present");
    assert_eq!(string_t.kind, "primitive");
}

#[test]
fn unknown_allowlisted_verb_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixtures(tmp.path());
    let allow = Allowlist {
        public_verbs: vec!["cbu.does-not-exist".into()],
        public_decisions: vec![],
    };
    let cfg = ExporterConfig::new("ob-poc", "v1.0.0");
    let err = export_to_yaml(tmp.path(), &allow, &cfg).unwrap_err();
    assert!(
        err.to_string().contains("not present in the catalogue"),
        "got: {err}"
    );
}

#[test]
fn decisions_in_allowlist_are_rejected_for_ob_poc() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixtures(tmp.path());
    let allow = Allowlist {
        public_verbs: vec!["cbu.create".into()],
        public_decisions: vec!["cbu_type_routing".into()],
    };
    let cfg = ExporterConfig::new("ob-poc", "v1.0.0");
    let err = export_to_yaml(tmp.path(), &allow, &cfg).unwrap_err();
    assert!(
        err.to_string()
            .contains("ob-poc does not own DMN decisions"),
        "got: {err}"
    );
}

#[test]
fn empty_allowlist_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixtures(tmp.path());
    let allow = Allowlist {
        public_verbs: vec![],
        public_decisions: vec![],
    };
    let cfg = ExporterConfig::new("ob-poc", "v1.0.0");
    let err = export_to_yaml(tmp.path(), &allow, &cfg).unwrap_err();
    assert!(err.to_string().contains("nothing to export"), "got: {err}");
}

#[test]
fn naive_utc_from_unix_matches_epoch_origin() {
    let (y, m, d, h, mi, s) = naive_utc_from_unix(0);
    assert_eq!((y, m, d, h, mi, s), (1970, 1, 1, 0, 0, 0));
    // `date -u -d "2026-05-20T00:00:00Z" +%s` = 1_779_235_200
    let (y, m, d, h, mi, s) = naive_utc_from_unix(1_779_235_200);
    assert_eq!((y, m, d, h, mi, s), (2026, 5, 20, 0, 0, 0));
}
