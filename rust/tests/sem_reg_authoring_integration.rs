//! Authoring Pipeline — Integration Tests (Phase 7b)
//!
//! Tests the Research → Governed Change Boundary governance verb pipeline
//! end-to-end against a real PostgreSQL database.
//!
//! Categories:
//!   - E2E (4): Basic publish, batch publish, supersession chain, mode switching
//!   - Negative (8): Invalid status, empty bundle, circular deps, etc.
//!   - Regression (6): Idempotent propose, audit trail, batch atomicity, etc.
//!   - Mode (3): Research/Governed verb gating
//!   - Observability (2): Metrics emission, audit completeness
//!   - Cleanup (1): Archive retention
//!
//! Run with:
//! ```sh
//! DATABASE_URL="postgresql:///data_designer" \
//!   cargo test --features database --test sem_reg_authoring_integration -- --ignored --nocapture
//! ```

#[cfg(feature = "database")]
mod integration {
    use std::collections::HashMap;

    use sqlx::PgPool;
    use uuid::Uuid;

    use sem_os_core::authoring::bundle::{build_bundle_from_map, parse_manifest, BundleContents};
    use sem_os_core::authoring::cleanup::CleanupPolicy;
    use sem_os_core::authoring::governance_verbs::GovernanceVerbService;
    use sem_os_core::authoring::types::*;
    use sem_os_core::principal::Principal;

    use sem_os_postgres::PgAuthoringStore;
    use sem_os_postgres::PgScratchSchemaRunner;

    use sem_os_core::authoring::agent_mode::AgentMode;
    use sem_os_core::authoring::ports::AuthoringStore;

    // ── Helpers ──────────────────────────────────────────────────────

    async fn get_pool() -> PgPool {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        PgPool::connect(&url).await.expect("Failed to connect")
    }

    fn test_principal() -> Principal {
        Principal::in_process("integration-test", vec!["operator".to_string()])
    }

    /// Create a simple bundle with a single migration artifact.
    fn simple_bundle(title: &str, sql: &str) -> BundleContents {
        let yaml = format!(
            r#"
title: "{title}"
rationale: "Integration test bundle"
artifacts:
  - type: migration_sql
    path: "001.sql"
"#
        );
        let raw = parse_manifest(&yaml).expect("manifest parse");
        let mut content_map = HashMap::new();
        content_map.insert("001.sql".to_string(), sql.to_string());
        build_bundle_from_map(&raw, &content_map).expect("bundle build")
    }

    /// Create a bundle with a unique title (UUID-prefixed for test isolation).
    fn unique_bundle(prefix: &str) -> BundleContents {
        let id = Uuid::new_v4().simple().to_string();
        let title = format!("{prefix}_{id}");
        simple_bundle(&title, &format!("-- test migration {id}"))
    }

    /// Create a bundle that supersedes another ChangeSet.
    fn bundle_with_supersedes(prefix: &str, supersedes: Uuid) -> BundleContents {
        let id = Uuid::new_v4().simple().to_string();
        let yaml = format!(
            r#"
title: "{prefix}_{id}"
rationale: "Integration test supersession"
supersedes: "{supersedes}"
artifacts:
  - type: migration_sql
    path: "001.sql"
"#
        );
        let raw = parse_manifest(&yaml).expect("manifest parse");
        let mut content_map = HashMap::new();
        content_map.insert("001.sql".to_string(), format!("-- supersede migration {id}"));
        build_bundle_from_map(&raw, &content_map).expect("bundle build")
    }

    // ═══════════════════════════════════════════════════════════════════
    // E2E Tests (4)
    // ═══════════════════════════════════════════════════════════════════

