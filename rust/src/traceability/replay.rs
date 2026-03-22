//! Replay and narrowing-drift helpers for utterance traces.

use super::types::UtteranceTraceRecord;
#[cfg(feature = "database")]
use super::UtteranceTraceRepository;

/// Verdict for comparing an original trace resolution to a replayed one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayVerdict {
    Unchanged,
    ImprovedResolution,
    DegradedResolution,
    ChangedResolution,
    CandidateSetExpandedUnexpectedly,
    CandidateSetContractedUnexpectedly,
    FallbackNewlyRequired,
    FallbackNoLongerRequired,
}

/// Drift classification for Phase 3 candidate narrowing.
#[derive(Debug, Clone, PartialEq)]
pub enum NarrowingDrift {
    Stable,
    Weakened { expansion_ratio: f64 },
    Strengthened { contraction_ratio: f64 },
    FallbackRegressed,
    FallbackImproved,
}

/// Summary of Phase 3 narrowing differences between an original and replayed trace.
#[derive(Debug, Clone, PartialEq)]
pub struct ReplayNarrowingDiff {
    pub original_phase3_size: usize,
    pub replayed_phase3_size: usize,
    pub original_fallback_invoked: bool,
    pub replayed_fallback_invoked: bool,
    pub narrowing_drift: NarrowingDrift,
}

/// Row-level replay comparison result for persisted utterance traces.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceReplayComparison {
    pub trace_id: uuid::Uuid,
    pub original_resolved_verb: Option<String>,
    pub replayed_resolved_verb: Option<String>,
    pub narrowing_diff: ReplayNarrowingDiff,
    pub verdict: ReplayVerdict,
}

/// Compute Phase 3 narrowing drift between two trace payloads.
///
/// # Examples
/// ```rust
/// use ob_poc::traceability::{compute_replay_narrowing_diff, NarrowingDrift};
///
/// let original = serde_json::json!({
///     "phase_3": { "phase4_candidate_set": ["kyc.open-case"] },
///     "phase_4": { "fallback_invoked": false }
/// });
/// let replayed = serde_json::json!({
///     "phase_3": { "phase4_candidate_set": ["kyc.open-case", "deal.create"] },
///     "phase_4": { "fallback_invoked": false }
/// });
///
/// let diff = compute_replay_narrowing_diff(&original, &replayed);
/// assert!(matches!(diff.narrowing_drift, NarrowingDrift::Weakened { .. }));
/// ```
pub fn compute_replay_narrowing_diff(
    original_payload: &serde_json::Value,
    replayed_payload: &serde_json::Value,
) -> ReplayNarrowingDiff {
    let original_phase3_size = phase4_candidate_set_size(original_payload);
    let replayed_phase3_size = phase4_candidate_set_size(replayed_payload);
    let original_fallback_invoked = fallback_invoked(original_payload);
    let replayed_fallback_invoked = fallback_invoked(replayed_payload);

    let narrowing_drift = if !original_fallback_invoked && replayed_fallback_invoked {
        NarrowingDrift::FallbackRegressed
    } else if original_fallback_invoked && !replayed_fallback_invoked {
        NarrowingDrift::FallbackImproved
    } else if original_phase3_size == replayed_phase3_size {
        NarrowingDrift::Stable
    } else if replayed_phase3_size > original_phase3_size {
        let base = original_phase3_size.max(1) as f64;
        NarrowingDrift::Weakened {
            expansion_ratio: replayed_phase3_size as f64 / base,
        }
    } else {
        let base = original_phase3_size.max(1) as f64;
        NarrowingDrift::Strengthened {
            contraction_ratio: replayed_phase3_size as f64 / base,
        }
    };

    ReplayNarrowingDiff {
        original_phase3_size,
        replayed_phase3_size,
        original_fallback_invoked,
        replayed_fallback_invoked,
        narrowing_drift,
    }
}

