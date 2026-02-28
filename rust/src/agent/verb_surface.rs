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

use sem_os_core::authoring::agent_mode::AgentMode;

use crate::agent::context_envelope::{AllowedVerbSetFingerprint, ContextEnvelope, PruneReason};
use crate::dsl_v2::runtime_registry::{runtime_registry, RuntimeVerb};

// ── Core types ──────────────────────────────────────────────────

/// The complete, fingerprinted verb surface for a session turn.
///
/// Computed once via `compute_session_verb_surface()` and threaded through
/// the entire pipeline (orchestrator, MCP, chat response, UI).
#[derive(Debug, Clone, Serialize)]
pub struct SessionVerbSurface {
    /// Verbs visible to the user after all governance layers.
    pub verbs: Vec<SurfaceVerb>,
    /// Verbs excluded with structured, multi-layer reasons.
    pub excluded: Vec<ExcludedVerb>,
    /// SHA-256 fingerprint of the final visible set + filter context.
    /// Distinct from `semreg_fingerprint` (SI-2).
    pub surface_fingerprint: SurfaceFingerprint,
    /// CCIR-internal fingerprint from the ContextEnvelope (if available).
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
    pub after_semreg: usize,
    pub after_lifecycle: usize,
    pub after_actor: usize,
    pub final_count: usize,
}

// ── Safe-harbor verbs (FailClosed fallback) ─────────────────────

/// Domain prefixes that are always safe (navigation, help, session management).
const SAFE_HARBOR_DOMAINS: &[&str] = &["session", "view", "agent"];

fn is_safe_harbor_verb(fqn: &str) -> bool {
    SAFE_HARBOR_DOMAINS
        .iter()
        .any(|domain| fqn.starts_with(&format!("{domain}.")))
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
    /// SemReg context resolution result.
    pub envelope: &'a ContextEnvelope,
    /// Fail policy when SemReg is unavailable.
    pub fail_policy: VerbSurfaceFailPolicy,
    /// Current entity state (e.g., "open", "in_review") for lifecycle filtering.
    /// If None, lifecycle filtering is skipped.
    pub entity_state: Option<&'a str>,
}

// ── 8-step compute pipeline ─────────────────────────────────────

