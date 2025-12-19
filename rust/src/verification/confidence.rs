//! Confidence Calculation for Adversarial Verification
//!
//! Implements a weighted confidence aggregation algorithm that considers:
//! - Source type credibility (government registry > client allegation)
//! - Authoritative source bonus
//! - Recency decay
//! - Corroboration bonus
//! - Inconsistency penalties
//! - Pattern detection penalties

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::types::{Evidence, InconsistencySeverity};
use super::PatternSeverity;

// ============================================================================
// Confidence Thresholds
// ============================================================================

/// Confidence threshold configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceThresholds {
    /// Score >= this is VERIFIED (default 0.80)
    pub verified: f64,
    /// Score >= this is PROVISIONAL (default 0.60)
    pub provisional: f64,
    /// Score >= this is SUSPECT (default 0.40)
    pub suspect: f64,
    // Below suspect is REJECTED
}

impl Default for ConfidenceThresholds {
    fn default() -> Self {
        Self {
            verified: 0.80,
            provisional: 0.60,
            suspect: 0.40,
        }
    }
}

/// Confidence band classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConfidenceBand {
    /// High confidence, verified (≥0.80)
    Verified,
    /// Acceptable with caveats (≥0.60)
    Provisional,
    /// Requires investigation (≥0.40)
    Suspect,
    /// Insufficient evidence (<0.40)
    Rejected,
}

impl ConfidenceBand {
    /// Get the band from a confidence score
    pub fn from_score(score: f64, thresholds: &ConfidenceThresholds) -> Self {
        if score >= thresholds.verified {
            ConfidenceBand::Verified
        } else if score >= thresholds.provisional {
            ConfidenceBand::Provisional
        } else if score >= thresholds.suspect {
            ConfidenceBand::Suspect
        } else {
            ConfidenceBand::Rejected
        }
    }

    /// Is this band acceptable for KYC approval?
    pub fn is_acceptable(&self) -> bool {
        matches!(self, ConfidenceBand::Verified | ConfidenceBand::Provisional)
    }

    /// Does this band require investigation?
    pub fn requires_investigation(&self) -> bool {
        matches!(self, ConfidenceBand::Suspect | ConfidenceBand::Rejected)
    }
}

impl std::fmt::Display for ConfidenceBand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfidenceBand::Verified => write!(f, "VERIFIED"),
            ConfidenceBand::Provisional => write!(f, "PROVISIONAL"),
            ConfidenceBand::Suspect => write!(f, "SUSPECT"),
            ConfidenceBand::Rejected => write!(f, "REJECTED"),
        }
    }
}

// ============================================================================
// Confidence Result
// ============================================================================

/// Result of confidence calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceResult {
    /// Final aggregated confidence score (0.0 - 1.0)
    pub score: f64,

    /// Classified band
    pub band: ConfidenceBand,

    /// Number of observations considered
    pub observation_count: usize,

    /// Number of authoritative sources
    pub authoritative_count: usize,

    /// Number of corroborating observations
    pub corroborating_count: usize,

    /// Number of inconsistencies detected
    pub inconsistency_count: usize,

    /// Number of patterns detected
    pub pattern_count: usize,

    /// Breakdown of score components
    pub breakdown: ConfidenceBreakdown,
}

/// Breakdown of confidence score components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceBreakdown {
    /// Base score from source weighting
    pub base_score: f64,
    /// Bonus from authoritative sources
    pub authoritative_bonus: f64,
    /// Bonus from recency
    pub recency_factor: f64,
    /// Bonus from corroboration
    pub corroboration_bonus: f64,
    /// Penalty from inconsistencies
    pub inconsistency_penalty: f64,
    /// Penalty from detected patterns
    pub pattern_penalty: f64,
}

// ============================================================================
// Confidence Calculator
// ============================================================================

