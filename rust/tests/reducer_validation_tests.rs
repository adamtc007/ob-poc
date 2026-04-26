#[cfg(feature = "database")]
mod tests {
    use std::collections::HashMap;

    use dsl_runtime::state_reducer::{
        validate_state_machine, ConditionDef, ConsistencyCheckDef, OverlaySourceDef, ReducerDef,
        RuleDef, StateMachineDefinition,
    };

    fn base_definition() -> StateMachineDefinition {
        StateMachineDefinition {
            state_machine: "entity_kyc".into(),
            description: None,
            states: vec!["approved".into(), "proved".into()],
            initial: "proved".into(),
            transitions: vec![],
            reducer: ReducerDef {
                overlay_sources: HashMap::from([(
                    "entity".into(),
                    OverlaySourceDef {
                        table: "entity_overlay".into(),
                        join: "entity_id".into(),
                        provides: vec!["state".into()],
                        cardinality: None,
                    },
                )]),
                conditions: HashMap::from([
                    (
                        "status_is".into(),
                        ConditionDef {
                            expr: "entity.state = $1".into(),
                            description: None,
                            parameterized: true,
                        },
                    ),
                    (
                        "approved".into(),
                        ConditionDef {
                            expr: "status_is('approved')".into(),
                            description: None,
                            parameterized: false,
                        },
                    ),
                ]),
                rules: vec![
                    RuleDef {
                        state: "approved".into(),
                        requires: vec!["approved".into()],
                        excludes: vec![],
                        consistency_check: None,
                    },
                    RuleDef {
                        state: "proved".into(),
                        requires: vec![],
                        excludes: vec![],
                        consistency_check: None,
                    },
                ],
            },
        }
    }

    #[test]
    fn validates_parameterized_condition() {
        let definition = base_definition();
        let validated = validate_state_machine(&definition).unwrap();
        assert_eq!(validated.name, "entity_kyc");
    }

    #[test]
    fn rejects_unknown_overlay_source() {
        let mut definition = base_definition();
        definition.reducer.conditions.insert(
            "broken".into(),
            ConditionDef {
                expr: "unknown.state = 'approved'".into(),
                description: None,
                parameterized: false,
            },
        );
        assert!(validate_state_machine(&definition).is_err());
    }

    #[test]
    fn rejects_unknown_condition_ref() {
        let mut definition = base_definition();
        definition.reducer.conditions.insert(
            "broken".into(),
            ConditionDef {
                expr: "missing_condition".into(),
                description: None,
                parameterized: false,
            },
        );
        assert!(validate_state_machine(&definition).is_err());
    }

    #[test]
    fn rejects_condition_cycle() {
        let mut definition = base_definition();
        definition.reducer.conditions.insert(
            "a".into(),
            ConditionDef {
                expr: "b".into(),
                description: None,
                parameterized: false,
            },
        );
        definition.reducer.conditions.insert(
            "b".into(),
            ConditionDef {
                expr: "a".into(),
                description: None,
                parameterized: false,
            },
        );
        assert!(validate_state_machine(&definition).is_err());
    }

    #[test]
    fn rejects_param_arity_mismatch() {
        let mut definition = base_definition();
        definition.reducer.conditions.insert(
            "approved".into(),
            ConditionDef {
                expr: "status_is()".into(),
                description: None,
                parameterized: false,
            },
        );
        assert!(validate_state_machine(&definition).is_err());
    }

    #[test]
    fn rejects_missing_state_rule() {
        let mut definition = base_definition();
        definition.reducer.rules.pop();
        assert!(validate_state_machine(&definition).is_err());
    }

    #[test]
    fn rejects_duplicate_state_rule() {
        let mut definition = base_definition();
        definition.reducer.rules.push(RuleDef {
            state: "approved".into(),
            requires: vec![],
            excludes: vec![],
            consistency_check: None,
        });
        assert!(validate_state_machine(&definition).is_err());
    }

    #[test]
    fn rejects_nonempty_fallback() {
        let mut definition = base_definition();
        if let Some(last) = definition.reducer.rules.last_mut() {
            last.requires = vec!["approved".into()];
        }
        assert!(validate_state_machine(&definition).is_err());
    }

    #[test]
    fn rejects_unknown_rule_condition_reference() {
        let mut definition = base_definition();
        definition.reducer.rules[0].requires = vec!["missing".into()];
        assert!(validate_state_machine(&definition).is_err());
    }

    #[test]
    fn rejects_unknown_consistency_check_reference() {
        let mut definition = base_definition();
        definition.reducer.rules[0].consistency_check = Some(ConsistencyCheckDef {
            warn_unless: "missing".into(),
            warning: "broken".into(),
        });
        assert!(validate_state_machine(&definition).is_err());
    }
}
