# DSL CLI Implementation Spec

**Purpose**: Build a command-line interface for the DSL pipeline that Claude Code can use for testing and prototyping agent workflows.

**Goal**: Enable Claude Code to generate DSL, validate it, execute it, and iterate based on feedback - all via shell commands.

---

## Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        dsl_cli                                   │
├─────────────────────────────────────────────────────────────────┤
│  Commands:                                                       │
│    parse     - Parse DSL → JSON AST                             │
│    validate  - Parse + CSG Lint → diagnostics                   │
│    plan      - Parse + Lint + Compile → execution plan          │
│    execute   - Full pipeline → database mutations               │
│    intent    - Intent JSON → DSL source                         │
│    demo      - Run built-in demo scenarios                      │
└─────────────────────────────────────────────────────────────────┘
```

---

## File Structure

```
rust/
├── src/
│   └── bin/
│       └── dsl_cli.rs          # Main CLI entry point
├── Cargo.toml                   # Add [[bin]] entry
└── tests/
    └── cli_integration.rs       # CLI integration tests
```

---

## Step 1: Update Cargo.toml

Add to `rust/Cargo.toml`:

```toml
[[bin]]
name = "dsl_cli"
path = "src/bin/dsl_cli.rs"
required-features = ["cli"]

[features]
default = []
database = ["sqlx"]
cli = ["clap", "colored", "atty"]

[dependencies]
# Add these if not present
clap = { version = "4.4", features = ["derive"], optional = true }
colored = { version = "2.1", optional = true }
atty = { version = "0.2", optional = true }
```

---

## Step 2: Implement dsl_cli.rs

Create file: `rust/src/bin/dsl_cli.rs`

```rust
//! DSL Command Line Interface
//!
//! A CLI for parsing, validating, and executing DSL programs.
//! Designed for use by Claude Code and other agents.
//!
//! # Usage
//!
//! ```bash
//! # Parse DSL to AST
//! echo '(cbu.create :name "Test")' | dsl_cli parse
//!
//! # Validate DSL (parse + CSG lint)
//! dsl_cli validate --file program.dsl
//!
//! # Execute DSL against database
//! dsl_cli execute --file program.dsl --db-url postgres://...
//!
//! # Generate DSL from intent JSON
//! echo '{"intent":"onboard_individual",...}' | dsl_cli intent
//!
//! # Run demo scenario
//! dsl_cli demo onboard-individual
//! ```

use clap::{Parser, Subcommand};
use colored::Colorize;
use std::io::{self, BufRead, Read};
use std::path::PathBuf;
use std::process::ExitCode;

// Import from library
use ob_poc::dsl_v2::{
    parse_program,
    compile,
    validation::{RustStyleFormatter, ValidationContext, ClientType},
    CsgLinter,
    ApplicabilityRules,
};

#[cfg(feature = "database")]
use ob_poc::dsl_v2::DslExecutor;

#[derive(Parser)]
#[command(name = "dsl_cli")]
#[command(author = "ob-poc")]
#[command(version = "0.1.0")]
#[command(about = "DSL pipeline CLI for parsing, validating, and executing DSL programs")]
#[command(long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output format: json, text, or pretty (default)
    #[arg(long, short, global = true, default_value = "pretty")]
    format: OutputFormat,

    /// Suppress non-essential output
    #[arg(long, short, global = true)]
    quiet: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Json,
    Text,
    Pretty,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(OutputFormat::Json),
            "text" => Ok(OutputFormat::Text),
            "pretty" => Ok(OutputFormat::Pretty),
            _ => Err(format!("Unknown format: {}", s)),
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Parse DSL source into AST (no validation)
    Parse {
        /// Input file (reads stdin if not provided)
        #[arg(short, long)]
        file: Option<PathBuf>,
    },

    /// Validate DSL source (parse + CSG lint)
    Validate {
        /// Input file (reads stdin if not provided)
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Client type context: individual, corporate, fund, trust
        #[arg(long)]
        client_type: Option<String>,

        /// Jurisdiction context (ISO 2-letter code)
        #[arg(long)]
        jurisdiction: Option<String>,
    },

    /// Compile DSL to execution plan (parse + lint + compile)
    Plan {
        /// Input file (reads stdin if not provided)
        #[arg(short, long)]
        file: Option<PathBuf>,
    },

    /// Execute DSL against database
    #[cfg(feature = "database")]
    Execute {
        /// Input file (reads stdin if not provided)
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Database URL (or use DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL")]
        db_url: String,

        /// Dry run - validate and plan but don't execute
        #[arg(long)]
        dry_run: bool,

        /// Client type context
        #[arg(long)]
        client_type: Option<String>,

        /// Jurisdiction context
        #[arg(long)]
        jurisdiction: Option<String>,
    },

    /// Generate DSL from intent JSON
    Intent {
        /// Input file containing intent JSON (reads stdin if not provided)
        #[arg(short, long)]
        file: Option<PathBuf>,
    },

    /// Run a built-in demo scenario
    Demo {
        /// Scenario name: onboard-individual, onboard-corporate, add-document
        scenario: String,

        /// Actually execute against database (requires --db-url)
        #[arg(long)]
        execute: bool,

        /// Database URL for execution
        #[arg(long, env = "DATABASE_URL")]
        db_url: Option<String>,
    },

    /// List available verbs and their schemas
    Verbs {
        /// Filter by domain: cbu, entity, document, attribute, role, kyc
        #[arg(short, long)]
        domain: Option<String>,

        /// Show full schema details
        #[arg(long)]
        verbose: bool,
    },

    /// Show example DSL programs
    Examples {
        /// Category: onboarding, documents, entities, all
        #[arg(default_value = "all")]
        category: String,
    },
}