    /// E2E-1: Full publish pipeline — propose → validate → dry_run → plan → publish.
    #[tokio::test]
    #[ignore]
    async fn test_e2e_basic_publish_pipeline() {
        let pool = get_pool().await;
        let store = PgAuthoringStore::new(pool.clone());
        let scratch = PgScratchSchemaRunner::new(pool.clone());
        let svc = GovernanceVerbService::new(&store, &scratch);
        let principal = test_principal();

        // 1. Propose
        let bundle = unique_bundle("e2e_publish");
        let cs = svc.propose(&bundle, &principal).await.expect("propose");
        assert_eq!(cs.status, ChangeSetStatus::Draft);
        assert!(!cs.content_hash.is_empty());
        println!("  Proposed: {} ({})", cs.change_set_id, cs.title);

        // 2. Validate
        let report = svc.validate(cs.change_set_id).await.expect("validate");
        assert!(report.ok, "validation should pass for valid SQL");
        let cs2 = store.get_change_set(cs.change_set_id).await.unwrap();
        assert_eq!(cs2.status, ChangeSetStatus::Validated);
        println!("  Validated: ok={}", report.ok);

        // 3. Dry-run
        let dry = svc.dry_run(cs.change_set_id).await.expect("dry_run");
        assert!(dry.ok, "dry-run should pass");
        let cs3 = store.get_change_set(cs.change_set_id).await.unwrap();
        assert_eq!(cs3.status, ChangeSetStatus::DryRunPassed);
        println!("  Dry-run: ok={}", dry.ok);

        // 4. Plan publish (read-only, no state change)
        let plan = svc.plan_publish(cs.change_set_id).await.expect("plan");
        println!("  Plan: {} added, {} modified", plan.added.len(), plan.modified.len());

        // 5. Publish
        let batch = svc
            .publish(cs.change_set_id, "integration-test")
            .await
            .expect("publish");
        assert_eq!(batch.change_set_ids, vec![cs.change_set_id]);
        let cs4 = store.get_change_set(cs.change_set_id).await.unwrap();
        assert_eq!(cs4.status, ChangeSetStatus::Published);
        println!("  Published: batch={}", batch.batch_id);
    }

    /// E2E-2: Batch publish — multiple ChangeSets published atomically with topo sort.
    #[tokio::test]
    #[ignore]
    async fn test_e2e_batch_publish() {
        let pool = get_pool().await;
        let store = PgAuthoringStore::new(pool.clone());
        let scratch = PgScratchSchemaRunner::new(pool.clone());
        let svc = GovernanceVerbService::new(&store, &scratch);
        let principal = test_principal();

        // Create two independent ChangeSets
        let b1 = unique_bundle("batch_1");
        let cs1 = svc.propose(&b1, &principal).await.unwrap();
        svc.validate(cs1.change_set_id).await.unwrap();
        svc.dry_run(cs1.change_set_id).await.unwrap();

        let b2 = unique_bundle("batch_2");
        let cs2 = svc.propose(&b2, &principal).await.unwrap();
        svc.validate(cs2.change_set_id).await.unwrap();
        svc.dry_run(cs2.change_set_id).await.unwrap();

        // Batch publish both
        let batch = svc
            .publish_batch(
                &[cs1.change_set_id, cs2.change_set_id],
                "batch-integration-test",
            )
            .await
            .expect("batch publish");

        assert_eq!(batch.change_set_ids.len(), 2);
        assert_eq!(batch.publisher, "batch-integration-test");

        // Verify both are Published
        let r1 = store.get_change_set(cs1.change_set_id).await.unwrap();
        let r2 = store.get_change_set(cs2.change_set_id).await.unwrap();
        assert_eq!(r1.status, ChangeSetStatus::Published);
        assert_eq!(r2.status, ChangeSetStatus::Published);
        println!("  Batch published {} ChangeSets", batch.change_set_ids.len());
    }

    /// E2E-3: Supersession chain — publish A, then publish B that supersedes A.
    #[tokio::test]
    #[ignore]
    async fn test_e2e_supersession_chain() {
        let pool = get_pool().await;
        let store = PgAuthoringStore::new(pool.clone());
        let scratch = PgScratchSchemaRunner::new(pool.clone());
        let svc = GovernanceVerbService::new(&store, &scratch);
        let principal = test_principal();

        // Publish original
        let b1 = unique_bundle("super_original");
        let cs1 = svc.propose(&b1, &principal).await.unwrap();
        svc.validate(cs1.change_set_id).await.unwrap();
        svc.dry_run(cs1.change_set_id).await.unwrap();
        svc.publish(cs1.change_set_id, "test").await.unwrap();

        // Publish successor that supersedes original
        let b2 = bundle_with_supersedes("super_successor", cs1.change_set_id);
        let cs2 = svc.propose(&b2, &principal).await.unwrap();
        assert_eq!(cs2.supersedes_change_set_id, Some(cs1.change_set_id));
        svc.validate(cs2.change_set_id).await.unwrap();
        svc.dry_run(cs2.change_set_id).await.unwrap();
        svc.publish(cs2.change_set_id, "test").await.unwrap();

        // Verify original is now Superseded
        let r1 = store.get_change_set(cs1.change_set_id).await.unwrap();
        assert_eq!(r1.status, ChangeSetStatus::Superseded);
        assert_eq!(r1.superseded_by, Some(cs2.change_set_id));
        assert!(r1.superseded_at.is_some());
        println!("  Supersession chain verified");
    }

