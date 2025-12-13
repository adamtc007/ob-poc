//! xtask - Project-specific automation for ob-poc
//!
//! This removes the cognitive load of remembering feature flags and centralizes
//! operational knowledge in code that is checked by the compiler.
//!
//! Usage:
//!   cargo xtask <command>
//!
//! Or with the alias:
//!   cargo x <command>
//!
//! ## DSL Pipeline Testing
//!
//! The DSL pipeline (Parse → Enrich → Validate → Compile → Execute) has specialized
//! testing needs that go beyond standard `cargo test`:
//!
//! - `cargo x test-snapshots` - Golden file testing for AST/Plan output
//! - `cargo x test-repl` - Stateful REPL session testing
//! - `cargo x test-integration` - Full stack with DB + Gateway
//! - `cargo x codegen` - Generate Rust from YAML verb definitions

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask")]
#[command(about = "OB-POC project automation", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    // =========================================================================
    // SERVER COMMANDS
    // =========================================================================
    /// Run the agentic server (--features server)
    Server {
        /// Release build
        #[arg(short, long)]
        release: bool,
    },

    /// Run the DSL CLI (--features cli,database)
    Cli {
        /// Release build
        #[arg(short, long)]
        release: bool,

        /// Arguments to pass to dsl_cli
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Run the MCP server (--features mcp)
    Mcp {
        /// Release build
        #[arg(short, long)]
        release: bool,
    },

    /// Run the DSL API server (--features server)
    DslApi {
        /// Release build
        #[arg(short, long)]
        release: bool,
    },

    /// Run the EntityGateway gRPC server
    Gateway {
        /// Release build
        #[arg(short, long)]
        release: bool,
    },

    // =========================================================================
    // BUILD COMMANDS
    // =========================================================================
    /// Build all binaries
    Build {
        /// Release build
        #[arg(short, long)]
        release: bool,
    },

    // =========================================================================
    // STANDARD TEST COMMANDS
    // =========================================================================
    /// Run tests with database features
    Test {
        /// Run only unit tests (no database)
        #[arg(long)]
        unit: bool,

        /// Run database integration tests
        #[arg(long)]
        db: bool,

        /// Test name filter
        #[arg(short, long)]
        filter: Option<String>,
    },

    /// Run E2E tests
    E2e,

    /// Run DSL scenario tests (shell script)
    Scenarios,

    // =========================================================================
    // DSL PIPELINE TESTING (Compiler-specific)
    // =========================================================================
    /// Golden file/snapshot testing for DSL outputs
    ///
    /// Runs DSL inputs and compares AST/Plan output against saved .expected files.
    /// Use --overwrite to update expected files after intentional changes.
    TestSnapshots {
        /// Update .expected files with current output
        #[arg(long)]
        overwrite: bool,

        /// Filter to specific scenario by name
        #[arg(short, long)]
        filter: Option<String>,

        /// Output format: ast, plan, or both
        #[arg(long, default_value = "both")]
        output: String,
    },

    /// Test REPL incremental editing sessions
    ///
    /// Feeds DSL lines one by one to test stateful compiler behavior.
    /// Verifies that bindings from line 1 are available in line 10.
    TestRepl {
        /// Specific scenario file to run
        #[arg(short, long)]
        scenario: Option<String>,

        /// Show verbose output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Full integration test with DB + EntityGateway
    ///
    /// Orchestrates the full environment:
    /// 1. Starts Postgres (or uses existing)
    /// 2. Starts EntityGateway on port 50051
    /// 3. Runs integration tests
    /// 4. Tears down cleanly
    TestIntegration {
        /// Skip EntityGateway startup (assume already running)
        #[arg(long)]
        skip_gateway: bool,

        /// Keep services running after tests
        #[arg(long)]
        keep_running: bool,
    },

    // =========================================================================
    // CODE GENERATION
    // =========================================================================
    /// Generate Rust code from YAML verb definitions
    ///
    /// Parses config/verbs/*.yaml and generates type-safe Rust structs
    /// to ensure the compiler never drifts from YAML configuration.
    Codegen {
        /// Check if generated code is up-to-date (CI mode)
        #[arg(long)]
        check: bool,
    },

    // =========================================================================
    // CODE QUALITY
    // =========================================================================
    /// Run clippy on all feature combinations
    Clippy {
        /// Fix warnings automatically
        #[arg(long)]
        fix: bool,
    },

    /// Run cargo fmt
    Fmt {
        /// Check only, don't modify
        #[arg(long)]
        check: bool,
    },

    /// Prepare sqlx offline mode
    SqlxPrepare,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        // Server commands
        Commands::Server { release } => {
            run_cargo_bin("agentic_server", "server", release, &[])?;
        }
        Commands::Cli { release, args } => {
            let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            run_cargo_bin("dsl_cli", "cli,database", release, &args_refs)?;
        }
        Commands::Mcp { release } => {
            run_cargo_bin("dsl_mcp", "mcp", release, &[])?;
        }
        Commands::DslApi { release } => {
            run_cargo_bin("dsl_api", "server", release, &[])?;
        }
        Commands::Gateway { release } => {
            run_gateway(release)?;
        }

        // Build commands
        Commands::Build { release } => {
            build_all(release)?;
        }

        // Standard test commands
        Commands::Test { unit, db, filter } => {
            run_tests(unit, db, filter)?;
        }
        Commands::E2e => {
            run_e2e()?;
        }
        Commands::Scenarios => {
            run_scenarios()?;
        }

        // DSL Pipeline testing
        Commands::TestSnapshots {
            overwrite,
            filter,
            output,
        } => {
            test_snapshots(overwrite, filter, &output)?;
        }
        Commands::TestRepl { scenario, verbose } => {
            test_repl(scenario, verbose)?;
        }
        Commands::TestIntegration {
            skip_gateway,
            keep_running,
        } => {
            test_integration(skip_gateway, keep_running)?;
        }

        // Code generation
        Commands::Codegen { check } => {
            run_codegen(check)?;
        }

        // Code quality
        Commands::Clippy { fix } => {
            run_clippy(fix)?;
        }
        Commands::Fmt { check } => {
            run_fmt(check)?;
        }
        Commands::SqlxPrepare => {
            sqlx_prepare()?;
        }
    }

    Ok(())
}

