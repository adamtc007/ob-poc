//! Pure default generation for onboarding pipeline steps.
//!
//! When a caller provides an `OnboardingRequest` with empty attribute,
//! verb contract, taxonomy, or view lists, these functions generate
//! sensible defaults from the entity type definition.
//!
//! All functions are **pure** (no DB, no I/O).

use sem_os_core::{
    attribute_def::{AttributeConstraints, AttributeDataType, AttributeDefBody},
    entity_type_def::EntityTypeDefBody,
    membership::{MembershipKind, MembershipRuleBody},
    verb_contract::{
        VerbArgDef, VerbContractBody, VerbContractMetadata, VerbPrecondition, VerbProducesSpec,
        VerbReturnSpec,
    },
    view_def::ViewColumn,
};

// -- Step 2: Default attributes -----------------------------------------------

/// Generate default `AttributeDefBody` entries from the entity type's
/// required and optional attribute FQNs.
pub fn default_attributes_for_entity_type(
    entity_type: &EntityTypeDefBody,
) -> Vec<AttributeDefBody> {
    let mut attrs = Vec::new();

    for fqn in &entity_type.required_attributes {
        attrs.push(attribute_from_fqn(fqn, &entity_type.domain, true));
    }

    for fqn in &entity_type.optional_attributes {
        attrs.push(attribute_from_fqn(fqn, &entity_type.domain, false));
    }

    attrs
}

fn attribute_from_fqn(fqn: &str, domain: &str, required: bool) -> AttributeDefBody {
    let short_name = fqn
        .rsplit_once('.')
        .map(|(_, name)| name)
        .unwrap_or(fqn)
        .replace('-', " ");

    let name = short_name
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().to_string() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    AttributeDefBody {
        fqn: fqn.to_string(),
        name,
        description: format!("Attribute {fqn}"),
        domain: domain.to_string(),
        data_type: AttributeDataType::String,
        source: None,
        constraints: if required {
            Some(AttributeConstraints {
                required: true,
                unique: false,
                min_length: None,
                max_length: None,
                pattern: None,
                valid_values: None,
            })
        } else {
            None
        },
        sinks: vec![],
    }
}

// -- Step 3: Default verb contracts -------------------------------------------

/// Generate standard CRUD verb contracts for the entity type.
pub fn default_verb_contracts_for_entity_type(
    entity_type: &EntityTypeDefBody,
) -> Vec<VerbContractBody> {
    let domain = &entity_type.domain;
    let et_fqn = &entity_type.fqn;
    let name = &entity_type.name;

    vec![
        crud_contract(
            domain,
            "create",
            et_fqn,
            name,
            "crud",
            &entity_type.required_attributes,
        ),
        crud_contract(domain, "get", et_fqn, name, "crud", &[]),
        crud_contract(domain, "list", et_fqn, name, "crud", &[]),
        crud_contract(
            domain,
            "update",
            et_fqn,
            name,
            "crud",
            &entity_type.required_attributes,
        ),
        crud_contract(domain, "delete", et_fqn, name, "crud", &[]),
    ]
}