/// Derive a replay verdict from original and replayed resolution data.
///
/// # Examples
/// ```rust
/// use ob_poc::traceability::{
///     compute_replay_narrowing_diff, derive_replay_verdict, ReplayVerdict,
/// };
///
/// let original = serde_json::json!({
///     "phase_3": { "phase4_candidate_set": ["kyc.open-case"] },
///     "phase_4": { "resolved_verb": "kyc.open-case", "fallback_invoked": false }
/// });
/// let replayed = serde_json::json!({
///     "phase_3": { "phase4_candidate_set": ["kyc.open-case"] },
///     "phase_4": { "resolved_verb": "kyc.open-case", "fallback_invoked": false }
/// });
/// let diff = compute_replay_narrowing_diff(&original, &replayed);
/// assert_eq!(
///     derive_replay_verdict(Some("kyc.open-case"), Some("kyc.open-case"), &diff),
///     ReplayVerdict::Unchanged
/// );
/// ```
pub fn derive_replay_verdict(
    original_resolved_verb: Option<&str>,
    replayed_resolved_verb: Option<&str>,
    diff: &ReplayNarrowingDiff,
) -> ReplayVerdict {
    if !diff.original_fallback_invoked && diff.replayed_fallback_invoked {
        return ReplayVerdict::FallbackNewlyRequired;
    }
    if diff.original_fallback_invoked && !diff.replayed_fallback_invoked {
        return ReplayVerdict::FallbackNoLongerRequired;
    }

    match (&original_resolved_verb, &replayed_resolved_verb) {
        (Some(original), Some(replayed)) if original == replayed => match diff.narrowing_drift {
            NarrowingDrift::Weakened { .. } => ReplayVerdict::CandidateSetExpandedUnexpectedly,
            NarrowingDrift::Strengthened { .. } => {
                ReplayVerdict::CandidateSetContractedUnexpectedly
            }
            _ => ReplayVerdict::Unchanged,
        },
        (None, Some(_)) => ReplayVerdict::ImprovedResolution,
        (Some(_), None) => ReplayVerdict::DegradedResolution,
        (Some(_), Some(_)) => ReplayVerdict::ChangedResolution,
        (None, None) => match diff.narrowing_drift {
            NarrowingDrift::Weakened { .. } => ReplayVerdict::CandidateSetExpandedUnexpectedly,
            NarrowingDrift::Strengthened { .. } => {
                ReplayVerdict::CandidateSetContractedUnexpectedly
            }
            _ => ReplayVerdict::Unchanged,
        },
    }
}

/// Compare an original persisted utterance trace against a replayed one.
///
/// # Examples
/// ```rust
/// use chrono::Utc;
/// use ob_poc::traceability::{
///     compare_trace_records, NewUtteranceTrace, SurfaceVersions, TraceKind, TraceOutcome,
///     UtteranceTraceRecord,
/// };
/// use uuid::Uuid;
///
/// let trace_id = Uuid::new_v4();
/// let original = UtteranceTraceRecord {
///     trace_id,
///     utterance_id: Uuid::new_v4(),
///     session_id: Uuid::new_v4(),
///     correlation_id: None,
///     trace_kind: TraceKind::Original,
///     parent_trace_id: None,
///     timestamp: Utc::now(),
///     raw_utterance: "open the case".to_string(),
///     outcome: TraceOutcome::ExecutedSuccessfully,
///     halt_reason_code: None,
///     halt_phase: None,
///     resolved_verb: Some("kyc.open-case".to_string()),
///     plane: None,
///     polarity: None,
///     execution_shape_kind: None,
///     fallback_invoked: false,
///     fallback_reason_code: None,
///     situation_signature_hash: None,
///     template_id: None,
///     template_version: None,
///     surface_versions: SurfaceVersions::default(),
///     trace_payload: serde_json::json!({
///         "phase_3": { "phase4_candidate_set": ["kyc.open-case"] },
///         "phase_4": { "fallback_invoked": false }
///     }),
/// };
/// let replayed = original.clone();
/// let comparison = compare_trace_records(&original, &replayed);
/// assert_eq!(comparison.verdict, ob_poc::traceability::ReplayVerdict::Unchanged);
/// ```
pub fn compare_trace_records(
    original: &UtteranceTraceRecord,
    replayed: &UtteranceTraceRecord,
) -> TraceReplayComparison {
    let narrowing_diff =
        compute_replay_narrowing_diff(&original.trace_payload, &replayed.trace_payload);
    let verdict = derive_replay_verdict(
        original.resolved_verb.as_deref(),
        replayed.resolved_verb.as_deref(),
        &narrowing_diff,
    );

    TraceReplayComparison {
        trace_id: original.trace_id,
        original_resolved_verb: original.resolved_verb.clone(),
        replayed_resolved_verb: replayed.resolved_verb.clone(),
        narrowing_diff,
        verdict,
    }
}

