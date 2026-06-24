//! SessionVerbSurface — single queryable type composing all governance layers.
//!
//! Consolidates SemReg CCIR, AgentMode, workflow phase, lifecycle state, and
//! actor gating into one fingerprinted result, computed once per turn.
//!
//! **Safety invariants:**
//! - SI-1: No ungoverned expansion — FailClosed safe-harbor if SemReg unavailable
//! - SI-2: Dual fingerprints — `surface_fingerprint` ≠ `semreg_fingerprint`
//! - SI-3: Exclusion reasons are additive (multi-reason per verb)

use chrono::{DateTime, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

use sem_os_types::agent_mode::AgentMode;

use crate::agent::composite_state::GroupCompositeState;
use crate::agent::sem_os_context_envelope::{
    AllowedVerbSetFingerprint, PruneReason, SemOsContextEnvelope,
};
use crate::dsl_v2::config::types::HarmClass;
use crate::dsl_v2::runtime_registry::{runtime_registry, RuntimeVerb};
use crate::traceability::Phase2Service;

// ── Core types ──────────────────────────────────────────────────

/// The complete, fingerprinted verb surface for a session turn.
///
/// Computed once via `compute_session_verb_surface()` and threaded through
/// the entire pipeline (orchestrator, MCP, chat response, UI).
#[derive(Debug, Clone, Serialize)]
pub struct SessionVerbSurface {
    /// Verbs visible to the user after all governance layers.
    pub verbs: Vec<SurfaceVerb>,
    /// Macro/scenario FQNs owned by the composed workspace (by mode-tag
    /// membership, not FQN leading-domain). Empty when no workspace resolves.
    /// These are admitted into `allowed_fqns()` so the macro/scenario tiers in
    /// verb search can surface them — the tier filters stay; the set widens by
    /// membership. See `crate::agent::workspace_mode_tags`.
    pub owned_macros: Vec<String>,
    /// Verbs excluded with structured, multi-layer reasons.
    pub excluded: Vec<ExcludedVerb>,
    /// SHA-256 fingerprint of the final visible set + filter context.
    /// Distinct from `semreg_fingerprint` (SI-2).
    pub surface_fingerprint: SurfaceFingerprint,
    /// CCIR-internal fingerprint from the SemOsContextEnvelope (if available).
    pub semreg_fingerprint: Option<AllowedVerbSetFingerprint>,
    /// Which fail policy was applied (FailClosed or FailOpen).
    pub fail_policy_applied: VerbSurfaceFailPolicy,
    /// When this surface was computed.
    pub computed_at: DateTime<Utc>,
    /// Progressive narrowing counts at each filter stage.
    pub filter_summary: FilterSummary,
}

/// A verb that survived all governance layers and is visible to the user.
#[derive(Debug, Clone, Serialize)]
pub struct SurfaceVerb {
    pub fqn: String,
    pub domain: String,
    pub action: String,
    pub description: String,
    pub governance_tier: Option<String>,
    pub lifecycle_eligible: bool,
    /// Workflow affinity boost (0.0 = no boost).
    pub rank_boost: f64,
}

/// A verb that was excluded, with all reasons it was pruned.
#[derive(Debug, Clone, Serialize)]
pub struct ExcludedVerb {
    pub fqn: String,
    /// One or more reasons (SI-3: additive, not first-hit).
    pub reasons: Vec<SurfacePrune>,
}

/// A single prune reason tagged with the governance layer that applied it.
#[derive(Debug, Clone, Serialize)]
pub struct SurfacePrune {
    pub layer: PruneLayer,
    pub reason: String,
}

/// Which governance layer excluded the verb.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PruneLayer {
    AgentMode,
    WorkflowPhase,
    GroupScope,
    SemRegCcir,
    LifecycleState,
    ActorGating,
    FailPolicy,
}

/// Fail policy when SemReg is unavailable.
#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerbSurfaceFailPolicy {
    /// Default: reduce to ~30 always-safe verbs.
    #[default]
    FailClosed,
    /// Dev-only: full registry tagged "ungoverned".
    FailOpen,
}

/// SHA-256 fingerprint of the entire surface + filter context.
///
/// Format: `"vs1:<hex>"` (versioned, distinct from CCIR `"v1:<hex>"`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SurfaceFingerprint(pub String);

impl std::fmt::Display for SurfaceFingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Progressive narrowing counts at each filter stage.
#[derive(Debug, Clone, Serialize)]
pub struct FilterSummary {
    pub total_registry: usize,
    pub after_agent_mode: usize,
    pub after_workflow: usize,
    pub after_group_scope: usize,
    pub after_semreg: usize,
    pub after_lifecycle: usize,
    pub final_count: usize,
}

// ── Safe-harbor verbs (FailClosed fallback) ─────────────────────

/// Domain prefixes that are always safe (navigation, help, session management).
const SAFE_HARBOR_DOMAINS: &[&str] = &[
    "agent", "audit", "focus", "registry", "schema", "session", "view",
];

pub fn is_safe_harbor_verb(fqn: &str) -> bool {
    SAFE_HARBOR_DOMAINS
        .iter()
        .any(|domain| fqn.starts_with(&format!("{domain}.")))
}

// ── Bootstrap domains (no group in scope) ────────────────────────
//
// When no client group is set, only these domains are available.
// This forces the user to select a group before doing domain work.
// Exception: new group onboarding (client-group.create, gleif.import-tree).
const NO_GROUP_ALLOWED_DOMAINS: &[&str] = &[
    "agent",
    "audit",
    "client-group",
    "focus",
    "gleif",
    "onboarding",
    "registry",
    "schema",
    "session",
    "view",
];

