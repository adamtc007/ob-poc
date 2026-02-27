//! xtask - Development automation for ob-poc
//!
//! Usage: cargo xtask <command>
//!
//! This provides type-safe, cross-platform build automation that replaces
//! shell scripts with Rust code.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use xshell::{cmd, Shell};

mod allianz_harness;
mod aviva_deal_harness;
mod bpmn_lite;
mod deal_harness;
mod entity;
mod fund_programme;
mod gleif_crawl_dsl;
mod gleif_import;
mod gleif_load;
mod gleif_test;
mod governed_cache;
mod governed_check;
mod harness;
mod lexicon;
mod replay_tuner;
mod seed_allianz;
mod sem_reg;
mod ubo_test;
mod verbs;

#[derive(Parser)]
#[command(name = "xtask")]
#[command(about = "ob-poc development automation")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run all checks (clippy, tests) - fast pre-commit validation
    Check {
        /// Also run database integration tests
        #[arg(long)]
        db: bool,
    },

    /// Run clippy on all feature combinations
    Clippy {
        /// Fix warnings automatically
        #[arg(long)]
        fix: bool,
    },

    /// Run tests
    Test {
        /// Run only lib tests (faster)
        #[arg(long)]
        lib: bool,
        /// Run database integration tests
        #[arg(long)]
        db: bool,
        /// Filter test name
        #[arg(long)]
        filter: Option<String>,
    },

    /// Format code
    Fmt {
        /// Check only, don't modify
        #[arg(long)]
        check: bool,
    },

    /// Build all binaries
    Build {
        /// Build in release mode
        #[arg(long)]
        release: bool,
    },

    /// Export database schema to schema_export.sql
    SchemaExport,

    /// Generate TypeScript bindings from ob-poc-types
    TsBindings,

    /// Start the web server (ob-poc-web)
    Serve {
        /// Port to listen on
        #[arg(long, default_value = "3000")]
        port: u16,
    },

    /// Full CI pipeline (fmt, clippy, test, build)
    Ci,

    /// Pre-commit hook: check + clippy + test
    PreCommit,

    /// Build and deploy web server, then start
    Deploy {
        /// Build in release mode
        #[arg(long)]
        release: bool,
        /// Port to listen on
        #[arg(long, default_value = "3000")]
        port: u16,
        /// Don't start the server after building
        #[arg(long)]
        no_run: bool,
        /// Skip React frontend build (faster for backend-only changes)
        #[arg(long)]
        skip_frontend: bool,
    },

    /// Seed Allianz test data from scraped JSON
    SeedAllianz {
        /// Path to scraped JSON file (default: scrapers/allianz/output/allianz-lu-*.json)
        #[arg(long)]
        file: Option<std::path::PathBuf>,

        /// Only seed first N funds (for testing)
        #[arg(long)]
        limit: Option<usize>,

        /// Skip cleaning existing Allianz data
        #[arg(long)]
        no_clean: bool,

        /// Dry run - show what would be done without making changes
        #[arg(long)]
        dry_run: bool,
    },

    /// Clean all Allianz test data from the database
    CleanAllianz {
        /// Dry run - show what would be deleted without making changes
        #[arg(long)]
        dry_run: bool,
    },

    /// Run batch import test using onboard-fund-cbu template
    BatchImport {
        /// Limit number of funds to process (default: all)
        #[arg(long)]
        limit: Option<usize>,

        /// Dry run - expand templates but don't execute
        #[arg(long)]
        dry_run: bool,

        /// Show expanded DSL for each entity
        #[arg(long)]
        verbose: bool,

        /// Products to add via agent after CBU creation (comma-separated codes)
        /// Example: --add-products "CUSTODY,FUND_ACCOUNTING"
        #[arg(long)]
        add_products: Option<String>,

        /// Agent API URL for DSL generation (default: http://localhost:3000)
        #[arg(long, default_value = "http://localhost:3000")]
        agent_url: String,
    },

    /// Clean CBUs created by batch import (using cbu.delete-cascade DSL verb)
    BatchClean {
        /// Limit number of CBUs to delete (default: all)
        #[arg(long)]
        limit: Option<usize>,

        /// Dry run - show what would be deleted without making changes
        #[arg(long)]
        dry_run: bool,
    },

    /// Run UBO convergence test harness
    UboTest {
        /// Test command: scenario-1, scenario-2, scenario-3, all, clean, seed
        #[arg(default_value = "all")]
        command: String,

        /// Show verbose output
        #[arg(long, short)]
        verbose: bool,
    },

    /// Import funds from GLEIF API by search term or manager LEI
    GleifImport {
        /// Search term for GLEIF API (e.g., "Allianz Global Investors")
        #[arg(long, short = 's', required_unless_present = "manager_lei")]
        search: Option<String>,

        /// Manager LEI to fetch managed funds (e.g., "OJ2TIQSVQND4IZYYK658" for AllianzGI)
        #[arg(long, short = 'm', conflicts_with = "search")]
        manager_lei: Option<String>,

        /// Limit number of records to import
        #[arg(long, short = 'l')]
        limit: Option<usize>,

        /// Dry run - show what would be imported
        #[arg(long, short = 'n')]
        dry_run: bool,

        /// Also create CBUs for each fund (with ASSET_OWNER role)
        #[arg(long)]
        create_cbus: bool,
    },

    /// Load Allianz structure via DSL (clean + execute DSL file)
    EtlAllianz {
        /// DSL file to execute (default: data/derived/dsl/allianz_full_etl.dsl)
        #[arg(long, short = 'f')]
        file: Option<std::path::PathBuf>,

        /// Skip cleaning existing data
        #[arg(long)]
        no_clean: bool,

        /// Dry run - validate DSL without executing
        #[arg(long)]
        dry_run: bool,

        /// Skip regenerating DSL from sources (use existing file)
        #[arg(long)]
        no_regen: bool,
    },

    /// Load Allianz GLEIF data from JSON files and generate/execute DSL
    GleifLoad {
        /// Output DSL file (default: data/derived/dsl/allianz_gleif_load.dsl)
        #[arg(long, short = 'o')]
        output: Option<std::path::PathBuf>,

        /// Limit number of funds to include
        #[arg(long)]
        fund_limit: Option<usize>,

        /// Limit number of corporate subsidiaries to include (legacy mode only)
        #[arg(long)]
        corp_limit: Option<usize>,

        /// Dry run - generate DSL but don't save or execute
        #[arg(long)]
        dry_run: bool,

        /// Execute the generated DSL against the database
        #[arg(long, short = 'x')]
        execute: bool,

        /// Use complete funds file (417 funds with umbrella data) instead of legacy sample
        #[arg(long, short = 'c')]
        complete: bool,
    },

    /// Allianz Test Harness - Complete GLEIF/BODS onboarding pipeline test
    AllianzHarness {
        /// Mode: discover, import, clean, full
        #[arg(long, short = 'm', default_value = "full")]
        mode: String,

        /// Limit number of funds to process
        #[arg(long, short = 'l')]
        limit: Option<usize>,

        /// Dry run - show what would be done without making changes
        #[arg(long, short = 'n')]
        dry_run: bool,

        /// Verbose output
        #[arg(long, short = 'v')]
        verbose: bool,
    },

    /// GLEIF Verb Test Harness - Test all GLEIF DSL verbs with Allianz seed data
    GleifTest {
        /// Verbose output
        #[arg(long, short = 'v')]
        verbose: bool,
    },

    /// GLEIF Crawl - DSL-based entity import from GLEIF API
    ///
    /// This command uses DSL verbs (gleif.import-managed-funds, gleif.trace-ownership)
    /// instead of raw SQL for data import.
    GleifCrawl {
        /// Root LEI to start crawl from (default: Allianz GI)
        #[arg(long)]
        root_lei: Option<String>,

        /// Maximum funds to import (default: unlimited)
        #[arg(long)]
        limit: Option<usize>,

        /// Create CBUs for each fund
        #[arg(long, default_value = "true")]
        create_cbus: bool,

        /// Trace parent ownership chains
        #[arg(long, default_value = "true")]
        trace_parents: bool,

        /// Dry run - generate DSL but don't execute
        #[arg(long)]
        dry_run: bool,

        /// Verbose output
        #[arg(long, short = 'v')]
        verbose: bool,
    },

    /// Verb contract management commands
    Verbs {
        #[command(subcommand)]
        action: VerbsAction,
    },

    /// Lexicon service commands (compile, lint, bench)
    ///
    /// Manages the lexical vocabulary snapshot used for fast verb discovery.
    Lexicon {
        #[command(subcommand)]
        action: LexiconAction,
    },

    /// Semantic Registry commands (stats, describe, list, history, scan)
    ///
    /// Manages the immutable snapshot-based Semantic OS registry.
    SemReg {
        #[command(subcommand)]
        action: SemRegAction,
    },

    /// Test verb search and semantic matching
    ///
    /// Runs the verb search integration tests to verify semantic matching
    /// after teaching new phrases or tuning thresholds.
    TestVerbs {
        /// Run all scenarios (default)
        #[arg(long)]
        all: bool,

        /// Run only taught phrase tests
        #[arg(long)]
        taught: bool,

        /// Run threshold sweep to find optimal settings
        #[arg(long)]
        sweep: bool,

        /// Run hard negative tests (dangerous confusion detection)
        #[arg(long)]
        hard_negatives: bool,

        /// Explore a specific query interactively
        #[arg(long)]
        explore: Option<String>,

        /// Dump mismatched results to JSON file for analysis
        #[arg(long)]
        dump_mismatch: Option<std::path::PathBuf>,
    },

    /// Lexicon test harness - stress test tokenizer and intent parser
    LexiconHarness {
        /// Load test cases from YAML file
        #[arg(long, short = 'p')]
        prompts: Option<std::path::PathBuf>,

        /// Run built-in standard test cases
        #[arg(long)]
        standard: bool,

        /// Only run tests with this tag
        #[arg(long, short = 'f')]
        filter: Option<String>,

        /// Output results as JSON
        #[arg(long)]
        json: bool,

        /// Interactive mode - enter prompts to test
        #[arg(long, short = 'i')]
        interactive: bool,
    },

    /// Test staged runbook REPL (anti-hallucination execution model)
    ///
    /// Runs integration tests for the staged runbook system:
    /// - Commands staged, never auto-executed
    /// - Entity resolution from DB
    /// - Picker loop validation
    /// - DAG ordering
    TestRunbook {
        /// Run all tests (default)
        #[arg(long)]
        all: bool,

        /// Run only basic staging tests
        #[arg(long)]
        staging: bool,

        /// Run only lifecycle tests (abort, remove)
        #[arg(long)]
        lifecycle: bool,

        /// Run only DAG ordering tests
        #[arg(long)]
        dag: bool,

        /// Run the full Irish funds flow test
        #[arg(long)]
        irish_funds: bool,

        /// Filter by test name
        #[arg(long, short = 'f')]
        filter: Option<String>,
    },

    /// Load fund programme from CSV using config-driven column mapping
    ///
    /// Generic loader that supports any fund programme (Allianz, BlackRock, Vanguard, etc.)
    /// by using a YAML config file to map CSV columns to database fields.
    LoadFundProgramme {
        /// Path to YAML config file with column mappings
        #[arg(long, short = 'c')]
        config: std::path::PathBuf,

        /// Path to CSV input file
        #[arg(long, short = 'i')]
        input: std::path::PathBuf,

        /// Output DSL file (default: stdout)
        #[arg(long, short = 'o')]
        output: Option<std::path::PathBuf>,

        /// Limit number of records to process
        #[arg(long, short = 'l')]
        limit: Option<usize>,

        /// Dry run - generate DSL but don't save
        #[arg(long, short = 'n')]
        dry_run: bool,

        /// Execute the generated DSL against the database
        #[arg(long, short = 'x')]
        execute: bool,
    },

    /// Check and validate playbook files
    ///
    /// Parses playbook YAML files and reports any errors or missing slots.
    PlaybookCheck {
        /// Path to playbook file(s) to check (glob patterns supported)
        #[arg(required = true)]
        files: Vec<std::path::PathBuf>,

        /// Output format: text (default) or json
        #[arg(long, short = 'f', default_value = "text")]
        format: String,

        /// Show verbose output including slot analysis
        #[arg(long, short = 'v')]
        verbose: bool,
    },

    /// Entity linking service commands (compile, lint, stats)
    ///
    /// Manages the entity snapshot used for fast entity resolution.
    Entity {
        #[command(subcommand)]
        action: EntityAction,
    },

    /// Deal Hierarchy Test Harness - Tests Deal → Products → Rate Cards DAG
    ///
    /// Creates test data using DSL verbs only (no direct SQL):
    /// - Creates a deal with products
    /// - Creates contracts and links to deal
    /// - Creates rate cards linked to products/contracts
    /// - Validates all deal.products are linked to a contract
    /// - Validates precedence constraints (one AGREED rate card per combo)
    DealHarness {
        /// Verbose output
        #[arg(long, short = 'v')]
        verbose: bool,

        /// Dry run - show what would be done without making changes
        #[arg(long, short = 'n')]
        dry_run: bool,

        /// Clean up test data after harness completes
        #[arg(long)]
        cleanup: bool,
    },

    /// Aviva Deal Test Harness - Full deal setup for Aviva Investors
    ///
    /// Creates a complete deal using DSL verbs only (idempotent, re-runnable):
    /// - Creates a deal for Aviva Investors client group
    /// - Adds all products to the deal scope
    /// - Creates 2 contracts (Core Services + Ancillary Services)
    /// - Links Custody and Fund Accounting to Contract 1
    /// - Links all other products to Contract 2
    /// - Creates rate cards with made-up rates for all products
    /// - Adds fee lines to each rate card
    AvivaDealHarness {
        /// Verbose output - show DSL statements
        #[arg(long, short = 'v')]
        verbose: bool,

        /// Dry run - show what would be done without making changes
        #[arg(long, short = 'n')]
        dry_run: bool,
    },

    /// Replay-tuner — Offline replay of decision logs with scoring constant sweeps
    ///
    /// Loads golden corpus YAML and/or session decision logs, then replays them
    /// with different scoring configurations to find optimal constants.
    ReplayTuner {
        #[command(subcommand)]
        action: ReplayTunerAction,
    },

    /// Agentic Scenario Harness — deterministic pipeline testing
    ///
    /// Drives multi-turn YAML scenarios through the orchestrator.
    /// Asserts structured fields only (outcome, verb, SemReg, trace).
    Harness {
        #[command(subcommand)]
        action: HarnessAction,
    },

    /// BPMN-Lite service commands (build, test, clippy, docker, deploy)
    ///
    /// Manages the standalone bpmn-lite orchestration service at bpmn-lite/.
    BpmnLite {
        #[command(subcommand)]
        action: BpmnLiteAction,
    },

    /// GovernedQuery cache management (refresh, stats)
    ///
    /// Generates the bincode cache file used by #[governed_query] proc macro
    /// for compile-time governance verification.
    GovernedCache {
        #[command(subcommand)]
        action: GovernedCacheAction,
    },

    /// GovernedQuery governance checker
    ///
    /// Scans source files for #[governed_query] annotations and checks
    /// against the live Semantic OS registry. Reports hard errors
    /// (GC001-GC003) and soft warnings (GC010-GC011).
    GovernedCheck {
        /// Fail with non-zero exit code on hard errors (for CI)
        #[arg(long)]
        strict: bool,
    },
}

