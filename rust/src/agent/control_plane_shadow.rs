//! T2.7 (EOP-PLAN-CONTROLPLANE-001): shadow wiring for `ob-poc-control-plane`.
//!
//! Translates already-computed `SemOsContextEnvelope` state into the
//! control plane's `EvaluationContext` (never recomputes verb-surface
//! membership or SemOS pruning — see `ob_poc_control_plane::context`),
//! calls `ob_poc_control_plane::evaluate_shadow`, and persists the report
//! beside the legacy Phase 5 recheck outcome for divergence triage. This
//! module never gates dispatch; persistence is best-effort (failures are
//! logged, never propagated — same posture as `agent::telemetry::store`).

use std::collections::HashMap;

use sem_os_policy::abac::ActorContext;
use uuid::Uuid;

use crate::agent::sem_os_context_envelope::{PruneReason, SemOsContextEnvelope};
use crate::journey::pack::PackManifest;
use crate::repl::verb_config_index::VerbConfigIndex;

/// T9.1a (EOP-PLAN-CONTROLPLANE-001 Addendum B): builds G3's
/// `PackResolutionInput` from the REPL's real, live single-active-pack
/// session state — not from `sem_os_policy::domain_pack::DomainPackManifest`
/// (the SemOS Domain Pack taxonomy system). This corrects an error in the
/// T9.1-pre design pass, recorded in the ownership ledger's T9.1a entries:
/// `pack_resolution.rs`'s own module doc names its designed production
/// analogue as `src/runbook/constraint_gate.rs::check_pack_constraints`
/// against `journey::pack_manager::{PackManager, EffectiveConstraints}` —
/// and those operate on REPL *journey* packs (`config/packs/*.yaml`, bare
/// ids like `"kyc-case"`), not SemOS Domain Packs (dotted ids like
/// `"ob-poc.cbu"`). The design pass's caution about not conflating "pack"
/// with `constellation_family`/`constellation_map` was correct in
/// principle but pointed at the wrong "other" system — the SemOS Domain
/// Pack taxonomy has no live runtime instance at all (confirmed: zero
/// production `PackManager::new(` call sites, see the T9.1a ledger entry),
/// while the REPL journey pack this function actually uses is real,
/// live, and already tracked (`ReplSessionV2::active_pack_id()`,
/// `ReplOrchestratorV2::pack_router`).
///
/// `PackManager` itself also has zero production callers — resolved here
/// not by building new session-persistent activation tracking (which
/// would duplicate what the REPL already tracks), but by constructing a
/// **fresh, throwaway `PackManager`** per shadow-recheck call: register
/// the single currently-active pack, activate it, and call the exact
/// same `check_pack_constraints` the C-015/C-016 ledger rows say G3 was
/// designed to invoke. `PackManager` is pure in-memory state (`HashMap`s,
/// no I/O) — this is cheap, not a workaround.
///
/// `constraint_denies_intent` is always `false` here, not a placeholder:
/// `EffectiveConstraints::is_empty_intersection()` can only be `true`
/// when the *intersection* of multiple simultaneously-active packs'
/// `allowed_verbs` is empty (`journey/pack_manager.rs::effective_constraints`'s
/// intersection logic) — with exactly one active pack (this REPL's real
/// model; `active_pack_id()` returns a single `Option<String>`, never a
/// set), there is nothing to intersect against, so the condition is
/// unreachable by construction. Whether the verb itself is permitted is
/// carried by `candidate_pack_ids` instead (`vec![pack_id]` when
/// `check_pack_constraints` returns `Ok`, `vec![]` — read as `MissingPack`
/// by `decide()` — when it returns `Err`, i.e. the active pack forbids or
/// doesn't declare this verb).
///
/// **Known limitation, not swept under the rug:** `pack_resolution.rs`'s
/// own doc says "No active pack means no execution" — taken literally,
/// `active_pack_id() == None` always yields `MissingPack`. Some verbs
/// (navigation, `session.*`) legitimately execute outside any pack's
/// InPack tollgate. This may over-report G3 failures for those verbs.
/// Safe because this is shadow-only (never gates real dispatch) — a
/// gating (non-shadow) use of this function would need this resolved
/// first, not inherited as-is.
pub(crate) fn build_pack_resolution_input(
    active_pack: Option<(&str, &PackManifest)>,
    verb_fqn: &str,
    semreg_allowed_set_available: bool,
) -> ob_poc_control_plane::pack_resolution::PackResolutionInput {
    let candidate_pack_ids = match active_pack {
        None => Vec::new(),
        Some((pack_id, manifest)) => {
            let mut manager = crate::journey::pack_manager::PackManager::new();
            manager.register_pack(manifest.clone());
            match manager.activate_pack(pack_id) {
                Ok(()) => {
                    let constraints = manager.effective_constraints();
                    match crate::runbook::constraint_gate::check_pack_constraints(
                        &[verb_fqn.to_string()],
                        &constraints,
                    ) {
                        Ok(()) => vec![pack_id.to_string()],
                        Err(_) => Vec::new(),
                    }
                }
                Err(e) => {
                    // Dormant -> Active should never fail for a freshly
                    // registered pack; if it somehow does, fail honestly
                    // (no candidate) rather than guess.
                    tracing::warn!(
                        error = %e,
                        pack_id,
                        "T9.1a: freshly-registered pack failed to activate — treating as MissingPack"
                    );
                    Vec::new()
                }
            }
        }
    };

    ob_poc_control_plane::pack_resolution::PackResolutionInput {
        candidate_pack_ids,
        semreg_allowed_set_available,
        constraint_denies_intent: false,
    }
}

