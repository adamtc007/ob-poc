//! Obligation fold (§4.2 of EOP-DD-KYCUBO-001 / V&S §7.3–§7.5).
//!
//! Q4 ratified: person overall-state is a *fold* over parallel per-obligation
//! tracks; approval gates on all-required-terminal (K-23).
//!
//! The same natural person appearing under multiple bases (shareholder +
//! director) folds into ONE subject with DISTINCT basis-obligations (K-21/22).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::event::IntentEvent;
use crate::types::{EventId, ObligationId, SubjectId};

// ── Obligation basis (K-21) ───────────────────────────────────────────────────

/// Why this subject is in KYC scope.  Never inferred; always recorded by the
/// emitting verb (K-21, K-35).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObligationBasis {
    /// The role that triggered this obligation.
    pub role: String,
    /// Jurisdiction driving the requirement, if applicable.
    pub jurisdiction: Option<String>,
    /// CBU / Deal role linking obligation to exposure (K-24).
    pub cbu_role: Option<String>,
    /// The event that established this basis (K-35 traceability).
    pub source_event_id: EventId,
}

// ── Per-obligation tracks (Q4: parallel) ─────────────────────────────────────

/// State of one obligation track.  Tracks advance independently; all-required-
/// terminal enables the subject approval gate (Q4, K-23).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackState {
    Pending,
    InProgress,
    Satisfied { by_event: EventId },
    Waived { by_event: EventId, reason: String },
    Deferred { by_event: EventId },
    Expired { by_event: EventId },
    Rejected { by_event: EventId },
}

impl TrackState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TrackState::Satisfied { .. }
                | TrackState::Waived { .. }
                | TrackState::Expired { .. }
                | TrackState::Rejected { .. }
        )
    }

    /// Text name for projection storage (round-trips with `Display`).
    pub fn state_name(&self) -> &'static str {
        match self {
            TrackState::Pending => "Pending",
            TrackState::InProgress => "InProgress",
            TrackState::Satisfied { .. } => "Satisfied",
            TrackState::Waived { .. } => "Waived",
            TrackState::Deferred { .. } => "Deferred",
            TrackState::Expired { .. } => "Expired",
            TrackState::Rejected { .. } => "Rejected",
        }
    }
}

/// Parallel per-obligation tracks for one subject (Q4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObligationTracks {
    pub obligation_id: ObligationId,
    /// The reason this obligation exists (K-21).
    pub basis: ObligationBasis,
    /// Identity verification track.
    pub identity: TrackState,
    /// Screening track (PEP/sanctions/adverse-media — K-26).
    pub screening: TrackState,
    /// Risk assessment track.
    pub risk: TrackState,
    /// The originating event that created this obligation (K-35).
    pub originating_event_id: EventId,
}

impl ObligationTracks {
    /// Whether all required tracks for this obligation are terminal (Q4, K-23).
    pub fn all_required_terminal(&self) -> bool {
        self.identity.is_terminal() && self.screening.is_terminal() && self.risk.is_terminal()
    }
}

// ── Subject rollup ────────────────────────────────────────────────────────────

/// The overall KYC state of one subject — a fold over its obligation tracks.
/// One subject may carry many obligations (K-22: one identity, many obligations).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubjectOverallState {
    /// At least one obligation exists; at least one is not terminal.
    InProgress,
    /// All obligations are terminal — eligible for the approval gate.
    AllTerminal,
    // `Approved`/`Rejected` retired TS.6 P2 (K-G7): `kyc_ubo.decide.subject.approve`/
    // `kyc_ubo.decide.subject.reject` no longer append to the fact stream (structural
    // Evaluation-pack split — `ob-poc-kyc-decide`, no dependency on
    // `ob-poc-kyc-seam`), so this fold can never observe a decision event
    // again. The K-23 "decision is final" finality check moved with them —
    // `kyc_ubo.decide.subject.approve`/`kyc_ubo.decide.subject.reject` query `kyc_decision_records`
    // directly rather than relying on this fold to have seen their own
    // prior event. Obligation state describes the world Assembly builds;
    // decisions are Evaluation's own record, never smeared back in here.
}