/// Validate that every fail-closed safe-harbor verb is read-only.
///
/// # Examples
/// ```rust
/// let _ = ob_poc::agent::verb_surface::validate_fail_closed_safe_harbor_harm_class();
/// ```
pub fn validate_fail_closed_safe_harbor_harm_class() -> anyhow::Result<()> {
    let mut violations = Vec::new();
    let mut missing = Vec::new();

    for verb in runtime_registry().all_verbs() {
        if !is_safe_harbor_verb(&verb.full_name) {
            continue;
        }

        match verb.harm_class {
            Some(HarmClass::ReadOnly) => {}
            Some(other) => violations.push(format!("{} ({other:?})", verb.full_name)),
            None => missing.push(verb.full_name.clone()),
        }
    }

    if !missing.is_empty() {
        tracing::warn!(
            count = missing.len(),
            verbs = ?missing,
            "Safe-harbor harm-class audit found verbs without harm_class metadata"
        );
    }

    if violations.is_empty() {
        return Ok(());
    }

    anyhow::bail!(
        "FailClosed safe-harbor contains non-read-only verbs: {}",
        violations.join(", ")
    );
}

// ── Workflow phase → domain allowlists ──────────────────────────

/// Returns an optional set of allowed domain prefixes for a given stage_focus.
///
/// When `None`, no workflow constraint is applied (all domains pass).
fn workflow_allowed_domains(stage_focus: &str) -> Option<HashSet<&'static str>> {
    match stage_focus {
        "semos-onboarding" => Some(
            [
                "cbu",
                "entity",
                "session",
                "view",
                "agent",
                "contract",
                "deal",
                "billing",
                "trading-profile",
                "custody",
                "onboarding",
                "gleif",
                "research",
            ]
            .into_iter()
            .collect(),
        ),
        "semos-kyc" => Some(
            [
                "kyc",
                "screening",
                "document",
                "requirement",
                "ubo",
                "session",
                "view",
                "agent",
                "entity",
            ]
            .into_iter()
            .collect(),
        ),
        "semos-data" | "semos-data-management" => Some(
            [
                "registry",
                "changeset",
                "governance",
                "schema",
                "authoring",
                "deal",
                "cbu",
                "document",
                "product",
                "session",
                "view",
                "agent",
                "audit",
            ]
            .into_iter()
            .collect(),
        ),
        "semos-stewardship" => Some(
            [
                "focus",
                "changeset",
                "governance",
                "audit",
                "maintenance",
                "registry",
                "schema",
                "session",
                "view",
                "agent",
            ]
            .into_iter()
            .collect(),
        ),
        _ => None, // No workflow constraint
    }
}

// ── Computation context ─────────────────────────────────────────

/// Input context for computing the verb surface.
pub struct VerbSurfaceContext<'a> {
    /// Current agent mode (Research vs Governed).
    pub agent_mode: AgentMode,
    /// Session workflow focus (e.g., "semos-kyc", "semos-onboarding").
    pub stage_focus: Option<&'a str>,
    /// Sem OS context resolution result.
    pub envelope: &'a SemOsContextEnvelope,
    /// Fail policy when Sem OS is unavailable.
    pub fail_policy: VerbSurfaceFailPolicy,
    /// Current entity state (e.g., "open", "in_review") for lifecycle filtering.
    /// If None, lifecycle filtering is skipped.
    pub entity_state: Option<&'a str>,
    /// Whether the session has a client group in scope.
    /// When `false`, only bootstrap/onboarding verbs are available.
    /// When `true`, full domain verbs cascade from the group.
    pub has_group_scope: bool,
    /// Whether the session is in infrastructure scope (SemOS maintenance).
    /// When `true`, all domains are available without a client group.
    pub is_infrastructure_scope: bool,
    /// Group composite state for state-to-intent bias.
    /// When present, verb candidates receive score boosts/penalties
    /// based on the "as-is → to-be" gap analysis.
    pub composite_state: Option<&'a GroupCompositeState>,
}

// ── 7-step compute pipeline ─────────────────────────────────────

