//! Coordination strategy table (v0.5 §5.3, §5.4, §4.4).
//!
//! Maps `EffectClass` → `ConcurrencyPolicy`. The runtime applies the cheapest
//! safe coordination strategy per plan based on its steps' declared effect classes.
//!
//! # The architectural rule (v0.5 §5.5)
//!
//! **No verb may acquire locks directly. The runtime coordination layer
//! owns all lock acquisition, idempotency checks, optimistic guards,
//! conflict handling, and lock timeout behaviour.**
//!
//! # Six-level hierarchy (cheapest to most expensive)
//!
//! 1. `None` — no coordination (pure/read-only)
//! 2. `IdempotencyGuard` — hash-of-inputs duplicate detection
//! 3. `UniqueInsert` — DB unique constraint (natural-key ensure)
//! 4. `OptimisticSnapshotCheck` — CAS on expected predecessor (T13)
//! 5. `PessimisticResourceLock` — advisory lock on UUID, transaction-scoped
//! 6. `ExclusiveScopeLock` — scope-level lock (admin, migration)

use dsl_core::EffectClass;

// =============================================================================
// ConcurrencyPolicy (v0.5 §5.3)
// =============================================================================

/// The coordination strategy to apply for a verb or plan (v0.5 §5.3).
///
/// Selected by the runtime from the verb's declared `effect_class`.
/// Verbs do NOT declare a policy directly — the runtime derives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConcurrencyPolicy {
    /// No coordination needed. For `pure` and `read_snapshot` verbs.
    /// The runtime acquires no locks, performs no checks.
    None,

    /// Hash inputs; return prior result if identical submission seen.
    /// For `idempotent_ensure` and `append_fact` verbs (per-plan).
    /// DB-backed; checked before plan execution.
    IdempotencyGuard,

    /// Rely on DB unique constraint at insert time.
    /// For `idempotent_ensure` verbs (upsert pattern with conflict_keys).
    /// Concurrent inserts are safely serialised by the constraint.
    UniqueInsert,

    /// Optimistic CAS on expected predecessor version.
    /// For `append_transition_snapshot` verbs.
    /// No pre-lock; conflict detected at insert time → `OptimisticConflict`.
    /// Phase 5: stub — wired to PessimisticResourceLock as fallback (T13).
    OptimisticSnapshotCheck,

    /// Postgres advisory lock on entity UUID, transaction-scoped.
    /// For `read_modify_write` and `cross_resource_invariant` verbs.
    /// Acquired in lexicographic UUID order to prevent deadlocks (§11.3).
    PessimisticResourceLock,

    /// Scope-level exclusive lock (tenant/workspace/catalogue).
    /// For `admin_override` verbs. Very rare; most plans never use this.
    ExclusiveScopeLock,
}

// =============================================================================
// Coordination strategy lookup (v0.5 §5.3 table)
// =============================================================================

/// Map a single verb's `effect_class` to its `ConcurrencyPolicy` (v0.5 §5.3).
///
/// The runtime derives policy from class; verbs never declare policy directly.
pub fn effect_class_to_concurrency_policy(class: EffectClass) -> ConcurrencyPolicy {
    match class {
        EffectClass::Pure | EffectClass::ReadSnapshot => ConcurrencyPolicy::None,
        EffectClass::IdempotentEnsure => ConcurrencyPolicy::UniqueInsert,
        EffectClass::AppendFact | EffectClass::CommutativeAccumulate => {
            ConcurrencyPolicy::IdempotencyGuard
        }
        EffectClass::AppendTransitionSnapshot => {
            // Phase 5 fallback: use PessimisticResourceLock until T13 wires CAS.
            // Phase 6: switch to OptimisticSnapshotCheck for this class.
            ConcurrencyPolicy::PessimisticResourceLock
        }
        EffectClass::ReadModifyWrite | EffectClass::CrossResourceInvariant => {
            ConcurrencyPolicy::PessimisticResourceLock
        }
        EffectClass::ExternalEffect => {
            // External calls use outbox + idempotency guard; no pre-lock.
            ConcurrencyPolicy::IdempotencyGuard
        }
        EffectClass::AdminOverride => ConcurrencyPolicy::ExclusiveScopeLock,
    }
}