// =============================================================================
// MAIN
// =============================================================================

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Parse { file } => cmd_parse(file, cli.format),
        Commands::Validate { file, client_type, jurisdiction } => {
            cmd_validate(file, client_type, jurisdiction, cli.format)
        }
        Commands::Plan { file } => cmd_plan(file, cli.format),
        #[cfg(feature = "database")]
        Commands::Execute { file, db_url, dry_run, client_type, jurisdiction } => {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(cmd_execute(file, db_url, dry_run, client_type, jurisdiction, cli.format))
        }
        Commands::Intent { file } => cmd_intent(file, cli.format),
        Commands::Demo { scenario, execute, db_url } => {
            cmd_demo(&scenario, execute, db_url, cli.format, cli.quiet)
        }
        Commands::Verbs { domain, verbose } => cmd_verbs(domain, verbose, cli.format),
        Commands::Examples { category } => cmd_examples(&category, cli.format),
    };

    match result {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            if cli.format == OutputFormat::Json {
                println!(r#"{{"error": "{}"}}"#, e.replace('"', "\\\""));
            } else {
                eprintln!("{}: {}", "error".red().bold(), e);
            }
            ExitCode::FAILURE
        }
    }
}

// =============================================================================
// COMMAND IMPLEMENTATIONS
// =============================================================================

fn cmd_parse(file: Option<PathBuf>, format: OutputFormat) -> Result<(), String> {
    let source = read_input(file)?;
    
    let ast = parse_program(&source)
        .map_err(|e| format!("Parse error: {:?}", e))?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&ast)
                .map_err(|e| format!("JSON serialization failed: {}", e))?;
            println!("{}", json);
        }
        OutputFormat::Text | OutputFormat::Pretty => {
            println!("{} Parsed {} statement(s)", "✓".green(), ast.statements.len());
            for (i, stmt) in ast.statements.iter().enumerate() {
                println!("  [{}] {:?}", i, stmt);
            }
        }
    }

    Ok(())
}