/// Compute the session verb surface by applying all governance layers.
///
/// 7-step pipeline:
/// 1. Base set from RuntimeVerbRegistry
/// 2. AgentMode filter (Research vs Governed)
/// 3. Scope + workflow filter (group scope + workflow phase, merged)
/// 4. SemReg CCIR (single enforcement point)
/// 5. Lifecycle state filter
/// 6. FailPolicy check (safe-harbor reduction when SemReg unavailable)
/// 7. Rank + composite state bias + fingerprint
pub fn compute_session_verb_surface(ctx: &VerbSurfaceContext<'_>) -> SessionVerbSurface {
    let registry = runtime_registry();

    // Collect all verbs as (fqn, RuntimeVerb ref)
    let all_verbs: Vec<(&str, &RuntimeVerb)> = registry
        .all_verbs()
        .map(|v| (v.full_name.as_str(), v))
        .collect();
    let total_registry = all_verbs.len();

    // Track exclusions: fqn → Vec<SurfacePrune>
    let mut exclusions: HashMap<String, Vec<SurfacePrune>> = HashMap::new();

    // ── Step 2: AgentMode filter ────────────────────────────────
    let after_mode: Vec<(&str, &RuntimeVerb)> = all_verbs
        .into_iter()
        .filter(|(fqn, _)| {
            if ctx.agent_mode.is_verb_allowed(fqn) {
                true
            } else {
                exclusions
                    .entry(fqn.to_string())
                    .or_default()
                    .push(SurfacePrune {
                        layer: PruneLayer::AgentMode,
                        reason: format!("Blocked by {} mode", ctx.agent_mode),
                    });
                false
            }
        })
        .collect();
    let after_agent_mode = after_mode.len();

    // ── Step 3: Scope + workflow filter (merged) ────────────────
    // Three cases:
    //   1. No group → only bootstrap domains (session, view, agent, gleif, etc.)
    //   2. Group set + workflow focus → workflow-specific domains
    //   3. Group set + no workflow → all domains pass through
    let scope_domains: Option<HashSet<&str>> = if ctx.is_infrastructure_scope {
        // Infrastructure scope → all domains available (SemOS maintenance)
        None
    } else if !ctx.has_group_scope {
        // No group → bootstrap domains only
        Some(NO_GROUP_ALLOWED_DOMAINS.iter().copied().collect())
    } else {
        // Group set → workflow domains (if any)
        ctx.stage_focus.and_then(workflow_allowed_domains)
    };
    let allowed_domains = scope_domains.clone();

    let prune_layer = if !ctx.has_group_scope {
        PruneLayer::GroupScope
    } else {
        PruneLayer::WorkflowPhase
    };
    let prune_reason_ctx = if !ctx.has_group_scope {
        "no group in scope"
    } else {
        ctx.stage_focus.unwrap_or("unknown workflow")
    };

    let after_wf: Vec<(&str, &RuntimeVerb)> = if let Some(ref domains) = scope_domains {
        after_mode
            .into_iter()
            .filter(|(fqn, rv)| {
                if domains.contains(rv.domain.as_str()) {
                    true
                } else {
                    exclusions
                        .entry(fqn.to_string())
                        .or_default()
                        .push(SurfacePrune {
                            layer: prune_layer.clone(),
                            reason: format!(
                                "Domain '{}' not allowed ({})",
                                rv.domain, prune_reason_ctx,
                            ),
                        });
                    false
                }
            })
            .collect()
    } else {
        after_mode
    };
    let after_workflow = after_wf.len();
    let after_group_scope = after_wf.len(); // Same step now

    // ── Step 4: SemReg CCIR ────────────────────────────────────
    let phase2 = Phase2Service::evaluate_from_envelope(ctx.envelope.clone());
    let semreg_available = phase2.is_available;
    let semreg_has_verbs = phase2.has_usable_legal_set;
    let phase2_legal_verbs = phase2.legal_verbs_or_empty.clone();

    let after_sr: Vec<(&str, &RuntimeVerb)> = if semreg_has_verbs {
        after_wf
            .into_iter()
            .filter(|(fqn, _)| {
                if phase2_legal_verbs.contains(*fqn) {
                    true
                } else {
                    // Find the specific prune reason from the envelope
                    let reason_str = ctx
                        .envelope
                        .pruned_verbs
                        .iter()
                        .find(|pv| pv.fqn == *fqn)
                        .map(|pv| format_prune_reason(&pv.reason))
                        .unwrap_or_else(|| "Not in SemReg allowed set".to_string());

                    exclusions
                        .entry(fqn.to_string())
                        .or_default()
                        .push(SurfacePrune {
                            layer: PruneLayer::SemRegCcir,
                            reason: reason_str,
                        });
                    false
                }
            })
            .collect()
    } else {
        // SemReg unavailable or deny-all — pass through, handle in Step 7
        after_wf
    };
    let after_semreg = after_sr.len();

    // ── Step 5: Lifecycle eligibility — TAG, never prune (Phase 3 C2) ────
    // Discovery is membership-scoped: an execution gate (`requires_states`)
    // must NEVER remove a classification candidate (invariant M2,
    // "execution gate ≠ discovery filter"). The per-verb `lifecycle_eligible`
    // flag computed in Step 7 carries the state-eligibility signal; the
    // site-441 fast-path gate consults it via `is_lifecycle_eligible`, and
    // executability is validated at execution
    // (`DslExecutor::execute_verb_in_scope`, Phase 3 C1). This step is now a
    // pass-through — `ctx.entity_state` still feeds the Step-7 tag + the
    // fingerprint. (`PruneLayer::LifecycleState` is retained on the enum for
    // wire/back-compat but is no longer produced.)
    let after_lc = after_sr;
    let after_lifecycle = after_lc.len();

    // ── Step 6: FailPolicy check ────────────────────────────────
    let after_fp: Vec<(&str, &RuntimeVerb)> = if !semreg_available {
        match ctx.fail_policy {
            VerbSurfaceFailPolicy::FailClosed => {
                // Reduce to safe-harbor set
                after_lc
                    .into_iter()
                    .filter(|(fqn, _)| {
                        if is_safe_harbor_verb(fqn) {
                            true
                        } else {
                            exclusions
                                .entry(fqn.to_string())
                                .or_default()
                                .push(SurfacePrune {
                                    layer: PruneLayer::FailPolicy,
                                    reason: "SemReg unavailable, FailClosed safe-harbor only"
                                        .to_string(),
                                });
                            false
                        }
                    })
                    .collect()
            }
            VerbSurfaceFailPolicy::FailOpen => {
                // Dev-only: pass through all remaining verbs
                after_lc
            }
        }
    } else {
        after_lc
    };

    // ── Step 7: Rank, group, fingerprint ────────────────────────
    let verbs: Vec<SurfaceVerb> = after_fp
        .iter()
        .map(|(_, rv)| {
            let governance_tier = ctx
                .envelope
                .allowed_verb_contracts
                .iter()
                .find(|c| c.fqn == rv.full_name)
                .map(|c| c.governance_tier.clone());

            let lifecycle_eligible = rv
                .lifecycle
                .as_ref()
                .map(|lc| {
                    if lc.requires_states.is_empty() {
                        true
                    } else if let Some(entity_state) = ctx.entity_state {
                        lc.requires_states.iter().any(|s| s == entity_state)
                    } else {
                        true // No entity → eligible (can't check)
                    }
                })
                .unwrap_or(true);

            let mut rank_boost = compute_rank_boost(rv, ctx.stage_focus, &allowed_domains);

            // State-to-intent bias: boost/penalize based on composite state
            if let Some(composite) = ctx.composite_state {
                let state_boost = composite.compute_state_boost(&rv.full_name);
                rank_boost += state_boost as f64;
            }

            SurfaceVerb {
                fqn: rv.full_name.clone(),
                domain: rv.domain.clone(),
                action: rv.verb.clone(),
                description: rv.description.clone(),
                governance_tier,
                lifecycle_eligible,
                rank_boost,
            }
        })
        .collect();

    let final_count = verbs.len();

    // Membership-owned macros for the composed workspace. The allowed set is
    // atomic verbs of the composed domains UNION macros owned (by mode-tag
    // membership) by this workspace, so the macro/scenario tiers in verb search
    // can surface them instead of matching-then-dropping them. Admission is by
    // membership only — never by the macro FQN's leading-domain token.
    let owned_macros: Vec<String> = ctx
        .stage_focus
        .and_then(crate::agent::workspace_mode_tags::stage_focus_to_workspace)
        .map(|workspace| {
            let mut fqns: Vec<String> =
                crate::agent::workspace_mode_tags::workspace_owned_macro_fqns(workspace)
                    .into_iter()
                    .collect();
            fqns.sort();
            fqns
        })
        .unwrap_or_default();

    // Build excluded list
    let excluded: Vec<ExcludedVerb> = exclusions
        .into_iter()
        .map(|(fqn, reasons)| ExcludedVerb { fqn, reasons })
        .collect();

    // Compute surface fingerprint (includes filter context)
    let surface_fingerprint = compute_surface_fingerprint(
        &verbs,
        &ctx.agent_mode,
        ctx.stage_focus,
        ctx.entity_state,
        ctx.fail_policy,
        ctx.has_group_scope,
    );

    let semreg_fingerprint = if semreg_available {
        Some(ctx.envelope.fingerprint.clone())
    } else {
        None
    };

    SessionVerbSurface {
        verbs,
        owned_macros,
        excluded,
        surface_fingerprint,
        semreg_fingerprint,
        fail_policy_applied: ctx.fail_policy,
        computed_at: Utc::now(),
        filter_summary: FilterSummary {
            total_registry,
            after_agent_mode,
            after_workflow,
            after_group_scope,
            after_semreg,
            after_lifecycle,
            final_count,
        },
    }
}