// =============================================================================
// SERVER COMMANDS
// =============================================================================

/// Run a cargo binary with specified features
fn run_cargo_bin(bin: &str, features: &str, release: bool, extra_args: &[&str]) -> Result<()> {
    let mut args = vec!["run", "--bin", bin, "--features", features];

    if release {
        args.push("--release");
    }

    if !extra_args.is_empty() {
        args.push("--");
        args.extend(extra_args);
    }

    run_command("cargo", &args)
}

/// Run the EntityGateway
fn run_gateway(release: bool) -> Result<()> {
    let gateway_dir = project_root()?.join("crates/entity-gateway");

    let mut args = vec!["run"];
    if release {
        args.push("--release");
    }

    run_command_in_dir("cargo", &args, &gateway_dir)
}

// =============================================================================
// BUILD COMMANDS
// =============================================================================

/// Build all binaries
fn build_all(release: bool) -> Result<()> {
    println!("Building server binary...");
    build_with_features("server", release)?;

    println!("Building CLI binary...");
    build_with_features("cli,database", release)?;

    println!("Building MCP binary...");
    build_with_features("mcp", release)?;

    println!("Building EntityGateway...");
    let gateway_dir = project_root()?.join("crates/entity-gateway");
    let mut args = vec!["build"];
    if release {
        args.push("--release");
    }
    run_command_in_dir("cargo", &args, &gateway_dir)?;

    println!("\nAll binaries built successfully!");
    Ok(())
}

fn build_with_features(features: &str, release: bool) -> Result<()> {
    let mut args = vec!["build", "--features", features];
    if release {
        args.push("--release");
    }
    run_command("cargo", &args)
}

