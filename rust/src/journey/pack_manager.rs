//! Pack Manager — lifecycle manager and constraint projection.
//!
//! The `PackManager` owns the lifecycle state of all packs in a session
//! and projects their constraints into `EffectiveConstraints`.
//!
//! ## Key Design Decisions
//!
//! 1. **Event-driven state**: The pack manager processes events from
//!    verb execution to advance pack progress and detect completion.
//!
//! 2. **Constraint composition by intersection**: When multiple packs
//!    are active, the effective `allowed_verbs` is the intersection.
//!    An empty intersection → `ConstraintViolation`.
//!
//! 3. **Never called during verb discovery**: The pack manager is only
//!    consulted AFTER macro expansion, during the constraint gate phase.
//!    It does not participate in verb search or arg extraction.
//!
//! 4. **Completion widening**: When a pack transitions to `Completed`,
//!    its constraints are removed from the effective set, widening
//!    what's available for the NEXT `process_utterance` call.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::pack::PackManifest;
use super::pack_state::{PackState, PackTransitionError, SuspendReason};

// ---------------------------------------------------------------------------
// PackManager
// ---------------------------------------------------------------------------

/// Manages the lifecycle of all packs within a session.
///
/// Each pack is tracked by its ID. The manager provides:
/// - State transitions (activate, suspend, resume, complete)
/// - Event processing (verb execution → progress tracking)
/// - Constraint projection (effective constraints from active packs)
#[derive(Debug, Clone)]
pub struct PackManager {
    /// Pack definitions indexed by pack ID.
    manifests: HashMap<String, PackManifest>,

    /// Current lifecycle state of each pack.
    states: HashMap<String, PackState>,
}

impl PackManager {
    /// Create a new empty pack manager.
    pub fn new() -> Self {
        Self {
            manifests: HashMap::new(),
            states: HashMap::new(),
        }
    }

    /// Register a pack (starts in `Dormant` state).
    pub fn register_pack(&mut self, manifest: PackManifest) {
        let id = manifest.id.clone();
        self.manifests.insert(id.clone(), manifest);
        self.states.insert(id, PackState::dormant());
    }

    /// Activate a pack (Dormant → Active).
    pub fn activate_pack(&mut self, pack_id: &str) -> Result<(), PackManagerError> {
        let state = self
            .states
            .get(pack_id)
            .ok_or_else(|| PackManagerError::UnknownPack(pack_id.to_string()))?;

        let new_state = state
            .activate()
            .map_err(PackManagerError::TransitionError)?;
        self.states.insert(pack_id.to_string(), new_state);
        Ok(())
    }

    /// Suspend an active pack.
    pub fn suspend_pack(
        &mut self,
        pack_id: &str,
        reason: SuspendReason,
    ) -> Result<(), PackManagerError> {
        let state = self
            .states
            .get(pack_id)
            .ok_or_else(|| PackManagerError::UnknownPack(pack_id.to_string()))?;

        let new_state = state
            .suspend(reason)
            .map_err(PackManagerError::TransitionError)?;
        self.states.insert(pack_id.to_string(), new_state);
        Ok(())
    }

    /// Resume a suspended pack.
    pub fn resume_pack(&mut self, pack_id: &str) -> Result<(), PackManagerError> {
        let state = self
            .states
            .get(pack_id)
            .ok_or_else(|| PackManagerError::UnknownPack(pack_id.to_string()))?;

        let new_state = state.resume().map_err(PackManagerError::TransitionError)?;
        self.states.insert(pack_id.to_string(), new_state);
        Ok(())
    }

