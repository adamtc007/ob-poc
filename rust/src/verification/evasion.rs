//! Evasion Detection for Adversarial Verification
//!
//! Analyzes behavioral patterns in document request history to detect
//! potential evasion tactics that may indicate bad faith actors.
//!
//! ## Evasion Signals
//!
//! - **Repeated Delays**: Multiple deadline extensions requested
//! - **Selective Response**: Answers some questions, ignores others
//! - **Changing Explanations**: Story changes between interactions
//! - **Document Quality Issues**: Blurry scans, partial documents
//! - **Unresponsive to Follow-up**: Ignores clarification requests

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

// ============================================================================
// Evasion Signal Types
// ============================================================================

/// Types of evasion signals
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvasionSignal {
    /// Multiple deadline extensions requested
    RepeatedDelays,
    /// Answers some questions, ignores others
    SelectiveResponse,
    /// Explanations change between interactions
    ChangingExplanations,
    /// Poor quality documents submitted
    DocumentQualityIssues,
    /// Doesn't respond to follow-up questions
    UnresponsiveToFollowUp,
    /// Submits incorrect or mismatched documents
    IncorrectDocuments,
    /// Long response times compared to norms
    SlowResponse,
    /// Repeated document rejections
    HighRejectionRate,
}

impl std::fmt::Display for EvasionSignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvasionSignal::RepeatedDelays => write!(f, "REPEATED_DELAYS"),
            EvasionSignal::SelectiveResponse => write!(f, "SELECTIVE_RESPONSE"),
            EvasionSignal::ChangingExplanations => write!(f, "CHANGING_EXPLANATIONS"),
            EvasionSignal::DocumentQualityIssues => write!(f, "DOCUMENT_QUALITY_ISSUES"),
            EvasionSignal::UnresponsiveToFollowUp => write!(f, "UNRESPONSIVE_TO_FOLLOW_UP"),
            EvasionSignal::IncorrectDocuments => write!(f, "INCORRECT_DOCUMENTS"),
            EvasionSignal::SlowResponse => write!(f, "SLOW_RESPONSE"),
            EvasionSignal::HighRejectionRate => write!(f, "HIGH_REJECTION_RATE"),
        }
    }
}

/// Severity of evasion signal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvasionSeverity {
    /// Minor concern
    Low,
    /// Moderate concern
    Medium,
    /// Significant concern
    High,
    /// Critical - may indicate bad faith
    Critical,
}

// ============================================================================
// Evasion Detection Result
// ============================================================================

/// A detected evasion signal with evidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedEvasionSignal {
    /// Signal type
    pub signal: EvasionSignal,

    /// Severity
    pub severity: EvasionSeverity,

    /// Human-readable description
    pub description: String,

    /// Metric value that triggered detection
    pub metric_value: f64,

    /// Threshold that was exceeded
    pub threshold: f64,

    /// Related document request IDs
    pub related_requests: Vec<Uuid>,
}

/// Complete evasion analysis report for a case
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvasionReport {
    /// Case ID analyzed
    pub case_id: Uuid,

    /// When analysis was performed
    pub analyzed_at: DateTime<Utc>,

    /// Overall evasion score (0.0 - 1.0, higher = more suspicious)
    pub evasion_score: f64,

    /// Classification
    pub classification: EvasionClassification,

    /// Individual signals detected
    pub signals: Vec<DetectedEvasionSignal>,

    /// Metrics collected
    pub metrics: EvasionMetrics,

    /// Recommendation
    pub recommendation: EvasionRecommendation,
}

/// Overall evasion classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvasionClassification {
    /// No significant evasion signals
    Clean,
    /// Minor signals, proceed with caution
    LowRisk,
    /// Moderate signals, requires investigation
    MediumRisk,
    /// Significant signals, escalate
    HighRisk,
    /// Critical signals, likely bad faith
    Critical,
}

/// Recommendation based on evasion analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvasionRecommendation {
    /// Continue normal processing
    Proceed,
    /// Increase scrutiny
    IncreasedScrutiny,
    /// Escalate to senior analyst
    Escalate,
    /// Challenge the client formally
    FormalChallenge,
    /// Consider rejection
    ConsiderRejection,
}

/// Metrics collected during evasion analysis
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvasionMetrics {
    /// Total document requests made
    pub total_requests: i64,
    /// Requests that were fulfilled
    pub fulfilled_requests: i64,
    /// Requests still pending
    pub pending_requests: i64,
    /// Requests that were rejected (quality/wrong doc)
    pub rejected_requests: i64,
    /// Requests that were waived
    pub waived_requests: i64,

    /// Average response time in days
    pub avg_response_days: f64,
    /// Maximum response time in days
    pub max_response_days: f64,

    /// Number of deadline extensions
    pub extension_count: i64,

    /// Rejection rate (rejected / total attempted)
    pub rejection_rate: f64,

    /// Completion rate (fulfilled / total required)
    pub completion_rate: f64,

    /// Follow-up response rate
    pub followup_response_rate: f64,
}