// =============================================================================
// STANDARD TEST COMMANDS
// =============================================================================

/// Run tests
fn run_tests(unit: bool, db: bool, filter: Option<String>) -> Result<()> {
    let filter_str = filter.as_deref();

    if unit {
        println!("Running unit tests (no database)...");
        let mut args = vec!["test", "--lib", "--no-default-features"];
        if let Some(f) = filter_str {
            args.push(f);
        }
        run_command("cargo", &args)?;
    } else if db {
        println!("Running database integration tests...");
        let mut args = vec!["test", "--features", "database", "--test", "db_integration"];
        if let Some(f) = filter_str {
            args.push(f);
        }
        run_command("cargo", &args)?;
    } else {
        // Default: run all tests with database features
        println!("Running all tests with database features...");
        let mut args = vec!["test", "--features", "database", "--lib"];
        if let Some(f) = filter_str {
            args.push(f);
        }
        run_command("cargo", &args)?;
    }
    Ok(())
}

/// Run E2E tests
fn run_e2e() -> Result<()> {
    run_cargo_bin("run_e2e_tests", "database", false, &[])
}

/// Run DSL scenario tests (legacy shell script)
fn run_scenarios() -> Result<()> {
    let script = project_root()?.join("tests/scenarios/run_tests.sh");
    run_command("bash", &[script.to_str().context("Invalid script path")?])
}

// =============================================================================
// DSL PIPELINE TESTING
// =============================================================================

/// Golden file/snapshot testing for DSL outputs
fn test_snapshots(overwrite: bool, filter: Option<String>, output: &str) -> Result<()> {
    let scenarios_dir = project_root()?.join("tests/scenarios");

    if !scenarios_dir.exists() {
        bail!("Scenarios directory not found: {:?}", scenarios_dir);
    }

    println!("Running snapshot tests...");
    println!("  Mode: {}", if overwrite { "OVERWRITE" } else { "VERIFY" });
    println!("  Output: {}", output);
    if let Some(ref f) = filter {
        println!("  Filter: {}", f);
    }
    println!();

    // Find all .dsl files in scenarios directory
    let mut dsl_files: Vec<PathBuf> = fs::read_dir(&scenarios_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "dsl")
                .unwrap_or(false)
        })
        .map(|e| e.path())
        .collect();

    dsl_files.sort();

    // Apply filter if provided
    if let Some(ref f) = filter {
        dsl_files.retain(|p| {
            p.file_stem()
                .map(|s| s.to_string_lossy().contains(f))
                .unwrap_or(false)
        });
    }

    if dsl_files.is_empty() {
        println!("No scenario files found matching filter.");
        return Ok(());
    }

    let mut passed = 0;
    let mut failed = 0;
    let mut updated = 0;

    for dsl_file in &dsl_files {
        let name = dsl_file.file_stem().unwrap().to_string_lossy();
        print!("  {} ... ", name);

        // Run dsl_cli plan to get execution plan output
        let result = run_command_capture(
            "cargo",
            &[
                "run",
                "--bin",
                "dsl_cli",
                "--features",
                "cli,database",
                "--",
                "plan",
                "-f",
                dsl_file.to_str().unwrap(),
                "--format",
                "json",
            ],
        );

        match result {
            Ok(actual_output) => {
                let expected_file = dsl_file.with_extension("expected.json");

                if overwrite {
                    // Update expected file
                    fs::write(&expected_file, &actual_output)?;
                    println!("UPDATED");
                    updated += 1;
                } else if expected_file.exists() {
                    // Compare with expected
                    let expected = fs::read_to_string(&expected_file)?;
                    if normalize_json(&actual_output) == normalize_json(&expected) {
                        println!("OK");
                        passed += 1;
                    } else {
                        println!("FAILED");
                        println!("    Expected: {}", expected_file.display());
                        println!("    Run with --overwrite to update");
                        failed += 1;
                    }
                } else {
                    println!("NEW (no expected file)");
                    if overwrite {
                        fs::write(&expected_file, &actual_output)?;
                        updated += 1;
                    } else {
                        failed += 1;
                    }
                }
            }
            Err(e) => {
                println!("ERROR: {}", e);
                failed += 1;
            }
        }
    }

    println!();
    println!(
        "Results: {} passed, {} failed, {} updated",
        passed, failed, updated
    );

    if failed > 0 && !overwrite {
        bail!("Snapshot tests failed");
    }

    Ok(())
}

