#[cfg(feature = "database")]
mod tests {
    use std::collections::HashMap;

    use uuid::Uuid;

    use ob_poc::sem_reg::reducer::{
        load_builtin_state_machine, load_state_machine, reduce_slot, EvalScope, FieldValue,
        OverlayRow, SlotOverlayData, SlotRecord,
    };

    const CASE_LIFECYCLE_YAML: &str = r#"
state_machine: case_lifecycle
description: Cross-slot aggregate lifecycle
states: [ready, blocked]
initial: blocked
transitions:
  - from: blocked
    to: ready
    verbs: [case.advance]
reducer:
  overlay_sources: {}
  conditions:
    all_entities_verified:
      expr: "ALL(scope.slots WHERE type = 'entity' AND effective_state IN ('verified', 'approved'))"
    any_blocked_slots:
      expr: "ANY(scope.slots WHERE type = 'entity' AND effective_state IN ('workstream_open', 'screening_complete'))"
  rules:
    - state: ready
      requires: [all_entities_verified]
    - state: blocked
      requires: []
"#;

    struct OverlayBuilder {
        sources: HashMap<String, Vec<OverlayRow>>,
    }

    impl OverlayBuilder {
        fn new() -> Self {
            Self {
                sources: HashMap::new(),
            }
        }

        fn entity_ref(mut self, role: &str) -> Self {
            self.sources
                .entry("entity_ref".into())
                .or_default()
                .push(OverlayRow {
                    fields: HashMap::from([
                        (
                            "entity_id".into(),
                            FieldValue::Str(Uuid::new_v4().to_string()),
                        ),
                        ("role".into(), FieldValue::Str(role.to_string())),
                    ]),
                });
            self
        }

        fn workstream(mut self, status: &str, risk_rating: Option<&str>) -> Self {
            self.sources
                .entry("workstream".into())
                .or_default()
                .push(OverlayRow {
                    fields: HashMap::from([
                        ("status".into(), FieldValue::Str(status.to_string())),
                        (
                            "risk_rating".into(),
                            risk_rating
                                .map(|value| FieldValue::Str(value.to_string()))
                                .unwrap_or(FieldValue::Null),
                        ),
                    ]),
                });
            self
        }

        fn screening(mut self, screening_type: &str, status: &str) -> Self {
            self.sources
                .entry("screenings".into())
                .or_default()
                .push(OverlayRow {
                    fields: HashMap::from([
                        (
                            "screening_type".into(),
                            FieldValue::Str(screening_type.to_string()),
                        ),
                        ("status".into(), FieldValue::Str(status.to_string())),
                    ]),
                });
            self
        }

        fn evidence(mut self, evidence_type: &str, status: &str) -> Self {
            self.sources
                .entry("evidence".into())
                .or_default()
                .push(OverlayRow {
                    fields: HashMap::from([
                        (
                            "evidence_type".into(),
                            FieldValue::Str(evidence_type.to_string()),
                        ),
                        ("status".into(), FieldValue::Str(status.to_string())),
                    ]),
                });
            self
        }

        fn red_flag(mut self, severity: &str, status: &str) -> Self {
            self.sources
                .entry("red_flags".into())
                .or_default()
                .push(OverlayRow {
                    fields: HashMap::from([
                        ("flag_type".into(), FieldValue::Str("SCREENING".into())),
                        ("severity".into(), FieldValue::Str(severity.to_string())),
                        ("status".into(), FieldValue::Str(status.to_string())),
                    ]),
                });
            self
        }

        fn doc_request(mut self, request_type: &str, status: &str) -> Self {
            self.sources
                .entry("doc_requests".into())
                .or_default()
                .push(OverlayRow {
                    fields: HashMap::from([
                        (
                            "request_type".into(),
                            FieldValue::Str(request_type.to_string()),
                        ),
                        ("status".into(), FieldValue::Str(status.to_string())),
                    ]),
                });
            self
        }

        fn with_source(mut self, name: &str, rows: Vec<OverlayRow>) -> Self {
            self.sources.insert(name.to_string(), rows);
            self
        }

        fn build(self) -> SlotOverlayData {
            SlotOverlayData {
                sources: self.sources,
                scope: EvalScope::default().as_scope_data(),
                slots: vec![],
            }
        }
    }

    struct ReduceResult {
        computed_state: String,
        available_verbs: Vec<String>,
    }

    fn load_entity_kyc_sm() -> ob_poc::sem_reg::reducer::ValidatedStateMachine {
        load_builtin_state_machine("entity_kyc_lifecycle").unwrap()
    }

    fn load_ubo_sm() -> ob_poc::sem_reg::reducer::ValidatedStateMachine {
        load_builtin_state_machine("ubo_epistemic_lifecycle").unwrap()
    }

    fn load_case_sm() -> ob_poc::sem_reg::reducer::ValidatedStateMachine {
        load_state_machine(CASE_LIFECYCLE_YAML).unwrap()
    }