#[derive(Subcommand)]
enum HarnessAction {
    /// List all scenario suites and counts
    List,
    /// Run scenarios
    Run {
        /// Path to a specific suite YAML file
        #[arg(long)]
        suite: Option<std::path::PathBuf>,
        /// Run a specific scenario by name (searches all suites)
        #[arg(long)]
        scenario: Option<String>,
        /// Run all suites
        #[arg(long)]
        all: bool,
    },
    /// Dump full artifacts for a scenario
    Dump {
        /// Scenario name to dump
        #[arg(long)]
        scenario: String,
        /// Output file path
        #[arg(long, default_value = "/tmp/harness_dump.json")]
        out: std::path::PathBuf,
    },
}

#[derive(Subcommand)]
enum BpmnLiteAction {
    /// Build the bpmn-lite workspace
    Build {
        /// Build in release mode
        #[arg(long)]
        release: bool,
    },

    /// Run all bpmn-lite tests
    Test {
        /// Filter test name
        #[arg(long, short = 'f')]
        filter: Option<String>,
    },

    /// Run clippy on bpmn-lite workspace
    Clippy,

    /// Start the bpmn-lite gRPC server (native, release build, background)
    Start {
        /// Port to listen on
        #[arg(long, short = 'p', default_value = "50051")]
        port: u16,
    },

    /// Stop the bpmn-lite gRPC server
    Stop {
        /// Port the server is listening on
        #[arg(long, short = 'p', default_value = "50051")]
        port: u16,
    },

    /// Show bpmn-lite service status (native and Docker)
    Status {
        /// Port to check
        #[arg(long, short = 'p', default_value = "50051")]
        port: u16,
    },

    /// Build Docker image for bpmn-lite
    DockerBuild,

    /// Build and deploy bpmn-lite via docker compose
    Deploy {
        /// Skip Docker image rebuild (use existing image)
        #[arg(long)]
        skip_build: bool,
    },
}

#[derive(Subcommand)]
enum GovernedCacheAction {
    /// Refresh the governed cache from the database
    ///
    /// Queries sem_reg.snapshots for active entries and writes
    /// assets/governed_cache.bin for compile-time verification.
    Refresh,

    /// Show governed cache statistics
    ///
    /// Reads the cache file and prints counts by object type,
    /// governance tier, and PII status.
    Stats,
}

#[derive(Subcommand)]
enum EntityAction {
    /// Compile entity snapshot from database
    ///
    /// Reads entities, aliases, and concept links from the database
    /// and produces entity.snapshot.bin for fast runtime loading.
    Compile {
        /// Output snapshot file (default: assets/entity.snapshot.bin)
        #[arg(long, short = 'o')]
        output: Option<std::path::PathBuf>,

        /// Show verbose output
        #[arg(long, short = 'v')]
        verbose: bool,
    },

    /// Lint entity data for quality issues
    ///
    /// Checks for:
    /// - Empty name_norm values
    /// - Ambiguous aliases (same alias → multiple entities)
    /// - Missing concept links
    Lint {
        /// Show only errors, not warnings
        #[arg(long)]
        errors_only: bool,
    },

    /// Show entity snapshot statistics
    Stats {
        /// Path to snapshot file (default: assets/entity.snapshot.bin)
        #[arg(long, short = 's')]
        snapshot: Option<std::path::PathBuf>,
    },
}

#[derive(Subcommand)]
enum VerbsAction {
    /// Compile all verbs from YAML and sync to database
    Compile {
        /// Show additional details
        #[arg(long, short = 'v')]
        verbose: bool,
    },

    /// Show compiled contract for a specific verb
    Show {
        /// Verb name (e.g., 'cbu.ensure' or 'entity.create-proper-person')
        verb_name: String,
    },

    /// Show all verbs with diagnostics (errors or warnings)
    Diagnostics {
        /// Show only errors, not warnings
        #[arg(long)]
        errors_only: bool,
    },

    /// Check if verb configs are up-to-date (CI check)
    ///
    /// Compares YAML config hashes to database compiled hashes.
    /// Exits with code 1 if any verbs need recompilation.
    Check {
        /// Show details even when all verbs are up-to-date
        #[arg(long, short = 'v')]
        verbose: bool,
    },

    /// Lint verbs for tiering rule compliance
    ///
    /// Validates verb metadata against tiering rules from 027-trading-matrix-canonical-pivot.
    /// Exits with code 1 if any verbs have tiering errors.
    Lint {
        /// Show only errors, not warnings
        #[arg(long)]
        errors_only: bool,

        /// Show tier distribution and additional details
        #[arg(long, short = 'v')]
        verbose: bool,

        /// Lint tier level: minimal, basic, standard (default: minimal)
        #[arg(long, short = 't', default_value = "minimal")]
        tier: String,
    },

    /// Generate verb inventory report
    ///
    /// Creates a comprehensive report of all verbs grouped by domain, tier, and noun.
    /// Useful for documentation and auditing.
    Inventory {
        /// Output markdown file (default: docs/verb-inventory.md)
        #[arg(long, short = 'o')]
        output: Option<std::path::PathBuf>,

        /// Also update CLAUDE.md header stats
        #[arg(long)]
        update_claude_md: bool,

        /// Show verbs missing metadata
        #[arg(long)]
        show_untagged: bool,
    },