// ── State reachability observer (eval mode, read-only) ──────────

/// Observe — but DO NOT filter — Step-5 lifecycle reachability over a candidate
/// verb set at a given entity state.
///
/// Applies the identical predicate `compute_session_verb_surface` uses in Step 5
/// (`lifecycle.requires_states` vs `entity_state`), but only TAGS each verb. The
/// candidate order and membership the caller holds are untouched — this is the
/// non-mutating observer the Option C plan mandates, used to size the
/// `state_collapse_counterfactual` (Option A prize) and
/// `post_selection_state_rejection_rate` (Option B cost).
///
/// With `entity_state == None`, lifecycle cannot be checked, so every verb is
/// reported reachable.
pub fn observe_state_reachability(
    fqns: &[String],
    entity_state: Option<&str>,
) -> Vec<crate::agent::telemetry::StateObservation> {
    use crate::agent::telemetry::StateObservation;
    let registry = runtime_registry();
    fqns.iter()
        .map(|fqn| {
            let lifecycle = registry.get_by_name(fqn).and_then(|v| v.lifecycle.as_ref());
            let (state_reachable, failing_predicate) = match (lifecycle, entity_state) {
                (Some(lc), Some(state))
                    if !lc.requires_states.is_empty()
                        && !lc.requires_states.iter().any(|s| s == state) =>
                {
                    (
                        false,
                        Some(format!(
                            "requires_states {:?}, current '{}'",
                            lc.requires_states, state
                        )),
                    )
                }
                _ => (true, None),
            };
            StateObservation {
                verb: fqn.clone(),
                state_reachable,
                failing_predicate,
            }
        })
        .collect()
}

// ── Helpers ─────────────────────────────────────────────────────

fn compute_rank_boost(
    rv: &RuntimeVerb,
    stage_focus: Option<&str>,
    allowed_domains: &Option<HashSet<&str>>,
) -> f64 {
    if let Some(ref domains) = allowed_domains {
        if domains.contains(rv.domain.as_str()) {
            // Extra boost if verb domain matches the first (primary) domain
            if let Some(focus) = stage_focus {
                let primary_domain = match focus {
                    "semos-kyc" => "kyc",
                    "semos-onboarding" => "cbu",
                    "semos-data" | "semos-data-management" => "registry",
                    "semos-stewardship" => "focus",
                    _ => "",
                };
                if rv.domain == primary_domain {
                    return 0.15;
                }
            }
            return 0.05;
        }
    }
    0.0
}

