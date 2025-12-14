//! Trading Profile Validation
//!
//! Validates trading profile documents before materialization.
//! Checks internal consistency (e.g., CSA SSI refs exist in standing_instructions).

use std::collections::HashSet;

use super::types::TradingProfileDocument;

/// Validation error for trading profile documents
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub value: String,
    pub expected_in: String,
    pub context: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: '{}' not found in {} ({})",
            self.field, self.value, self.expected_in, self.context
        )
    }
}

/// Validate CSA collateral_ssi_ref values exist in standing_instructions
///
/// Each CSA that has a `collateral_ssi_ref` must reference an SSI name
/// that exists in the `standing_instructions` section.
pub fn validate_csa_ssi_refs(doc: &TradingProfileDocument) -> Vec<ValidationError> {
    // Collect all SSI names from standing_instructions
    let mut ssi_names: HashSet<&str> = HashSet::new();

    for (_category, ssis) in doc.standing_instructions.iter() {
        for ssi in ssis {
            ssi_names.insert(&ssi.name);
        }
    }

    // Check all CSA collateral_ssi_ref values exist
    let mut errors = vec![];

    for isda in &doc.isda_agreements {
        if let Some(ref csa) = isda.csa {
            if let Some(ref ssi_ref) = csa.collateral_ssi_ref {
                if !ssi_names.contains(ssi_ref.as_str()) {
                    errors.push(ValidationError {
                        field: "csa.collateral_ssi_ref".to_string(),
                        value: ssi_ref.clone(),
                        expected_in: "standing_instructions".to_string(),
                        context: format!("ISDA with counterparty {}", isda.counterparty.value),
                    });
                }
            }
        }
    }

    errors
}

/// Validate booking rule SSI refs exist in standing_instructions
pub fn validate_booking_rule_ssi_refs(doc: &TradingProfileDocument) -> Vec<ValidationError> {
    // Collect all SSI names from standing_instructions
    let mut ssi_names: HashSet<&str> = HashSet::new();

    for (_category, ssis) in doc.standing_instructions.iter() {
        for ssi in ssis {
            ssi_names.insert(&ssi.name);
        }
    }

    // Check all booking rule ssi_ref values exist
    let mut errors = vec![];

    for rule in &doc.booking_rules {
        if !ssi_names.contains(rule.ssi_ref.as_str()) {
            errors.push(ValidationError {
                field: "booking_rule.ssi_ref".to_string(),
                value: rule.ssi_ref.clone(),
                expected_in: "standing_instructions".to_string(),
                context: format!("Booking rule '{}'", rule.name),
            });
        }
    }

    errors
}

/// Run all validations on a trading profile document
pub fn validate_document(doc: &TradingProfileDocument) -> Vec<ValidationError> {
    let mut errors = vec![];
    errors.extend(validate_csa_ssi_refs(doc));
    errors.extend(validate_booking_rule_ssi_refs(doc));
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trading_profile::types::{
        BookingMatch, BookingRule, CsaConfig, EntityRef, EntityRefType, IsdaAgreementConfig,
        StandingInstruction, Universe,
    };
    use std::collections::HashMap;

    fn make_test_doc() -> TradingProfileDocument {
        let mut standing_instructions = HashMap::new();
        standing_instructions.insert(
            "CUSTODY".to_string(),
            vec![StandingInstruction {
                name: "DEFAULT_SSI".to_string(),
                mic: None,
                currency: None,
                custody_account: None,
                custody_bic: None,
                cash_account: None,
                cash_bic: None,
                settlement_model: None,
                cutoff: None,
                counterparty: None,
                counterparty_lei: None,
                provider_ref: None,
                channel: None,
                reporting_frequency: None,
            }],
        );
        standing_instructions.insert(
            "OTC_COLLATERAL".to_string(),
            vec![StandingInstruction {
                name: "GS_COLLATERAL_SSI".to_string(),
                mic: None,
                currency: Some("USD".to_string()),
                custody_account: Some("COLL-001".to_string()),
                custody_bic: Some("IRVTUS3N".to_string()),
                cash_account: Some("CASH-001".to_string()),
                cash_bic: Some("IRVTUS3N".to_string()),
                settlement_model: None,
                cutoff: None,
                counterparty: Some(EntityRef {
                    ref_type: EntityRefType::Lei,
                    value: "W22LROWP2IHZNBB6K528".to_string(),
                }),
                counterparty_lei: None,
                provider_ref: None,
                channel: None,
                reporting_frequency: None,
            }],
        );

        TradingProfileDocument {
            universe: Universe {
                base_currency: "USD".to_string(),
                allowed_currencies: vec![],
                allowed_markets: vec![],
                instrument_classes: vec![],
            },
            investment_managers: vec![],
            isda_agreements: vec![IsdaAgreementConfig {
                counterparty: EntityRef {
                    ref_type: EntityRefType::Lei,
                    value: "W22LROWP2IHZNBB6K528".to_string(),
                },
                agreement_date: "2020-01-01".to_string(),
                governing_law: "ENGLISH".to_string(),
                effective_date: None,
                product_coverage: vec![],
                csa: Some(CsaConfig {
                    csa_type: "VM".to_string(),
                    threshold_amount: None,
                    threshold_currency: None,
                    minimum_transfer_amount: None,
                    rounding_amount: None,
                    eligible_collateral: vec![],
                    initial_margin: None,
                    collateral_ssi_ref: Some("GS_COLLATERAL_SSI".to_string()),
                    collateral_ssi: None,
                    valuation_time: None,
                    valuation_timezone: None,
                    notification_time: None,
                    settlement_days: None,
                    dispute_resolution: None,
                }),
            }],
            settlement_config: None,
            booking_rules: vec![BookingRule {
                name: "Default Rule".to_string(),
                priority: 100,
                match_criteria: BookingMatch::default(),
                ssi_ref: "DEFAULT_SSI".to_string(),
            }],
            standing_instructions,
            pricing_matrix: vec![],
            valuation_config: None,
            constraints: None,
            metadata: None,
        }
    }

    #[test]
    fn test_valid_document_passes() {
        let doc = make_test_doc();
        let errors = validate_document(&doc);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_missing_csa_ssi_ref_fails() {
        let mut doc = make_test_doc();
        // Change the ref to something that doesn't exist
        if let Some(ref mut csa) = doc.isda_agreements[0].csa {
            csa.collateral_ssi_ref = Some("NONEXISTENT_SSI".to_string());
        }

        let errors = validate_document(&doc);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].value.contains("NONEXISTENT"));
    }

    #[test]
    fn test_missing_booking_rule_ssi_ref_fails() {
        let mut doc = make_test_doc();
        doc.booking_rules[0].ssi_ref = "MISSING_SSI".to_string();

        let errors = validate_document(&doc);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].value.contains("MISSING_SSI"));
    }
}
