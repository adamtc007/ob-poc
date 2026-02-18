//! Bootstrap derivation spec definitions for the semantic registry.
//!
//! Seeds derivation specs for composite/derived attributes:
//! - UBO percentage (sum of control chain percentages)
//! - Composite risk score (weighted average of risk factors)
//! - Document completeness (percentage of required docs)
//! - Beneficial ownership flag (threshold-based)
//! - Aggregate AUM (sum across CBU portfolio)

use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::sem_reg::{
    derivation_spec::{
        DerivationExpression, DerivationInput, DerivationSpecBody, DerivationTestCase,
        EvidenceGrade, FreshnessRule, NullSemantics, SecurityInheritanceMode,
    },
    ids::{definition_hash, object_id_for},
    store::SnapshotStore,
    types::{ChangeType, ObjectType, SnapshotMeta},
};

/// Report from derivation spec seeding.
#[derive(Debug, Default)]
pub struct DerivationSeedReport {
    pub derivations_published: usize,
    pub derivations_skipped: usize,
    pub derivations_updated: usize,
}

impl std::fmt::Display for DerivationSeedReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Derivation specs: {} published, {} updated, {} skipped",
            self.derivations_published, self.derivations_updated, self.derivations_skipped,
        )
    }
}

/// Core derivation specs to bootstrap.
fn core_derivation_specs() -> Vec<DerivationSpecBody> {
    vec![
        ubo_percentage_derivation(),
        composite_risk_score_derivation(),
        document_completeness_derivation(),
        beneficial_ownership_flag_derivation(),
        aggregate_aum_derivation(),
    ]
}