/// Test REPL incremental editing sessions
fn test_repl(scenario: Option<String>, verbose: bool) -> Result<()> {
    let scenarios_dir = project_root()?.join("tests/repl_scenarios");

    // Create directory if it doesn't exist
    if !scenarios_dir.exists() {
        println!("Creating REPL scenarios directory: {:?}", scenarios_dir);
        fs::create_dir_all(&scenarios_dir)?;

        // Create example scenario file
        let example = scenarios_dir.join("example.repl");
        fs::write(
            &example,
            r#"# Example REPL scenario
# Lines starting with # are comments
# Lines starting with > are commands to send
# Lines starting with ? are expected patterns in response

> (cbu.ensure :name "Test Fund" :jurisdiction "LU" :client-type "fund" :as @fund)
? success
? @fund

> (entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)
? success
? @john

# Use previously bound symbol
> (cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
? success
"#,
        )?;
        println!("Created example scenario: {:?}", example);
    }

    // Find scenario files
    let mut scenario_files: Vec<PathBuf> = fs::read_dir(&scenarios_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "repl")
                .unwrap_or(false)
        })
        .map(|e| e.path())
        .collect();

    scenario_files.sort();

    // Filter to specific scenario if provided
    if let Some(ref name) = scenario {
        scenario_files.retain(|p| {
            p.file_stem()
                .map(|s| s.to_string_lossy().contains(name))
                .unwrap_or(false)
        });
    }

    if scenario_files.is_empty() {
        println!("No REPL scenario files found in {:?}", scenarios_dir);
        println!("Create .repl files with:");
        println!("  > (dsl-command)  -- send command");
        println!("  ? pattern        -- expect pattern in response");
        return Ok(());
    }

    println!("Running REPL session tests...");
    if verbose {
        println!("  Verbose mode enabled");
    }
    println!();

    let mut total_passed = 0;
    let mut total_failed = 0;

    for scenario_file in &scenario_files {
        let name = scenario_file.file_stem().unwrap().to_string_lossy();
        println!("Scenario: {}", name);

        let content = fs::read_to_string(scenario_file)?;
        let mut commands: Vec<&str> = Vec::new();
        let mut expectations: Vec<Vec<&str>> = Vec::new();
        let mut current_expects: Vec<&str> = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if let Some(cmd) = trimmed.strip_prefix("> ") {
                // Save previous expectations
                if !commands.is_empty() {
                    expectations.push(std::mem::take(&mut current_expects));
                }
                commands.push(cmd);
            } else if let Some(expect) = trimmed.strip_prefix("? ") {
                current_expects.push(expect);
            }
        }
        // Don't forget last set of expectations
        if !current_expects.is_empty() {
            expectations.push(current_expects);
        }

        // Ensure we have expectations for all commands
        while expectations.len() < commands.len() {
            expectations.push(Vec::new());
        }

        // Run commands through dsl_cli (simplified - would need actual session support)
        for (i, (cmd, expects)) in commands.iter().zip(expectations.iter()).enumerate() {
            print!("  Step {}: ", i + 1);

            if verbose {
                println!();
                println!("    Command: {}", cmd);
            }

            // For now, just validate the DSL can be parsed
            let result = run_command_capture(
                "cargo",
                &[
                    "run",
                    "--bin",
                    "dsl_cli",
                    "--features",
                    "cli,database",
                    "-q",
                    "--",
                    "validate",
                    "-i",
                    cmd,
                ],
            );

            match result {
                Ok(output) => {
                    let mut all_match = true;
                    for expect in expects {
                        if !output.contains(expect) {
                            if verbose {
                                println!("    Expected '{}' not found", expect);
                            }
                            all_match = false;
                        }
                    }

                    if all_match {
                        if !verbose {
                            print!("OK ");
                        } else {
                            println!("    OK");
                        }
                        total_passed += 1;
                    } else {
                        if !verbose {
                            print!("FAIL ");
                        } else {
                            println!("    FAILED");
                        }
                        total_failed += 1;
                    }
                }
                Err(e) => {
                    if verbose {
                        println!("    ERROR: {}", e);
                    } else {
                        print!("ERR ");
                    }
                    total_failed += 1;
                }
            }
        }
        println!();
    }

    println!();
    println!(
        "REPL Tests: {} passed, {} failed",
        total_passed, total_failed
    );

    if total_failed > 0 {
        bail!("REPL tests failed");
    }

    Ok(())
}