/// T9.1b (EOP-PLAN-CONTROLPLANE-001 Addendum B): builds G4's
/// `DagProofInput` by reusing the real v1.3 gate's own resolution
/// mechanism — `resolve_transition_probe`, extracted from
/// `pre_dispatch_gate_check`'s original inline body (see that function's
/// doc for the extraction's equivalence proof) — rather than re-deriving
/// verb→transition resolution from scratch. This is the same
/// `entity_id_arg`/`target_state_arg`/`target_workspace`/`target_slot`
/// (`transition_args`, 87 verbs declaring it) and the same
/// `GateChecker::check_transition` the real dispatch-path gate uses.
///
/// `gate_pipeline` is `ReplOrchestratorV2`'s own `GatePipeline` (built at
/// `ob-poc-web::main` startup) — already reachable at this call site
/// since `phase5_runtime_recheck` is a method on the same struct; no new
/// plumbing needed.
///
/// Returns `None` when: no `GatePipeline` is wired (shadow simply has
/// nothing to observe yet — not an error); the verb has no
/// `transition_args` declared (most verbs are not state transitions at
/// all); the DAG has no matching transition for this verb; or resolution
/// itself failed (missing/invalid entity_id arg, unresolvable
/// workspace — logged, not silently promoted to a wrong-but-passing
/// fact, same posture as `build_entity_binding_input`'s entity_facts
/// lookup failure).
///
/// `lifecycle_fail_open_class` stays `None` and
/// `lifecycle_gate_mode_fail_closed` stays `false` — T0.2's
/// `enforce_requires_states_precondition` needs a live `&mut dyn
/// TransactionScope` (designed for real dispatch, not read-only shadow
/// observation); unifying it here is real follow-on work, not silently
/// folded into this tranche (see the ownership ledger).
pub(crate) async fn build_dag_proof_input(
    gate_pipeline: Option<&crate::runbook::step_executor_bridge::GatePipeline>,
    verb_fqn: &str,
    entry_args: &HashMap<String, String>,
) -> Option<ob_poc_control_plane::dag_proof::DagProofInput> {
    let pipe = gate_pipeline?;
    let probe = crate::runbook::step_executor_bridge::resolve_transition_probe(
        pipe,
        verb_fqn,
        |arg| entry_args.get(arg).map(|s| s.as_str()),
    )
    .await;

    match probe {
        Ok(Some(probe)) => Some(ob_poc_control_plane::dag_proof::DagProofInput {
            entity_id: probe.entity_id,
            from_state: probe.from_state,
            to_state: probe.to_state,
            blocking_violations: probe.blocking_violations,
            lifecycle_fail_open_class: None,
            lifecycle_gate_mode_fail_closed: false,
        }),
        Ok(None) => None,
        Err(e) => {
            tracing::warn!(
                verb_fqn,
                error = %e,
                "T9.1b: DAG transition probe resolution failed — G4 shadow-evaluates as not-attempted"
            );
            None
        }
    }
}

