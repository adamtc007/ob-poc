//! T2.7 (EOP-PLAN-CONTROLPLANE-001): shadow wiring for `ob-poc-control-plane`.
//!
//! Translates already-computed `SemOsContextEnvelope` state into the
//! control plane's `EvaluationContext` (never recomputes verb-surface
//! membership or SemOS pruning — see `ob_poc_control_plane::context`),
//! calls `ob_poc_control_plane::evaluate_shadow`, and persists the report
//! beside the legacy Phase 5 recheck outcome for divergence triage. This
//! module never gates dispatch; persistence is best-effort (failures are
//! logged, never propagated — same posture as `agent::telemetry::store`).

use sem_os_policy::abac::ActorContext;
use uuid::Uuid;

use crate::agent::sem_os_context_envelope::{PruneReason, SemOsContextEnvelope};

/// Builds the T2/T9.1-wired portion of `EvaluationContext`.
///
/// **Wired with real data (not fabricated) as of T9.1c/T9.1d
/// (EOP-PLAN-CONTROLPLANE-001 Addendum B):**
/// - G1 (intent admission, T2.1): `envelope.allowed_verbs`/`pruned_verbs`.
/// - G5 (authority, T9.1c): `access_decision` is `Deny` iff `envelope.pruned_verbs`
///   carries an `AbacDenied` entry for this verb, else `Allow` — the only
///   authority-specific signal this call site has. `actor_id`/`role` come
///   from the SAME `ActorContext` this call site already resolves for the
///   G1 check (`sequencer.rs`'s `phase5_runtime_recheck`), not a new or
///   divergent actor-resolution mechanism.
/// - G6 (evidence, T9.1d): `evidence_gaps` maps directly from
///   `envelope.evidence_gaps` (SemOS's own real governance/evidence
///   computation, already run to build the envelope) — no new source. The
///   KYC-specific fields (`kyc_precondition_failures`,
///   `*_obligation_ids`) stay empty: no KYC-substrate adapter is wired at
///   this call site, and most verbs dispatched through Path A are not
///   KYC-domain verbs at all. This makes the resulting `EvidenceOutcome`
///   `Sufficient` for the common case and `MissingRequiredEvidence`
///   whenever SemOS itself detected a gap — never a fabricated split
///   finer than what's actually observed.
///
/// **Still `not_evaluated` (G3/G4/G7 — T9.1a/b/e, deferred, see the
/// ownership ledger):** G3 needs a resolved SemOS *pack* identifier, which
/// is a distinct concept from `envelope`'s constellation family/map (V&S
/// §1.1's own naming note exists specifically to prevent conflating the
/// two) and isn't exposed on `SemOsContextEnvelope` today — wiring it
/// requires understanding the Domain Pack registry, not just reading a
/// field. G4 needs a real proposed state transition (current entity state
/// + declared to-state), which needs a DB read against the DAG/slot-state
/// machinery this call site doesn't currently perform. G7 needs a richer
/// write-set (tables, columns, state slots — not just entity ids) than
/// the legacy `derive_write_set_heuristic` can produce, and that
/// legacy function needs parsed verb args this call site doesn't have
/// (only the raw DSL string). None of these three should be guessed at;
/// each is real follow-on integration work.
///
/// `is_ai_originated`/`interpretation_attested` are conservatively `false`
/// (no attestation requirement applied) because this call site has no
/// Sage-pre-classification / intent-telemetry signal threaded through yet
/// (V&S §6.13.1's attestation source is net-new per T2.1's module doc) —
/// marking every intent as AI-originated without a real attestation signal
/// would make G1 fail unconditionally in shadow, which is not an honest
/// reflection of anything this call site actually observed.
pub(crate) fn build_evaluation_context(
    envelope: &SemOsContextEnvelope,
    verb_fqn: &str,
    intent_id: Uuid,
    actor: &ActorContext,
) -> ob_poc_control_plane::context::EvaluationContext {
    let is_admitted = envelope.allowed_verbs.contains(verb_fqn);
    let exclusion_reasons = envelope
        .pruned_verbs
        .iter()
        .filter(|pruned| pruned.fqn == verb_fqn)
        .map(|pruned| format!("{:?}", pruned.reason))
        .collect();

    let abac_denied = envelope.pruned_verbs.iter().find(|pruned| {
        pruned.fqn == verb_fqn && matches!(pruned.reason, PruneReason::AbacDenied { .. })
    });
    let access_decision = if abac_denied.is_some() {
        ob_poc_control_plane::authority_gate::AccessDecisionKind::Deny
    } else {
        ob_poc_control_plane::authority_gate::AccessDecisionKind::Allow
    };
    let deny_reason = abac_denied.map(|pruned| format!("{:?}", pruned.reason));

    ob_poc_control_plane::context::EvaluationContext {
        intent_admission: Some(ob_poc_control_plane::intent_admission::IntentAdmissionInput {
            intent_id,
            verb_fqn: verb_fqn.to_string(),
            is_admitted,
            exclusion_reasons,
            is_ai_originated: false,
            interpretation_attested: false,
        }),
        authority: Some(ob_poc_control_plane::authority_gate::AuthorityInput {
            actor_id: actor.actor_id.clone(),
            role: actor.roles.join(","),
            access_decision,
            deny_reason,
            // Not runtime-observable at this call site (T4.3's verify_pins
            // has zero production call sites — see the ownership ledger);
            // `false` honestly means "no TOCTOU check occurred here", not
            // "no drift exists". Same posture for the three flags below —
            // no signal source exists yet, so they stay at their safe
            // (non-blocking) default rather than a guessed value.
            toctou_drifted: false,
            requires_human_approval: false,
            requires_second_line_review: false,
            segregation_of_duties_violated: false,
        }),
        evidence: Some(ob_poc_control_plane::evidence_gate::EvidenceInput {
            evidence_gaps: envelope.evidence_gaps.clone(),
            // No KYC-substrate adapter wired at this call site (T9.1d
            // scope) — most Path A dispatches are not KYC-domain verbs at
            // all. Leaving these empty is honest: it means "not observed
            // here", not "confirmed absent".
            kyc_precondition_failures: Vec::new(),
            satisfied_obligation_ids: Vec::new(),
            open_obligation_ids: Vec::new(),
        }),
        ..Default::default()
    }
}

