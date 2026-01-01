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
mod gleif_crawl_dsl;
mod gleif_import;
mod gleif_load;
mod gleif_test;
mod seed_allianz;
mod ubo_test;

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

    /// Run DSL test scenarios
    DslTests,

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
        Command::DslTests => dsl_tests(&sh),
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
    }
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

fn dsl_tests(sh: &Shell) -> Result<()> {
    println!("Running DSL test scenarios...");
    cmd!(sh, "bash tests/scenarios/run_tests.sh").run()?;
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
