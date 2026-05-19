//! Effect-class heuristic derivation (v0.5 §5.2, D1 decision).
//!
//! `effect_class` and `three_axis` are orthogonal verb declarations that
//! answer different questions:
//!
//! - `three_axis` → upstream UX/authority: "who may fire this, what consent
//!   is needed, which DAG transitions are legal"
//! - `effect_class` → downstream coordination: "how does the runtime
//!   serialize this verb against concurrent plans"
//!
//! When `effect_class` is not explicitly declared in YAML, this module
//! attempts to derive a default from the three-axis triple plus `behavior`
//! and `crud` config. The heuristic is conservative: it returns `Some` only
//! when the mapping is unambiguous (Appendix B.3, rows 1-6, 10, 12).
//! Ambiguous cases (rows 7, 8, 11, 13) return `None` — the verb author must
//! supply an explicit declaration.

use crate::config::types::{CrudOperation, ExternalEffect, StateEffect, VerbConfig};
use crate::executable_plan::EffectClass;

/// Attempt to derive `EffectClass` from a verb's three-axis declaration,
/// behavior, and CRUD config.
///
/// Returns `Some(effect_class)` only for unambiguous mappings.
/// Returns `None` for ambiguous cases (see module doc); the validator will
/// produce an `EffectClassAmbiguous` warning / error per `ValidationContext`.
///
/// # Mapping table (Appendix B.3)
///
/// | Pattern | Derived class | Confidence |
/// |---------|---------------|-----------|
/// | `crud: select` | `ReadSnapshot` | high |
/// | `external_effects: [navigating]` | `Pure` | high |
/// | `external_effects: [observational]` + preserving | `ReadSnapshot` | high |
/// | `state_effect: preserving` + benign + no externals + no state write | `Pure` | high |
/// | `crud: upsert` + `conflict_keys` non-empty | `IdempotentEnsure` | high |
/// | `external_effects: [emitting]` + preserving | `AppendFact` | medium |
/// | `state_effect: transition` + `[observational]` | `ReadModifyWrite` | high |
/// | `state_effect: transition` + `requires_explicit_authorisation` | `AdminOverride` | high |
/// | `state_effect: transition` + benign + no externals | `AppendTransitionSnapshot` | medium |
/// | (all other transition patterns) | None — ambiguous | low |
pub fn derive_effect_class_from_three_axis(verb: &VerbConfig) -> Option<EffectClass> {
    // Fast path: explicit CRUD select is unambiguously ReadSnapshot.
    if let Some(ref crud) = verb.crud {
        if crud.operation == CrudOperation::Select {
            return Some(EffectClass::ReadSnapshot);
        }
        // CRUD upsert with conflict_keys = idempotent-ensure pattern.
        let has_conflict_keys = crud
            .conflict_keys
            .as_ref()
            .map_or(false, |keys| !keys.is_empty());
        if crud.operation == CrudOperation::Upsert && has_conflict_keys {
            return Some(EffectClass::IdempotentEnsure);
        }
    }

    let decl = verb.three_axis.as_ref()?;
    let state = decl.state_effect;
    let externals = &decl.external_effects;
    let baseline = decl.consequence.baseline;

    let only_navigating = externals.len() == 1 && externals.contains(&ExternalEffect::Navigating);
    let only_observational =
        externals.len() == 1 && externals.contains(&ExternalEffect::Observational);
    let only_emitting = externals.len() == 1 && externals.contains(&ExternalEffect::Emitting);
    let no_externals = externals.is_empty();

    use crate::config::types::ConsequenceTier;

    match state {
        StateEffect::Preserving => {
            if only_navigating {
                // Navigation-only: viewport change, no DB effect.
                return Some(EffectClass::Pure);
            }
            if only_observational {
                // Reads external state; no mutation.
                return Some(EffectClass::ReadSnapshot);
            }
            if only_emitting {
                // Emits event/signal; no read-then-modify.
                return Some(EffectClass::AppendFact);
            }
            if no_externals && baseline == ConsequenceTier::Benign {
                // Pure compute: no externals, no state-write, safe for agent autonomy.
                return Some(EffectClass::Pure);
            }
            // Ambiguous: preserving + reviewable + plugin with state_write side_effects.
            // Rows 7, 13 from B.3 — author must declare explicitly.
            None
        }
        StateEffect::Transition => {
            if baseline == ConsequenceTier::RequiresExplicitAuthorisation {
                // Admin / repair / destructive — row 12.
                return Some(EffectClass::AdminOverride);
            }
            if only_observational {
                // Reads then modifies the same resource — row 10.
                return Some(EffectClass::ReadModifyWrite);
            }
            if no_externals && baseline == ConsequenceTier::Benign {
                // Workflow state snapshot advance — row 14 (BPMN canonical case).
                return Some(EffectClass::AppendTransitionSnapshot);
            }
            // All remaining transition patterns are ambiguous:
            // - transition + [] + reviewable (rows 8, 11 — RMW vs ATS vs CRI)
            // - transition + [emitting] (rows 9)
            // Author must supply explicit effect_class.
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::{
        ConsequenceDeclaration, ConsequenceTier, CrudConfig, CrudOperation, ExternalEffect,
        StateEffect, ThreeAxisDeclaration, VerbConfig,
    };

    fn crud_config(operation: CrudOperation, conflict_keys: Option<Vec<String>>) -> CrudConfig {
        CrudConfig {
            operation,
            table: None,
            schema: None,
            key: None,
            returning: None,
            conflict_keys,
            conflict_constraint: None,
            junction: None,
            from_col: None,
            to_col: None,
            role_table: None,
            role_col: None,
            fk_col: None,
            filter_col: None,
            primary_table: None,
            join_table: None,
            join_col: None,
            base_table: None,
            extension_table: None,
            extension_table_column: None,
            type_id_column: None,
            type_code: None,
            order_by: None,
            set_values: None,
        }
    }

    fn verb_with_three_axis(
        state: StateEffect,
        externals: Vec<ExternalEffect>,
        baseline: ConsequenceTier,
    ) -> VerbConfig {
        let mut v = VerbConfig::default();
        v.three_axis = Some(ThreeAxisDeclaration {
            state_effect: state,
            external_effects: externals,
            consequence: ConsequenceDeclaration {
                baseline,
                escalation: vec![],
            },
            transitions: None,
        });
        v
    }

    #[test]
    fn crud_select_is_read_snapshot() {
        let mut v = VerbConfig::default();
        v.crud = Some(crud_config(CrudOperation::Select, None));
        assert_eq!(
            derive_effect_class_from_three_axis(&v),
            Some(EffectClass::ReadSnapshot)
        );
    }

    #[test]
    fn crud_upsert_with_conflict_keys_is_idempotent_ensure() {
        let mut v = VerbConfig::default();
        v.crud = Some(crud_config(
            CrudOperation::Upsert,
            Some(vec!["entity_id".to_string(), "regulator_code".to_string()]),
        ));
        assert_eq!(
            derive_effect_class_from_three_axis(&v),
            Some(EffectClass::IdempotentEnsure)
        );
    }

    #[test]
    fn navigating_is_pure() {
        let v = verb_with_three_axis(
            StateEffect::Preserving,
            vec![ExternalEffect::Navigating],
            ConsequenceTier::Benign,
        );
        assert_eq!(
            derive_effect_class_from_three_axis(&v),
            Some(EffectClass::Pure)
        );
    }

    #[test]
    fn observational_preserving_is_read_snapshot() {
        let v = verb_with_three_axis(
            StateEffect::Preserving,
            vec![ExternalEffect::Observational],
            ConsequenceTier::Reviewable,
        );
        assert_eq!(
            derive_effect_class_from_three_axis(&v),
            Some(EffectClass::ReadSnapshot)
        );
    }

    #[test]
    fn emitting_preserving_is_append_fact() {
        let v = verb_with_three_axis(
            StateEffect::Preserving,
            vec![ExternalEffect::Emitting],
            ConsequenceTier::Reviewable,
        );
        assert_eq!(
            derive_effect_class_from_three_axis(&v),
            Some(EffectClass::AppendFact)
        );
    }

    #[test]
    fn benign_no_externals_preserving_is_pure() {
        let v = verb_with_three_axis(StateEffect::Preserving, vec![], ConsequenceTier::Benign);
        assert_eq!(
            derive_effect_class_from_three_axis(&v),
            Some(EffectClass::Pure)
        );
    }

    #[test]
    fn preserving_reviewable_no_externals_is_ambiguous() {
        let v = verb_with_three_axis(StateEffect::Preserving, vec![], ConsequenceTier::Reviewable);
        assert_eq!(derive_effect_class_from_three_axis(&v), None);
    }

    #[test]
    fn transition_explicit_auth_is_admin_override() {
        let v = verb_with_three_axis(
            StateEffect::Transition,
            vec![],
            ConsequenceTier::RequiresExplicitAuthorisation,
        );
        assert_eq!(
            derive_effect_class_from_three_axis(&v),
            Some(EffectClass::AdminOverride)
        );
    }

    #[test]
    fn transition_observational_is_read_modify_write() {
        let v = verb_with_three_axis(
            StateEffect::Transition,
            vec![ExternalEffect::Observational],
            ConsequenceTier::Reviewable,
        );
        assert_eq!(
            derive_effect_class_from_three_axis(&v),
            Some(EffectClass::ReadModifyWrite)
        );
    }

    #[test]
    fn transition_benign_no_externals_is_append_transition_snapshot() {
        let v = verb_with_three_axis(StateEffect::Transition, vec![], ConsequenceTier::Benign);
        assert_eq!(
            derive_effect_class_from_three_axis(&v),
            Some(EffectClass::AppendTransitionSnapshot)
        );
    }

    #[test]
    fn transition_reviewable_no_externals_is_ambiguous() {
        // Row 8 from B.3 — could be RMW or ATS
        let v = verb_with_three_axis(
            StateEffect::Transition,
            vec![],
            ConsequenceTier::RequiresConfirmation,
        );
        assert_eq!(derive_effect_class_from_three_axis(&v), None);
    }

    #[test]
    fn no_three_axis_returns_none() {
        let v = VerbConfig::default();
        assert_eq!(derive_effect_class_from_three_axis(&v), None);
    }
}