/// Configuration for confidence calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceConfig {
    /// Threshold configuration
    pub thresholds: ConfidenceThresholds,

    /// Bonus multiplier for authoritative sources (default 1.20 = +20%)
    pub authoritative_bonus: f64,

    /// Bonus per corroborating observation (default 0.10 = +10%)
    pub corroboration_bonus_per_source: f64,

    /// Maximum corroboration bonus (default 0.30 = +30%)
    pub max_corroboration_bonus: f64,

    /// Penalty per inconsistency (default 0.15 = -15%)
    pub inconsistency_penalty_per: f64,

    /// Penalty for detected patterns by severity
    pub pattern_penalty_low: f64,
    pub pattern_penalty_medium: f64,
    pub pattern_penalty_high: f64,
    pub pattern_penalty_critical: f64,

    /// Half-life for recency decay in days (default 365)
    pub recency_half_life_days: i64,
}

impl Default for ConfidenceConfig {
    fn default() -> Self {
        Self {
            thresholds: ConfidenceThresholds::default(),
            authoritative_bonus: 1.20,
            corroboration_bonus_per_source: 0.10,
            max_corroboration_bonus: 0.30,
            inconsistency_penalty_per: 0.15,
            pattern_penalty_low: 0.05,
            pattern_penalty_medium: 0.15,
            pattern_penalty_high: 0.25,
            pattern_penalty_critical: 0.40,
            recency_half_life_days: 365,
        }
    }
}

/// Calculates aggregate confidence scores from multiple observations
pub struct ConfidenceCalculator {
    config: ConfidenceConfig,
}

impl ConfidenceCalculator {
    /// Create a new calculator with default configuration
    pub fn new() -> Self {
        Self {
            config: ConfidenceConfig::default(),
        }
    }

    /// Create a calculator with custom configuration
    pub fn with_config(config: ConfidenceConfig) -> Self {
        Self { config }
    }