/// One row for `"ob-poc".control_plane_shadow_decisions`.
#[derive(Debug, Clone)]
pub(crate) struct ShadowDecisionRow {
    pub session_id: Uuid,
    pub entry_id: Uuid,
    pub verb_fqn: String,
    pub gate_results: serde_json::Value,
    pub legacy_outcome_blocked: bool,
    pub shadow_intent_admission_blocked: bool,
    pub diverged: bool,
}

/// Serialises an `EvaluationReport` into the `gate_results` JSONB column:
/// `{"IntentAdmission": "Success", "PackResolution": "NotEvaluated { blocked_by: [...] }", ...}`.
fn report_to_json(report: &ob_poc_control_plane::gate::EvaluationReport) -> serde_json::Value {
    let map: serde_json::Map<String, serde_json::Value> = ob_poc_control_plane::gate::GateId::ALL
        .iter()
        .map(|id| {
            let rendered = report
                .get(*id)
                .map(|result| format!("{result:?}"))
                .unwrap_or_else(|| "missing".to_string());
            (format!("{id:?}"), serde_json::Value::String(rendered))
        })
        .collect();
    serde_json::Value::Object(map)
}

/// Builds the persistable row: compares the shadow G1 outcome against the
/// legacy Phase 5 recheck's block/allow decision for this entry.
pub(crate) fn build_shadow_decision_row(
    session_id: Uuid,
    entry_id: Uuid,
    verb_fqn: &str,
    report: &ob_poc_control_plane::gate::EvaluationReport,
    legacy_outcome_blocked: bool,
) -> ShadowDecisionRow {
    let shadow_intent_admission_blocked = !matches!(
        report.get(ob_poc_control_plane::gate::GateId::IntentAdmission),
        Some(&ob_poc_control_plane::gate::GateResult::Success)
    );

    ShadowDecisionRow {
        session_id,
        entry_id,
        verb_fqn: verb_fqn.to_string(),
        gate_results: report_to_json(report),
        legacy_outcome_blocked,
        shadow_intent_admission_blocked,
        diverged: shadow_intent_admission_blocked != legacy_outcome_blocked,
    }
}

