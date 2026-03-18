#[cfg(feature = "database")]
mod tests {
    use std::collections::HashMap;

    use ob_poc::state_reducer::{
        evaluate_rules, parse_condition_body, ConditionEvaluator, FieldValue, OverlayRow, RuleDef,
        ScopeData, SlotOverlayData, SlotRecord,
    };

    fn row(fields: &[(&str, FieldValue)]) -> OverlayRow {
        OverlayRow {
            fields: fields
                .iter()
                .map(|(key, value)| ((*key).to_string(), value.clone()))
                .collect(),
        }
    }

    fn data() -> SlotOverlayData {
        SlotOverlayData {
            sources: HashMap::from([
                (
                    "screening".into(),
                    vec![row(&[
                        ("status", FieldValue::Str("CLEAR".into())),
                        ("severity", FieldValue::Str("INFO".into())),
                        ("resolved_at", FieldValue::Null),
                    ])],
                ),
                (
                    "entity".into(),
                    vec![row(&[("state", FieldValue::Str("verified".into()))])],
                ),
            ]),
            scope: ScopeData {
                fields: serde_json::json!({
                    "case_status": "APPROVED"
                }),
            },
            slots: vec![
                SlotRecord {
                    slot_type: "entity".into(),
                    cardinality: "mandatory".into(),
                    effective_state: "verified".into(),
                    computed_state: "verified".into(),
                },
                SlotRecord {
                    slot_type: "entity".into(),
                    cardinality: "optional".into(),
                    effective_state: "approved".into(),
                    computed_state: "approved".into(),
                },
            ],
        }
    }

    #[test]
    fn eval_scope_comparison_match() {
        let asts = HashMap::from([(
            "case_approved".into(),
            parse_condition_body("scope.case_status = 'APPROVED'").unwrap(),
        )]);
        let order = vec!["case_approved".into()];
        let mut evaluator = ConditionEvaluator::new(&order, &asts);
        let results = evaluator.evaluate_all(&data()).unwrap();
        assert_eq!(results.get("case_approved"), Some(&true));
    }

    #[test]
    fn eval_call_binds_params() {
        let asts = HashMap::from([
            (
                "status_is".into(),
                parse_condition_body("entity.state = $1").unwrap(),
            ),
            (
                "approved".into(),
                parse_condition_body("status_is('verified')").unwrap(),
            ),
        ]);
        let order = vec!["status_is".into(), "approved".into()];
        let mut evaluator = ConditionEvaluator::new(&order, &asts);
        let results = evaluator.evaluate_all(&data()).unwrap();
        assert_eq!(results.get("approved"), Some(&true));
    }

    #[test]
    fn eval_rules_first_match_wins() {
        let results = HashMap::from([("approved".into(), true), ("verified".into(), true)]);
        let rules = vec![
            RuleDef {
                state: "approved".into(),
                requires: vec!["approved".into()],
                excludes: vec![],
                consistency_check: None,
            },
            RuleDef {
                state: "verified".into(),
                requires: vec!["verified".into()],
                excludes: vec![],
                consistency_check: None,
            },
        ];
        assert_eq!(evaluate_rules(&rules, &results).unwrap(), "approved");
    }

    #[test]
    fn eval_slot_aggregate_all() {
        let asts = HashMap::from([(
            "all_verified".into(),
            parse_condition_body(
                "ALL(scope.slots WHERE type = 'entity' AND effective_state IN ('verified', 'approved'))",
            )
            .unwrap(),
        )]);
        let order = vec!["all_verified".into()];
        let mut evaluator = ConditionEvaluator::new(&order, &asts);
        let results = evaluator.evaluate_all(&data()).unwrap();
        assert_eq!(results.get("all_verified"), Some(&true));
    }
}