    /// Calculate aggregate confidence from observations
    ///
    /// # Arguments
    /// * `observations` - List of evidence observations
    /// * `inconsistencies` - List of detected inconsistencies with severities
    /// * `patterns` - List of detected pattern severities
    /// * `now` - Current timestamp for recency calculation
    pub fn calculate(
        &self,
        observations: &[Evidence],
        inconsistencies: &[(InconsistencySeverity, f64)], // (severity, weight)
        patterns: &[PatternSeverity],
        now: DateTime<Utc>,
    ) -> ConfidenceResult {
        if observations.is_empty() {
            return ConfidenceResult {
                score: 0.0,
                band: ConfidenceBand::Rejected,
                observation_count: 0,
                authoritative_count: 0,
                corroborating_count: 0,
                inconsistency_count: 0,
                pattern_count: 0,
                breakdown: ConfidenceBreakdown {
                    base_score: 0.0,
                    authoritative_bonus: 0.0,
                    recency_factor: 1.0,
                    corroboration_bonus: 0.0,
                    inconsistency_penalty: 0.0,
                    pattern_penalty: 0.0,
                },
            };
        }

        // 1. Calculate base weighted score from all observations
        let mut total_weight = 0.0;
        let mut weighted_sum = 0.0;

        for obs in observations {
            let base = obs.source.base_confidence();
            let recency = self.recency_factor(obs.observed_at, now);
            let weight = base * recency * obs.confidence;

            weighted_sum += weight * obs.confidence;
            total_weight += weight;
        }

        let base_score = if total_weight > 0.0 {
            weighted_sum / total_weight
        } else {
            0.0
        };

        // 2. Count authoritative sources
        let authoritative_count = observations.iter().filter(|o| o.is_authoritative).count();

        // 3. Calculate authoritative bonus
        let authoritative_bonus = if authoritative_count > 0 {
            (self.config.authoritative_bonus - 1.0).min(0.20)
        } else {
            0.0
        };

        // 4. Calculate corroboration bonus
        // Count unique source types that agree
        let unique_sources: std::collections::HashSet<_> = observations
            .iter()
            .map(|o| std::mem::discriminant(&o.source))
            .collect();
        let corroborating_count = unique_sources.len().saturating_sub(1);
        let corroboration_bonus = (corroborating_count as f64
            * self.config.corroboration_bonus_per_source)
            .min(self.config.max_corroboration_bonus);

        // 5. Calculate inconsistency penalty
        let inconsistency_penalty: f64 = inconsistencies
            .iter()
            .map(|(severity, weight)| {
                let base_penalty = match severity {
                    InconsistencySeverity::Info => 0.02,
                    InconsistencySeverity::Low => 0.05,
                    InconsistencySeverity::Medium => 0.10,
                    InconsistencySeverity::High => 0.15,
                    InconsistencySeverity::Critical => 0.25,
                };
                base_penalty * weight
            })
            .sum();

        // 6. Calculate pattern penalty
        let pattern_penalty: f64 = patterns
            .iter()
            .map(|severity| match severity {
                PatternSeverity::Info => 0.0, // Info patterns are informational, no penalty
                PatternSeverity::Low => self.config.pattern_penalty_low,
                PatternSeverity::Medium => self.config.pattern_penalty_medium,
                PatternSeverity::High => self.config.pattern_penalty_high,
                PatternSeverity::Critical => self.config.pattern_penalty_critical,
            })
            .sum();

        // 7. Calculate average recency factor
        let avg_recency: f64 = observations
            .iter()
            .map(|o| self.recency_factor(o.observed_at, now))
            .sum::<f64>()
            / observations.len() as f64;

        // 8. Aggregate final score
        let score = (base_score + authoritative_bonus + corroboration_bonus
            - inconsistency_penalty
            - pattern_penalty)
            .clamp(0.0, 1.0);

        // 9. Classify into band
        let band = ConfidenceBand::from_score(score, &self.config.thresholds);

        ConfidenceResult {
            score,
            band,
            observation_count: observations.len(),
            authoritative_count,
            corroborating_count,
            inconsistency_count: inconsistencies.len(),
            pattern_count: patterns.len(),
            breakdown: ConfidenceBreakdown {
                base_score,
                authoritative_bonus,
                recency_factor: avg_recency,
                corroboration_bonus,
                inconsistency_penalty,
                pattern_penalty,
            },
        }
    }

    /// Calculate recency factor (exponential decay with half-life)
    fn recency_factor(&self, observed_at: DateTime<Utc>, now: DateTime<Utc>) -> f64 {
        let age = now.signed_duration_since(observed_at);
        let age_days = age.num_days() as f64;

        if age_days <= 0.0 {
            return 1.0;
        }

        let half_life = self.config.recency_half_life_days as f64;
        0.5_f64.powf(age_days / half_life)
    }

    /// Calculate confidence for a single claim against its evidence
    pub fn calculate_claim_confidence(
        &self,
        evidence: &[Evidence],
        inconsistencies: &[(InconsistencySeverity, f64)],
        patterns: &[PatternSeverity],
    ) -> ConfidenceResult {
        self.calculate(evidence, inconsistencies, patterns, Utc::now())
    }
}