/// T9.1-pre (EOP-PLAN-CONTROLPLANE-001 Addendum B): identifies which of a
/// verb's resolved args are entity references, by contract — not by
/// regexing values for UUID shape (`write_set.rs::derive_write_set_heuristic`
/// was explicitly ruled out as a G2 input source during the T9.1-pre design
/// pass: a UUID-shaped string isn't necessarily a bound entity, and a
/// missed one is a silently ungraded binding). Uses
/// `VerbConfigIndex::entries[fqn].args[].lookup_entity_type` (the same
/// contract metadata `write_set.rs`'s A4 contract-driven path already
/// consumes) to find entity-typed args, then resolves each matched arg
/// name against `entry_args` for its UUID value. Args present in the
/// contract but absent from `entry_args`, or whose value doesn't parse as
/// a UUID, are silently skipped — not every entity-typed arg is required
/// to be bound for every verb (optional args), and a non-UUID value here
/// indicates the arg wasn't resolved to an entity reference at all (e.g.
/// still a symbol placeholder), not a binding failure this function should
/// report.
pub(crate) fn entity_binding_requests(
    verb_config_index: &VerbConfigIndex,
    verb_fqn: &str,
    entry_args: &HashMap<String, String>,
) -> Vec<(Uuid, String)> {
    let Some(entry) = verb_config_index.get(verb_fqn) else {
        return Vec::new();
    };
    entry
        .args
        .iter()
        .filter_map(|arg| {
            let entity_type = arg.lookup_entity_type.as_ref()?;
            let raw = entry_args.get(&arg.name)?;
            let id = Uuid::parse_str(raw.trim()).ok()?;
            Some((id, entity_type.clone()))
        })
        .collect()
}

/// Converts a batched [`ob_poc_boundary::entity_facts::EntityFactsRow`]
/// lookup result into G2's `EntityBindingInput`. Every `(entity_id, kind)`
/// in `requests` gets an entry — entities missing from `facts` (the
/// `EntityFactsSource` contract: absent means not found) become an
/// honest `exists: false` fact rather than being silently dropped, so a
/// dangling reference is graded `NotFound`, not skipped.
pub(crate) fn build_entity_binding_input(
    requests: &[(Uuid, String)],
    facts: &HashMap<Uuid, ob_poc_boundary::entity_facts::EntityFactsRow>,
) -> ob_poc_control_plane::entity_binding::EntityBindingInput {
    let entities = requests
        .iter()
        .map(|(id, kind)| match facts.get(id) {
            Some(row) => row.facts.clone(),
            None => ob_poc_control_plane::entity_binding::EntityFacts {
                entity_id: *id,
                exists: false,
                expected_kind: kind.clone(),
                actual_kind: String::new(),
                lifecycle_state_readable: false,
                availability_blocked: false,
                availability_reason: None,
                in_active_pack: false,
            },
        })
        .collect();
    ob_poc_control_plane::entity_binding::EntityBindingInput { entities }
}