/// Effective coordination policy for an entire plan — the maximum policy
/// across all step effect classes (v0.5 §5.4, "cheapest safe strategy").
///
/// Returns `None` if every step is lock-free (pure / read_snapshot /
/// idempotent_ensure / append_fact). In that case the executor can skip
/// all pre-plan lock acquisition.
///
/// Returns `Some(policy)` with the most expensive policy seen otherwise.
pub fn plan_effective_policy(
    effect_classes: impl IntoIterator<Item = Option<EffectClass>>,
) -> Option<ConcurrencyPolicy> {
    let mut max_policy: Option<ConcurrencyPolicy> = std::option::Option::None;

    for class_opt in effect_classes {
        let class = match class_opt {
            // None = undeclared effect_class → treat as PessimisticResourceLock
            // (safe fallback: matches pre-T12 behaviour for all verbs).
            std::option::Option::None => ConcurrencyPolicy::PessimisticResourceLock,
            Some(c) => effect_class_to_concurrency_policy(c),
        };
        max_policy = Some(match max_policy {
            std::option::Option::None => class,
            Some(current) => current.max(class),
        });
    }

    max_policy
}

/// True if the plan requires any pre-plan lock acquisition.
///
/// A plan that is entirely `None`-policy (all pure/read_snapshot/
/// idempotent_ensure/append_fact) does NOT need advisory locks.
/// The executor can skip `acquire_locks()` for such plans.
pub fn plan_requires_locking(
    effect_classes: impl IntoIterator<Item = Option<EffectClass>>,
) -> bool {
    match plan_effective_policy(effect_classes) {
        std::option::Option::None => false, // empty plan — no locking
        Some(policy) => policy >= ConcurrencyPolicy::PessimisticResourceLock,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_maps_to_none() {
        assert_eq!(
            effect_class_to_concurrency_policy(EffectClass::Pure),
            ConcurrencyPolicy::None
        );
    }

    #[test]
    fn read_snapshot_maps_to_none() {
        assert_eq!(
            effect_class_to_concurrency_policy(EffectClass::ReadSnapshot),
            ConcurrencyPolicy::None
        );
    }

    #[test]
    fn idempotent_ensure_maps_to_unique_insert() {
        assert_eq!(
            effect_class_to_concurrency_policy(EffectClass::IdempotentEnsure),
            ConcurrencyPolicy::UniqueInsert
        );
    }

    #[test]
    fn read_modify_write_maps_to_pessimistic() {
        assert_eq!(
            effect_class_to_concurrency_policy(EffectClass::ReadModifyWrite),
            ConcurrencyPolicy::PessimisticResourceLock
        );
    }

    #[test]
    fn admin_override_maps_to_exclusive() {
        assert_eq!(
            effect_class_to_concurrency_policy(EffectClass::AdminOverride),
            ConcurrencyPolicy::ExclusiveScopeLock
        );
    }

    #[test]
    fn all_read_plan_does_not_require_locking() {
        let classes = vec![
            Some(EffectClass::Pure),
            Some(EffectClass::ReadSnapshot),
            Some(EffectClass::IdempotentEnsure),
        ];
        assert!(!plan_requires_locking(classes));
    }

    #[test]
    fn mixed_plan_with_rmw_requires_locking() {
        let classes = vec![
            Some(EffectClass::ReadSnapshot),
            Some(EffectClass::ReadModifyWrite),
        ];
        assert!(plan_requires_locking(classes));
    }

    #[test]
    fn undeclared_effect_class_requires_locking() {
        // None = undeclared → falls back to PessimisticResourceLock (safe)
        let classes: Vec<Option<EffectClass>> = vec![None];
        assert!(plan_requires_locking(classes));
    }

    #[test]
    fn empty_plan_does_not_require_locking() {
        let classes: Vec<Option<EffectClass>> = vec![];
        assert!(!plan_requires_locking(classes));
    }
}