    /// Lint macro schema files
    ///
    /// Validates macro YAML schemas against lint rules (MACRO000-MACRO080):
    /// - UI fields required (label, description, target_label)
    /// - No forbidden tokens (cbu, entity_ref, etc.)
    /// - Operator types only (structure_ref, party_ref, etc.)
    /// - Enum args must use ${arg.X.internal} in expansion
    LintMacros {
        /// Show only errors, not warnings
        #[arg(long)]
        errors_only: bool,

        /// Show verbose output
        #[arg(long, short = 'v')]
        verbose: bool,
    },

    /// Lint durable verb YAML definitions
    ///
    /// Validates verbs with `behavior: durable` have correct config:
    /// - process_key is non-empty
    /// - correlation_field references a verb arg
    /// - timeout is valid ISO 8601 duration (if present)
    /// - task_bindings values reference known verbs
    /// - runtime is a valid variant
    LintDurable {
        /// Show only errors, not warnings
        #[arg(long)]
        errors_only: bool,

        /// Show verbose output
        #[arg(long, short = 'v')]
        verbose: bool,
    },

    /// Generate comprehensive verb atlas with findings
    ///
    /// Produces a full report of every verb in the system including:
    /// - Metadata (tier, domain, scope, invocation phrases)
    /// - Pack membership (which packs include/forbid each verb)
    /// - Handler existence (plugin verbs with registered CustomOps)
    /// - Collision detection (exact phrase collisions, near-collisions)
    /// - Lint findings (ghost verbs, missing tier, control verbs in packs, etc.)
    ///
    /// Outputs:
    /// - docs/generated/verb_atlas.md (full table)
    /// - docs/generated/verb_atlas.json (machine-readable)
    /// - docs/generated/verb_findings.md (findings by severity)
    /// - docs/generated/verb_phrase_collisions.md (collision report)
    Atlas {
        /// Output directory (default: docs/generated/)
        #[arg(long, short = 'o')]
        output: Option<std::path::PathBuf>,

        /// Only run lint checks and exit with non-zero on errors (CI mode)
        #[arg(long)]
        lint_only: bool,

        /// Show verbose output
        #[arg(long, short = 'v')]
        verbose: bool,
    },
}

#[derive(Subcommand)]
enum LexiconAction {
    /// Compile lexicon from YAML to binary snapshot
    ///
    /// Reads verb_concepts.yaml, entity_types.yaml, domains.yaml
    /// and produces lexicon.snapshot.bin for fast runtime loading.
    Compile {
        /// Config root directory (default: config/)
        #[arg(long, short = 'c')]
        config_root: Option<std::path::PathBuf>,

        /// Output snapshot file (default: assets/lexicon.snapshot.bin)
        #[arg(long, short = 'o')]
        output: Option<std::path::PathBuf>,

        /// Show verbose output
        #[arg(long, short = 'v')]
        verbose: bool,
    },

    /// Lint lexicon YAML files for consistency
    Lint {
        /// Config root directory (default: config/)
        #[arg(long, short = 'c')]
        config_root: Option<std::path::PathBuf>,

        /// Show only errors, not warnings
        #[arg(long)]
        errors_only: bool,
    },

    /// Benchmark lexicon search performance
    ///
    /// Loads a compiled snapshot and runs search queries to measure latency.
    Bench {
        /// Path to compiled snapshot (default: assets/lexicon.snapshot.bin)
        #[arg(long, short = 's')]
        snapshot: Option<std::path::PathBuf>,

        /// Number of iterations
        #[arg(long, short = 'n', default_value = "10000")]
        iterations: usize,
    },
}

#[derive(Subcommand)]
enum SemRegAction {
    /// Show registry statistics (counts by object type)
    Stats,

    /// Describe an attribute definition by FQN
    AttrDescribe {
        /// Attribute FQN (e.g., "cbu.jurisdiction_code")
        fqn: String,
    },

    /// List active attribute definitions
    AttrList {
        /// Maximum number of results
        #[arg(long, short = 'n', default_value = "100")]
        limit: i64,
    },

    /// Describe an entity type definition by FQN
    EntityTypeDescribe {
        /// Entity type FQN (e.g., "entity.fund")
        fqn: String,
    },

    /// Describe a verb contract by FQN
    VerbDescribe {
        /// Verb FQN (e.g., "cbu.create")
        fqn: String,
    },

    /// List active verb contracts
    VerbList {
        /// Maximum number of results
        #[arg(long, short = 'n', default_value = "100")]
        limit: i64,
    },

    /// Show snapshot history for a registry object
    History {
        /// Object type (attr, entity-type, verb, taxonomy, policy, evidence, etc.)
        object_type: String,
        /// Object FQN
        fqn: String,
    },

    /// Scan verb YAML and bootstrap registry snapshots
    Scan {
        /// Report counts without writing to database
        #[arg(long)]
        dry_run: bool,
        /// Show per-object detail
        #[arg(long, short = 'v')]
        verbose: bool,
    },

    /// Describe a derivation spec by FQN
    DerivationDescribe {
        /// Derivation FQN (e.g., "kyc.risk_score_derived")
        fqn: String,
    },

    /// Backfill security labels on snapshots with default labels
    BackfillLabels {
        /// Report what would change without writing to database
        #[arg(long)]
        dry_run: bool,
    },

    /// Run publish gates against all active snapshots
    Validate {
        /// Fail with non-zero exit on errors (default: report-only)
        #[arg(long)]
        enforce: bool,
    },

    /// List available Semantic Registry MCP tools (Phase 8)
    AgentTools,

