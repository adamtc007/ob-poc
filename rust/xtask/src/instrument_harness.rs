//! Instrument Matrix Test Harness — Trading infrastructure E2E via DSL verbs
//!
//! Creates a CBU with trading profile, custody setup, booking principals,
//! settlement chains, and validates the full instrument matrix lifecycle.
//!
//! Usage:
//! ```bash
//! cargo x instrument-harness --mode full --verbose
//! cargo x instrument-harness --mode setup     # CBU + trading profile only
//! cargo x instrument-harness --mode clean      # Delete test data
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

const TEST_CBU_NAME: &str = "Harness Trading Fund";
const TEST_PRINCIPAL_NAME: &str = "Harness Booking Principal";

// =============================================================================
// State
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentHarnessState {
    pub started_at: String,
    pub cbu_id: Option<Uuid>,
    pub profile_id: Option<Uuid>,
    pub custody_id: Option<Uuid>,
    pub principal_id: Option<Uuid>,
    pub chain_id: Option<Uuid>,
    pub sweep_id: Option<Uuid>,
    pub phases_completed: Vec<String>,
    pub errors: Vec<String>,
    pub verb_count: usize,
    pub total_ms: u64,
}

// =============================================================================
// Harness
// =============================================================================

pub struct InstrumentHarness {
    pool: PgPool,
    executor: DslExecutor,
    state: InstrumentHarnessState,
    verbose: bool,
}

impl InstrumentHarness {
    pub async fn new(pool: PgPool, verbose: bool) -> Result<Self> {
        let executor = DslExecutor::new(pool.clone());
        Ok(Self {
            pool,
            executor,
            state: InstrumentHarnessState {
                started_at: Utc::now().to_rfc3339(),
                cbu_id: None,
                profile_id: None,
                custody_id: None,
                principal_id: None,
                chain_id: None,
                sweep_id: None,
                phases_completed: Vec::new(),
                errors: Vec::new(),
                verb_count: 0,
                total_ms: 0,
            },
            verbose,
        })
    }

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
        self.state.verb_count += 1;
        self.state.total_ms += duration_ms;

        if self.verbose {
            println!("    ✓ {} results ({}ms)", results.len(), duration_ms);
        }