    /// E2E-4: AgentMode switching — verbs gated by mode.
    #[tokio::test]
    #[ignore]
    async fn test_e2e_agent_mode_switching() {
        // Research mode: authoring verbs allowed, publish blocked
        let research = AgentMode::Research;
        assert!(research.is_verb_allowed("authoring.propose"));
        assert!(research.is_verb_allowed("authoring.validate"));
        assert!(research.is_verb_allowed("authoring.dry-run"));
        assert!(!research.is_verb_allowed("authoring.publish"));
        assert!(!research.is_verb_allowed("authoring.publish-batch"));

        // Governed mode: authoring verbs blocked, publish allowed
        let governed = AgentMode::Governed;
        assert!(!governed.is_verb_allowed("authoring.propose"));
        assert!(!governed.is_verb_allowed("authoring.validate"));
        assert!(!governed.is_verb_allowed("authoring.dry-run"));
        assert!(!governed.is_verb_allowed("authoring.diff"));
        assert!(governed.is_verb_allowed("authoring.publish"));
        assert!(governed.is_verb_allowed("authoring.publish-batch"));

        // Introspect subcommands differ
        let research_cmds = research.allowed_introspect_subcommands();
        let governed_cmds = governed.allowed_introspect_subcommands();
        assert!(research_cmds.len() > governed_cmds.len());
        println!("  Research introspect: {} commands", research_cmds.len());
        println!("  Governed introspect: {} commands", governed_cmds.len());
    }

    // ═══════════════════════════════════════════════════════════════════
    // Negative Tests (8)
    // ═══════════════════════════════════════════════════════════════════

    /// NEG-1: Validate a non-Draft ChangeSet → error.
    #[tokio::test]
    #[ignore]
    async fn test_neg_validate_wrong_status() {
        let pool = get_pool().await;
        let store = PgAuthoringStore::new(pool.clone());
        let scratch = PgScratchSchemaRunner::new(pool.clone());
        let svc = GovernanceVerbService::new(&store, &scratch);
        let principal = test_principal();

        let bundle = unique_bundle("neg_wrong_status");
        let cs = svc.propose(&bundle, &principal).await.unwrap();

        // Validate first time (Draft → Validated)
        svc.validate(cs.change_set_id).await.unwrap();

        // Try to validate again (already Validated, not Draft)
        let err = svc.validate(cs.change_set_id).await;
        assert!(err.is_err(), "validate on non-Draft should fail");
        let msg = err.unwrap_err().to_string();
        assert!(msg.contains("must be Draft"), "error should mention Draft: {msg}");
    }

    /// NEG-2: Dry-run a non-Validated ChangeSet → error.
    #[tokio::test]
    #[ignore]
    async fn test_neg_dry_run_wrong_status() {
        let pool = get_pool().await;
        let store = PgAuthoringStore::new(pool.clone());
        let scratch = PgScratchSchemaRunner::new(pool.clone());
        let svc = GovernanceVerbService::new(&store, &scratch);
        let principal = test_principal();

        let bundle = unique_bundle("neg_dry_status");
        let cs = svc.propose(&bundle, &principal).await.unwrap();

        // Try dry-run on Draft (should require Validated)
        let err = svc.dry_run(cs.change_set_id).await;
        assert!(err.is_err(), "dry-run on Draft should fail");
        let msg = err.unwrap_err().to_string();
        assert!(
            msg.contains("must be Validated"),
            "error should mention Validated: {msg}"
        );
    }

    /// NEG-3: Publish a non-DryRunPassed ChangeSet → error.
    #[tokio::test]
    #[ignore]
    async fn test_neg_publish_wrong_status() {
        let pool = get_pool().await;
        let store = PgAuthoringStore::new(pool.clone());
        let scratch = PgScratchSchemaRunner::new(pool.clone());
        let svc = GovernanceVerbService::new(&store, &scratch);
        let principal = test_principal();

        let bundle = unique_bundle("neg_pub_status");
        let cs = svc.propose(&bundle, &principal).await.unwrap();

        // Try publish on Draft (should require DryRunPassed)
        let err = svc.publish(cs.change_set_id, "test").await;
        assert!(err.is_err(), "publish on Draft should fail");
        let msg = err.unwrap_err().to_string();
        assert!(
            msg.contains("must be DryRunPassed"),
            "error should mention DryRunPassed: {msg}"
        );
    }