fn compute_surface_fingerprint(
    verbs: &[SurfaceVerb],
    agent_mode: &AgentMode,
    stage_focus: Option<&str>,
    entity_state: Option<&str>,
    fail_policy: VerbSurfaceFailPolicy,
    has_group_scope: bool,
) -> SurfaceFingerprint {
    let mut sorted_fqns: Vec<&str> = verbs.iter().map(|v| v.fqn.as_str()).collect();
    sorted_fqns.sort();

    let mut hasher = Sha256::new();
    for fqn in &sorted_fqns {
        hasher.update(fqn.as_bytes());
        hasher.update(b"\n");
    }
    // Include filter context so identical verb sets with different contexts differ
    hasher.update(format!("mode:{agent_mode}").as_bytes());
    hasher.update(format!("focus:{}", stage_focus.unwrap_or("none")).as_bytes());
    hasher.update(format!("entity_state:{}", entity_state.unwrap_or("none")).as_bytes());
    hasher.update(format!("fail_policy:{fail_policy:?}").as_bytes());
    hasher.update(format!("group_scope:{has_group_scope}").as_bytes());

    let hash = hasher.finalize();
    let hex = hex::encode(hash);
    SurfaceFingerprint(format!("vs1:{hex}"))
}

fn format_prune_reason(reason: &PruneReason) -> String {
    match reason {
        PruneReason::AbacDenied {
            actor_role,
            required,
        } => format!("ABAC denied (actor: {actor_role}, required: {required})"),
        PruneReason::EntityKindMismatch {
            verb_kinds,
            subject_kind,
        } => format!(
            "Entity kind mismatch (verb: {:?}, subject: {subject_kind})",
            verb_kinds
        ),
        PruneReason::AgentModeBlocked { mode } => format!("Blocked by {mode} mode"),
        PruneReason::PolicyDenied { policy_fqn, reason } => {
            format!("Policy denied ({policy_fqn}: {reason})")
        }
    }
}

// ── Convenience methods on SessionVerbSurface ───────────────────

impl SessionVerbSurface {
    /// Get the allowed verb FQNs as a HashSet (for pipeline pre-constraint).
    ///
    /// This is atomic verbs of the composed domains UNION the membership-owned
    /// macro/scenario FQNs (`owned_macros`). Including the macros here is what
    /// lets the macro/scenario tiers in verb search surface them: those tiers
    /// match then drop any FQN not in this set, so a workspace macro must be in
    /// the set to survive. Cross-workspace macros are absent → still dropped.
    pub fn allowed_fqns(&self) -> HashSet<String> {
        let mut fqns: HashSet<String> = self.verbs.iter().map(|v| v.fqn.clone()).collect();
        fqns.extend(self.owned_macros.iter().cloned());
        fqns
    }

    /// Filter verbs by domain.
    pub fn verbs_for_domain(&self, domain: &str) -> Vec<&SurfaceVerb> {
        self.verbs.iter().filter(|v| v.domain == domain).collect()
    }

    /// Get unique domain names in the surface (sorted).
    pub fn domains(&self) -> Vec<&str> {
        let mut domains: Vec<&str> = self
            .verbs
            .iter()
            .map(|v| v.domain.as_str())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        domains.sort();
        domains
    }

    /// Check if a specific verb FQN is in the surface.
    pub fn contains(&self, fqn: &str) -> bool {
        self.verbs.iter().any(|v| v.fqn == fqn)
    }

    /// Whether `fqn` is currently lifecycle-eligible — its `requires_states`
    /// are satisfied by the current entity state (or it has no lifecycle gate).
    /// Verbs absent from the surface (e.g. owned macros) default to eligible.
    ///
    /// Phase 3 C2: discovery no longer prunes on lifecycle; the site-441
    /// fast-path execution gate consults this tag instead of raw membership, so
    /// a state-ineligible verb stays a classification candidate yet is never
    /// auto-executed.
    pub(crate) fn is_lifecycle_eligible(&self, fqn: &str) -> bool {
        self.verbs
            .iter()
            .find(|v| v.fqn == fqn)
            .map(|v| v.lifecycle_eligible)
            .unwrap_or(true)
    }