/// UBO percentage: sum of control chain ownership percentages.
fn ubo_percentage_derivation() -> DerivationSpecBody {
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
        freshness_rule: Some(FreshnessRule {
            max_age_seconds: 86400, // 24 hours
        }),
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

/// Composite risk score: weighted average of risk factors.
fn composite_risk_score_derivation() -> DerivationSpecBody {
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
        freshness_rule: Some(FreshnessRule {
            max_age_seconds: 3600, // 1 hour
        }),
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

/// Document completeness: percentage of required documents that are verified.
fn document_completeness_derivation() -> DerivationSpecBody {
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

/// Beneficial ownership flag: true if total ownership exceeds threshold.
fn beneficial_ownership_flag_derivation() -> DerivationSpecBody {
    DerivationSpecBody {
        fqn: "ubo.is_beneficial_owner".into(),
        name: "Beneficial Ownership Flag".into(),
        description: "Determines whether an entity qualifies as a beneficial owner based on whether their total ownership percentage exceeds the regulatory threshold (typically 25%).".into(),
        output_attribute_fqn: "ubo.is_beneficial_owner_flag".into(),
        inputs: vec![
            DerivationInput {
                attribute_fqn: "ubo.total_ownership_pct_value".into(),
                role: "primary".into(),
                required: true,
            },
        ],
        expression: DerivationExpression::FunctionRef {
            ref_name: "threshold_flag".into(),
        },
        null_semantics: NullSemantics::Default(serde_json::json!(false)),
        freshness_rule: Some(FreshnessRule {
            max_age_seconds: 86400, // 24 hours
        }),
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

/// Aggregate AUM: sum of assets across CBU portfolio.
fn aggregate_aum_derivation() -> DerivationSpecBody {
    DerivationSpecBody {
        fqn: "trading.aggregate_aum".into(),
        name: "Aggregate Assets Under Management".into(),
        description: "Sums assets under management across all CBUs in a client book to produce total AUM for billing and reporting.".into(),
        output_attribute_fqn: "trading.aggregate_aum_value".into(),
        inputs: vec![
            DerivationInput {
                attribute_fqn: "trading.cbu_aum".into(),
                role: "addend".into(),
                required: true,
            },
        ],
        expression: DerivationExpression::FunctionRef {
            ref_name: "sum_aggregate".into(),
        },
        null_semantics: NullSemantics::Skip,
        freshness_rule: Some(FreshnessRule {
            max_age_seconds: 3600, // 1 hour
        }),
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

/// Seed core derivation specs into the registry.
///
/// Uses the same idempotent publish pattern as other seeds.
pub async fn seed_derivation_specs(
    pool: &PgPool,
    set_id: Uuid,
    dry_run: bool,
    verbose: bool,
) -> Result<DerivationSeedReport> {
    let mut report = DerivationSeedReport::default();
    let specs = core_derivation_specs();

    if dry_run {
        report.derivations_published = specs.len();
        if verbose {
            for s in &specs {
                println!("  [DRY] derivation: {}", s.fqn);
            }
        }
        return Ok(report);
    }

    for spec in &specs {
        publish_idempotent(
            pool,
            &spec.fqn,
            &serde_json::to_value(spec)?,
            set_id,
            verbose,
            &mut report.derivations_published,
            &mut report.derivations_updated,
            &mut report.derivations_skipped,
        )
        .await?;
    }

    Ok(report)
}

/// Idempotent publish for derivation specs.
#[allow(clippy::too_many_arguments)]
async fn publish_idempotent(
    pool: &PgPool,
    fqn: &str,
    definition: &serde_json::Value,
    set_id: Uuid,
    verbose: bool,
    published: &mut usize,
    updated: &mut usize,
    skipped: &mut usize,
) -> Result<()> {
    let existing = SnapshotStore::find_active_by_definition_field(
        pool,
        ObjectType::DerivationSpec,
        "fqn",
        fqn,
    )
    .await?;

    let object_id = object_id_for(ObjectType::DerivationSpec, fqn);
    let new_hash = definition_hash(definition);

    if let Some(existing_row) = existing {
        let old_hash = definition_hash(&existing_row.definition);
        if old_hash == new_hash {
            *skipped += 1;
            if verbose {
                println!("  SKIP derivation: {} (unchanged)", fqn);
            }
        } else {
            let mut meta =
                SnapshotMeta::new_operational(ObjectType::DerivationSpec, object_id, "seed");
            meta.predecessor_id = Some(existing_row.snapshot_id);
            meta.version_major = existing_row.version_major;
            meta.version_minor = existing_row.version_minor + 1;
            meta.change_type = ChangeType::NonBreaking;
            meta.change_rationale = Some("Seed definition update".into());
            SnapshotStore::publish_snapshot(pool, &meta, definition, Some(set_id)).await?;
            *updated += 1;
            if verbose {
                println!("  UPD  derivation: {} (definition changed)", fqn);
            }
        }
    } else {
        let meta = SnapshotMeta::new_operational(ObjectType::DerivationSpec, object_id, "seed");
        SnapshotStore::insert_snapshot(pool, &meta, definition, Some(set_id)).await?;
        *published += 1;
        if verbose {
            println!("  NEW  derivation: {}", fqn);
        }
    }

    Ok(())
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
            assert!(!s.name.is_empty(), "Derivation {} has no name", s.fqn);
            assert!(
                !s.description.is_empty(),
                "Derivation {} has no description",
                s.fqn
            );
            assert!(
                !s.output_attribute_fqn.is_empty(),
                "Derivation {} has no output FQN",
                s.fqn
            );
            assert!(!s.inputs.is_empty(), "Derivation {} has no inputs", s.fqn);
            assert!(
                !s.tests.is_empty(),
                "Derivation {} has no test cases",
                s.fqn
            );
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
    fn test_ubo_percentage_derivation() {
        let spec = ubo_percentage_derivation();
        assert_eq!(spec.fqn, "ubo.total_ownership_pct");
        assert_eq!(spec.inputs.len(), 2);
        assert!(matches!(
            spec.expression,
            DerivationExpression::FunctionRef { ref_name } if ref_name == "sum_ownership_chain"
        ));
        assert!(matches!(spec.null_semantics, NullSemantics::Default(_)));
        assert!(matches!(
            spec.evidence_grade,
            EvidenceGrade::AllowedWithConstraints
        ));
        assert_eq!(spec.tests.len(), 2);
    }

    #[test]
    fn test_composite_risk_score() {
        let spec = composite_risk_score_derivation();
        assert_eq!(spec.fqn, "risk.composite_score");
        assert_eq!(spec.inputs.len(), 3);
        assert!(matches!(spec.null_semantics, NullSemantics::Propagate));
        assert!(matches!(spec.evidence_grade, EvidenceGrade::Prohibited));
    }

    #[test]
    fn test_document_completeness() {
        let spec = document_completeness_derivation();
        assert_eq!(spec.fqn, "kyc.document_completeness_pct");
        assert!(matches!(spec.null_semantics, NullSemantics::Error));
        assert_eq!(spec.tests.len(), 2);
    }

    #[test]
    fn test_beneficial_ownership_flag() {
        let spec = beneficial_ownership_flag_derivation();
        assert_eq!(spec.fqn, "ubo.is_beneficial_owner");
        assert_eq!(spec.inputs.len(), 1);
        // Tests both threshold outcomes
        assert_eq!(spec.tests.len(), 2);
        assert_eq!(spec.tests[0].expected, serde_json::json!(true));
        assert_eq!(spec.tests[1].expected, serde_json::json!(false));
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
