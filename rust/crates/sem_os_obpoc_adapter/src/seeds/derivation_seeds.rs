//! Pure derivation spec seed builders for the semantic registry.
//!
//! All functions are **pure** (no DB, no I/O). The DB-publishing orchestrator
//! remains in `ob-poc/src/sem_reg/seeds/derivation_seeds.rs`.

use sem_os_core::derivation_spec::{
    DerivationExpression, DerivationInput, DerivationSpecBody, DerivationTestCase, EvidenceGrade,
    FreshnessRule, NullSemantics, SecurityInheritanceMode,
};

/// Core derivation specs to bootstrap.
pub fn core_derivation_specs() -> Vec<DerivationSpecBody> {
    vec![
        ubo_percentage_derivation(),
        composite_risk_score_derivation(),
        document_completeness_derivation(),
        beneficial_ownership_flag_derivation(),
        aggregate_aum_derivation(),
    ]
}

pub fn ubo_percentage_derivation() -> DerivationSpecBody {
    DerivationSpecBody {
        fqn: "ubo.total_ownership_pct".into(),
        name: "UBO Total Ownership Percentage".into(),
        description: "Computes total beneficial ownership percentage by summing control chain ownership paths from an entity through intermediate holders.".into(),
        output_attribute_fqn: "ubo.total_ownership_pct_value".into(),
        inputs: vec![
            DerivationInput {
                attribute_fqn: "ubo.direct_holding_pct".into(),
                role: "primary".into(),
                required: true,
            },
            DerivationInput {
                attribute_fqn: "ubo.indirect_holding_pct".into(),
                role: "secondary".into(),
                required: false,
            },
        ],
        expression: DerivationExpression::FunctionRef {
            ref_name: "sum_ownership_chain".into(),
        },
        null_semantics: NullSemantics::Default(serde_json::json!(0.0)),
        freshness_rule: Some(FreshnessRule { max_age_seconds: 86400 }),
        security_inheritance: SecurityInheritanceMode::Strict,
        evidence_grade: EvidenceGrade::AllowedWithConstraints,
        tests: vec![
            DerivationTestCase {
                inputs: serde_json::json!({
                    "direct_holding_pct": 25.0,
                    "indirect_holding_pct": 10.0
                }),
                expected: serde_json::json!(35.0),
            },
            DerivationTestCase {
                inputs: serde_json::json!({
                    "direct_holding_pct": 50.0,
                    "indirect_holding_pct": null
                }),
                expected: serde_json::json!(50.0),
            },
        ],
    }
}

pub fn composite_risk_score_derivation() -> DerivationSpecBody {
    DerivationSpecBody {
        fqn: "risk.composite_score".into(),
        name: "Composite Risk Score".into(),
        description: "Weighted average of individual risk factors (credit, market, operational) to produce a single composite risk score.".into(),
        output_attribute_fqn: "risk.composite_score_value".into(),
        inputs: vec![
            DerivationInput {
                attribute_fqn: "risk.credit_score".into(),
                role: "primary".into(),
                required: true,
            },
            DerivationInput {
                attribute_fqn: "risk.market_volatility".into(),
                role: "secondary".into(),
                required: false,
            },
            DerivationInput {
                attribute_fqn: "risk.operational_score".into(),
                role: "weight".into(),
                required: false,
            },
        ],
        expression: DerivationExpression::FunctionRef {
            ref_name: "weighted_average".into(),
        },
        null_semantics: NullSemantics::Propagate,
        freshness_rule: Some(FreshnessRule { max_age_seconds: 3600 }),
        security_inheritance: SecurityInheritanceMode::Strict,
        evidence_grade: EvidenceGrade::Prohibited,
        tests: vec![DerivationTestCase {
            inputs: serde_json::json!({
                "credit_score": 750,
                "market_volatility": 0.15,
                "operational_score": 0.8
            }),
            expected: serde_json::json!(0.65),
        }],
    }
}

