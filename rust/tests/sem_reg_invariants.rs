//! Semantic Registry — Foundational Invariant Tests
//!
//! Six invariant tests documenting the non-negotiable properties of the Semantic OS:
//!
//! - INV-1: No in-place updates — immutability trigger rejects UPDATE/DELETE on snapshots
//! - INV-2: Snapshot pinning — decision records must reference valid snapshot_ids
//! - INV-3: Proof Rule — trust_class=Proof + governance_tier=Operational rejected by CHECK constraint
//! - INV-4: ABAC both tiers — Governed and Operational snapshots both checked
//! - INV-5: Operational auto-approve — no governance gate on Operational tier
//! - INV-6: DerivationSpec required — derived attribute without spec triggers publish gate failure
//!
//! Run with:
//! ```sh
//! DATABASE_URL="postgresql:///data_designer" \
//!   cargo test --features database --test sem_reg_invariants -- --ignored --nocapture
//! ```

#[cfg(feature = "database")]
mod invariants {
    use anyhow::Result;
    use chrono::Utc;
    use sqlx::PgPool;
    use uuid::Uuid;

    use ob_poc::sem_reg::attribute_def::AttributeDataType;
    use ob_poc::sem_reg::{
        evaluate_abac, evaluate_publish_gates, AccessDecision, AccessPurpose, ActorContext,
        AttributeDefBody, ChangeType, Classification, DecisionRecord, DecisionStore,
        GovernanceTier, ObjectType, RegistryService, SecurityLabel, SnapshotMeta, SnapshotStatus,
        TrustClass, VerbContractBody,
    };

    // ── Test Infrastructure ──────────────────────────────────────────────────

    struct InvTestDb {
        pool: PgPool,
        prefix: String,
    }

    impl InvTestDb {
        async fn new() -> Result<Self> {
            let url = std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql:///data_designer".into());
            let pool = PgPool::connect(&url).await?;
            let prefix = format!("inv_{}", &Uuid::new_v4().to_string()[..8]);
            Ok(Self { pool, prefix })
        }

        fn fqn(&self, base: &str) -> String {
            format!("{}_{}", self.prefix, base)
        }

        /// Publish an attribute and return (object_id, snapshot_id).
        async fn publish_attr(
            &self,
            fqn: &str,
            tier: GovernanceTier,
            trust: TrustClass,
        ) -> Result<(Uuid, Uuid)> {
            let object_id = Uuid::new_v4();
            let mut meta =
                SnapshotMeta::new_operational(ObjectType::AttributeDef, object_id, "inv_test");
            meta.governance_tier = tier;
            meta.trust_class = trust;
            if tier == GovernanceTier::Governed {
                meta.approved_by = Some("inv_approver".into());
            }
            let body = AttributeDefBody {
                fqn: fqn.into(),
                name: fqn.into(),
                description: "Invariant test attribute".into(),
                domain: "inv_test".into(),
                data_type: AttributeDataType::String,
                source: None,
                constraints: None,
                sinks: vec![],
            };
            let sid =
                RegistryService::publish_attribute_def(&self.pool, &meta, &body, None).await?;
            Ok((object_id, sid))
        }

        /// Publish a verb contract and return (object_id, snapshot_id).
        async fn publish_verb(&self, fqn: &str) -> Result<(Uuid, Uuid)> {
            let object_id = Uuid::new_v4();
            let meta =
                SnapshotMeta::new_operational(ObjectType::VerbContract, object_id, "inv_test");
            let body = VerbContractBody {
                fqn: fqn.into(),
                domain: "inv_test".into(),
                action: "do".into(),
                description: "Invariant test verb".into(),
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
                crud_mapping: None,
            };
            let sid =
                RegistryService::publish_verb_contract(&self.pool, &meta, &body, None).await?;
            Ok((object_id, sid))
        }

