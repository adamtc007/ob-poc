//! xtask - Development automation for ob-poc
//!
//! Usage: cargo xtask <command>
//!
//! This provides type-safe, cross-platform build automation that replaces
//! shell scripts with Rust code.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use xshell::{cmd, Shell};

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

    /// Start the agentic server
    Serve {
        /// Port to listen on
        #[arg(long, default_value = "3000")]
        port: u16,
    },

    /// Full CI pipeline (fmt, clippy, test, build)
    Ci,

    /// Pre-commit hook: check + clippy + test
    PreCommit,
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
    }
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
    let binaries = [
        ("agentic_server", "server"),
        ("dsl_cli", "cli,database"),
        ("dsl_mcp", "mcp"),
    ];

    for (bin, features) in binaries {
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
    println!("Starting agentic server on port {port}...");
    let port_str = port.to_string();
    cmd!(
        sh,
        "cargo run --features server --bin agentic_server -- --port {port_str}"
    )
    .run()?;
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