// ============================================================================
// Evasion Detector Configuration
// ============================================================================

/// Configuration for evasion detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvasionDetectorConfig {
    /// Days of slow response before flagging
    pub slow_response_threshold_days: i64,

    /// Number of extensions before flagging
    pub extension_threshold: i64,

    /// Rejection rate threshold (0.0 - 1.0)
    pub rejection_rate_threshold: f64,

    /// Completion rate below which to flag
    pub low_completion_threshold: f64,

    /// Follow-up response rate below which to flag
    pub followup_response_threshold: f64,

    /// Weight for each signal in overall score
    pub signal_weights: std::collections::HashMap<EvasionSignal, f64>,
}

impl Default for EvasionDetectorConfig {
    fn default() -> Self {
        use std::collections::HashMap;

        let mut weights = HashMap::new();
        weights.insert(EvasionSignal::RepeatedDelays, 0.15);
        weights.insert(EvasionSignal::SelectiveResponse, 0.20);
        weights.insert(EvasionSignal::ChangingExplanations, 0.25);
        weights.insert(EvasionSignal::DocumentQualityIssues, 0.10);
        weights.insert(EvasionSignal::UnresponsiveToFollowUp, 0.20);
        weights.insert(EvasionSignal::IncorrectDocuments, 0.15);
        weights.insert(EvasionSignal::SlowResponse, 0.10);
        weights.insert(EvasionSignal::HighRejectionRate, 0.15);

        Self {
            slow_response_threshold_days: 14,
            extension_threshold: 2,
            rejection_rate_threshold: 0.30,
            low_completion_threshold: 0.50,
            followup_response_threshold: 0.70,
            signal_weights: weights,
        }
    }
}

// ============================================================================
// Evasion Detector
// ============================================================================

/// Detects evasion patterns in document request history
pub struct EvasionDetector {
    config: EvasionDetectorConfig,
}

impl EvasionDetector {
    /// Create a new detector with default configuration
    pub fn new() -> Self {
        Self {
            config: EvasionDetectorConfig::default(),
        }
    }

    /// Create a detector with custom configuration
    pub fn with_config(config: EvasionDetectorConfig) -> Self {
        Self { config }
    }

    /// Analyze a case for evasion signals
    #[cfg(feature = "database")]
    pub async fn analyze(
        &self,
        pool: &PgPool,
        case_id: Uuid,
    ) -> Result<EvasionReport, sqlx::Error> {
        // Collect metrics from doc_requests
        let metrics = self.collect_metrics(pool, case_id).await?;

        // Detect signals based on metrics
        let signals = self.detect_signals(&metrics);

        // Calculate overall evasion score
        let evasion_score = self.calculate_score(&signals);

        // Classify
        let classification = self.classify(evasion_score);

        // Recommend action
        let recommendation = self.recommend(&classification, &signals);

        Ok(EvasionReport {
            case_id,
            analyzed_at: Utc::now(),
            evasion_score,
            classification,
            signals,
            metrics,
            recommendation,
        })
    }