fn cmd_validate(
    file: Option<PathBuf>,
    client_type: Option<String>,
    jurisdiction: Option<String>,
    format: OutputFormat,
) -> Result<(), String> {
    let source = read_input(file)?;

    // Parse
    let ast = parse_program(&source)
        .map_err(|e| format!("Parse error: {:?}", e))?;

    // Build context
    let mut context = ValidationContext::default();
    if let Some(ct) = client_type {
        context.client_type = Some(parse_client_type(&ct)?);
    }
    if let Some(j) = jurisdiction {
        context.jurisdiction = Some(j);
    }

    // CSG Lint (without database, uses empty rules)
    let linter = CsgLinter::new();
    let source_clone = source.clone();
    
    // For non-database validation, we do a simpler check
    // Full validation requires database connection
    let lint_result = futures::executor::block_on(async {
        let mut linter = linter;
        let _ = linter.initialize().await;
        linter.lint(ast.clone(), &context, &source_clone).await
    });

    // Output results
    match format {
        OutputFormat::Json => {
            let output = serde_json::json!({
                "valid": !lint_result.has_errors(),
                "errors": lint_result.diagnostics.iter()
                    .filter(|d| d.severity == ob_poc::dsl_v2::validation::Severity::Error)
                    .count(),
                "warnings": lint_result.diagnostics.iter()
                    .filter(|d| d.severity == ob_poc::dsl_v2::validation::Severity::Warning)
                    .count(),
                "diagnostics": lint_result.diagnostics,
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        OutputFormat::Text | OutputFormat::Pretty => {
            if lint_result.diagnostics.is_empty() {
                println!("{} Validation passed ({} statements)", 
                    "✓".green().bold(), ast.statements.len());
            } else {
                let formatted = RustStyleFormatter::format(&source, &lint_result.diagnostics);
                println!("{}", formatted);
            }
        }
    }

    if lint_result.has_errors() {
        Err("Validation failed".to_string())
    } else {
        Ok(())
    }
}

fn cmd_plan(file: Option<PathBuf>, format: OutputFormat) -> Result<(), String> {
    let source = read_input(file)?;

    // Parse
    let ast = parse_program(&source)
        .map_err(|e| format!("Parse error: {:?}", e))?;

    // Compile to execution plan
    let plan = compile(&ast)
        .map_err(|e| format!("Compile error: {:?}", e))?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&plan)
                .map_err(|e| format!("JSON serialization failed: {}", e))?;
            println!("{}", json);
        }
        OutputFormat::Text | OutputFormat::Pretty => {
            println!("{} Compiled to {} step(s)", "✓".green().bold(), plan.steps.len());
            println!();
            for (i, step) in plan.steps.iter().enumerate() {
                println!("{}:", format!("Step {}", i).cyan().bold());
                println!("  Verb: {}.{}", step.domain, step.verb);
                println!("  Args: {:?}", step.args);
                if let Some(ref binding) = step.binding {
                    println!("  Binding: @{} → ${}", binding, i);
                }
                if !step.injections.is_empty() {
                    println!("  Injections: {:?}", step.injections);
                }
                println!();
            }
        }
    }

    Ok(())
}

#[cfg(feature = "database")]
async fn cmd_execute(
    file: Option<PathBuf>,
    db_url: String,
    dry_run: bool,
    client_type: Option<String>,
    jurisdiction: Option<String>,
    format: OutputFormat,
) -> Result<(), String> {
    let source = read_input(file)?;

    // Connect to database
    let pool = sqlx::PgPool::connect(&db_url)
        .await
        .map_err(|e| format!("Database connection failed: {}", e))?;

    // Parse
    let ast = parse_program(&source)
        .map_err(|e| format!("Parse error: {:?}", e))?;

    // Build context
    let mut context = ValidationContext::default();
    if let Some(ct) = client_type {
        context.client_type = Some(parse_client_type(&ct)?);
    }
    if let Some(j) = jurisdiction {
        context.jurisdiction = Some(j);
    }

    // CSG Lint with database rules
    let mut linter = CsgLinter::new(pool.clone());
    linter.initialize().await
        .map_err(|e| format!("Linter initialization failed: {}", e))?;

    let lint_result = linter.lint(ast.clone(), &context, &source).await;

    if lint_result.has_errors() {
        let formatted = RustStyleFormatter::format(&source, &lint_result.diagnostics);
        if format == OutputFormat::Json {
            println!(r#"{{"error": "validation_failed", "diagnostics": {}}}"#,
                serde_json::to_string(&lint_result.diagnostics).unwrap());
        } else {
            eprintln!("{}", formatted);
        }
        return Err("Validation failed".to_string());
    }

    // Compile
    let plan = compile(&ast)
        .map_err(|e| format!("Compile error: {:?}", e))?;

    if dry_run {
        if format == OutputFormat::Json {
            println!(r#"{{"dry_run": true, "steps": {}, "would_execute": true}}"#, plan.steps.len());
        } else {
            println!("{} Dry run: {} step(s) would execute", "✓".green().bold(), plan.steps.len());
        }
        return Ok(());
    }

    // Execute
    let executor = DslExecutor::new(pool);
    let exec_context = ob_poc::dsl_v2::ExecutionContext::default();
    
    let result = executor.execute(&plan, &exec_context)
        .await
        .map_err(|e| format!("Execution failed: {}", e))?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&result)
                .map_err(|e| format!("JSON serialization failed: {}", e))?;
            println!("{}", json);
        }
        OutputFormat::Text | OutputFormat::Pretty => {
            println!("{} Executed {} step(s)", "✓".green().bold(), plan.steps.len());
            println!();
            println!("Results:");
            println!("{:#?}", result);
        }
    }

    Ok(())
}