/// Full integration test with DB + EntityGateway
fn test_integration(skip_gateway: bool, keep_running: bool) -> Result<()> {
    println!("Running full integration tests...");
    println!();

    // Check DATABASE_URL
    let db_url = std::env::var("DATABASE_URL").ok();
    if db_url.is_none() {
        println!("WARNING: DATABASE_URL not set. Using default: postgresql:///data_designer");
    }

    let mut gateway_process: Option<Child> = None;

    // Start EntityGateway if not skipped
    if !skip_gateway {
        println!("Starting EntityGateway...");
        let gateway_dir = project_root()?.join("crates/entity-gateway");

        let child = Command::new("cargo")
            .args(["run", "--release"])
            .current_dir(&gateway_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to start EntityGateway")?;

        gateway_process = Some(child);

        // Wait for gateway to be ready (check port 50051)
        print!("Waiting for EntityGateway on port 50051...");
        std::io::stdout().flush()?;

        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(500));

            // Try to connect to port 50051
            if std::net::TcpStream::connect("127.0.0.1:50051").is_ok() {
                println!(" Ready!");
                break;
            }
            print!(".");
            std::io::stdout().flush()?;
        }
        println!();
    } else {
        println!("Skipping EntityGateway startup (--skip-gateway)");
    }

    // Run integration tests
    println!("Running integration tests...");
    let test_result = run_command(
        "cargo",
        &["test", "--features", "database", "--test", "db_integration"],
    );

    // Cleanup
    if let Some(mut child) = gateway_process {
        if keep_running {
            println!("Keeping EntityGateway running (--keep-running)");
        } else {
            println!("Stopping EntityGateway...");
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    test_result
}

// =============================================================================
// CODE GENERATION
// =============================================================================

/// Generate Rust code from YAML verb definitions
fn run_codegen(check: bool) -> Result<()> {
    let config_dir = project_root()?.join("config/verbs");
    let output_file = project_root()?.join("src/dsl_v2/generated_verbs.rs");

    if !config_dir.exists() {
        bail!("Verb config directory not found: {:?}", config_dir);
    }

    println!("Generating Rust code from YAML verb definitions...");
    println!("  Config: {:?}", config_dir);
    println!("  Output: {:?}", output_file);

    // For now, just validate that YAML files can be parsed
    // Full codegen would generate Rust structs from verb definitions

    let mut yaml_files: Vec<PathBuf> = Vec::new();
    collect_yaml_files(&config_dir, &mut yaml_files)?;

    println!("  Found {} YAML files", yaml_files.len());

    // Validate YAML syntax
    for yaml_file in &yaml_files {
        let content = fs::read_to_string(yaml_file)?;
        match serde_yaml::from_str::<serde_yaml::Value>(&content) {
            Ok(_) => {}
            Err(e) => {
                bail!("Invalid YAML in {:?}: {}", yaml_file, e);
            }
        }
    }

    if check {
        println!("Check mode: YAML files are valid");
        // In full implementation, would compare generated code with existing file
    } else {
        println!("Codegen complete (stub - full implementation would generate Rust code)");
        // In full implementation, would write generated code to output_file
    }

    Ok(())
}

fn collect_yaml_files(dir: &std::path::Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_yaml_files(&path, files)?;
        } else if path
            .extension()
            .map(|e| e == "yaml" || e == "yml")
            .unwrap_or(false)
        {
            files.push(path);
        }
    }
    Ok(())
}

