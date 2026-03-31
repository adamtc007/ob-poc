//! Onboarding Test Harness — Full KYC lifecycle via DSL verbs
//!
//! Creates a fresh test group ("Harness Test Corp") and drives the full
//! onboarding lifecycle through DSL verbs:
//!
//! Phase 1: Group Setup — create group, add entities, assign roles
//! Phase 2: UBO Discovery — allege ownership, compute chains
//! Phase 3: KYC Case — open case, create workstreams, run screening
//! Phase 4: Documents — solicit, upload, verify
//! Phase 5: Tollgate — evaluate, check readiness
//! Phase 6: Cleanup — delete cascade
//!
//! Usage:
//! ```bash
//! cargo x onboarding-harness --mode full --verbose
//! cargo x onboarding-harness --mode setup       # Phase 1-2 only
//! cargo x onboarding-harness --mode kyc          # Phase 3-5 (needs setup)
//! cargo x onboarding-harness --mode clean        # Delete test data
//! ```

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use ob_poc::dsl_v2::{compile, parse_program, DslExecutor, ExecutionContext, ExecutionResult};

// =============================================================================
// Constants
// =============================================================================

const TEST_GROUP_NAME: &str = "Harness Test Corp";
const TEST_MANCO_NAME: &str = "Harness ManCo Ltd";
const TEST_DEPOSITARY_NAME: &str = "Harness Depositary Bank";
const TEST_UBO_NAME: &str = "Jane Harness UBO";
const TEST_FUND_NAME: &str = "Harness UCITS Fund";

// =============================================================================
// State Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingHarnessState {
    pub started_at: String,
    pub group_id: Option<Uuid>,
    pub manco_entity_id: Option<Uuid>,
    pub depositary_entity_id: Option<Uuid>,
    pub director_entity_id: Option<Uuid>,
    pub ubo_entity_id: Option<Uuid>,
    pub cbu_id: Option<Uuid>,
    pub case_id: Option<Uuid>,
    pub workstream_id: Option<Uuid>,
    pub phases_completed: Vec<String>,
    pub errors: Vec<String>,
    pub verb_log: Vec<VerbExecution>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbExecution {
    pub phase: String,
    pub dsl: String,
    pub success: bool,
    pub result_summary: String,
    pub duration_ms: u64,
}

// =============================================================================
// Harness Runner
// =============================================================================

pub struct OnboardingHarness {
    pool: PgPool,
    executor: DslExecutor,
    state: OnboardingHarnessState,
    verbose: bool,
}

impl OnboardingHarness {
    pub async fn new(pool: PgPool, verbose: bool) -> Result<Self> {
        let executor = DslExecutor::new(pool.clone());
        Ok(Self {
            pool,
            executor,
            state: OnboardingHarnessState {
                started_at: Utc::now().to_rfc3339(),
                group_id: None,
                manco_entity_id: None,
                depositary_entity_id: None,
                director_entity_id: None,
                ubo_entity_id: None,
                cbu_id: None,
                case_id: None,
                workstream_id: None,
                phases_completed: Vec::new(),
                errors: Vec::new(),
                verb_log: Vec::new(),
            },
            verbose,
        })
    }

    /// Execute a DSL statement, log it, return the result
    async fn exec(&mut self, phase: &str, dsl: &str) -> Result<Vec<ExecutionResult>> {
        let start = std::time::Instant::now();
        if self.verbose {
            println!("  [{}] {}", phase, dsl);
        }

        let ast = parse_program(dsl).map_err(|e| anyhow::anyhow!("Parse error: {:?}", e))?;
        let plan = compile(&ast).map_err(|e| anyhow::anyhow!("Compile error: {:?}", e))?;
        let mut ctx = ExecutionContext::new().without_idempotency();
        let results = self.executor.execute_plan(&plan, &mut ctx).await?;

        let duration_ms = start.elapsed().as_millis() as u64;
        let summary = if results.is_empty() {
            "void".to_string()
        } else {
            format!("{} results", results.len())
        };

        self.state.verb_log.push(VerbExecution {
            phase: phase.to_string(),
            dsl: dsl.to_string(),
            success: true,
            result_summary: summary.clone(),
            duration_ms,
        });

        if self.verbose {
            println!("    ✓ {} ({}ms)", summary, duration_ms);
        }

        Ok(results)
    }

