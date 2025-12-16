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
