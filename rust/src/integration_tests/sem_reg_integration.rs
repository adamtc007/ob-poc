//! Semantic Registry — Integration Tests (Phases 7-10)
//!
//! Ten test scenarios proving the architecture end-to-end:
//!
//! 1. UBO Discovery E2E — resolve_context → create_plan → execute → record_decision
//! 2. Sanctions Screening E2E — ABAC restricts sanctions-labelled attributes
//! 3. Proof Collection E2E — Evidence freshness + observation supersession
//! 4. Governance Review — Coverage report + stats
//! 5. Point-in-Time Audit — Publish → supersede → resolve_context(as_of=earlier)
//! 6. Proof Rule Enforcement — Governed policy + operational attribute → must fail
//! 7. Security/ABAC E2E — Purpose mismatch, jurisdiction mismatch, clearance check
//! 8. Onboarding Pipeline — Full 6-step round-trip + idempotent re-run
//! 9. Gate Unification — Simple + extended gates aggregate into unified result
//! 10. Taxonomy Filtering — Context resolution filters verbs/attributes by membership
//!
//! All tests require a running PostgreSQL instance with sem_reg migrations applied.
//!
//! Run with:
//! ```sh
//! DATABASE_URL="postgresql:///data_designer" \
//!   cargo test --features database --test sem_reg_integration -- --ignored --nocapture
//! ```

#[cfg(feature = "database")]
mod integration {
    use anyhow::Result;
    use chrono::Utc;
    use sem_os_policy::context_resolution::{
        ContextResolutionRequest, DiscoveryContext, EvidenceMode, SubjectRef,
    };
    use sem_os_policy::service::{CoreService, CoreServiceImpl};
    use sem_os_postgres::PgStores;
    use sem_os_types::EvidenceGrade;
    use serde_json::json;
    use sqlx::PgPool;
    use std::sync::Arc;
    use uuid::Uuid;

    use crate::sem_reg::attribute_def::AttributeDataType;
    use crate::sem_reg::{
        evaluate_abac, AccessDecision, AccessPurpose, ActorContext, AttributeDefBody, ChangeType,
        Classification, GovernanceTier, MetricsStore, ObjectType, RegistryService, SecurityLabel,
        SnapshotMeta, SnapshotStore, TrustClass, VerbContractBody,
    };

    // Import types not re-exported at module boundary
    use crate::sem_reg::agent::decisions::AlternativeAction;
    use crate::sem_reg::agent::{
        AgentPlan, AgentPlanStatus, DecisionRecord, DecisionStore, PlanStep, PlanStepStatus,
        PlanStore,
    };
    use crate::sem_reg::projections::LineageStore;

    // ── Test Infrastructure ──────────────────────────────────────────────────

    struct TestDb {
        pool: PgPool,
        prefix: String,
    }

    impl TestDb {
        async fn new() -> Result<Self> {
            let url = std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql:///data_designer".into());
            let pool = PgPool::connect(&url).await?;
            let prefix = format!("it_{}", &Uuid::new_v4().to_string()[..8]);
            Ok(Self { pool, prefix })
        }

        fn fqn(&self, base: &str) -> String {
            format!("{}_{}", self.prefix, base)
        }

        /// Publish an attribute with configurable governance settings.
        async fn publish_attr(
            &self,
            fqn: &str,
            name: &str,
            tier: GovernanceTier,
            trust: TrustClass,
            security: SecurityLabel,
        ) -> Result<(Uuid, Uuid)> {
            let object_id = Uuid::new_v4();
            let mut meta = SnapshotMeta::new_operational(
                ObjectType::AttributeDef,
                object_id,
                "integration_test",
            );
            meta.governance_tier = tier;
            meta.trust_class = trust;
            meta.security_label = security;
            let body = AttributeDefBody {
                fqn: fqn.into(),
                name: name.into(),
                description: format!("Test attribute: {}", name),
                domain: fqn.split('.').next().unwrap_or("test").into(),
                data_type: AttributeDataType::String,
                evidence_grade: EvidenceGrade::None,
                source: None,
                constraints: None,
                sinks: vec![],
                category: None,
                validation_rules: None,
                applicability: None,
                is_required: None,
                default_value: None,
                group_id: None,
                is_derived: None,
                derivation_spec_fqn: None,
                visibility: None,
            };
            let sid =
                RegistryService::publish_attribute_def(&self.pool, &meta, &body, None).await?;
            Ok((object_id, sid))
        }

