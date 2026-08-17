//! T7.1 (`EOP-PLAN-KYCUBO-KIT-T7` §2) — capture telemetry recorder for the
//! plain-English ramp, per `EOP-DD-KYCUBO-CAPTURE-CHARTER` v0.1 §1 (RATIFIED
//! 2026-08-14). Writes to `"ob-poc".kyc_ramp_capture` — deliberately NOT
//! `kyc_intent_events`: this is proposal telemetry about what the ramp
//! showed an operator and what they did with it, not a governed verb-stream
//! event, and must stay independently deletable on the charter's retention
//! schedule (§2) without touching the verb stream's append-only immutability
//! guarantees.
//!
//! **I-6 (T7 §1): only the charter §1 field list is ever persisted.** Adding
//! a field here requires a charter amendment first, not a silent schema
//! change — see `capture_respects_charter_scope` in
//! `rust/tests/kyc_ramp_capture.rs`, which fails on drift.
//!
//! This module records; it never proposes or stages. It has no dependency on
//! `crate::mcp` or `crate::agent` — the ramp itself (T7.2) calls in here, not
//! the other way around.

use semantic_decision_contracts::MoveAttemptOutcome;
use uuid::Uuid;

/// The ramp's own classification of what it showed the operator — per T7 §1
/// I-3, a model may only ever produce scalars; the deterministic disposition
/// policy is what decides this value. This struct only records that
/// decision, it never computes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Select,
    Clarify,
    Abstain,
}

impl Disposition {
    fn as_db_str(self) -> &'static str {
        match self {
            Disposition::Select => "select",
            Disposition::Clarify => "clarify",
            Disposition::Abstain => "abstain",
        }
    }
}

/// What the operator actually did with the proposal — the ground-truth
/// signal for later tuning (charter §1 Amendment 2026-08-17: widened from a
/// local 3-value `UserAction` to `semantic_decision_contracts`'
/// `MoveAttemptOutcome`, the same domain-agnostic terminal-outcome
/// vocabulary `PlacementSet`'s abstention identity already draws from — see
/// `EOP-PLAN-KYCUBO-KIT-T7_Plain-English-Ramp_v0.1.md` §8. Only `Applied` /
/// `Corrected` / `CompilerRefused` are produced by any call site today
/// (`kyc_workbook_surface.rs`'s `Stage` arm); the rest are
/// adopted-but-unpopulated.
fn user_action_db_str(action: MoveAttemptOutcome) -> &'static str {
    match action {
        MoveAttemptOutcome::Applied => "applied",
        MoveAttemptOutcome::Incomplete => "incomplete",
        MoveAttemptOutcome::Ambiguous => "ambiguous",
        MoveAttemptOutcome::Inapplicable => "inapplicable",
        MoveAttemptOutcome::DisclosureSafeRefusal => "disclosure_safe_refusal",
        MoveAttemptOutcome::Stale => "stale",
        MoveAttemptOutcome::CompilerRefused => "compiler_refused",
        MoveAttemptOutcome::RejectedByUser => "rejected_by_user",
        MoveAttemptOutcome::Corrected => "corrected",
        MoveAttemptOutcome::SystemFailure => "system_failure",
    }
}

/// One ramp interaction, per `EOP-DD-KYCUBO-CAPTURE-CHARTER` v0.1 §1. This is
/// the ENTIRE field list the charter permits — do not add fields without a
/// charter amendment (I-6).
#[derive(Debug, Clone)]
pub struct CaptureRecord {
    pub utterance_text: String,
    pub placement_set_hash: String,
    pub proposal: String,
    pub disposition: Disposition,
    pub user_action: MoveAttemptOutcome,
    pub staged_move_id: Option<Uuid>,
}

/// Persists one capture record. Charter §1 scope only — see `CaptureRecord`.
/// Returns the generated row id.
pub async fn record_capture(
    conn: &mut sqlx::PgConnection,
    record: &CaptureRecord,
) -> Result<Uuid, sqlx::Error> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO "ob-poc".kyc_ramp_capture
            (utterance_text, placement_set_hash, proposal, disposition, user_action, staged_move_id)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#,
    )
    .bind(&record.utterance_text)
    .bind(&record.placement_set_hash)
    .bind(&record.proposal)
    .bind(record.disposition.as_db_str())
    .bind(user_action_db_str(record.user_action))
    .bind(record.staged_move_id)
    .fetch_one(&mut *conn)
    .await?;
    Ok(id)
}