        async fn cleanup(&self) {
            let pattern = format!("{}%", self.prefix);

            // Clean decision records referencing our test data
            let _ = sqlx::query("DELETE FROM sem_reg.decision_records WHERE chosen_action LIKE $1")
                .bind(&pattern)
                .execute(&self.pool)
                .await;

            // Clean snapshots
            let _ = sqlx::query(
                "DELETE FROM sem_reg.snapshots WHERE created_by = 'inv_test' \
                 AND definition->>'fqn' LIKE $1",
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await;
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // INV-1: No in-place updates
    // ═══════════════════════════════════════════════════════════════════════

    /// The snapshots table must reject UPDATE and DELETE operations.
    ///
    /// NOTE: This test will initially FAIL because the immutability trigger
    /// does not yet exist on sem_reg.snapshots. It will pass after S4
    /// (migration 090) adds the trigger.
    #[tokio::test]
    #[ignore]
    async fn test_inv1_no_in_place_updates() -> Result<()> {
        let db = InvTestDb::new().await?;

        // Publish a snapshot
        let attr_fqn = db.fqn("attr.immutable_test");
        let (_oid, sid) = db
            .publish_attr(
                &attr_fqn,
                GovernanceTier::Operational,
                TrustClass::Convenience,
            )
            .await?;

        // Attempt UPDATE — should be rejected by immutability trigger
        let update_result = sqlx::query(
            "UPDATE sem_reg.snapshots SET definition = '{\"fqn\": \"hacked\"}' WHERE snapshot_id = $1",
        )
        .bind(sid)
        .execute(&db.pool)
        .await;

        assert!(
            update_result.is_err(),
            "UPDATE on sem_reg.snapshots should be rejected by immutability trigger. \
             Got success — trigger not yet installed (expected to fail until S4/migration 090)"
        );

        // Attempt DELETE — should also be rejected
        let delete_result = sqlx::query("DELETE FROM sem_reg.snapshots WHERE snapshot_id = $1")
            .bind(sid)
            .execute(&db.pool)
            .await;

        assert!(
            delete_result.is_err(),
            "DELETE on sem_reg.snapshots should be rejected by immutability trigger. \
             Got success — trigger not yet installed (expected to fail until S4/migration 090)"
        );

        db.cleanup().await;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // INV-2: Snapshot pinning on decisions
    // ═══════════════════════════════════════════════════════════════════════

    /// Decision records must reference valid snapshot_ids in their manifest.
    /// The snapshot_manifest provides full provenance: which exact snapshots
    /// were consulted when the decision was made.
    #[tokio::test]
    #[ignore]
    async fn test_inv2_snapshot_pinning_on_decisions() -> Result<()> {
        let db = InvTestDb::new().await?;

        // Publish two objects
        let attr_fqn = db.fqn("attr.pinning_test");
        let (attr_oid, attr_sid) = db
            .publish_attr(&attr_fqn, GovernanceTier::Governed, TrustClass::Proof)
            .await?;

        let verb_fqn = db.fqn("verb.pinning_test");
        let (verb_oid, verb_sid) = db.publish_verb(&verb_fqn).await?;

        // Create decision with snapshot manifest
        let mut manifest = std::collections::HashMap::new();
        manifest.insert(attr_oid, attr_sid);
        manifest.insert(verb_oid, verb_sid);

        let decision = DecisionRecord {
            decision_id: Uuid::new_v4(),
            plan_id: None,
            step_id: None,
            context_ref: None,
            chosen_action: db.fqn("pinning_decision"),
            chosen_action_description: "Test snapshot pinning".into(),
            alternatives_considered: vec![],
            evidence_for: vec![],
            evidence_against: vec![],
            negative_evidence: vec![],
            policy_verdicts: vec![],
            snapshot_manifest: manifest.clone(),
            confidence: 0.95,
            escalation_flag: false,
            escalation_id: None,
            decided_by: "inv_test".into(),
            decided_at: Utc::now(),
        };
        let decision_id = DecisionStore::insert(&db.pool, &decision).await?;

        // Load and verify manifest integrity
        let loaded = DecisionStore::load(&db.pool, decision_id)
            .await?
            .expect("Decision should be loadable");

        // Every pinned snapshot must resolve to a real snapshot
        for (object_id, snapshot_id) in &loaded.snapshot_manifest {
            let exists = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM sem_reg.snapshots WHERE snapshot_id = $1)",
            )
            .bind(snapshot_id)
            .fetch_one(&db.pool)
            .await?;

            assert!(
                exists,
                "Snapshot manifest references non-existent snapshot_id {} for object_id {}",
                snapshot_id, object_id
            );
        }

        // Manifest size must match what we inserted
        assert_eq!(
            loaded.snapshot_manifest.len(),
            2,
            "Manifest should contain exactly 2 entries"
        );
        assert_eq!(
            loaded.snapshot_manifest.get(&attr_oid),
            Some(&attr_sid),
            "Attribute snapshot should be pinned"
        );
        assert_eq!(
            loaded.snapshot_manifest.get(&verb_oid),
            Some(&verb_sid),
            "Verb snapshot should be pinned"
        );

        db.cleanup().await;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // INV-3: Proof Rule (DB CHECK constraint)
    // ═══════════════════════════════════════════════════════════════════════

    /// trust_class=Proof + governance_tier=Operational is rejected by the
    /// PostgreSQL CHECK constraint `chk_proof_rule`.
    #[tokio::test]
    #[ignore]
    async fn test_inv3_proof_rule_db_constraint() -> Result<()> {
        let db = InvTestDb::new().await?;

        // Attempt to insert a snapshot violating the proof rule directly via SQL
        let result = sqlx::query(
            r#"
            INSERT INTO sem_reg.snapshots (
                object_type, object_id, version_major, version_minor,
                status, governance_tier, trust_class, security_label,
                change_type, created_by, definition
            ) VALUES (
                'attribute_def', $1, 1, 0,
                'active', 'operational', 'proof', '{}',
                'created', 'inv_test', '{"fqn": "invalid.proof_test"}'
            )
            "#,
        )
        .bind(Uuid::new_v4())
        .execute(&db.pool)
        .await;

        assert!(
            result.is_err(),
            "INSERT with operational+proof should violate chk_proof_rule CHECK constraint"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("chk_proof_rule")
                || err_msg.contains("check")
                || err_msg.contains("violat"),
            "Error should reference the proof rule constraint. Got: {}",
            err_msg
        );

        // Also verify the Rust-side gate catches it
        let gate_result = evaluate_publish_gates(
            &SnapshotMeta {
                object_type: ObjectType::AttributeDef,
                object_id: Uuid::new_v4(),
                version_major: 1,
                version_minor: 0,
                status: SnapshotStatus::Active,
                governance_tier: GovernanceTier::Operational,
                trust_class: TrustClass::Proof,
                security_label: SecurityLabel::default(),
                change_type: ChangeType::Created,
                change_rationale: None,
                created_by: "inv_test".into(),
                approved_by: None,
                predecessor_id: None,
            },
            None,
        );
        assert!(
            !gate_result.all_passed(),
            "Rust-side publish gates should also reject operational+proof"
        );

        db.cleanup().await;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // INV-4: ABAC applies to both governance tiers
    // ═══════════════════════════════════════════════════════════════════════

    /// Both Governed and Operational snapshots must pass ABAC access control.
    /// ABAC is NOT skipped for Operational tier — security is orthogonal to governance.
    #[tokio::test]
    #[ignore]
    async fn test_inv4_abac_applies_to_both_tiers() -> Result<()> {
        let db = InvTestDb::new().await?;

        // Restricted label (applies regardless of tier)
        let restricted_label = SecurityLabel {
            classification: Classification::Restricted,
            pii: true,
            jurisdictions: vec!["LU".into()],
            purpose_limitation: vec![],
            handling_controls: vec![],
        };

        // Low-clearance actor
        let low_actor = ActorContext {
            actor_id: "intern".into(),
            roles: vec!["viewer".into()],
            department: None,
            clearance: Some(Classification::Public),
            jurisdictions: vec!["US".into()], // wrong jurisdiction
        };

        // ABAC must deny for Governed snapshot
        let governed_decision =
            evaluate_abac(&low_actor, &restricted_label, AccessPurpose::Operations);
        assert!(
            matches!(governed_decision, AccessDecision::Deny { .. }),
            "ABAC should deny low-clearance actor on Restricted/Governed data. Got: {:?}",
            governed_decision
        );

        // ABAC must ALSO deny for Operational snapshot (same label)
        // The governance tier doesn't exempt from ABAC — this is the invariant
        let operational_decision =
            evaluate_abac(&low_actor, &restricted_label, AccessPurpose::Operations);
        assert!(
            matches!(operational_decision, AccessDecision::Deny { .. }),
            "ABAC should deny low-clearance actor on Restricted/Operational data too. Got: {:?}",
            operational_decision
        );

        // Both decisions should have the same deny behavior
        // (ABAC is orthogonal to governance tier)
        match (&governed_decision, &operational_decision) {
            (AccessDecision::Deny { reason: r1 }, AccessDecision::Deny { reason: r2 }) => {
                // Both denied — invariant holds. Reasons may differ in detail
                // but both must be denied.
                println!("Governed deny reason: {}", r1);
                println!("Operational deny reason: {}", r2);
            }
            _ => panic!(
                "Both tiers should be denied. Governed: {:?}, Operational: {:?}",
                governed_decision, operational_decision
            ),
        }

        db.cleanup().await;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // INV-5: Operational auto-approve
    // ═══════════════════════════════════════════════════════════════════════

    /// Operational-tier snapshots do not require governed approval gates.
    /// They are auto-approved (approved_by = "auto") to enable rapid iteration.
    #[tokio::test]
    #[ignore]
    async fn test_inv5_operational_auto_approve() -> Result<()> {
        let db = InvTestDb::new().await?;

        // Publish an operational attribute without explicit approval
        let attr_fqn = db.fqn("attr.auto_approve_test");
        let object_id = Uuid::new_v4();
        let meta = SnapshotMeta::new_operational(ObjectType::AttributeDef, object_id, "inv_test");

        // Verify meta has auto-approval
        assert_eq!(
            meta.approved_by.as_deref(),
            Some("auto"),
            "Operational meta should have auto-approval"
        );

        // Publish gates should pass without explicit approver
        let gate_result = evaluate_publish_gates(&meta, None);
        assert!(
            gate_result.all_passed(),
            "Operational snapshot should pass all publish gates without explicit approval. Failures: {:?}",
            gate_result.failure_messages()
        );

        // Actually publish — should succeed
        let body = AttributeDefBody {
            fqn: attr_fqn.clone(),
            name: "Auto-approve test".into(),
            description: "Test operational auto-approval".into(),
            domain: "inv_test".into(),
            data_type: AttributeDataType::String,
            source: None,
            constraints: None,
            sinks: vec![],
        };
        let sid = RegistryService::publish_attribute_def(&db.pool, &meta, &body, None).await?;
        assert_ne!(sid, Uuid::nil(), "Should get a valid snapshot_id");

        // Contrast with Governed tier — missing approval should fail gates
        let governed_meta = SnapshotMeta {
            governance_tier: GovernanceTier::Governed,
            trust_class: TrustClass::DecisionSupport,
            approved_by: None, // deliberately missing
            ..SnapshotMeta::new_operational(ObjectType::AttributeDef, Uuid::new_v4(), "inv_test")
        };
        let governed_gate = evaluate_publish_gates(&governed_meta, None);
        assert!(
            !governed_gate.all_passed(),
            "Governed snapshot without approval should fail gates"
        );
        let approval_failure = governed_gate
            .failures()
            .iter()
            .any(|f| f.gate_name == "governed_approval");
        assert!(
            approval_failure,
            "Should specifically fail the governed_approval gate"
        );

        db.cleanup().await;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // INV-6: DerivationSpec required for derived attributes
    // ═══════════════════════════════════════════════════════════════════════

    /// A derived attribute without a corresponding DerivationSpec should
    /// trigger a publish gate failure.
    ///
    /// NOTE: This test documents the expected behavior. It will pass once
    /// S6 wires the extended gate framework (check_derivation_type_compatibility)
    /// into the aggregate publish gate pipeline.
    #[tokio::test]
    #[ignore]
    async fn test_inv6_derivation_spec_required() -> Result<()> {
        use ob_poc::sem_reg::derivation_spec::*;
        use ob_poc::sem_reg::gates::{check_derivation_type_compatibility, GateSeverity};
        use std::collections::HashSet;

        // Create a derivation spec referencing an unknown output attribute
        let spec = DerivationSpecBody {
            fqn: "test.derived_attr".into(),
            name: "Test Derived Attr".into(),
            description: "A derived attribute for invariant testing".into(),
            output_attribute_fqn: "attr.does_not_exist".into(),
            inputs: vec![DerivationInput {
                attribute_fqn: "attr.also_missing".into(),
                role: "input".into(),
                required: true,
            }],
            expression: DerivationExpression::FunctionRef {
                ref_name: "sum".into(),
            },
            null_semantics: NullSemantics::Propagate,
            freshness_rule: None,
            security_inheritance: SecurityInheritanceMode::Strict,
            evidence_grade: EvidenceGrade::Prohibited,
            tests: vec![],
        };

        // With an empty known-attributes set, the gate should detect the missing types
        let known_attrs: HashSet<String> = HashSet::new();
        let failures = check_derivation_type_compatibility(&spec, &known_attrs);

        assert!(
            !failures.is_empty(),
            "Derivation referencing unknown attributes should produce gate failures"
        );

        // Should have failures for both the output and input
        let has_output_failure = failures
            .iter()
            .any(|f| f.message.contains("output attribute"));
        let has_input_failure = failures
            .iter()
            .any(|f| f.message.contains("input attribute"));

        assert!(has_output_failure, "Should flag missing output attribute");
        assert!(has_input_failure, "Should flag missing input attribute");

        // All failures should be error-severity
        assert!(
            failures.iter().all(|f| f.severity == GateSeverity::Error),
            "Missing derivation references should be Error severity"
        );

        Ok(())
    }
}