/// Compare two ordered trace sequences positionally.
///
/// # Examples
/// ```rust
/// use chrono::Utc;
/// use ob_poc::traceability::{
///     compare_trace_sequences, SurfaceVersions, TraceKind, TraceOutcome, UtteranceTraceRecord,
/// };
/// use uuid::Uuid;
///
/// let original = vec![UtteranceTraceRecord {
///     trace_id: Uuid::new_v4(),
///     utterance_id: Uuid::new_v4(),
///     session_id: Uuid::new_v4(),
///     correlation_id: None,
///     trace_kind: TraceKind::Original,
///     parent_trace_id: None,
///     timestamp: Utc::now(),
///     raw_utterance: "open".to_string(),
///     outcome: TraceOutcome::ExecutedSuccessfully,
///     halt_reason_code: None,
///     halt_phase: None,
///     resolved_verb: Some("case.open".to_string()),
///     plane: None,
///     polarity: None,
///     execution_shape_kind: None,
///     fallback_invoked: false,
///     fallback_reason_code: None,
///     situation_signature_hash: None,
///     template_id: None,
///     template_version: None,
///     surface_versions: SurfaceVersions::default(),
///     trace_payload: serde_json::json!({
///         "phase_3": { "phase4_candidate_set": ["case.open"] },
///         "phase_4": { "fallback_invoked": false }
///     }),
/// }];
/// let replayed = original.clone();
/// assert_eq!(compare_trace_sequences(&original, &replayed).len(), 1);
/// ```
pub fn compare_trace_sequences(
    original: &[UtteranceTraceRecord],
    replayed: &[UtteranceTraceRecord],
) -> Vec<TraceReplayComparison> {
    original
        .iter()
        .zip(replayed.iter())
        .map(|(left, right)| compare_trace_records(left, right))
        .collect()
}

/// Load two persisted utterance traces and compare them for replay drift.
///
/// # Examples
/// ```rust,no_run
/// use ob_poc::traceability::{compare_trace_ids, UtteranceTraceRepository};
/// use uuid::Uuid;
///
/// # async fn demo(repo: UtteranceTraceRepository) -> anyhow::Result<()> {
/// let _comparison = compare_trace_ids(&repo, Uuid::new_v4(), Uuid::new_v4()).await?;
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "database")]
pub async fn compare_trace_ids(
    repository: &UtteranceTraceRepository,
    original_trace_id: uuid::Uuid,
    replayed_trace_id: uuid::Uuid,
) -> anyhow::Result<Option<TraceReplayComparison>> {
    let Some(original) = repository.get(original_trace_id).await? else {
        return Ok(None);
    };
    let Some(replayed) = repository.get(replayed_trace_id).await? else {
        return Ok(None);
    };

    Ok(Some(compare_trace_records(&original, &replayed)))
}

/// Load two sessions of persisted utterance traces and compare them positionally.
///
/// # Examples
/// ```rust,no_run
/// use ob_poc::traceability::{compare_session_traces, UtteranceTraceRepository};
/// use uuid::Uuid;
///
/// # async fn demo(repo: UtteranceTraceRepository) -> anyhow::Result<()> {
/// let _comparisons = compare_session_traces(&repo, Uuid::new_v4(), Uuid::new_v4(), 50).await?;
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "database")]
pub async fn compare_session_traces(
    repository: &UtteranceTraceRepository,
    original_session_id: uuid::Uuid,
    replayed_session_id: uuid::Uuid,
    limit: i64,
) -> anyhow::Result<Vec<TraceReplayComparison>> {
    let original = repository
        .list_for_session(original_session_id, limit)
        .await?;
    let replayed = repository
        .list_for_session(replayed_session_id, limit)
        .await?;
    Ok(compare_trace_sequences(&original, &replayed))
}

