//! Sequencer cross-step atomicity baseline tests (Phase B.2b TDD)
//!
//! These tests pin the CURRENT behavior of multi-step runbook execution,
//! which is **per-step atomic but cross-step NON-atomic**. Each step opens
//! and commits its own transaction; a failure in step N does NOT roll back
//! steps 1..N-1.
//!
//! Phase B.2b hoists transaction ownership to the Sequencer so the whole
//! runbook runs under one outer transaction. When B.2b lands:
//!
//! 1. The `POST_B2B_*` assertion comments in each test become the active
//!    assertions.
//! 2. The `CURRENT_*` assertions must fail (because cross-step rollback
//!    now actually happens).
//!
//! The test file is structured so the flip is mechanical: search for
//! `// POST_B2B:` comments and invert. See `sequencer_cross_step_atomicity_test_flip`
//! below — it's a compile-time sentinel that documents the flip.
//!
//! Why baseline-first? Without these tests, the B.2b migration could
//! silently break or silently NOT deliver cross-step atomicity, and
//! nobody would notice until production. These tests make the behavior
//! observable.

#[cfg(feature = "database")]
mod cross_step_tests {
    use anyhow::Result;
    use sqlx::PgPool;
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use uuid::Uuid;

    use ob_poc::repl::executor_bridge::RealDslExecutor;
    use ob_poc::runbook::{
        execute_runbook, CompiledRunbook, CompiledStep, DslStepExecutor, ExecutionMode,
        ReplayEnvelope, RunbookStore, RunbookStoreBackend, StepOutcome,
    };

    // ─────────────────────────────────────────────────────────────────────
    // TestDb — mirrors transaction_rollback_integration.rs
    // ─────────────────────────────────────────────────────────────────────

    struct TestDb {
        pool: PgPool,
        prefix: String,
    }

    impl TestDb {
        async fn new() -> Result<Self> {
            let url = std::env::var("TEST_DATABASE_URL")
                .or_else(|_| std::env::var("DATABASE_URL"))
                .unwrap_or_else(|_| "postgresql:///data_designer".into());
            let pool = PgPool::connect(&url).await?;
            let prefix = format!("b2btest_{}", &Uuid::new_v4().to_string()[..8]);
            Ok(Self { pool, prefix })
        }

        fn name(&self, base: &str) -> String {
            format!("{}_{}", self.prefix, base)
        }