fn cmd_intent(file: Option<PathBuf>, format: OutputFormat) -> Result<(), String> {
    let input = read_input(file)?;

    // Parse intent JSON
    let intent: ob_poc::intent::Intent = serde_json::from_str(&input)
        .map_err(|e| format!("Invalid intent JSON: {}", e))?;

    // Generate DSL
    let dsl = ob_poc::planner::plan_to_dsl(&intent)
        .map_err(|e| format!("DSL generation failed: {}", e))?;

    match format {
        OutputFormat::Json => {
            let output = serde_json::json!({
                "dsl": dsl,
                "intent_type": format!("{:?}", intent),
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        OutputFormat::Text | OutputFormat::Pretty => {
            println!("{} Generated DSL:", "✓".green().bold());
            println!();
            println!("{}", dsl);
        }
    }

    Ok(())
}

fn cmd_demo(
    scenario: &str,
    execute: bool,
    db_url: Option<String>,
    format: OutputFormat,
    quiet: bool,
) -> Result<(), String> {
    let (name, dsl) = match scenario {
        "onboard-individual" | "individual" => (
            "Onboard Individual Client",
            r#"
; Onboard an individual client with passport
(cbu.create 
    :name "John Smith" 
    :client-type "individual" 
    :jurisdiction "GB"
    :as @cbu)

(entity.create-proper-person 
    :cbu-id @cbu
    :name "John Smith"
    :first-name "John"
    :last-name "Smith"
    :as @person)

(document.catalog
    :cbu-id @cbu
    :entity-id @person
    :document-type "PASSPORT"
    :as @passport)
"#,
        ),
        "onboard-corporate" | "corporate" => (
            "Onboard Corporate Client",
            r#"
; Onboard a corporate client with UBO
(cbu.create 
    :name "Acme Holdings Ltd" 
    :client-type "corporate" 
    :jurisdiction "GB"
    :as @cbu)

(entity.create-limited-company 
    :cbu-id @cbu
    :name "Acme Holdings Ltd"
    :company-number "12345678"
    :as @company)

(entity.create-proper-person
    :cbu-id @cbu
    :name "Jane Doe"
    :first-name "Jane"
    :last-name "Doe"
    :as @ubo)

(role.assign
    :cbu-id @cbu
    :entity-id @ubo
    :target-entity-id @company
    :role "BENEFICIAL_OWNER"
    :ownership-percentage 75.0)

(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :document-type "CERTIFICATE_OF_INCORPORATION")

(document.catalog
    :cbu-id @cbu
    :entity-id @ubo
    :document-type "PASSPORT")
"#,
        ),
        "add-document" | "document" => (
            "Add Document to Existing Entity",
            r#"
; Add a document to an existing entity
; Note: In real use, @entity would be resolved from database
(document.catalog
    :cbu-id @cbu
    :entity-id @entity
    :document-type "PROOF_OF_ADDRESS"
    :as @poa)
"#,
        ),
        "invalid-passport-company" | "invalid" => (
            "Invalid: Passport for Company (Should Fail)",
            r#"
; This should fail CSG validation - passport not valid for company
(cbu.create 
    :name "Test Corp" 
    :client-type "corporate" 
    :jurisdiction "GB"
    :as @cbu)

(entity.create-limited-company 
    :cbu-id @cbu
    :name "Test Corp"
    :as @company)

(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :document-type "PASSPORT")
"#,
        ),
        _ => {
            return Err(format!(
                "Unknown scenario: '{}'\n\nAvailable scenarios:\n  \
                onboard-individual  - Individual client with passport\n  \
                onboard-corporate   - Corporate client with UBO structure\n  \
                add-document        - Add document to existing entity\n  \
                invalid             - Invalid DSL (passport for company)",
                scenario
            ));
        }
    };

    if !quiet {
        println!("{}: {}", "Demo".cyan().bold(), name);
        println!("{}", "─".repeat(60));
        println!();
    }

    // Show the DSL
    if format == OutputFormat::Pretty && !quiet {
        println!("{}:", "DSL Source".yellow());
        for line in dsl.lines() {
            if line.trim().starts_with(';') {
                println!("  {}", line.dimmed());
            } else if !line.trim().is_empty() {
                println!("  {}", line);
            }
        }
        println!();
    }

    // Validate
    if !quiet {
        println!("{}:", "Validating".yellow());
    }

    let validation_result = cmd_validate(
        None,
        Some("corporate".to_string()),
        Some("GB".to_string()),
        if quiet { OutputFormat::Json } else { OutputFormat::Pretty },
    );

    // Handle the "invalid" demo specially - it SHOULD fail
    if scenario == "invalid-passport-company" || scenario == "invalid" {
        match validation_result {
            Ok(_) => {
                println!("{} Expected validation to fail but it passed!", "✗".red().bold());
                return Err("Demo validation should have failed".to_string());
            }
            Err(_) => {
                println!();
                println!("{} Correctly rejected invalid DSL", "✓".green().bold());
                return Ok(());
            }
        }
    }

    validation_result?;

    // Optionally execute
    if execute {
        let db_url = db_url.ok_or("--db-url required for execution")?;
        
        #[cfg(feature = "database")]
        {
            println!();
            println!("{}:", "Executing".yellow());
            // Would call cmd_execute here
            println!("{} Execution not yet implemented in demo", "⚠".yellow());
        }
        
        #[cfg(not(feature = "database"))]
        {
            return Err("Compile with --features database for execution".to_string());
        }
    }

    Ok(())
}

fn cmd_verbs(domain: Option<String>, verbose: bool, format: OutputFormat) -> Result<(), String> {
    use ob_poc::dsl_v2::verbs::{domains, verbs_for_domain, STANDARD_VERBS};

    let domains_to_show: Vec<String> = match domain {
        Some(d) => vec![d],
        None => domains().into_iter().map(|s| s.to_string()).collect(),
    };

    match format {
        OutputFormat::Json => {
            let mut output = serde_json::Map::new();
            for d in &domains_to_show {
                let verbs: Vec<_> = verbs_for_domain(d)
                    .iter()
                    .map(|v| {
                        serde_json::json!({
                            "verb": format!("{}.{}", v.domain, v.verb),
                            "description": v.description,
                            "args": v.args,
                        })
                    })
                    .collect();
                output.insert(d.clone(), serde_json::Value::Array(verbs));
            }
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        OutputFormat::Text | OutputFormat::Pretty => {
            for d in &domains_to_show {
                println!("{}", format!("Domain: {}", d).cyan().bold());
                println!();
                
                for v in verbs_for_domain(d) {
                    println!("  {}.{}", v.domain.green(), v.verb.green().bold());
                    if verbose {
                        println!("    {}", v.description.dimmed());
                        println!("    Args: {:?}", v.args);
                    }
                }
                println!();
            }
        }
    }

    Ok(())
}

fn cmd_examples(category: &str, format: OutputFormat) -> Result<(), String> {
    let examples = vec![
        ("onboarding", "Create CBU", "(cbu.create :name \"Client Name\" :client-type \"individual\" :jurisdiction \"GB\" :as @cbu)"),
        ("onboarding", "Create Person", "(entity.create-proper-person :cbu-id @cbu :name \"John Doe\" :as @person)"),
        ("onboarding", "Create Company", "(entity.create-limited-company :cbu-id @cbu :name \"Acme Ltd\" :company-number \"12345678\" :as @company)"),
        ("documents", "Catalog Passport", "(document.catalog :cbu-id @cbu :entity-id @person :document-type \"PASSPORT\")"),
        ("documents", "Catalog Certificate", "(document.catalog :cbu-id @cbu :entity-id @company :document-type \"CERTIFICATE_OF_INCORPORATION\")"),
        ("entities", "Create Trust", "(entity.create-trust :cbu-id @cbu :name \"Family Trust\" :as @trust)"),
        ("entities", "Create Partnership", "(entity.create-partnership :cbu-id @cbu :name \"Smith & Co\" :as @partnership)"),
        ("roles", "Assign UBO Role", "(role.assign :cbu-id @cbu :entity-id @person :target-entity-id @company :role \"BENEFICIAL_OWNER\" :ownership-percentage 51.0)"),
        ("roles", "Assign Director", "(role.assign :cbu-id @cbu :entity-id @person :target-entity-id @company :role \"DIRECTOR\")"),
    ];

    let filtered: Vec<_> = if category == "all" {
        examples
    } else {
        examples.into_iter().filter(|(cat, _, _)| *cat == category).collect()
    };

    if filtered.is_empty() {
        return Err(format!(
            "Unknown category: '{}'\n\nAvailable: onboarding, documents, entities, roles, all",
            category
        ));
    }

    match format {
        OutputFormat::Json => {
            let output: Vec<_> = filtered.iter().map(|(cat, name, dsl)| {
                serde_json::json!({
                    "category": cat,
                    "name": name,
                    "dsl": dsl,
                })
            }).collect();
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        OutputFormat::Text | OutputFormat::Pretty => {
            let mut current_cat = "";
            for (cat, name, dsl) in &filtered {
                if *cat != current_cat {
                    if !current_cat.is_empty() {
                        println!();
                    }
                    println!("{}", format!("Category: {}", cat).cyan().bold());
                    current_cat = cat;
                }
                println!();
                println!("  {}", name.yellow());
                println!("  {}", dsl.green());
            }
        }
    }

    Ok(())
}

// =============================================================================
// HELPERS
// =============================================================================

fn read_input(file: Option<PathBuf>) -> Result<String, String> {
    match file {
        Some(path) => {
            std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read '{}': {}", path.display(), e))
        }
        None => {
            // Check if stdin has data
            if atty::is(atty::Stream::Stdin) {
                return Err("No input provided. Use --file or pipe input via stdin.".to_string());
            }
            let mut buffer = String::new();
            io::stdin()
                .read_to_string(&mut buffer)
                .map_err(|e| format!("Failed to read stdin: {}", e))?;
            Ok(buffer)
        }
    }
}

fn parse_client_type(s: &str) -> Result<ClientType, String> {
    match s.to_lowercase().as_str() {
        "individual" => Ok(ClientType::Individual),
        "corporate" => Ok(ClientType::Corporate),
        "fund" => Ok(ClientType::Fund),
        "trust" => Ok(ClientType::Trust),
        _ => Err(format!("Unknown client type: '{}'. Use: individual, corporate, fund, trust", s)),
    }
}
```

---

## Step 3: Self-Test Script

Create file: `rust/tests/cli_self_test.sh`

```bash
#!/bin/bash
# Self-test script for dsl_cli
# Run with: bash rust/tests/cli_self_test.sh

set -e

CLI="cargo run --features cli --bin dsl_cli --"
PASS=0
FAIL=0

echo "═══════════════════════════════════════════════════════════"
echo "                    DSL CLI Self-Test                       "
echo "═══════════════════════════════════════════════════════════"
echo ""

# Helper function
test_case() {
    local name="$1"
    local cmd="$2"
    local expect_fail="${3:-false}"
    
    echo -n "Testing: $name... "
    
    if eval "$cmd" > /tmp/dsl_test_output.txt 2>&1; then
        if [ "$expect_fail" = "true" ]; then
            echo "FAIL (expected failure but passed)"
            FAIL=$((FAIL + 1))
            cat /tmp/dsl_test_output.txt
        else
            echo "PASS"
            PASS=$((PASS + 1))
        fi
    else
        if [ "$expect_fail" = "true" ]; then
            echo "PASS (expected failure)"
            PASS=$((PASS + 1))
        else
            echo "FAIL"
            FAIL=$((FAIL + 1))
            cat /tmp/dsl_test_output.txt
        fi
    fi
}

# Build first
echo "Building CLI..."
cargo build --features cli --bin dsl_cli
echo ""

# ─────────────────────────────────────────────────────────────
# PARSE TESTS
# ─────────────────────────────────────────────────────────────
echo "── Parse Tests ──"

test_case "Parse simple CBU create" \
    "echo '(cbu.create :name \"Test\")' | $CLI parse"

test_case "Parse with binding" \
    "echo '(cbu.create :name \"Test\" :as @cbu)' | $CLI parse"

test_case "Parse multiple statements" \
    "echo '(cbu.create :name \"Test\" :as @cbu)
(entity.create-proper-person :cbu-id @cbu :name \"John\" :as @person)' | $CLI parse"

test_case "Parse JSON output" \
    "echo '(cbu.create :name \"Test\")' | $CLI parse --format json"

test_case "Parse invalid syntax" \
    "echo '(cbu.create :name' | $CLI parse" \
    true

echo ""

# ─────────────────────────────────────────────────────────────
# VALIDATE TESTS
# ─────────────────────────────────────────────────────────────
echo "── Validate Tests ──"

test_case "Validate simple program" \
    "echo '(cbu.create :name \"Test\" :jurisdiction \"GB\" :as @cbu)' | $CLI validate"

test_case "Validate with context" \
    "echo '(cbu.create :name \"Test\")' | $CLI validate --client-type individual --jurisdiction GB"

test_case "Validate undefined symbol" \
    "echo '(document.catalog :entity-id @nonexistent :document-type \"PASSPORT\")' | $CLI validate" \
    true

test_case "Validate JSON output" \
    "echo '(cbu.create :name \"Test\")' | $CLI validate --format json"

echo ""

# ─────────────────────────────────────────────────────────────
# PLAN TESTS
# ─────────────────────────────────────────────────────────────
echo "── Plan Tests ──"

test_case "Plan simple program" \
    "echo '(cbu.create :name \"Test\" :as @cbu)' | $CLI plan"

test_case "Plan with dependencies" \
    "echo '(cbu.create :name \"Test\" :as @cbu)
(entity.create-proper-person :cbu-id @cbu :name \"John\")' | $CLI plan"

test_case "Plan JSON output" \
    "echo '(cbu.create :name \"Test\")' | $CLI plan --format json"

echo ""

# ─────────────────────────────────────────────────────────────
# DEMO TESTS
# ─────────────────────────────────────────────────────────────
echo "── Demo Tests ──"

test_case "Demo: onboard-individual" \
    "$CLI demo onboard-individual --quiet"

test_case "Demo: onboard-corporate" \
    "$CLI demo onboard-corporate --quiet"

test_case "Demo: invalid (should fail validation)" \
    "$CLI demo invalid --quiet"

test_case "Demo: unknown scenario" \
    "$CLI demo unknown-scenario" \
    true

echo ""

# ─────────────────────────────────────────────────────────────
# VERBS & EXAMPLES TESTS
# ─────────────────────────────────────────────────────────────
echo "── Info Tests ──"

test_case "List all verbs" \
    "$CLI verbs"

test_case "List verbs for domain" \
    "$CLI verbs --domain cbu"

test_case "List verbs JSON" \
    "$CLI verbs --format json"

test_case "Show examples" \
    "$CLI examples"

test_case "Show examples by category" \
    "$CLI examples onboarding"

echo ""

# ─────────────────────────────────────────────────────────────
# SUMMARY
# ─────────────────────────────────────────────────────────────
echo "═══════════════════════════════════════════════════════════"
echo "                       RESULTS                              "
echo "═══════════════════════════════════════════════════════════"
echo ""
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo ""

if [ $FAIL -gt 0 ]; then
    echo "Some tests failed!"
    exit 1
else
    echo "All tests passed!"
    exit 0
fi
```

---

## Step 4: Integration Test (Rust)

Create file: `rust/tests/cli_integration.rs`

```rust
//! CLI integration tests
//! Run with: cargo test --features cli --test cli_integration

use std::process::Command;

fn run_cli(args: &[&str], stdin: Option<&str>) -> (bool, String, String) {
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--features", "cli", "--bin", "dsl_cli", "--"]);
    cmd.args(args);
    
    if let Some(input) = stdin {
        use std::io::Write;
        use std::process::Stdio;
        
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        
        let mut child = cmd.spawn().expect("Failed to spawn");
        child.stdin.as_mut().unwrap().write_all(input.as_bytes()).unwrap();
        let output = child.wait_with_output().expect("Failed to wait");
        
        return (
            output.status.success(),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        );
    }
    
    let output = cmd.output().expect("Failed to execute");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

#[test]
fn test_parse_simple() {
    let (success, stdout, _) = run_cli(
        &["parse"],
        Some("(cbu.create :name \"Test\")"),
    );
    assert!(success, "Parse should succeed");
    assert!(stdout.contains("Parsed") || stdout.contains("statements"));
}

#[test]
fn test_parse_json_output() {
    let (success, stdout, _) = run_cli(
        &["parse", "--format", "json"],
        Some("(cbu.create :name \"Test\")"),
    );
    assert!(success, "Parse should succeed");
    assert!(stdout.contains("{") && stdout.contains("}"), "Should be JSON");
}

#[test]
fn test_validate_success() {
    let (success, stdout, _) = run_cli(
        &["validate"],
        Some("(cbu.create :name \"Test\" :jurisdiction \"GB\" :as @cbu)"),
    );
    assert!(success, "Validation should succeed");
    assert!(stdout.contains("passed") || stdout.contains("✓"));
}

#[test]
fn test_validate_undefined_symbol() {
    let (success, _, stderr) = run_cli(
        &["validate"],
        Some("(document.catalog :entity-id @nonexistent :document-type \"PASSPORT\")"),
    );
    assert!(!success, "Validation should fail for undefined symbol");
    assert!(stderr.contains("undefined") || stderr.contains("error"));
}

#[test]
fn test_demo_individual() {
    let (success, stdout, _) = run_cli(
        &["demo", "onboard-individual", "--quiet"],
        None,
    );
    assert!(success, "Demo should succeed");
}

#[test]
fn test_demo_invalid() {
    let (success, stdout, _) = run_cli(
        &["demo", "invalid", "--quiet"],
        None,
    );
    // This demo tests that invalid DSL is correctly rejected
    assert!(success, "Demo 'invalid' should pass (by correctly failing validation)");
    assert!(stdout.contains("Correctly rejected") || stdout.contains("✓"));
}

#[test]
fn test_verbs_list() {
    let (success, stdout, _) = run_cli(&["verbs"], None);
    assert!(success, "Verbs command should succeed");
    assert!(stdout.contains("cbu") && stdout.contains("create"));
}

#[test]
fn test_examples() {
    let (success, stdout, _) = run_cli(&["examples"], None);
    assert!(success, "Examples command should succeed");
    assert!(stdout.contains("cbu.create"));
}
```

---

## Execution Checklist

### Phase 1: Create Files
- [ ] Update `Cargo.toml` with `[[bin]]` entry and features
- [ ] Create `rust/src/bin/dsl_cli.rs`
- [ ] Create `rust/tests/cli_self_test.sh`
- [ ] Create `rust/tests/cli_integration.rs`

### Phase 2: Build & Fix Compile Errors
- [ ] Run `cargo build --features cli --bin dsl_cli`
- [ ] Fix any missing imports or type errors
- [ ] Ensure all commands compile

### Phase 3: Run Self-Tests
- [ ] Make test script executable: `chmod +x rust/tests/cli_self_test.sh`
- [ ] Run: `bash rust/tests/cli_self_test.sh`
- [ ] Fix any failing tests

### Phase 4: Run Rust Integration Tests
- [ ] Run: `cargo test --features cli --test cli_integration`
- [ ] Fix any failing tests

### Phase 5: Manual Verification
Test each command manually:

```bash
# Parse
echo '(cbu.create :name "Test")' | cargo run --features cli --bin dsl_cli -- parse

# Validate
echo '(cbu.create :name "Test" :as @cbu)' | cargo run --features cli --bin dsl_cli -- validate

# Plan
echo '(cbu.create :name "Test" :as @cbu)' | cargo run --features cli --bin dsl_cli -- plan

# Demo
cargo run --features cli --bin dsl_cli -- demo onboard-individual

# Verbs
cargo run --features cli --bin dsl_cli -- verbs --domain cbu

# Examples
cargo run --features cli --bin dsl_cli -- examples onboarding
```

---

## Usage Examples for Claude Code

Once built, Claude Code can use the CLI like this:

```bash
# Generate and validate DSL
echo '
(cbu.create :name "New Client" :client-type "corporate" :jurisdiction "GB" :as @cbu)
(entity.create-limited-company :cbu-id @cbu :name "New Client Ltd" :as @company)
(document.catalog :cbu-id @cbu :entity-id @company :document-type "CERTIFICATE_OF_INCORPORATION")
' | dsl_cli validate --format json

# If validation fails, check the error and fix
# If validation passes, execute (when database feature enabled)
```

---

## Notes

1. **No database required for basic testing** - parse, validate (with local rules), plan, demo all work without database
2. **JSON output for machine parsing** - Use `--format json` for structured output Claude Code can parse
3. **Demo scenarios test the full pipeline** - Good for quick verification
4. **The `invalid` demo is intentionally broken** - Tests that CSG validation correctly rejects passport for company
