//! `LegalityGrant` — the single minted answer to "is this legal" for a turn.
//!
//! T11.1b/slice 2 (2026-07-12): the boundary trace over `agent/orchestrator.rs`
//! found the SemOS envelope / `SessionVerbSurface` / Phase 2 legality bundle
//! was independently re-derived at multiple call sites (`prepare_turn_context`,
//! `legacy_handle_utterance`, `handle_utterance_with_forced_verb`), each
//! hand-rolling the same `resolve_sem_reg_verbs` -> `VerbSurfaceContext` ->
//! `compute_session_verb_surface` -> `Phase2Service::evaluate*` sequence —
//! with real drift between the copies (`legacy_handle_utterance` never loaded
//! `composite_state`/`entity_state`, a latent gap `prepare_turn_context`'s
//! copy didn't have). `mint_legality_grant` is the one implementation; callers
//! that need a verdict call it instead of re-deriving their own.
//!
//! Not in scope for this grant: the TOCTOU recheck in `legacy_handle_utterance`
//! (drift detection against an already-minted grant) deliberately stays a
//! lighter, envelope-only `resolve_sem_reg_verbs` call — it's a staleness
//! comparison, not a legality decision, and doesn't need a full surface
//! rebuild. See `docs/todo/control-plane/
//! EOP-TRACE-CONTROLPLANE-T11.1b-SLICE2-ORCHESTRATOR-BOUNDARY-001.md`.
//!
//! Final crate home (per the ratified separation law, `EOP-DESIGN-
//! CONTROLPLANE-T11.1b-SLICE2-ORCHESTRATOR-SPLIT-001.md`) is `ob-poc-
//! control-plane` once T11.2 formalizes the keyed-door mechanism; this module
//! is the CP-tier surface staged ahead of that crate split.

use crate::agent::orchestrator::{resolve_sem_reg_verbs, OrchestratorContext};
use crate::agent::sem_os_context_envelope::SemOsContextEnvelope;
use crate::agent::verb_surface::{
    compute_session_verb_surface, SessionVerbSurface, VerbSurfaceContext, VerbSurfaceFailPolicy,
};
use crate::traceability::{Phase2Evaluation, Phase2Service};

/// The minted, per-turn legality verdict: SemOS envelope, ranked verb
/// surface, and Phase 2 evaluation, resolved together from one context read.
///
/// To agent-tier code this is an advisory hint (rank/constrain candidates,
/// never recomputed). To CP-tier code (today: the mint call site itself;
/// post-T11.2: the floor/gates) it is the verdict, recomputed at decision
/// time — never trusted from a stale grant.
pub(crate) struct LegalityGrant {
    pub envelope: SemOsContextEnvelope,
    pub surface: SessionVerbSurface,
    pub phase2: Phase2Evaluation,
    #[allow(dead_code)]
    pub fail_policy: VerbSurfaceFailPolicy,
    #[allow(dead_code)]
    pub composite_state: Option<crate::agent::composite_state::GroupCompositeState>,
}

/// Mint a `LegalityGrant` for this turn.
///
/// `lookup_result` feeds `Phase2Service::evaluate` (entity-linking recovery
/// signals); pass `None` when the caller has no lookup result to hand (e.g.
/// `handle_utterance_with_forced_verb`, which validates a user-selected verb
/// rather than discovering one).
pub(crate) async fn mint_legality_grant(
    ctx: &OrchestratorContext,
    utterance: &str,
    sage_intent: Option<&crate::sage::OutcomeIntent>,
    semreg_entity_kind: Option<&str>,
    use_generic_task_subject: bool,
    lookup_result: Option<&crate::lookup::LookupResult>,
) -> LegalityGrant {
    let envelope = resolve_sem_reg_verbs(
        ctx,
        utterance,
        sage_intent,
        semreg_entity_kind,
        use_generic_task_subject,
    )
    .await;

    let fail_policy = if ctx.policy_gate.semreg_fail_closed() {
        VerbSurfaceFailPolicy::FailClosed
    } else {
        VerbSurfaceFailPolicy::FailOpen
    };
    let has_group_scope = ctx.scope.as_ref().and_then(|s| s.client_group_id).is_some();

    #[cfg(feature = "database")]
    let composite_state = if has_group_scope {
        let cbu_ids: Vec<uuid::Uuid> = ctx.session_cbu_ids.as_deref().unwrap_or(&[]).to_vec();
        if !cbu_ids.is_empty() {
            match crate::agent::composite_state_loader::load_group_composite_state(
                &ctx.pool, &cbu_ids,
            )
            .await
            {
                Ok(state) => state,
                Err(e) => {
                    tracing::warn!("Failed to load composite state: {e}");
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };
    #[cfg(not(feature = "database"))]
    let composite_state: Option<crate::agent::composite_state::GroupCompositeState> = None;

    // SemOS is the source of truth for current state — the state machine
    // determines which verbs are reachable. Every utterance is a delta
    // against current state.
    let grounded_entity_state: Option<String> = envelope
        .grounded_action_surface
        .as_ref()
        .and_then(|gas| gas.current_state.clone());

    let surface_ctx = VerbSurfaceContext {
        agent_mode: ctx.agent_mode,
        stage_focus: ctx.stage_focus.as_deref(),
        envelope: &envelope,
        fail_policy,
        entity_state: grounded_entity_state.as_deref(),
        has_group_scope,
        is_infrastructure_scope: ctx
            .scope
            .as_ref()
            .and_then(|s| s.client_group_id)
            .is_some_and(|id| id == uuid::Uuid::nil()),
        composite_state: composite_state.as_ref(),
    };
    let surface = compute_session_verb_surface(&surface_ctx);

    let phase2 = Phase2Service::evaluate(lookup_result.cloned(), Some(envelope.clone()));

    LegalityGrant {
        envelope,
        surface,
        phase2,
        fail_policy,
        composite_state,
    }
}

/// Lighter, envelope-only legality check — resolves the SemOS envelope and
/// evaluates Phase 2, without building a `SessionVerbSurface` or loading
/// composite state. For call sites that are verifying a single already-known
/// verb (TOCTOU staleness recheck, forced-verb re-entry) rather than
/// discovering/ranking candidates against a surface — the shape those call
/// sites had before this refactor, now shared as one implementation instead
/// of two ad hoc copies.
pub(crate) async fn verify_envelope_legality(
    ctx: &OrchestratorContext,
    utterance: &str,
    sage_intent: Option<&crate::sage::OutcomeIntent>,
    semreg_entity_kind: Option<&str>,
    use_generic_task_subject: bool,
) -> (SemOsContextEnvelope, Phase2Evaluation) {
    let envelope = resolve_sem_reg_verbs(
        ctx,
        utterance,
        sage_intent,
        semreg_entity_kind,
        use_generic_task_subject,
    )
    .await;
    let phase2 = Phase2Service::evaluate_from_envelope(envelope.clone());
    (envelope, phase2)
}
