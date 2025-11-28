//! Intent validation
//!
//! Validates semantic correctness of intents beyond JSON schema.

use super::*;

/// Validation error for intents
#[derive(Debug, Clone)]
pub struct IntentValidationError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for IntentValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for IntentValidationError {}

/// Validate an intent for semantic correctness
pub fn validate_intent(intent: &KycIntent) -> Result<(), Vec<IntentValidationError>> {
    let mut errors = Vec::new();

    match intent {
        KycIntent::OnboardIndividual { client, documents, .. } => {
            validate_individual_client(client, &mut errors);
            for (i, doc) in documents.iter().enumerate() {
                validate_document_spec(doc, &format!("documents[{}]", i), &mut errors);
            }
        }

        KycIntent::OnboardCorporate { 
            client, 
            beneficial_owners, 
            directors,
            .. 
        } => {
            validate_corporate_client(client, &mut errors);
            
            // Validate UBO percentages
            let total_ownership: f64 = beneficial_owners
                .iter()
                .filter(|o| o.is_direct)
                .map(|o| o.ownership_percentage)
                .sum();
            
            if total_ownership > 100.0 {
                errors.push(IntentValidationError {
                    field: "beneficial_owners".to_string(),
                    message: format!(
                        "Direct ownership percentages sum to {}%, exceeds 100%",
                        total_ownership
                    ),
                });
            }

            for (i, owner) in beneficial_owners.iter().enumerate() {
                validate_beneficial_owner(owner, &format!("beneficial_owners[{}]", i), &mut errors);
            }

            for (i, director) in directors.iter().enumerate() {
                if director.name.trim().is_empty() {
                    errors.push(IntentValidationError {
                        field: format!("directors[{}].name", i),
                        message: "Director name cannot be empty".to_string(),
                    });
                }
            }
        }

        KycIntent::AddBeneficialOwner { owner, .. } => {
            validate_beneficial_owner(owner, "owner", &mut errors);
        }

        KycIntent::UpdateRiskRating { rationale, risk_rating, .. } => {
            // High/Prohibited ratings should have rationale
            if matches!(risk_rating, RiskRating::High | RiskRating::Prohibited) 
                && rationale.is_none() 
            {
                errors.push(IntentValidationError {
                    field: "rationale".to_string(),
                    message: "Rationale required for High/Prohibited risk ratings".to_string(),
                });
            }
        }

        _ => {}
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_individual_client(client: &IndividualClient, errors: &mut Vec<IntentValidationError>) {
    if client.name.trim().is_empty() {
        errors.push(IntentValidationError {
            field: "client.name".to_string(),
            message: "Client name cannot be empty".to_string(),
        });
    }

    if let Some(nationality) = &client.nationality {
        if nationality.len() != 3 {
            errors.push(IntentValidationError {
                field: "client.nationality".to_string(),
                message: "Nationality should be ISO 3166-1 alpha-3 code (3 characters)".to_string(),
            });
        }
    }
}

fn validate_corporate_client(client: &CorporateClient, errors: &mut Vec<IntentValidationError>) {
    if client.name.trim().is_empty() {
        errors.push(IntentValidationError {
            field: "client.name".to_string(),
            message: "Company name cannot be empty".to_string(),
        });
    }

    if let Some(lei) = &client.lei_code {
        if lei.len() != 20 {
            errors.push(IntentValidationError {
                field: "client.lei_code".to_string(),
                message: "LEI code must be exactly 20 characters".to_string(),
            });
        }
    }
}

fn validate_document_spec(doc: &DocumentSpec, prefix: &str, errors: &mut Vec<IntentValidationError>) {
    if doc.document_type.trim().is_empty() {
        errors.push(IntentValidationError {
            field: format!("{}.document_type", prefix),
            message: "Document type cannot be empty".to_string(),
        });
    }

    // Document type should be uppercase with underscores
    if !doc.document_type.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
        errors.push(IntentValidationError {
            field: format!("{}.document_type", prefix),
            message: "Document type should be uppercase (e.g., PASSPORT_GBR)".to_string(),
        });
    }
}

fn validate_beneficial_owner(
    owner: &BeneficialOwnerSpec, 
    prefix: &str, 
    errors: &mut Vec<IntentValidationError>
) {
    if owner.name.trim().is_empty() {
        errors.push(IntentValidationError {
            field: format!("{}.name", prefix),
            message: "Beneficial owner name cannot be empty".to_string(),
        });
    }

    if owner.ownership_percentage < 0.0 || owner.ownership_percentage > 100.0 {
        errors.push(IntentValidationError {
            field: format!("{}.ownership_percentage", prefix),
            message: "Ownership percentage must be between 0 and 100".to_string(),
        });
    }

    // Indirect ownership requires via_entity
    if !owner.is_direct && owner.via_entity.is_none() {
        errors.push(IntentValidationError {
            field: format!("{}.via_entity", prefix),
            message: "Indirect ownership requires via_entity to be specified".to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_client_name() {
        let intent = KycIntent::OnboardIndividual {
            client: IndividualClient {
                name: "   ".to_string(),
                jurisdiction: None,
                nationality: None,
                date_of_birth: None,
                tax_residency: None,
                occupation: None,
                source_of_wealth: None,
            },
            documents: vec![],
            contact: None,
        };

        let result = validate_intent(&intent);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.field == "client.name"));
    }

    #[test]
    fn test_validate_ubo_percentage_overflow() {
        let intent = KycIntent::OnboardCorporate {
            client: CorporateClient {
                name: "Test Corp".to_string(),
                jurisdiction: None,
                registration_number: None,
                entity_type: None,
                incorporation_date: None,
                registered_address: None,
                trading_address: None,
                industry_sector: None,
                lei_code: None,
            },
            documents: vec![],
            beneficial_owners: vec![
                BeneficialOwnerSpec {
                    name: "Owner 1".to_string(),
                    ownership_percentage: 60.0,
                    nationality: None,
                    date_of_birth: None,
                    is_direct: true,
                    via_entity: None,
                    control_type: None,
                },
                BeneficialOwnerSpec {
                    name: "Owner 2".to_string(),
                    ownership_percentage: 50.0,
                    nationality: None,
                    date_of_birth: None,
                    is_direct: true,
                    via_entity: None,
                    control_type: None,
                },
            ],
            directors: vec![],
            authorized_signatories: vec![],
        };

        let result = validate_intent(&intent);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("exceeds 100%")));
    }

    #[test]
    fn test_validate_indirect_ownership_needs_via_entity() {
        let intent = KycIntent::AddBeneficialOwner {
            cbu_id: CbuReference::ByCode { code: "TEST".to_string() },
            owner: BeneficialOwnerSpec {
                name: "Indirect Owner".to_string(),
                ownership_percentage: 25.0,
                nationality: None,
                date_of_birth: None,
                is_direct: false,  // indirect
                via_entity: None,  // but no via_entity!
                control_type: None,
            },
            evidence_documents: vec![],
        };

        let result = validate_intent(&intent);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("via_entity")));
    }
}
