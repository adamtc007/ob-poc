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
mod fund_programme;
mod gleif_crawl_dsl;
mod gleif_import;
mod gleif_load;
mod gleif_test;
mod lexicon;
mod seed_allianz;
mod ubo_test;
mod verb_migrate;
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

    /// Build and deploy: WASM components + web server, then start
    Deploy {
        /// Build in release mode
        #[arg(long)]
        release: bool,
        /// Port to listen on
        #[arg(long, default_value = "3000")]
        port: u16,
        /// Skip WASM rebuild (faster if only Rust server changed)
        #[arg(long)]
        skip_wasm: bool,
        /// Don't start the server after building
        #[arg(long)]
        no_run: bool,
    },

    /// Build WASM component (ob-poc-ui only, includes ob-poc-graph as dependency)
    Wasm {
        /// Build in release mode
        #[arg(long)]
        release: bool,
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

    /// Check and validate graph configuration
    ///
    /// Loads graph_settings.yaml and validates all configuration values.
    /// Reports errors for invalid thresholds, missing values, or constraint violations.
    GraphConfig {
        /// Path to config file (default: config/graph_settings.yaml)
        #[arg(long, short = 'c')]
        config: Option<std::path::PathBuf>,

        /// Show all configuration values
        #[arg(long, short = 'v')]
        verbose: bool,

        /// Output as JSON (for programmatic use)
        #[arg(long)]
        json: bool,

        /// Validate only, don't print config
        #[arg(long)]
        validate_only: bool,
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

    /// Migrate V1 verb YAML to V2 schema format
    ///
    /// Converts existing verb definitions to the new V2 format with:
    /// - Inline args (HashMap style)
    /// - Generated invocation phrases
    /// - Positional sugar (max 2)
    /// - Alias deduplication
    MigrateV2 {
        /// Dry run - show what would be done without writing files
        #[arg(long)]
        dry_run: bool,

        /// Show additional details
        #[arg(long, short = 'v')]
        verbose: bool,
    },

    /// Lint V2 schema files (CI gate)
    ///
    /// Validates V2 schemas against lint rules:
    /// - Minimum 3 invocation phrases
    /// - Max 2 positional args
    /// - No alias collisions
    LintV2 {
        /// Show only errors, not warnings
        #[arg(long)]
        errors_only: bool,
    },

    /// Build compiled VerbRegistry artifact
    ///
    /// Compiles all V2 schemas into a single registry.json file
    /// for fast runtime loading.
    BuildRegistry,

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
            skip_wasm,
            no_run,
        } => deploy(&sh, release, port, skip_wasm, no_run),
        Command::Wasm { release } => build_wasm(&sh, release),
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
                VerbsAction::MigrateV2 { dry_run, verbose } => {
                    verb_migrate::run_migrate_v2(dry_run, verbose)
                }
                VerbsAction::LintV2 { errors_only } => verb_migrate::run_lint(errors_only),
                VerbsAction::BuildRegistry => verb_migrate::run_build_registry(),
                VerbsAction::LintMacros {
                    errors_only,
                    verbose,
                } => lint_macros(errors_only, verbose),
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
        Command::GraphConfig {
            config,
            verbose,
            json,
            validate_only,
        } => graph_config_check(config, verbose, json, validate_only),
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

    println!("\nPre-commit checks passed!");
    Ok(())
}

fn build_wasm(sh: &Shell, release: bool) -> Result<()> {
    let root = project_root()?;
    let wasm_out = root.join("rust/crates/ob-poc-web/static/wasm");

    // Ensure wasm-pack is installed
    if cmd!(sh, "which wasm-pack").run().is_err() {
        println!("Installing wasm-pack...");
        cmd!(sh, "cargo install wasm-pack").run()?;
    }

    // Only build ob-poc-ui - it includes ob-poc-graph as a dependency
    let wasm_crates = ["ob-poc-ui"];

    for crate_name in wasm_crates {
        println!("\n=== Building {} WASM ===", crate_name);
        let crate_dir = root.join(format!("rust/crates/{}", crate_name));
        sh.change_dir(&crate_dir);

        let out_dir = wasm_out.to_str().context("Invalid wasm output path")?;
        if release {
            cmd!(
                sh,
                "wasm-pack build --release --target web --out-dir {out_dir}"
            )
            .run()
            .with_context(|| format!("Failed to build {} WASM", crate_name))?;
        } else {
            cmd!(sh, "wasm-pack build --dev --target web --out-dir {out_dir}")
                .run()
                .with_context(|| format!("Failed to build {} WASM", crate_name))?;
        }
        println!("  {} built successfully", crate_name);
    }

    // Return to rust dir
    sh.change_dir(root.join("rust"));

    println!("\nWASM components built to: {}", wasm_out.display());
    Ok(())
}