    /// Complete an active pack (Active → Completed).
    ///
    /// After completion, the pack's constraints are removed from the
    /// effective set, **widening** what is available for the next
    /// `process_utterance` call (INV-1a: does not affect currently
    /// executing runbook).
    pub fn complete_pack(&mut self, pack_id: &str) -> Result<(), PackManagerError> {
        let state = self
            .states
            .get(pack_id)
            .ok_or_else(|| PackManagerError::UnknownPack(pack_id.to_string()))?;

        let new_state = state
            .complete()
            .map_err(PackManagerError::TransitionError)?;
        self.states.insert(pack_id.to_string(), new_state);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Event Processing
    // -----------------------------------------------------------------------

    /// Process a verb execution event.
    ///
    /// Updates progress on all active packs and checks if any should
    /// be transitioned to `Completed`.
    pub fn process_event(&mut self, event: &PackEvent) {
        match event {
            PackEvent::VerbExecuted { verb, .. } => {
                // Update progress on active packs that include this verb
                let active_ids: Vec<String> = self
                    .states
                    .iter()
                    .filter(|(_, s)| s.is_active())
                    .map(|(id, _)| id.clone())
                    .collect();

                for pack_id in active_ids {
                    if let Some(state) = self.states.get_mut(&pack_id) {
                        if let Some(progress) = state.progress_mut() {
                            progress.record_verb_execution(verb);
                        }
                    }
                }
            }
            PackEvent::SignalEmitted { signal, .. } => {
                // Record the signal on all active packs
                let active_ids: Vec<String> = self
                    .states
                    .iter()
                    .filter(|(_, s)| s.is_active())
                    .map(|(id, _)| id.clone())
                    .collect();

                for pack_id in active_ids {
                    if let Some(state) = self.states.get_mut(&pack_id) {
                        if let Some(progress) = state.progress_mut() {
                            progress.emit_signal(signal);
                        }
                    }
                }
            }
        }
    }

    /// Check if a pack's stop conditions are met and transition to Completed.
    ///
    /// Returns `true` if the pack was completed.
    pub fn check_and_complete(&mut self, pack_id: &str) -> Result<bool, PackManagerError> {
        let manifest = self
            .manifests
            .get(pack_id)
            .ok_or_else(|| PackManagerError::UnknownPack(pack_id.to_string()))?;

        let state = self
            .states
            .get(pack_id)
            .ok_or_else(|| PackManagerError::UnknownPack(pack_id.to_string()))?;

        if !state.is_active() {
            return Ok(false);
        }

        // Check if all progress signals have been emitted
        let progress = state.progress().unwrap();
        let all_signals_met = manifest
            .progress_signals
            .iter()
            .all(|ps| progress.signals_emitted.contains(&ps.signal));

        if all_signals_met && !manifest.progress_signals.is_empty() {
            self.complete_pack(pack_id)?;
            return Ok(true);
        }

        Ok(false)
    }

    // -----------------------------------------------------------------------
    // Constraint Projection
    // -----------------------------------------------------------------------

    /// Compute effective constraints from all active packs.
    ///
    /// ## Composition Rules
    ///
    /// - **allowed_verbs**: Intersection of all active packs' allowed sets.
    ///   If any pack has an empty allowed set, it means "no restriction from
    ///   this pack" (unconstrained).
    /// - **forbidden_verbs**: Union of all active packs' forbidden sets.
    ///
    /// An empty `allowed_verbs` in the result with active packs that all
    /// have non-empty allowed sets means the intersection is empty →
    /// `ConstraintViolation`.
    pub fn effective_constraints(&self) -> EffectiveConstraints {
        let active_packs: Vec<&str> = self
            .states
            .iter()
            .filter(|(_, s)| s.is_active())
            .map(|(id, _)| id.as_str())
            .collect();

        if active_packs.is_empty() {
            return EffectiveConstraints::unconstrained();
        }

        // Collect allowed and forbidden sets per active pack
        let mut constrained_allowed: Option<HashSet<String>> = None;
        let mut all_forbidden: HashSet<String> = HashSet::new();
        let mut contributing_packs: Vec<ConstraintSource> = Vec::new();

        for pack_id in &active_packs {
            if let Some(manifest) = self.manifests.get(*pack_id) {
                // Forbidden: union across all active packs
                for verb in &manifest.forbidden_verbs {
                    all_forbidden.insert(verb.clone());
                }

                // Allowed: intersection across active packs
                // Empty allowed set = "no restriction from this pack"
                if !manifest.allowed_verbs.is_empty() {
                    let pack_allowed: HashSet<String> =
                        manifest.allowed_verbs.iter().cloned().collect();

                    constrained_allowed = Some(match constrained_allowed {
                        Some(existing) => existing.intersection(&pack_allowed).cloned().collect(),
                        None => pack_allowed,
                    });
                }

                contributing_packs.push(ConstraintSource {
                    pack_id: pack_id.to_string(),
                    pack_name: manifest.name.clone(),
                    allowed_count: manifest.allowed_verbs.len(),
                    forbidden_count: manifest.forbidden_verbs.len(),
                });
            }
        }

        EffectiveConstraints {
            allowed_verbs: constrained_allowed,
            forbidden_verbs: all_forbidden,
            contributing_packs,
        }
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Get the state of a specific pack.
    pub fn pack_state(&self, pack_id: &str) -> Option<&PackState> {
        self.states.get(pack_id)
    }

    /// Get the manifest of a specific pack.
    pub fn pack_manifest(&self, pack_id: &str) -> Option<&PackManifest> {
        self.manifests.get(pack_id)
    }

    /// List all active pack IDs.
    pub fn active_packs(&self) -> Vec<&str> {
        self.states
            .iter()
            .filter(|(_, s)| s.is_active())
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// List all registered pack IDs.
    pub fn all_packs(&self) -> Vec<&str> {
        self.states.keys().map(|id| id.as_str()).collect()
    }
}

impl Default for PackManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// EffectiveConstraints
// ---------------------------------------------------------------------------

/// The combined constraints from all active packs.
///
/// Used by `PackConstraintGate` to check expanded verb lists.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectiveConstraints {
    /// Allowed verbs (intersection of active packs).
    /// `None` means unconstrained — all verbs are allowed.
    /// `Some(set)` means only verbs in the set are allowed.
    pub allowed_verbs: Option<HashSet<String>>,

    /// Forbidden verbs (union of active packs).
    /// Always enforced — these verbs are never allowed.
    pub forbidden_verbs: HashSet<String>,

    /// Which packs contributed to these constraints.
    pub contributing_packs: Vec<ConstraintSource>,
}

impl EffectiveConstraints {
    /// Create unconstrained (no active packs).
    pub fn unconstrained() -> Self {
        Self {
            allowed_verbs: None,
            forbidden_verbs: HashSet::new(),
            contributing_packs: vec![],
        }
    }

    /// Check if a verb is allowed under these constraints.
    pub fn is_verb_allowed(&self, verb: &str) -> bool {
        // Forbidden always wins
        if self.forbidden_verbs.contains(verb) {
            return false;
        }

        // If no allowed set, everything not forbidden is allowed
        match &self.allowed_verbs {
            None => true,
            Some(allowed) => allowed.contains(verb),
        }
    }

    /// Check a list of verbs and return violations.
    pub fn check_verbs(&self, verbs: &[String]) -> Vec<ConstraintViolation> {
        let mut violations = Vec::new();

        for verb in verbs {
            if self.forbidden_verbs.contains(verb) {
                // Find which pack(s) forbid this verb
                violations.push(ConstraintViolation {
                    verb: verb.clone(),
                    reason: ViolationReason::Forbidden,
                });
            } else if let Some(allowed) = &self.allowed_verbs {
                if !allowed.contains(verb) {
                    violations.push(ConstraintViolation {
                        verb: verb.clone(),
                        reason: ViolationReason::NotInAllowedSet,
                    });
                }
            }
        }

        violations
    }

    /// Whether any constraints are active.
    pub fn is_constrained(&self) -> bool {
        self.allowed_verbs.is_some() || !self.forbidden_verbs.is_empty()
    }

    /// Whether the allowed set intersection is empty (deadlock).
    pub fn is_empty_intersection(&self) -> bool {
        matches!(&self.allowed_verbs, Some(set) if set.is_empty())
    }
}

/// Which pack contributed what constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintSource {
    pub pack_id: String,
    pub pack_name: String,
    pub allowed_count: usize,
    pub forbidden_count: usize,
}

/// A single constraint violation on a verb.
#[derive(Debug, Clone)]
pub struct ConstraintViolation {
    pub verb: String,
    pub reason: ViolationReason,
}

/// Why a verb violates constraints.
#[derive(Debug, Clone, PartialEq)]
pub enum ViolationReason {
    /// Verb is in a forbidden set.
    Forbidden,
    /// Verb is not in the allowed set (intersection of active packs).
    NotInAllowedSet,
}

// ---------------------------------------------------------------------------
// PackEvent
// ---------------------------------------------------------------------------

/// Events processed by the PackManager to advance pack progress.
#[derive(Debug, Clone)]
pub enum PackEvent {
    /// A verb was successfully executed.
    VerbExecuted {
        /// The verb that was executed.
        verb: String,
        /// Session this occurred in.
        session_id: uuid::Uuid,
    },
    /// A progress signal was emitted.
    SignalEmitted {
        /// The signal name (matches PackManifest.progress_signals[].signal).
        signal: String,
        /// Session this occurred in.
        session_id: uuid::Uuid,
    },
}

// ---------------------------------------------------------------------------
// PackManagerError
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, thiserror::Error)]
pub enum PackManagerError {
    #[error("Unknown pack: {0}")]
    UnknownPack(String),