    /// Collect metrics from document request history
    #[cfg(feature = "database")]
    async fn collect_metrics(
        &self,
        pool: &PgPool,
        case_id: Uuid,
    ) -> Result<EvasionMetrics, sqlx::Error> {
        // Get request counts by status
        let counts: Option<(i64, i64, i64, i64, i64)> = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) as total,
                COUNT(*) FILTER (WHERE status IN ('VERIFIED', 'RECEIVED')) as fulfilled,
                COUNT(*) FILTER (WHERE status IN ('REQUIRED', 'REQUESTED')) as pending,
                COUNT(*) FILTER (WHERE status = 'REJECTED') as rejected,
                COUNT(*) FILTER (WHERE status = 'WAIVED') as waived
            FROM kyc.doc_requests dr
            JOIN kyc.entity_workstreams w ON w.workstream_id = dr.workstream_id
            WHERE w.case_id = $1
            "#,
        )
        .bind(case_id)
        .fetch_optional(pool)
        .await?;

        let (total, fulfilled, pending, rejected, waived) = counts.unwrap_or((0, 0, 0, 0, 0));

        // Get response time statistics
        let response_times: Option<(Option<f64>, Option<f64>)> = sqlx::query_as(
            r#"
            SELECT
                AVG(EXTRACT(EPOCH FROM (received_at - requested_at)) / 86400)::float8 as avg_days,
                MAX(EXTRACT(EPOCH FROM (received_at - requested_at)) / 86400)::float8 as max_days
            FROM kyc.doc_requests dr
            JOIN kyc.entity_workstreams w ON w.workstream_id = dr.workstream_id
            WHERE w.case_id = $1
            AND received_at IS NOT NULL
            AND requested_at IS NOT NULL
            "#,
        )
        .bind(case_id)
        .fetch_optional(pool)
        .await?;

        let (avg_response, max_response) = response_times.unwrap_or((None, None));

        // Count extension-like patterns (multiple status changes from REQUESTED back to REQUESTED)
        // This is a proxy for extensions since we don't have explicit extension tracking
        let extension_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(DISTINCT dr.request_id)
            FROM kyc.doc_requests dr
            JOIN kyc.entity_workstreams w ON w.workstream_id = dr.workstream_id
            WHERE w.case_id = $1
            AND dr.due_date IS NOT NULL
            AND dr.received_at IS NOT NULL
            AND dr.received_at > dr.due_date
            "#,
        )
        .bind(case_id)
        .fetch_optional(pool)
        .await?
        .unwrap_or(0);

        // Calculate derived metrics
        let rejection_rate = if fulfilled + rejected > 0 {
            rejected as f64 / (fulfilled + rejected) as f64
        } else {
            0.0
        };

        let completion_rate = if total > waived {
            fulfilled as f64 / (total - waived) as f64
        } else {
            1.0
        };

        Ok(EvasionMetrics {
            total_requests: total,
            fulfilled_requests: fulfilled,
            pending_requests: pending,
            rejected_requests: rejected,
            waived_requests: waived,
            avg_response_days: avg_response.unwrap_or(0.0),
            max_response_days: max_response.unwrap_or(0.0),
            extension_count,
            rejection_rate,
            completion_rate,
            followup_response_rate: completion_rate, // Proxy
        })
    }

    /// Detect signals based on collected metrics
    fn detect_signals(&self, metrics: &EvasionMetrics) -> Vec<DetectedEvasionSignal> {
        let mut signals = vec![];

        // Check for slow response
        if metrics.avg_response_days > self.config.slow_response_threshold_days as f64 {
            signals.push(DetectedEvasionSignal {
                signal: EvasionSignal::SlowResponse,
                severity: if metrics.avg_response_days > 30.0 {
                    EvasionSeverity::High
                } else {
                    EvasionSeverity::Medium
                },
                description: format!(
                    "Average response time ({:.1} days) exceeds threshold ({} days)",
                    metrics.avg_response_days, self.config.slow_response_threshold_days
                ),
                metric_value: metrics.avg_response_days,
                threshold: self.config.slow_response_threshold_days as f64,
                related_requests: vec![],
            });
        }

        // Check for repeated delays/extensions
        if metrics.extension_count >= self.config.extension_threshold {
            signals.push(DetectedEvasionSignal {
                signal: EvasionSignal::RepeatedDelays,
                severity: if metrics.extension_count >= 4 {
                    EvasionSeverity::High
                } else {
                    EvasionSeverity::Medium
                },
                description: format!(
                    "Multiple deadline extensions ({}) detected",
                    metrics.extension_count
                ),
                metric_value: metrics.extension_count as f64,
                threshold: self.config.extension_threshold as f64,
                related_requests: vec![],
            });
        }

        // Check for high rejection rate
        if metrics.rejection_rate > self.config.rejection_rate_threshold {
            signals.push(DetectedEvasionSignal {
                signal: EvasionSignal::HighRejectionRate,
                severity: if metrics.rejection_rate > 0.50 {
                    EvasionSeverity::High
                } else {
                    EvasionSeverity::Medium
                },
                description: format!(
                    "High document rejection rate ({:.0}%) exceeds threshold ({:.0}%)",
                    metrics.rejection_rate * 100.0,
                    self.config.rejection_rate_threshold * 100.0
                ),
                metric_value: metrics.rejection_rate,
                threshold: self.config.rejection_rate_threshold,
                related_requests: vec![],
            });
        }

        // Check for selective response (low completion rate)
        if metrics.completion_rate < self.config.low_completion_threshold
            && metrics.total_requests > 0
        {
            signals.push(DetectedEvasionSignal {
                signal: EvasionSignal::SelectiveResponse,
                severity: if metrics.completion_rate < 0.30 {
                    EvasionSeverity::High
                } else {
                    EvasionSeverity::Medium
                },
                description: format!(
                    "Low document completion rate ({:.0}%) suggests selective response",
                    metrics.completion_rate * 100.0
                ),
                metric_value: metrics.completion_rate,
                threshold: self.config.low_completion_threshold,
                related_requests: vec![],
            });
        }

        signals
    }

    /// Calculate overall evasion score from signals
    fn calculate_score(&self, signals: &[DetectedEvasionSignal]) -> f64 {
        if signals.is_empty() {
            return 0.0;
        }

        let mut total_weight = 0.0;
        let mut weighted_sum = 0.0;

        for signal in signals {
            let weight = self
                .config
                .signal_weights
                .get(&signal.signal)
                .copied()
                .unwrap_or(0.10);

            let severity_multiplier = match signal.severity {
                EvasionSeverity::Low => 0.5,
                EvasionSeverity::Medium => 0.75,
                EvasionSeverity::High => 1.0,
                EvasionSeverity::Critical => 1.25,
            };

            weighted_sum += weight * severity_multiplier;
            total_weight += weight;
        }

        if total_weight > 0.0 {
            (weighted_sum / total_weight).min(1.0)
        } else {
            0.0
        }
    }

    /// Classify based on evasion score
    fn classify(&self, score: f64) -> EvasionClassification {
        if score >= 0.80 {
            EvasionClassification::Critical
        } else if score >= 0.60 {
            EvasionClassification::HighRisk
        } else if score >= 0.40 {
            EvasionClassification::MediumRisk
        } else if score >= 0.20 {
            EvasionClassification::LowRisk
        } else {
            EvasionClassification::Clean
        }
    }

    /// Recommend action based on classification
    fn recommend(
        &self,
        classification: &EvasionClassification,
        signals: &[DetectedEvasionSignal],
    ) -> EvasionRecommendation {
        match classification {
            EvasionClassification::Clean => EvasionRecommendation::Proceed,
            EvasionClassification::LowRisk => EvasionRecommendation::IncreasedScrutiny,
            EvasionClassification::MediumRisk => {
                // Check if any high severity signals
                if signals.iter().any(|s| s.severity == EvasionSeverity::High) {
                    EvasionRecommendation::Escalate
                } else {
                    EvasionRecommendation::IncreasedScrutiny
                }
            }
            EvasionClassification::HighRisk => EvasionRecommendation::FormalChallenge,
            EvasionClassification::Critical => EvasionRecommendation::ConsiderRejection,
        }
    }
}