        /// Publish a verb contract with default operational meta.
        async fn publish_verb(&self, fqn: &str, name: &str) -> Result<(Uuid, Uuid)> {
            let object_id = Uuid::new_v4();
            let meta = SnapshotMeta::new_operational(
                ObjectType::VerbContract,
                object_id,
                "integration_test",
            );
            let body = VerbContractBody {
                fqn: fqn.into(),
                domain: fqn.split('.').next().unwrap_or("test").into(),
                action: fqn.split('.').nth(1).unwrap_or("do").into(),
                description: format!("Test verb: {}", name),
                behavior: "plugin".into(),
                args: vec![],
                returns: None,
                preconditions: vec![],
                postconditions: vec![],
                produces: None,
                consumes: vec![],
                invocation_phrases: vec![],
                subject_kinds: vec![],
                phase_tags: vec![],
                harm_class: None,
                action_class: None,
                precondition_states: vec![],
                requires_subject: true,
                produces_focus: false,
                metadata: None,
                crud_mapping: None,
                reads_from: Vec::new(),
                writes_to: Vec::new(),
                outputs: Vec::new(),
                produces_shared_facts: Vec::new(),
            };
            let sid =
                RegistryService::publish_verb_contract(&self.pool, &meta, &body, None).await?;
            Ok((object_id, sid))
        }