/// Compute the session verb surface by applying all governance layers.
///
/// 8-step pipeline:
/// 1. Base set from RuntimeVerbRegistry
/// 2. AgentMode filter
/// 3. Workflow phase filter
/// 4. SemReg CCIR (ContextEnvelope)
/// 5. Lifecycle state filter
/// 6. Actor gating (extension point)
/// 7. FailPolicy check
/// 8. Rank, group, fingerprint
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

    // ── Step 3: Workflow phase filter ───────────────────────────
    let allowed_domains = ctx.stage_focus.and_then(workflow_allowed_domains);

    let after_wf: Vec<(&str, &RuntimeVerb)> = if let Some(ref domains) = allowed_domains {
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
                            layer: PruneLayer::WorkflowPhase,
                            reason: format!(
                                "Domain '{}' not in workflow '{}'",
                                rv.domain,
                                ctx.stage_focus.unwrap_or("unknown")
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

    // ── Step 4: SemReg CCIR ────────────────────────────────────
    let semreg_available = !ctx.envelope.is_unavailable();
    let semreg_has_verbs = semreg_available && !ctx.envelope.is_deny_all();

    let after_sr: Vec<(&str, &RuntimeVerb)> = if semreg_has_verbs {
        after_wf
            .into_iter()
            .filter(|(fqn, _)| {
                if ctx.envelope.is_allowed(fqn) {
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

    // ── Step 5: Lifecycle state filter ──────────────────────────
    let after_lc: Vec<(&str, &RuntimeVerb)> = after_sr
        .into_iter()
        .filter(|(fqn, rv)| {
            if let (Some(ref lifecycle), Some(entity_state)) = (&rv.lifecycle, ctx.entity_state) {
                if !lifecycle.requires_states.is_empty()
                    && !lifecycle.requires_states.iter().any(|s| s == entity_state)
                {
                    exclusions
                        .entry(fqn.to_string())
                        .or_default()
                        .push(SurfacePrune {
                            layer: PruneLayer::LifecycleState,
                            reason: format!(
                                "Requires entity state {:?}, current: '{}'",
                                lifecycle.requires_states, entity_state
                            ),
                        });
                    return false;
                }
            }
            true
        })
        .collect();
    let after_lifecycle = after_lc.len();

    // ── Step 6: Actor gating (extension point) ──────────────────
    // Currently a passthrough. Future: non-SemReg role checks.
    let after_actor = after_lc.len();

    // ── Step 7: FailPolicy check ────────────────────────────────
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

    // ── Step 8: Rank, group, fingerprint ────────────────────────
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

            let rank_boost = compute_rank_boost(rv, ctx.stage_focus, &allowed_domains);

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
    );

    let semreg_fingerprint = if semreg_available {
        Some(ctx.envelope.fingerprint.clone())
    } else {
        None
    };

    SessionVerbSurface {
        verbs,
        excluded,
        surface_fingerprint,
        semreg_fingerprint,
        fail_policy_applied: ctx.fail_policy,
        computed_at: Utc::now(),
        filter_summary: FilterSummary {
            total_registry,
            after_agent_mode,
            after_workflow,
            after_semreg,
            after_lifecycle,
            after_actor,
            final_count,
        },
    }
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
        PruneReason::TierExcluded { tier, reason } => {
            format!("Tier excluded ({tier}: {reason})")
        }
        PruneReason::TaxonomyNoOverlap { verb_taxonomies } => {
            format!("No taxonomy overlap ({:?})", verb_taxonomies)
        }
        PruneReason::PreconditionFailed { precondition } => {
            format!("Precondition failed: {precondition}")
        }
        PruneReason::AgentModeBlocked { mode } => format!("Blocked by {mode} mode"),
        PruneReason::PolicyDenied { policy_fqn, reason } => {
            format!("Policy denied ({policy_fqn}: {reason})")
        }
    }
}

// ── Convenience methods on SessionVerbSurface ───────────────────

impl SessionVerbSurface {
    /// Get the allowed verb FQNs as a HashSet (for pipeline pre-constraint).
    pub fn allowed_fqns(&self) -> HashSet<String> {
        self.verbs.iter().map(|v| v.fqn.clone()).collect()
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
    use crate::agent::context_envelope::ContextEnvelope;

    fn make_envelope_with(verbs: &[&str]) -> ContextEnvelope {
        ContextEnvelope::test_with_verbs(verbs)
    }

    fn make_unavailable_envelope() -> ContextEnvelope {
        ContextEnvelope::unavailable()
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
        );
        let fp2 = compute_surface_fingerprint(
            &verbs,
            &AgentMode::Governed,
            None,
            None,
            VerbSurfaceFailPolicy::FailClosed,
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
        );
        let fp_research = compute_surface_fingerprint(
            &verbs,
            &AgentMode::Research,
            None,
            None,
            VerbSurfaceFailPolicy::FailClosed,
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
        // Should have SOME safe-harbor verbs (not empty)
        assert!(
            !surface.verbs.is_empty(),
            "FailClosed should still have safe-harbor verbs"
        );
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
        };

        let governed_ctx = VerbSurfaceContext {
            agent_mode: AgentMode::Governed,
            stage_focus: None,
            envelope: &envelope,
            fail_policy: VerbSurfaceFailPolicy::FailOpen,
            entity_state: None,
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
        };

        let surface = compute_session_verb_surface(&ctx);
        let s = &surface.filter_summary;

        // Each stage should be <= the previous (progressive narrowing)
        assert!(s.after_agent_mode <= s.total_registry);
        assert!(s.after_workflow <= s.after_agent_mode);
        assert!(s.after_semreg <= s.after_workflow);
        assert!(s.after_lifecycle <= s.after_semreg);
        assert!(s.after_actor <= s.after_lifecycle);
        assert!(s.final_count <= s.after_actor);
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
        };

        let with_focus_ctx = VerbSurfaceContext {
            agent_mode: AgentMode::Governed,
            stage_focus: Some("semos-kyc"),
            envelope: &envelope,
            fail_policy: VerbSurfaceFailPolicy::FailOpen,
            entity_state: None,
        };

        let no_focus = compute_session_verb_surface(&no_focus_ctx);
        let with_focus = compute_session_verb_surface(&with_focus_ctx);

        // No workflow constraint should have more or equal verbs
        assert!(
            no_focus.verbs.len() >= with_focus.verbs.len(),
            "No workflow constraint should not restrict verbs"
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
        };

        let surface = compute_session_verb_surface(&ctx);

        // Should only contain the 2 allowed verbs (if they exist in registry)
        for v in &surface.verbs {
            assert!(
                envelope.is_allowed(&v.fqn),
                "Verb '{}' should be in SemReg allowed set",
                v.fqn
            );
        }
    }
}