    fn scope_with_case_status(status: &str) -> EvalScope {
        EvalScope {
            case_status: Some(status.to_string()),
            ..Default::default()
        }
    }

    fn reduce(
        sm: &ob_poc::sem_reg::reducer::ValidatedStateMachine,
        mut overlays: SlotOverlayData,
        scope: EvalScope,
    ) -> ReduceResult {
        overlays.scope = scope.as_scope_data();
        let result = reduce_slot(sm, "slot.primary", &overlays, None).unwrap();
        ReduceResult {
            computed_state: result.computed_state,
            available_verbs: result.available_verbs,
        }
    }

    fn row(fields: &[(&str, FieldValue)]) -> OverlayRow {
        OverlayRow {
            fields: fields
                .iter()
                .map(|(key, value)| ((*key).to_string(), value.clone()))
                .collect(),
        }
    }

    #[test]
    fn tv01_empty_slot() {
        let result = reduce(
            &load_entity_kyc_sm(),
            OverlayBuilder::new().build(),
            EvalScope::default(),
        );
        assert_eq!(result.computed_state, "empty");
        assert!(result
            .available_verbs
            .contains(&"entity.ensure-or-placeholder".to_string()));
        assert!(result.available_verbs.contains(&"party.add".to_string()));
    }

    #[test]
    fn tv02_placeholder_assigned() {
        let result = reduce(
            &load_entity_kyc_sm(),
            OverlayBuilder::new().entity_ref("PLACEHOLDER").build(),
            EvalScope::default(),
        );
        assert_eq!(result.computed_state, "placeholder");
    }

    #[test]
    fn tv03_real_entity_no_workstream() {
        let result = reduce(
            &load_entity_kyc_sm(),
            OverlayBuilder::new()
                .entity_ref("management-company")
                .build(),
            EvalScope::default(),
        );
        assert_eq!(result.computed_state, "filled");
        assert!(result
            .available_verbs
            .contains(&"kyc-workstream.add".to_string()));
    }

    #[test]
    fn tv04_workstream_open() {
        let result = reduce(
            &load_entity_kyc_sm(),
            OverlayBuilder::new()
                .entity_ref("management-company")
                .workstream("OPEN", None)
                .build(),
            EvalScope::default(),
        );
        assert_eq!(result.computed_state, "workstream_open");
    }

    #[test]
    fn tv05_screening_clear_evidence_pending() {
        let result = reduce(
            &load_entity_kyc_sm(),
            OverlayBuilder::new()
                .entity_ref("management-company")
                .workstream("COLLECT", None)
                .screening("SANCTIONS", "CLEAR")
                .screening("PEP", "CLEAR")
                .evidence("SHARE_REGISTER", "REQUIRED")
                .build(),
            EvalScope::default(),
        );
        assert_eq!(result.computed_state, "screening_complete");
    }

    #[test]
    fn tv06_screening_clear_evidence_verified() {
        let result = reduce(
            &load_entity_kyc_sm(),
            OverlayBuilder::new()
                .entity_ref("management-company")
                .workstream("COLLECT", None)
                .screening("SANCTIONS", "CLEAR")
                .screening("PEP", "CLEAR")
                .evidence("SHARE_REGISTER", "VERIFIED")
                .doc_request("PASSPORT", "VERIFIED")
                .build(),
            EvalScope::default(),
        );
        assert_eq!(result.computed_state, "evidence_collected");
    }

    #[test]
    fn tv07_everything_done_but_blocking_flag() {
        let result = reduce(
            &load_entity_kyc_sm(),
            OverlayBuilder::new()
                .entity_ref("management-company")
                .workstream("COLLECT", None)
                .screening("SANCTIONS", "CLEAR")
                .evidence("SHARE_REGISTER", "VERIFIED")
                .red_flag("HARD_STOP", "OPEN")
                .build(),
            EvalScope::default(),
        );
        assert_eq!(result.computed_state, "workstream_open");
    }

    #[test]
    fn tv09_workstream_closed_case_not_approved() {
        let result = reduce(
            &load_entity_kyc_sm(),
            OverlayBuilder::new()
                .entity_ref("management-company")
                .workstream("COMPLETE", Some("MEDIUM"))
                .screening("SANCTIONS", "CLEAR")
                .evidence("SHARE_REGISTER", "VERIFIED")
                .doc_request("PASSPORT", "VERIFIED")
                .build(),
            scope_with_case_status("ASSESSMENT"),
        );
        assert_eq!(result.computed_state, "verified");
    }

    #[test]
    fn tv10_full_approval() {
        let result = reduce(
            &load_entity_kyc_sm(),
            OverlayBuilder::new()
                .entity_ref("management-company")
                .workstream("COMPLETE", Some("MEDIUM"))
                .screening("SANCTIONS", "CLEAR")
                .evidence("SHARE_REGISTER", "VERIFIED")
                .doc_request("PASSPORT", "VERIFIED")
                .build(),
            scope_with_case_status("APPROVED"),
        );
        assert_eq!(result.computed_state, "approved");
    }