/// Builds the T2/T9.1-wired portion of `EvaluationContext`.
///
/// **Wired with real data (not fabricated) as of T9.1c/T9.1d
/// (EOP-PLAN-CONTROLPLANE-001 Addendum B):**
/// - G1 (intent admission, T2.1): `envelope.allowed_verbs`/`pruned_verbs`.
/// - G2 (entity binding, T9.1-pre): `entity_binding` is `Some(input)`
///   whenever the caller *attempted* binding at all — including a verb
///   with zero entity-typed args, which correctly yields
///   `Some(EntityBindingInput { entities: vec![] })`. Per
///   `entity_binding.rs::decide`, an empty `entities` list is vacuous
///   `Success` (nothing to check, so nothing failed) — passing `None`
///   instead for the no-entity-args case would incorrectly turn every
///   entity-less verb (e.g. `session.info`) into a hard
///   `GateResult::Failure("no EntityBindingInput supplied")`, exactly the
///   "guaranteed-wrong signal" class of bug the T9.1c/d empirical probe
///   exists to catch. Reserve `None` for when the caller genuinely
///   couldn't attempt the check at all (no DB access) — an honest "we
///   don't know", appropriately graded `Failure`, not "there was nothing
///   to check". The caller does the I/O
///   (`ob_poc_boundary::entity_facts::EntityFactsSource`, §9.1's
///   decision-assembler law); this function only assembles what it's
///   given via [`build_entity_binding_input`].
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
/// - G3 (pack resolution, T9.1a): built by [`build_pack_resolution_input`]
///   from the REPL's live single-active-pack session state
///   (`ReplSessionV2::active_pack_id()` + `ReplOrchestratorV2::pack_router`)
///   — see that function's doc for the full design, including the
///   correction of the T9.1-pre design pass's original (wrong) assumption
///   that this needed the SemOS Domain Pack taxonomy.
///
/// - G4 (DAG transition proof, T9.1b): built by [`build_dag_proof_input`]
///   by reusing the real v1.3 gate's own resolution mechanism
///   (`step_executor_bridge::resolve_transition_probe`, extracted from
///   `pre_dispatch_gate_check` — see that function's doc) against the
///   `GatePipeline` already carried on `ReplOrchestratorV2`. `None` when
///   no `GatePipeline` is wired, the verb has no `transition_args`
///   declared, or the DAG has no matching transition — all legitimate
///   (most verbs are not state transitions at all). `lifecycle_fail_open_class`
///   stays `None` / `lifecycle_gate_mode_fail_closed` stays `false`: T0.2's
///   `enforce_requires_states_precondition` needs a live `&mut dyn
///   TransactionScope` (it's designed for real dispatch, not a read-only
///   shadow observation) — unifying it here is real follow-on work, not
///   silently folded into this tranche.
///
/// **Still `not_evaluated` (G7 — T9.1e, deferred, see the ownership
/// ledger):** G7 needs a richer write-set (tables, columns, state slots —
/// not just entity ids) than the legacy `derive_write_set_heuristic` can
/// produce, and that legacy function needs parsed verb args this call site
/// doesn't have (only the raw DSL string). Should not be guessed at — real
/// follow-on integration work.
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
    entity_binding: Option<ob_poc_control_plane::entity_binding::EntityBindingInput>,
    pack_resolution: Option<ob_poc_control_plane::pack_resolution::PackResolutionInput>,
    dag_proof: Option<ob_poc_control_plane::dag_proof::DagProofInput>,
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
        entity_binding,
        pack_resolution,
        dag_proof,
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
        let ctx = build_evaluation_context(&envelope, "cbu.confirm", Uuid::nil(), &test_actor(), None, None, None);
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
        let ctx = build_evaluation_context(&envelope, "cbu.confirm", Uuid::nil(), &test_actor(), None, None, None);
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
        let ctx = build_evaluation_context(&envelope, "cbu.confirm", Uuid::nil(), &test_actor(), None, None, None);
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
        let ctx = build_evaluation_context(&envelope, "cbu.confirm", Uuid::nil(), &test_actor(), None, None, None);
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
        let ctx = build_evaluation_context(&envelope, "cbu.confirm", Uuid::nil(), &test_actor(), None, None, None);
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

    // ── T9.1-pre (Addendum B): entity_binding_requests + G2 reachability ──

    fn verb_config_with_entity_arg(
        verb_fqn: &str,
        arg_name: &str,
        entity_type: &str,
    ) -> VerbConfigIndex {
        use crate::repl::verb_config_index::{ArgSummary, VerbIndexEntry};
        let mut index = VerbConfigIndex::empty();
        index.insert_test_entry(VerbIndexEntry {
            fqn: verb_fqn.to_string(),
            description: String::new(),
            invocation_phrases: Vec::new(),
            sentence_templates: Vec::new(),
            sentences: None,
            args: vec![ArgSummary {
                name: arg_name.to_string(),
                arg_type: "uuid".to_string(),
                required: true,
                description: None,
                maps_to: None,
                lookup_entity_type: Some(entity_type.to_string()),
            }],
            crud_key: None,
            confirm_policy: crate::repl::runbook::ConfirmPolicy::Always,
            precondition_checks: Vec::new(),
        });
        index
    }

    #[test]
    fn entity_binding_requests_finds_contract_typed_arg() {
        let index = verb_config_with_entity_arg("cbu.confirm", "cbu-id", "cbu");
        let id = Uuid::new_v4();
        let mut args = HashMap::new();
        args.insert("cbu-id".to_string(), id.to_string());

        let requests = entity_binding_requests(&index, "cbu.confirm", &args);
        assert_eq!(requests, vec![(id, "cbu".to_string())]);
    }

    #[test]
    fn entity_binding_requests_skips_unresolved_and_non_uuid_values() {
        let index = verb_config_with_entity_arg("cbu.confirm", "cbu-id", "cbu");

        // Arg not present in entry_args at all.
        let requests = entity_binding_requests(&index, "cbu.confirm", &HashMap::new());
        assert!(requests.is_empty());

        // Arg present but not a UUID (unresolved symbol placeholder).
        let mut args = HashMap::new();
        args.insert("cbu-id".to_string(), "@some-symbol".to_string());
        let requests = entity_binding_requests(&index, "cbu.confirm", &args);
        assert!(requests.is_empty());
    }

    #[test]
    fn entity_binding_requests_empty_for_verb_with_no_entity_args() {
        let index = VerbConfigIndex::empty();
        let requests = entity_binding_requests(&index, "session.info", &HashMap::new());
        assert!(requests.is_empty());
    }

    #[test]
    fn build_entity_binding_input_marks_missing_facts_as_not_found() {
        let id = Uuid::new_v4();
        let requests = vec![(id, "cbu".to_string())];
        let facts = HashMap::new(); // batched lookup found nothing for `id`
        let input = build_entity_binding_input(&requests, &facts);
        assert_eq!(input.entities.len(), 1);
        assert!(!input.entities[0].exists);
        assert_eq!(input.entities[0].expected_kind, "cbu");
    }

    #[test]
    fn empty_entity_binding_input_is_vacuous_success_not_failure() {
        // The doc-corrected contract this test locks in: a verb with zero
        // entity-typed args must pass G2 (Some(entities: vec![]) ->
        // vacuous Success), not fail it via a spurious None.
        let envelope = SemOsContextEnvelope::test_with_verbs(&["session.info"]);
        let entity_binding = Some(ob_poc_control_plane::entity_binding::EntityBindingInput {
            entities: Vec::new(),
        });
        let ctx = build_evaluation_context(
            &envelope,
            "session.info",
            Uuid::nil(),
            &test_actor(),
            entity_binding,
            None,
            None,
        );
        let report = ob_poc_control_plane::evaluate_shadow(&ctx);
        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::EntityBinding),
            Some(&ob_poc_control_plane::gate::GateResult::Success)
        );
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn g2_reaches_success_end_to_end_against_a_real_cbu_row() {
        // Empirical reachability proof (this session's established
        // discipline — verified via evaluate_shadow, not assumed from
        // reading GATE_DEPENDENCIES): contract-typed arg detection ->
        // real batched DB fetch -> EntityBindingInput -> evaluate_shadow
        // actually reports G2 Success, for a verb whose only prerequisite
        // (per GATE_DEPENDENCIES, EntityBinding has none) is satisfied.
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for db-integration tests");
        let pool = sqlx::PgPool::connect(&url).await.expect("connect");

        let cbu_id: Uuid = sqlx::query_scalar(r#"SELECT cbu_id FROM "ob-poc".cbus LIMIT 1"#)
            .fetch_one(&pool)
            .await
            .expect("at least one cbu row exists in the dev database");

        let index = verb_config_with_entity_arg("test.verb-with-cbu-arg", "cbu-id", "cbu");
        let mut args = HashMap::new();
        args.insert("cbu-id".to_string(), cbu_id.to_string());

        let requests = entity_binding_requests(&index, "test.verb-with-cbu-arg", &args);
        assert_eq!(requests, vec![(cbu_id, "cbu".to_string())]);

        let source = ob_poc_boundary::entity_facts::PgEntityFactsSource { pool: &pool };
        let facts = ob_poc_boundary::entity_facts::EntityFactsSource::entity_facts(&source, &requests)
            .await
            .expect("batched fetch succeeds");

        let entity_binding = Some(build_entity_binding_input(&requests, &facts));
        let envelope = SemOsContextEnvelope::test_with_verbs(&["test.verb-with-cbu-arg"]);
        let ctx = build_evaluation_context(
            &envelope,
            "test.verb-with-cbu-arg",
            Uuid::nil(),
            &test_actor(),
            entity_binding,
            None,
            None,
        );
        let report = ob_poc_control_plane::evaluate_shadow(&ctx);

        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::EntityBinding),
            Some(&ob_poc_control_plane::gate::GateResult::Success),
            "G2 must report a real, non-not_evaluated Success against a real cbu row"
        );
    }

    /// Historical snapshot (T9.1-pre, before T9.1a landed): with G2 real
    /// but `pack_resolution: None` explicitly passed, PackResolution
    /// reports its own genuine `Failure("no PackResolutionInput
    /// supplied")` rather than staying blocked by EntityBinding, and
    /// Authority/Evidence are blocked *solely* by PackResolution — this
    /// is what motivated T9.1a. Kept as a regression check on the
    /// `pack_resolution: None` code path specifically (a caller that
    /// can't or doesn't supply pack data), not a claim that G3 is
    /// globally unwired — see `g3_reaches_success_and_unblocks_authority_evidence`
    /// below for the now-real end-to-end path.
    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn g3_none_leaves_authority_and_evidence_blocked() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for db-integration tests");
        let pool = sqlx::PgPool::connect(&url).await.expect("connect");
        let cbu_id: Uuid = sqlx::query_scalar(r#"SELECT cbu_id FROM "ob-poc".cbus LIMIT 1"#)
            .fetch_one(&pool)
            .await
            .expect("cbu row exists");

        let index = verb_config_with_entity_arg("cbu.confirm", "cbu-id", "cbu");
        let mut args = std::collections::HashMap::new();
        args.insert("cbu-id".to_string(), cbu_id.to_string());
        let requests = entity_binding_requests(&index, "cbu.confirm", &args);
        let source = ob_poc_boundary::entity_facts::PgEntityFactsSource { pool: &pool };
        let facts = ob_poc_boundary::entity_facts::EntityFactsSource::entity_facts(&source, &requests)
            .await
            .unwrap();
        let entity_binding = Some(build_entity_binding_input(&requests, &facts));

        let envelope = SemOsContextEnvelope::test_with_verbs(&["cbu.confirm"]);
        let ctx = build_evaluation_context(
            &envelope,
            "cbu.confirm",
            Uuid::nil(),
            &test_actor(),
            entity_binding,
            None,
            None,
        );
        let report = ob_poc_control_plane::evaluate_shadow(&ctx);

        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::EntityBinding),
            Some(&ob_poc_control_plane::gate::GateResult::Success)
        );
        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::Authority),
            Some(&ob_poc_control_plane::gate::GateResult::NotEvaluated {
                blocked_by: vec![ob_poc_control_plane::gate::GateId::PackResolution],
            })
        );
        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::Evidence),
            Some(&ob_poc_control_plane::gate::GateResult::NotEvaluated {
                blocked_by: vec![ob_poc_control_plane::gate::GateId::PackResolution],
            })
        );
    }

    // ── T9.1a (Addendum B): build_pack_resolution_input ────────────────────

    fn test_pack_manifest(pack_id: &str, allowed_verbs: Vec<&str>) -> PackManifest {
        PackManifest {
            id: pack_id.to_string(),
            name: pack_id.to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            invocation_phrases: Vec::new(),
            required_context: Vec::new(),
            optional_context: Vec::new(),
            workspaces: Vec::new(),
            allowed_verbs: allowed_verbs.into_iter().map(String::from).collect(),
            forbidden_verbs: Vec::new(),
            risk_policy: Default::default(),
            required_questions: Vec::new(),
            optional_questions: Vec::new(),
            stop_rules: Vec::new(),
            templates: Vec::new(),
            pack_summary_template: None,
            section_layout: Vec::new(),
            definition_of_done: Vec::new(),
            progress_signals: Vec::new(),
            handoff_target: None,
        }
    }

    #[test]
    fn no_active_pack_yields_missing_pack_candidates() {
        let input = build_pack_resolution_input(None, "cbu.confirm", true);
        assert!(input.candidate_pack_ids.is_empty());
        assert!(!input.constraint_denies_intent);
        assert!(input.semreg_allowed_set_available);
    }

    #[test]
    fn active_pack_allowing_the_verb_resolves_it_as_candidate() {
        let manifest = test_pack_manifest("cbu-maintenance", vec!["cbu.confirm"]);
        let input = build_pack_resolution_input(Some(("cbu-maintenance", &manifest)), "cbu.confirm", true);
        assert_eq!(input.candidate_pack_ids, vec!["cbu-maintenance".to_string()]);
        assert!(!input.constraint_denies_intent);
    }

    #[test]
    fn active_pack_not_declaring_the_verb_yields_no_candidates() {
        let manifest = test_pack_manifest("cbu-maintenance", vec!["cbu.confirm"]);
        let input = build_pack_resolution_input(Some(("cbu-maintenance", &manifest)), "kyc-case.approve", true);
        assert!(input.candidate_pack_ids.is_empty());
        assert!(!input.constraint_denies_intent);
    }

    #[test]
    fn active_pack_forbidding_the_verb_yields_no_candidates() {
        let mut manifest = test_pack_manifest("cbu-maintenance", vec![]); // unconstrained allowed set
        manifest.forbidden_verbs = vec!["cbu.confirm".to_string()];
        let input = build_pack_resolution_input(Some(("cbu-maintenance", &manifest)), "cbu.confirm", true);
        assert!(input.candidate_pack_ids.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn g3_reaches_success_and_unblocks_authority_evidence() {
        // Empirical reachability proof (this session's established
        // discipline): with G2 real (T9.1-pre) and G3 now real (T9.1a),
        // verify via evaluate_shadow — not assumed from GATE_DEPENDENCIES
        // — that Authority/Evidence stop being NotEvaluated once both
        // their prerequisites genuinely succeed.
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for db-integration tests");
        let pool = sqlx::PgPool::connect(&url).await.expect("connect");
        let cbu_id: Uuid = sqlx::query_scalar(r#"SELECT cbu_id FROM "ob-poc".cbus LIMIT 1"#)
            .fetch_one(&pool)
            .await
            .expect("cbu row exists");

        let index = verb_config_with_entity_arg("cbu.confirm", "cbu-id", "cbu");
        let mut args = std::collections::HashMap::new();
        args.insert("cbu-id".to_string(), cbu_id.to_string());
        let requests = entity_binding_requests(&index, "cbu.confirm", &args);
        let source = ob_poc_boundary::entity_facts::PgEntityFactsSource { pool: &pool };
        let facts = ob_poc_boundary::entity_facts::EntityFactsSource::entity_facts(&source, &requests)
            .await
            .unwrap();
        let entity_binding = Some(build_entity_binding_input(&requests, &facts));

        let manifest = test_pack_manifest("cbu-maintenance", vec!["cbu.confirm"]);
        let pack_resolution = Some(build_pack_resolution_input(
            Some(("cbu-maintenance", &manifest)),
            "cbu.confirm",
            true,
        ));

        let envelope = SemOsContextEnvelope::test_with_verbs(&["cbu.confirm"]);
        let ctx = build_evaluation_context(
            &envelope,
            "cbu.confirm",
            Uuid::nil(),
            &test_actor(),
            entity_binding,
            pack_resolution,
            None,
        );
        let report = ob_poc_control_plane::evaluate_shadow(&ctx);

        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::EntityBinding),
            Some(&ob_poc_control_plane::gate::GateResult::Success)
        );
        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::PackResolution),
            Some(&ob_poc_control_plane::gate::GateResult::Success)
        );
        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::Authority),
            Some(&ob_poc_control_plane::gate::GateResult::Success),
            "Authority must reach a real outcome now that its declared dependencies (IntentAdmission, PackResolution) both succeed"
        );
        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::Evidence),
            Some(&ob_poc_control_plane::gate::GateResult::Success),
            "Evidence must reach a real outcome now that its declared dependencies (EntityBinding, PackResolution) both succeed"
        );
    }

    // ── T9.1b (Addendum B): build_dag_proof_input ──────────────────────

    #[tokio::test]
    async fn build_dag_proof_input_none_when_no_gate_pipeline() {
        let args = HashMap::new();
        let dag_proof = build_dag_proof_input(None, "cbu.confirm", &args).await;
        assert!(dag_proof.is_none(), "no GatePipeline wired -> nothing to observe, not an error");
    }

    /// Minimal self-contained GatePipeline fixture — no live DB, no
    /// `harness` feature, same in-memory pattern
    /// `step_executor_bridge`'s equivalence tests use.
    struct FixedSlotState(std::collections::HashMap<(String, String, Uuid), Option<String>>);

    #[async_trait::async_trait]
    impl dsl_runtime::cross_workspace::SlotStateProvider for FixedSlotState {
        async fn read_slot_state(
            &self,
            workspace: &str,
            slot: &str,
            entity_id: Uuid,
            _pool: &sqlx::PgPool,
        ) -> anyhow::Result<Option<String>> {
            Ok(self
                .0
                .get(&(workspace.to_string(), slot.to_string(), entity_id))
                .cloned()
                .unwrap_or(None))
        }
    }

    struct FixedLookup(Option<dsl_core::TransitionArgs>);

    impl crate::runbook::step_executor_bridge::VerbTransitionLookup for FixedLookup {
        fn lookup(&self, _verb_fqn: &str) -> Option<dsl_core::TransitionArgs> {
            self.0.clone()
        }
    }

    const TEST_DAG_YAML: &str = r#"