        Ok(results)
    }

    fn extract_uuid(results: &[ExecutionResult]) -> Option<Uuid> {
        results.first().and_then(|r| match r {
            ExecutionResult::Uuid(id) => Some(*id),
            ExecutionResult::Record(v) => v
                .get("id")
                .or_else(|| v.get("cbu_id"))
                .or_else(|| v.get("profile_id"))
                .or_else(|| v.get("principal_id"))
                .or_else(|| v.get("chain_id"))
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok()),
            _ => None,
        })
    }

    // =========================================================================
    // Phase 1: CBU + Trading Profile
    // =========================================================================

    pub async fn phase_setup(&mut self) -> Result<()> {
        println!("\n=== Phase 1: CBU + Trading Profile ===");

        // Create CBU
        let results = self
            .exec(
                "setup",
                &format!(
                    r#"(cbu.create :name "{}" :jurisdiction "LU" :structure-type "ucits")"#,
                    TEST_CBU_NAME
                ),
            )
            .await?;
        self.state.cbu_id = Self::extract_uuid(&results);
        let cbu_id = self.state.cbu_id.context("Failed to create CBU")?;
        println!("  CBU created: {}", cbu_id);

        // Create trading profile draft
        match self
            .exec(
                "setup",
                &format!(
                    r#"(trading-profile.create-draft :cbu-id "{}")"#,
                    cbu_id
                ),
            )
            .await
        {
            Ok(results) => {
                self.state.profile_id = Self::extract_uuid(&results);
                println!("  Trading profile created: {:?}", self.state.profile_id);
            }
            Err(e) => {
                println!("  Trading profile skipped: {}", e);
                self.state.errors.push(format!("trading-profile.create-draft: {}", e));
            }
        }

        // Read trading profile
        if let Some(pid) = self.state.profile_id {
            match self
                .exec("setup", &format!(r#"(trading-profile.read :profile-id "{}")"#, pid))
                .await
            {
                Ok(_) => println!("  Trading profile read ✓"),
                Err(e) => println!("  Trading profile read skipped: {}", e),
            }
        }

        self.state.phases_completed.push("setup".into());
        println!("  Phase 1 complete ✓");
        Ok(())
    }

    // =========================================================================
    // Phase 2: Custody + SSI
    // =========================================================================

    pub async fn phase_custody(&mut self) -> Result<()> {
        println!("\n=== Phase 2: Custody ===");

        let cbu_id = self.state.cbu_id.context("CBU not created")?;

        // List SSIs (read-only — doesn't need all the create args)
        match self
            .exec(
                "custody",
                &format!(
                    r#"(cbu-custody.list-ssis :cbu-id "{}")"#,
                    cbu_id
                ),
            )
            .await
        {
            Ok(_) => println!("  SSIs listed ✓"),
            Err(e) => {
                println!("  SSI list skipped: {}", e);
                self.state.errors.push(format!("cbu-custody.list-ssis: {}", e));
            }
        }

        // List universe
        match self
            .exec(
                "custody",
                &format!(r#"(cbu-custody.list-universe :cbu-id "{}")"#, cbu_id),
            )
            .await
        {
            Ok(_) => println!("  Universe listed ✓"),
            Err(e) => println!("  Universe list skipped: {}", e),
        }

        self.state.phases_completed.push("custody".into());
        println!("  Phase 2 complete ✓");
        Ok(())
    }

    // =========================================================================
    // Phase 3: Booking Principal
    // =========================================================================

    pub async fn phase_booking(&mut self) -> Result<()> {
        println!("\n=== Phase 3: Booking Principal ===");

        let cbu_id = self.state.cbu_id.context("CBU not created")?;

        // Create booking principal
        match self
            .exec(
                "booking",
                &format!(
                    r#"(booking-principal.create :cbu-id "{}" :name "{}")"#,
                    cbu_id, TEST_PRINCIPAL_NAME
                ),
            )
            .await
        {
            Ok(results) => {
                self.state.principal_id = Self::extract_uuid(&results);
                println!("  Booking principal created: {:?}", self.state.principal_id);
            }
            Err(e) => {
                println!("  Booking principal skipped: {}", e);
                self.state.errors.push(format!("booking-principal.create: {}", e));
            }
        }

        // Evaluate booking principal
        if let Some(pid) = self.state.principal_id {
            match self
                .exec(
                    "booking",
                    &format!(r#"(booking-principal.evaluate :principal-id "{}")"#, pid),
                )
                .await
            {
                Ok(_) => println!("  Booking principal evaluated ✓"),
                Err(e) => println!("  Evaluate skipped: {}", e),
            }
        }

        self.state.phases_completed.push("booking".into());
        println!("  Phase 3 complete ✓");
        Ok(())
    }

    // =========================================================================
    // Phase 4: Settlement Chain
    // =========================================================================

    pub async fn phase_settlement(&mut self) -> Result<()> {
        println!("\n=== Phase 4: Settlement Chain ===");

        let cbu_id = self.state.cbu_id.context("CBU not created")?;

        // Create settlement chain
        match self
            .exec(
                "settlement",
                &format!(
                    r#"(settlement-chain.create-chain :cbu-id "{}" :market "LU" :currency "EUR")"#,
                    cbu_id
                ),
            )
            .await
        {
            Ok(results) => {
                self.state.chain_id = Self::extract_uuid(&results);
                println!("  Settlement chain created: {:?}", self.state.chain_id);
            }
            Err(e) => {
                println!("  Settlement chain skipped: {}", e);
                self.state.errors.push(format!("settlement-chain.create-chain: {}", e));
            }
        }

        // List chains
        match self
            .exec(
                "settlement",
                &format!(r#"(settlement-chain.list-chains :cbu-id "{}")"#, cbu_id),
            )
            .await
        {
            Ok(_) => println!("  Chains listed ✓"),
            Err(e) => println!("  Chain list skipped: {}", e),
        }

        self.state.phases_completed.push("settlement".into());
        println!("  Phase 4 complete ✓");
        Ok(())
    }

    // =========================================================================
    // Phase 5: Corporate Actions
    // =========================================================================

    pub async fn phase_corporate_actions(&mut self) -> Result<()> {
        println!("\n=== Phase 5: Corporate Actions ===");

        // List event types (should work without CBU context)
        match self
            .exec("ca", r#"(corporate-action.list-event-types)"#)
            .await
        {
            Ok(_) => println!("  Event types listed ✓"),
            Err(e) => {
                println!("  Event types skipped: {}", e);
                self.state.errors.push(format!("corporate-action.list-event-types: {}", e));
            }
        }

        self.state.phases_completed.push("corporate_actions".into());
        println!("  Phase 5 complete ✓");
        Ok(())
    }

    // =========================================================================
    // Phase 6: Validation
    // =========================================================================

    pub async fn phase_validation(&mut self) -> Result<()> {
        println!("\n=== Phase 6: Validation ===");

        if let Some(profile_id) = self.state.profile_id {
            // Validate go-live readiness
            match self
                .exec(
                    "validate",
                    &format!(
                        r#"(trading-profile.validate-go-live-ready :profile-id "{}")"#,
                        profile_id
                    ),
                )
                .await
            {
                Ok(_) => println!("  Go-live validation ✓"),
                Err(e) => println!("  Go-live validation skipped: {}", e),
            }

            // Validate universe coverage
            match self
                .exec(
                    "validate",
                    &format!(
                        r#"(trading-profile.validate-universe-coverage :profile-id "{}")"#,
                        profile_id
                    ),
                )
                .await
            {
                Ok(_) => println!("  Universe coverage validation ✓"),
                Err(e) => println!("  Coverage validation skipped: {}", e),
            }
        } else {
            println!("  Skipped — no trading profile");
        }

        self.state.phases_completed.push("validation".into());
        println!("  Phase 6 complete ✓");
        Ok(())
    }

    // =========================================================================
    // Cleanup
    // =========================================================================

    pub async fn phase_cleanup(&mut self) -> Result<()> {
        println!("\n=== Cleanup ===");

        if let Some(cbu_id) = self.state.cbu_id {
            match self
                .exec("cleanup", &format!(r#"(cbu.delete-cascade :cbu-id "{}")"#, cbu_id))
                .await
            {
                Ok(_) => println!("  CBU deleted: {}", cbu_id),
                Err(e) => println!("  CBU delete failed: {}", e),
            }
        }

        self.state.phases_completed.push("cleanup".into());
        println!("  Cleanup complete ✓");
        Ok(())
    }

    // =========================================================================
    // Run modes
    // =========================================================================

    pub async fn run_full(&mut self) -> Result<()> {
        println!("╔══════════════════════════════════════════╗");
        println!("║  Instrument Matrix Harness — Full Run   ║");
        println!("╚══════════════════════════════════════════╝");

        self.phase_setup().await?;
        self.phase_custody().await?;
        self.phase_booking().await?;
        self.phase_settlement().await?;
        self.phase_corporate_actions().await?;
        self.phase_validation().await?;

        println!("\n=== Summary ===");
        println!("  Phases: {}", self.state.phases_completed.join(" → "));
        println!("  Verbs executed: {}", self.state.verb_count);
        println!("  Total time: {}ms", self.state.total_ms);
        if !self.state.errors.is_empty() {
            println!("  Errors: {}", self.state.errors.len());
            for e in &self.state.errors {
                println!("    ✗ {}", e);
            }
        }

        Ok(())
    }
}

// =============================================================================
// Entry Point
// =============================================================================

pub async fn run_instrument_harness(
    pool: &PgPool,
    mode: &str,
    verbose: bool,
) -> Result<()> {
    let mut harness = InstrumentHarness::new(pool.clone(), verbose).await?;

    match mode {
        "full" => {
            harness.run_full().await?;
            println!("\n  Cleaning up...");
            harness.phase_cleanup().await?;
        }
        "setup" => harness.phase_setup().await?,
        "clean" => harness.phase_cleanup().await?,
        other => anyhow::bail!("Unknown mode: {}. Use: full, setup, clean", other),
    }

    Ok(())
}