/// One subject's consolidated KYC view (K-22: identity record distinct from
/// obligations; K-21: each obligation has a distinct basis).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubjectRollup {
    pub subject_id: SubjectId,
    /// All obligations for this subject, keyed by ObligationId.
    /// Each has its own distinct basis (K-21).
    pub obligations: Vec<ObligationId>,
    // `overall_state` removed TS.6 §5 (2026-08-22). It was a STORED copy of
    // what `derive_subject_state` computes, and the only two events that ever
    // updated it were `kyc.person.approve`/`.reject` — both retired to the
    // Evaluation pack in TS.6 P2. From that moment it was pinned at
    // `InProgress` for every subject forever, while the real fold happily
    // reached `AllTerminal`: a fold-blind stored field, the K-G7 defect class.
    // `derive_subject_state` already ignored it; the projector did not, and so
    // published a permanently-wrong `overall_state`/`all_terminal` pair.
    // Derive it — there is one authority for this value, not two.
}

// ── Obligation state ──────────────────────────────────────────────────────────

/// The folded obligation graph for one `SubjectId`.
/// `state = fold_obligations(events)`.
#[derive(Debug, Default, Clone)]
pub struct ObligationState {
    /// All obligations keyed by ObligationId.
    pub obligations: BTreeMap<ObligationId, ObligationTracks>,
    /// Subject rollups keyed by SubjectId.
    /// One PersonId → one SubjectId → ≥1 obligations (K-22).
    pub subjects: BTreeMap<SubjectId, SubjectRollup>,
}

impl ObligationState {
    /// Derive the overall state of `subject_id` from its obligation tracks.
    /// This is the fold over parallel tracks (Q4).
    pub fn derive_subject_state(&self, subject_id: SubjectId) -> SubjectOverallState {
        let rollup = match self.subjects.get(&subject_id) {
            Some(r) => r,
            None => return SubjectOverallState::InProgress,
        };
        // Fold over obligations.
        if rollup.obligations.is_empty() {
            return SubjectOverallState::InProgress;
        }
        let all_terminal = rollup.obligations.iter().all(|oid| {
            self.obligations
                .get(oid)
                .map(|t| t.all_required_terminal())
                .unwrap_or(false)
        });
        if all_terminal {
            SubjectOverallState::AllTerminal
        } else {
            SubjectOverallState::InProgress
        }
    }
}

// ── Fold function ─────────────────────────────────────────────────────────────

/// Parse a `SubjectId` from an event target or payload.
fn subject_id_from_event(event: &IntentEvent) -> Option<SubjectId> {
    event.target.subject_root.or_else(|| {
        event
            .payload
            .get("subject_id")
            .and_then(|v| v.as_str())
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
            .map(SubjectId)
    })
}

/// `pub(crate)`: T6.1's unified precondition checker (`fold::control::check_preconditions`,
/// `ObligationExists`) reads the obligation id off the event payload the same
/// way the obligation fold itself does — obligation-scoped verbs never set
/// `TargetBinding.obligation_id` in production (`kyc_stream_ops.rs`), only the
/// payload's `obligation_id` key.
pub(crate) fn obligation_id_from_payload(payload: &serde_json::Value) -> Option<ObligationId> {
    payload
        .get("obligation_id")
        .and_then(|v| v.as_str())
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
        .map(ObligationId)
}

fn str_field(payload: &serde_json::Value, field: &str) -> Option<String> {
    payload.get(field)?.as_str().map(str::to_owned)
}

fn track_state_from_event(event_id: EventId, payload: &serde_json::Value) -> TrackState {
    match payload.get("state").and_then(|v| v.as_str()) {
        Some("in_progress") => TrackState::InProgress,
        Some("satisfied") => TrackState::Satisfied { by_event: event_id },
        Some("waived") => {
            let reason = str_field(payload, "reason").unwrap_or_default();
            TrackState::Waived {
                by_event: event_id,
                reason,
            }
        }
        Some("deferred") => TrackState::Deferred { by_event: event_id },
        Some("expired") => TrackState::Expired { by_event: event_id },
        Some("rejected") => TrackState::Rejected { by_event: event_id },
        _ => TrackState::Pending,
    }
}

