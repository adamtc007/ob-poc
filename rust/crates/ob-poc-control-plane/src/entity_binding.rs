//! G2 — Entity Binding (V&S §6.2).
//!
//! No production analogue exists today (Phase 0 inventory: "Gates with no
//! full production analogue: G2, G8, G9, G10, G13, G14"). T3.1 implements
//! the real binding gate; T1 defines the shape only.

use uuid::Uuid;

/// `EntityBindingReport` — V&S §6.2 "Output". A verb without entity scope
/// is not executable, so this is not a plain success/failure enum: it is
/// the report the gate always produces, and callers inspect
/// `EntityBindingReport::success()` for the `BoundEntities` proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityBindingReport {
    outcome: EntityBindingOutcome,
}

// No production analogue exists yet (T3.1 constructs real values); the
// only current caller of these variants is this module's own test.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum EntityBindingOutcome {
    Bound(BoundEntities),
    NotFound { entity_id: Uuid },
    WrongKind { entity_id: Uuid, expected: String, actual: String },
    LifecycleUnreadable { entity_id: Uuid },
    Unavailable { entity_id: Uuid, reason: String },
    OutsidePack { entity_id: Uuid },
}

impl EntityBindingReport {
    pub fn success(&self) -> Option<&BoundEntities> {
        match &self.outcome {
            EntityBindingOutcome::Bound(bound) => Some(bound),
            _ => None,
        }
    }
}

/// Success-form proof: every referenced entity exists, is of the expected
/// kind, has a readable lifecycle state, is available (not locked/archived)
/// and belongs to the active pack. Constructible only from within this
/// module.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BoundEntities {
    entity_ids: Vec<Uuid>,
}

impl BoundEntities {
    // Called by the (future) T3.1 adapter; the only caller today is the
    // cfg(test) bridge below.
    #[allow(dead_code)]
    fn new(entity_ids: Vec<Uuid>) -> Self {
        Self { entity_ids }
    }

    pub fn entity_ids(&self) -> &[Uuid] {
        &self.entity_ids
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    use super::BoundEntities;
    use uuid::Uuid;

    pub(crate) fn bound(entity_ids: Vec<Uuid>) -> BoundEntities {
        BoundEntities::new(entity_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bound_report_exposes_proof_only_on_success() {
        let bound = BoundEntities::new(vec![Uuid::nil()]);
        let report = EntityBindingReport {
            outcome: EntityBindingOutcome::Bound(bound.clone()),
        };
        assert_eq!(report.success(), Some(&bound));

        let rejected = EntityBindingReport {
            outcome: EntityBindingOutcome::NotFound {
                entity_id: Uuid::nil(),
            },
        };
        assert_eq!(rejected.success(), None);
    }
}