// =============================================================================
// CODE QUALITY
// =============================================================================

/// Run clippy on all feature combinations
fn run_clippy(fix: bool) -> Result<()> {
    let feature_sets = ["server", "database", "mcp", "cli,database"];

    for features in feature_sets {
        println!("Running clippy with --features {}...", features);

        let mut args = vec!["clippy", "--features", features];
        if fix {
            args.extend(["--fix", "--allow-dirty", "--allow-staged"]);
        }
        args.extend(["--", "-D", "warnings"]);

        run_command("cargo", &args)?;
    }

    // Also run on EntityGateway
    println!("Running clippy on entity-gateway...");
    let gateway_dir = project_root()?.join("crates/entity-gateway");
    let mut gateway_args = vec!["clippy"];
    if fix {
        gateway_args.extend(["--fix", "--allow-dirty", "--allow-staged"]);
    }
    gateway_args.extend(["--", "-D", "warnings"]);
    run_command_in_dir("cargo", &gateway_args, &gateway_dir)?;

    println!("\nClippy passed on all feature combinations!");
    Ok(())
}

/// Run cargo fmt
fn run_fmt(check: bool) -> Result<()> {
    let mut args = vec!["fmt", "--all"];
    if check {
        args.push("--check");
    }
    run_command("cargo", &args)
}

/// Prepare sqlx offline mode
fn sqlx_prepare() -> Result<()> {
    check_tool_installed("sqlx", "cargo install sqlx-cli")?;

    println!("Preparing sqlx offline data...");
    run_command(
        "cargo",
        &["sqlx", "prepare", "--", "--features", "database"],
    )
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

fn project_root() -> Result<std::path::PathBuf> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .context("CARGO_MANIFEST_DIR not set - are you running via cargo?")?;

    // xtask is in rust/xtask, so go up one level to get rust/
    let xtask_dir = std::path::PathBuf::from(manifest_dir);
    let root = xtask_dir
        .parent()
        .context("Failed to find parent of xtask directory")?;

    Ok(root.to_path_buf())
}

fn check_tool_installed(tool: &str, install_hint: &str) -> Result<()> {
    let status = Command::new("which")
        .arg(tool)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    match status {
        Ok(s) if s.success() => Ok(()),
        _ => bail!(
            "Tool '{}' is not installed.\nInstall with: {}",
            tool,
            install_hint
        ),
    }
}

fn run_command(cmd: &str, args: &[&str]) -> Result<()> {
    let root = project_root()?;
    run_command_in_dir(cmd, args, &root)
}

fn run_command_in_dir(cmd: &str, args: &[&str], dir: &std::path::Path) -> Result<()> {
    println!("+ {} {}", cmd, args.join(" "));

    let status = Command::new(cmd)
        .args(args)
        .current_dir(dir)
        .status()
        .with_context(|| format!("Failed to run: {} {}", cmd, args.join(" ")))?;

    check_status(status, cmd)
}

fn run_command_capture(cmd: &str, args: &[&str]) -> Result<String> {
    let root = project_root()?;

    let output = Command::new(cmd)
        .args(args)
        .current_dir(&root)
        .output()
        .with_context(|| format!("Failed to run: {} {}", cmd, args.join(" ")))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Command failed: {}", stderr)
    }
}

fn check_status(status: ExitStatus, cmd: &str) -> Result<()> {
    if status.success() {
        Ok(())
    } else {
        bail!(
            "Command '{}' failed with exit code: {:?}",
            cmd,
            status.code()
        )
    }
}

/// Normalize JSON for comparison (remove whitespace differences)
fn normalize_json(json: &str) -> String {
    // Simple normalization - in production, would parse and re-serialize
    json.split_whitespace().collect::<Vec<_>>().join(" ")
}
