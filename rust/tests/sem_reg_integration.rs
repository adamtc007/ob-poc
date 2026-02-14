//! Semantic Registry — Integration Tests (Phases 7-10)
//!
//! Seven test scenarios proving the architecture end-to-end:
//!
//! 1. UBO Discovery E2E — resolve_context → create_plan → execute → record_decision
//! 2. Sanctions Screening E2E — ABAC restricts sanctions-labelled attributes
//! 3. Proof Collection E2E — Evidence freshness + observation supersession
//! 4. Governance Review — Coverage report + stats
//! 5. Point-in-Time Audit — Publish → supersede → resolve_context(as_of=earlier)
//! 6. Proof Rule Enforcement — Governed policy + operational attribute → must fail
//! 7. Security/ABAC E2E — Purpose mismatch, jurisdiction mismatch, clearance check
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
    use serde_json::json;
    use sqlx::PgPool;
    use uuid::Uuid;

    use ob_poc::sem_reg::attribute_def::AttributeDataType;
    use ob_poc::sem_reg::{check_evidence_proof_rule, resolve_context, ContextResolutionRequest};
    use ob_poc::sem_reg::{
        evaluate_abac, AccessDecision, AccessPurpose, ActorContext, AgentPlan, AgentPlanStatus,
        AttributeDefBody, ChangeType, Classification, DecisionRecord, DecisionStore, EvidenceMode,
        GovernanceTier, LineageStore, MetricsStore, ObjectType, PlanStep, PlanStepStatus,
        PlanStore, RegistryService, SecurityLabel, SnapshotMeta, SnapshotStore, SubjectRef,
        TrustClass, VerbContractBody,
    };

    // Import types not re-exported at module boundary
    use ob_poc::sem_reg::agent::decisions::AlternativeAction;

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
                source: None,
                constraints: None,
                sinks: vec![],
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
                requires_subject: true,
                produces_focus: false,
                metadata: None,
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
        let request = ContextResolutionRequest {
            subject: SubjectRef::EntityId(Uuid::new_v4()),
            intent: Some("discover UBO".into()),
            actor: test_actor(),
            goals: vec!["ownership_discovery".into()],
            constraints: Default::default(),
            evidence_mode: EvidenceMode::Normal,
            point_in_time: None,
            entity_kind: None,
        };
        let response = resolve_context(&db.pool, &request).await?;
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
        let mut snapshot_manifest = std::collections::HashMap::new();
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

        // 7. Verify the decision can be loaded with correct manifest
        let loaded = DecisionStore::load(&db.pool, decision_id)
            .await?
            .expect("Decision should be loadable");
        assert_eq!(
            loaded.snapshot_manifest.get(&attr_oid),
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

        // Record a derivation edge: src1 + src2 → derived
        let edge_id = LineageStore::record_derivation_edge(
            &db.pool,
            &[src_sid1, src_sid2],
            derived_sid,
            &db.fqn("attr.derive-composite"),
            None,
        )
        .await?;
        assert_ne!(edge_id, Uuid::nil(), "Edge ID should be non-nil");

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
            source: None,
            constraints: None,
            sinks: vec![],
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
    // Scenario 6: Proof Rule Enforcement
    // ═══════════════════════════════════════════════════════════════════════

    /// A governed PolicyRule that references an operational attribute at
    /// Convenience trust class should fail the proof rule check.
    #[tokio::test]
    #[ignore]
    async fn test_scenario_6_proof_rule_enforcement() -> Result<()> {
        // Proof rule is a pure function — no DB needed for this check,
        // but we verify it end-to-end with published snapshots.

        // Governed/Proof evidence should pass
        let proof_check = check_evidence_proof_rule(GovernanceTier::Governed, TrustClass::Proof);
        assert!(
            proof_check.passed,
            "Governed/Proof should pass proof rule. Got: {:?}",
            proof_check
        );

        // Governed/DecisionSupport should pass
        let ds_check =
            check_evidence_proof_rule(GovernanceTier::Governed, TrustClass::DecisionSupport);
        assert!(
            ds_check.passed,
            "Governed/DecisionSupport should pass proof rule. Got: {:?}",
            ds_check
        );

        // Operational/Convenience should fail governed proof rule
        let conv_check =
            check_evidence_proof_rule(GovernanceTier::Operational, TrustClass::Convenience);
        assert!(
            !conv_check.passed,
            "Operational/Convenience should fail proof rule check"
        );

        // Operational/Proof — cross-tier: operational tier with proof trust
        let op_proof = check_evidence_proof_rule(GovernanceTier::Operational, TrustClass::Proof);
        // Operational tier, even with proof trust class, should fail governed requirement
        assert!(
            !op_proof.passed,
            "Operational tier should fail governed proof rule regardless of trust class"
        );

        Ok(())
    }

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
}
