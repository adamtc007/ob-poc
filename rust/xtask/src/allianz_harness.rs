//! Allianz Test Harness - DSL-Driven GLEIF Integration Test
//!
//! This module provides a complete end-to-end test of the onboarding pipeline
//! using DSL verbs exclusively (no direct SQL):
//!
//! 1. Discovery: `gleif.search` to find Allianz funds
//! 2. Entity Import: `gleif.enrich` to create entities from GLEIF data
//! 3. ManCo Discovery: `gleif.get-manager` for each fund
//! 4. CBU Creation: `cbu.ensure` + `cbu.assign-role` via DSL
//! 5. UBO Tracing: `gleif.trace-ownership` for ownership chains
//!
//! Usage:
//! ```bash
//! cargo x allianz-harness --mode discover    # Discovery only
//! cargo x allianz-harness --mode import      # Full import
//! cargo x allianz-harness --mode clean       # Clean up test data
//! cargo x allianz-harness --mode full        # All phases
//! ```

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use ob_poc::dsl_v2::{compile, parse_program, DslExecutor, ExecutionContext, ExecutionResult};

// =============================================================================
// Configuration Constants
// =============================================================================

/// Allianz Global Investors GmbH LEI (fund manager)
pub const ALLIANZ_GI_LEI: &str = "529900FAHFDMSXCPII15";

/// Jurisdictions to filter for (Luxembourg focus)
pub const TARGET_JURISDICTIONS: &[&str] = &["LU", "DE", "IE"];

