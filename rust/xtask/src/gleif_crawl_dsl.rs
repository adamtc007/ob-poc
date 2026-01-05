//! GLEIF Crawl via DSL Execution
//!
//! This module implements a GLEIF crawler that generates and executes DSL statements
//! instead of using raw SQL. It leverages the existing GLEIF DSL verbs:
//!
//! - `gleif.import-managed-funds` - Imports funds managed by an investment manager
//! - `gleif.get-parent` - Gets direct parent from GLEIF
//! - `gleif.get-children` - Gets direct children from GLEIF
//! - `gleif.trace-ownership` - Traces ownership chain to UBO terminus
//!
//! The crawler works by:
//! 1. Starting from a root LEI (e.g., Allianz GI for managed funds)
//! 2. Generating DSL statements for the crawl operations
//! 3. Executing DSL through the proper DSL executor pipeline
//! 4. Optionally tracing parent chains for discovered entities

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

/// Allianz Global Investors GmbH - Fund manager with managed funds
pub const ALLIANZ_GI_LEI: &str = "529900FAHFDMSXCPII15";

/// Configuration for DSL-based GLEIF crawl
#[derive(Debug, Clone)]
pub struct DslCrawlConfig {
    /// Root LEI to start from (defaults to Allianz GI for fund import)
    pub root_lei: String,
    /// Maximum funds to import (None = all)
    pub limit: Option<usize>,
    /// Whether to create CBUs for each fund
    pub create_cbus: bool,
    /// Whether to trace parent chains
    pub trace_parents: bool,
    /// Dry run - generate DSL but don't execute
    pub dry_run: bool,
    /// Verbose output
    pub verbose: bool,
    /// Output directory for generated DSL
    pub output_dir: PathBuf,
}

impl Default for DslCrawlConfig {
    fn default() -> Self {
        Self {
            root_lei: ALLIANZ_GI_LEI.to_string(),
            limit: None,
            create_cbus: true,
            trace_parents: true,
            dry_run: false,
            verbose: false,
            output_dir: PathBuf::from("data/gleif_crawl_output"),
        }
    }
}

/// Statistics from the DSL-based crawl
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DslCrawlStats {
    pub dsl_statements_generated: usize,
    pub dsl_statements_executed: usize,
    pub funds_imported: usize,
    pub entities_created: usize,
    pub cbus_created: usize,
    pub roles_assigned: usize,
    pub parent_chains_traced: usize,
    pub errors: Vec<String>,
    pub elapsed_secs: f64,
}

/// DSL-based GLEIF crawler
pub struct GleifDslCrawler {
    config: DslCrawlConfig,
    pool: Option<PgPool>,
    dsl_statements: Vec<String>,
    stats: DslCrawlStats,
    traced_leis: HashSet<String>,
}

impl GleifDslCrawler {
    pub fn new(config: DslCrawlConfig, pool: Option<PgPool>) -> Result<Self> {
        fs::create_dir_all(&config.output_dir)?;

        Ok(Self {
            config,
            pool,
            dsl_statements: Vec::new(),
            stats: DslCrawlStats::default(),
            traced_leis: HashSet::new(),
        })
    }

    /// Generate DSL for importing managed funds
    fn generate_import_managed_funds_dsl(&mut self) {
        let mut dsl = format!(
            r#"(gleif.import-managed-funds
  :manager-lei "{}"
  :create-cbus {}"#,
            self.config.root_lei, self.config.create_cbus
        );

        if let Some(limit) = self.config.limit {
            dsl.push_str(&format!("\n  :limit {}", limit));
        }

        if self.config.dry_run {
            dsl.push_str("\n  :dry-run true");
        }

        dsl.push_str("\n  :as @import-result)");

        self.dsl_statements.push(dsl);
        self.stats.dsl_statements_generated += 1;
    }

    /// Generate DSL for tracing ownership chain
    fn generate_trace_ownership_dsl(&mut self, lei: &str) {
        if self.traced_leis.contains(lei) {
            return;
        }
        self.traced_leis.insert(lei.to_string());

        let dsl = format!(
            r#"(gleif.trace-ownership :lei "{}" :as @chain-{})"#,
            lei,
            lei.chars().take(8).collect::<String>()
        );

        self.dsl_statements.push(dsl);
        self.stats.dsl_statements_generated += 1;
    }