        async fn cbu_exists(&self, name: &str) -> Result<bool> {
            let row: (i64,) =
                sqlx::query_as(r#"SELECT COUNT(*) FROM "ob-poc".cbus WHERE name = $1"#)
                    .bind(name)
                    .fetch_one(&self.pool)
                    .await?;
            Ok(row.0 > 0)
        }

        async fn cleanup(&self) -> Result<()> {
            let pattern = format!("{}%", self.prefix);
            sqlx::query(
                r#"DELETE FROM "ob-poc".cbu_entity_roles WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();
            sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE name LIKE $1"#)
                .bind(&pattern)
                .execute(&self.pool)
                .await
                .ok();
            Ok(())
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Helpers — construct a runbook step with raw DSL
    // ─────────────────────────────────────────────────────────────────────

    fn make_step(dsl: &str, verb: &str) -> CompiledStep {
        CompiledStep {
            step_id: Uuid::new_v4(),
            sentence: format!("B.2b-test: {verb}"),
            verb: verb.into(),
            dsl: dsl.into(),
            args: BTreeMap::new(),
            depends_on: vec![],
            execution_mode: ExecutionMode::Sync,
            write_set: vec![],
            verb_contract_snapshot_id: None,
        }
    }

    async fn run_two_step_runbook(
        pool: PgPool,
        step1_dsl: &str,
        step2_dsl: &str,
    ) -> Result<Vec<StepOutcome>> {
        let store = RunbookStore::new();
        let steps = vec![
            make_step(step1_dsl, "cbu.ensure"),
            make_step(step2_dsl, "cbu.assign-role"),
        ];
        let rb = CompiledRunbook::new(Uuid::new_v4(), 1, steps, ReplayEnvelope::empty());
        let id = rb.id;
        store.insert(&rb).await?;

        let dsl_exec = Arc::new(RealDslExecutor::new(pool));
        let step_exec = DslStepExecutor::new(dsl_exec);
        let result = execute_runbook(&store, id, None, &step_exec).await?;
        Ok(result.step_results.into_iter().map(|r| r.outcome).collect())
    }

    // ─────────────────────────────────────────────────────────────────────
    // Test 1: Two-step runbook, step 2 fails — step 1's write STAYS
    // committed in the current per-step-atomic architecture.
    //
    // POST_B2B: step 1's write is rolled back along with the failed step 2.
    // ─────────────────────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_cross_step_rollback_two_steps() -> Result<()> {
        let db = TestDb::new().await?;
        let cbu_name = db.name("fund_two_step");

        // Step 1: creates a CBU (should succeed).
        let step1_dsl = format!(
            r#"(cbu.ensure :name "{}" :jurisdiction "LU")"#,
            cbu_name
        );
        // Step 2: assign a role with an INVALID role code — triggers a
        // CRUD validation / FK failure.
        let step2_dsl = format!(
            r#"(cbu.ensure :name "{}" :jurisdiction "LU" :as @fund)
               (entity.create :entity-type "proper-person" :first-name "{}" :last-name "Smith" :as @person)
               (cbu.assign-role :cbu-id @fund :entity-id @person :role "INVALID_ROLE_XYZ_B2B")"#,
            cbu_name,
            db.name("person")
        );

        let outcomes = run_two_step_runbook(db.pool.clone(), &step1_dsl, &step2_dsl).await?;

        // Step 1: Completed. Step 2: Failed.
        assert!(
            matches!(outcomes[0], StepOutcome::Completed { .. }),
            "step 1 must complete, got: {:?}",
            outcomes[0]
        );
        assert!(
            matches!(outcomes[1], StepOutcome::Failed { .. }),
            "step 2 must fail, got: {:?}",
            outcomes[1]
        );

        // CURRENT behavior (pre-B.2b): step 1's CBU is still committed.
        // POST_B2B: this assertion must flip to `!db.cbu_exists(...)`
        // because the outer transaction rolls back when step 2 fails.
        let cbu_found = db.cbu_exists(&cbu_name).await?;
        assert!(
            cbu_found,
            "CURRENT: CBU '{}' SHOULD exist after step 2 fails — \
             cross-step atomicity is NOT provided pre-B.2b. \
             POST_B2B: this assertion flips to assert!(!cbu_found, ...).",
            cbu_name
        );

        db.cleanup().await?;
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────
    // Test 2: Three-step runbook, step 3 fails — both prior steps stay
    // committed. Documents that non-atomicity compounds with step count.
    // All three steps use cbu.ensure (crud) — no entity-type resolution.
    // ─────────────────────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_cross_step_rollback_three_steps() -> Result<()> {
        let db = TestDb::new().await?;
        let cbu_a = db.name("fund_a");
        let cbu_b = db.name("fund_b");

        let step1_dsl = format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU")"#, cbu_a);
        let step2_dsl = format!(r#"(cbu.ensure :name "{}" :jurisdiction "IE")"#, cbu_b);
        // Step 3 fails: assigning a role with an obviously invalid code.
        // We reference CBU A by name via cbu.ensure to re-bind @fund,
        // then try an INVALID role, which triggers FK / enum validation.
        let step3_dsl = format!(
            r#"(cbu.ensure :name "{}" :as @fund)
               (cbu.assign-role :cbu-id @fund :entity-id "00000000-0000-0000-0000-000000000000" :role "TOTALLY_BAD_ROLE_B2B")"#,
            cbu_a
        );

        let store = RunbookStore::new();
        let steps = vec![
            make_step(&step1_dsl, "cbu.ensure"),
            make_step(&step2_dsl, "cbu.ensure"),
            make_step(&step3_dsl, "cbu.assign-role"),
        ];
        let rb = CompiledRunbook::new(Uuid::new_v4(), 1, steps, ReplayEnvelope::empty());
        let id = rb.id;
        store.insert(&rb).await?;

        let dsl_exec = Arc::new(RealDslExecutor::new(db.pool.clone()));
        let step_exec = DslStepExecutor::new(dsl_exec);
        let result = execute_runbook(&store, id, None, &step_exec).await?;

        let outcomes: Vec<_> = result.step_results.iter().map(|r| &r.outcome).collect();
        assert!(
            matches!(outcomes[0], StepOutcome::Completed { .. }),
            "step 1 must complete, got: {:?}",
            outcomes[0]
        );
        assert!(
            matches!(outcomes[1], StepOutcome::Completed { .. }),
            "step 2 must complete, got: {:?}",
            outcomes[1]
        );
        assert!(
            matches!(outcomes[2], StepOutcome::Failed { .. }),
            "step 3 must fail, got: {:?}",
            outcomes[2]
        );

        // CURRENT: both prior steps' writes committed.
        // POST_B2B: both assertions flip — outer txn rolls back all three.
        let cbu_a_found = db.cbu_exists(&cbu_a).await?;
        let cbu_b_found = db.cbu_exists(&cbu_b).await?;
        assert!(
            cbu_a_found,
            "CURRENT: CBU A from step 1 SHOULD exist after step 3 fails. \
             POST_B2B: flips to !cbu_a_found."
        );
        assert!(
            cbu_b_found,
            "CURRENT: CBU B from step 2 SHOULD exist after step 3 fails. \
             POST_B2B: flips to !cbu_b_found."
        );

        db.cleanup().await?;
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────
    // Test 3: Happy path — all steps succeed, all writes committed.
    // This test's assertions do NOT flip post-B.2b (commit is commit).
    // ─────────────────────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_all_steps_succeed_all_writes_committed() -> Result<()> {
        let db = TestDb::new().await?;
        let cbu_a = db.name("happy_a");
        let cbu_b = db.name("happy_b");

        let step1_dsl = format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU")"#, cbu_a);
        let step2_dsl = format!(r#"(cbu.ensure :name "{}" :jurisdiction "IE")"#, cbu_b);

        let outcomes = run_two_step_runbook(db.pool.clone(), &step1_dsl, &step2_dsl).await?;
        assert!(matches!(outcomes[0], StepOutcome::Completed { .. }));
        assert!(matches!(outcomes[1], StepOutcome::Completed { .. }));

        assert!(
            db.cbu_exists(&cbu_a).await?,
            "CBU A from step 1 must exist after successful runbook commit"
        );
        assert!(
            db.cbu_exists(&cbu_b).await?,
            "CBU B from step 2 must exist after successful runbook commit"
        );

        db.cleanup().await?;
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────
    // Test 4: Cross-step atomicity WORKS when the runbook runs under a
    // caller-owned scope via execute_runbook_in_scope (Phase B.2b-ε).
    //
    // This is the positive B.2b-ε proof: same 2-step shape as test 1
    // (step 2 fails after step 1 succeeds), but the test drives the
    // scope-aware runbook path directly. Caller rolls back the scope
    // when the final status is Failed → step 1's write is gone.
    // ─────────────────────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_in_scope_rollback_delivers_cross_step_atomicity() -> Result<()> {
        use ob_poc::runbook::{execute_runbook_in_scope, CompiledRunbookStatus};
        use ob_poc::sequencer_tx::PgTransactionScope;

        let db = TestDb::new().await?;
        let cbu_name = db.name("fund_in_scope");

        let step1_dsl = format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU")"#, cbu_name);
        let step2_dsl = format!(
            r#"(cbu.ensure :name "{}" :as @fund)
               (cbu.assign-role :cbu-id @fund :entity-id "00000000-0000-0000-0000-000000000000" :role "BAD_ROLE_IN_SCOPE")"#,
            cbu_name
        );

        let store = RunbookStore::new();
        let steps = vec![
            make_step(&step1_dsl, "cbu.ensure"),
            make_step(&step2_dsl, "cbu.assign-role"),
        ];
        let rb = CompiledRunbook::new(Uuid::new_v4(), 1, steps, ReplayEnvelope::empty());
        let id = rb.id;
        store.insert(&rb).await?;

        let dsl_exec = Arc::new(RealDslExecutor::new(db.pool.clone()));
        let step_exec = DslStepExecutor::new(dsl_exec);

        // Open outer scope. Run runbook. Caller (this test) owns commit/rollback.
        let mut scope = PgTransactionScope::begin(&db.pool).await?;
        let result = {
            let scope_dyn: &mut dyn dsl_runtime::tx::TransactionScope = &mut scope;
            execute_runbook_in_scope(&store, id, None, &step_exec, scope_dyn).await?
        };

        // Final status: Failed (step 2 bombed).
        assert!(
            matches!(result.final_status, CompiledRunbookStatus::Failed { .. }),
            "expected Failed final status, got {:?}",
            result.final_status
        );

        // ROLL BACK the scope — simulates the Sequencer's B.2b-ζ behavior
        // when a runbook fails.
        scope.rollback().await?;

        // Step 1's CBU write was inside the outer scope, so rollback
        // undoes it. Cross-step atomicity delivered.
        let cbu_found = db.cbu_exists(&cbu_name).await?;
        assert!(
            !cbu_found,
            "CBU '{}' MUST NOT exist after scope rollback — \
             cross-step atomicity via execute_runbook_in_scope is the \
             B.2b-ε contract.",
            cbu_name
        );

        db.cleanup().await?;
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────
    // Test 5: in-scope happy path — all steps succeed, caller commits,
    // all writes visible post-commit.
    // ─────────────────────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_in_scope_commit_persists_writes() -> Result<()> {
        use ob_poc::runbook::{execute_runbook_in_scope, CompiledRunbookStatus};
        use ob_poc::sequencer_tx::PgTransactionScope;

        let db = TestDb::new().await?;
        let cbu_a = db.name("scope_happy_a");
        let cbu_b = db.name("scope_happy_b");

        let step1_dsl = format!(r#"(cbu.ensure :name "{}" :jurisdiction "LU")"#, cbu_a);
        let step2_dsl = format!(r#"(cbu.ensure :name "{}" :jurisdiction "IE")"#, cbu_b);

        let store = RunbookStore::new();
        let steps = vec![
            make_step(&step1_dsl, "cbu.ensure"),
            make_step(&step2_dsl, "cbu.ensure"),
        ];
        let rb = CompiledRunbook::new(Uuid::new_v4(), 1, steps, ReplayEnvelope::empty());
        let id = rb.id;
        store.insert(&rb).await?;

        let dsl_exec = Arc::new(RealDslExecutor::new(db.pool.clone()));
        let step_exec = DslStepExecutor::new(dsl_exec);

        let mut scope = PgTransactionScope::begin(&db.pool).await?;
        let result = {
            let scope_dyn: &mut dyn dsl_runtime::tx::TransactionScope = &mut scope;
            execute_runbook_in_scope(&store, id, None, &step_exec, scope_dyn).await?
        };

        assert!(
            matches!(result.final_status, CompiledRunbookStatus::Completed { .. }),
            "expected Completed, got {:?}",
            result.final_status
        );

        scope.commit().await?;

        assert!(
            db.cbu_exists(&cbu_a).await?,
            "CBU A must exist after scope commit"
        );
        assert!(
            db.cbu_exists(&cbu_b).await?,
            "CBU B must exist after scope commit"
        );

        db.cleanup().await?;
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────
    // Sentinel: a compile-time reminder that these tests have a flip
    // contract. When B.2b migration lands, flip the assertions in
    // tests 1 and 2; test 3 is the positive control and does NOT flip.
    //
    // Keep this module present so grep for `POST_B2B` surfaces the
    // migration punch list from a single spot.
    // ─────────────────────────────────────────────────────────────────────

    #[allow(dead_code)]
    const POST_B2B_FLIP_CHECKLIST: &str = r#"
    Post-B.2b migration checklist (flip assertions):

    - test_cross_step_rollback_two_steps:
        cbu_found → !cbu_found
    - test_cross_step_rollback_three_steps:
        cbu_found → !cbu_found
        entity_found → !entity_found
    - test_all_steps_succeed_all_writes_committed:
        NO CHANGE (positive control)

    Also: remove `CURRENT:` comment markers; keep `POST_B2B:` prose
    promoted to primary assertion docs.
    "#;
}
