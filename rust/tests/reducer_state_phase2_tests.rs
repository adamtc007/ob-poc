#[cfg(feature = "database")]
mod tests {
    use std::collections::HashMap;

    use chrono::Utc;
    use uuid::Uuid;

    use ob_poc::sem_reg::reducer::{
        diagnose_slot, load_builtin_state_machine, reduce_slot, EvalScope, FieldValue, OverlayRow,
        SlotOverlayData, StateOverride,
    };

    fn overlays() -> SlotOverlayData {
        SlotOverlayData {
            sources: HashMap::from([
                (
                    "entity_ref".into(),
                    vec![OverlayRow {
                        fields: HashMap::from([
                            (
                                "entity_id".into(),
                                FieldValue::Str(Uuid::new_v4().to_string()),
                            ),
                            ("role".into(), FieldValue::Str("PRIMARY".into())),
                        ]),
                    }],
                ),
                (
                    "workstream".into(),
                    vec![OverlayRow {
                        fields: HashMap::from([(
                            "status".into(),
                            FieldValue::Str("VERIFIED".into()),
                        )]),
                    }],
                ),
                (
                    "screenings".into(),
                    vec![OverlayRow {
                        fields: HashMap::from([("status".into(), FieldValue::Str("CLEAR".into()))]),
                    }],
                ),
                (
                    "evidence".into(),
                    vec![OverlayRow {
                        fields: HashMap::from([(
                            "status".into(),
                            FieldValue::Str("VERIFIED".into()),
                        )]),
                    }],
                ),
                (
                    "doc_requests".into(),
                    vec![OverlayRow {
                        fields: HashMap::from([(
                            "status".into(),
                            FieldValue::Str("VERIFIED".into()),
                        )]),
                    }],
                ),
            ]),
            scope: EvalScope {
                cbu_id: None,
                case_id: None,
                case_status: Some("APPROVED".into()),
                fields: HashMap::new(),
            }
            .as_scope_data(),
            slots: vec![],
        }
    }

    fn ubo_overlays(status: &str) -> SlotOverlayData {
        SlotOverlayData {
            sources: HashMap::from([(
                "registry".into(),
                vec![OverlayRow {
                    fields: HashMap::from([("status".into(), FieldValue::Str(status.into()))]),
                }],
            )]),
            scope: EvalScope::default().as_scope_data(),
            slots: vec![],
        }
    }

    #[test]
    fn reduce_slot_uses_override_as_effective_state() {
        let sm = load_builtin_state_machine("entity_kyc_lifecycle").unwrap();
        let override_entry = StateOverride {
            id: Uuid::new_v4(),
            cbu_id: Uuid::new_v4(),
            case_id: None,
            constellation_type: "entity_kyc_lifecycle".into(),
            slot_path: "entity.primary".into(),
            computed_state: "verified".into(),
            override_state: "approved".into(),
            justification: "manual".into(),
            authority: "compliance".into(),
            conditions: None,
            reducer_revision: sm.reducer_revision.clone(),
            created_at: Utc::now(),
            expires_at: None,
            revoked_at: None,
            revoked_by: None,
            revoke_reason: None,
        };

        let result = reduce_slot(&sm, "entity.primary", &overlays(), Some(override_entry)).unwrap();
        assert_eq!(result.effective_state, "approved");
    }

    #[test]
    fn diagnose_slot_returns_rule_trace() {
        let sm = load_builtin_state_machine("entity_kyc_lifecycle").unwrap();
        let trace =
            diagnose_slot(&sm, Uuid::new_v4(), "entity.primary", &overlays(), None).unwrap();
        assert!(!trace.rules_evaluated.is_empty());
        assert_eq!(trace.state_machine, "entity_kyc_lifecycle");
    }

    #[test]
    fn reduce_slot_emits_consistency_warning() {
        let sm = load_builtin_state_machine("ubo_epistemic_lifecycle").unwrap();
        let result = reduce_slot(&sm, "entity.primary", &ubo_overlays("PROVABLE"), None).unwrap();
        assert_eq!(result.computed_state, "provable");
        assert_eq!(
            result.consistency_warnings,
            vec![String::from(
                "Provable UBO still lacks fully verified evidence"
            )]
        );
    }

    #[test]
    fn diagnose_slot_includes_consistency_warning() {
        let sm = load_builtin_state_machine("ubo_epistemic_lifecycle").unwrap();
        let trace = diagnose_slot(
            &sm,
            Uuid::new_v4(),
            "entity.primary",
            &ubo_overlays("PROVABLE"),
            None,
        )
        .unwrap();
        assert_eq!(
            trace.consistency_warnings,
            vec![String::from(
                "Provable UBO still lacks fully verified evidence"
            )]
        );
    }
}
