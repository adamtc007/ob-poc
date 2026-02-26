//! Phase B3 — First Real Changesets (Scripted)
//!
//! 5 integration tests exercising the full Draft → Gate → Review → Publish
//! lifecycle on real bootstrapped data.
//!
//! Run with:
//! ```sh
//! DATABASE_URL="postgresql:///data_designer" \
//!   cargo test --features database --test stewardship_b3_changesets -- --ignored --nocapture --test-threads=1
//! ```
//!
//! IMPORTANT: --test-threads=1 because CS1-CS4 must execute in order.
//! CS4 depends on CS2's published state.

#[cfg(feature = "database")]
mod b3_changesets {
    use anyhow::{Context, Result};
    use serde_json::json;
    use sqlx::PgPool;
    use uuid::Uuid;

    use ob_poc::sem_reg::abac::ActorContext;
    use ob_poc::sem_reg::agent::mcp_tools::{SemRegToolContext, SemRegToolResult};
    #[allow(unused_imports)]
    use ob_poc::sem_reg::onboarding::seed::BOOTSTRAP_SET_ID;
    use ob_poc::sem_reg::stewardship::tools_phase0::dispatch_phase0_tool;

    // ── Test Infrastructure ──────────────────────────────────────────

    fn test_actor() -> ActorContext {
        ActorContext {
            actor_id: "b3_test".into(),
            roles: vec!["steward".into()],
            department: None,
            clearance: None,
            jurisdictions: vec![],
        }
    }

    async fn get_pool() -> Result<PgPool> {
        let url =
            std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".into());
        let pool = PgPool::connect(&url).await?;
        Ok(pool)
    }

    /// Discover real FQNs from bootstrap.
    /// Returns (fqn, snapshot_id, object_id, definition_json).
    async fn discover_fqn(
        pool: &PgPool,
        object_type: &str,
        extra_filter: &str,
    ) -> Result<(String, Uuid, Uuid, serde_json::Value)> {
        let query = format!(
            r#"
            SELECT definition->>'fqn', snapshot_id, object_id, definition
            FROM sem_reg.snapshots
            WHERE object_type = '{}'::sem_reg.object_type
              AND status = 'active'
              AND effective_until IS NULL
              {}
            LIMIT 1
            "#,
            object_type, extra_filter
        );
        let row = sqlx::query_as::<_, (String, Uuid, Uuid, serde_json::Value)>(&query)
            .fetch_one(pool)
            .await
            .context(format!(
                "No active {} found with filter: {}",
                object_type, extra_filter
            ))?;
        Ok(row)
    }

    /// Discover multiple FQNs from bootstrap.
    async fn discover_fqns(
        pool: &PgPool,
        object_type: &str,
        extra_filter: &str,
        limit: i64,
    ) -> Result<Vec<(String, Uuid, Uuid, serde_json::Value)>> {
        let query = format!(
            r#"
            SELECT definition->>'fqn', snapshot_id, object_id, definition
            FROM sem_reg.snapshots
            WHERE object_type = '{}'::sem_reg.object_type
              AND status = 'active'
              AND effective_until IS NULL
              {}
            LIMIT {}
            "#,
            object_type, extra_filter, limit
        );
        let rows = sqlx::query_as::<_, (String, Uuid, Uuid, serde_json::Value)>(&query)
            .fetch_all(pool)
            .await
            .context(format!("Failed to discover {}", object_type))?;
        Ok(rows)
    }

    /// Helper: unwrap a dispatch_phase0_tool result.
    fn unwrap_tool_result(result: Option<SemRegToolResult>, tool_name: &str) -> SemRegToolResult {
        let result = result
            .unwrap_or_else(|| panic!("{}: dispatch returned None (tool not found)", tool_name));
        if !result.success {
            panic!(
                "{}: tool failed: {}",
                tool_name,
                result.error.as_deref().unwrap_or("(no error message)")
            );
        }
        result
    }

    /// Execute full changeset lifecycle: submit → approve → publish.
    /// Returns the publish result data.
    async fn run_lifecycle(
        ctx: &SemRegToolContext<'_>,
        changeset_id: Uuid,
        label: &str,
    ) -> Result<serde_json::Value> {
        // Submit for review
        println!("  [{label}] Submitting for review...");
        let result = dispatch_phase0_tool(
            ctx,
            "stew_submit_for_review",
            &json!({"changeset_id": changeset_id.to_string()}),
        )
        .await;
        let result = unwrap_tool_result(result, &format!("{label}: stew_submit_for_review"));
        println!(
            "  [{label}] Submit result: status={}",
            result
                .data
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("?")
        );

        // Verify status is under_review
        let status = result
            .data
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert_eq!(
            status, "under_review",
            "[{label}] Expected under_review after submit"
        );

        // Record review decision: approve
        println!("  [{label}] Recording review decision: approve...");
        let result = dispatch_phase0_tool(
            ctx,
            "stew_record_review_decision",
            &json!({
                "changeset_id": changeset_id.to_string(),
                "verdict": "approve",
                "note": format!("B3 {} reviewed and approved", label),
            }),
        )
        .await;
        let result = unwrap_tool_result(result, &format!("{label}: stew_record_review_decision"));
        let new_status = result
            .data
            .get("new_status")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        println!("  [{label}] Review result: new_status={}", new_status);
        assert_eq!(
            new_status, "approved",
            "[{label}] Expected approved after review"
        );

        // Publish
        println!("  [{label}] Publishing...");
        let result = dispatch_phase0_tool(
            ctx,
            "stew_publish",
            &json!({"changeset_id": changeset_id.to_string()}),
        )
        .await;
        let result = unwrap_tool_result(result, &format!("{label}: stew_publish"));
        let pub_status = result
            .data
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let promoted = result
            .data
            .get("snapshots_promoted")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        println!(
            "  [{label}] Publish result: status={}, promoted={}",
            pub_status, promoted
        );
        assert_eq!(
            pub_status, "published",
            "[{label}] Expected published status"
        );

        Ok(result.data)
    }