    /// Generate DSL for getting parent chain (for corporate hierarchy)
    #[allow(dead_code)]
    fn generate_get_parent_dsl(&mut self, lei: &str) {
        let dsl = format!(r#"(gleif.get-parent :lei "{}")"#, lei);
        self.dsl_statements.push(dsl);
        self.stats.dsl_statements_generated += 1;
    }

    /// Generate DSL for getting children (for corporate hierarchy)
    #[allow(dead_code)]
    fn generate_get_children_dsl(&mut self, lei: &str, limit: usize) {
        let dsl = format!(r#"(gleif.get-children :lei "{}" :limit {})"#, lei, limit);
        self.dsl_statements.push(dsl);
        self.stats.dsl_statements_generated += 1;
    }

    /// Run the DSL-based crawl
    pub async fn crawl(&mut self) -> Result<DslCrawlStats> {
        let start_time = Instant::now();

        println!("\n{}", "=".repeat(70));
        println!("  GLEIF DSL-BASED CRAWL");
        println!("  Manager LEI: {}", self.config.root_lei);
        println!(
            "  Limit: {}",
            self.config
                .limit
                .map(|l| l.to_string())
                .unwrap_or_else(|| "unlimited".to_string())
        );
        println!("  Create CBUs: {}", self.config.create_cbus);
        println!("  Trace Parents: {}", self.config.trace_parents);
        println!("  Dry Run: {}", self.config.dry_run);
        println!("{}\n", "=".repeat(70));

        // Phase 1: Generate import-managed-funds DSL
        println!("Phase 1: Generating DSL for managed funds import...");
        self.generate_import_managed_funds_dsl();

        // If tracing parents, we'll add those after the main import
        if self.config.trace_parents && !self.config.dry_run {
            // After import, get the manager's parent chain
            println!("Phase 2: Generating DSL for parent chain tracing...");
            let root_lei = self.config.root_lei.clone();
            self.generate_trace_ownership_dsl(&root_lei);
        }

        // Save generated DSL to file
        let dsl_content = self.dsl_statements.join("\n\n");
        let dsl_file = self.config.output_dir.join("gleif_crawl.dsl");
        fs::write(&dsl_file, &dsl_content)?;
        println!("\n  DSL saved to: {}", dsl_file.display());

        // Execute DSL if not dry run and we have a pool
        if !self.config.dry_run {
            if let Some(pool) = self.pool.take() {
                println!("\nPhase 3: Executing DSL...");
                self.execute_dsl(&pool).await?;
                // Put pool back
                self.pool = Some(pool);
            } else {
                println!("\n  [WARN] No database connection - DSL not executed");
            }
        } else {
            println!("\n  [DRY RUN] DSL generated but not executed");
            println!("\n  Generated DSL:");
            println!("{}", "-".repeat(50));
            for stmt in &self.dsl_statements {
                println!("{}", stmt);
                println!();
            }
            println!("{}", "-".repeat(50));
        }

        self.stats.elapsed_secs = start_time.elapsed().as_secs_f64();
        self.print_summary();
        self.save_stats()?;

        Ok(self.stats.clone())
    }

    /// Execute the generated DSL statements
    async fn execute_dsl(&mut self, pool: &PgPool) -> Result<()> {
        use ob_poc::dsl_v2::{DslExecutor, ExecutionContext, ExecutionResult};

        // Create executor
        let executor = DslExecutor::new(pool.clone());

        for (i, dsl) in self.dsl_statements.iter().enumerate() {
            if self.config.verbose {
                println!("\n  [{}] Executing:", i + 1);
                println!("    {}", dsl.replace('\n', "\n    "));
            } else {
                print!(
                    "  [{}/{}] Executing DSL statement... ",
                    i + 1,
                    self.dsl_statements.len()
                );
            }

            // Create fresh execution context for each statement
            let mut ctx = ExecutionContext::new();

            match executor.execute_dsl(dsl, &mut ctx).await {
                Ok(results) => {
                    self.stats.dsl_statements_executed += 1;

                    if self.config.verbose {
                        println!("    Results: {} items", results.len());
                    }

                    // Extract stats from results
                    for result in &results {
                        if self.config.verbose {
                            println!("    Result type: {:?}", std::mem::discriminant(result));
                        }
                        // Match on the ExecutionResult enum to extract Record variant
                        if let ExecutionResult::Record(record) = result {
                            if self.config.verbose {
                                println!("    Record: {}", record);
                            }
                            // Parse import-managed-funds result
                            if let Some(funds) =
                                record.get("funds_imported").and_then(|v| v.as_u64())
                            {
                                self.stats.funds_imported += funds as usize;
                            }
                            if let Some(entities) =
                                record.get("entities_created").and_then(|v| v.as_u64())
                            {
                                self.stats.entities_created += entities as usize;
                            }
                            if let Some(cbus) = record.get("cbus_created").and_then(|v| v.as_u64())
                            {
                                self.stats.cbus_created += cbus as usize;
                            }
                            if let Some(roles) =
                                record.get("roles_assigned").and_then(|v| v.as_u64())
                            {
                                self.stats.roles_assigned += roles as usize;
                            }

                            // Parse trace-ownership result
                            if record.get("chain").is_some() {
                                self.stats.parent_chains_traced += 1;
                            }
                        }
                    }

                    if !self.config.verbose {
                        println!("OK");
                    }
                }
                Err(e) => {
                    let err_msg = format!("DSL execution failed: {}", e);
                    self.stats.errors.push(err_msg.clone());
                    if !self.config.verbose {
                        println!("FAILED");
                    }
                    println!("    Error: {}", e);
                }
            }
        }

        Ok(())
    }

    fn print_summary(&self) {
        println!("\n{}", "=".repeat(70));
        println!("  CRAWL SUMMARY");
        println!("{}", "=".repeat(70));

        println!("\n  DSL EXECUTION:");
        println!(
            "    Statements generated: {}",
            self.stats.dsl_statements_generated
        );
        println!(
            "    Statements executed:  {}",
            self.stats.dsl_statements_executed
        );

        println!("\n  DATA IMPORTED:");
        println!("    Funds imported:       {}", self.stats.funds_imported);
        println!("    Entities created:     {}", self.stats.entities_created);
        println!("    CBUs created:         {}", self.stats.cbus_created);
        println!("    Roles assigned:       {}", self.stats.roles_assigned);
        println!(
            "    Parent chains traced: {}",
            self.stats.parent_chains_traced
        );

        println!("\n  PERFORMANCE:");
        println!("    Total time: {:.1}s", self.stats.elapsed_secs);

        if !self.stats.errors.is_empty() {
            println!("\n  ERRORS ({}):", self.stats.errors.len());
            for (i, err) in self.stats.errors.iter().take(5).enumerate() {
                println!("    {}. {}", i + 1, err);
            }
            if self.stats.errors.len() > 5 {
                println!("    ... and {} more", self.stats.errors.len() - 5);
            }
        }

        println!("\n{}", "=".repeat(70));
    }

    fn save_stats(&self) -> Result<()> {
        let stats_path = self.config.output_dir.join("crawl_stats.json");
        let stats_json = serde_json::to_string_pretty(&self.stats)?;
        fs::write(&stats_path, &stats_json)?;
        println!("\n  Stats saved to: {}", stats_path.display());
        Ok(())
    }
}

/// Run the DSL-based GLEIF crawl
pub async fn run_gleif_crawl_dsl(
    root_lei: Option<String>,
    limit: Option<usize>,
    create_cbus: bool,
    trace_parents: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<DslCrawlStats> {
    let config = DslCrawlConfig {
        root_lei: root_lei.unwrap_or_else(|| ALLIANZ_GI_LEI.to_string()),
        limit,
        create_cbus,
        trace_parents,
        dry_run,
        verbose,
        ..Default::default()
    };

    // Connect to database if not dry run
    let pool = if !dry_run {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql:///data_designer".to_string());
        Some(
            PgPool::connect(&db_url)
                .await
                .context("Failed to connect to database")?,
        )
    } else {
        None
    };

    let mut crawler = GleifDslCrawler::new(config, pool)?;
    crawler.crawl().await
}