fn crud_contract(
    domain: &str,
    action: &str,
    entity_type_fqn: &str,
    entity_name: &str,
    behavior: &str,
    required_attr_fqns: &[String],
) -> VerbContractBody {
    let fqn = format!("{domain}.{action}");
    let description = match action {
        "create" => format!("Create a new {entity_name}"),
        "get" => format!("Get a {entity_name} by ID"),
        "list" => format!("List all {entity_name} records"),
        "update" => format!("Update an existing {entity_name}"),
        "delete" => format!("Delete a {entity_name}"),
        _ => format!("{action} a {entity_name}"),
    };

    let mut args = Vec::new();
    if action == "get" || action == "update" || action == "delete" {
        args.push(VerbArgDef {
            name: "id".to_string(),
            arg_type: "uuid".to_string(),
            required: true,
            description: Some(format!("{entity_name} ID")),
            lookup: None,
            valid_values: None,
            default: None,
        });
    }
    if action == "create" || action == "update" {
        for attr_fqn in required_attr_fqns {
            let arg_name = attr_fqn
                .rsplit_once('.')
                .map(|(_, n)| n)
                .unwrap_or(attr_fqn)
                .to_string();
            args.push(VerbArgDef {
                name: arg_name,
                arg_type: "string".to_string(),
                required: action == "create",
                description: None,
                lookup: None,
                valid_values: None,
                default: None,
            });
        }
    }

    let returns = match action {
        "create" => Some(VerbReturnSpec {
            return_type: "uuid".to_string(),
            schema: None,
        }),
        "get" => Some(VerbReturnSpec {
            return_type: "record".to_string(),
            schema: None,
        }),
        "list" => Some(VerbReturnSpec {
            return_type: "record_set".to_string(),
            schema: None,
        }),
        "update" | "delete" => Some(VerbReturnSpec {
            return_type: "affected".to_string(),
            schema: None,
        }),
        _ => None,
    };

    let produces = if action == "create" {
        Some(VerbProducesSpec {
            entity_type: entity_type_fqn.to_string(),
            resolved: false,
        })
    } else {
        None
    };

    let preconditions = if action == "get" || action == "update" || action == "delete" {
        vec![VerbPrecondition {
            kind: "requires_scope".to_string(),
            value: domain.to_string(),
            description: Some(format!("Requires {entity_name} in scope")),
        }]
    } else {
        vec![]
    };

    VerbContractBody {
        fqn,
        domain: domain.to_string(),
        action: action.to_string(),
        description,
        behavior: behavior.to_string(),
        args,
        returns,
        preconditions,
        postconditions: vec![],
        produces,
        consumes: vec![],
        invocation_phrases: vec![],
        subject_kinds: vec![],
        phase_tags: vec![],
        requires_subject: action != "create" && action != "list",
        produces_focus: action == "create",
        metadata: Some(VerbContractMetadata {
            tier: Some("intent".to_string()),
            source_of_truth: Some("operational".to_string()),
            scope: None,
            noun: None,
            tags: vec![],
            subject_kinds: vec![],
            phase_tags: vec![],
        }),
        crud_mapping: None,
    }
}

// -- Step 4: Default taxonomy FQNs --------------------------------------------

/// Derive default taxonomy FQNs for an entity type from its domain.
pub fn default_taxonomy_fqns_for_entity_type(entity_type: &EntityTypeDefBody) -> Vec<String> {
    let mut fqns = Vec::new();
    fqns.push("taxonomy.entity-classification".to_string());

    match entity_type.domain.as_str() {
        "cbu" | "entity" | "trading-profile" | "custody" => {
            fqns.push("taxonomy.domain".to_string());
        }
        "kyc" | "screening" => {
            fqns.push("taxonomy.domain".to_string());
        }
        _ => {
            fqns.push("taxonomy.domain".to_string());
        }
    }

    fqns
}

/// Create a membership rule linking an entity type to a taxonomy node.
pub fn membership_rule_for_entity_in_taxonomy(
    entity_type_fqn: &str,
    taxonomy_fqn: &str,
) -> MembershipRuleBody {
    let rule_fqn = format!("{taxonomy_fqn}.{entity_type_fqn}");

    let domain = entity_type_fqn
        .split_once('.')
        .map(|(d, _)| d)
        .unwrap_or("root");
    let node_fqn = format!("{taxonomy_fqn}.{domain}");

    MembershipRuleBody {
        fqn: rule_fqn,
        name: format!("{entity_type_fqn} in {taxonomy_fqn}"),
        description: Some(format!(
            "Places {entity_type_fqn} under {node_fqn} in {taxonomy_fqn}"
        )),
        taxonomy_fqn: taxonomy_fqn.to_string(),
        node_fqn,
        membership_kind: MembershipKind::Direct,
        target_type: "entity_type_def".to_string(),
        target_fqn: entity_type_fqn.to_string(),
        conditions: vec![],
    }
}

// -- Step 5: Default view FQNs + columns --------------------------------------

/// Derive default view FQNs to update for this entity type.
pub fn default_view_fqns_for_entity_type(entity_type: &EntityTypeDefBody) -> Vec<String> {
    match entity_type.domain.as_str() {
        "cbu" => vec!["view.trading-overview".to_string()],
        "kyc" | "screening" => vec!["view.kyc-case".to_string()],
        "entity" => vec!["view.entity-detail".to_string()],
        _ => vec![],
    }
}