    /// Resolve context for a subject (Phase 7)
    ///
    /// Runs the 12-step context resolution pipeline and returns ranked verbs,
    /// attributes, policy verdicts, and governance signals.
    CtxResolve {
        /// Subject ID (UUID)
        subject: String,
        /// Subject type: case, entity, document, task, view
        #[arg(long, default_value = "entity")]
        subject_type: String,
        /// Actor type: agent, analyst, governance
        #[arg(long, default_value = "analyst")]
        actor: String,
        /// Evidence mode: strict, normal, exploratory, governance
        #[arg(long, default_value = "normal")]
        mode: String,
        /// Point-in-time (ISO 8601), omit for current
        #[arg(long)]
        as_of: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show governance coverage report (Phase 9)
    ///
    /// Aggregates classification, stewardship, policy attachment, evidence
    /// freshness, and security label metrics across the registry.
    Coverage {
        /// Filter by governance tier: governed, operational, or all (default)
        #[arg(long, default_value = "all")]
        tier: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Scan schema + verbs and produce onboarding manifest (read-only)
    ///
    /// Runs the 5-step extraction pipeline (verb extract, schema introspect,
    /// cross-reference, entity inference, manifest assembly) and writes
    /// the result to data/onboarding-manifest.json.
    OnboardScan {
        /// Show per-step detail
        #[arg(long, short = 'v')]
        verbose: bool,
    },

    /// Display onboarding manifest summary report
    OnboardReport {
        /// Path to manifest JSON (default: data/onboarding-manifest.json)
        #[arg(long)]
        manifest_path: Option<String>,
    },

    /// Apply bootstrap seed from manifest (one-time write to sem_reg.snapshots)
    ///
    /// Reads an onboarding manifest and writes AttributeDefs, VerbContracts,
    /// EntityTypeDefs, and RelationshipTypeDefs to the semantic registry.
    /// Protected by BOOTSTRAP_SET_ID — refuses to run if already bootstrapped.
    OnboardApply {
        /// Path to manifest JSON (default: data/onboarding-manifest.json)
        #[arg(long)]
        manifest_path: Option<String>,
    },

    // ── Authoring Pipeline (Governed Change Boundary) ───────
    /// List authoring pipeline ChangeSets
    AuthoringList {
        /// Filter by status (draft, validated, dry_run_passed, etc.)
        #[arg(long)]
        status: Option<String>,
        /// Maximum number of results
        #[arg(long, short = 'n', default_value = "50")]
        limit: i64,
    },

    /// Show details for a single ChangeSet
    AuthoringGet {
        /// ChangeSet UUID
        id: String,
    },

    /// Propose a new ChangeSet from a bundle directory
    AuthoringPropose {
        /// Path to bundle directory containing changeset.yaml + artifacts
        bundle_path: String,
    },

    /// Run Stage 1 validation on a ChangeSet (Draft → Validated/Rejected)
    AuthoringValidate {
        /// ChangeSet UUID
        id: String,
    },

    /// Run Stage 2 dry-run on a ChangeSet (Validated → DryRunPassed/DryRunFailed)
    AuthoringDryRun {
        /// ChangeSet UUID
        id: String,
    },

    /// Generate a publish plan (diff) for a ChangeSet (read-only)
    AuthoringPlan {
        /// ChangeSet UUID
        id: String,
    },

    /// Publish a ChangeSet (DryRunPassed → Published)
    AuthoringPublish {
        /// ChangeSet UUID
        id: String,
        /// Publisher identifier
        #[arg(long, default_value = "cli")]
        publisher: String,
    },

    /// Publish multiple ChangeSets atomically in topological order
    AuthoringPublishBatch {
        /// ChangeSet UUIDs (comma-separated or multiple args)
        ids: Vec<String>,
        /// Publisher identifier
        #[arg(long, default_value = "cli")]
        publisher: String,
    },

    /// Compute structural diff between two ChangeSets
    AuthoringDiff {
        /// Base ChangeSet UUID
        base_id: String,
        /// Target ChangeSet UUID
        target_id: String,
    },

    /// Show authoring pipeline health (pending changesets, stale dry-runs)
    AuthoringHealth,

    /// Archive old terminal/orphan ChangeSets (report-only for now)
    AuthoringCleanup {
        /// Days to retain terminal ChangeSets (Rejected/DryRunFailed)
        #[arg(long, default_value = "90")]
        terminal_days: Option<u32>,
        /// Days to retain orphan ChangeSets (Draft/Validated)
        #[arg(long, default_value = "30")]
        orphan_days: Option<u32>,
    },
}

#[derive(Subcommand)]
enum ReplayTunerAction {
    /// Run golden corpus with current scoring constants
    ///
    /// Loads test cases from golden_corpus/ directory and evaluates them
    /// against the current scoring constants. Reports accuracy by category.
    Run {
        /// Path to golden corpus directory (default: tests/golden_corpus/)
        #[arg(long, short = 'c')]
        corpus: Option<std::path::PathBuf>,

        /// Path to a session decision log JSON file (optional)
        #[arg(long, short = 'l')]
        session_log: Option<std::path::PathBuf>,

        /// Show per-test detail
        #[arg(long, short = 'v')]
        verbose: bool,
    },

    /// Sweep a scoring parameter across a range
    ///
    /// Varies one scoring constant from min to max in steps,
    /// running the full golden corpus at each point. Identifies
    /// the optimal value for that parameter.
    Sweep {
        /// Scoring parameter to sweep
        /// Valid: pack_verb_boost, pack_verb_penalty, template_step_boost,
        /// domain_affinity_boost, absolute_floor, threshold, margin, strong_threshold
        #[arg(long, short = 'p')]
        param: String,

        /// Minimum value for sweep
        #[arg(long, default_value = "0.0")]
        min: f32,

        /// Maximum value for sweep
        #[arg(long, default_value = "0.30")]
        max: f32,

        /// Step size for sweep
        #[arg(long, default_value = "0.05")]
        step: f32,

        /// Path to golden corpus directory
        #[arg(long, short = 'c')]
        corpus: Option<std::path::PathBuf>,

        /// Path to a session decision log JSON file
        #[arg(long, short = 'l')]
        session_log: Option<std::path::PathBuf>,

        /// Write sweep results as JSON to this file
        #[arg(long, short = 'o')]
        output: Option<std::path::PathBuf>,
    },

    /// Compare two report JSON files (baseline vs candidate)
    ///
    /// Shows accuracy delta, category breakdown, regressions, and improvements.
    Compare {
        /// Baseline report JSON file
        #[arg(long, short = 'b')]
        baseline: std::path::PathBuf,

        /// Candidate report JSON file
        #[arg(long, short = 'k')]
        candidate: std::path::PathBuf,
    },

    /// Generate report from a session decision log
    ///
    /// Shows turn-by-turn breakdown of verb matching, entity resolution,
    /// and extraction methods from a recorded session.
    Report {
        /// Session decision log JSON file
        #[arg(long, short = 'l')]
        session_log: std::path::PathBuf,

        /// Show per-turn detail
        #[arg(long, short = 'v')]
        verbose: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let sh = Shell::new()?;

    // Change to rust directory
    let rust_dir = project_root()?.join("rust");
    sh.change_dir(&rust_dir);

    match cli.command {
        Command::Check { db } => check(&sh, db),
        Command::Clippy { fix } => clippy(&sh, fix),
        Command::Test { lib, db, filter } => test(&sh, lib, db, filter),
        Command::Fmt { check } => fmt(&sh, check),
        Command::Build { release } => build(&sh, release),
        Command::SchemaExport => schema_export(&sh),
        Command::TsBindings => ts_bindings(&sh),

        Command::Serve { port } => serve(&sh, port),
        Command::Ci => ci(&sh),
        Command::PreCommit => pre_commit(&sh),
        Command::Deploy {
            release,
            port,
            no_run,
            skip_frontend,
        } => deploy(&sh, release, port, no_run, skip_frontend),
        Command::SeedAllianz {
            file,
            limit,
            no_clean,
            dry_run,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(seed_allianz::seed_allianz(file, limit, no_clean, dry_run))
        }
        Command::CleanAllianz { dry_run } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(seed_allianz::clean_allianz(dry_run))
        }
        Command::BatchImport {
            limit,
            dry_run,
            verbose,
            add_products,
            agent_url,
        } => batch_import(&sh, limit, dry_run, verbose, add_products, agent_url),
        Command::BatchClean { limit, dry_run } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(batch_clean(limit, dry_run))
        }
        Command::UboTest { command, verbose } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(ubo_test::run_ubo_test(&command, verbose))
        }
        Command::GleifImport {
            search,
            manager_lei,
            limit,
            dry_run,
            create_cbus,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(gleif_import::gleif_import(
                search.as_deref(),
                manager_lei.as_deref(),
                limit,
                dry_run,
                create_cbus,
            ))
        }
        Command::EtlAllianz {
            file,
            no_clean,
            dry_run,
            no_regen,
        } => etl_allianz(&sh, file, no_clean, dry_run, no_regen),
        Command::GleifLoad {
            output,
            fund_limit,
            corp_limit,
            dry_run,
            execute,
            complete,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(gleif_load::gleif_load(
                output, fund_limit, corp_limit, dry_run, execute, complete,
            ))
        }
        Command::AllianzHarness {
            mode,
            limit,
            dry_run,
            verbose,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(run_allianz_harness(&mode, limit, dry_run, verbose))
        }
        Command::GleifTest { verbose } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(gleif_test::run_gleif_tests(verbose))
        }
        Command::GleifCrawl {
            root_lei,
            limit,
            create_cbus,
            trace_parents,
            dry_run,
            verbose,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(gleif_crawl_dsl::run_gleif_crawl_dsl(
                root_lei,
                limit,
                create_cbus,
                trace_parents,
                dry_run,
                verbose,
            ))?;
            Ok(())
        }
        Command::Verbs { action } => {
            let rt = tokio::runtime::Runtime::new()?;
            match action {
                VerbsAction::Compile { verbose } => rt.block_on(verbs::verbs_compile(verbose)),
                VerbsAction::Show { verb_name } => rt.block_on(verbs::verbs_show(&verb_name)),
                VerbsAction::Diagnostics { errors_only } => {
                    rt.block_on(verbs::verbs_diagnostics(errors_only))
                }
                VerbsAction::Check { verbose } => rt.block_on(verbs::verbs_check(verbose)),
                VerbsAction::Lint {
                    errors_only,
                    verbose,
                    tier,
                } => rt.block_on(verbs::verbs_lint(errors_only, verbose, &tier)),
                VerbsAction::Inventory {
                    output,
                    update_claude_md,
                    show_untagged,
                } => verbs::verbs_inventory(output, update_claude_md, show_untagged),
                VerbsAction::LintMacros {
                    errors_only,
                    verbose,
                } => lint_macros(errors_only, verbose),
                VerbsAction::LintDurable {
                    errors_only,
                    verbose,
                } => verbs::verbs_lint_durable(errors_only, verbose),
                VerbsAction::Atlas {
                    output,
                    lint_only,
                    verbose,
                } => verbs::verbs_atlas(output, lint_only, verbose),
            }
        }
        Command::Lexicon { action } => match action {
            LexiconAction::Compile {
                config_root,
                output,
                verbose,
            } => lexicon::compile(config_root.as_deref(), output.as_deref(), verbose),
            LexiconAction::Lint {
                config_root,
                errors_only,
            } => lexicon::lint(config_root.as_deref(), errors_only),
            LexiconAction::Bench {
                snapshot,
                iterations,
            } => lexicon::bench(snapshot.as_deref(), iterations),
        },
        Command::SemReg { action } => {
            let rt = tokio::runtime::Runtime::new()?;
            match action {
                SemRegAction::Stats => rt.block_on(sem_reg::stats()),
                SemRegAction::AttrDescribe { fqn } => rt.block_on(sem_reg::attr_describe(&fqn)),
                SemRegAction::AttrList { limit } => rt.block_on(sem_reg::attr_list(limit)),
                SemRegAction::EntityTypeDescribe { fqn } => {
                    rt.block_on(sem_reg::entity_type_describe(&fqn))
                }
                SemRegAction::VerbDescribe { fqn } => rt.block_on(sem_reg::verb_describe(&fqn)),
                SemRegAction::VerbList { limit } => rt.block_on(sem_reg::verb_list(limit)),
                SemRegAction::History { object_type, fqn } => {
                    rt.block_on(sem_reg::history(&object_type, &fqn))
                }
                SemRegAction::Scan { dry_run, verbose } => {
                    rt.block_on(sem_reg::scan(dry_run, verbose))
                }
                SemRegAction::DerivationDescribe { fqn } => {
                    rt.block_on(sem_reg::derivation_describe(&fqn))
                }
                SemRegAction::BackfillLabels { dry_run } => {
                    rt.block_on(sem_reg::backfill_labels(dry_run))
                }
                SemRegAction::Validate { enforce } => rt.block_on(sem_reg::validate(enforce)),
                SemRegAction::AgentTools => rt.block_on(sem_reg::agent_tools()),
                SemRegAction::CtxResolve {
                    subject,
                    subject_type,
                    actor,
                    mode,
                    as_of,
                    json,
                } => rt.block_on(sem_reg::ctx_resolve(
                    &subject,
                    &subject_type,
                    &actor,
                    &mode,
                    as_of.as_deref(),
                    json,
                )),
                SemRegAction::Coverage { tier, json } => {
                    rt.block_on(sem_reg::coverage(&tier, json))
                }
                SemRegAction::OnboardScan { verbose } => {
                    rt.block_on(sem_reg::onboard_scan(verbose))
                }
                SemRegAction::OnboardReport { manifest_path } => {
                    rt.block_on(sem_reg::onboard_report(manifest_path.as_deref()))
                }
                SemRegAction::OnboardApply { manifest_path } => {
                    rt.block_on(sem_reg::onboard_apply(manifest_path.as_deref()))
                }
                // ── Authoring Pipeline ──
                SemRegAction::AuthoringList { status, limit } => {
                    rt.block_on(sem_reg::authoring_list(status.as_deref(), limit))
                }
                SemRegAction::AuthoringGet { id } => rt.block_on(sem_reg::authoring_get(&id)),
                SemRegAction::AuthoringPropose { bundle_path } => {
                    rt.block_on(sem_reg::authoring_propose(&bundle_path))
                }
                SemRegAction::AuthoringValidate { id } => {
                    rt.block_on(sem_reg::authoring_validate(&id))
                }
                SemRegAction::AuthoringDryRun { id } => {
                    rt.block_on(sem_reg::authoring_dry_run(&id))
                }
                SemRegAction::AuthoringPlan { id } => rt.block_on(sem_reg::authoring_plan(&id)),
                SemRegAction::AuthoringPublish { id, publisher } => {
                    rt.block_on(sem_reg::authoring_publish(&id, &publisher))
                }
                SemRegAction::AuthoringPublishBatch { ids, publisher } => {
                    rt.block_on(sem_reg::authoring_publish_batch(&ids, &publisher))
                }
                SemRegAction::AuthoringDiff { base_id, target_id } => {
                    rt.block_on(sem_reg::authoring_diff(&base_id, &target_id))
                }
                SemRegAction::AuthoringHealth => rt.block_on(sem_reg::authoring_health()),
                SemRegAction::AuthoringCleanup {
                    terminal_days,
                    orphan_days,
                } => rt.block_on(sem_reg::authoring_cleanup(terminal_days, orphan_days)),
            }
        }
        Command::Entity { action } => {
            let rt = tokio::runtime::Runtime::new()?;
            match action {
                EntityAction::Compile { output, verbose } => {
                    rt.block_on(entity::compile(output.as_deref(), verbose))
                }
                EntityAction::Lint { errors_only } => rt.block_on(entity::lint(errors_only)),
                EntityAction::Stats { snapshot } => entity::stats(snapshot.as_deref()),
            }
        }
        Command::LoadFundProgramme {
            config,
            input,
            output,
            limit,
            dry_run,
            execute,
        } => run_load_fund_programme(&config, &input, output.as_deref(), limit, dry_run, execute),
        Command::PlaybookCheck {
            files,
            format,
            verbose,
        } => playbook_check(files, &format, verbose),

