//! Coverage metrics — governance dashboard projections.
//!
//! `CoverageReport` aggregates classification, stewardship, policy, evidence,
//! and security-label coverage across the registry.  Used by the
//! `sem_reg_coverage_report` MCP tool and the `cargo x sem-reg coverage` CLI.

use serde::{Deserialize, Serialize};

#[cfg(feature = "database")]
use anyhow::Result;
#[cfg(feature = "database")]
use sqlx::PgPool;

// ── Types ────────────────────────────────────────────────────────────────────

/// Tier distribution breakdown.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TierDistribution {
    pub governed: i64,
    pub operational: i64,
    pub total: i64,
}

/// Full coverage report across all governance dimensions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoverageReport {
    /// Percentage of objects that have at least one taxonomy classification.
    pub classification_coverage_pct: f64,
    /// Percentage of objects that have a non-null steward/owner.
    pub stewardship_coverage_pct: f64,
    /// Percentage of governed objects with at least one attached PolicyRule.
    pub policy_attachment_pct: f64,
    /// Percentage of observations whose evidence is still fresh (within
    /// retention_days).
    pub evidence_freshness_pct: f64,
    /// Whether all governed objects with retention policies are compliant.
    pub retention_compliance: bool,
    /// Percentage of objects with a non-default security label.
    pub security_label_completeness_pct: f64,
    /// Whether all Proof-tier policy rules reference only governed inputs.
    pub proof_rule_compliance: bool,
    /// Breakdown by governance tier.
    pub tier_distribution: TierDistribution,
    /// Total number of active (non-superseded) snapshots.
    pub snapshot_volume: i64,
    /// Optional tier filter applied.
    pub filter_tier: Option<String>,
}

// ── Store ────────────────────────────────────────────────────────────────────

pub struct MetricsStore;

impl MetricsStore {
    /// Compute a full coverage report, optionally filtered by governance tier.
    #[cfg(feature = "database")]
    pub async fn coverage_report(
        pool: &PgPool,
        filter_tier: Option<&str>,
    ) -> Result<CoverageReport> {
        // Tier distribution
        let tier_clause = match filter_tier {
            Some("governed") => "AND s.governance_tier = 'governed'",
            Some("operational") => "AND s.governance_tier = 'operational'",
            _ => "",
        };

        let query = format!(
            r#"
            SELECT
                -- Total active snapshots
                COUNT(*) AS total,
                COUNT(*) FILTER (WHERE s.governance_tier = 'governed') AS governed,
                COUNT(*) FILTER (WHERE s.governance_tier = 'operational') AS operational,

                -- Classification: has taxonomy_memberships in definition
                ROUND(
                    100.0 * COUNT(*) FILTER (
                        WHERE s.definition ? 'taxonomy_memberships'
                          AND jsonb_array_length(s.definition->'taxonomy_memberships') > 0
                    )::numeric / GREATEST(COUNT(*), 1),
                    1
                ) AS classification_pct,

                -- Stewardship: has steward field in definition
                ROUND(
                    100.0 * COUNT(*) FILTER (
                        WHERE s.definition ? 'steward'
                          AND s.definition->>'steward' IS NOT NULL
                          AND s.definition->>'steward' != ''
                    )::numeric / GREATEST(COUNT(*), 1),
                    1
                ) AS stewardship_pct,

                -- Security label completeness (non-default)
                ROUND(
                    100.0 * COUNT(*) FILTER (
                        WHERE s.security_label != '{{}}'::jsonb
                          AND s.security_label IS NOT NULL
                    )::numeric / GREATEST(COUNT(*), 1),
                    1
                ) AS security_label_pct

            FROM sem_reg.snapshots s
            WHERE s.effective_until IS NULL
              AND s.status = 'published'
              {tier_clause}
            "#
        );

        let row: (
            i64,
            i64,
            i64,
            rust_decimal::Decimal,
            rust_decimal::Decimal,
            rust_decimal::Decimal,
        ) = sqlx::query_as(&query).fetch_one(pool).await?;

        let total = row.0;
        let governed = row.1;
        let operational = row.2;
        let classification_pct: f64 = row.3.to_string().parse().unwrap_or(0.0);
        let stewardship_pct: f64 = row.4.to_string().parse().unwrap_or(0.0);
        let security_label_pct: f64 = row.5.to_string().parse().unwrap_or(0.0);

        // Policy attachment: governed objects with at least one policy rule referencing them
        let policy_pct = Self::compute_policy_attachment_pct(pool, filter_tier).await?;

        // Evidence freshness (observations within retention window)
        let evidence_pct = Self::compute_evidence_freshness_pct(pool).await?;

        // Proof rule compliance: all Proof-tier policies reference only governed inputs
        let proof_compliance = Self::check_proof_rule_compliance(pool).await?;

        Ok(CoverageReport {
            classification_coverage_pct: classification_pct,
            stewardship_coverage_pct: stewardship_pct,
            policy_attachment_pct: policy_pct,
            evidence_freshness_pct: evidence_pct,
            retention_compliance: evidence_pct >= 100.0,
            security_label_completeness_pct: security_label_pct,
            proof_rule_compliance: proof_compliance,
            tier_distribution: TierDistribution {
                governed,
                operational,
                total,
            },
            snapshot_volume: total,
            filter_tier: filter_tier.map(String::from),
        })
    }