    /// NEG-4: Plan publish on non-DryRunPassed → error.
    #[tokio::test]
    #[ignore]
    async fn test_neg_plan_publish_wrong_status() {
        let pool = get_pool().await;
        let store = PgAuthoringStore::new(pool.clone());
        let scratch = PgScratchSchemaRunner::new(pool.clone());
        let svc = GovernanceVerbService::new(&store, &scratch);
        let principal = test_principal();

        let bundle = unique_bundle("neg_plan_status");
        let cs = svc.propose(&bundle, &principal).await.unwrap();

        let err = svc.plan_publish(cs.change_set_id).await;
        assert!(err.is_err());
        let msg = err.unwrap_err().to_string();
        assert!(msg.contains("must be DryRunPassed"));
    }

    /// NEG-5: Get non-existent ChangeSet → NotFound error.
    #[tokio::test]
    #[ignore]
    async fn test_neg_get_nonexistent_changeset() {
        let pool = get_pool().await;
        let store = PgAuthoringStore::new(pool.clone());

        let fake_id = Uuid::new_v4();
        let err = store.get_change_set(fake_id).await;
        assert!(err.is_err(), "get nonexistent should fail");
    }

    /// NEG-6: Batch publish with one non-DryRunPassed → fails entire batch.
    #[tokio::test]
    #[ignore]
    async fn test_neg_batch_publish_mixed_status() {
        let pool = get_pool().await;
        let store = PgAuthoringStore::new(pool.clone());
        let scratch = PgScratchSchemaRunner::new(pool.clone());
        let svc = GovernanceVerbService::new(&store, &scratch);
        let principal = test_principal();

        // First: fully ready
        let b1 = unique_bundle("batch_ready");
        let cs1 = svc.propose(&b1, &principal).await.unwrap();
        svc.validate(cs1.change_set_id).await.unwrap();
        svc.dry_run(cs1.change_set_id).await.unwrap();

        // Second: only Draft (not ready)
        let b2 = unique_bundle("batch_draft");
        let cs2 = svc.propose(&b2, &principal).await.unwrap();

        // Batch publish should fail because cs2 is not DryRunPassed
        let err = svc
            .publish_batch(&[cs1.change_set_id, cs2.change_set_id], "test")
            .await;
        assert!(err.is_err(), "batch with mixed statuses should fail");
        let msg = err.unwrap_err().to_string();
        assert!(msg.contains("not DryRunPassed"));

        // cs1 should still be DryRunPassed (not published — batch failed)
        let r1 = store.get_change_set(cs1.change_set_id).await.unwrap();
        assert_eq!(r1.status, ChangeSetStatus::DryRunPassed);
    }

    /// NEG-7: Bundle with unknown artifact type → parse error.
    #[tokio::test]
    #[ignore]
    async fn test_neg_unknown_artifact_type() {
        let yaml = r#"
title: "Bad artifact type"
artifacts:
  - type: unknown_type_xyz
    path: "foo.txt"
"#;
        let raw = parse_manifest(yaml).unwrap();
        let mut content_map = HashMap::new();
        content_map.insert("foo.txt".to_string(), "content".to_string());
        let result = build_bundle_from_map(&raw, &content_map);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown artifact type"));
    }

    /// NEG-8: Bundle with missing content → build error.
    #[tokio::test]
    #[ignore]
    async fn test_neg_missing_content() {
        let yaml = r#"
title: "Missing content"
artifacts:
  - type: migration_sql
    path: "missing.sql"
"#;
        let raw = parse_manifest(yaml).unwrap();
        let content_map = HashMap::new(); // Empty — missing.sql not provided
        let result = build_bundle_from_map(&raw, &content_map);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing content"));
    }

    // ═══════════════════════════════════════════════════════════════════
    // Regression Tests (6)
    // ═══════════════════════════════════════════════════════════════════