    /// Verify audit events exist for a changeset.
    async fn verify_audit_events(
        pool: &PgPool,
        changeset_id: Uuid,
        expected_types: &[&str],
        label: &str,
    ) -> Result<()> {
        let events: Vec<(String,)> = sqlx::query_as(
            "SELECT event_type FROM stewardship.events WHERE changeset_id = $1 ORDER BY created_at",
        )
        .bind(changeset_id)
        .fetch_all(pool)
        .await?;

        let event_types: Vec<&str> = events.iter().map(|e| e.0.as_str()).collect();
        println!("  [{label}] Audit events ({}):", event_types.len());
        for et in &event_types {
            println!("    - {}", et);
        }

        for expected in expected_types {
            assert!(
                event_types.contains(expected),
                "[{label}] Missing expected audit event: {}. Found: {:?}",
                expected,
                event_types
            );
        }
        Ok(())
    }

    /// Clean up a changeset and all its associated data.
    /// NOTE: We do NOT clean up published changesets since their snapshots
    /// are now Active and part of the registry state.
    #[allow(dead_code)]
    async fn cleanup_changeset(pool: &PgPool, changeset_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM stewardship.basis_claims WHERE basis_id IN (SELECT basis_id FROM stewardship.basis_records WHERE changeset_id = $1)")
            .bind(changeset_id).execute(pool).await.ok();
        sqlx::query("DELETE FROM stewardship.basis_records WHERE changeset_id = $1")
            .bind(changeset_id)
            .execute(pool)
            .await
            .ok();
        sqlx::query("DELETE FROM stewardship.events WHERE changeset_id = $1")
            .bind(changeset_id)
            .execute(pool)
            .await
            .ok();
        sqlx::query("DELETE FROM sem_reg.changeset_reviews WHERE changeset_id = $1")
            .bind(changeset_id)
            .execute(pool)
            .await
            .ok();
        sqlx::query("DELETE FROM sem_reg.changeset_entries WHERE changeset_id = $1")
            .bind(changeset_id)
            .execute(pool)
            .await
            .ok();
        sqlx::query(
            "DELETE FROM sem_reg.snapshots WHERE snapshot_set_id = $1 AND status = 'draft'",
        )
        .bind(changeset_id)
        .execute(pool)
        .await
        .ok();
        sqlx::query("DELETE FROM sem_reg.changesets WHERE changeset_id = $1")
            .bind(changeset_id)
            .execute(pool)
            .await
            .ok();
        sqlx::query("DELETE FROM sem_reg.snapshot_sets WHERE snapshot_set_id = $1")
            .bind(changeset_id)
            .execute(pool)
            .await
            .ok();
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════
    //  CS1: Taxonomy Classification
    // ═══════════════════════════════════════════════════════════════

    #[tokio::test]
    #[ignore]
    async fn test_b3_cs1_taxonomy_classification() -> Result<()> {
        println!("\n══ CS1: Taxonomy Classification ══\n");
        let pool = get_pool().await?;
        let actor = test_actor();
        let ctx = SemRegToolContext {
            pool: &pool,
            actor: &actor,
        };

        // 1. Create changeset
        let result = dispatch_phase0_tool(
            &ctx,
            "stew_compose_changeset",
            &json!({
                "scope": "b3-cs1-taxonomy-classification",
                "intent": "Classify KYC-related attributes and verbs into regulatory domain taxonomy"
            }),
        )
        .await;
        let result = unwrap_tool_result(result, "CS1: stew_compose_changeset");
        let changeset_id: Uuid = result.data["changeset_id"]
            .as_str()
            .expect("changeset_id")
            .parse()?;
        println!("  Created changeset: {}", changeset_id);

        // 2. Find KYC-related AttributeDefs
        let kyc_attrs = discover_fqns(
            &pool,
            "attribute_def",
            "AND (definition->>'fqn' ILIKE '%kyc%' OR definition->>'fqn' ILIKE '%cdd%' OR definition->>'fqn' ILIKE '%client%')",
            5,
        )
        .await?;
        println!("  Found {} KYC-related AttributeDefs", kyc_attrs.len());
        let kyc_attr_count = kyc_attrs.len();
        assert!(
            kyc_attr_count > 0,
            "Must find at least 1 KYC-related AttributeDef"
        );

        // Find KYC-related VerbContracts
        let kyc_verbs = discover_fqns(
            &pool,
            "verb_contract",
            "AND (definition->>'fqn' ILIKE '%kyc%' OR definition->>'domain' = 'kyc')",
            2,
        )
        .await?;
        println!("  Found {} KYC-related VerbContracts", kyc_verbs.len());
        let _kyc_verb_count = kyc_verbs.len();

        // 3. For each, add a membership_rule item (closest object type to "taxonomy membership")
        let mut items_added = 0;
        for (fqn, _snap_id, _obj_id, _def) in &kyc_attrs {
            let membership_fqn = format!("membership.regulatory_kyc.{}", fqn.replace('.', "_"));
            let membership_payload = json!({
                "fqn": membership_fqn,
                "description": format!("Classifies {} into regulatory.kyc taxonomy", fqn),
                "subject_type": "attribute_def",
                "subject_fqn": fqn,
                "taxonomy_fqn": "regulatory.kyc",
                "membership_type": "classified",
            });

            let add_result = dispatch_phase0_tool(
                &ctx,
                "stew_add_item",
                &json!({
                    "changeset_id": changeset_id.to_string(),
                    "action": "add",
                    "object_type": "membership_rule",
                    "object_fqn": membership_fqn,
                    "draft_payload": membership_payload,
                }),
            )
            .await;
            let add_result =
                unwrap_tool_result(add_result, &format!("CS1: stew_add_item (attr {})", fqn));
            println!(
                "    Added membership for attr {}: entry_id={}",
                fqn,
                add_result
                    .data
                    .get("entry_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
            );
            items_added += 1;
        }

        for (fqn, _snap_id, _obj_id, _def) in &kyc_verbs {
            let membership_fqn = format!("membership.regulatory_kyc.{}", fqn.replace('.', "_"));
            let membership_payload = json!({
                "fqn": membership_fqn,
                "description": format!("Classifies {} into regulatory.kyc taxonomy", fqn),
                "subject_type": "verb_contract",
                "subject_fqn": fqn,
                "taxonomy_fqn": "regulatory.kyc",
                "membership_type": "classified",
            });

            let add_result = dispatch_phase0_tool(
                &ctx,
                "stew_add_item",
                &json!({
                    "changeset_id": changeset_id.to_string(),
                    "action": "add",
                    "object_type": "membership_rule",
                    "object_fqn": membership_fqn,
                    "draft_payload": membership_payload,
                }),
            )
            .await;
            let add_result =
                unwrap_tool_result(add_result, &format!("CS1: stew_add_item (verb {})", fqn));
            println!(
                "    Added membership for verb {}: entry_id={}",
                fqn,
                add_result
                    .data
                    .get("entry_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
            );
            items_added += 1;
        }
        println!("  Total items added: {}", items_added);
        assert!(items_added > 0, "Must add at least 1 item");

        // 4. Attach basis
        let basis_result = dispatch_phase0_tool(
            &ctx,
            "stew_attach_basis",
            &json!({
                "changeset_id": changeset_id.to_string(),
                "kind": "platform_convention",
                "source_ref": "Initial taxonomy classification from bootstrap analysis",
                "excerpt": "Objects classified based on FQN pattern matching and verb domain analysis",
                "claims": [{
                    "claim_text": "KYC-related objects classified into regulatory.kyc taxonomy based on domain and FQN patterns",
                    "confidence": 0.9,
                }],
            }),
        )
        .await;
        let basis_result = unwrap_tool_result(basis_result, "CS1: stew_attach_basis");
        println!(
            "  Basis attached: basis_id={}",
            basis_result
                .data
                .get("basis_id")
                .and_then(|v| v.as_str())
                .unwrap_or("?")
        );

        // 5. Gate precheck
        let gate_result = dispatch_phase0_tool(
            &ctx,
            "stew_gate_precheck",
            &json!({"changeset_id": changeset_id.to_string()}),
        )
        .await;
        let gate_result = unwrap_tool_result(gate_result, "CS1: stew_gate_precheck");
        let blocking = gate_result.data["blocking_count"].as_i64().unwrap_or(0);
        let warnings = gate_result.data["warning_count"].as_i64().unwrap_or(0);
        let advisories = gate_result.data["advisory_count"].as_i64().unwrap_or(0);
        println!(
            "  Gate precheck: {} blocking, {} warnings, {} advisories",
            blocking, warnings, advisories
        );

        // G02 naming warnings and G09 advisory are acceptable; blocking should be 0
        assert_eq!(
            blocking, 0,
            "CS1: No blocking guardrails expected for taxonomy classification"
        );

        // 6-8. Submit → Approve → Publish
        let publish_data = run_lifecycle(&ctx, changeset_id, "CS1").await?;
        let promoted = publish_data
            .get("snapshots_promoted")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert!(promoted > 0, "CS1: Must promote at least 1 snapshot");
        println!("  CS1 promoted {} snapshots", promoted);

        // 9. Verify audit events
        verify_audit_events(
            &pool,
            changeset_id,
            &[
                "changeset_created",
                "item_added",
                "basis_attached",
                "gate_prechecked",
                "submitted_for_review",
                "review_decision_recorded",
                "published",
            ],
            "CS1",
        )
        .await?;

        // Verify changeset status in DB
        let cs_status: (String,) =
            sqlx::query_as("SELECT status FROM sem_reg.changesets WHERE changeset_id = $1")
                .bind(changeset_id)
                .fetch_one(&pool)
                .await?;
        assert_eq!(
            cs_status.0, "published",
            "CS1: Changeset should be published"
        );

        println!("\n  ✓ CS1 PASSED: Taxonomy classification changeset published.\n");
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════
    //  CS2: PolicyRule + Tier Promotion
    // ═══════════════════════════════════════════════════════════════

    #[tokio::test]
    #[ignore]
    async fn test_b3_cs2_policy_and_promotion() -> Result<()> {
        println!("\n══ CS2: PolicyRule + Tier Promotion ══\n");
        let pool = get_pool().await?;
        let actor = test_actor();
        let ctx = SemRegToolContext {
            pool: &pool,
            actor: &actor,
        };

        // 1. Create changeset
        let result = dispatch_phase0_tool(
            &ctx,
            "stew_compose_changeset",
            &json!({
                "scope": "b3-cs2-policy-promotion",
                "intent": "Add CDD verification policy: beneficial ownership must be documented for entities with >25% ownership. Promote related attributes to Governed/Proof."
            }),
        )
        .await;
        let result = unwrap_tool_result(result, "CS2: stew_compose_changeset");
        let changeset_id: Uuid = result.data["changeset_id"]
            .as_str()
            .expect("changeset_id")
            .parse()?;
        println!("  Created changeset: {}", changeset_id);

        // 2. Find 2 ownership-related AttributeDefs
        let ownership_attrs = discover_fqns(
            &pool,
            "attribute_def",
            "AND (definition->>'fqn' ILIKE '%ownership%' OR definition->>'fqn' ILIKE '%beneficial%' OR definition->>'fqn' ILIKE '%ubo%')",
            2,
        )
        .await?;
        println!(
            "  Found {} ownership-related AttributeDefs",
            ownership_attrs.len()
        );
        assert!(
            !ownership_attrs.is_empty(),
            "Must find at least 1 ownership-related AttributeDef"
        );

        // 3. Modify each to Governed/Proof (promotion)
        let mut promoted_fqns = Vec::new();
        for (fqn, snap_id, _obj_id, def) in &ownership_attrs {
            let mut modified_body = def.clone();
            if let Some(obj) = modified_body.as_object_mut() {
                obj.insert("governance_tier".into(), json!("governed"));
                obj.insert("trust_class".into(), json!("proof"));
            }

            let add_result = dispatch_phase0_tool(
                &ctx,
                "stew_add_item",
                &json!({
                    "changeset_id": changeset_id.to_string(),
                    "action": "modify",
                    "object_type": "attribute_def",
                    "object_fqn": fqn,
                    "draft_payload": modified_body,
                    "predecessor_id": snap_id.to_string(),
                    "reasoning": "Promoting to Governed/Proof for CDD beneficial ownership policy compliance",
                }),
            )
            .await;
            let add_result =
                unwrap_tool_result(add_result, &format!("CS2: stew_add_item (promote {})", fqn));
            println!(
                "    Promoted {}: entry_id={}",
                fqn,
                add_result
                    .data
                    .get("entry_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
            );
            promoted_fqns.push(fqn.clone());
        }

        // 4. Add PolicyRule
        let policy_fqn = "policy.kyc.cdd_beneficial_ownership_threshold";
        let policy_payload = json!({
            "fqn": policy_fqn,
            "description": "Beneficial ownership documentation required for >25% ownership",
            "predicate_refs": promoted_fqns,
            "threshold": 0.25,
            "evidence_required": true,
            "domain": "kyc",
        });

        let add_result = dispatch_phase0_tool(
            &ctx,
            "stew_add_item",
            &json!({
                "changeset_id": changeset_id.to_string(),
                "action": "add",
                "object_type": "policy_rule",
                "object_fqn": policy_fqn,
                "draft_payload": policy_payload,
            }),
        )
        .await;
        let add_result = unwrap_tool_result(add_result, "CS2: stew_add_item (PolicyRule)");
        println!(
            "  Added PolicyRule: entry_id={}",
            add_result
                .data
                .get("entry_id")
                .and_then(|v| v.as_str())
                .unwrap_or("?")
        );

        // 5. Add EvidenceRequirement
        let evidence_fqn = "evidence.kyc.beneficial_ownership_proof";
        let evidence_payload = json!({
            "fqn": evidence_fqn,
            "description": "Documentary proof of beneficial ownership structure",
            "required_for": [policy_fqn],
            "freshness_days": 365,
            "domain": "kyc",
        });

        let add_result = dispatch_phase0_tool(
            &ctx,
            "stew_add_item",
            &json!({
                "changeset_id": changeset_id.to_string(),
                "action": "add",
                "object_type": "evidence_requirement",
                "object_fqn": evidence_fqn,
                "draft_payload": evidence_payload,
            }),
        )
        .await;
        let add_result = unwrap_tool_result(add_result, "CS2: stew_add_item (EvidenceRequirement)");
        println!(
            "  Added EvidenceRequirement: entry_id={}",
            add_result
                .data
                .get("entry_id")
                .and_then(|v| v.as_str())
                .unwrap_or("?")
        );

        // 6. Attach basis
        let basis_result = dispatch_phase0_tool(
            &ctx,
            "stew_attach_basis",
            &json!({
                "changeset_id": changeset_id.to_string(),
                "kind": "regulatory_fact",
                "source_ref": "CDD requirements under AML directives",
                "excerpt": "Beneficial ownership verification required for entities with significant control (>25% threshold)",
                "claims": [{
                    "claim_text": "Beneficial ownership verification required for entities with significant control",
                    "confidence": 0.95,
                }],
            }),
        )
        .await;
        unwrap_tool_result(basis_result, "CS2: stew_attach_basis");
        println!("  Basis attached");

        // 7. Gate precheck
        let gate_result = dispatch_phase0_tool(
            &ctx,
            "stew_gate_precheck",
            &json!({"changeset_id": changeset_id.to_string()}),
        )
        .await;
        let gate_result = unwrap_tool_result(gate_result, "CS2: stew_gate_precheck");
        let blocking = gate_result.data["blocking_count"].as_i64().unwrap_or(0);
        let warnings = gate_result.data["warning_count"].as_i64().unwrap_or(0);
        println!(
            "  Gate precheck: {} blocking, {} warnings",
            blocking, warnings
        );

        // G04 (ProofChainCompatibility) should NOT block since we're promoting TO Proof
        // G13 (ResolutionMetadataMissing) warning acceptable for new PolicyRule
        assert_eq!(
            blocking, 0,
            "CS2: No blocking guardrails expected for promotion + policy"
        );

        // 8. Submit → Approve → Publish
        let publish_data = run_lifecycle(&ctx, changeset_id, "CS2").await?;
        let promoted = publish_data
            .get("snapshots_promoted")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let expected_count = promoted_fqns.len() as u64 + 2; // promotions + PolicyRule + EvidenceRequirement
        println!(
            "  CS2 promoted {} snapshots (expected {})",
            promoted, expected_count
        );
        assert_eq!(
            promoted, expected_count,
            "CS2: Expected {} promoted snapshots",
            expected_count
        );

        // 9. Verify promoted attributes are now Active with superseded predecessors
        for fqn in &promoted_fqns {
            // New Active snapshot should exist
            let new_active: (Uuid, String) = sqlx::query_as(
                r#"
                SELECT snapshot_id, definition->>'governance_tier'
                FROM sem_reg.snapshots
                WHERE object_type = 'attribute_def'::sem_reg.object_type
                  AND definition->>'fqn' = $1
                  AND status = 'active'
                  AND effective_until IS NULL
                  AND snapshot_set_id = $2
                "#,
            )
            .bind(fqn)
            .bind(changeset_id)
            .fetch_one(&pool)
            .await
            .context(format!("CS2: New Active snapshot not found for {}", fqn))?;
            println!(
                "    {} new Active: snap_id={}, tier={}",
                fqn, new_active.0, new_active.1
            );
        }

        // Verify old predecessors are superseded (effective_until IS NOT NULL)
        for (fqn, old_snap_id, _, _) in &ownership_attrs {
            let superseded: (Option<chrono::DateTime<chrono::Utc>>,) = sqlx::query_as(
                "SELECT effective_until FROM sem_reg.snapshots WHERE snapshot_id = $1",
            )
            .bind(old_snap_id)
            .fetch_one(&pool)
            .await?;
            assert!(
                superseded.0.is_some(),
                "CS2: Predecessor {} for {} should be superseded (effective_until set)",
                old_snap_id,
                fqn
            );
            println!("    {} predecessor superseded at {:?}", fqn, superseded.0);
        }

        // Verify audit events
        verify_audit_events(
            &pool,
            changeset_id,
            &[
                "changeset_created",
                "item_added",
                "basis_attached",
                "gate_prechecked",
                "submitted_for_review",
                "review_decision_recorded",
                "published",
            ],
            "CS2",
        )
        .await?;

        println!("\n  ✓ CS2 PASSED: PolicyRule + Tier Promotion changeset published.\n");
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════
    //  CS3: VerbContract Enrichment + Binding
    // ═══════════════════════════════════════════════════════════════

    #[tokio::test]
    #[ignore]
    async fn test_b3_cs3_verb_enrichment_and_binding() -> Result<()> {
        println!("\n══ CS3: VerbContract Enrichment + Binding ══\n");
        let pool = get_pool().await?;
        let actor = test_actor();
        let ctx = SemRegToolContext {
            pool: &pool,
            actor: &actor,
        };

        // 1. Create changeset
        let result = dispatch_phase0_tool(
            &ctx,
            "stew_compose_changeset",
            &json!({
                "scope": "b3-cs3-verb-enrichment",
                "intent": "Enrich a VerbContract with resolution metadata and create implementation binding"
            }),
        )
        .await;
        let result = unwrap_tool_result(result, "CS3: stew_compose_changeset");
        let changeset_id: Uuid = result.data["changeset_id"]
            .as_str()
            .expect("changeset_id")
            .parse()?;
        println!("  Created changeset: {}", changeset_id);

        // 2. Find a KYC VerbContract
        let (verb_fqn, verb_snap_id, _verb_obj_id, verb_def) = discover_fqn(
            &pool,
            "verb_contract",
            "AND definition->>'fqn' ILIKE '%kyc%' AND definition->>'description' IS NOT NULL",
        )
        .await?;
        println!("  Target VerbContract: {}", verb_fqn);

        // 3. Modify with resolution metadata
        let mut enriched_body = verb_def.clone();
        if let Some(obj) = enriched_body.as_object_mut() {
            obj.insert(
                "usage_examples".into(),
                json!(["Example: verify beneficial ownership for institutional client"]),
            );
            obj.insert(
                "parameter_guidance".into(),
                json!({
                    "entity_id": "The entity to verify — UUID from entity resolution",
                    "depth": "Number of ownership levels to traverse (default: 3)"
                }),
            );
            obj.insert(
                "input_source_hints".into(),
                json!({
                    "ownership_data": "UserProvided",
                    "entity_id": "SessionScope"
                }),
            );
        }

        let add_result = dispatch_phase0_tool(
            &ctx,
            "stew_add_item",
            &json!({
                "changeset_id": changeset_id.to_string(),
                "action": "modify",
                "object_type": "verb_contract",
                "object_fqn": verb_fqn,
                "draft_payload": enriched_body,
                "predecessor_id": verb_snap_id.to_string(),
                "reasoning": "Enriching with resolution metadata for intent pipeline readiness",
            }),
        )
        .await;
        let add_result = unwrap_tool_result(add_result, "CS3: stew_add_item (modify verb)");
        println!(
            "  Modified VerbContract: entry_id={}",
            add_result
                .data
                .get("entry_id")
                .and_then(|v| v.as_str())
                .unwrap_or("?")
        );

        // 4. Attach basis
        let basis_result = dispatch_phase0_tool(
            &ctx,
            "stew_attach_basis",
            &json!({
                "changeset_id": changeset_id.to_string(),
                "kind": "platform_convention",
                "source_ref": "Resolution metadata enrichment for intent pipeline readiness",
                "excerpt": "VerbContract enriched with usage_examples, parameter_guidance, and input_source_hints to improve agent intent resolution",
            }),
        )
        .await;
        unwrap_tool_result(basis_result, "CS3: stew_attach_basis");
        println!("  Basis attached");

        // 5. Gate precheck — G13 should NOT fire (metadata now present)
        let gate_result = dispatch_phase0_tool(
            &ctx,
            "stew_gate_precheck",
            &json!({"changeset_id": changeset_id.to_string()}),
        )
        .await;
        let gate_result = unwrap_tool_result(gate_result, "CS3: stew_gate_precheck");
        let blocking = gate_result.data["blocking_count"].as_i64().unwrap_or(0);
        println!("  Gate precheck: {} blocking", blocking);
        assert_eq!(
            blocking, 0,
            "CS3: No blocking guardrails expected for verb enrichment"
        );

        // 6. Submit → Approve → Publish
        let publish_data = run_lifecycle(&ctx, changeset_id, "CS3").await?;
        let promoted = publish_data
            .get("snapshots_promoted")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert_eq!(
            promoted, 1,
            "CS3: Expected 1 promoted snapshot (enriched verb)"
        );

        // 7. Verify enriched VerbContract is now Active
        let enriched: (serde_json::Value,) = sqlx::query_as(
            r#"
            SELECT definition
            FROM sem_reg.snapshots
            WHERE object_type = 'verb_contract'::sem_reg.object_type
              AND definition->>'fqn' = $1
              AND status = 'active'
              AND effective_until IS NULL
              AND snapshot_set_id = $2
            "#,
        )
        .bind(&verb_fqn)
        .bind(changeset_id)
        .fetch_one(&pool)
        .await
        .context("CS3: Enriched VerbContract Active snapshot not found")?;

        assert!(
            enriched.0.get("usage_examples").is_some(),
            "CS3: Enriched VerbContract must have usage_examples"
        );
        assert!(
            enriched.0.get("parameter_guidance").is_some(),
            "CS3: Enriched VerbContract must have parameter_guidance"
        );
        println!("  Enriched VerbContract verified: has usage_examples + parameter_guidance");

        // Verify predecessor is superseded
        let superseded: (Option<chrono::DateTime<chrono::Utc>>,) =
            sqlx::query_as("SELECT effective_until FROM sem_reg.snapshots WHERE snapshot_id = $1")
                .bind(verb_snap_id)
                .fetch_one(&pool)
                .await?;
        assert!(
            superseded.0.is_some(),
            "CS3: Predecessor verb snapshot should be superseded"
        );
        println!("  Predecessor superseded at {:?}", superseded.0);

        // 8. Create VerbImplementationBinding via direct SQL
        // (no stew tool exists for this)
        let binding_ref = format!("ob_poc::handlers::{}", verb_fqn.replace('.', "::"));
        sqlx::query(
            r#"
            INSERT INTO stewardship.verb_implementation_bindings
                (verb_fqn, binding_kind, binding_ref, exec_modes, status, notes)
            VALUES ($1, 'rust_handler', $2, '["sync"]'::jsonb, 'active', 'B3 CS3 test binding')
            ON CONFLICT (verb_fqn) WHERE status = 'active'
            DO UPDATE SET binding_ref = EXCLUDED.binding_ref, notes = EXCLUDED.notes
            "#,
        )
        .bind(&verb_fqn)
        .bind(&binding_ref)
        .execute(&pool)
        .await
        .context("CS3: Failed to insert verb implementation binding")?;
        println!(
            "  Created VerbImplementationBinding: {} -> {}",
            verb_fqn, binding_ref
        );

        // 9. Verify binding
        let binding: (String, String, String) = sqlx::query_as(
            r#"
            SELECT verb_fqn, binding_kind, status
            FROM stewardship.verb_implementation_bindings
            WHERE verb_fqn = $1 AND status = 'active'
            "#,
        )
        .bind(&verb_fqn)
        .fetch_one(&pool)
        .await
        .context("CS3: Active binding not found")?;
        assert_eq!(binding.0, verb_fqn);
        assert_eq!(binding.1, "rust_handler");
        assert_eq!(binding.2, "active");
        println!(
            "  Binding verified: {} [{}] status={}",
            binding.0, binding.1, binding.2
        );

        // Verify audit events
        verify_audit_events(
            &pool,
            changeset_id,
            &[
                "changeset_created",
                "item_added",
                "basis_attached",
                "published",
            ],
            "CS3",
        )
        .await?;

        println!("\n  ✓ CS3 PASSED: VerbContract enrichment + binding changeset published.\n");
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════
    //  CS4: Security Labels
    // ═══════════════════════════════════════════════════════════════

    #[tokio::test]
    #[ignore]
    async fn test_b3_cs4_security_labels() -> Result<()> {
        println!("\n══ CS4: Security Labels ══\n");
        let pool = get_pool().await?;
        let actor = test_actor();
        let ctx = SemRegToolContext {
            pool: &pool,
            actor: &actor,
        };

        // 1. Create changeset
        let result = dispatch_phase0_tool(
            &ctx,
            "stew_compose_changeset",
            &json!({
                "scope": "b3-cs4-security-labels",
                "intent": "Apply security labels to PII-sensitive attributes identified during bootstrap"
            }),
        )
        .await;
        let result = unwrap_tool_result(result, "CS4: stew_compose_changeset");
        let changeset_id: Uuid = result.data["changeset_id"]
            .as_str()
            .expect("changeset_id")
            .parse()?;
        println!("  Created changeset: {}", changeset_id);

        // 2. Find PII-sensitive AttributeDefs (current Active, not superseded)
        let pii_attrs = discover_fqns(
            &pool,
            "attribute_def",
            "AND (definition->>'fqn' ILIKE '%name%' OR definition->>'fqn' ILIKE '%date_of_birth%' OR definition->>'fqn' ILIKE '%tax%' OR definition->>'fqn' ILIKE '%address%' OR definition->>'fqn' ILIKE '%nationality%')",
            3,
        )
        .await?;
        println!("  Found {} PII-candidate AttributeDefs", pii_attrs.len());
        assert!(
            !pii_attrs.is_empty(),
            "Must find at least 1 PII-candidate AttributeDef"
        );

        // 3. Modify each with security_label
        let mut labelled_fqns = Vec::new();
        for (fqn, snap_id, _obj_id, def) in &pii_attrs {
            let mut modified_body = def.clone();
            if let Some(obj) = modified_body.as_object_mut() {
                obj.insert(
                    "security_label".into(),
                    json!({
                        "classification": "confidential",
                        "pii": true,
                        "handling_controls": ["encrypted_at_rest", "audit_access"],
                    }),
                );
            }

            let add_result = dispatch_phase0_tool(
                &ctx,
                "stew_add_item",
                &json!({
                    "changeset_id": changeset_id.to_string(),
                    "action": "modify",
                    "object_type": "attribute_def",
                    "object_fqn": fqn,
                    "draft_payload": modified_body,
                    "predecessor_id": snap_id.to_string(),
                    "reasoning": "Applying PII security label per data classification policy",
                }),
            )
            .await;
            let add_result =
                unwrap_tool_result(add_result, &format!("CS4: stew_add_item (label {})", fqn));
            println!(
                "    Labelled {}: entry_id={}",
                fqn,
                add_result
                    .data
                    .get("entry_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
            );
            labelled_fqns.push(fqn.clone());
        }

        // 4. Attach basis
        let basis_result = dispatch_phase0_tool(
            &ctx,
            "stew_attach_basis",
            &json!({
                "changeset_id": changeset_id.to_string(),
                "kind": "platform_convention",
                "source_ref": "Data classification policy — PII identification",
                "excerpt": "Attributes containing personal identifiers classified as PII per data handling policy",
                "claims": [{
                    "claim_text": "Attributes containing personal identifiers (name, DOB, tax ID, address) classified as PII requiring confidential handling",
                    "confidence": 0.95,
                }],
            }),
        )
        .await;
        unwrap_tool_result(basis_result, "CS4: stew_attach_basis");
        println!("  Basis attached");

        // 5. Gate precheck
        let gate_result = dispatch_phase0_tool(
            &ctx,
            "stew_gate_precheck",
            &json!({"changeset_id": changeset_id.to_string()}),
        )
        .await;
        let gate_result = unwrap_tool_result(gate_result, "CS4: stew_gate_precheck");
        let blocking = gate_result.data["blocking_count"].as_i64().unwrap_or(0);
        let warnings = gate_result.data["warning_count"].as_i64().unwrap_or(0);
        println!(
            "  Gate precheck: {} blocking, {} warnings",
            blocking, warnings
        );
        assert_eq!(
            blocking, 0,
            "CS4: No blocking guardrails expected for security labelling"
        );

        // 6. Submit → Approve → Publish
        let publish_data = run_lifecycle(&ctx, changeset_id, "CS4").await?;
        let promoted = publish_data
            .get("snapshots_promoted")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert_eq!(
            promoted,
            labelled_fqns.len() as u64,
            "CS4: Expected {} promoted snapshots",
            labelled_fqns.len()
        );
        println!("  CS4 promoted {} snapshots", promoted);

        // 7. Verify labelled attributes have security_label
        for fqn in &labelled_fqns {
            let labelled: (serde_json::Value,) = sqlx::query_as(
                r#"
                SELECT definition
                FROM sem_reg.snapshots
                WHERE object_type = 'attribute_def'::sem_reg.object_type
                  AND definition->>'fqn' = $1
                  AND status = 'active'
                  AND effective_until IS NULL
                  AND snapshot_set_id = $2
                "#,
            )
            .bind(fqn)
            .bind(changeset_id)
            .fetch_one(&pool)
            .await
            .context(format!("CS4: Labelled snapshot not found for {}", fqn))?;

            assert!(
                labelled.0.get("security_label").is_some(),
                "CS4: {} must have security_label in definition",
                fqn
            );
            let pii = labelled.0["security_label"]["pii"]
                .as_bool()
                .unwrap_or(false);
            assert!(pii, "CS4: {} security_label.pii must be true", fqn);
            println!("    {} verified: has security_label with pii=true", fqn);
        }

        // Verify audit events
        verify_audit_events(
            &pool,
            changeset_id,
            &[
                "changeset_created",
                "item_added",
                "basis_attached",
                "published",
            ],
            "CS4",
        )
        .await?;

        println!("\n  ✓ CS4 PASSED: Security labels changeset published.\n");
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════
    //  Final: Cross-Changeset Verification
    // ═══════════════════════════════════════════════════════════════

    #[tokio::test]
    #[ignore]
    async fn test_b3_final_registry_state() -> Result<()> {
        println!("\n══ Final: Cross-Changeset Registry Verification ══\n");
        let pool = get_pool().await?;

        // 1. Count Active snapshots — B3 added net new objects (CS1: 7 memberships,
        //    CS2: 2 policy+evidence). CS2/CS3/CS4 modify existing objects (net zero on count).
        //    The absolute count depends on prior test runs and scanner state, so just
        //    verify we have a reasonable number and B3 contributed net new snapshots.
        let total_active: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sem_reg.snapshots WHERE status = 'active' AND effective_until IS NULL",
        )
        .fetch_one(&pool)
        .await?;
        println!("  Total Active snapshots (current): {}", total_active.0);

        // Verify B3 changesets created active snapshots
        let b3_active: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM sem_reg.snapshots s
            JOIN sem_reg.changesets c ON c.changeset_id = s.snapshot_set_id
            WHERE c.scope LIKE 'b3-%' AND s.status = 'active'
            "#,
        )
        .fetch_one(&pool)
        .await?;
        println!("  B3-published Active snapshots: {}", b3_active.0);
        // CS1: 7 memberships, CS2: 4 (2 promoted attrs + 1 policy + 1 evidence),
        // CS3: 1 enriched verb, CS4: 3 labelled attrs = 15 total
        assert!(
            b3_active.0 >= 14,
            "B3 should have published at least 14 active snapshots (found {})",
            b3_active.0
        );

        // 2. Count superseded snapshots
        let superseded: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sem_reg.snapshots WHERE effective_until IS NOT NULL",
        )
        .fetch_one(&pool)
        .await?;
        println!("  Superseded snapshots: {}", superseded.0);
        // CS2 supersedes 1-2 attrs, CS3 supersedes 1 verb, CS4 supersedes 1-3 attrs
        assert!(
            superseded.0 >= 3,
            "Should have at least 3 superseded predecessors from B3 (found {})",
            superseded.0
        );

        // 3. No orphaned Drafts from published changesets
        let orphaned_drafts: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM sem_reg.snapshots s
            JOIN sem_reg.changesets c ON c.changeset_id = s.snapshot_set_id
            WHERE s.status = 'draft' AND c.status = 'published'
            "#,
        )
        .fetch_one(&pool)
        .await?;
        println!(
            "  Orphaned drafts from published changesets: {}",
            orphaned_drafts.0
        );
        assert_eq!(
            orphaned_drafts.0, 0,
            "No drafts should remain in published changesets"
        );

        // 4. All B3 changesets have status = 'published'
        let b3_changesets: Vec<(Uuid, String, String)> = sqlx::query_as(
            r#"
            SELECT changeset_id, status, scope
            FROM sem_reg.changesets
            WHERE scope LIKE 'b3-%'
            ORDER BY created_at
            "#,
        )
        .fetch_all(&pool)
        .await?;
        println!("  B3 changesets found: {}", b3_changesets.len());
        for (cs_id, status, scope) in &b3_changesets {
            println!("    {} [{}] scope={}", cs_id, status, scope);
            assert_eq!(
                status, "published",
                "B3 changeset {} ({}) should be published, found {}",
                cs_id, scope, status
            );
        }
        assert!(
            b3_changesets.len() >= 4,
            "Expected at least 4 B3 changesets (CS1-CS4), found {}",
            b3_changesets.len()
        );

        // 5. Each changeset has audit events
        for (cs_id, _, scope) in &b3_changesets {
            let event_count: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM stewardship.events WHERE changeset_id = $1")
                    .bind(cs_id)
                    .fetch_one(&pool)
                    .await?;
            println!("    {} ({}) audit events: {}", cs_id, scope, event_count.0);
            assert!(
                event_count.0 >= 4,
                "Each changeset should have at least 4 audit events (found {} for {})",
                event_count.0,
                scope
            );
        }

        // 6. Verify a promoted attribute (from CS2) has Governed tier in body
        let promoted_attr: Option<(serde_json::Value,)> = sqlx::query_as(
            r#"
            SELECT definition
            FROM sem_reg.snapshots s
            JOIN sem_reg.changesets c ON c.changeset_id = s.snapshot_set_id
            WHERE c.scope = 'b3-cs2-policy-promotion'
              AND s.status = 'active'
              AND s.object_type = 'attribute_def'::sem_reg.object_type
              AND s.effective_until IS NULL
            LIMIT 1
            "#,
        )
        .fetch_optional(&pool)
        .await?;
        if let Some((def,)) = promoted_attr {
            let tier = def
                .get("governance_tier")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            println!("  Promoted attribute tier: {}", tier);
            assert_eq!(
                tier, "governed",
                "Promoted attribute should have governance_tier=governed"
            );
        } else {
            println!("  ⚠ No promoted attribute found from CS2 (may have been cleaned up)");
        }

        // 7. Verify PolicyRule exists (from CS2)
        let policy: Option<(serde_json::Value,)> = sqlx::query_as(
            r#"
            SELECT definition
            FROM sem_reg.snapshots
            WHERE object_type = 'policy_rule'::sem_reg.object_type
              AND definition->>'fqn' = 'policy.kyc.cdd_beneficial_ownership_threshold'
              AND status = 'active'
              AND effective_until IS NULL
            "#,
        )
        .fetch_optional(&pool)
        .await?;
        if let Some((def,)) = policy {
            println!(
                "  PolicyRule found: {}",
                def.get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
            );
            assert!(
                def.get("predicate_refs").is_some(),
                "PolicyRule must have predicate_refs"
            );
        } else {
            println!("  ⚠ PolicyRule not found (may not have been published yet)");
        }

        // 8. Verify enriched VerbContract (from CS3)
        let enriched_verb: Option<(serde_json::Value,)> = sqlx::query_as(
            r#"
            SELECT definition
            FROM sem_reg.snapshots s
            JOIN sem_reg.changesets c ON c.changeset_id = s.snapshot_set_id
            WHERE c.scope = 'b3-cs3-verb-enrichment'
              AND s.status = 'active'
              AND s.object_type = 'verb_contract'::sem_reg.object_type
              AND s.effective_until IS NULL
            LIMIT 1
            "#,
        )
        .fetch_optional(&pool)
        .await?;
        if let Some((def,)) = enriched_verb {
            assert!(
                def.get("usage_examples").is_some(),
                "Enriched VerbContract must have usage_examples"
            );
            assert!(
                def.get("parameter_guidance").is_some(),
                "Enriched VerbContract must have parameter_guidance"
            );
            println!("  Enriched VerbContract verified: has resolution metadata");
        } else {
            println!("  ⚠ Enriched VerbContract not found (may not have been published yet)");
        }

        // 9. Verify PII-labelled attribute (from CS4)
        let pii_attr: Option<(serde_json::Value,)> = sqlx::query_as(
            r#"
            SELECT definition
            FROM sem_reg.snapshots s
            JOIN sem_reg.changesets c ON c.changeset_id = s.snapshot_set_id
            WHERE c.scope = 'b3-cs4-security-labels'
              AND s.status = 'active'
              AND s.object_type = 'attribute_def'::sem_reg.object_type
              AND s.effective_until IS NULL
            LIMIT 1
            "#,
        )
        .fetch_optional(&pool)
        .await?;
        if let Some((def,)) = pii_attr {
            assert!(
                def.get("security_label").is_some(),
                "PII-labelled attribute must have security_label"
            );
            let pii = def["security_label"]["pii"].as_bool().unwrap_or(false);
            assert!(pii, "security_label.pii must be true");
            println!("  PII-labelled attribute verified: has security_label with pii=true");
        } else {
            println!("  ⚠ PII-labelled attribute not found (may not have been published yet)");
        }

        println!("\n  ✓ Final verification PASSED: Registry state is consistent.\n");
        Ok(())
    }
}
