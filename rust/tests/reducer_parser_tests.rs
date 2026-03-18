#[cfg(feature = "database")]
mod tests {
    use ob_poc::state_reducer::{
        parse_condition_body, AggFn, CompareOp, ConditionBody, Expr, Literal, Predicate,
        SlotPredicate, Value,
    };

    #[test]
    fn parse_param_ref() {
        let body = parse_condition_body("status_is($1)").unwrap();
        assert_eq!(
            body,
            ConditionBody::Call {
                name: "status_is".into(),
                args: vec![Value::Param(1)],
            }
        );
    }

    #[test]
    fn parse_scope_comparison() {
        let body = parse_condition_body("scope.case_status = 'APPROVED'").unwrap();
        assert_eq!(
            body,
            ConditionBody::Leaf {
                expr: Expr::ScopeComparison {
                    path: vec!["case_status".into()],
                    op: CompareOp::Eq,
                    value: Value::Literal(Literal::Str("APPROVED".into())),
                },
                compare: None,
            }
        );
    }

    #[test]
    fn parse_scope_slot_aggregate_any() {
        let body = parse_condition_body(
            "ANY(scope.slots WHERE type = 'entity' AND effective_state IN ('verified', 'approved'))",
        )
        .unwrap();
        match body {
            ConditionBody::Leaf {
                expr: Expr::SlotAggregate { function, filter },
                compare: None,
            } => {
                assert_eq!(function, AggFn::Any);
                assert!(matches!(filter, SlotPredicate::And(_)));
            }
            _ => panic!("unexpected parse shape"),
        }
    }

    #[test]
    fn parse_where_and_or_precedence() {
        let body = parse_condition_body(
            "COUNT(screening WHERE status = 'CLEAR' OR severity = 'HARD_STOP' AND resolved_at IS NULL) > 0",
        )
        .unwrap();
        match body {
            ConditionBody::Leaf {
                expr:
                    Expr::Aggregate {
                        filter: Some(Predicate::Or(items)),
                        ..
                    },
                ..
            } => {
                assert_eq!(items.len(), 2);
                assert!(matches!(items[1], Predicate::And(_)));
            }
            _ => panic!("unexpected parse shape"),
        }
    }

    #[test]
    fn parse_rejects_keyword_as_ref() {
        assert!(parse_condition_body("AND").is_err());
    }

    #[test]
    fn parse_rejects_empty_input() {
        assert!(parse_condition_body("").is_err());
    }

    #[test]
    fn parse_rejects_unclosed_paren() {
        assert!(parse_condition_body("(foo").is_err());
    }

    #[test]
    fn parse_slot_predicate_precedence() {
        let body = parse_condition_body(
            "ALL(scope.slots WHERE type = 'entity' OR cardinality = 'mandatory' AND effective_state = 'verified')",
        )
        .unwrap();
        match body {
            ConditionBody::Leaf {
                expr: Expr::SlotAggregate { filter, .. },
                ..
            } => match filter {
                SlotPredicate::Or(items) => {
                    assert_eq!(items.len(), 2);
                    assert!(matches!(items[1], SlotPredicate::And(_)));
                }
                _ => panic!("unexpected slot predicate shape"),
            },
            _ => panic!("unexpected parse shape"),
        }
    }
}