pub fn document_completeness_derivation() -> DerivationSpecBody {
    DerivationSpecBody {
        fqn: "kyc.document_completeness_pct".into(),
        name: "Document Completeness Percentage".into(),
        description: "Computes the percentage of required documents that have been received and verified for a given entity or case.".into(),
        output_attribute_fqn: "kyc.document_completeness_pct_value".into(),
        inputs: vec![
            DerivationInput {
                attribute_fqn: "kyc.required_document_count".into(),
                role: "denominator".into(),
                required: true,
            },
            DerivationInput {
                attribute_fqn: "kyc.verified_document_count".into(),
                role: "numerator".into(),
                required: true,
            },
        ],
        expression: DerivationExpression::FunctionRef {
            ref_name: "percentage_ratio".into(),
        },
        null_semantics: NullSemantics::Error,
        freshness_rule: None,
        security_inheritance: SecurityInheritanceMode::Strict,
        evidence_grade: EvidenceGrade::AllowedWithConstraints,
        tests: vec![
            DerivationTestCase {
                inputs: serde_json::json!({
                    "required_document_count": 10,
                    "verified_document_count": 7
                }),
                expected: serde_json::json!(70.0),
            },
            DerivationTestCase {
                inputs: serde_json::json!({
                    "required_document_count": 5,
                    "verified_document_count": 5
                }),
                expected: serde_json::json!(100.0),
            },
        ],
    }
}

pub fn beneficial_ownership_flag_derivation() -> DerivationSpecBody {
    DerivationSpecBody {
        fqn: "ubo.is_beneficial_owner".into(),
        name: "Beneficial Ownership Flag".into(),
        description: "Determines whether an entity qualifies as a beneficial owner based on whether their total ownership percentage exceeds the regulatory threshold (typically 25%).".into(),
        output_attribute_fqn: "ubo.is_beneficial_owner_flag".into(),
        inputs: vec![DerivationInput {
            attribute_fqn: "ubo.total_ownership_pct_value".into(),
            role: "primary".into(),
            required: true,
        }],
        expression: DerivationExpression::FunctionRef {
            ref_name: "threshold_flag".into(),
        },
        null_semantics: NullSemantics::Default(serde_json::json!(false)),
        freshness_rule: Some(FreshnessRule { max_age_seconds: 86400 }),
        security_inheritance: SecurityInheritanceMode::Strict,
        evidence_grade: EvidenceGrade::AllowedWithConstraints,
        tests: vec![
            DerivationTestCase {
                inputs: serde_json::json!({"total_ownership_pct_value": 30.0}),
                expected: serde_json::json!(true),
            },
            DerivationTestCase {
                inputs: serde_json::json!({"total_ownership_pct_value": 15.0}),
                expected: serde_json::json!(false),
            },
        ],
    }
}

pub fn aggregate_aum_derivation() -> DerivationSpecBody {
    DerivationSpecBody {
        fqn: "trading.aggregate_aum".into(),
        name: "Aggregate Assets Under Management".into(),
        description: "Sums assets under management across all CBUs in a client book to produce total AUM for billing and reporting.".into(),
        output_attribute_fqn: "trading.aggregate_aum_value".into(),
        inputs: vec![DerivationInput {
            attribute_fqn: "trading.cbu_aum".into(),
            role: "addend".into(),
            required: true,
        }],
        expression: DerivationExpression::FunctionRef {
            ref_name: "sum_aggregate".into(),
        },
        null_semantics: NullSemantics::Skip,
        freshness_rule: Some(FreshnessRule { max_age_seconds: 3600 }),
        security_inheritance: SecurityInheritanceMode::Strict,
        evidence_grade: EvidenceGrade::Prohibited,
        tests: vec![DerivationTestCase {
            inputs: serde_json::json!({
                "cbu_aum": [1000000, 2500000, 500000]
            }),
            expected: serde_json::json!(4000000),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_derivation_specs_well_formed() {
        let specs = core_derivation_specs();
        assert_eq!(specs.len(), 5, "Expected 5 core derivation specs");

        for s in &specs {
            assert!(
                s.fqn.contains('.'),
                "Derivation FQN should be dot-separated: {}",
                s.fqn
            );
            assert!(!s.name.is_empty());
            assert!(!s.description.is_empty());
            assert!(!s.output_attribute_fqn.is_empty());
            assert!(!s.inputs.is_empty());
            assert!(!s.tests.is_empty());
        }
    }

    #[test]
    fn test_derivation_fqns_unique() {
        let specs = core_derivation_specs();
        let mut fqns: Vec<&str> = specs.iter().map(|s| s.fqn.as_str()).collect();
        let original_len = fqns.len();
        fqns.sort();
        fqns.dedup();
        assert_eq!(fqns.len(), original_len, "Duplicate derivation FQNs found");
    }

    #[test]
    fn test_derivation_serde_round_trip() {
        for s in &core_derivation_specs() {
            let json = serde_json::to_value(s).unwrap();
            let back: DerivationSpecBody = serde_json::from_value(json).unwrap();
            assert_eq!(back.fqn, s.fqn);
            assert_eq!(back.inputs.len(), s.inputs.len());
            assert_eq!(back.tests.len(), s.tests.len());
        }
    }
}