    #[error("{0}")]
    TransitionError(#[from] PackTransitionError),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manifest(id: &str, allowed: &[&str], forbidden: &[&str]) -> PackManifest {
        PackManifest {
            id: id.to_string(),
            name: format!("Test Pack {}", id),
            version: "1.0".to_string(),
            description: "Test pack".to_string(),
            invocation_phrases: vec![],
            required_context: vec![],
            optional_context: vec![],
            allowed_verbs: allowed.iter().map(|s| s.to_string()).collect(),
            forbidden_verbs: forbidden.iter().map(|s| s.to_string()).collect(),
            risk_policy: Default::default(),
            required_questions: vec![],
            optional_questions: vec![],
            stop_rules: vec![],
            templates: vec![],
            pack_summary_template: None,
            section_layout: vec![],
            definition_of_done: vec![],
            progress_signals: vec![],
            handoff_target: None,
        }
    }

    fn test_manifest_with_signals(id: &str, allowed: &[&str], signals: &[&str]) -> PackManifest {
        let mut m = test_manifest(id, allowed, &[]);
        m.progress_signals = signals
            .iter()
            .map(|s| super::super::pack::ProgressSignal {
                signal: s.to_string(),
                description: format!("Signal: {}", s),
            })
            .collect();
        m
    }