    #[test]
    fn tv13_no_registry_entry() {
        let result = reduce(
            &load_ubo_sm(),
            OverlayBuilder::new().build(),
            EvalScope::default(),
        );
        assert_eq!(result.computed_state, "undiscovered");
    }

    #[test]
    fn tv14_alleged_no_evidence() {
        let result = reduce(
            &load_ubo_sm(),
            OverlayBuilder::new()
                .with_source(
                    "registry",
                    vec![row(&[("status", FieldValue::Str("ALLEGED".into()))])],
                )
                .build(),
            EvalScope::default(),
        );
        assert_eq!(result.computed_state, "alleged");
    }

    #[test]
    fn tv15_provable_no_evidence() {
        let result = reduce(
            &load_ubo_sm(),
            OverlayBuilder::new()
                .with_source(
                    "registry",
                    vec![row(&[("status", FieldValue::Str("PROVABLE".into()))])],
                )
                .build(),
            EvalScope::default(),
        );
        assert_eq!(result.computed_state, "provable");
    }

    #[test]
    fn tv16_proved_evidence_verified() {
        let result = reduce(
            &load_ubo_sm(),
            OverlayBuilder::new()
                .with_source(
                    "registry",
                    vec![row(&[("status", FieldValue::Str("PROVED".into()))])],
                )
                .evidence("SHARE_REGISTER", "VERIFIED")
                .build(),
            EvalScope::default(),
        );
        assert_eq!(result.computed_state, "proved");
    }

    #[test]
    fn tv18_approved() {
        let result = reduce(
            &load_ubo_sm(),
            OverlayBuilder::new()
                .with_source(
                    "registry",
                    vec![row(&[("status", FieldValue::Str("APPROVED".into()))])],
                )
                .build(),
            EvalScope::default(),
        );
        assert_eq!(result.computed_state, "approved");
    }

    #[test]
    fn tv24_hit_pending_plus_clear() {
        let result = reduce(
            &load_entity_kyc_sm(),
            OverlayBuilder::new()
                .entity_ref("management-company")
                .workstream("COLLECT", None)
                .screening("SANCTIONS", "HIT_PENDING")
                .screening("PEP", "CLEAR")
                .build(),
            EvalScope::default(),
        );
        assert_eq!(result.computed_state, "workstream_open");
    }

    #[test]
    fn tv25_all_evidence_waived_not_yet_collected_in_current_machine() {
        let result = reduce(
            &load_entity_kyc_sm(),
            OverlayBuilder::new()
                .entity_ref("management-company")
                .workstream("COLLECT", None)
                .screening("SANCTIONS", "CLEAR")
                .evidence("SHARE_REGISTER", "WAIVED")
                .build(),
            EvalScope::default(),
        );
        assert_eq!(result.computed_state, "screening_complete");
    }

    #[test]
    fn trace_captures_reducer_revision_stability() {
        let sm1 = load_entity_kyc_sm();
        let sm2 = load_entity_kyc_sm();
        assert_eq!(sm1.reducer_revision, sm2.reducer_revision);
        assert_eq!(sm1.reducer_revision.len(), 16);
    }

    #[test]
    fn tv19_all_entities_verified_cross_slot() {
        let sm = load_case_sm();
        let overlays = SlotOverlayData {
            sources: HashMap::new(),
            scope: EvalScope::default().as_scope_data(),
            slots: vec![
                SlotRecord {
                    slot_type: "entity".into(),
                    cardinality: "mandatory".into(),
                    effective_state: "verified".into(),
                    computed_state: "verified".into(),
                },
                SlotRecord {
                    slot_type: "entity".into(),
                    cardinality: "mandatory".into(),
                    effective_state: "approved".into(),
                    computed_state: "approved".into(),
                },
            ],
        };
        let result = reduce_slot(&sm, "case.root", &overlays, None).unwrap();
        assert_eq!(result.computed_state, "ready");
    }

    #[test]
    fn tv20_partial_verification_cross_slot() {
        let sm = load_case_sm();
        let overlays = SlotOverlayData {
            sources: HashMap::new(),
            scope: EvalScope::default().as_scope_data(),
            slots: vec![
                SlotRecord {
                    slot_type: "entity".into(),
                    cardinality: "mandatory".into(),
                    effective_state: "verified".into(),
                    computed_state: "verified".into(),
                },
                SlotRecord {
                    slot_type: "entity".into(),
                    cardinality: "mandatory".into(),
                    effective_state: "workstream_open".into(),
                    computed_state: "workstream_open".into(),
                },
            ],
        };
        let result = reduce_slot(&sm, "case.root", &overlays, None).unwrap();
        assert_eq!(result.computed_state, "blocked");
    }
}