impl Default for ConfidenceCalculator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verification::types::{Evidence, EvidenceSource};
    use chrono::Duration;
    use uuid::Uuid;

    fn make_evidence(source: EvidenceSource, confidence: f64, authoritative: bool) -> Evidence {
        Evidence {
            evidence_id: Uuid::new_v4(),
            entity_id: Uuid::new_v4(),
            attribute_id: Uuid::new_v4(),
            observed_value: serde_json::json!("test"),
            source,
            confidence,
            is_authoritative: authoritative,
            observed_at: Utc::now(),
            source_document_id: None,
            extraction_method: None,
            effective_from: None,
            effective_to: None,
        }
    }

    #[test]
    fn test_empty_observations() {
        let calc = ConfidenceCalculator::new();
        let result = calc.calculate(&[], &[], &[], Utc::now());

        assert_eq!(result.score, 0.0);
        assert_eq!(result.band, ConfidenceBand::Rejected);
    }

    #[test]
    fn test_single_government_registry() {
        let calc = ConfidenceCalculator::new();
        let evidence = vec![make_evidence(
            EvidenceSource::GovernmentRegistry,
            0.95,
            true,
        )];

        let result = calc.calculate(&evidence, &[], &[], Utc::now());

        assert!(result.score >= 0.80);
        assert_eq!(result.band, ConfidenceBand::Verified);
        assert_eq!(result.authoritative_count, 1);
    }

    #[test]
    fn test_client_allegation_low_confidence() {
        let calc = ConfidenceCalculator::new();
        let evidence = vec![make_evidence(EvidenceSource::Allegation, 0.50, false)];

        let result = calc.calculate(&evidence, &[], &[], Utc::now());

        assert!(result.score < 0.60);
        assert!(matches!(
            result.band,
            ConfidenceBand::Suspect | ConfidenceBand::Rejected
        ));
    }

    #[test]
    fn test_corroboration_bonus() {
        let calc = ConfidenceCalculator::new();
        let evidence = vec![
            make_evidence(EvidenceSource::Document, 0.80, false),
            make_evidence(EvidenceSource::ThirdParty, 0.80, false),
            make_evidence(EvidenceSource::GovernmentRegistry, 0.90, true),
        ];

        let result = calc.calculate(&evidence, &[], &[], Utc::now());

        assert!(result.corroborating_count >= 2);
        assert!(result.breakdown.corroboration_bonus > 0.0);
    }

    #[test]
    fn test_inconsistency_penalty() {
        let calc = ConfidenceCalculator::new();
        let evidence = vec![make_evidence(
            EvidenceSource::GovernmentRegistry,
            0.95,
            true,
        )];

        let inconsistencies = vec![
            (InconsistencySeverity::High, 1.0),
            (InconsistencySeverity::Medium, 1.0),
        ];

        let result = calc.calculate(&evidence, &inconsistencies, &[], Utc::now());

        assert!(result.breakdown.inconsistency_penalty > 0.0);
        assert!(result.score < 0.80); // Should drop below verified threshold
    }

    #[test]
    fn test_pattern_penalty() {
        let calc = ConfidenceCalculator::new();
        let evidence = vec![make_evidence(
            EvidenceSource::GovernmentRegistry,
            0.95,
            true,
        )];

        let patterns = vec![PatternSeverity::Critical];

        let result = calc.calculate(&evidence, &[], &patterns, Utc::now());

        assert!(result.breakdown.pattern_penalty > 0.0);
        assert!(result.score < 0.80); // Critical pattern should drop below verified
    }

    #[test]
    fn test_recency_decay() {
        let calc = ConfidenceCalculator::new();
        let now = Utc::now();
        let old_date = now - Duration::days(730); // 2 years ago (2 half-lives)

        let mut old_evidence = make_evidence(EvidenceSource::Document, 0.80, false);
        old_evidence.observed_at = old_date;

        let fresh_evidence = make_evidence(EvidenceSource::Document, 0.80, false);

        let old_result = calc.calculate(&[old_evidence], &[], &[], now);
        let fresh_result = calc.calculate(&[fresh_evidence], &[], &[], now);

        assert!(old_result.score < fresh_result.score);
    }

    #[test]
    fn test_confidence_band_classification() {
        let thresholds = ConfidenceThresholds::default();

        assert_eq!(
            ConfidenceBand::from_score(0.85, &thresholds),
            ConfidenceBand::Verified
        );
        assert_eq!(
            ConfidenceBand::from_score(0.70, &thresholds),
            ConfidenceBand::Provisional
        );
        assert_eq!(
            ConfidenceBand::from_score(0.50, &thresholds),
            ConfidenceBand::Suspect
        );
        assert_eq!(
            ConfidenceBand::from_score(0.30, &thresholds),
            ConfidenceBand::Rejected
        );
    }
}