    #[test]
    fn test_register_and_activate() {
        let mut mgr = PackManager::new();
        mgr.register_pack(test_manifest("p1", &["cbu.create"], &[]));

        assert!(mgr.pack_state("p1").unwrap().is_dormant());
        mgr.activate_pack("p1").unwrap();
        assert!(mgr.pack_state("p1").unwrap().is_active());
    }

    #[test]
    fn test_suspend_and_resume() {
        let mut mgr = PackManager::new();
        mgr.register_pack(test_manifest("p1", &["cbu.create"], &[]));
        mgr.activate_pack("p1").unwrap();
        mgr.suspend_pack("p1", SuspendReason::UserPaused).unwrap();
        assert!(!mgr.pack_state("p1").unwrap().is_active());
        mgr.resume_pack("p1").unwrap();
        assert!(mgr.pack_state("p1").unwrap().is_active());
    }

    #[test]
    fn test_unknown_pack_error() {
        let mut mgr = PackManager::new();
        assert!(mgr.activate_pack("nonexistent").is_err());
    }

    // -- Constraint tests --

    #[test]
    fn test_no_active_packs_unconstrained() {
        let mgr = PackManager::new();
        let constraints = mgr.effective_constraints();
        assert!(!constraints.is_constrained());
        assert!(constraints.is_verb_allowed("anything.goes"));
    }

