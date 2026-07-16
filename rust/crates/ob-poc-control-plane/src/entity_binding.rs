//! G2 — Entity Binding (V&S §6.2).
//!
//! No production analogue exists today (Phase 0 inventory: "Gates with no
//! full production analogue: G2, G8, G9, G10, G13, G14"). T3.1 implements
//! the real binding gate: existence, kind match, lifecycle-state
//! readability, availability (locked/archived), and pack membership, for
//! every entity a candidate verb references — the call site collects these
//! per-entity facts (no existing validator to wrap; this crate does not
//! perform the entity lookup itself, per §9.1), this module only grades
//! them. Replaces the placeholder `ResolvedEntity{row_version:0}` (RR-5
//! Mode-1 row 1) referenced by the plan.

use uuid::Uuid;

use crate::gate::{Gate, GateId, GateResult};

/// `EntityBindingReport` — V&S §6.2 "Output". A verb without entity scope
/// is not executable, so this is not a plain success/failure enum: it is
/// the report the gate always produces, and callers inspect
/// `EntityBindingReport::success()` for the `BoundEntities` proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityBindingReport {
    outcome: EntityBindingOutcome,
}

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

#[cfg(any(test, feature = "test-support"))]
pub mod tests_support {
    use super::BoundEntities;
    use uuid::Uuid;

    pub fn bound(entity_ids: Vec<Uuid>) -> BoundEntities {
        BoundEntities::new(entity_ids)
    }
}

/// Per-entity facts the call site collects for one referenced entity — no
/// existing validator to wrap (RR-8), so this is the raw fact set a real
/// lookup would produce, not a translation of an existing decision.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EntityFacts {
    pub entity_id: Uuid,
    pub exists: bool,
    pub expected_kind: String,
    pub actual_kind: String,
    pub lifecycle_state_readable: bool,
    /// `true` when the entity is locked/archived/otherwise unavailable for
    /// mutation.
    pub availability_blocked: bool,
    pub availability_reason: Option<String>,
    /// `true` when the entity belongs to the active pack's scope.
    pub in_active_pack: bool,
}

/// Pre-computed input for the entity binding gate: the full set of entities
/// a candidate verb references.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct EntityBindingInput {
    pub entities: Vec<EntityFacts>,
}

/// Grades pre-collected per-entity facts against §6.2's checklist in order
/// (existence, kind, lifecycle readability, availability, pack membership),
/// returning the first entity that fails any check. Pure function — no I/O,
/// no entity lookup.
pub(crate) fn decide(input: &EntityBindingInput) -> EntityBindingReport {
    for facts in &input.entities {
        let outcome = if !facts.exists {
            Some(EntityBindingOutcome::NotFound {
                entity_id: facts.entity_id,
            })
        } else if facts.expected_kind != facts.actual_kind {
            Some(EntityBindingOutcome::WrongKind {
                entity_id: facts.entity_id,
                expected: facts.expected_kind.clone(),
                actual: facts.actual_kind.clone(),
            })
        } else if !facts.lifecycle_state_readable {
            Some(EntityBindingOutcome::LifecycleUnreadable {
                entity_id: facts.entity_id,
            })
        } else if facts.availability_blocked {
            Some(EntityBindingOutcome::Unavailable {
                entity_id: facts.entity_id,
                reason: facts
                    .availability_reason
                    .clone()
                    .unwrap_or_else(|| "unavailable".to_string()),
            })
        } else if !facts.in_active_pack {
            Some(EntityBindingOutcome::OutsidePack {
                entity_id: facts.entity_id,
            })
        } else {
            None
        };
        if let Some(outcome) = outcome {
            return EntityBindingReport { outcome };
        }
    }
    EntityBindingReport {
        outcome: EntityBindingOutcome::Bound(BoundEntities::new(
            input.entities.iter().map(|f| f.entity_id).collect(),
        )),
    }
}

/// T3.1 adapter: `Gate<crate::context::EvaluationContext>` impl for G2.
pub struct EntityBindingGate;