fn phase4_candidate_set_size(payload: &serde_json::Value) -> usize {
    payload
        .get("phase_3")
        .and_then(|phase| phase.get("phase4_candidate_set"))
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

fn fallback_invoked(payload: &serde_json::Value) -> bool {
    payload
        .get("phase_4")
        .and_then(|phase| phase.get("fallback_invoked"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{
        compare_trace_records, compare_trace_sequences, compute_replay_narrowing_diff,
        derive_replay_verdict, NarrowingDrift, ReplayVerdict,
    };
    use crate::traceability::{SurfaceVersions, TraceKind, TraceOutcome, UtteranceTraceRecord};
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_narrowing_diff_detects_weakened_candidate_set() {
        let original = serde_json::json!({
            "phase_3": { "phase4_candidate_set": ["kyc.open-case"] },
            "phase_4": { "fallback_invoked": false }
        });
        let replayed = serde_json::json!({
            "phase_3": { "phase4_candidate_set": ["kyc.open-case", "deal.create"] },
            "phase_4": { "fallback_invoked": false }
        });

        let diff = compute_replay_narrowing_diff(&original, &replayed);
        assert!(matches!(
            diff.narrowing_drift,
            NarrowingDrift::Weakened { .. }
        ));
    }

    #[test]
    fn test_replay_verdict_detects_fallback_regression() {
        let original = serde_json::json!({
            "phase_3": { "phase4_candidate_set": ["kyc.open-case"] },
            "phase_4": { "fallback_invoked": false }
        });
        let replayed = serde_json::json!({
            "phase_3": { "phase4_candidate_set": ["kyc.open-case"] },
            "phase_4": { "fallback_invoked": true }
        });

        let diff = compute_replay_narrowing_diff(&original, &replayed);
        assert_eq!(
            derive_replay_verdict(Some("kyc.open-case"), Some("kyc.open-case"), &diff),
            ReplayVerdict::FallbackNewlyRequired
        );
    }

    #[test]
    fn test_compare_trace_records_returns_row_level_verdict() {
        let original = UtteranceTraceRecord {
            trace_id: Uuid::new_v4(),
            utterance_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            correlation_id: None,
            trace_kind: TraceKind::Original,
            parent_trace_id: None,
            timestamp: Utc::now(),
            raw_utterance: "open the case".to_string(),
            is_synthetic: false,
            outcome: TraceOutcome::ExecutedSuccessfully,
            halt_reason_code: None,
            halt_phase: None,
            resolved_verb: Some("kyc.open-case".to_string()),
            plane: None,
            polarity: None,
            execution_shape_kind: None,
            fallback_invoked: false,
            fallback_reason_code: None,
            situation_signature_hash: None,
            template_id: None,
            template_version: None,
            surface_versions: SurfaceVersions::default(),
            trace_payload: serde_json::json!({
                "phase_3": { "phase4_candidate_set": ["kyc.open-case"] },
                "phase_4": { "fallback_invoked": false }
            }),
        };
        let replayed = original.clone();

        let comparison = compare_trace_records(&original, &replayed);
        assert_eq!(comparison.verdict, ReplayVerdict::Unchanged);
        assert_eq!(
            comparison.original_resolved_verb.as_deref(),
            Some("kyc.open-case")
        );
    }

    #[test]
    fn test_compare_trace_sequences_zips_in_order() {
        let base = UtteranceTraceRecord {
            trace_id: Uuid::new_v4(),
            utterance_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            correlation_id: None,
            trace_kind: TraceKind::Original,
            parent_trace_id: None,
            timestamp: Utc::now(),
            raw_utterance: "open the case".to_string(),
            is_synthetic: false,
            outcome: TraceOutcome::ExecutedSuccessfully,
            halt_reason_code: None,
            halt_phase: None,
            resolved_verb: Some("kyc.open-case".to_string()),
            plane: None,
            polarity: None,
            execution_shape_kind: None,
            fallback_invoked: false,
            fallback_reason_code: None,
            situation_signature_hash: None,
            template_id: None,
            template_version: None,
            surface_versions: SurfaceVersions::default(),
            trace_payload: serde_json::json!({
                "phase_3": { "phase4_candidate_set": ["kyc.open-case"] },
                "phase_4": { "fallback_invoked": false }
            }),
        };
        let mut second = base.clone();
        second.trace_id = Uuid::new_v4();

        let comparisons = compare_trace_sequences(&[base.clone(), second.clone()], &[base, second]);
        assert_eq!(comparisons.len(), 2);
        assert!(comparisons
            .iter()
            .all(|item| item.verdict == ReplayVerdict::Unchanged));
    }
}