    /// True if the surface is in safe-harbor mode (FailClosed, SemReg unavailable).
    pub fn is_safe_harbor(&self) -> bool {
        self.fail_policy_applied == VerbSurfaceFailPolicy::FailClosed
            && self.semreg_fingerprint.is_none()
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::sem_os_context_envelope::SemOsContextEnvelope;

    fn make_envelope_with(verbs: &[&str]) -> SemOsContextEnvelope {
        SemOsContextEnvelope::test_with_verbs(verbs)
    }

    fn make_unavailable_envelope() -> SemOsContextEnvelope {
        SemOsContextEnvelope::unavailable()
    }

    #[test]
    fn test_surface_fingerprint_deterministic() {
        let verbs = vec![
            SurfaceVerb {
                fqn: "a.b".into(),
                domain: "a".into(),
                action: "b".into(),
                description: String::new(),
                governance_tier: None,
                lifecycle_eligible: true,
                rank_boost: 0.0,
            },
            SurfaceVerb {
                fqn: "c.d".into(),
                domain: "c".into(),
                action: "d".into(),
                description: String::new(),
                governance_tier: None,
                lifecycle_eligible: true,
                rank_boost: 0.0,
            },
        ];

        let fp1 = compute_surface_fingerprint(
            &verbs,
            &AgentMode::Governed,
            None,
            None,
            VerbSurfaceFailPolicy::FailClosed,
            true,
        );
        let fp2 = compute_surface_fingerprint(
            &verbs,
            &AgentMode::Governed,
            None,
            None,
            VerbSurfaceFailPolicy::FailClosed,
            true,
        );
        assert_eq!(fp1, fp2, "Same inputs must produce same fingerprint");
    }

    #[test]
    fn test_surface_fingerprint_format() {
        let fp = compute_surface_fingerprint(
            &[],
            &AgentMode::Governed,
            None,
            None,
            VerbSurfaceFailPolicy::FailClosed,
            true,
        );
        assert!(
            fp.0.starts_with("vs1:"),
            "Surface fingerprint must start with vs1:"
        );
        // vs1: + 64 hex chars
        assert_eq!(fp.0.len(), 4 + 64);
    }

    #[test]
    fn test_surface_fingerprint_differs_with_context() {
        let verbs = vec![SurfaceVerb {
            fqn: "a.b".into(),
            domain: "a".into(),
            action: "b".into(),
            description: String::new(),
            governance_tier: None,
            lifecycle_eligible: true,
            rank_boost: 0.0,
        }];

        let fp_governed = compute_surface_fingerprint(
            &verbs,
            &AgentMode::Governed,
            None,
            None,
            VerbSurfaceFailPolicy::FailClosed,
            true,
        );
        let fp_research = compute_surface_fingerprint(
            &verbs,
            &AgentMode::Research,
            None,
            None,
            VerbSurfaceFailPolicy::FailClosed,
            true,
        );
        assert_ne!(
            fp_governed, fp_research,
            "Different agent mode must produce different fingerprint"
        );
    }

    #[test]
    fn test_si1_fail_closed_safe_harbor() {
        let envelope = make_unavailable_envelope();

        let ctx = VerbSurfaceContext {
            agent_mode: AgentMode::Governed,
            stage_focus: None,
            envelope: &envelope,
            fail_policy: VerbSurfaceFailPolicy::FailClosed,
            entity_state: None,
            has_group_scope: true,
            is_infrastructure_scope: false,
            composite_state: None,
        };

        let surface = compute_session_verb_surface(&ctx);

        // SI-1: All verbs must be in safe-harbor domains
        for v in &surface.verbs {
            assert!(
                is_safe_harbor_verb(&v.fqn),
                "SI-1 violated: verb '{}' not in safe-harbor set",
                v.fqn
            );
        }
        assert!(surface.is_safe_harbor());
    }

    #[test]
    fn test_si1_fail_open_no_restriction() {
        let envelope = make_unavailable_envelope();

        let ctx = VerbSurfaceContext {
            agent_mode: AgentMode::Governed,
            stage_focus: None,
            envelope: &envelope,
            fail_policy: VerbSurfaceFailPolicy::FailOpen,
            entity_state: None,
            has_group_scope: true,
            is_infrastructure_scope: false,
            composite_state: None,
        };

        let surface = compute_session_verb_surface(&ctx);

        // FailOpen should have more verbs than safe-harbor
        let safe_count = surface
            .verbs
            .iter()
            .filter(|v| is_safe_harbor_verb(&v.fqn))
            .count();
        assert!(
            surface.verbs.len() >= safe_count,
            "FailOpen should not restrict to safe-harbor"
        );
    }

    #[test]
    fn test_si2_dual_fingerprints() {
        let envelope = make_envelope_with(&["session.info", "cbu.create"]);

        let ctx = VerbSurfaceContext {
            agent_mode: AgentMode::Governed,
            stage_focus: None,
            envelope: &envelope,
            fail_policy: VerbSurfaceFailPolicy::FailClosed,
            entity_state: None,
            has_group_scope: true,
            is_infrastructure_scope: false,
            composite_state: None,
        };

        let surface = compute_session_verb_surface(&ctx);

        // SI-2: surface_fingerprint uses "vs1:" prefix
        assert!(surface.surface_fingerprint.0.starts_with("vs1:"));

        // SI-2: semreg_fingerprint uses "v1:" prefix
        if let Some(ref fp) = surface.semreg_fingerprint {
            assert!(fp.0.starts_with("v1:"));
        }

        // The two fingerprints should be different (different hash inputs)
        if let Some(ref semreg_fp) = surface.semreg_fingerprint {
            assert_ne!(
                surface.surface_fingerprint.0, semreg_fp.0,
                "SI-2: surface and semreg fingerprints must differ"
            );
        }
    }

    #[test]
    fn test_si3_multi_reason_exclusion() {
        // A verb blocked by both AgentMode AND WorkflowPhase gets both reasons
        // This is hard to test without a real registry, but we can verify the
        // exclusion tracking logic
        let envelope = make_unavailable_envelope();

        let ctx = VerbSurfaceContext {
            agent_mode: AgentMode::Governed, // Blocks changeset.* verbs
            stage_focus: Some("semos-kyc"),  // Only allows kyc-related domains
            envelope: &envelope,
            fail_policy: VerbSurfaceFailPolicy::FailOpen, // Don't restrict further
            entity_state: None,
            has_group_scope: true,
            is_infrastructure_scope: false,
            composite_state: None,
        };

        let surface = compute_session_verb_surface(&ctx);

        // Check that excluded verbs can have multiple reasons
        // changeset.* verbs should be blocked by AgentMode (Governed blocks changeset)
        // AND possibly by workflow phase
        for excl in &surface.excluded {
            if excl.fqn.starts_with("changeset.") {
                assert!(
                    !excl.reasons.is_empty(),
                    "Excluded verb should have at least one reason"
                );
            }
        }
    }

    #[test]
    fn test_agent_mode_filter() {
        let envelope = make_unavailable_envelope();

        let research_ctx = VerbSurfaceContext {
            agent_mode: AgentMode::Research,
            stage_focus: None,
            envelope: &envelope,
            fail_policy: VerbSurfaceFailPolicy::FailOpen,
            entity_state: None,
            has_group_scope: true,
            is_infrastructure_scope: false,
            composite_state: None,
        };

        let governed_ctx = VerbSurfaceContext {
            agent_mode: AgentMode::Governed,
            stage_focus: None,
            envelope: &envelope,
            fail_policy: VerbSurfaceFailPolicy::FailOpen,
            entity_state: None,
            has_group_scope: true,
            is_infrastructure_scope: false,
            composite_state: None,
        };

        let research_surface = compute_session_verb_surface(&research_ctx);
        let governed_surface = compute_session_verb_surface(&governed_ctx);

        // Research should not contain governance.* verbs
        assert!(
            !research_surface
                .verbs
                .iter()
                .any(|v| v.fqn.starts_with("governance.")),
            "Research mode should block governance.* verbs"
        );

        // Governed should not contain changeset.* verbs
        assert!(
            !governed_surface
                .verbs
                .iter()
                .any(|v| v.fqn.starts_with("changeset.")),
            "Governed mode should block changeset.* verbs"
        );
    }

    #[test]
    fn test_workflow_phase_filter() {
        let envelope = make_unavailable_envelope();

        let kyc_ctx = VerbSurfaceContext {
            agent_mode: AgentMode::Governed,
            stage_focus: Some("semos-kyc"),
            envelope: &envelope,
            fail_policy: VerbSurfaceFailPolicy::FailOpen,
            entity_state: None,
            has_group_scope: true,
            is_infrastructure_scope: false,
            composite_state: None,
        };

        let surface = compute_session_verb_surface(&kyc_ctx);

        // KYC workflow should not contain deal.* or billing.* verbs
        assert!(
            !surface.verbs.iter().any(|v| v.domain == "deal"),
            "KYC workflow should filter out deal.* verbs"
        );
        assert!(
            !surface.verbs.iter().any(|v| v.domain == "billing"),
            "KYC workflow should filter out billing.* verbs"
        );
    }

    #[test]
    fn test_filter_summary_progressive_narrowing() {
        let envelope = make_unavailable_envelope();

        let ctx = VerbSurfaceContext {
            agent_mode: AgentMode::Governed,
            stage_focus: Some("semos-kyc"),
            envelope: &envelope,
            fail_policy: VerbSurfaceFailPolicy::FailClosed,
            entity_state: None,
            has_group_scope: true,
            is_infrastructure_scope: false,
            composite_state: None,
        };

        let surface = compute_session_verb_surface(&ctx);
        let s = &surface.filter_summary;

        // Each stage should be <= the previous (progressive narrowing)
        assert!(s.after_agent_mode <= s.total_registry);
        assert!(s.after_workflow <= s.after_agent_mode);
        assert!(s.after_group_scope == s.after_workflow);
        assert!(s.after_semreg <= s.after_workflow);
        assert!(s.after_lifecycle <= s.after_semreg);
        assert!(s.final_count <= s.after_lifecycle);
    }

    #[test]
    fn test_convenience_methods() {
        let envelope = make_unavailable_envelope();

        let ctx = VerbSurfaceContext {
            agent_mode: AgentMode::Governed,
            stage_focus: None,
            envelope: &envelope,
            fail_policy: VerbSurfaceFailPolicy::FailOpen,
            entity_state: None,
            has_group_scope: true,
            is_infrastructure_scope: false,
            composite_state: None,
        };

        let surface = compute_session_verb_surface(&ctx);

        // allowed_fqns should match verbs
        let fqns = surface.allowed_fqns();
        assert_eq!(fqns.len(), surface.verbs.len());

        // domains should be sorted
        let domains = surface.domains();
        let mut sorted = domains.clone();
        sorted.sort();
        assert_eq!(domains, sorted);

        // contains should work
        if let Some(first) = surface.verbs.first() {
            assert!(surface.contains(&first.fqn));
        }
        assert!(!surface.contains("nonexistent.verb"));
    }

    #[test]
    fn test_no_workflow_constraint_passes_all() {
        let envelope = make_unavailable_envelope();

        let no_focus_ctx = VerbSurfaceContext {
            agent_mode: AgentMode::Governed,
            stage_focus: None,
            envelope: &envelope,
            fail_policy: VerbSurfaceFailPolicy::FailOpen,
            entity_state: None,
            has_group_scope: true,
            is_infrastructure_scope: false,
            composite_state: None,
        };

        let with_focus_ctx = VerbSurfaceContext {
            agent_mode: AgentMode::Governed,
            stage_focus: Some("semos-kyc"),
            envelope: &envelope,
            fail_policy: VerbSurfaceFailPolicy::FailOpen,
            entity_state: None,
            has_group_scope: true,
            is_infrastructure_scope: false,
            composite_state: None,
        };

        let no_focus = compute_session_verb_surface(&no_focus_ctx);
        let with_focus = compute_session_verb_surface(&with_focus_ctx);

        // No workflow constraint should have more or equal verbs
        assert!(
            no_focus.verbs.len() >= with_focus.verbs.len(),
            "No workflow constraint should not restrict verbs"
        );
    }

    /// Phase-1 red→green: the composed CBU workspace's allowed set now includes
    /// the membership-owned macros, and `allowed_fqns()` emits them.
    ///
    /// RED (before this fix): `allowed_fqns()` returned atomic `RuntimeVerb`s
    /// only, so a `struct.*`/`structure.*` macro — though it matched at the macro
    /// tier — was dropped by the atomic-only `allowed_verbs` filter. A
    /// cross-workspace stewardship macro was (correctly) absent too, but for the
    /// wrong reason (everything was absent).
    ///
    /// GREEN (after): with `stage_focus = semos-onboarding` (→ workspace `cbu`),
    /// the CBU-owned macro is present in `allowed_fqns()` and the stewardship
    /// macro is still correctly absent — admission by membership, not by FQN.
    #[test]
    fn test_owned_macros_in_allowed_set() {
        let envelope = make_unavailable_envelope();

        let cbu_ctx = VerbSurfaceContext {
            agent_mode: AgentMode::Governed,
            stage_focus: Some("semos-onboarding"),
            envelope: &envelope,
            fail_policy: VerbSurfaceFailPolicy::FailOpen,
            entity_state: None,
            has_group_scope: true,
            is_infrastructure_scope: false,
            composite_state: None,
        };
        let surface = compute_session_verb_surface(&cbu_ctx);
        let allowed = surface.allowed_fqns();

        // GREEN: the CBU-owned onboarding/structure macro now survives.
        assert!(
            allowed.contains("structure.product-suite-full"),
            "CBU workspace allowed set must include its owned macro \
             structure.product-suite-full; owned_macros={}",
            surface.owned_macros.len()
        );
        // Still correctly dropped: stewardship macro is owned by a different
        // workspace, so it is NOT admitted into the CBU allowed set.
        assert!(
            !allowed.contains("governance.bootstrap-attribute-registry"),
            "cross-workspace stewardship macro must NOT leak into the CBU allowed set"
        );

        // No stage_focus → no workspace → no owned macros (atomic-only set).
        let no_focus_ctx = VerbSurfaceContext {
            stage_focus: None,
            ..cbu_ctx
        };
        let no_focus = compute_session_verb_surface(&no_focus_ctx);
        assert!(
            no_focus.owned_macros.is_empty(),
            "no workspace resolved ⇒ no owned macros"
        );
    }

    #[test]
    fn test_semreg_filters_verbs() {
        // Create envelope with only 2 allowed verbs
        let envelope = make_envelope_with(&["session.info", "session.list"]);

        let ctx = VerbSurfaceContext {
            agent_mode: AgentMode::Governed,
            stage_focus: None,
            envelope: &envelope,
            fail_policy: VerbSurfaceFailPolicy::FailClosed,
            entity_state: None,
            has_group_scope: true,
            is_infrastructure_scope: false,
            composite_state: None,
        };

        let surface = compute_session_verb_surface(&ctx);
        let phase2 = Phase2Service::evaluate_from_envelope(envelope.clone());

        // Should only contain the 2 allowed verbs (if they exist in registry)
        for v in &surface.verbs {
            assert!(
                phase2.allows_verb(&v.fqn),
                "Verb '{}' should be in SemReg allowed set",
                v.fqn
            );
        }
    }

    /// Phase 3 C2: lifecycle is a TAG, not a discovery prune. A verb whose
    /// `requires_states` the current entity state does not satisfy must remain
    /// a classification candidate (invariant M2) — but be flagged
    /// lifecycle-ineligible so the fast-path execution gate refuses it.
    ///
    /// RED before C2: Step 5 pruned `cbu.confirm` (requires VALIDATION_PENDING)
    /// at `entity_state = DISCOVERED`, so `contains("cbu.confirm")` was false.
    /// GREEN after C2: it is present, with `is_lifecycle_eligible == false`,
    /// while an in-state verb (`submit-for-validation`, requires DISCOVERED)
    /// stays eligible.
    #[test]
    fn test_c2_lifecycle_tags_but_does_not_prune() {
        let envelope = make_unavailable_envelope();
        let ctx = VerbSurfaceContext {
            agent_mode: AgentMode::Governed,
            stage_focus: Some("semos-onboarding"),
            envelope: &envelope,
            fail_policy: VerbSurfaceFailPolicy::FailOpen,
            entity_state: Some("DISCOVERED"),
            has_group_scope: true,
            is_infrastructure_scope: false,
            composite_state: None,
        };
        let surface = compute_session_verb_surface(&ctx);

        // M2: the state-ineligible verb is NOT pruned from discovery.
        assert!(
            surface.contains("cbu.confirm"),
            "C2: cbu.confirm (requires VALIDATION_PENDING) must remain a \
             classification candidate at DISCOVERED — discovery must not prune \
             on lifecycle"
        );
        // …but it is tagged ineligible for the fast-path execution gate.
        assert!(
            !surface.is_lifecycle_eligible("cbu.confirm"),
            "C2: cbu.confirm must be tagged lifecycle-ineligible at DISCOVERED"
        );
        // An in-state verb stays eligible.
        assert!(
            surface.is_lifecycle_eligible("cbu.submit-for-validation"),
            "C2: submit-for-validation (requires DISCOVERED) must be eligible at DISCOVERED"
        );
    }
}