/// Apply a single event to `ObligationState` — the inner step of the v1 fold.
///
/// Extracted so `fold/registry.rs` can compose it without duplicating the match.
/// `pub(crate)`: visible to the registry, not to crate consumers.
///
/// `continue` arms from the loop become early `return state` (semantically identical
/// since they skip the rest of the match arm and move to the next event).
pub(crate) fn apply_one_obligation_event(
    mut state: ObligationState,
    event: &IntentEvent,
) -> ObligationState {
    let p = &event.payload;
    match event.verb_fqn.as_str() {
        "kyc_ubo.assert.subject.register" => {
            if let Some(sid) = subject_id_from_event(event) {
                state.subjects.entry(sid).or_insert_with(|| SubjectRollup {
                    subject_id: sid,
                    obligations: vec![],
                });
            }
        }

        "kyc_ubo.assert.obligation.creation" => {
            let Some(sid) = subject_id_from_event(event) else {
                return state;
            };
            let Some(oid) = obligation_id_from_payload(p) else {
                return state;
            };

            let basis = ObligationBasis {
                role: str_field(p, "role").unwrap_or_else(|| "unknown".into()),
                jurisdiction: str_field(p, "jurisdiction"),
                cbu_role: str_field(p, "cbu_role"),
                source_event_id: event.id,
            };

            state.obligations.insert(
                oid,
                ObligationTracks {
                    obligation_id: oid,
                    basis,
                    identity: TrackState::Pending,
                    screening: TrackState::Pending,
                    risk: TrackState::Pending,
                    originating_event_id: event.id,
                },
            );

            state
                .subjects
                .entry(sid)
                .or_insert_with(|| SubjectRollup {
                    subject_id: sid,
                    obligations: vec![],
                })
                .obligations
                .push(oid);
        }

        "kyc_ubo.decide.determination.freeze" => {}

        "kyc_ubo.assert.entity.identity" => {
            if let Some(oid) = obligation_id_from_payload(p) {
                if let Some(tracks) = state.obligations.get_mut(&oid) {
                    tracks.identity = track_state_from_event(event.id, p);
                }
            }
        }
        "kyc_ubo.assert.entity.screening" => {
            if let Some(oid) = obligation_id_from_payload(p) {
                if let Some(tracks) = state.obligations.get_mut(&oid) {
                    tracks.screening = track_state_from_event(event.id, p);
                }
            }
        }
        "kyc_ubo.assert.entity.risk" => {
            if let Some(oid) = obligation_id_from_payload(p) {
                if let Some(tracks) = state.obligations.get_mut(&oid) {
                    tracks.risk = track_state_from_event(event.id, p);
                }
            }
        }

        "kyc_ubo.assert.obligation.satisfaction" => {
            if let Some(oid) = obligation_id_from_payload(p) {
                if let Some(tracks) = state.obligations.get_mut(&oid) {
                    tracks.identity = TrackState::Satisfied { by_event: event.id };
                    tracks.screening = TrackState::Satisfied { by_event: event.id };
                    tracks.risk = TrackState::Satisfied { by_event: event.id };
                }
            }
        }
        "kyc_ubo.assert.obligation.waiver" => {
            let reason = str_field(p, "reason").unwrap_or_default();
            if let Some(oid) = obligation_id_from_payload(p) {
                if let Some(tracks) = state.obligations.get_mut(&oid) {
                    let s = TrackState::Waived {
                        by_event: event.id,
                        reason,
                    };
                    tracks.identity = s.clone();
                    tracks.screening = s.clone();
                    tracks.risk = s;
                }
            }
        }

        // `kyc.person.approve`/`kyc.person.reject` retired from this fold
        // TS.6 P2 (K-G7) — renamed `kyc_ubo.decide.subject.approve`/`kyc_ubo.decide.subject.reject` and
        // moved to `ob-poc-kyc-decide`, which never appends to the fact
        // stream, so these FQNs can no longer appear here. Falls to the
        // catch-all below, same as any other unrecognized/foreign FQN.
        _ => {}
    }
    state
}

/// Pure fold of the ordered event stream onto `ObligationState`.
///
/// Obligations are emitted by:
/// - `kyc_ubo.assert.subject.register` (initial registration)
/// - `kyc_ubo.decide.determination.freeze` (resolved persons → person obligations)
/// - Explicit `kyc_ubo.assert.obligation.creation`
///
/// The same person under multiple bases gets distinct `ObligationId`s but
/// shares one `SubjectId` (K-21/22).
///
/// For version-dispatched replay (D2), use `fold_obligations_versioned` in
/// `fold::registry`.
pub fn fold_obligations(events: &[&IntentEvent]) -> ObligationState {
    events.iter().fold(ObligationState::default(), |st, e| {
        apply_one_obligation_event(st, e)
    })
}