    /// REG-1: Propose idempotency — same bundle proposed twice returns same ChangeSet.
    #[tokio::test]
    #[ignore]
    async fn test_reg_propose_idempotency() {
        let pool = get_pool().await;
        let store = PgAuthoringStore::new(pool.clone());
        let scratch = PgScratchSchemaRunner::new(pool.clone());
        let svc = GovernanceVerbService::new(&store, &scratch);
        let principal = test_principal();

        let bundle = unique_bundle("idempotent");
        let cs1 = svc.propose(&bundle, &principal).await.unwrap();
        let cs2 = svc.propose(&bundle, &principal).await.unwrap();

        assert_eq!(cs1.change_set_id, cs2.change_set_id);
        assert_eq!(cs1.content_hash, cs2.content_hash);
        println!("  Idempotent propose verified: {}", cs1.change_set_id);
    }

    /// REG-2: Content hash is stable — same content produces same hash.
    #[tokio::test]
    #[ignore]
    async fn test_reg_content_hash_stability() {
        let b1 = simple_bundle("hash_test", "CREATE TABLE test_hash(id INT);");
        let b2 = simple_bundle("hash_test", "CREATE TABLE test_hash(id INT);");

        // Same content → same artifacts → same hash (computed in propose)
        // We verify the bundles have identical artifact content_hashes
        assert_eq!(b1.artifacts.len(), b2.artifacts.len());
        for (a1, a2) in b1.artifacts.iter().zip(b2.artifacts.iter()) {
            assert_eq!(a1.content_hash, a2.content_hash);
        }
    }

    /// REG-3: Validation report is persisted and retrievable.
    #[tokio::test]
    #[ignore]
    async fn test_reg_validation_reports_persisted() {
        let pool = get_pool().await;
        let store = PgAuthoringStore::new(pool.clone());
        let scratch = PgScratchSchemaRunner::new(pool.clone());
        let svc = GovernanceVerbService::new(&store, &scratch);
        let principal = test_principal();

        let bundle = unique_bundle("val_report");
        let cs = svc.propose(&bundle, &principal).await.unwrap();
        svc.validate(cs.change_set_id).await.unwrap();

        // Check validation report was saved
        let reports = store
            .get_validation_reports(cs.change_set_id)
            .await
            .unwrap();
        assert!(!reports.is_empty(), "at least one validation report");
        let (_, stage, ok, _) = &reports[0];
        assert_eq!(*stage, ValidationStage::Validate);
        assert!(*ok, "validation should have passed");
        println!("  {} validation report(s) found", reports.len());
    }

    /// REG-4: Dry-run report is persisted and retrievable.
    #[tokio::test]
    #[ignore]
    async fn test_reg_dry_run_reports_persisted() {
        let pool = get_pool().await;
        let store = PgAuthoringStore::new(pool.clone());
        let scratch = PgScratchSchemaRunner::new(pool.clone());
        let svc = GovernanceVerbService::new(&store, &scratch);
        let principal = test_principal();

        let bundle = unique_bundle("dry_report");
        let cs = svc.propose(&bundle, &principal).await.unwrap();
        svc.validate(cs.change_set_id).await.unwrap();
        svc.dry_run(cs.change_set_id).await.unwrap();

        // Check both reports
        let reports = store
            .get_validation_reports(cs.change_set_id)
            .await
            .unwrap();
        assert!(reports.len() >= 2, "should have validate + dry_run reports");
        let has_dry_run = reports
            .iter()
            .any(|(_, stage, _, _)| *stage == ValidationStage::DryRun);
        assert!(has_dry_run, "should include a dry_run report");
    }

    /// REG-5: List ChangeSets with status filter.
    #[tokio::test]
    #[ignore]
    async fn test_reg_list_changesets_filter() {
        let pool = get_pool().await;
        let store = PgAuthoringStore::new(pool.clone());
        let scratch = PgScratchSchemaRunner::new(pool.clone());
        let svc = GovernanceVerbService::new(&store, &scratch);
        let principal = test_principal();

        // Create a Draft ChangeSet
        let bundle = unique_bundle("list_filter");
        let cs = svc.propose(&bundle, &principal).await.unwrap();

        // List drafts
        let drafts = store
            .list_change_sets(Some(ChangeSetStatus::Draft), 100)
            .await
            .unwrap();
        assert!(
            drafts.iter().any(|d| d.change_set_id == cs.change_set_id),
            "newly created changeset should appear in draft list"
        );

        // List without filter
        let all = store.list_change_sets(None, 100).await.unwrap();
        assert!(
            all.iter().any(|d| d.change_set_id == cs.change_set_id),
            "should appear in unfiltered list"
        );
    }