/// Best-effort insert. Never returns `Err` — a shadow-decision persistence
/// failure must not affect the request it was observing.
#[cfg(feature = "database")]
pub(crate) async fn insert_shadow_decision(pool: &sqlx::PgPool, row: &ShadowDecisionRow) -> bool {
    let result = sqlx::query(
        r#"
        INSERT INTO "ob-poc".control_plane_shadow_decisions (
            session_id, entry_id, verb_fqn, gate_results,
            legacy_outcome_blocked, shadow_intent_admission_blocked, diverged
        ) VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(row.session_id)
    .bind(row.entry_id)
    .bind(&row.verb_fqn)
    .bind(&row.gate_results)
    .bind(row.legacy_outcome_blocked)
    .bind(row.shadow_intent_admission_blocked)
    .bind(row.diverged)
    .execute(pool)
    .await;

    match result {
        Ok(_) => true,
        Err(err) => {
            tracing::warn!(
                error = %err,
                entry_id = %row.entry_id,
                "control_plane_shadow_decisions insert failed (best-effort, non-blocking)"
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::sem_os_context_envelope::PrunedVerb;

    fn test_actor() -> ActorContext {
        ActorContext {
            actor_id: "actor-1".to_string(),
            roles: vec!["compliance_officer".to_string()],
            department: None,
            clearance: None,
            jurisdictions: Vec::new(),
        }
    }

    #[test]
    fn admitted_verb_builds_true_is_admitted() {
        let envelope = SemOsContextEnvelope::test_with_verbs(&["cbu.confirm"]);
        let ctx = build_evaluation_context(&envelope, "cbu.confirm", Uuid::nil(), &test_actor());
        let input = ctx.intent_admission.expect("intent_admission set");
        assert!(input.is_admitted);
        assert!(input.exclusion_reasons.is_empty());
    }

    #[test]
    fn pruned_verb_carries_stringified_reason() {
        let envelope = SemOsContextEnvelope::test_with_verbs_and_pruned(
            &[],
            vec![PrunedVerb {
                fqn: "cbu.confirm".to_string(),
                reason: PruneReason::AgentModeBlocked {
                    mode: "read_only".to_string(),
                },
            }],
        );
        let ctx = build_evaluation_context(&envelope, "cbu.confirm", Uuid::nil(), &test_actor());
        let input = ctx.intent_admission.expect("intent_admission set");
        assert!(!input.is_admitted);
        assert_eq!(input.exclusion_reasons.len(), 1);
        assert!(input.exclusion_reasons[0].contains("AgentModeBlocked"));
    }

    #[test]
    fn abac_denied_prune_reason_maps_to_authority_deny() {
        let envelope = SemOsContextEnvelope::test_with_verbs_and_pruned(
            &[],
            vec![PrunedVerb {
                fqn: "cbu.confirm".to_string(),
                reason: PruneReason::AbacDenied {
                    actor_role: "viewer".to_string(),
                    required: "compliance_officer".to_string(),
                },
            }],
        );
        let ctx = build_evaluation_context(&envelope, "cbu.confirm", Uuid::nil(), &test_actor());
        let input = ctx.authority.expect("authority set");
        assert_eq!(
            input.access_decision,
            ob_poc_control_plane::authority_gate::AccessDecisionKind::Deny
        );
        assert!(input.deny_reason.is_some());
    }

    #[test]
    fn no_abac_denial_maps_to_authority_allow() {
        let envelope = SemOsContextEnvelope::test_with_verbs(&["cbu.confirm"]);
        let ctx = build_evaluation_context(&envelope, "cbu.confirm", Uuid::nil(), &test_actor());
        let input = ctx.authority.expect("authority set");
        assert_eq!(
            input.access_decision,
            ob_poc_control_plane::authority_gate::AccessDecisionKind::Allow
        );
        assert!(input.deny_reason.is_none());
    }

    #[test]
    fn evidence_gaps_thread_through_from_envelope() {
        let mut envelope = SemOsContextEnvelope::test_with_verbs(&["cbu.confirm"]);
        envelope.evidence_gaps = vec!["missing_source_of_wealth".to_string()];
        let ctx = build_evaluation_context(&envelope, "cbu.confirm", Uuid::nil(), &test_actor());
        let input = ctx.evidence.expect("evidence set");
        assert_eq!(input.evidence_gaps, vec!["missing_source_of_wealth".to_string()]);
    }

    #[test]
    fn divergence_flagged_when_shadow_and_legacy_disagree() {
        let ctx = ob_poc_control_plane::context::EvaluationContext {
            intent_admission: Some(ob_poc_control_plane::intent_admission::IntentAdmissionInput {
                intent_id: Uuid::nil(),
                verb_fqn: "cbu.confirm".to_string(),
                is_admitted: true,
                exclusion_reasons: vec![],
                is_ai_originated: false,
                interpretation_attested: false,
            }),
            ..Default::default()
        };
        let report = ob_poc_control_plane::evaluate_shadow(&ctx);

        // Shadow says admitted (not blocked); legacy says blocked -> diverged.
        let row = build_shadow_decision_row(Uuid::nil(), Uuid::nil(), "cbu.confirm", &report, true);
        assert!(!row.shadow_intent_admission_blocked);
        assert!(row.diverged);

        // Legacy agrees (not blocked) -> no divergence.
        let row = build_shadow_decision_row(Uuid::nil(), Uuid::nil(), "cbu.confirm", &report, false);
        assert!(!row.diverged);
    }
}