workspace: testws
dag_id: test_dag
slots:
  - id: testslot
    stateless: false
    state_machine:
      id: sm
      states: [{ id: FROM, entry: true }, { id: TO }]
      transitions:
        - from: FROM
          to: TO
          via: test.transition-verb
cross_workspace_constraints: []
"#;

    fn test_gate_pipeline() -> crate::runbook::step_executor_bridge::GatePipeline {
        let dir = std::env::temp_dir().join(format!("t91b_shadow_test_dag_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("test.yaml"), TEST_DAG_YAML).unwrap();
        let registry =
            std::sync::Arc::new(dsl_runtime::cross_workspace::DagRegistry::from_dir(&dir).unwrap());
        std::fs::remove_dir_all(&dir).ok();

        let gate_checker = std::sync::Arc::new(dsl_runtime::GateChecker::new(
            registry.clone(),
            std::sync::Arc::new(FixedSlotState(Default::default())),
            std::sync::Arc::new(dsl_runtime::cross_workspace::SameEntityResolver),
        ));
        let verb_metadata: std::sync::Arc<dyn crate::runbook::step_executor_bridge::VerbTransitionLookup> =
            std::sync::Arc::new(FixedLookup(Some(dsl_core::TransitionArgs {
                entity_id_arg: "entity-id".into(),
                target_state_arg: None,
                target_workspace: Some("testws".into()),
                target_slot: Some("testslot".into()),
            })));
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://harness-mock-never-connects")
            .expect("connect_lazy with a valid-shaped URL never fails");

        crate::runbook::step_executor_bridge::GatePipeline {
            registry,
            gate_checker,
            verb_metadata,
            pool: std::sync::Arc::new(pool),
            cascade_planner: None,
        }
    }

    #[tokio::test]
    async fn g4_reaches_success_end_to_end_against_a_fixture_dag() {
        // Empirical reachability proof (this session's established
        // discipline, matching g2_reaches_success/g3_reaches_success
        // above): build_dag_proof_input -> build_evaluation_context ->
        // evaluate_shadow actually reports G4 Success for a verb whose
        // declared transition_args resolve cleanly against a legal
        // transition with no blocking violations.
        let pipe = test_gate_pipeline();
        let entity_id = Uuid::new_v4();
        let mut args = HashMap::new();
        args.insert("entity-id".to_string(), entity_id.to_string());

        let dag_proof = build_dag_proof_input(Some(&pipe), "test.transition-verb", &args)
            .await
            .expect("verb has transition_args and a matching DAG transition");
        assert_eq!(dag_proof.entity_id, entity_id);
        assert!(dag_proof.blocking_violations.is_empty());

        // G4 depends on EntityBinding + PackResolution (GATE_DEPENDENCIES)
        // — both must genuinely succeed too, or G4 stays NotEvaluated
        // regardless of dag_proof's own content.
        let entity_binding = Some(ob_poc_control_plane::entity_binding::EntityBindingInput {
            entities: Vec::new(),
        });
        let pack_resolution = Some(ob_poc_control_plane::pack_resolution::PackResolutionInput {
            candidate_pack_ids: vec!["fixture-pack".to_string()],
            semreg_allowed_set_available: true,
            constraint_denies_intent: false,
        });

        let envelope = SemOsContextEnvelope::test_with_verbs(&["test.transition-verb"]);
        let ctx = build_evaluation_context(
            &envelope,
            "test.transition-verb",
            Uuid::nil(),
            &test_actor(),
            entity_binding,
            pack_resolution,
            Some(dag_proof),
        );
        let report = ob_poc_control_plane::evaluate_shadow(&ctx);

        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::DagProof),
            Some(&ob_poc_control_plane::gate::GateResult::Success),
            "G4 must report a real, non-not_evaluated Success against a legal DAG transition"
        );
    }

    #[tokio::test]
    async fn build_dag_proof_input_none_when_dag_has_no_matching_transition() {
        let pipe = test_gate_pipeline();
        let mut args = HashMap::new();
        args.insert("entity-id".to_string(), Uuid::new_v4().to_string());
        // FixedLookup returns Some(transition_args) for every verb_fqn
        // (it doesn't discriminate), but the fixture DAG only declares a
        // transition `via: test.transition-verb` — "unrelated.verb" has
        // no matching TransitionRef, so candidates come back empty ->
        // None, exactly like a real verb with transition_args declared
        // but no matching DAG transition.
        let dag_proof = build_dag_proof_input(Some(&pipe), "unrelated.verb", &args).await;
        assert!(dag_proof.is_none());
    }
}