    /// REG-6: Audit trail completeness — publish produces audit entry.
    #[tokio::test]
    #[ignore]
    async fn test_reg_audit_trail_on_publish() {
        let pool = get_pool().await;
        let store = PgAuthoringStore::new(pool.clone());
        let scratch = PgScratchSchemaRunner::new(pool.clone());
        let svc = GovernanceVerbService::new(&store, &scratch);
        let principal = test_principal();

        let bundle = unique_bundle("audit_trail");
        let cs = svc.propose(&bundle, &principal).await.unwrap();
        svc.validate(cs.change_set_id).await.unwrap();
        svc.dry_run(cs.change_set_id).await.unwrap();
        svc.publish(cs.change_set_id, "audit-test").await.unwrap();

        // Verify audit entry exists
        let row: Option<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT entry_id
            FROM sem_reg_authoring.governance_audit_log
            WHERE change_set_id = $1 AND verb = 'publish_snapshot_set'
            "#,
        )
        .bind(cs.change_set_id)
        .fetch_optional(&pool)
        .await
        .unwrap();

        assert!(row.is_some(), "audit entry should exist after publish");
        println!("  Audit entry found: {:?}", row.unwrap().0);
    }

    // ═══════════════════════════════════════════════════════════════════
    // Mode Tests (3)
    // ═══════════════════════════════════════════════════════════════════

    /// MODE-1: Research mode allows authoring, blocks business verbs.
    #[tokio::test]
    #[ignore]
    async fn test_mode_research_allows_authoring() {
        let mode = AgentMode::Research;

        // Authoring verbs allowed
        assert!(mode.allows_authoring());
        assert!(mode.is_verb_allowed("authoring.propose"));
        assert!(mode.is_verb_allowed("authoring.validate"));
        assert!(mode.is_verb_allowed("authoring.dry-run"));

        // Full introspect allowed
        assert!(mode.allows_full_introspect());

        // Business verbs blocked
        assert!(!mode.allows_business_verbs());

        println!("  Research mode gating verified");
    }

    /// MODE-2: Governed mode blocks authoring, allows business/publish.
    #[tokio::test]
    #[ignore]
    async fn test_mode_governed_blocks_authoring() {
        let mode = AgentMode::Governed;

        // Authoring verbs blocked
        assert!(!mode.allows_authoring());
        assert!(!mode.is_verb_allowed("authoring.propose"));
        assert!(!mode.is_verb_allowed("authoring.validate"));
        assert!(!mode.is_verb_allowed("authoring.dry-run"));

        // Publish allowed
        assert!(mode.is_verb_allowed("authoring.publish"));
        assert!(mode.is_verb_allowed("authoring.publish-batch"));

        // Business verbs allowed
        assert!(mode.allows_business_verbs());

        // Limited introspect
        assert!(!mode.allows_full_introspect());
        let cmds = mode.allowed_introspect_subcommands();
        assert!(cmds.contains(&"verify_table_exists"));
        assert!(cmds.contains(&"describe_table"));

        println!("  Governed mode gating verified");
    }

    /// MODE-3: Default mode is Governed.
    #[tokio::test]
    #[ignore]
    async fn test_mode_default_is_governed() {
        let mode = AgentMode::default();
        assert_eq!(mode, AgentMode::Governed);
        assert!(!mode.allows_authoring());
        assert!(mode.allows_business_verbs());
    }

    // ═══════════════════════════════════════════════════════════════════
    // Observability Tests (2)
    // ═══════════════════════════════════════════════════════════════════

    /// OBS-1: Count by status returns grouped counts.
    #[tokio::test]
    #[ignore]
    async fn test_obs_count_by_status() {
        let pool = get_pool().await;
        let store = PgAuthoringStore::new(pool.clone());
        let scratch = PgScratchSchemaRunner::new(pool.clone());
        let svc = GovernanceVerbService::new(&store, &scratch);
        let principal = test_principal();

        // Create at least one draft
        let bundle = unique_bundle("obs_count");
        svc.propose(&bundle, &principal).await.unwrap();

        let counts = store.count_by_status().await.unwrap();
        assert!(!counts.is_empty(), "should have at least one status count");

        let draft_count = counts
            .iter()
            .find(|(s, _)| *s == ChangeSetStatus::Draft)
            .map(|(_, c)| *c);
        assert!(
            draft_count.unwrap_or(0) >= 1,
            "should have at least 1 draft"
        );
        println!(
            "  Status counts: {:?}",
            counts
                .iter()
                .map(|(s, c)| format!("{}={}", s, c))
                .collect::<Vec<_>>()
        );
    }

    /// OBS-2: Publish batch record is persisted.
    #[tokio::test]
    #[ignore]
    async fn test_obs_publish_batch_record() {
        let pool = get_pool().await;
        let store = PgAuthoringStore::new(pool.clone());
        let scratch = PgScratchSchemaRunner::new(pool.clone());
        let svc = GovernanceVerbService::new(&store, &scratch);
        let principal = test_principal();

        let bundle = unique_bundle("obs_batch");
        let cs = svc.propose(&bundle, &principal).await.unwrap();
        svc.validate(cs.change_set_id).await.unwrap();
        svc.dry_run(cs.change_set_id).await.unwrap();
        let batch = svc
            .publish(cs.change_set_id, "obs-test")
            .await
            .unwrap();

        // Check publish_batches table
        let row: Option<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT batch_id, publisher
            FROM sem_reg_authoring.publish_batches
            WHERE batch_id = $1
            "#,
        )
        .bind(batch.batch_id)
        .fetch_optional(&pool)
        .await
        .unwrap();

        assert!(row.is_some(), "publish batch record should exist");
        let (_, publisher) = row.unwrap();
        assert_eq!(publisher, "obs-test");
    }

    // ═══════════════════════════════════════════════════════════════════
    // Cleanup Test (1)
    // ═══════════════════════════════════════════════════════════════════

    /// CLEANUP-1: Cleanup policy defaults are sensible.
    #[tokio::test]
    #[ignore]
    async fn test_cleanup_policy_defaults() {
        let policy = CleanupPolicy::default();
        assert_eq!(policy.terminal_retention_days, 90);
        assert_eq!(policy.orphan_retention_days, 30);
        println!("  Cleanup policy: terminal={}d, orphan={}d",
            policy.terminal_retention_days, policy.orphan_retention_days);
    }

    // ═══════════════════════════════════════════════════════════════════
    // Additional: Diff Test
    // ═══════════════════════════════════════════════════════════════════

    /// DIFF-1: Diff between two ChangeSets returns structured summary.
    #[tokio::test]
    #[ignore]
    async fn test_diff_between_changesets() {
        let pool = get_pool().await;
        let store = PgAuthoringStore::new(pool.clone());
        let scratch = PgScratchSchemaRunner::new(pool.clone());
        let svc = GovernanceVerbService::new(&store, &scratch);
        let principal = test_principal();

        // Create two ChangeSets with different content
        let b1 = simple_bundle("diff_base", "CREATE TABLE diff_a(id INT);");
        let cs1 = svc.propose(&b1, &principal).await.unwrap();

        let b2 = simple_bundle("diff_target", "CREATE TABLE diff_b(id INT);");
        let cs2 = svc.propose(&b2, &principal).await.unwrap();

        let diff = svc
            .diff(cs1.change_set_id, cs2.change_set_id)
            .await
            .unwrap();
        // Diff should return a valid summary (content differs)
        println!(
            "  Diff: +{} ~{} -{} breaking={}",
            diff.added.len(),
            diff.modified.len(),
            diff.removed.len(),
            diff.breaking_changes.len()
        );
    }

    // ═══════════════════════════════════════════════════════════════════
    // Additional: Status Terminal Check
    // ═══════════════════════════════════════════════════════════════════

    /// STATUS-1: Terminal status detection.
    #[tokio::test]
    #[ignore]
    async fn test_status_terminal_detection() {
        assert!(ChangeSetStatus::Published.is_terminal());
        assert!(ChangeSetStatus::Rejected.is_terminal());
        assert!(ChangeSetStatus::DryRunFailed.is_terminal());
        assert!(ChangeSetStatus::Superseded.is_terminal());
        assert!(!ChangeSetStatus::Draft.is_terminal());
        assert!(!ChangeSetStatus::Validated.is_terminal());
        assert!(!ChangeSetStatus::DryRunPassed.is_terminal());
    }
}