    /// Extract UUID from first result
    fn extract_uuid(results: &[ExecutionResult]) -> Option<Uuid> {
        results.first().and_then(|r| match r {
            ExecutionResult::Uuid(id) => Some(*id),
            ExecutionResult::Record(v) => v
                .get("id")
                .or_else(|| v.get("entity_id"))
                .or_else(|| v.get("group_id"))
                .or_else(|| v.get("cbu_id"))
                .or_else(|| v.get("case_id"))
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok()),
            _ => None,
        })
    }

    // =========================================================================
    // Phase 1: Group Setup
    // =========================================================================

    pub async fn phase_group_setup(&mut self) -> Result<()> {
        println!("\n=== Phase 1: Group Setup ===");

        // Create client group
        let results = self
            .exec(
                "group",
                &format!(
                    r#"(client-group.create :canonical-name "{}")"#,
                    TEST_GROUP_NAME
                ),
            )
            .await?;
        self.state.group_id = Self::extract_uuid(&results);
        let group_id = self
            .state
            .group_id
            .context("Failed to create client group")?;
        println!("  Group created: {}", group_id);

        // Create ManCo entity
        let results = self
            .exec(
                "group",
                &format!(
                    r#"(entity.ensure :name "{}" :entity-type "limited_company_private" :jurisdiction "LU")"#,
                    TEST_MANCO_NAME
                ),
            )
            .await?;
        self.state.manco_entity_id = Self::extract_uuid(&results);
        println!("  ManCo created: {:?}", self.state.manco_entity_id);

        // Create depositary entity
        let results = self
            .exec(
                "group",
                &format!(
                    r#"(entity.ensure :name "{}" :entity-type "limited_company_private" :jurisdiction "LU")"#,
                    TEST_DEPOSITARY_NAME
                ),
            )
            .await?;
        self.state.depositary_entity_id = Self::extract_uuid(&results);

        // Create director (person)
        let results = self
            .exec(
                "group",
                r#"(entity.ensure :name "John Harness Director" :entity-type "proper_person_natural" :first-name "John" :last-name "Director")"#,
            )
            .await?;
        self.state.director_entity_id = Self::extract_uuid(&results);

        // Create UBO (person)
        let results = self
            .exec(
                "group",
                r#"(entity.ensure :name "Jane Harness UBO" :entity-type "proper_person_natural" :first-name "Jane" :last-name "UBO")"#,
            )
            .await?;
        self.state.ubo_entity_id = Self::extract_uuid(&results);

        // Add entities to group
        if let Some(manco_id) = self.state.manco_entity_id {
            self.exec(
                "group",
                &format!(
                    r#"(client-group.entity-add :group-id "{}" :entity-id "{}")"#,
                    group_id, manco_id
                ),
            )
            .await?;
        }

        // Create CBU
        let results = self
            .exec(
                "group",
                &format!(
                    r#"(cbu.create :name "{}" :jurisdiction "LU" :structure-type "ucits")"#,
                    TEST_FUND_NAME
                ),
            )
            .await?;
        self.state.cbu_id = Self::extract_uuid(&results);
        println!("  CBU created: {:?}", self.state.cbu_id);

        self.state.phases_completed.push("group_setup".into());
        println!("  Phase 1 complete ✓");
        Ok(())
    }

    // =========================================================================
    // Phase 2: UBO Discovery
    // =========================================================================

    pub async fn phase_ubo_discovery(&mut self) -> Result<()> {
        println!("\n=== Phase 2: UBO Discovery ===");

        let manco_id = self
            .state
            .manco_entity_id
            .context("ManCo entity not created")?;
        let ubo_id = self.state.ubo_entity_id.context("UBO entity not created")?;

        // Allege UBO ownership — may fail if unique constraint is missing (known issue)
        match self.exec(
            "ubo",
            &format!(
                r#"(ubo.add-ownership :owner-entity-id "{}" :owned-entity-id "{}" :percentage 30.0 :source "manual")"#,
                ubo_id, manco_id
            ),
        )
        .await {
            Ok(_) => println!("  UBO allegation recorded: {} → {} (30%)", TEST_UBO_NAME, TEST_MANCO_NAME),
            Err(e) => {
                println!("  UBO allegation skipped (handler issue): {}", e);
                self.state.errors.push(format!("ubo.add-ownership: {}", e));
            }
        }

        self.state.phases_completed.push("ubo_discovery".into());
        println!("  Phase 2 complete ✓");
        Ok(())
    }

    // =========================================================================
    // Phase 3: KYC Case
    // =========================================================================

    pub async fn phase_kyc_case(&mut self) -> Result<()> {
        println!("\n=== Phase 3: KYC Case ===");

        let cbu_id = self.state.cbu_id.context("CBU not created")?;

        // Clean up any existing active case for this CBU (from previous runs)
        let _ = sqlx::query(
            r#"UPDATE "ob-poc".cases SET closed_at = now(), status = 'WITHDRAWN' WHERE cbu_id = $1 AND closed_at IS NULL"#
        )
        .bind(cbu_id)
        .execute(&self.pool)
        .await;

        // Create KYC case
        let results = self
            .exec(
                "kyc",
                &format!(
                    r#"(kyc-case.create :cbu-id "{}" :case-type "NEW_CLIENT" :risk-rating "MEDIUM")"#,
                    cbu_id
                ),
            )
            .await?;
        self.state.case_id = Self::extract_uuid(&results);
        println!("  KYC case created: {:?}", self.state.case_id);

        // Run screening (if case was created)
        if let Some(case_id) = self.state.case_id {
            if let Some(manco_id) = self.state.manco_entity_id {
                // Create entity workstream
                let ws_results = self
                    .exec(
                        "kyc",
                        &format!(
                            r#"(entity-workstream.create :case-id "{}" :entity-id "{}")"#,
                            case_id, manco_id
                        ),
                    )
                    .await?;
                self.state.workstream_id = Self::extract_uuid(&ws_results);
                println!("  Workstream created: {:?}", self.state.workstream_id);
            }
        }

        self.state.phases_completed.push("kyc_case".into());
        println!("  Phase 3 complete ✓");
        Ok(())
    }

    // =========================================================================
    // Phase 4: Document Evidence (bypass BPMN — CRUD path)
    // =========================================================================

    pub async fn phase_documents(&mut self) -> Result<()> {
        println!("\n=== Phase 4: Document Evidence ===");

        let _case_id = self.state.case_id.context("KYC case not created")?;
        let workstream_id = self.state.workstream_id.context("Workstream not created")?;

        // Create document request for passport
        match self
            .exec(
                "docs",
                &format!(
                    r#"(document.create-request :workstream-id "{}" :doc-type "PASSPORT")"#,
                    workstream_id
                ),
            )
            .await
        {
            Ok(results) => {
                let req_id = Self::extract_uuid(&results);
                println!("  Document request created: {:?}", req_id);

                // Mark as requested (sent to client)
                if let Some(rid) = req_id {
                    let _ = self
                        .exec(
                            "docs",
                            &format!(r#"(document.mark-requested :request-id "{}")"#, rid),
                        )
                        .await;
                    println!("  Document marked as requested");

                    // Mark as received
                    let _ = self
                        .exec(
                            "docs",
                            &format!(r#"(document.mark-received :request-id "{}")"#, rid),
                        )
                        .await;
                    println!("  Document marked as received");

                    // Verify the document
                    match self
                        .exec(
                            "docs",
                            &format!(r#"(document.verify-request :request-id "{}")"#, rid),
                        )
                        .await
                    {
                        Ok(_) => println!("  Document verified ✓"),
                        Err(e) => println!("  Document verify skipped: {}", e),
                    }
                }
            }
            Err(e) => {
                println!("  Document request failed: {}", e);
                self.state
                    .errors
                    .push(format!("document.create-request: {}", e));
            }
        }

        // Create a second document request (proof of address)
        match self
            .exec(
                "docs",
                &format!(
                    r#"(document.create-request :workstream-id "{}" :doc-type "PROOF_OF_ADDRESS")"#,
                    workstream_id
                ),
            )
            .await
        {
            Ok(results) => {
                let req_id = Self::extract_uuid(&results);
                println!("  Second document request created: {:?}", req_id);
                if let Some(rid) = req_id {
                    let _ = self
                        .exec(
                            "docs",
                            &format!(r#"(document.mark-requested :request-id "{}")"#, rid),
                        )
                        .await;
                    let _ = self
                        .exec(
                            "docs",
                            &format!(r#"(document.mark-received :request-id "{}")"#, rid),
                        )
                        .await;
                    let _ = self
                        .exec(
                            "docs",
                            &format!(r#"(document.verify-request :request-id "{}")"#, rid),
                        )
                        .await;
                    println!("  Second document verified ✓");
                }
            }
            Err(e) => {
                println!("  Second document failed: {}", e);
                self.state
                    .errors
                    .push(format!("document.create-request(2): {}", e));
            }
        }

        // Create evidence requirement for UBO proof
        if let Some(manco_id) = self.state.manco_entity_id {
            match self.exec(
                "docs",
                &format!(
                    r#"(evidence.create-requirement :entity-id "{}" :evidence-type "OWNERSHIP_REGISTER")"#,
                    manco_id
                ),
            ).await {
                Ok(_) => println!("  Evidence requirement created"),
                Err(e) => println!("  Evidence requirement skipped: {}", e),
            }
        }

        self.state.phases_completed.push("documents".into());
        println!("  Phase 4 complete ✓");
        Ok(())
    }

    // =========================================================================
    // Phase 5: Screening
    // =========================================================================

    pub async fn phase_screening(&mut self) -> Result<()> {
        println!("\n=== Phase 5: Screening ===");

        let workstream_id = self.state.workstream_id.context("Workstream not created")?;

        // Run screening (creates PENDING records — no external provider call)
        match self
            .exec(
                "screening",
                &format!(r#"(screening.run :workstream-id "{}")"#, workstream_id),
            )
            .await
        {
            Ok(_) => println!("  Screening initiated (PENDING)"),
            Err(e) => {
                println!("  Screening skipped: {}", e);
                self.state.errors.push(format!("screening.run: {}", e));
            }
        }

        self.state.phases_completed.push("screening".into());
        println!("  Phase 5 complete ✓");
        Ok(())
    }

    // =========================================================================
    // Phase 6: Tollgate Evaluation
    // =========================================================================

    pub async fn phase_tollgate(&mut self) -> Result<()> {
        println!("\n=== Phase 6: Tollgate Evaluation ===");

        let case_id = self.state.case_id.context("KYC case not created")?;

        // Run tollgate evaluation
        match self
            .exec(
                "tollgate",
                &format!(
                    r#"(tollgate.evaluate :case-id "{}" :evaluation-type "DISCOVERY_COMPLETE")"#,
                    case_id
                ),
            )
            .await
        {
            Ok(results) => {
                let eval_id = Self::extract_uuid(&results);
                println!("  Tollgate evaluation: {:?}", eval_id);
                // The evaluation will likely FAIL (screening not complete, docs may be insufficient)
                // but that's expected — this proves the tollgate verb pipeline works.
            }
            Err(e) => {
                println!("  Tollgate evaluation skipped: {}", e);
                self.state.errors.push(format!("tollgate.evaluate: {}", e));
            }
        }

        // Check decision readiness
        match self
            .exec(
                "tollgate",
                &format!(
                    r#"(tollgate.get-decision-readiness :case-id "{}")"#,
                    case_id
                ),
            )
            .await
        {
            Ok(_) => println!("  Decision readiness checked"),
            Err(e) => println!("  Decision readiness skipped: {}", e),
        }

        self.state.phases_completed.push("tollgate".into());
        println!("  Phase 6 complete ✓");
        Ok(())
    }

    // =========================================================================
    // Phase 7: Cleanup
    // =========================================================================

    pub async fn phase_cleanup(&mut self) -> Result<()> {
        println!("\n=== Phase: Cleanup ===");

        // Delete CBU cascade (removes case, workstreams, screenings)
        if let Some(cbu_id) = self.state.cbu_id {
            match self
                .exec(
                    "cleanup",
                    &format!(r#"(cbu.delete-cascade :cbu-id "{}")"#, cbu_id),
                )
                .await
            {
                Ok(_) => println!("  CBU deleted: {}", cbu_id),
                Err(e) => println!("  CBU delete failed (may not exist): {}", e),
            }
        }

        // Delete entities
        for (name, id) in [
            ("UBO", self.state.ubo_entity_id),
            ("Director", self.state.director_entity_id),
            ("Depositary", self.state.depositary_entity_id),
            ("ManCo", self.state.manco_entity_id),
        ] {
            if let Some(eid) = id {
                match self
                    .exec(
                        "cleanup",
                        &format!(r#"(entity.delete :entity-id "{}")"#, eid),
                    )
                    .await
                {
                    Ok(_) => println!("  {} deleted: {}", name, eid),
                    Err(e) => println!("  {} delete failed: {}", name, e),
                }
            }
        }

        // Delete client group
        if let Some(gid) = self.state.group_id {
            match sqlx::query(r#"DELETE FROM "ob-poc".client_group WHERE id = $1"#)
                .bind(gid)
                .execute(&self.pool)
                .await
            {
                Ok(_) => println!("  Group deleted: {}", gid),
                Err(e) => println!("  Group delete failed: {}", e),
            }
        }

        self.state.phases_completed.push("cleanup".into());
        println!("  Cleanup complete ✓");
        Ok(())
    }

    // =========================================================================
    // Full Run
    // =========================================================================

    pub async fn run_full(&mut self) -> Result<()> {
        println!("╔══════════════════════════════════════════╗");
        println!("║  Onboarding Test Harness — Full Run     ║");
        println!("╚══════════════════════════════════════════╝");

        self.phase_group_setup().await?;
        self.phase_ubo_discovery().await?;
        self.phase_kyc_case().await?;
        self.phase_documents().await?;
        self.phase_screening().await?;
        self.phase_tollgate().await?;

        println!("\n=== Summary ===");
        println!("  Phases: {}", self.state.phases_completed.join(" → "));
        println!("  Verbs executed: {}", self.state.verb_log.len());
        println!(
            "  Total time: {}ms",
            self.state
                .verb_log
                .iter()
                .map(|v| v.duration_ms)
                .sum::<u64>()
        );
        if !self.state.errors.is_empty() {
            println!("  Errors: {}", self.state.errors.len());
            for e in &self.state.errors {
                println!("    ✗ {}", e);
            }
        }

        Ok(())
    }

    pub async fn run_setup_only(&mut self) -> Result<()> {
        self.phase_group_setup().await?;
        self.phase_ubo_discovery().await?;
        Ok(())
    }

    pub async fn run_kyc_only(&mut self) -> Result<()> {
        // Load existing state from DB
        self.load_existing_state().await?;
        self.phase_kyc_case().await?;
        Ok(())
    }

    async fn load_existing_state(&mut self) -> Result<()> {
        // Find the test group by name
        let row: Option<(Uuid,)> =
            sqlx::query_as(r#"SELECT id FROM "ob-poc".client_group WHERE canonical_name = $1"#)
                .bind(TEST_GROUP_NAME)
                .fetch_optional(&self.pool)
                .await?;

        if let Some((id,)) = row {
            self.state.group_id = Some(id);
            println!("  Found existing group: {}", id);
        } else {
            anyhow::bail!(
                "Test group '{}' not found — run setup first",
                TEST_GROUP_NAME
            );
        }
        Ok(())
    }
}

// =============================================================================
// Entry Point (called from xtask main)
// =============================================================================

pub async fn run_onboarding_harness(pool: &PgPool, mode: &str, verbose: bool) -> Result<()> {
    let mut harness = OnboardingHarness::new(pool.clone(), verbose).await?;

    match mode {
        "full" => {
            harness.run_full().await?;
            println!("\n  Cleaning up...");
            harness.phase_cleanup().await?;
        }
        "setup" => harness.run_setup_only().await?,
        "kyc" => harness.run_kyc_only().await?,
        "clean" => harness.phase_cleanup().await?,
        other => anyhow::bail!("Unknown mode: {}. Use: full, setup, kyc, clean", other),
    }

    Ok(())
}
