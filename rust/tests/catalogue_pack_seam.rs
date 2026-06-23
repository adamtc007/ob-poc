use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct PackManifestSubset {
    allowed_verbs: Vec<String>,
    #[serde(default)]
    forbidden_verbs: Vec<String>,
}

const CATALOGUE_MAINTENANCE_VERBS: &[&str] = &[
    "product.define",
    "product.amend",
    "product.retire",
    "service.define",
    "service.propose-revision",
    "service.deprecate",
    "service.retire",
    "service-version.draft",
    "service-version.submit-for-review",
    "service-version.publish",
    "service-version.retire",
    "service-version.update",
    "service-version.compare",
    "attribute.define",
    "attribute.define-derived",
    "attribute.define-internal",
    "product-service.override-option",
    "product-service.link",
    "product-service.amend",
    "product-service.unlink",
    "service-resource.define-type",
    "service-resource.amend-type",
    "service-resource.retire-type",
    "service-resource.add-capability",
    "service-resource.amend-capability",
    "service-resource.remove-capability",
    "service-resource.sync-definitions",
    "resource-owner.assign",
    "resource-owner.amend",
    "resource-owner.unassign",
];

const CATALOGUE_TABLES: &[&str] = &[
    "products",
    "services",
    "product_services",
    "service_versions",
    "service_resource_types",
    "service_resource_capabilities",
    "resource_owner_principals",
    "resource_attribute_requirements",
    "attribute_registry",
    "product_service_conditions",
    "product_service_option_overrides",
];

const OPERATIONAL_INSTANCE_TABLES: &[&str] = &[
    "application_instances",
    "capability_bindings",
    "cbu_resource_instances",
    "cbu_lifecycle_instances",
    "service_delivery_map",
    "provisioning_requests",
    "provisioning_events",
    "onboarding_data_request_slices",
];

fn pack_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("config")
        .join("packs")
        .join(name)
}

fn load_pack(name: &str) -> PackManifestSubset {
    let raw = fs::read_to_string(pack_path(name)).expect("pack manifest should be readable");
    serde_yaml::from_str(&raw).expect("pack manifest should parse")
}

fn as_set(values: &[String]) -> BTreeSet<&str> {
    values.iter().map(String::as_str).collect()
}

fn known_write_table(verb: &str) -> Option<&'static str> {
    match verb {
        "product.define" | "product.amend" | "product.retire" => Some("products"),
        "service.define" | "service.propose-revision" | "service.deprecate" | "service.retire" => {
            Some("services")
        }
        "service-version.draft"
        | "service-version.submit-for-review"
        | "service-version.publish"
        | "service-version.retire"
        | "service-version.update" => Some("service_versions"),
        "attribute.define" | "attribute.define-derived" | "attribute.define-internal" => {
            Some("attribute_registry")
        }
        "product-service.override-option" => Some("product_service_option_overrides"),
        "product-service.link" | "product-service.amend" | "product-service.unlink" => {
            Some("product_services")
        }
        "service-resource.define-type"
        | "service-resource.amend-type"
        | "service-resource.retire-type"
        | "service-resource.sync-definitions" => Some("service_resource_types"),
        "service-resource.add-capability"
        | "service-resource.amend-capability"
        | "service-resource.remove-capability" => Some("service_resource_capabilities"),
        "resource-owner.assign" | "resource-owner.amend" | "resource-owner.unassign" => {
            Some("resource_owner_principals")
        }
        "service-resource.provision"
        | "service-resource.activate"
        | "service-resource.set-attr" => Some("cbu_resource_instances"),
        "service-resource.confirm-provisioning-result"
        | "service-resource.get-provisioning-status" => Some("provisioning_requests"),
        "application-instance.provision"
        | "application-instance.activate"
        | "application-instance.enter-maintenance"
        | "application-instance.exit-maintenance"
        | "application-instance.take-offline"
        | "application-instance.bring-online"
        | "application-instance.decommission" => Some("application_instances"),
        "capability-binding.draft"
        | "capability-binding.start-pilot"
        | "capability-binding.promote-live"
        | "capability-binding.abort-pilot"
        | "capability-binding.deprecate"
        | "capability-binding.retire" => Some("capability_bindings"),
        "service-consumption.provision"
        | "service-consumption.activate"
        | "service-consumption.suspend"
        | "service-consumption.reinstate"
        | "service-consumption.begin-winddown"
        | "service-consumption.retire" => Some("service_delivery_map"),
        "service-version.compare" => None,
        _ => None,
    }
}

#[test]
fn catalogue_pack_lists_catalogue_maintenance_verbs() {
    let pack = load_pack("product-service-taxonomy.yaml");
    let allowed = as_set(&pack.allowed_verbs);

    for verb in CATALOGUE_MAINTENANCE_VERBS {
        assert!(
            allowed.contains(verb),
            "catalogue-maintenance verb {verb} is missing from product-service-taxonomy"
        );
    }
}

#[test]
fn catalogue_and_operational_packs_do_not_cross_write_tables() {
    let catalogue_pack = load_pack("product-service-taxonomy.yaml");
    let catalogue_allowed = as_set(&catalogue_pack.allowed_verbs);
    let catalogue_forbidden = as_set(&catalogue_pack.forbidden_verbs);
    let catalogue_tables: BTreeSet<&str> = CATALOGUE_TABLES.iter().copied().collect();
    let operational_tables: BTreeSet<&str> = OPERATIONAL_INSTANCE_TABLES.iter().copied().collect();

    for verb in &catalogue_pack.allowed_verbs {
        if let Some(table) = known_write_table(verb) {
            assert!(
                !operational_tables.contains(table),
                "catalogue pack allows {verb}, which writes operational table {table}"
            );
        }
    }

    for verb in [
        "service-resource.provision",
        "service-resource.activate",
        "service-resource.set-attr",
    ] {
        assert!(
            !catalogue_allowed.contains(verb) && catalogue_forbidden.contains(verb),
            "catalogue pack must forbid operational instance verb {verb}"
        );
    }

    for pack_name in ["lifecycle-resources.yaml", "onboarding-request.yaml"] {
        let operational_pack = load_pack(pack_name);
        for verb in &operational_pack.allowed_verbs {
            if let Some(table) = known_write_table(verb) {
                assert!(
                    !catalogue_tables.contains(table),
                    "operational pack {pack_name} allows {verb}, which writes catalogue table {table}"
                );
            }
        }
    }
}