        Command::LexiconHarness {
            prompts,
            standard,
            filter,
            json,
            interactive,
        } => run_lexicon_harness(&sh, prompts, standard, filter, json, interactive),
        Command::TestVerbs {
            all,
            taught,
            sweep,
            hard_negatives,
            explore,
            dump_mismatch,
        } => test_verbs(
            &sh,
            all,
            taught,
            sweep,
            hard_negatives,
            explore,
            dump_mismatch,
        ),
        Command::TestRunbook {
            all,
            staging,
            lifecycle,
            dag,
            irish_funds,
            filter,
        } => test_runbook(&sh, all, staging, lifecycle, dag, irish_funds, filter),
        Command::DealHarness {
            verbose,
            dry_run,
            cleanup,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(run_deal_harness(verbose, dry_run, cleanup))
        }
        Command::AvivaDealHarness { verbose, dry_run } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(run_aviva_deal_harness(verbose, dry_run))
        }
        Command::ReplayTuner { action } => match action {
            ReplayTunerAction::Run {
                corpus,
                session_log,
                verbose,
            } => replay_tuner::run(corpus.as_deref(), session_log.as_deref(), verbose),
            ReplayTunerAction::Sweep {
                param,
                min,
                max,
                step,
                corpus,
                session_log,
                output,
            } => replay_tuner::sweep(
                corpus.as_deref(),
                session_log.as_deref(),
                &param,
                min,
                max,
                step,
                output.as_deref(),
            ),
            ReplayTunerAction::Compare {
                baseline,
                candidate,
            } => replay_tuner::compare(&baseline, &candidate),
            ReplayTunerAction::Report {
                session_log,
                verbose,
            } => replay_tuner::report(&session_log, verbose),
        },
        Command::Harness { action } => {
            let scenarios_dir = std::path::Path::new("scenarios/suites");
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            match action {
                HarnessAction::List => {
                    harness::list(scenarios_dir)?;
                    Ok(())
                }
                HarnessAction::Run {
                    suite,
                    scenario,
                    all,
                } => {
                    let db_url = std::env::var("DATABASE_URL")
                        .unwrap_or_else(|_| "postgresql:///data_designer".into());
                    let pool = rt.block_on(sqlx::PgPool::connect(&db_url))?;
                    let passed = rt.block_on(harness::run(
                        &pool,
                        scenarios_dir,
                        suite.as_deref(),
                        scenario.as_deref(),
                        all,
                    ))?;
                    if !passed {
                        std::process::exit(1);
                    }
                    Ok(())
                }
                HarnessAction::Dump { scenario, out } => {
                    let db_url = std::env::var("DATABASE_URL")
                        .unwrap_or_else(|_| "postgresql:///data_designer".into());
                    let pool = rt.block_on(sqlx::PgPool::connect(&db_url))?;
                    rt.block_on(harness::dump(&pool, scenarios_dir, &scenario, &out))?;
                    Ok(())
                }
            }
        }
        Command::BpmnLite { action } => match action {
            BpmnLiteAction::Build { release } => bpmn_lite::build(&sh, release),
            BpmnLiteAction::Test { filter } => bpmn_lite::test(&sh, filter.as_deref()),
            BpmnLiteAction::Clippy => bpmn_lite::clippy(&sh),
            BpmnLiteAction::Start { port } => bpmn_lite::start(&sh, port),
            BpmnLiteAction::Stop { port } => bpmn_lite::stop(&sh, port),
            BpmnLiteAction::Status { port } => bpmn_lite::status(&sh, port),
            BpmnLiteAction::DockerBuild => bpmn_lite::docker_build(&sh),
            BpmnLiteAction::Deploy { skip_build } => bpmn_lite::deploy(&sh, !skip_build),
        },
        Command::GovernedCache { action } => {
            let rt = tokio::runtime::Runtime::new()?;
            match action {
                GovernedCacheAction::Refresh => rt.block_on(governed_cache::refresh(None)),
                GovernedCacheAction::Stats => rt.block_on(governed_cache::stats(None)),
            }
        }
        Command::GovernedCheck { strict } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(governed_check::run_check(strict))
        }
    }
}

fn test_verbs(
    sh: &Shell,
    all: bool,
    taught: bool,
    sweep: bool,
    hard_negatives: bool,
    explore: Option<String>,
    dump_mismatch: Option<std::path::PathBuf>,
) -> Result<()> {
    println!("===========================================");
    println!("  Verb Search Test Harness");
    println!("===========================================\n");

    // Build test filter based on flags
    let mut filters: Vec<&str> = Vec::new();

    if taught {
        filters.push("taught_phrase");
    }
    if sweep {
        filters.push("threshold_sweep");
    }
    if hard_negatives {
        filters.push("hard_negative");
    }

    // If explore is specified, run interactive exploration
    if let Some(query) = explore {
        println!("Exploring query: \"{}\"\n", query);
        filters.push("explore_query");
        // Pass query via env var since cargo test doesn't support arbitrary args well
        sh.set_var("VERB_SEARCH_EXPLORE_QUERY", &query);
    }

    // If dump_mismatch is specified, set env var for the test harness
    if let Some(ref path) = dump_mismatch {
        let path_str = path.to_string_lossy().to_string();
        println!("Dumping mismatches to: {}\n", path_str);
        sh.set_var("VERB_SEARCH_DUMP_MISMATCH", &path_str);
        // Run the dump_mismatch test specifically
        filters.clear();
        filters.push("test_dump_mismatches");
    }

    // Default to all if no specific flags
    let filter_arg = if filters.is_empty() || all {
        "verb_search".to_string()
    } else {
        filters.join("|")
    };

    println!("Running tests matching: {}\n", filter_arg);

    // Run the integration tests with database feature
    cmd!(
        sh,
        "cargo test --features database --test verb_search_integration {filter_arg} -- --ignored --nocapture"
    )
    .run()
    .context("Verb search tests failed")?;

    println!("\n===========================================");
    println!("  Tests complete");
    println!("===========================================");

    Ok(())
}

fn test_runbook(
    sh: &Shell,
    all: bool,
    staging: bool,
    lifecycle: bool,
    dag: bool,
    irish_funds: bool,
    filter: Option<String>,
) -> Result<()> {
    println!("===========================================");
    println!("  Staged Runbook Integration Tests");
    println!("  (Anti-Hallucination Execution Model)");
    println!("===========================================\n");

    // Build test filter based on flags
    let filter_arg = if let Some(f) = filter {
        f
    } else if all || (!staging && !lifecycle && !dag && !irish_funds) {
        // Default: run all tests
        "test_".to_string()
    } else {
        let mut filters: Vec<&str> = Vec::new();
        if staging {
            filters.push("test_stage");
        }
        if lifecycle {
            filters.push("test_abort|test_remove");
        }
        if dag {
            filters.push("test_dag");
        }
        if irish_funds {
            filters.push("test_irish_funds");
        }
        filters.join("|")
    };

    println!("Running tests matching: {}\n", filter_arg);

    // Run the integration tests with database feature
    cmd!(
        sh,
        "cargo test --features database --test staged_runbook_integration {filter_arg} -- --ignored --nocapture"
    )
    .run()
    .context("Staged runbook tests failed")?;

    println!("\n===========================================");
    println!("  Tests complete");
    println!("===========================================");

    Ok(())
}