/// Generate view columns for the entity type's required attributes in a given view.
pub fn columns_for_entity_in_view(
    entity_type: &EntityTypeDefBody,
    _view_fqn: &str,
) -> Vec<ViewColumn> {
    entity_type
        .required_attributes
        .iter()
        .map(|attr_fqn| {
            let short_name = attr_fqn
                .rsplit_once('.')
                .map(|(_, n)| n)
                .unwrap_or(attr_fqn)
                .replace('-', " ");
            let label = short_name
                .split_whitespace()
                .map(|w| {
                    let mut c = w.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().to_string() + c.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            ViewColumn {
                attribute_fqn: attr_fqn.clone(),
                label: Some(label),
                width: None,
                visible: true,
                format: Some("text".to_string()),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entity_type() -> EntityTypeDefBody {
        EntityTypeDefBody {
            fqn: "entity.test-widget".to_string(),
            name: "Test Widget".to_string(),
            description: "A test entity".to_string(),
            domain: "entity".to_string(),
            db_table: None,
            lifecycle_states: vec![],
            required_attributes: vec![
                "entity.widget-name".to_string(),
                "entity.widget-status".to_string(),
            ],
            optional_attributes: vec!["entity.widget-description".to_string()],
            parent_type: None,
        }
    }

    #[test]
    fn test_default_attributes_from_entity_type() {
        let et = sample_entity_type();
        let attrs = default_attributes_for_entity_type(&et);

        assert_eq!(attrs.len(), 3);
        assert_eq!(attrs[0].fqn, "entity.widget-name");
        assert_eq!(attrs[0].name, "Widget Name");
        assert!(attrs[0].constraints.as_ref().unwrap().required);

        assert_eq!(attrs[2].fqn, "entity.widget-description");
        assert!(attrs[2].constraints.is_none());
    }

    #[test]
    fn test_default_verb_contracts() {
        let et = sample_entity_type();
        let contracts = default_verb_contracts_for_entity_type(&et);

        assert_eq!(contracts.len(), 5);
        let fqns: Vec<&str> = contracts.iter().map(|c| c.fqn.as_str()).collect();
        assert!(fqns.contains(&"entity.create"));
        assert!(fqns.contains(&"entity.get"));
        assert!(fqns.contains(&"entity.list"));
        assert!(fqns.contains(&"entity.update"));
        assert!(fqns.contains(&"entity.delete"));

        let create = contracts.iter().find(|c| c.action == "create").unwrap();
        assert!(create.produces.is_some());
        assert!(create.produces_focus);
        assert!(!create.requires_subject);
    }

    #[test]
    fn test_default_taxonomy_fqns() {
        let et = sample_entity_type();
        let fqns = default_taxonomy_fqns_for_entity_type(&et);
        assert!(fqns.contains(&"taxonomy.entity-classification".to_string()));
        assert!(fqns.contains(&"taxonomy.domain".to_string()));
    }

    #[test]
    fn test_membership_rule_construction() {
        let rule = membership_rule_for_entity_in_taxonomy("entity.test-widget", "taxonomy.domain");
        assert_eq!(rule.fqn, "taxonomy.domain.entity.test-widget");
        assert_eq!(rule.taxonomy_fqn, "taxonomy.domain");
        assert_eq!(rule.node_fqn, "taxonomy.domain.entity");
        assert_eq!(rule.target_fqn, "entity.test-widget");
        assert_eq!(rule.membership_kind, MembershipKind::Direct);
    }

    #[test]
    fn test_default_view_fqns_by_domain() {
        let mut et = sample_entity_type();

        let fqns = default_view_fqns_for_entity_type(&et);
        assert_eq!(fqns, vec!["view.entity-detail"]);

        et.domain = "cbu".to_string();
        let fqns = default_view_fqns_for_entity_type(&et);
        assert_eq!(fqns, vec!["view.trading-overview"]);

        et.domain = "kyc".to_string();
        let fqns = default_view_fqns_for_entity_type(&et);
        assert_eq!(fqns, vec!["view.kyc-case"]);

        et.domain = "unknown".to_string();
        let fqns = default_view_fqns_for_entity_type(&et);
        assert!(fqns.is_empty());
    }

    #[test]
    fn test_columns_for_entity_in_view() {
        let et = sample_entity_type();
        let cols = columns_for_entity_in_view(&et, "view.entity-detail");
        assert_eq!(cols.len(), 2);
        assert_eq!(cols[0].attribute_fqn, "entity.widget-name");
        assert_eq!(cols[0].label.as_deref(), Some("Widget Name"));
        assert!(cols[0].visible);
    }

    #[test]
    fn test_attribute_name_title_casing() {
        let attr = attribute_from_fqn("test.some-long-name", "test", false);
        assert_eq!(attr.name, "Some Long Name");
    }
}