impl Default for EvasionDetector {
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

    #[test]
    fn test_default_config() {
        let config = EvasionDetectorConfig::default();
        assert_eq!(config.slow_response_threshold_days, 14);
        assert_eq!(config.extension_threshold, 2);
    }

    #[test]
    fn test_classification() {
        let detector = EvasionDetector::new();

        assert_eq!(detector.classify(0.10), EvasionClassification::Clean);
        assert_eq!(detector.classify(0.30), EvasionClassification::LowRisk);
        assert_eq!(detector.classify(0.50), EvasionClassification::MediumRisk);
        assert_eq!(detector.classify(0.70), EvasionClassification::HighRisk);
        assert_eq!(detector.classify(0.90), EvasionClassification::Critical);
    }

    #[test]
    fn test_signal_detection_slow_response() {
        let detector = EvasionDetector::new();
        let metrics = EvasionMetrics {
            avg_response_days: 20.0,
            ..Default::default()
        };

        let signals = detector.detect_signals(&metrics);
        assert!(signals
            .iter()
            .any(|s| s.signal == EvasionSignal::SlowResponse));
    }

    #[test]
    fn test_signal_detection_high_rejection() {
        let detector = EvasionDetector::new();
        let metrics = EvasionMetrics {
            rejection_rate: 0.50,
            ..Default::default()
        };

        let signals = detector.detect_signals(&metrics);
        assert!(signals
            .iter()
            .any(|s| s.signal == EvasionSignal::HighRejectionRate));
    }

    #[test]
    fn test_no_signals_clean_metrics() {
        let detector = EvasionDetector::new();
        let metrics = EvasionMetrics {
            total_requests: 10,
            fulfilled_requests: 10,
            pending_requests: 0,
            rejected_requests: 0,
            waived_requests: 0,
            avg_response_days: 5.0,
            max_response_days: 10.0,
            extension_count: 0,
            rejection_rate: 0.0,
            completion_rate: 1.0,
            followup_response_rate: 1.0,
        };

        let signals = detector.detect_signals(&metrics);
        assert!(signals.is_empty());
    }

    #[test]
    fn test_recommendation() {
        let detector = EvasionDetector::new();

        assert_eq!(
            detector.recommend(&EvasionClassification::Clean, &[]),
            EvasionRecommendation::Proceed
        );
        assert_eq!(
            detector.recommend(&EvasionClassification::Critical, &[]),
            EvasionRecommendation::ConsiderRejection
        );
    }
}