// =============================================================================
// Harness State Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllianzHarnessState {
    /// Run timestamp
    pub started_at: DateTime<Utc>,

    /// Discovery phase results
    pub discovery: Option<DiscoveryPhaseResult>,

    /// Entity creation phase results
    pub entity_import: Option<EntityImportResult>,

    /// CBU creation phase results
    pub cbu_creation: Option<CbuCreationResult>,

    /// UBO tracing phase results
    pub ubo_tracing: Option<UboTracingResult>,

    /// Errors encountered
    pub errors: Vec<HarnessError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredFund {
    pub lei: String,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub manager_lei: Option<String>,
    pub manager_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryPhaseResult {
    pub manager_lei: String,
    pub manager_name: String,
    pub funds_found: usize,
    pub funds: Vec<DiscoveredFund>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityImportResult {
    pub entities_created: usize,
    pub entities_skipped: usize,
    pub lei_to_entity_id: HashMap<String, Uuid>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuCreationResult {
    pub cbus_created: usize,
    pub roles_assigned: usize,
    pub lei_to_cbu_id: HashMap<String, Uuid>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UboTracingResult {
    pub chains_traced: usize,
    pub public_float_termini: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessError {
    pub phase: String,
    pub message: String,
    pub lei: Option<String>,
    pub recoverable: bool,
}

// =============================================================================
// Main Harness Struct
// =============================================================================

pub struct AllianzTestHarness {
    pool: PgPool,
    executor: DslExecutor,
    state: AllianzHarnessState,
    dry_run: bool,
    verbose: bool,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessMode {
    /// Discovery only - query GLEIF but don't persist
    Discover,
    /// Full import - discovery + entity creation + CBUs + roles
    Import,
    /// Clean test data - remove Allianz entities created by harness
    Clean,
    /// Full end-to-end including UBO tracing
    Full,
}

impl AllianzTestHarness {
    pub fn new(pool: PgPool) -> Self {
        let executor = DslExecutor::new(pool.clone());
        Self {
            pool,
            executor,
            state: AllianzHarnessState {
                started_at: Utc::now(),
                discovery: None,
                entity_import: None,
                cbu_creation: None,
                ubo_tracing: None,
                errors: Vec::new(),
            },
            dry_run: false,
            verbose: false,
            limit: None,
        }
    }

    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    pub fn with_limit(mut self, limit: Option<usize>) -> Self {
        self.limit = limit;
        self
    }

    /// Run the harness with the specified mode
    pub async fn run(&mut self, mode: HarnessMode) -> Result<&AllianzHarnessState> {
        self.log_header(&format!("Allianz Test Harness (DSL Mode) - {:?}", mode));

        match mode {
            HarnessMode::Discover => {
                self.run_discovery().await?;
            }
            HarnessMode::Import => {
                self.run_discovery().await?;
                if !self.dry_run {
                    self.run_entity_import().await?;
                    self.run_cbu_creation().await?;
                }
            }
            HarnessMode::Clean => {
                self.run_cleanup().await?;
            }
            HarnessMode::Full => {
                self.run_discovery().await?;
                if !self.dry_run {
                    self.run_entity_import().await?;
                    self.run_cbu_creation().await?;
                    self.run_ubo_tracing().await?;
                }
            }
        }

        self.print_summary();
        Ok(&self.state)
    }

    // =========================================================================
    // DSL Execution Helper
    // =========================================================================

    /// Execute DSL and return the execution context with bindings
    async fn execute_dsl(&self, dsl: &str) -> Result<ExecutionContext> {
        let ast = parse_program(dsl).map_err(|e| anyhow::anyhow!("Parse error: {:?}", e))?;
        let plan = compile(&ast).map_err(|e| anyhow::anyhow!("Compile error: {:?}", e))?;
        let mut ctx = ExecutionContext::new().without_idempotency();
        self.executor.execute_plan(&plan, &mut ctx).await?;
        Ok(ctx)
    }

    /// Execute DSL and return results
    async fn execute_dsl_with_results(
        &self,
        dsl: &str,
    ) -> Result<(ExecutionContext, Vec<ExecutionResult>)> {
        let ast = parse_program(dsl).map_err(|e| anyhow::anyhow!("Parse error: {:?}", e))?;
        let plan = compile(&ast).map_err(|e| anyhow::anyhow!("Compile error: {:?}", e))?;
        let mut ctx = ExecutionContext::new().without_idempotency();
        let results = self.executor.execute_plan(&plan, &mut ctx).await?;
        Ok((ctx, results))
    }

    // =========================================================================
    // Phase 1: Discovery via gleif.search
    // =========================================================================

    async fn run_discovery(&mut self) -> Result<()> {
        self.log_phase("Phase 1: GLEIF Discovery (via DSL)");
        let start = std::time::Instant::now();

        let limit = self.limit.unwrap_or(100);
        let mut funds: Vec<DiscoveredFund> = Vec::new();

        // First try gleif.get-managed-funds (uses GLEIF relationship data)
        self.log_info(&format!(
            "Trying managed-funds relationship for LEI: {}",
            ALLIANZ_GI_LEI
        ));

        let dsl = format!(
            r#"(gleif.get-managed-funds :manager-lei "{}" :limit {} :as @funds)"#,
            ALLIANZ_GI_LEI, limit
        );

        if let Ok((ctx, _)) = self.execute_dsl_with_results(&dsl).await {
            if let Some(json_val) = ctx.json_bindings.get("funds") {
                if let Some(fund_array) = json_val.get("funds").and_then(|f| f.as_array()) {
                    for fund_json in fund_array {
                        let lei = fund_json
                            .get("lei")
                            .and_then(|l| l.as_str())
                            .unwrap_or("")
                            .to_string();
                        let name = fund_json
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("")
                            .to_string();
                        let jurisdiction = fund_json
                            .get("jurisdiction")
                            .and_then(|j| j.as_str())
                            .map(String::from);

                        if let Some(ref jur) = jurisdiction {
                            if !TARGET_JURISDICTIONS.contains(&jur.as_str()) {
                                continue;
                            }
                        }

                        funds.push(DiscoveredFund {
                            lei,
                            name,
                            jurisdiction,
                            manager_lei: Some(ALLIANZ_GI_LEI.to_string()),
                            manager_name: Some("Allianz Global Investors GmbH".to_string()),
                        });
                    }
                }
            }
        }

        // Fallback: Use gleif.search to find funds by name pattern
        if funds.is_empty() {
            self.log_info("  No managed-funds relationship data, using name search fallback...");

            let search_dsl = format!(
                r#"(gleif.search :name "Allianz Global Investors" :limit {} :as @search)"#,
                limit
            );

            if self.verbose {
                self.log_info(&format!("DSL: {}", search_dsl));
            }

            if let Ok((ctx, _)) = self.execute_dsl_with_results(&search_dsl).await {
                // RecordSet is bound directly as an array in json_bindings
                if let Some(serde_json::Value::Array(entities)) = ctx.json_bindings.get("search") {
                    for entity in entities {
                        let lei = entity
                            .get("lei")
                            .and_then(|l| l.as_str())
                            .unwrap_or("")
                            .to_string();
                        let name = entity
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("")
                            .to_string();
                        let jurisdiction = entity
                            .get("jurisdiction")
                            .and_then(|j| j.as_str())
                            .map(String::from);
                        let category = entity.get("category").and_then(|c| c.as_str());

                        // Only include FUND category entities in target jurisdictions
                        if category != Some("FUND") {
                            continue;
                        }
                        if let Some(ref jur) = jurisdiction {
                            if !TARGET_JURISDICTIONS.contains(&jur.as_str()) {
                                continue;
                            }
                        }

                        funds.push(DiscoveredFund {
                            lei,
                            name,
                            jurisdiction,
                            manager_lei: Some(ALLIANZ_GI_LEI.to_string()),
                            manager_name: Some("Allianz Global Investors GmbH".to_string()),
                        });
                    }
                }
            }
        }

        // Apply limit
        if funds.len() > limit {
            funds.truncate(limit);
        }

        self.log_info(&format!(
            "Found {} funds in target jurisdictions",
            funds.len()
        ));

        if self.verbose {
            for fund in &funds {
                self.log_info(&format!("  - {} ({})", fund.name, fund.lei));
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        self.state.discovery = Some(DiscoveryPhaseResult {
            manager_lei: ALLIANZ_GI_LEI.to_string(),
            manager_name: "Allianz Global Investors GmbH".to_string(),
            funds_found: funds.len(),
            funds,
            duration_ms,
        });

        self.log_success(&format!("Discovery complete in {}ms", duration_ms));
        Ok(())
    }

    // =========================================================================
    // Phase 2: Entity Import via gleif.enrich
    // =========================================================================

    async fn run_entity_import(&mut self) -> Result<()> {
        self.log_phase("Phase 2: Entity Import (via gleif.enrich)");
        let start = std::time::Instant::now();

        let funds = self
            .state
            .discovery
            .as_ref()
            .context("Discovery must run before entity import")?
            .funds
            .clone();

        let mut lei_to_entity_id: HashMap<String, Uuid> = HashMap::new();
        let mut entities_created = 0;
        let mut entities_skipped = 0;

        // First, ensure the manager entity exists
        self.log_info("Creating manager entity...");
        let manager_dsl = format!(r#"(gleif.enrich :lei "{}" :as @manager)"#, ALLIANZ_GI_LEI);

        match self.execute_dsl(&manager_dsl).await {
            Ok(ctx) => {
                if let Some(id) = ctx.symbols.get("manager") {
                    lei_to_entity_id.insert(ALLIANZ_GI_LEI.to_string(), *id);
                    entities_created += 1;
                    self.log_info(&format!("  Created manager entity: {}", id));
                }
            }
            Err(e) => {
                self.add_error(
                    "entity_import",
                    &format!("Manager creation failed: {}", e),
                    Some(ALLIANZ_GI_LEI),
                    true,
                );
                entities_skipped += 1;
            }
        }

        // Create fund entities
        self.log_info(&format!("Creating {} fund entities...", funds.len()));

        for fund in &funds {
            let fund_dsl = format!(r#"(gleif.enrich :lei "{}" :as @fund)"#, fund.lei);

            match self.execute_dsl(&fund_dsl).await {
                Ok(ctx) => {
                    if let Some(id) = ctx.symbols.get("fund") {
                        lei_to_entity_id.insert(fund.lei.clone(), *id);
                        entities_created += 1;
                        if self.verbose {
                            self.log_info(&format!("  Created fund: {} -> {}", fund.name, id));
                        }
                    }
                }
                Err(e) => {
                    self.add_error(
                        "entity_import",
                        &format!("Fund creation failed: {}", e),
                        Some(&fund.lei),
                        true,
                    );
                    entities_skipped += 1;
                }
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        self.state.entity_import = Some(EntityImportResult {
            entities_created,
            entities_skipped,
            lei_to_entity_id,
            duration_ms,
        });

        self.log_success(&format!(
            "Entity import complete: {} created, {} skipped in {}ms",
            entities_created, entities_skipped, duration_ms
        ));

        Ok(())
    }

    // =========================================================================
    // Phase 3: CBU Creation via DSL
    // =========================================================================

    async fn run_cbu_creation(&mut self) -> Result<()> {
        self.log_phase("Phase 3: CBU Creation + Role Assignment (via DSL)");
        let start = std::time::Instant::now();

        let funds = self
            .state
            .discovery
            .as_ref()
            .context("Discovery must run before CBU creation")?
            .funds
            .clone();

        let lei_to_entity_id = self
            .state
            .entity_import
            .as_ref()
            .context("Entity import must run before CBU creation")?
            .lei_to_entity_id
            .clone();

        let manager_entity_id = lei_to_entity_id.get(ALLIANZ_GI_LEI);

        let mut lei_to_cbu_id: HashMap<String, Uuid> = HashMap::new();
        let mut cbus_created = 0;
        let mut roles_assigned = 0;

        for fund in &funds {
            let jurisdiction = fund.jurisdiction.as_deref().unwrap_or("LU");

            // Build DSL to create CBU and assign roles
            let mut dsl_lines = vec![format!(
                r#"(cbu.ensure :name "{}" :jurisdiction "{}" :client-type "FUND" :as @cbu)"#,
                escape_dsl_string(&fund.name),
                jurisdiction
            )];

            // Assign ASSET_OWNER role (fund entity owns itself)
            if let Some(fund_entity_id) = lei_to_entity_id.get(&fund.lei) {
                dsl_lines.push(format!(
                    r#"(cbu.assign-role :cbu-id @cbu :entity-id "{}" :role "ASSET_OWNER")"#,
                    fund_entity_id
                ));
            }

            // Assign INVESTMENT_MANAGER and MANAGEMENT_COMPANY roles
            if let Some(mgr_id) = manager_entity_id {
                dsl_lines.push(format!(
                    r#"(cbu.assign-role :cbu-id @cbu :entity-id "{}" :role "INVESTMENT_MANAGER")"#,
                    mgr_id
                ));
                dsl_lines.push(format!(
                    r#"(cbu.assign-role :cbu-id @cbu :entity-id "{}" :role "MANAGEMENT_COMPANY")"#,
                    mgr_id
                ));
            }

            let dsl = dsl_lines.join("\n");

            if self.verbose {
                self.log_info(&format!("DSL for {}:", fund.name));
                for line in &dsl_lines {
                    self.log_info(&format!("  {}", line));
                }
            }

            match self.execute_dsl(&dsl).await {
                Ok(ctx) => {
                    if let Some(cbu_id) = ctx.symbols.get("cbu") {
                        lei_to_cbu_id.insert(fund.lei.clone(), *cbu_id);
                        cbus_created += 1;

                        // Count roles assigned
                        let role_count = dsl_lines.len() - 1; // Subtract the cbu.ensure line
                        roles_assigned += role_count;

                        if self.verbose {
                            self.log_info(&format!(
                                "  Created CBU: {} ({} roles)",
                                cbu_id, role_count
                            ));
                        }
                    }
                }
                Err(e) => {
                    self.add_error(
                        "cbu_creation",
                        &format!("CBU creation failed: {}", e),
                        Some(&fund.lei),
                        true,
                    );
                }
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        self.state.cbu_creation = Some(CbuCreationResult {
            cbus_created,
            roles_assigned,
            lei_to_cbu_id,
            duration_ms,
        });

        self.log_success(&format!(
            "CBU creation complete: {} CBUs, {} roles in {}ms",
            cbus_created, roles_assigned, duration_ms
        ));

        Ok(())
    }

    // =========================================================================
    // Phase 4: UBO Tracing via gleif.trace-ownership
    // =========================================================================

    async fn run_ubo_tracing(&mut self) -> Result<()> {
        self.log_phase("Phase 4: UBO Chain Tracing (via gleif.trace-ownership)");
        let start = std::time::Instant::now();

        let funds = self
            .state
            .discovery
            .as_ref()
            .context("Discovery must run before UBO tracing")?
            .funds
            .clone();

        let mut chains_traced = 0;
        let mut public_float_termini = 0;

        // For each fund, trace ownership chain
        for fund in &funds {
            let dsl = format!(
                r#"(gleif.trace-ownership :lei "{}" :max-depth 10 :as @chain)"#,
                fund.lei
            );

            match self.execute_dsl(&dsl).await {
                Ok(ctx) => {
                    chains_traced += 1;

                    // Check if chain terminates in public float
                    if let Some(json_val) = ctx.json_bindings.get("chain") {
                        if let Some(terminus) =
                            json_val.get("terminus_type").and_then(|t| t.as_str())
                        {
                            if terminus == "PUBLIC_FLOAT" || terminus == "NO_PARENT" {
                                public_float_termini += 1;
                            }
                        }

                        if self.verbose {
                            let depth = json_val.get("depth").and_then(|d| d.as_u64()).unwrap_or(0);
                            self.log_info(&format!("  {} -> depth {}", fund.name, depth));
                        }
                    }
                }
                Err(e) => {
                    self.add_error(
                        "ubo_tracing",
                        &format!("Chain trace failed: {}", e),
                        Some(&fund.lei),
                        true,
                    );
                }
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        self.state.ubo_tracing = Some(UboTracingResult {
            chains_traced,
            public_float_termini,
            duration_ms,
        });

        self.log_success(&format!(
            "UBO tracing complete: {} chains, {} public float termini in {}ms",
            chains_traced, public_float_termini, duration_ms
        ));

        Ok(())
    }

    // =========================================================================
    // Cleanup via cbu.delete-cascade
    // =========================================================================

    async fn run_cleanup(&mut self) -> Result<()> {
        self.log_phase("Cleanup: Removing Allianz Test Data (via DSL)");

        if self.dry_run {
            self.log_info("DRY RUN - would delete Allianz data");
            return Ok(());
        }

        // Find all Allianz CBUs
        let cbus: Vec<(Uuid, String)> = sqlx::query_as(
            r#"SELECT cbu_id, name FROM "ob-poc".cbus WHERE name ILIKE '%Allianz%'"#,
        )
        .fetch_all(&self.pool)
        .await?;

        self.log_info(&format!("Found {} Allianz CBUs to delete", cbus.len()));

        let mut deleted = 0;
        for (cbu_id, name) in &cbus {
            let dsl = format!(r#"(cbu.delete-cascade :cbu-id "{}")"#, cbu_id);

            match self.execute_dsl(&dsl).await {
                Ok(_) => {
                    deleted += 1;
                    if self.verbose {
                        self.log_info(&format!("  Deleted: {}", name));
                    }
                }
                Err(e) => {
                    self.add_error("cleanup", &format!("Delete failed: {}", e), None, true);
                }
            }
        }

        // Also clean up orphan entities with Allianz in name
        let entities_deleted: u64 =
            sqlx::query(r#"DELETE FROM "ob-poc".entities WHERE name ILIKE '%Allianz%'"#)
                .execute(&self.pool)
                .await?
                .rows_affected();

        self.log_info(&format!(
            "Deleted {} CBUs, {} orphan entities",
            deleted, entities_deleted
        ));
        self.log_success("Cleanup complete");

        Ok(())
    }

    // =========================================================================
    // Logging Helpers
    // =========================================================================

    fn log_header(&self, message: &str) {
        println!("\n{}", "=".repeat(70));
        println!("  {}", message);
        println!("{}\n", "=".repeat(70));
    }

    fn log_phase(&self, message: &str) {
        println!("\n{}", "-".repeat(50));
        println!("{}", message);
        println!("{}", "-".repeat(50));
    }

    fn log_info(&self, message: &str) {
        println!("  {}", message);
    }

    fn log_success(&self, message: &str) {
        println!("  ✓ {}", message);
    }

    #[allow(dead_code)]
    fn log_warning(&self, message: &str) {
        println!("  ⚠ {}", message);
    }

    fn add_error(&mut self, phase: &str, message: &str, lei: Option<&str>, recoverable: bool) {
        self.state.errors.push(HarnessError {
            phase: phase.to_string(),
            message: message.to_string(),
            lei: lei.map(String::from),
            recoverable,
        });
        if !recoverable {
            println!("  ✗ ERROR: {}", message);
        } else if self.verbose {
            println!("  ⚠ Warning: {}", message);
        }
    }

    fn print_summary(&self) {
        println!("\n{}", "=".repeat(70));
        println!("  SUMMARY");
        println!("{}", "=".repeat(70));

        if let Some(ref disc) = self.state.discovery {
            println!("\n  Discovery:");
            println!("    Manager: {} ({})", disc.manager_name, disc.manager_lei);
            println!("    Funds: {}", disc.funds_found);
            println!("    Duration: {}ms", disc.duration_ms);
        }

        if let Some(ref imp) = self.state.entity_import {
            println!("\n  Entity Import:");
            println!("    Created: {}", imp.entities_created);
            println!("    Skipped: {}", imp.entities_skipped);
            println!("    Duration: {}ms", imp.duration_ms);
        }

        if let Some(ref cbu) = self.state.cbu_creation {
            println!("\n  CBU Creation:");
            println!("    CBUs: {}", cbu.cbus_created);
            println!("    Roles: {}", cbu.roles_assigned);
            println!("    Duration: {}ms", cbu.duration_ms);
        }

        if let Some(ref ubo) = self.state.ubo_tracing {
            println!("\n  UBO Tracing:");
            println!("    Chains traced: {}", ubo.chains_traced);
            println!("    Public float termini: {}", ubo.public_float_termini);
            println!("    Duration: {}ms", ubo.duration_ms);
        }

        if !self.state.errors.is_empty() {
            println!("\n  Errors: {}", self.state.errors.len());
            for err in &self.state.errors {
                println!("    - [{}] {}", err.phase, err.message);
            }
        }

        let total_duration = Utc::now().signed_duration_since(self.state.started_at);
        println!(
            "\n  Total Duration: {}ms",
            total_duration.num_milliseconds()
        );
        println!("{}\n", "=".repeat(70));
    }
}

/// Escape special characters in DSL strings
fn escape_dsl_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