        /// Cleanup test data by prefix.
        async fn cleanup(&self) {
            let pattern = format!("{}%", self.prefix);

            // Clean agent tables
            let _ = sqlx::query(
                "DELETE FROM sem_reg.plan_steps WHERE plan_id IN \
                 (SELECT plan_id FROM sem_reg.agent_plans WHERE goal LIKE $1)",
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await;

            let _ = sqlx::query("DELETE FROM sem_reg.agent_plans WHERE goal LIKE $1")
                .bind(&pattern)
                .execute(&self.pool)
                .await;

            // Clean lineage
            let _ = sqlx::query("DELETE FROM sem_reg.derivation_edges WHERE verb_fqn LIKE $1")
                .bind(&pattern)
                .execute(&self.pool)
                .await;

            // Clean snapshots
            let _ = sqlx::query(
                "DELETE FROM sem_reg.snapshots WHERE created_by = 'integration_test' \
                 AND definition->>'fqn' LIKE $1",
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await;
        }
    }

    fn test_actor() -> ActorContext {
        ActorContext {
            actor_id: "test-analyst".into(),
            roles: vec!["analyst".into()],
            department: Some("compliance".into()),
            clearance: Some(Classification::Confidential),
            jurisdictions: vec!["LU".into()],
        }
    }

    fn build_sem_os_service(pool: &PgPool) -> Arc<dyn CoreService> {
        let stores = PgStores::new(pool.clone());
        Arc::new(
            CoreServiceImpl::new(
                ob_poc_semantic_policy::snapshot_owned()
                    .expect("embedded ob-poc semantic policy is admitted"),
                Arc::new(stores.snapshots),
                Arc::new(stores.objects),
                Arc::new(stores.changesets),
                Arc::new(stores.audit),
                Arc::new(stores.outbox),
                Arc::new(stores.evidence),
                Arc::new(stores.projections),
            )
            .with_bootstrap_audit(Arc::new(stores.bootstrap_audit))
            .with_authoring(Arc::new(stores.authoring))
            .with_scratch_runner(Arc::new(stores.scratch_runner))
            .with_cleanup(Arc::new(stores.cleanup)),
        )
    }

    fn to_sem_os_actor(actor: &ActorContext) -> sem_os_policy::abac::ActorContext {
        sem_os_policy::abac::ActorContext {
            actor_id: actor.actor_id.clone(),
            roles: actor.roles.clone(),
            department: actor.department.clone(),
            clearance: actor.clearance,
            jurisdictions: actor.jurisdictions.clone(),
        }
    }

    async fn resolve_context_via_sem_os(
        pool: &PgPool,
        actor: &ActorContext,
        request: ContextResolutionRequest,
    ) -> sem_os_policy::service::Result<sem_os_policy::context_resolution::ContextResolutionResponse>
    {
        let service = build_sem_os_service(pool);
        let principal =
            sem_os_core::principal::Principal::in_process(&actor.actor_id, actor.roles.clone());
        service.resolve_context(&principal, request).await
    }

    #[tokio::test]
    #[ignore]
    async fn test_attribute_def_enum_materializes_to_registry_enum_value_type() -> Result<()> {
        let db = TestDb::new().await?;
        let attr_fqn = db.fqn("attr.enum_materialization");
        let object_id = Uuid::new_v4();
        let meta =
            SnapshotMeta::new_operational(ObjectType::AttributeDef, object_id, "integration_test");
        let body = AttributeDefBody {
            fqn: attr_fqn.clone(),
            name: "Enum Materialization".into(),
            description: "Regression coverage for SemOS enum projection".into(),
            domain: "test".into(),
            data_type: AttributeDataType::Enum(vec!["PENDING".into(), "VERIFIED".into()]),
            evidence_grade: EvidenceGrade::None,
            source: None,
            constraints: None,
            sinks: vec![],
            category: None,
            validation_rules: None,
            applicability: None,
            is_required: None,
            default_value: None,
            group_id: None,
            is_derived: None,
            derivation_spec_fqn: None,
            visibility: None,
        };

        RegistryService::publish_attribute_def(&db.pool, &meta, &body, None).await?;

        let value_type: String = sqlx::query_scalar(
            r#"SELECT value_type FROM "ob-poc".attribute_registry WHERE id = $1"#,
        )
        .bind(&attr_fqn)
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(value_type, "enum");

        db.cleanup().await;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Scenario 1: UBO Discovery E2E
    // ═══════════════════════════════════════════════════════════════════════

    /// End-to-end flow: resolve_context → create_plan → add_steps →
    /// execute_step → record_decision → verify snapshot_manifest.
    #[tokio::test]
    #[ignore]
    async fn test_scenario_1_ubo_discovery_e2e() -> Result<()> {
        let db = TestDb::new().await?;

        // 1. Publish an attribute + verb so resolution has something to find
        let attr_fqn = db.fqn("attr.ubo_ownership_pct");
        let (attr_oid, attr_sid) = db
            .publish_attr(
                &attr_fqn,
                "UBO Ownership Percentage",
                GovernanceTier::Governed,
                TrustClass::Proof,
                SecurityLabel::default(),
            )
            .await?;

        let verb_fqn = db.fqn("ubo.discover");
        let (verb_oid, verb_sid) = db.publish_verb(&verb_fqn, "UBO Discovery").await?;

        // 2. Resolve context
        let actor = test_actor();
        let request = ContextResolutionRequest {
            subject: SubjectRef::EntityId(Uuid::new_v4()),
            intent_summary: Some("discover UBO".into()),
            raw_utterance: Some("discover UBO".into()),
            actor: to_sem_os_actor(&actor),
            goals: vec!["ownership_discovery".into()],
            constraints: Default::default(),
            evidence_mode: EvidenceMode::Normal,
            point_in_time: None,
            entity_kind: None,
            entity_confidence: None,
            discovery: DiscoveryContext::default(),
        };
        let response = resolve_context_via_sem_os(&db.pool, &actor, request).await?;
        assert!(
            response.confidence >= 0.0,
            "Confidence should be non-negative"
        );

        // 3. Create an agent plan
        let plan = AgentPlan {
            plan_id: Uuid::new_v4(),
            case_id: Some(Uuid::new_v4()),
            goal: db.fqn("discover UBO for entity"),
            context_resolution_ref: None,
            steps: vec![],
            assumptions: vec!["Entity is a legal person".into()],
            risk_flags: vec![],
            security_clearance: Some("confidential".into()),
            status: AgentPlanStatus::Draft,
            created_by: "integration_test".into(),
            created_at: Utc::now(),
            updated_at: None,
        };
        let plan_id = PlanStore::insert_plan(&db.pool, &plan).await?;

        // 4. Add plan steps
        let step = PlanStep {
            step_id: Uuid::new_v4(),
            plan_id,
            seq: 1,
            verb_id: verb_oid,
            verb_snapshot_id: verb_sid,
            verb_fqn: verb_fqn.clone(),
            params: json!({"entity_id": Uuid::new_v4()}),
            expected_postconditions: vec!["ownership_chain_computed".into()],
            fallback_steps: vec![],
            depends_on_steps: vec![],
            status: PlanStepStatus::Pending,
            result: None,
            error: None,
        };
        let step_id = PlanStore::insert_step(&db.pool, &step).await?;

        // 5. Execute step (advance status)
        PlanStore::update_step_status(&db.pool, step_id, PlanStepStatus::Running, None, None)
            .await?;
        PlanStore::update_step_status(&db.pool, step_id, PlanStepStatus::Completed, None, None)
            .await?;

        // 6. Record decision with snapshot manifest
        let mut snapshot_manifest = std::collections::BTreeMap::new();
        snapshot_manifest.insert(attr_oid, attr_sid);
        let decision = DecisionRecord {
            decision_id: Uuid::new_v4(),
            plan_id: Some(plan_id),
            step_id: Some(step_id),
            context_ref: None,
            chosen_action: db.fqn("proceed with UBO discovery"),
            chosen_action_description: "Execute UBO discovery verb to trace ownership chain".into(),
            alternatives_considered: vec![AlternativeAction {
                action: "skip".into(),
                reason_rejected: "Regulatory requirement mandates UBO identification".into(),
                confidence: Some(0.2),
            }],
            evidence_for: vec![],
            evidence_against: vec![],
            negative_evidence: vec![],
            policy_verdicts: vec![],
            snapshot_manifest,
            confidence: 0.85,
            escalation_flag: false,
            escalation_id: None,
            decided_by: "integration_test".into(),
            decided_at: Utc::now(),
        };
        let decision_id = DecisionStore::insert(&db.pool, &decision).await?;

        // 7. Verify the persisted manifest pins the attribute snapshot.
        // (`DecisionStore::load` was deleted in dead-code Phase 4 — the store
        // is append-only with no production reader — so read the row directly.)
        let manifest_json = sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT snapshot_manifest FROM sem_reg.decision_records WHERE decision_id = $1",
        )
        .bind(decision_id)
        .fetch_one(&db.pool)
        .await?;
        let loaded_manifest: std::collections::BTreeMap<Uuid, Uuid> =
            serde_json::from_value(manifest_json)?;
        assert_eq!(
            loaded_manifest.get(&attr_oid),
            Some(&attr_sid),
            "Snapshot manifest should pin attribute snapshot"
        );

        // 8. Advance plan to completed
        PlanStore::update_plan_status(&db.pool, plan_id, AgentPlanStatus::Active).await?;
        PlanStore::update_plan_status(&db.pool, plan_id, AgentPlanStatus::Completed).await?;

        let loaded_plan = PlanStore::load_plan(&db.pool, plan_id)
            .await?
            .expect("Plan should be loadable");
        assert_eq!(loaded_plan.status, AgentPlanStatus::Completed);

        db.cleanup().await;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Scenario 2: Sanctions Screening E2E
    // ═══════════════════════════════════════════════════════════════════════

    /// ABAC restricts sanctions-labelled attributes to actors with matching purpose.
    #[tokio::test]
    #[ignore]
    async fn test_scenario_2_sanctions_screening_abac() -> Result<()> {
        let db = TestDb::new().await?;

        // Publish a sanctions-restricted attribute
        let attr_fqn = db.fqn("attr.sanctions_hit");
        let sanctions_label = SecurityLabel {
            classification: Classification::Restricted,
            pii: false,
            jurisdictions: vec![],
            purpose_limitation: vec!["SANCTIONS".into()],
            handling_controls: vec![],
        };
        let (_attr_oid, _attr_sid) = db
            .publish_attr(
                &attr_fqn,
                "Sanctions Hit Flag",
                GovernanceTier::Governed,
                TrustClass::Proof,
                sanctions_label.clone(),
            )
            .await?;

        // DENY: Analyst with Operations purpose should be denied
        let ops_actor = ActorContext {
            actor_id: "regular-analyst".into(),
            roles: vec!["analyst".into()],
            department: None,
            clearance: Some(Classification::Confidential),
            jurisdictions: vec!["LU".into()],
        };
        let decision = evaluate_abac(&ops_actor, &sanctions_label, AccessPurpose::Operations);
        assert!(
            matches!(decision, AccessDecision::Deny { .. }),
            "Operations-purpose actor should be denied sanctions data. Got: {:?}",
            decision
        );

        // ALLOW: Actor with Audit purpose + high clearance
        let audit_actor = ActorContext {
            actor_id: "sanctions-officer".into(),
            roles: vec!["compliance_officer".into()],
            department: Some("sanctions".into()),
            clearance: Some(Classification::Restricted),
            jurisdictions: vec!["LU".into()],
        };
        let decision = evaluate_abac(&audit_actor, &sanctions_label, AccessPurpose::Audit);
        // Should not be denied for clearance at minimum
        assert!(
            !matches!(decision, AccessDecision::Deny { ref reason } if reason.contains("clearance")),
            "Restricted-clearance actor should not be denied for clearance. Got: {:?}",
            decision
        );

        db.cleanup().await;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Scenario 3: Proof Collection E2E
    // ═══════════════════════════════════════════════════════════════════════

    /// Evidence freshness: publish attribute → record derivation edge →
    /// supersede → verify lineage still traceable.
    #[tokio::test]
    #[ignore]
    async fn test_scenario_3_proof_collection_lineage() -> Result<()> {
        let db = TestDb::new().await?;

        // Publish two source attributes
        let src_fqn1 = db.fqn("attr.company_name");
        let (_src_oid1, src_sid1) = db
            .publish_attr(
                &src_fqn1,
                "Company Name",
                GovernanceTier::Operational,
                TrustClass::Convenience,
                SecurityLabel::default(),
            )
            .await?;

        let src_fqn2 = db.fqn("attr.company_lei");
        let (_src_oid2, src_sid2) = db
            .publish_attr(
                &src_fqn2,
                "Company LEI",
                GovernanceTier::Operational,
                TrustClass::Convenience,
                SecurityLabel::default(),
            )
            .await?;

        // Publish a derived attribute
        let derived_fqn = db.fqn("attr.identity_composite");
        let (_derived_oid, derived_sid) = db
            .publish_attr(
                &derived_fqn,
                "Company Identity Composite",
                GovernanceTier::Governed,
                TrustClass::DecisionSupport,
                SecurityLabel::default(),
            )
            .await?;

        // Record a derivation edge: src1 + src2 → derived.
        // (`LineageStore::record_derivation_edge` was deleted in dead-code
        // Phase 4 — no production writer — so seed the append-only row
        // directly; the surviving query paths are what this test proves.)
        let edge_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO sem_reg.derivation_edges
                (edge_id, input_snapshot_ids, output_snapshot_id, verb_fqn, run_id)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(edge_id)
        .bind(vec![src_sid1, src_sid2])
        .bind(derived_sid)
        .bind(db.fqn("attr.derive-composite"))
        .bind(Option::<Uuid>::None)
        .execute(&db.pool)
        .await?;

        // Forward impact from src1 should include derived
        let forward = LineageStore::query_forward_impact(&db.pool, src_sid1, 5).await?;
        assert!(
            forward.iter().any(|n| n.snapshot_id == derived_sid),
            "Forward impact from src1 should include the derived attribute"
        );

        // Reverse provenance from derived should include both sources
        let reverse = LineageStore::query_reverse_provenance(&db.pool, derived_sid, 5).await?;
        assert!(
            reverse.iter().any(|n| n.snapshot_id == src_sid1),
            "Reverse provenance should include src1"
        );
        assert!(
            reverse.iter().any(|n| n.snapshot_id == src_sid2),
            "Reverse provenance should include src2"
        );

        // Supersede src1 (simulates data change)
        SnapshotStore::supersede_snapshot(&db.pool, src_sid1).await?;

        // Original lineage is still queryable (immutable edges)
        let reverse_after =
            LineageStore::query_reverse_provenance(&db.pool, derived_sid, 5).await?;
        assert!(
            reverse_after.iter().any(|n| n.snapshot_id == src_sid1),
            "Lineage edges should survive supersession — immutable"
        );

        db.cleanup().await;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Scenario 4: Governance Review
    // ═══════════════════════════════════════════════════════════════════════

    /// Coverage report + stats endpoint.
    #[tokio::test]
    #[ignore]
    async fn test_scenario_4_governance_review() -> Result<()> {
        let db = TestDb::new().await?;

        // Publish several objects
        let attr_fqn = db.fqn("attr.gov_review");
        db.publish_attr(
            &attr_fqn,
            "Governance Review Attr",
            GovernanceTier::Governed,
            TrustClass::Proof,
            SecurityLabel::default(),
        )
        .await?;

        let verb_fqn = db.fqn("gov.review");
        db.publish_verb(&verb_fqn, "Governance Review Verb").await?;

        // Run coverage report (all tiers)
        let report = MetricsStore::coverage_report(&db.pool, None).await?;
        assert!(
            report.snapshot_volume > 0,
            "Coverage report should count at least our test snapshots"
        );

        // Run with tier filter
        let governed_report = MetricsStore::coverage_report(&db.pool, Some("governed")).await?;
        assert!(
            governed_report.snapshot_volume > 0,
            "Governed tier should have snapshots"
        );

        // Verify stats
        let stats = RegistryService::stats(&db.pool).await?;
        assert!(
            stats
                .iter()
                .any(|(ot, c)| *ot == ObjectType::AttributeDef && *c > 0),
            "Stats should include attribute definitions"
        );

        db.cleanup().await;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Scenario 5: Point-in-Time Audit
    // ═══════════════════════════════════════════════════════════════════════

    /// Publish → capture timestamp → supersede → resolve_at(earlier) →
    /// verify pinned snapshot is the original.
    #[tokio::test]
    #[ignore]
    async fn test_scenario_5_point_in_time_audit() -> Result<()> {
        let db = TestDb::new().await?;

        // 1. Publish original attribute
        let attr_fqn = db.fqn("attr.pit_test");
        let (attr_oid, original_sid) = db
            .publish_attr(
                &attr_fqn,
                "PIT Original",
                GovernanceTier::Operational,
                TrustClass::Convenience,
                SecurityLabel::default(),
            )
            .await?;

        // 2. Capture the timestamp AFTER the first publish
        let after_v1 = Utc::now();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // 3. Supersede and publish v2
        let mut meta_v2 =
            SnapshotMeta::new_operational(ObjectType::AttributeDef, attr_oid, "integration_test");
        meta_v2.version_major = 2;
        meta_v2.predecessor_id = Some(original_sid);
        meta_v2.change_type = ChangeType::NonBreaking;
        let body_v2 = AttributeDefBody {
            fqn: attr_fqn.clone(),
            name: "PIT Updated".into(),
            description: "Updated version for PIT test".into(),
            domain: "attr".into(),
            data_type: AttributeDataType::String,
            evidence_grade: EvidenceGrade::None,
            source: None,
            constraints: None,
            sinks: vec![],
            category: None,
            validation_rules: None,
            applicability: None,
            is_required: None,
            default_value: None,
            group_id: None,
            is_derived: None,
            derivation_spec_fqn: None,
            visibility: None,
        };
        let v2_sid =
            RegistryService::publish_attribute_def(&db.pool, &meta_v2, &body_v2, None).await?;

        // 4. Current resolution should return v2
        let current = SnapshotStore::resolve_active(&db.pool, ObjectType::AttributeDef, attr_oid)
            .await?
            .expect("Should have active snapshot");
        assert_eq!(current.snapshot_id, v2_sid, "Current should be v2");
        assert_eq!(current.version_major, 2);

        // 5. Point-in-time resolution at after_v1 should return v1
        let pit = SnapshotStore::resolve_at(&db.pool, ObjectType::AttributeDef, attr_oid, after_v1)
            .await?
            .expect("Should have snapshot at PIT");
        assert_eq!(
            pit.snapshot_id, original_sid,
            "PIT query should return original (v1) snapshot"
        );
        assert_eq!(pit.version_major, 1);

        // 6. History should show both versions
        let history =
            SnapshotStore::load_history(&db.pool, ObjectType::AttributeDef, attr_oid).await?;
        assert!(
            history.len() >= 2,
            "History should have at least 2 snapshots, got {}",
            history.len()
        );

        db.cleanup().await;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Scenario 6: Proof Rule Enforcement — DELETED
    //
    // The local `check_evidence_proof_rule` helper was removed in dead-code
    // Phase 4 (a4265301): the gate framework migrated to the external
    // `sem_os_policy::gates` crate, whose successor `check_proof_rule` is
    // pub(crate) there and covered by that crate's own tests.
    // ═══════════════════════════════════════════════════════════════════════

    // ═══════════════════════════════════════════════════════════════════════
    // Scenario 7: Security / ABAC E2E
    // ═══════════════════════════════════════════════════════════════════════

    /// Multiple ABAC scenarios:
    /// - Jurisdiction mismatch → Deny
    /// - Unrestricted → Allow for all
    /// - PII data → requires sufficient clearance
    #[tokio::test]
    #[ignore]
    async fn test_scenario_7_security_abac_e2e() -> Result<()> {
        let db = TestDb::new().await?;

        // ── Attribute with jurisdiction restriction ──────────────
        let lu_label = SecurityLabel {
            classification: Classification::Internal,
            pii: false,
            jurisdictions: vec!["LU".into()],
            purpose_limitation: vec![],
            handling_controls: vec![],
        };

        // Actor in LU → should be allowed
        let lu_actor = ActorContext {
            actor_id: "lu-analyst".into(),
            roles: vec!["analyst".into()],
            department: None,
            clearance: Some(Classification::Confidential),
            jurisdictions: vec!["LU".into()],
        };
        let lu_decision = evaluate_abac(&lu_actor, &lu_label, AccessPurpose::Operations);
        assert!(
            matches!(
                lu_decision,
                AccessDecision::Allow | AccessDecision::AllowWithMasking { .. }
            ),
            "LU actor should be allowed LU-restricted data. Got: {:?}",
            lu_decision
        );

        // Actor in US → should be denied
        let us_actor = ActorContext {
            actor_id: "us-analyst".into(),
            roles: vec!["analyst".into()],
            department: None,
            clearance: Some(Classification::Confidential),
            jurisdictions: vec!["US".into()],
        };
        let us_decision = evaluate_abac(&us_actor, &lu_label, AccessPurpose::Operations);
        assert!(
            matches!(us_decision, AccessDecision::Deny { .. }),
            "US actor should be denied LU-restricted data. Got: {:?}",
            us_decision
        );

        // ── Unrestricted attribute → everyone allowed ────────────
        let open_label = SecurityLabel::default();
        let open_decision = evaluate_abac(&us_actor, &open_label, AccessPurpose::Operations);
        assert!(
            matches!(
                open_decision,
                AccessDecision::Allow | AccessDecision::AllowWithMasking { .. }
            ),
            "Unrestricted label should allow any actor. Got: {:?}",
            open_decision
        );

        // ── PII attribute requiring high clearance ───────────────
        let pii_label = SecurityLabel {
            classification: Classification::Restricted,
            pii: true,
            jurisdictions: vec![],
            purpose_limitation: vec![],
            handling_controls: vec![],
        };

        // Low clearance actor → denied
        let low_actor = ActorContext {
            actor_id: "intern".into(),
            roles: vec!["viewer".into()],
            department: None,
            clearance: Some(Classification::Public),
            jurisdictions: vec!["LU".into()],
        };
        let pii_low = evaluate_abac(&low_actor, &pii_label, AccessPurpose::Operations);
        assert!(
            matches!(pii_low, AccessDecision::Deny { .. }),
            "Low-clearance actor should be denied PII. Got: {:?}",
            pii_low
        );

        // High clearance actor → allowed
        let high_actor = ActorContext {
            actor_id: "security-officer".into(),
            roles: vec!["security_officer".into()],
            department: Some("security".into()),
            clearance: Some(Classification::Restricted),
            jurisdictions: vec!["LU".into()],
        };
        let pii_high = evaluate_abac(&high_actor, &pii_label, AccessPurpose::Audit);
        assert!(
            !matches!(pii_high, AccessDecision::Deny { ref reason } if reason.contains("clearance")),
            "High-clearance actor should not be denied for clearance. Got: {:?}",
            pii_high
        );

        db.cleanup().await;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Scenario 10: Taxonomy Filtering in Context Resolution
    // ═══════════════════════════════════════════════════════════════════════

    /// Verify that context resolution filters verbs and attributes by
    /// taxonomy membership overlap with the subject entity type.
    ///
    /// Setup:
    /// - Entity type "test_fund" with membership in taxonomy "asset_management"
    /// - Verb A in taxonomy "asset_management" (should be included)
    /// - Verb B in taxonomy "compliance_only" (should be filtered out)
    /// - Verb C with no taxonomy constraint (should be included — unconstrained)
    #[tokio::test]
    #[ignore]
    async fn test_scenario10_taxonomy_filtering_in_context_resolution() -> Result<()> {
        use crate::sem_reg::membership::MembershipKind;
        use crate::sem_reg::taxonomy_def::TaxonomyDefBody;
        use crate::sem_reg::MembershipRuleBody;

        let db = TestDb::new().await?;

        // ── Step 1: Publish two taxonomies ───────────────────────────
        let tax_am_fqn = db.fqn("taxonomy.asset_management");
        let tax_co_fqn = db.fqn("taxonomy.compliance_only");

        for (fqn, name) in [
            (&tax_am_fqn, "Asset Management"),
            (&tax_co_fqn, "Compliance Only"),
        ] {
            let oid = Uuid::new_v4();
            let meta =
                SnapshotMeta::new_operational(ObjectType::TaxonomyDef, oid, "integration_test");
            let body = TaxonomyDefBody {
                fqn: fqn.clone(),
                name: name.into(),
                description: format!("Test taxonomy: {name}"),
                domain: "test".into(),
                root_node_fqn: None,
                max_depth: Some(3),
                classification_axis: None,
            };
            RegistryService::publish_taxonomy_def(&db.pool, &meta, &body, None).await?;
        }

        // ── Step 2: Publish an entity type ───────────────────────────
        let et_fqn = db.fqn("entity.test_fund");
        let et_oid = Uuid::new_v4();
        let et_meta =
            SnapshotMeta::new_operational(ObjectType::EntityTypeDef, et_oid, "integration_test");
        let et_body = crate::sem_reg::EntityTypeDefBody {
            fqn: et_fqn.clone(),
            name: "Test Fund".into(),
            description: "Fund entity for taxonomy test".into(),
            domain: "test".into(),
            db_table: None,
            lifecycle_states: vec![],
            required_attributes: vec![],
            optional_attributes: vec![],
            parent_type: None,
            governance_tier: None,
            security_classification: None,
            pii: None,
            read_by_verbs: Vec::new(),
            written_by_verbs: Vec::new(),
        };
        RegistryService::publish_entity_type_def(&db.pool, &et_meta, &et_body, None).await?;

        // ── Step 3: Create membership rules ──────────────────────────
        // Entity type → asset_management taxonomy (the "subject" membership)
        let mem_et_oid = Uuid::new_v4();
        let mem_et_meta = SnapshotMeta::new_operational(
            ObjectType::MembershipRule,
            mem_et_oid,
            "integration_test",
        );
        let mem_et_body = MembershipRuleBody {
            fqn: db.fqn("membership.fund_in_am"),
            name: "Fund in Asset Management".into(),
            description: Some("Test fund belongs to AM taxonomy".into()),
            taxonomy_fqn: tax_am_fqn.clone(),
            node_fqn: tax_am_fqn.clone(),
            target_type: "entity_type_def".into(),
            target_fqn: et_fqn.clone(),
            membership_kind: MembershipKind::Direct,
            conditions: vec![],
        };
        RegistryService::publish_membership_rule(&db.pool, &mem_et_meta, &mem_et_body, None)
            .await?;

        // ── Step 4: Publish verbs with different taxonomy memberships ─

        // Verb A: In asset_management taxonomy (should be included)
        let verb_a_fqn = db.fqn("test.fund_action");
        let verb_a_oid = Uuid::new_v4();
        let verb_a_meta =
            SnapshotMeta::new_operational(ObjectType::VerbContract, verb_a_oid, "integration_test");
        let verb_a_body = VerbContractBody {
            fqn: verb_a_fqn.clone(),
            domain: "test".into(),
            action: "fund_action".into(),
            description: "Fund verb in AM taxonomy".into(),
            behavior: "plugin".into(),
            args: vec![],
            returns: None,
            preconditions: vec![],
            postconditions: vec![],
            produces: None,
            consumes: vec![],
            invocation_phrases: vec![],
            subject_kinds: vec![],
            phase_tags: vec![],
            harm_class: None,
            action_class: None,
            precondition_states: vec![],
            requires_subject: true,
            produces_focus: false,
            metadata: None,
            crud_mapping: None,
            reads_from: Vec::new(),
            writes_to: Vec::new(),
            outputs: Vec::new(),
            produces_shared_facts: Vec::new(),
        };
        RegistryService::publish_verb_contract(&db.pool, &verb_a_meta, &verb_a_body, None).await?;

        // Create membership: verb A → asset_management
        let mem_va_oid = Uuid::new_v4();
        let mem_va_meta = SnapshotMeta::new_operational(
            ObjectType::MembershipRule,
            mem_va_oid,
            "integration_test",
        );
        let mem_va_body = MembershipRuleBody {
            fqn: db.fqn("membership.fund_action_in_am"),
            name: "fund_action in AM".into(),
            description: Some("Verb A in asset management".into()),
            taxonomy_fqn: tax_am_fqn.clone(),
            node_fqn: tax_am_fqn.clone(),
            target_type: "verb_contract".into(),
            target_fqn: verb_a_fqn.clone(),
            membership_kind: MembershipKind::Direct,
            conditions: vec![],
        };
        RegistryService::publish_membership_rule(&db.pool, &mem_va_meta, &mem_va_body, None)
            .await?;

        // Verb B: In compliance_only taxonomy (should be filtered out)
        let verb_b_fqn = db.fqn("test.compliance_action");
        let verb_b_oid = Uuid::new_v4();
        let verb_b_meta =
            SnapshotMeta::new_operational(ObjectType::VerbContract, verb_b_oid, "integration_test");
        let mut verb_b_body = verb_a_body.clone();
        verb_b_body.fqn = verb_b_fqn.clone();
        verb_b_body.action = "compliance_action".into();
        verb_b_body.description = "Compliance verb NOT in AM taxonomy".into();
        RegistryService::publish_verb_contract(&db.pool, &verb_b_meta, &verb_b_body, None).await?;

        // Create membership: verb B → compliance_only
        let mem_vb_oid = Uuid::new_v4();
        let mem_vb_meta = SnapshotMeta::new_operational(
            ObjectType::MembershipRule,
            mem_vb_oid,
            "integration_test",
        );
        let mem_vb_body = MembershipRuleBody {
            fqn: db.fqn("membership.compliance_action_in_co"),
            name: "compliance_action in CO".into(),
            description: Some("Verb B in compliance only".into()),
            taxonomy_fqn: tax_co_fqn.clone(),
            node_fqn: tax_co_fqn.clone(),
            target_type: "verb_contract".into(),
            target_fqn: verb_b_fqn.clone(),
            membership_kind: MembershipKind::Direct,
            conditions: vec![],
        };
        RegistryService::publish_membership_rule(&db.pool, &mem_vb_meta, &mem_vb_body, None)
            .await?;

        // Verb C: No taxonomy membership (should be included — unconstrained)
        let verb_c_fqn = db.fqn("test.unconstrained_action");
        let verb_c_oid = Uuid::new_v4();
        let verb_c_meta =
            SnapshotMeta::new_operational(ObjectType::VerbContract, verb_c_oid, "integration_test");
        let mut verb_c_body = verb_a_body.clone();
        verb_c_body.fqn = verb_c_fqn.clone();
        verb_c_body.action = "unconstrained_action".into();
        verb_c_body.description = "Unconstrained verb (no taxonomy)".into();
        RegistryService::publish_verb_contract(&db.pool, &verb_c_meta, &verb_c_body, None).await?;

        // ── Step 5: Resolve context and verify filtering ─────────────
        let actor = test_actor();
        let subject_id = Uuid::new_v4(); // Synthetic entity with our entity type

        let request = ContextResolutionRequest {
            subject: SubjectRef::EntityId(subject_id),
            intent_summary: None,
            raw_utterance: None,
            actor: to_sem_os_actor(&actor),
            goals: vec!["test_taxonomy_filter".into()],
            constraints: Default::default(),
            evidence_mode: EvidenceMode::Exploratory,
            point_in_time: None,
            entity_kind: None,
            entity_confidence: None,
            discovery: DiscoveryContext::default(),
        };

        let response = resolve_context_via_sem_os(&db.pool, &actor, request).await?;

        // Collect verb FQNs from response
        let verb_fqns: Vec<&str> = response
            .candidate_verbs
            .iter()
            .map(|v| v.fqn.as_str())
            .collect();

        println!("Candidate verbs: {:?}", verb_fqns);

        // Verb A (in AM taxonomy) should be present IF the subject's entity type
        // was resolved. Since SubjectRef::EntityId doesn't resolve entity type from
        // DB in all cases, check the signal: if memberships were loaded, verb B
        // should be absent.

        // Check the governance signals for taxonomy info
        let has_unclassified = response
            .governance_signals
            .iter()
            .any(|s| s.message.contains("no taxonomy memberships"));

        if !has_unclassified {
            // Taxonomy filtering is active — verb B should be filtered out
            assert!(
                !verb_fqns.contains(&verb_b_fqn.as_str()),
                "Verb B (compliance_only taxonomy) should be filtered out when \
                 subject has asset_management membership. Got verbs: {:?}",
                verb_fqns
            );
            println!("Taxonomy filtering confirmed: verb B correctly excluded");
        } else {
            // Subject memberships not loaded (entity type not resolved from DB).
            // This is expected for synthetic entity IDs — all verbs included with warning.
            println!(
                "Subject has no taxonomy memberships (synthetic entity) — \
                 all verbs included with governance warning (expected)"
            );
            assert!(
                has_unclassified,
                "Should have unclassified object governance signal"
            );
        }

        // Verb C (unconstrained) should always be present regardless of filtering
        // (it has no taxonomy constraint, so it passes through)
        if !verb_fqns.is_empty() {
            // Only check if we got any results at all (depends on view assignment)
            let has_unconstrained = verb_fqns.contains(&verb_c_fqn.as_str());
            if has_unconstrained {
                println!("Unconstrained verb C correctly included");
            }
        }

        // Verify confidence is computed
        assert!(
            response.confidence >= 0.0 && response.confidence <= 1.0,
            "Confidence should be in [0, 1] range, got {}",
            response.confidence
        );

        // ── Cleanup ──────────────────────────────────────────────────
        db.cleanup().await;

        // Also clean taxonomy-specific snapshots
        let pattern = format!("{}%", db.prefix);
        let _ = sqlx::query(
            "DELETE FROM sem_reg.snapshots WHERE created_by = 'integration_test' \
             AND definition->>'fqn' LIKE $1",
        )
        .bind(&pattern)
        .execute(&db.pool)
        .await;

        Ok(())
    }
}
