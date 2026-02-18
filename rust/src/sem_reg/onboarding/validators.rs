//! Validation for onboarding requests.
//!
//! Ensures structural correctness before any snapshots are published.

use anyhow::{bail, Result};

use super::pipeline::OnboardingRequest;

/// Validate the onboarding request for structural correctness.
pub fn validate_request(request: &OnboardingRequest) -> Result<()> {
    validate_entity_type(request)?;
    validate_attributes(request)?;
    validate_verb_contracts(request)?;
    validate_evidence_requirements(request)?;
    Ok(())
}

fn validate_entity_type(request: &OnboardingRequest) -> Result<()> {
    let et = &request.entity_type;

    if et.fqn.is_empty() {
        bail!("Entity type FQN is required");
    }
    if !et.fqn.contains('.') {
        bail!(
            "Entity type FQN must be domain-qualified (e.g., 'entity.fund'), got '{}'",
            et.fqn
        );
    }
    if et.name.is_empty() {
        bail!("Entity type name is required");
    }
    if et.domain.is_empty() {
        bail!("Entity type domain is required");
    }
    if et.description.is_empty() {
        bail!("Entity type description is required");
    }

    Ok(())
}

fn validate_attributes(request: &OnboardingRequest) -> Result<()> {
    for attr in &request.attributes {
        if attr.fqn.is_empty() {
            bail!("Attribute FQN is required");
        }
        if !attr.fqn.contains('.') {
            bail!(
                "Attribute FQN must be domain-qualified (e.g., 'cbu.name'), got '{}'",
                attr.fqn
            );
        }
        if attr.name.is_empty() {
            bail!("Attribute name is required for '{}'", attr.fqn);
        }
    }

    // Check for duplicates
    let mut seen = std::collections::HashSet::new();
    for attr in &request.attributes {
        if !seen.insert(&attr.fqn) {
            bail!("Duplicate attribute FQN: '{}'", attr.fqn);
        }
    }

    Ok(())
}

fn validate_verb_contracts(request: &OnboardingRequest) -> Result<()> {
    for vc in &request.verb_contracts {
        if vc.fqn.is_empty() {
            bail!("Verb contract FQN is required");
        }
        if !vc.fqn.contains('.') {
            bail!(
                "Verb contract FQN must be domain-qualified (e.g., 'cbu.create'), got '{}'",
                vc.fqn
            );
        }
        if vc.domain.is_empty() {
            bail!("Verb contract domain is required for '{}'", vc.fqn);
        }
        if vc.action.is_empty() {
            bail!("Verb contract action is required for '{}'", vc.fqn);
        }
    }

    let mut seen = std::collections::HashSet::new();
    for vc in &request.verb_contracts {
        if !seen.insert(&vc.fqn) {
            bail!("Duplicate verb contract FQN: '{}'", vc.fqn);
        }
    }

    Ok(())
}

fn validate_evidence_requirements(request: &OnboardingRequest) -> Result<()> {
    for er in &request.evidence_requirements {
        if er.fqn.is_empty() {
            bail!("Evidence requirement FQN is required");
        }
        if er.target_entity_type.is_empty() {
            bail!(
                "Evidence requirement target_entity_type is required for '{}'",
                er.fqn
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sem_reg::attribute_def::{AttributeDataType, AttributeDefBody};
    use crate::sem_reg::entity_type_def::EntityTypeDefBody;

    fn minimal_entity_type() -> EntityTypeDefBody {
        EntityTypeDefBody {
            fqn: "entity.test".to_string(),
            name: "Test".to_string(),
            description: "A test entity".to_string(),
            domain: "entity".to_string(),
            db_table: None,
            lifecycle_states: vec![],
            required_attributes: vec![],
            optional_attributes: vec![],
            parent_type: None,
        }
    }

    fn minimal_request() -> OnboardingRequest {
        OnboardingRequest {
            entity_type: minimal_entity_type(),
            attributes: vec![],
            verb_contracts: vec![],
            taxonomy_fqns: vec![],
            view_fqns: vec![],
            evidence_requirements: vec![],
            dry_run: true,
            created_by: "test".to_string(),
        }
    }

    #[test]
    fn test_valid_minimal_request() {
        let req = minimal_request();
        assert!(validate_request(&req).is_ok());
    }

    #[test]
    fn test_empty_fqn_rejected() {
        let mut req = minimal_request();
        req.entity_type.fqn = String::new();
        let err = validate_request(&req).unwrap_err();
        assert!(err.to_string().contains("FQN is required"));
    }

    #[test]
    fn test_unqualified_fqn_rejected() {
        let mut req = minimal_request();
        req.entity_type.fqn = "test".to_string();
        let err = validate_request(&req).unwrap_err();
        assert!(err.to_string().contains("domain-qualified"));
    }

    #[test]
    fn test_empty_name_rejected() {
        let mut req = minimal_request();
        req.entity_type.name = String::new();
        let err = validate_request(&req).unwrap_err();
        assert!(err.to_string().contains("name is required"));
    }

    #[test]
    fn test_empty_domain_rejected() {
        let mut req = minimal_request();
        req.entity_type.domain = String::new();
        let err = validate_request(&req).unwrap_err();
        assert!(err.to_string().contains("domain is required"));
    }

    #[test]
    fn test_duplicate_attribute_fqn_rejected() {
        let attr = AttributeDefBody {
            fqn: "test.name".to_string(),
            name: "Name".to_string(),
            description: "A name".to_string(),
            domain: "test".to_string(),
            data_type: AttributeDataType::String,
            source: None,
            constraints: None,
            sinks: vec![],
        };
        let mut req = minimal_request();
        req.attributes = vec![attr.clone(), attr];
        let err = validate_request(&req).unwrap_err();
        assert!(err.to_string().contains("Duplicate attribute FQN"));
    }

    #[test]
    fn test_attribute_without_fqn_rejected() {
        let attr = AttributeDefBody {
            fqn: String::new(),
            name: "Name".to_string(),
            description: "A name".to_string(),
            domain: "test".to_string(),
            data_type: AttributeDataType::String,
            source: None,
            constraints: None,
            sinks: vec![],
        };
        let mut req = minimal_request();
        req.attributes = vec![attr];
        let err = validate_request(&req).unwrap_err();
        assert!(err.to_string().contains("Attribute FQN is required"));
    }

    #[test]
    fn test_attribute_unqualified_fqn_rejected() {
        let attr = AttributeDefBody {
            fqn: "name".to_string(),
            name: "Name".to_string(),
            description: "A name".to_string(),
            domain: "test".to_string(),
            data_type: AttributeDataType::String,
            source: None,
            constraints: None,
            sinks: vec![],
        };
        let mut req = minimal_request();
        req.attributes = vec![attr];
        let err = validate_request(&req).unwrap_err();
        assert!(err.to_string().contains("domain-qualified"));
    }

    #[test]
    fn test_valid_with_attributes() {
        let attr = AttributeDefBody {
            fqn: "test.name".to_string(),
            name: "Name".to_string(),
            description: "A name".to_string(),
            domain: "test".to_string(),
            data_type: AttributeDataType::String,
            source: None,
            constraints: None,
            sinks: vec![],
        };
        let mut req = minimal_request();
        req.attributes = vec![attr];
        assert!(validate_request(&req).is_ok());
    }
}