    #[test]
    fn test_single_active_pack_constraints() {
        let mut mgr = PackManager::new();
        mgr.register_pack(test_manifest(
            "kyc",
            &["kyc.create-case", "kyc.submit-case"],
            &["cbu.delete"],
        ));
        mgr.activate_pack("kyc").unwrap();

        let constraints = mgr.effective_constraints();
        assert!(constraints.is_constrained());
        assert!(constraints.is_verb_allowed("kyc.create-case"));
        assert!(constraints.is_verb_allowed("kyc.submit-case"));
        assert!(!constraints.is_verb_allowed("cbu.delete")); // forbidden
        assert!(!constraints.is_verb_allowed("entity.create")); // not in allowed set
    }

    #[test]
    fn test_multiple_active_packs_intersection() {
        let mut mgr = PackManager::new();
        mgr.register_pack(test_manifest(
            "p1",
            &["cbu.create", "entity.create", "cbu.assign-role"],
            &[],
        ));
        mgr.register_pack(test_manifest(
            "p2",
            &["cbu.create", "kyc.create-case", "cbu.assign-role"],
            &["entity.delete"],
        ));
        mgr.activate_pack("p1").unwrap();
        mgr.activate_pack("p2").unwrap();

        let constraints = mgr.effective_constraints();
        // Intersection: cbu.create, cbu.assign-role
        assert!(constraints.is_verb_allowed("cbu.create"));
        assert!(constraints.is_verb_allowed("cbu.assign-role"));
        assert!(!constraints.is_verb_allowed("entity.create")); // only in p1
        assert!(!constraints.is_verb_allowed("kyc.create-case")); // only in p2
        assert!(!constraints.is_verb_allowed("entity.delete")); // forbidden by p2
    }

    #[test]
    fn test_empty_intersection_detected() {
        let mut mgr = PackManager::new();
        mgr.register_pack(test_manifest("p1", &["cbu.create"], &[]));
        mgr.register_pack(test_manifest("p2", &["kyc.create-case"], &[]));
        mgr.activate_pack("p1").unwrap();
        mgr.activate_pack("p2").unwrap();

        let constraints = mgr.effective_constraints();
        // No overlap → empty intersection
        assert!(constraints.is_empty_intersection());
    }

    #[test]
    fn test_unconstrained_pack_doesnt_restrict() {
        let mut mgr = PackManager::new();
        // p1 has no allowed_verbs → unconstrained
        mgr.register_pack(test_manifest("p1", &[], &[]));
        mgr.register_pack(test_manifest("p2", &["cbu.create"], &[]));
        mgr.activate_pack("p1").unwrap();
        mgr.activate_pack("p2").unwrap();

        let constraints = mgr.effective_constraints();
        // p1 has no restrictions, so p2's allowed set is the result
        assert!(constraints.is_verb_allowed("cbu.create"));
        assert!(!constraints.is_verb_allowed("entity.create"));
    }

    #[test]
    fn test_forbidden_union() {
        let mut mgr = PackManager::new();
        mgr.register_pack(test_manifest("p1", &[], &["cbu.delete"]));
        mgr.register_pack(test_manifest("p2", &[], &["entity.delete"]));
        mgr.activate_pack("p1").unwrap();
        mgr.activate_pack("p2").unwrap();

        let constraints = mgr.effective_constraints();
        assert!(!constraints.is_verb_allowed("cbu.delete"));
        assert!(!constraints.is_verb_allowed("entity.delete"));
        assert!(constraints.is_verb_allowed("cbu.create")); // not forbidden, no allowed filter
    }