impl Gate<crate::context::EvaluationContext> for EntityBindingGate {
    fn id(&self) -> GateId {
        GateId::EntityBinding
    }

    fn evaluate(&self, ctx: &crate::context::EvaluationContext) -> GateResult {
        let Some(input) = &ctx.entity_binding else {
            return GateResult::Failure("no EntityBindingInput supplied".to_string());
        };
        let report = decide(input);
        match report.success() {
            Some(_) => GateResult::Success,
            None => GateResult::Failure(format!("{:?}", report.outcome)),
        }
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

    fn valid_facts(entity_id: Uuid) -> EntityFacts {
        EntityFacts {
            entity_id,
            exists: true,
            expected_kind: "cbu".to_string(),
            actual_kind: "cbu".to_string(),
            lifecycle_state_readable: true,
            availability_blocked: false,
            availability_reason: None,
            in_active_pack: true,
        }
    }

    #[test]
    fn all_entities_valid_binds_every_id() {
        let id1 = Uuid::from_u128(1);
        let id2 = Uuid::from_u128(2);
        let input = EntityBindingInput {
            entities: vec![valid_facts(id1), valid_facts(id2)],
        };
        let report = decide(&input);
        assert_eq!(report.success().map(|b| b.entity_ids().to_vec()), Some(vec![id1, id2]));
    }

    #[test]
    fn missing_entity_reports_not_found() {
        let id = Uuid::nil();
        let input = EntityBindingInput {
            entities: vec![EntityFacts {
                exists: false,
                ..valid_facts(id)
            }],
        };
        assert_eq!(
            decide(&input),
            EntityBindingReport {
                outcome: EntityBindingOutcome::NotFound { entity_id: id }
            }
        );
    }

    #[test]
    fn kind_mismatch_reports_wrong_kind() {
        let id = Uuid::nil();
        let input = EntityBindingInput {
            entities: vec![EntityFacts {
                actual_kind: "entity".to_string(),
                ..valid_facts(id)
            }],
        };
        assert_eq!(
            decide(&input),
            EntityBindingReport {
                outcome: EntityBindingOutcome::WrongKind {
                    entity_id: id,
                    expected: "cbu".to_string(),
                    actual: "entity".to_string(),
                }
            }
        );
    }

    #[test]
    fn locked_entity_reports_unavailable() {
        let id = Uuid::nil();
        let input = EntityBindingInput {
            entities: vec![EntityFacts {
                availability_blocked: true,
                availability_reason: Some("archived".to_string()),
                ..valid_facts(id)
            }],
        };
        assert_eq!(
            decide(&input),
            EntityBindingReport {
                outcome: EntityBindingOutcome::Unavailable {
                    entity_id: id,
                    reason: "archived".to_string(),
                }
            }
        );
    }

    #[test]
    fn first_failing_entity_wins_over_later_valid_ones() {
        let bad = Uuid::from_u128(1);
        let good = Uuid::from_u128(2);
        let input = EntityBindingInput {
            entities: vec![
                EntityFacts {
                    exists: false,
                    ..valid_facts(bad)
                },
                valid_facts(good),
            ],
        };
        assert_eq!(
            decide(&input),
            EntityBindingReport {
                outcome: EntityBindingOutcome::NotFound { entity_id: bad }
            }
        );
    }

    #[test]
    fn gate_evaluate_fails_closed_when_input_missing() {
        let ctx = crate::context::EvaluationContext::default();
        assert!(matches!(EntityBindingGate.evaluate(&ctx), GateResult::Failure(_)));
        assert_eq!(EntityBindingGate.id(), GateId::EntityBinding);
    }

    #[test]
    fn gate_evaluate_succeeds_when_all_entities_bind() {
        let ctx = crate::context::EvaluationContext {
            entity_binding: Some(EntityBindingInput {
                entities: vec![valid_facts(Uuid::nil())],
            }),
            ..Default::default()
        };
        assert_eq!(EntityBindingGate.evaluate(&ctx), GateResult::Success);
    }
}