fn deploy(sh: &Shell, release: bool, port: u16, skip_wasm: bool, no_run: bool) -> Result<()> {
    println!("===========================================");
    println!("  OB-POC Deploy Pipeline");
    println!("===========================================\n");

    let root = project_root()?;

    // Step 1: Kill existing server (by name and by port)
    println!("Step 1: Stopping existing server...");
    let _ = cmd!(sh, "pkill -f ob-poc-web").run(); // Ignore error if not running
    let port_str = port.to_string();
    // Also kill anything on the target port
    let _ = cmd!(sh, "lsof -ti:{port_str}").read().map(|pids| {
        for pid in pids.lines() {
            let _ = cmd!(sh, "kill -9 {pid}").run();
        }
    });
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Step 2: Build WASM (unless skipped)
    if !skip_wasm {
        println!("\nStep 2: Building WASM components...");
        build_wasm(sh, release)?;
    } else {
        println!("\nStep 2: Skipping WASM build (--skip-wasm)");
    }

    // Step 3: Build web server
    println!("\nStep 3: Building web server...");
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
    let mut files_with_issues = 0;

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
            files_with_issues += 1;
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
// Playbook Check
// ============================================================================

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
// Graph Config Check
// ============================================================================

fn graph_config_check(
    config_path: Option<std::path::PathBuf>,
    verbose: bool,
    json: bool,
    validate_only: bool,
) -> Result<()> {
    use ob_poc_graph::config::GraphConfig;

    println!("==========================================");
    println!("  Graph Configuration Check (Policy)");
    println!("==========================================");
    println!();

    // Load config from specified path or default
    let config = if let Some(ref path) = config_path {
        println!("Loading: {}", path.display());
        GraphConfig::load_from_path(path)
            .with_context(|| format!("Failed to load config from {}", path.display()))?
    } else {
        println!("Loading: config/graph_settings.yaml (default)");
        GraphConfig::load_default().context("Failed to load default config")?
    };

    // Show policy metadata
    println!("Policy:");
    println!("  name:    {}", config.policy.name);
    println!("  variant: {}", config.policy.variant);
    println!("  version: {}", config.policy.version);
    println!("  hash:    {:016x}", config.policy_hash());
    println!();

    // Validate
    println!("Validating configuration...");
    match config.validate() {
        Ok(()) => {
            println!("  All validation checks passed");
        }
        Err(errors) => {
            println!("  ERRORS:");
            for error in &errors {
                println!("    - {}", error);
            }
            println!();
            anyhow::bail!(
                "Configuration validation failed with {} errors",
                errors.len()
            );
        }
    }

    if validate_only {
        println!();
        println!("==========================================");
        println!("  Validation complete (--validate-only)");
        println!("==========================================");
        return Ok(());
    }

    println!();

    // Output config
    if json {
        let json_output =
            serde_json::to_string_pretty(&config).context("Failed to serialize config to JSON")?;
        println!("{}", json_output);
    } else if verbose {
        println!("Configuration values:");
        println!();

        // LOD Tiers (new zoom-based system)
        println!("LOD Tiers (zoom-based):");
        println!(
            "  icon:     zoom_max={:.2}, hysteresis={:.2}",
            config.lod.tiers.icon.zoom_max, config.lod.tiers.icon.hysteresis
        );
        println!(
            "  label:    zoom_max={:.2}, hysteresis={:.2}",
            config.lod.tiers.label.zoom_max, config.lod.tiers.label.hysteresis
        );
        println!(
            "  extended: zoom_max={:.2}, hysteresis={:.2}",
            config.lod.tiers.extended.zoom_max, config.lod.tiers.extended.hysteresis
        );
        println!(
            "  full:     zoom_max={:.2}, hysteresis={:.2}",
            config.lod.tiers.full.zoom_max, config.lod.tiers.full.hysteresis
        );
        println!();

        println!("LOD Thresholds (legacy screen-size):");
        println!("  micro:    < {:.1}px", config.lod.thresholds.micro);
        println!("  icon:     < {:.1}px", config.lod.thresholds.icon);
        println!("  compact:  < {:.1}px", config.lod.thresholds.compact);
        println!("  standard: < {:.1}px", config.lod.thresholds.standard);
        println!();

        println!("Budgets:");
        println!(
            "  icons_unlimited:           {}",
            config.budgets.icons_unlimited
        );
        println!(
            "  label_budget_count:        {}",
            config.budgets.label_budget_count
        );
        println!(
            "  full_budget_count:         {}",
            config.budgets.full_budget_count
        );
        println!(
            "  shape_budget_ms_per_frame: {:.1}ms",
            config.budgets.shape_budget_ms_per_frame
        );
        println!(
            "  visible_query_budget_ms:   {:.1}ms",
            config.budgets.visible_query_budget_ms
        );
        println!();

        println!("Flyover Navigation:");
        println!("  dwell_ticks:       {}", config.flyover.dwell_ticks);
        println!(
            "  settle_duration_s: {:.2}s",
            config.flyover.settle_duration_s
        );
        println!("  easing:            {}", config.flyover.easing);
        println!("  Phases:");
        println!(
            "    moving:   selection={}, siblings={}, shaping={}",
            config.flyover.phases.moving.selection_lod,
            config.flyover.phases.moving.siblings_lod,
            config.flyover.phases.moving.shaping_allowed
        );
        println!(
            "    settling: selection={}, siblings={}, shaping={}",
            config.flyover.phases.settling.selection_lod,
            config.flyover.phases.settling.siblings_lod,
            config.flyover.phases.settling.shaping_allowed
        );
        println!(
            "    focused:  selection={}, siblings={}, shaping={}",
            config.flyover.phases.focused.selection_lod,
            config.flyover.phases.focused.siblings_lod,
            config.flyover.phases.focused.shaping_allowed
        );
        println!();

        println!("Structural Mode:");
        println!(
            "  density_cutover.icon_only: {} nodes",
            config.structural.density_cutover.icon_only
        );
        println!(
            "  density_cutover.labels:    {} nodes",
            config.structural.density_cutover.labels
        );
        println!(
            "  max_labels_per_cluster:    {}",
            config.structural.max_labels_per_cluster
        );
        println!();

        println!("Camera:");
        println!("  pan_speed:     {:.1}", config.camera.pan_speed);
        println!("  zoom_speed:    {:.1}", config.camera.zoom_speed);
        println!("  snap_epsilon:  {:.4}", config.camera.snap_epsilon);
        println!("  focus_padding: {:.1}px", config.camera.focus_padding);
        println!();

        println!("Focus:");
        println!(
            "  selection_priority:    {:?}",
            config.focus.selection_priority
        );
        println!(
            "  neighbor_ring_size:    {}",
            config.focus.neighbor_ring_size
        );
        println!(
            "  prefetch_radius_cells: {}",
            config.focus.prefetch_radius_cells
        );
        println!();

        println!("Label Cache:");
        println!("  max_entries:        {}", config.label_cache.max_entries);
        println!(
            "  width_quantization: {}px",
            config.label_cache.width_quantization
        );
        println!("  eviction:           {}", config.label_cache.eviction);
        println!();

        println!("Spatial Index:");
        println!(
            "  default_cell_size: {:.1}px",
            config.spatial_index.default_cell_size
        );
        if !config.spatial_index.chamber_overrides.is_empty() {
            println!("  chamber_overrides:");
            for (name, cfg) in &config.spatial_index.chamber_overrides {
                println!("    {}: cell_size={:.1}px", name, cfg.cell_size);
            }
        }
        println!();

        println!("Layout Node:");
        println!("  width:     {:.1}px", config.layout.node.width);
        println!("  height:    {:.1}px", config.layout.node.height);
        println!("  min_scale: {:.2}", config.layout.node.min_scale);
        println!("  max_scale: {:.2}", config.layout.node.max_scale);
        println!();

        println!("Layout Spacing:");
        println!("  horizontal: {:.1}px", config.layout.spacing.horizontal);
        println!("  vertical:   {:.1}px", config.layout.spacing.vertical);
        println!();

        println!("Layout Tiers:");
        println!("  cbu:       {:.1}", config.layout.tiers.cbu);
        println!("  structure: {:.1}", config.layout.tiers.structure);
        println!("  officers:  {:.1}", config.layout.tiers.officers);
        println!("  ubo:       {:.1}", config.layout.tiers.ubo);
        println!("  investors: {:.1}", config.layout.tiers.investors);
        println!();

        println!("Viewport:");
        println!("  fit_margin:           {:.2}", config.viewport.fit_margin);
        println!(
            "  min_auto_zoom:        {:.2}",
            config.viewport.min_auto_zoom
        );
        println!(
            "  max_auto_zoom:        {:.2}",
            config.viewport.max_auto_zoom
        );
        println!(
            "  max_visible_nodes:    {}",
            config.viewport.max_visible_nodes
        );
        println!(
            "  max_visible_clusters: {}",
            config.viewport.max_visible_clusters
        );
        println!();

        println!(
            "Animation Springs ({} defined):",
            config.animation.springs.len()
        );
        let mut springs: Vec<_> = config.animation.springs.iter().collect();
        springs.sort_by_key(|(name, _)| *name);
        for (name, spring) in springs {
            println!(
                "  {}: stiffness={:.1}, damping={:.1}",
                name, spring.stiffness, spring.damping
            );
        }
        println!();

        println!("Debug:");
        println!("  overlay.enabled:      {}", config.debug.overlay.enabled);
        println!(
            "  overlay.show_hashes:  {}",
            config.debug.overlay.show_hashes
        );
        println!(
            "  overlay.show_phase:   {}",
            config.debug.overlay.show_phase
        );
        println!(
            "  overlay.show_timings: {}",
            config.debug.overlay.show_timings
        );
        println!();

        println!("Safety Clamps:");
        println!("  max_nodes_visible:   {}", config.clamps.max_nodes_visible);
        println!(
            "  max_chambers_loaded: {}",
            config.clamps.max_chambers_loaded
        );
        println!(
            "  max_snapshot_bytes:  {} bytes ({:.1}MB)",
            config.clamps.max_snapshot_bytes,
            config.clamps.max_snapshot_bytes as f64 / 1_000_000.0
        );
    } else {
        println!("Configuration loaded successfully.");
        println!("Use --verbose to see all values, or --json for machine-readable output.");
    }

    println!();
    println!("==========================================");
    println!("  Graph config check complete");
    println!("==========================================");

    Ok(())
}