    #[test]
    fn test_check_verbs() {
        let mut mgr = PackManager::new();
        mgr.register_pack(test_manifest("kyc", &["kyc.create-case"], &["cbu.delete"]));
        mgr.activate_pack("kyc").unwrap();

        let constraints = mgr.effective_constraints();
        let violations = constraints.check_verbs(&[
            "kyc.create-case".to_string(),
            "cbu.delete".to_string(),
            "entity.create".to_string(),
        ]);

        assert_eq!(violations.len(), 2);
        assert_eq!(violations[0].verb, "cbu.delete");
        assert_eq!(violations[0].reason, ViolationReason::Forbidden);
        assert_eq!(violations[1].verb, "entity.create");
        assert_eq!(violations[1].reason, ViolationReason::NotInAllowedSet);
    }

    // -- Event processing --

    #[test]
    fn test_process_verb_event() {
        let mut mgr = PackManager::new();
        mgr.register_pack(test_manifest("kyc", &["kyc.create-case"], &[]));
        mgr.activate_pack("kyc").unwrap();

        mgr.process_event(&PackEvent::VerbExecuted {
            verb: "kyc.create-case".to_string(),
            session_id: uuid::Uuid::new_v4(),
        });

        let progress = mgr.pack_state("kyc").unwrap().progress().unwrap();
        assert_eq!(progress.steps_completed, 1);
        assert_eq!(progress.executed_verbs, vec!["kyc.create-case"]);
    }

    #[test]
    fn test_process_signal_event() {
        let mut mgr = PackManager::new();
        mgr.register_pack(test_manifest("kyc", &[], &[]));
        mgr.activate_pack("kyc").unwrap();

        mgr.process_event(&PackEvent::SignalEmitted {
            signal: "case_opened".to_string(),
            session_id: uuid::Uuid::new_v4(),
        });

        let progress = mgr.pack_state("kyc").unwrap().progress().unwrap();
        assert!(progress
            .signals_emitted
            .contains(&"case_opened".to_string()));
    }

    // -- Completion widening --

    #[test]
    fn test_completion_widens_constraints() {
        let mut mgr = PackManager::new();
        mgr.register_pack(test_manifest("p1", &["cbu.create", "entity.create"], &[]));
        mgr.register_pack(test_manifest("p2", &["cbu.create", "kyc.create-case"], &[]));
        mgr.activate_pack("p1").unwrap();
        mgr.activate_pack("p2").unwrap();

        // Before completion: intersection is {cbu.create}
        let constraints = mgr.effective_constraints();
        assert!(!constraints.is_verb_allowed("entity.create"));

        // Complete p1 — only p2 active now
        mgr.complete_pack("p1").unwrap();

        // After completion: only p2's constraints apply
        let constraints = mgr.effective_constraints();
        assert!(constraints.is_verb_allowed("cbu.create"));
        assert!(constraints.is_verb_allowed("kyc.create-case"));
        assert!(!constraints.is_verb_allowed("entity.create")); // still not in p2
    }

    #[test]
    fn test_check_and_complete_with_signals() {
        let mut mgr = PackManager::new();
        mgr.register_pack(test_manifest_with_signals(
            "bootstrap",
            &["session.load-galaxy"],
            &["scope_set"],
        ));
        mgr.activate_pack("bootstrap").unwrap();

        // Not complete yet — signal not emitted
        assert!(!mgr.check_and_complete("bootstrap").unwrap());

        // Emit the signal
        mgr.process_event(&PackEvent::SignalEmitted {
            signal: "scope_set".to_string(),
            session_id: uuid::Uuid::new_v4(),
        });

        // Now all signals met → complete
        assert!(mgr.check_and_complete("bootstrap").unwrap());
        assert!(mgr.pack_state("bootstrap").unwrap().is_terminal());
    }

    #[test]
    fn test_active_packs_list() {
        let mut mgr = PackManager::new();
        mgr.register_pack(test_manifest("p1", &[], &[]));
        mgr.register_pack(test_manifest("p2", &[], &[]));
        mgr.activate_pack("p1").unwrap();

        let active = mgr.active_packs();
        assert_eq!(active.len(), 1);
        assert!(active.contains(&"p1"));
    }
}