fn run_lexicon_harness(
    sh: &Shell,
    prompts: Option<std::path::PathBuf>,
    standard: bool,
    filter: Option<String>,
    json: bool,
    interactive: bool,
) -> Result<()> {
    // Build the lexicon_harness binary
    cmd!(
        sh,
        "cargo build -p ob-agentic --features cli --bin lexicon_harness"
    )
    .run()
    .context("Failed to build lexicon_harness")?;

    // Use std::process::Command for proper argument handling
    let mut cmd = std::process::Command::new("./target/debug/lexicon_harness");

    if let Some(ref path) = prompts {
        cmd.args(["--prompts", &path.to_string_lossy()]);
    }

    if standard {
        cmd.arg("--standard");
    }

    if let Some(ref tag) = filter {
        cmd.args(["--filter", tag]);
    }

    if json {
        cmd.arg("--json");
    }

    if interactive {
        cmd.arg("--interactive");
    }

    // Default to standard if nothing specified
    if prompts.is_none() && !standard && !interactive {
        cmd.arg("--standard");
    }

    let status = cmd.status().context("Failed to run lexicon_harness")?;
    if !status.success() {
        // Non-zero exit is expected when tests fail - propagate exit code
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

fn run_load_fund_programme(
    config_path: &std::path::Path,
    input_path: &std::path::Path,
    output_path: Option<&std::path::Path>,
    limit: Option<usize>,
    dry_run: bool,
    _execute: bool,
) -> Result<()> {
    println!("Loading fund programme...");
    println!("  Config: {}", config_path.display());
    println!("  Input: {}", input_path.display());

    // Load config
    let config = fund_programme::FundProgrammeConfig::from_file(config_path)?;
    println!("  Programme: {}", config.programme_name);

    // Load CSV
    let records = fund_programme::load_csv_with_config(input_path, &config, limit)?;
    println!("  Loaded {} records", records.len());

    // Generate DSL
    let dsl = fund_programme::generate_dsl_file(&records, &config);

    if dry_run {
        println!("\n--- Generated DSL (dry run) ---\n");
        println!("{}", dsl);
        return Ok(());
    }

    // Write output
    match output_path {
        Some(path) => {
            std::fs::write(path, &dsl)?;
            println!("  Written to: {}", path.display());
        }
        None => {
            println!("\n{}", dsl);
        }
    }

    // TODO: Execute DSL if --execute flag is set
    // This would require connecting to the database and running the DSL executor

    Ok(())
}

async fn run_allianz_harness(
    mode: &str,
    limit: Option<usize>,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    use sqlx::PgPool;

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    let pool = PgPool::connect(&database_url).await?;

    let harness_mode = match mode.to_lowercase().as_str() {
        "discover" => allianz_harness::HarnessMode::Discover,
        "import" => allianz_harness::HarnessMode::Import,
        "clean" => allianz_harness::HarnessMode::Clean,
        "full" => allianz_harness::HarnessMode::Full,
        _ => anyhow::bail!("Unknown mode: {}. Use: discover, import, clean, full", mode),
    };

    let mut harness = allianz_harness::AllianzTestHarness::new(pool)
        .with_dry_run(dry_run)
        .with_verbose(verbose)
        .with_limit(limit);

    harness.run(harness_mode).await?;

    Ok(())
}

fn project_root() -> Result<std::path::PathBuf> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").context("CARGO_MANIFEST_DIR not set")?;
    let path = std::path::PathBuf::from(manifest_dir);
    // xtask is in rust/xtask, so go up two levels to get project root
    Ok(path.parent().unwrap().parent().unwrap().to_path_buf())
}

fn check(sh: &Shell, db: bool) -> Result<()> {
    println!("Running checks...");

    // Compile check
    println!("  Checking compilation...");
    cmd!(sh, "cargo check --features database").run()?;

    // Clippy
    println!("  Running clippy...");
    cmd!(sh, "cargo clippy --features database -- -D warnings").run()?;

    // Tests
    println!("  Running tests...");
    if db {
        cmd!(sh, "cargo test --features database").run()?;

        // Governance drift check (only with DB access)
        println!("  Running governance check...");
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(governed_check::run_check(true))?;
    } else {
        cmd!(sh, "cargo test --lib --features database").run()?;
    }

    println!("All checks passed!");
    Ok(())
}

fn clippy(sh: &Shell, fix: bool) -> Result<()> {
    let features = ["database", "server", "mcp", "cli,database"];

    for feature in features {
        println!("Clippy with --features {feature}...");
        if fix {
            cmd!(sh, "cargo clippy --features {feature} --fix --allow-dirty").run()?;
        } else {
            cmd!(sh, "cargo clippy --features {feature} -- -D warnings").run()?;
        }
    }

    println!("Clippy clean!");
    Ok(())
}

fn test(sh: &Shell, lib: bool, db: bool, filter: Option<String>) -> Result<()> {
    let mut args = vec!["test"];

    if lib {
        args.push("--lib");
    }

    args.push("--features");
    args.push("database");

    let filter_str;
    if let Some(ref f) = filter {
        filter_str = f.clone();
        args.push("--");
        args.push(&filter_str);
    }

    let args_str = args.join(" ");
    cmd!(sh, "cargo {args_str}").run()?;

    if db {
        println!("Running database integration tests...");
        cmd!(sh, "cargo test --features database --test db_integration").run()?;
    }

    Ok(())
}

fn fmt(sh: &Shell, check: bool) -> Result<()> {
    if check {
        cmd!(sh, "cargo fmt --check").run()?;
    } else {
        cmd!(sh, "cargo fmt").run()?;
    }
    Ok(())
}

fn build(sh: &Shell, release: bool) -> Result<()> {
    // Build CLI tools from main crate
    let cli_binaries = [("dsl_cli", "cli,database"), ("dsl_mcp", "mcp")];

    for (bin, features) in cli_binaries {
        println!("Building {bin}...");
        if release {
            cmd!(
                sh,
                "cargo build --release --bin {bin} --features {features}"
            )
            .run()?;
        } else {
            cmd!(sh, "cargo build --bin {bin} --features {features}").run()?;
        }
    }

    // Build web server (separate package)
    println!("Building ob-poc-web...");
    if release {
        cmd!(sh, "cargo build --release -p ob-poc-web").run()?;
    } else {
        cmd!(sh, "cargo build -p ob-poc-web").run()?;
    }

    Ok(())
}

fn schema_export(sh: &Shell) -> Result<()> {
    // Go to project root for schema export
    let root = project_root()?;
    sh.change_dir(&root);

    println!("Exporting database schema...");
    cmd!(
        sh,
        "pg_dump -d data_designer --schema-only --no-owner --no-privileges -f schema_export.sql"
    )
    .run()?;
    println!("Schema exported to schema_export.sql");
    Ok(())
}

fn ts_bindings(sh: &Shell) -> Result<()> {
    println!("Generating TypeScript bindings...");

    // Run the export test
    cmd!(sh, "cargo test --package ob-poc-types export_bindings").run()?;

    // Copy to web static
    let root = project_root()?;
    let src = root.join("rust/crates/ob-poc-types/bindings");
    let dst = root.join("rust/crates/ob-poc-web/static/ts/generated");

    if src.exists() {
        println!("Copying bindings to {}", dst.display());
        for entry in std::fs::read_dir(&src)? {
            let entry = entry?;
            if entry.path().extension().map(|e| e == "ts").unwrap_or(false) {
                let dest_file = dst.join(entry.file_name());
                std::fs::copy(entry.path(), &dest_file)?;
                println!("  Copied {}", entry.file_name().to_string_lossy());
            }
        }
    }

    Ok(())
}

fn serve(sh: &Shell, port: u16) -> Result<()> {
    let port_str = port.to_string();

    // Kill any existing server on this port
    println!("Stopping any existing server on port {port}...");
    let _ = cmd!(sh, "pkill -f ob-poc-web").run();
    let _ = cmd!(sh, "lsof -ti:{port_str}").read().map(|pids| {
        for pid in pids.lines() {
            let _ = cmd!(sh, "kill -9 {pid}").run();
        }
    });
    std::thread::sleep(std::time::Duration::from_millis(500));

    println!("Starting ob-poc-web server on port {port}...");
    let db_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    sh.set_var("DATABASE_URL", &db_url);
    sh.set_var("SERVER_PORT", &port_str);
    cmd!(sh, "cargo run -p ob-poc-web").run()?;
    Ok(())
}

fn ci(sh: &Shell) -> Result<()> {
    println!("Running full CI pipeline...");

    println!("\n=== Format Check ===");
    fmt(sh, true)?;

    println!("\n=== Clippy (all features) ===");
    clippy(sh, false)?;

    println!("\n=== Tests ===");
    test(sh, false, false, None)?;

    println!("\n=== Build ===");
    build(sh, false)?;

    // Governance drift check (requires DATABASE_URL)
    if std::env::var("DATABASE_URL").is_ok() {
        println!("\n=== Governance Check ===");
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(governed_check::run_check(true))?;
    }

    println!("\nCI pipeline passed!");
    Ok(())
}

fn pre_commit(sh: &Shell) -> Result<()> {
    println!("Pre-commit checks...");

    // Fast checks only
    println!("\n=== Format Check ===");
    fmt(sh, true)?;

    println!("\n=== Clippy ===");
    cmd!(sh, "cargo clippy --features database -- -D warnings").run()?;

    println!("\n=== Unit Tests ===");
    cmd!(sh, "cargo test --lib --features database").run()?;

    println!("\n=== Verb Atlas Lint ===");
    verbs::verbs_atlas(None, true, false)?;

    println!("\nPre-commit checks passed!");
    Ok(())
}

fn deploy(sh: &Shell, release: bool, port: u16, no_run: bool, skip_frontend: bool) -> Result<()> {
    println!("===========================================");
    println!("  OB-POC Deploy Pipeline");
    println!("===========================================\n");

    let root = project_root()?;
    let mut step = 1;

    // Step 1: Kill existing server (by name and by port)
    println!("Step {}: Stopping existing server...", step);
    step += 1;
    let _ = cmd!(sh, "pkill -f ob-poc-web").run(); // Ignore error if not running
    let port_str = port.to_string();
    // Also kill anything on the target port
    let _ = cmd!(sh, "lsof -ti:{port_str}").read().map(|pids| {
        for pid in pids.lines() {
            let _ = cmd!(sh, "kill -9 {pid}").run();
        }
    });
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Step 2: Build React frontend (unless skipped)
    if !skip_frontend {
        println!("\nStep {}: Building React frontend...", step);
        step += 1;
        let react_dir = root.join("ob-poc-ui-react");
        if react_dir.exists() {
            sh.change_dir(&react_dir);

            // Check if node_modules exists, if not run npm install
            if !react_dir.join("node_modules").exists() {
                println!("  Installing npm dependencies...");
                cmd!(sh, "npm install")
                    .run()
                    .context("Failed to install npm dependencies")?;
            }

            // Build React app
            cmd!(sh, "npm run build")
                .run()
                .context("Failed to build React frontend")?;
            println!("  React frontend built successfully");
        } else {
            println!(
                "  Warning: React frontend directory not found at {:?}",
                react_dir
            );
            println!("  Skipping frontend build...");
        }
    } else {
        println!("\nStep {}: Skipping React frontend (--skip-frontend)", step);
        step += 1;
    }

    // Step 3: Build web server
    println!("\nStep {}: Building web server...", step);
    step += 1;
    let _ = step; // suppress unused warning
    sh.change_dir(root.join("rust"));
    if release {
        cmd!(sh, "cargo build -p ob-poc-web --release")
            .run()
            .context("Failed to build ob-poc-web")?;
    } else {
        cmd!(sh, "cargo build -p ob-poc-web")
            .run()
            .context("Failed to build ob-poc-web")?;
    }
    println!("  ob-poc-web built successfully");

    if no_run {
        println!("\n===========================================");
        println!("  Build complete (--no-run specified)");
        println!("===========================================");
        return Ok(());
    }

    // Step 4: Start server
    println!("\nStep 4: Starting server on port {}...", port);
    println!("\n===========================================");
    println!("  Server starting at http://localhost:{}", port);
    println!("  Press Ctrl+C to stop");
    println!("===========================================\n");

    let port_str = port.to_string();
    let bin_path = if release {
        root.join("rust/target/release/ob-poc-web")
    } else {
        root.join("rust/target/debug/ob-poc-web")
    };
    let bin_path_str = bin_path.to_str().context("Invalid binary path")?;

    // Set environment and run
    let db_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    sh.set_var("DATABASE_URL", &db_url);
    sh.set_var("SERVER_PORT", &port_str);
    cmd!(sh, "{bin_path_str}").run()?;

    Ok(())
}

fn batch_import(
    sh: &Shell,
    limit: Option<usize>,
    dry_run: bool,
    verbose: bool,
    add_products: Option<String>,
    agent_url: String,
) -> Result<()> {
    println!("===========================================");
    println!("  Batch Import: Allianz Funds → CBUs");
    println!("===========================================\n");

    // Build the batch_test_harness binary
    println!("Building batch_test_harness...");
    cmd!(
        sh,
        "cargo build --features database,cli --bin batch_test_harness"
    )
    .run()
    .context("Failed to build batch_test_harness")?;

    // Construct arguments - use std::process::Command to properly handle args with spaces
    let mut cmd = std::process::Command::new("./target/debug/batch_test_harness");
    cmd.args([
        "--template",
        "onboard-fund-cbu",
        "--fund-query",
        "--shared",
        "manco_entity=Allianz Global Investors GmbH",
        "--shared",
        "im_entity=Allianz Global Investors GmbH",
        "--shared",
        "jurisdiction=LU",
        "--continue-on-error",
    ]);

    if let Some(n) = limit {
        cmd.args(["--limit", &n.to_string()]);
    }

    if dry_run {
        cmd.arg("--dry-run");
    }

    if verbose {
        cmd.arg("--verbose");
    }

    // Phase 2: Agent-generated product addition
    if let Some(ref products) = add_products {
        cmd.args(["--add-products", products]);
        cmd.args(["--agent-url", &agent_url]);
    }

    // Run the harness
    let status = cmd.status().context("Failed to run batch_test_harness")?;

    if !status.success() {
        anyhow::bail!(
            "batch_test_harness failed with exit code: {:?}",
            status.code()
        );
    }

    Ok(())
}

async fn batch_clean(limit: Option<usize>, dry_run: bool) -> Result<()> {
    use sqlx::PgPool;

    println!("===========================================");
    println!("  Batch Clean: Delete Allianz CBUs via DSL");
    println!("===========================================\n");

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    let pool = PgPool::connect(&database_url).await?;

    // Get Allianz CBUs
    let query = r#"
        SELECT cbu_id, name
        FROM "ob-poc".cbus
        WHERE name ILIKE 'Allianz%'
        ORDER BY name
    "#;

    let cbus: Vec<(uuid::Uuid, String)> = sqlx::query_as(query).fetch_all(&pool).await?;

    let cbus: Vec<_> = if let Some(n) = limit {
        cbus.into_iter().take(n).collect()
    } else {
        cbus
    };

    println!("Found {} Allianz CBUs to delete\n", cbus.len());

    if cbus.is_empty() {
        println!("Nothing to clean.");
        return Ok(());
    }

    if dry_run {
        println!("DRY RUN - would delete:");
        for (id, name) in &cbus {
            println!("  {} - {}", id, name);
        }
        println!("\nRun without --dry-run to actually delete.");
        return Ok(());
    }

    // Delete each CBU using the cbu.delete-cascade DSL verb
    let mut success_count = 0;
    let mut fail_count = 0;

    for (i, (cbu_id, name)) in cbus.iter().enumerate() {
        print!("[{}/{}] Deleting {}... ", i + 1, cbus.len(), name);

        let dsl = format!(r#"(cbu.delete-cascade :cbu-id "{}" :force true)"#, cbu_id);

        match execute_dsl_simple(&dsl, &pool).await {
            Ok(_) => {
                println!("OK");
                success_count += 1;
            }
            Err(e) => {
                println!("FAILED: {}", e);
                fail_count += 1;
            }
        }
    }

    println!("\n===========================================");
    println!("  Batch Clean Summary");
    println!("===========================================");
    println!("Success: {}", success_count);
    println!("Failed:  {}", fail_count);

    Ok(())
}

async fn execute_dsl_simple(dsl: &str, pool: &sqlx::PgPool) -> Result<()> {
    // Note: We can't use DslExecutor here because xtask can't depend on ob_poc
    // (would create circular dependency). So we implement the cascade delete directly.
    // This mirrors the logic in CbuDeleteCascadeOp.

    // Extract the UUID from the DSL
    let uuid_start = dsl.find('"').unwrap() + 1;
    let uuid_end = dsl[uuid_start..].find('"').unwrap() + uuid_start;
    let cbu_id: uuid::Uuid = dsl[uuid_start..uuid_end].parse()?;

    // Execute cascade delete directly (same logic as CbuDeleteCascadeOp)
    let mut tx = pool.begin().await?;

    // Phase 1: KYC schema
    sqlx::query(r#"DELETE FROM kyc.screenings WHERE workstream_id IN (SELECT workstream_id FROM kyc.entity_workstreams WHERE case_id IN (SELECT case_id FROM kyc.cases WHERE cbu_id = $1))"#)
        .bind(cbu_id).execute(&mut *tx).await?;
    sqlx::query(r#"DELETE FROM kyc.doc_requests WHERE workstream_id IN (SELECT workstream_id FROM kyc.entity_workstreams WHERE case_id IN (SELECT case_id FROM kyc.cases WHERE cbu_id = $1))"#)
        .bind(cbu_id).execute(&mut *tx).await?;
    sqlx::query(r#"DELETE FROM kyc.case_events WHERE case_id IN (SELECT case_id FROM kyc.cases WHERE cbu_id = $1)"#)
        .bind(cbu_id).execute(&mut *tx).await?;
    sqlx::query(r#"DELETE FROM kyc.red_flags WHERE case_id IN (SELECT case_id FROM kyc.cases WHERE cbu_id = $1)"#)
        .bind(cbu_id).execute(&mut *tx).await?;
    sqlx::query(r#"DELETE FROM kyc.entity_workstreams WHERE case_id IN (SELECT case_id FROM kyc.cases WHERE cbu_id = $1)"#)
        .bind(cbu_id).execute(&mut *tx).await?;
    sqlx::query(r#"DELETE FROM kyc.cases WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;

    // Phase 2: Custody schema
    sqlx::query(r#"DELETE FROM custody.ssi_booking_rules WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(r#"DELETE FROM custody.cbu_ssi_agent_override WHERE ssi_id IN (SELECT ssi_id FROM custody.cbu_ssi WHERE cbu_id = $1)"#)
        .bind(cbu_id).execute(&mut *tx).await?;
    sqlx::query(r#"DELETE FROM custody.cbu_ssi WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(r#"DELETE FROM custody.cbu_instrument_universe WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;

    // Phase 3: ob-poc schema
    sqlx::query(r#"DELETE FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(r#"DELETE FROM "ob-poc".document_catalog WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(r#"DELETE FROM "ob-poc".cbu_resource_instances WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(r#"DELETE FROM "ob-poc".service_delivery_map WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;

    // Phase 4: Delete CBU itself
    sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

/// Load Allianz structure via DSL (clean existing data + execute DSL file)
fn etl_allianz(
    sh: &Shell,
    file: Option<std::path::PathBuf>,
    no_clean: bool,
    dry_run: bool,
    no_regen: bool,
) -> Result<()> {
    println!("===========================================");
    println!("  Allianz ETL via DSL");
    println!("===========================================\n");

    let root = project_root()?;

    // Default DSL file
    let dsl_file = file.unwrap_or_else(|| root.join("data/derived/dsl/allianz_full_etl.dsl"));

    // Step 1: Regenerate DSL from sources (unless --no-regen)
    if !no_regen {
        println!("Step 1: Regenerating DSL from GLEIF + scraped fund data...");
        sh.change_dir(&root);
        cmd!(sh, "python3 scripts/generate_allianz_full_dsl.py")
            .run()
            .context("Failed to regenerate DSL")?;
        println!();
    } else {
        println!("Step 1: Skipped regeneration (--no-regen)\n");
    }

    if !dsl_file.exists() {
        anyhow::bail!("DSL file not found: {}", dsl_file.display());
    }

    println!("DSL file: {}\n", dsl_file.display());

    // Step 2: Clean existing data (unless --no-clean)
    if !no_clean {
        println!("Step 2: Cleaning existing Allianz data...");
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(seed_allianz::clean_allianz(false))?;
        println!();
    } else {
        println!("Step 2: Skipped (--no-clean)\n");
    }

    // Step 3: Run DSL
    let action = if dry_run { "Validating" } else { "Executing" };
    println!("Step 3: {} DSL...\n", action);

    sh.change_dir(root.join("rust"));

    let dsl_path = dsl_file.to_str().context("Invalid DSL path")?;
    let db_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());

    if dry_run {
        // Just validate/plan
        cmd!(sh, "cargo run --bin dsl_cli --features database,cli -- plan --db-url {db_url} --file {dsl_path}")
            .run()
            .context("DSL validation failed")?;
    } else {
        // Execute
        cmd!(sh, "cargo run --bin dsl_cli --features database,cli -- execute --db-url {db_url} --file {dsl_path}")
            .run()
            .context("DSL execution failed")?;
    }

    println!("\n===========================================");
    if dry_run {
        println!("  Validation complete (dry run)");
    } else {
        println!("  ETL complete");
    }
    println!("===========================================");

    Ok(())
}

/// Lint macro schema files
fn lint_macros(errors_only: bool, verbose: bool) -> Result<()> {
    println!("===========================================");
    println!("  Macro Schema Lint");
    println!("===========================================\n");

    let root = project_root()?;
    let macros_dir = root.join("rust/config/verb_schemas/macros");

    if !macros_dir.exists() {
        println!("No macros directory found at: {}", macros_dir.display());
        println!("Creating directory...");
        std::fs::create_dir_all(&macros_dir)?;
        println!("  Created: {}", macros_dir.display());
        println!("\nNo macro files to lint yet.");
        return Ok(());
    }

    // Find all YAML files in macros directory
    let yaml_files: Vec<_> = std::fs::read_dir(&macros_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .map(|ext| ext == "yaml" || ext == "yml")
                .unwrap_or(false)
        })
        .filter(|p| {
            // Skip files starting with underscore (like _prereq_keys.yaml)
            !p.file_name()
                .map(|n| n.to_string_lossy().starts_with('_'))
                .unwrap_or(false)
        })
        .collect();

    if yaml_files.is_empty() {
        println!("No macro YAML files found in: {}", macros_dir.display());
        println!("\nTo create macros, add YAML files like:");
        println!("  - config/verb_schemas/macros/structure.yaml");
        println!("  - config/verb_schemas/macros/case.yaml");
        return Ok(());
    }

    println!("Found {} macro file(s) to lint\n", yaml_files.len());

    let mut total_errors = 0;
    let mut total_warnings = 0;
    let mut _files_with_issues = 0;

    for path in &yaml_files {
        let file_name = path.file_name().unwrap().to_string_lossy();

        if verbose {
            println!("Linting: {}", file_name);
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                println!("  ERROR: Failed to read {}: {}", file_name, e);
                total_errors += 1;
                continue;
            }
        };

        // Use the lint module from ob-poc
        // Since xtask can't directly depend on ob-poc (circular), we use a subprocess
        let output = std::process::Command::new("cargo")
            .args([
                "run",
                "-p",
                "ob-poc",
                "--features",
                "database",
                "--bin",
                "macro_lint",
                "--",
                path.to_str().unwrap(),
            ])
            .current_dir(root.join("rust"))
            .output();

        match output {
            Ok(out) => {
                if !out.stdout.is_empty() {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    // Parse the output to count errors/warnings
                    for line in stdout.lines() {
                        if line.contains("[error]") {
                            total_errors += 1;
                            println!("  {}", line);
                        } else if line.contains("[warn]") && !errors_only {
                            total_warnings += 1;
                            println!("  {}", line);
                        } else if verbose {
                            println!("  {}", line);
                        }
                    }
                }
                if !out.stderr.is_empty() && verbose {
                    eprintln!("{}", String::from_utf8_lossy(&out.stderr));
                }
            }
            Err(_) => {
                // Fallback: run inline lint (less accurate but works without binary)
                let diags = inline_lint_yaml(&content);
                for d in &diags {
                    if d.0 == "error" {
                        total_errors += 1;
                        println!("  [{}] {}: {} at {}", d.1, d.0, d.3, d.2);
                    } else if d.0 == "warn" && !errors_only {
                        total_warnings += 1;
                        println!("  [{}] {}: {} at {}", d.1, d.0, d.3, d.2);
                    }
                }
            }
        }

        if total_errors > 0 || total_warnings > 0 {
            _files_with_issues += 1;
        }
    }

    println!("\n===========================================");
    println!("  Lint Summary");
    println!("===========================================");
    println!("Files checked: {}", yaml_files.len());
    println!("Errors:        {}", total_errors);
    if !errors_only {
        println!("Warnings:      {}", total_warnings);
    }

    if total_errors > 0 {
        println!("\nLint failed with {} error(s)", total_errors);
        std::process::exit(1);
    } else if total_warnings > 0 && !errors_only {
        println!("\nLint passed with {} warning(s)", total_warnings);
    } else {
        println!("\nAll macro schemas passed lint!");
    }

    Ok(())
}

/// Inline YAML lint for when the binary isn't available
/// Returns Vec<(severity, code, path, message)>
fn inline_lint_yaml(content: &str) -> Vec<(&'static str, &'static str, String, String)> {
    let mut diags = Vec::new();

    // Parse YAML
    let doc: Result<serde_yaml::Value, _> = serde_yaml::from_str(content);
    let doc = match doc {
        Ok(v) => v,
        Err(e) => {
            diags.push((
                "error",
                "MACRO000",
                "$".to_string(),
                format!("YAML parse error: {}", e),
            ));
            return diags;
        }
    };

    // Check top-level is mapping
    let top = match doc.as_mapping() {
        Some(m) => m,
        None => {
            diags.push((
                "error",
                "MACRO001",
                "$".to_string(),
                "Schema must be a mapping".to_string(),
            ));
            return diags;
        }
    };

    // Check each verb
    for (k, v) in top {
        let verb = k.as_str().unwrap_or("?");

        if let Some(spec) = v.as_mapping() {
            // Check kind
            let kind = spec.get("kind").and_then(|v| v.as_str());
            if kind != Some("macro") && kind != Some("primitive") {
                diags.push((
                    "error",
                    "MACRO010",
                    verb.to_string(),
                    "kind must be 'macro' or 'primitive'".to_string(),
                ));
            }

            if kind == Some("macro") {
                // Check UI fields
                if spec.get("ui").is_none() {
                    diags.push((
                        "error",
                        "MACRO011",
                        verb.to_string(),
                        "ui section is required".to_string(),
                    ));
                }

                // Check routing
                if spec.get("routing").is_none() {
                    diags.push((
                        "error",
                        "MACRO020",
                        verb.to_string(),
                        "routing section is required".to_string(),
                    ));
                }

                // Check target
                if spec.get("target").is_none() {
                    diags.push((
                        "error",
                        "MACRO030",
                        verb.to_string(),
                        "target section is required".to_string(),
                    ));
                }

                // Check args
                if spec.get("args").is_none() {
                    diags.push((
                        "error",
                        "MACRO040",
                        verb.to_string(),
                        "args section is required".to_string(),
                    ));
                }

                // Check prereqs
                if spec.get("prereqs").is_none() {
                    diags.push((
                        "error",
                        "MACRO050",
                        verb.to_string(),
                        "prereqs is required (can be empty [])".to_string(),
                    ));
                }

                // Check expands_to
                if spec.get("expands_to").is_none() {
                    diags.push((
                        "error",
                        "MACRO060",
                        verb.to_string(),
                        "expands_to is required for macros".to_string(),
                    ));
                }
            }
        }
    }

    diags
}

// ============================================================================
// Deal Harness
// ============================================================================

async fn run_deal_harness(verbose: bool, dry_run: bool, cleanup: bool) -> Result<()> {
    use sqlx::PgPool;

    println!("===========================================");
    println!("  Deal Hierarchy Test Harness");
    println!("  (Deal → Products → Rate Cards DAG)");
    println!("===========================================\n");

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    let pool = PgPool::connect(&database_url).await?;

    let results = deal_harness::run_deal_harness(pool, verbose, dry_run, cleanup).await?;

    println!("\n===========================================");
    println!("  Deal Harness Summary");
    println!("===========================================");
    println!("Passed: {}", results.passed);
    println!("Failed: {}", results.failed);

    if results.failed > 0 {
        println!("\nFailed steps:");
        for step in &results.steps {
            if !step.success {
                println!(
                    "  - {}: {}",
                    step.step,
                    step.error.as_deref().unwrap_or("unknown")
                );
            }
        }
        anyhow::bail!("Deal harness failed with {} errors", results.failed);
    }

    println!("\nAll deal hierarchy tests passed!");
    Ok(())
}

// ============================================================================
// Aviva Deal Harness
// ============================================================================

async fn run_aviva_deal_harness(verbose: bool, dry_run: bool) -> Result<()> {
    use sqlx::PgPool;

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    let pool = PgPool::connect(&database_url).await?;

    let results = aviva_deal_harness::run_aviva_deal_harness(pool, verbose, dry_run).await?;

    if results.steps_failed > 0 {
        anyhow::bail!(
            "Aviva deal harness completed with {} errors",
            results.steps_failed
        );
    }

    Ok(())
}

// ============================================================================
// Playbook Check
// ============================================================================

fn playbook_check(files: Vec<std::path::PathBuf>, format: &str, verbose: bool) -> Result<()> {
    use playbook_core::parse_playbook;
    use playbook_lower::{lower_playbook, SlotState};

    let _ = format; // unused for now

    println!("==========================================");
    println!("  Playbook Check");
    println!("==========================================");
    println!();

    let mut total_errors = 0;
    let mut total_warnings = 0;
    let mut checked_files = 0;

    for path in &files {
        let paths: Vec<std::path::PathBuf> = if path.to_string_lossy().contains('*') {
            glob::glob(&path.to_string_lossy())
                .map(|paths| paths.filter_map(|p| p.ok()).collect())
                .unwrap_or_else(|_| vec![path.clone()])
        } else {
            vec![path.clone()]
        };

        for file_path in paths {
            if !file_path.exists() {
                eprintln!("Error: File not found: {}", file_path.display());
                total_errors += 1;
                continue;
            }

            let source = match std::fs::read_to_string(&file_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error reading {}: {}", file_path.display(), e);
                    total_errors += 1;
                    continue;
                }
            };

            checked_files += 1;
            println!("Checking: {}", file_path.display());

            let output = match parse_playbook(&source) {
                Ok(result) => result,
                Err(e) => {
                    println!("  ERROR: Parse failed: {}", e);
                    total_errors += 1;
                    continue;
                }
            };

            let spec = output.spec;

            if verbose {
                println!("  Playbook: {}", spec.name);
                println!("  Slots: {}", spec.slots.len());
                println!("  Steps: {}", spec.steps.len());

                if !spec.slots.is_empty() {
                    println!("  Slot details:");
                    for (name, slot) in &spec.slots {
                        let req = if slot.required {
                            "required"
                        } else {
                            "optional"
                        };
                        let default = slot
                            .default
                            .as_ref()
                            .map(|d| format!(" = {:?}", d))
                            .unwrap_or_default();
                        println!("    - {} ({}){}", name, req, default);
                    }
                }
            }

            // Lower with empty slot state to find missing slots
            let slots = SlotState::new();
            let result = lower_playbook(&spec, &slots);

            if !result.missing_slots.is_empty() {
                println!("  WARNING: Missing required slots:");
                for missing in &result.missing_slots {
                    println!(
                        "    - slot in step {} ({})",
                        missing.step_index, missing.step_id
                    );
                }
                total_warnings += result.missing_slots.len();
            }

            if verbose && !result.dsl_statements.is_empty() {
                println!("  Generated {} DSL statements", result.dsl_statements.len());
            }

            if result.missing_slots.is_empty() {
                println!("  OK");
            }
            println!();
        }
    }

    println!("==========================================");
    println!(
        "Summary: {} files checked, {} errors, {} warnings",
        checked_files, total_errors, total_warnings
    );
    println!("==========================================");

    if total_errors > 0 {
        anyhow::bail!("Playbook check failed with {} errors", total_errors);
    }

    Ok(())
}

// ============================================================================