    #[cfg(feature = "database")]
    async fn compute_policy_attachment_pct(
        pool: &PgPool,
        filter_tier: Option<&str>,
    ) -> Result<f64> {
        let tier_clause = match filter_tier {
            Some("governed") => "AND s.governance_tier = 'governed'",
            Some("operational") => "AND s.governance_tier = 'operational'",
            _ => "",
        };

        let query = format!(
            r#"
            SELECT ROUND(
                100.0 * COUNT(DISTINCT pr.definition->>'subject_attribute_id')::numeric
                / GREATEST(
                    (SELECT COUNT(*) FROM sem_reg.snapshots s
                     WHERE s.effective_until IS NULL
                       AND s.status = 'published'
                       AND s.object_type = 'policy_rule'
                       {tier_clause}), 1
                ),
                1
            )
            FROM sem_reg.snapshots pr
            WHERE pr.effective_until IS NULL
              AND pr.status = 'published'
              AND pr.object_type = 'policy_rule'
              AND pr.definition ? 'subject_attribute_id'
              {tier_clause}
            "#
        );

        let pct: rust_decimal::Decimal = sqlx::query_scalar(&query).fetch_one(pool).await?;
        Ok(pct.to_string().parse().unwrap_or(0.0))
    }

    #[cfg(feature = "database")]
    async fn compute_evidence_freshness_pct(pool: &PgPool) -> Result<f64> {
        let row: Option<(rust_decimal::Decimal,)> = sqlx::query_as(
            r#"
            SELECT ROUND(
                100.0 * COUNT(*) FILTER (
                    WHERE o.definition->>'observed_at' IS NOT NULL
                      AND (o.definition->>'observed_at')::timestamptz
                          > NOW() - INTERVAL '90 days'
                )::numeric / GREATEST(COUNT(*), 1),
                1
            )
            FROM sem_reg.snapshots o
            WHERE o.effective_until IS NULL
              AND o.status = 'published'
              AND o.object_type = 'observation'
            "#,
        )
        .fetch_optional(pool)
        .await?;

        match row {
            Some((pct,)) => Ok(pct.to_string().parse().unwrap_or(100.0)),
            None => Ok(100.0), // No observations → vacuously fresh
        }
    }

    #[cfg(feature = "database")]
    async fn check_proof_rule_compliance(pool: &PgPool) -> Result<bool> {
        // A Proof-tier policy rule is compliant if all its referenced attributes
        // are also governed tier.
        let violating: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM sem_reg.snapshots pr
            JOIN sem_reg.snapshots attr
                ON attr.object_id = (pr.definition->>'subject_attribute_id')::uuid
               AND attr.effective_until IS NULL
               AND attr.status = 'published'
            WHERE pr.effective_until IS NULL
              AND pr.status = 'published'
              AND pr.object_type = 'policy_rule'
              AND pr.governance_tier = 'governed'
              AND pr.definition->>'trust_class' IN ('proof', 'decision_support')
              AND attr.governance_tier = 'operational'
            "#,
        )
        .fetch_one(pool)
        .await?;

        Ok(violating == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coverage_report_default() {
        let report = CoverageReport::default();
        assert_eq!(report.snapshot_volume, 0);
        assert_eq!(report.tier_distribution.total, 0);
        assert!(!report.proof_rule_compliance);
    }

    #[test]
    fn test_tier_distribution_serde() {
        let td = TierDistribution {
            governed: 10,
            operational: 90,
            total: 100,
        };
        let json = serde_json::to_value(&td).unwrap();
        assert_eq!(json["governed"], 10);
        assert_eq!(json["total"], 100);
    }

    #[test]
    fn test_coverage_report_roundtrip() {
        let report = CoverageReport {
            classification_coverage_pct: 85.5,
            stewardship_coverage_pct: 72.0,
            policy_attachment_pct: 91.3,
            evidence_freshness_pct: 100.0,
            retention_compliance: true,
            security_label_completeness_pct: 60.0,
            proof_rule_compliance: true,
            tier_distribution: TierDistribution {
                governed: 50,
                operational: 150,
                total: 200,
            },
            snapshot_volume: 200,
            filter_tier: Some("all".into()),
        };
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["classification_coverage_pct"], 85.5);
        assert_eq!(json["proof_rule_compliance"], true);
        assert_eq!(json["filter_tier"], "all");
    }
}
