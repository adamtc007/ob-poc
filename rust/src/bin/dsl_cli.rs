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
//! # Generate DSL from intent JSON
//! echo '{"intent":"onboard_individual",...}' | dsl_cli intent
//!
//! # Run demo scenario
//! dsl_cli demo onboard-individual
//! ```

use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;
use tracing_subscriber::EnvFilter;

// Import from library
use ob_poc::dsl_v2::{
    compile, parse_program,
    validation::{ClientType, RustStyleFormatter, Severity, ValidationContext},
    verb_registry::{registry, VerbBehavior},
};

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
    #[arg(long, short = 'o', global = true, default_value = "pretty", value_enum)]
    format: OutputFormat,

    /// Suppress non-essential output
    #[arg(long, short, global = true)]
    quiet: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Json,
    Text,
    Pretty,
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

    /// Run a built-in demo scenario
    Demo {
        /// Scenario name: onboard-individual, onboard-corporate, add-document, invalid
        scenario: String,
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

    /// Execute DSL against the database
    #[cfg(feature = "database")]
    Execute {
        /// Input file (reads stdin if not provided)
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Database URL (or use DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL")]
        db_url: String,

        /// Dry run - show what would happen without executing
        #[arg(long)]
        dry_run: bool,

        /// Client type context: individual, corporate, fund, trust
        #[arg(long)]
        client_type: Option<String>,

        /// Jurisdiction context (ISO 2-letter code)
        #[arg(long)]
        jurisdiction: Option<String>,
    },

    /// Generate DSL from natural language using Claude AI
    #[cfg(feature = "database")]
    Generate {
        /// Natural language instruction (or reads from stdin if not provided)
        #[arg(short, long)]
        instruction: Option<String>,

        /// Execute the generated DSL after validation
        #[arg(long)]
        execute: bool,

        /// Database URL (required if --execute, or use DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL")]
        db_url: Option<String>,

        /// Domain hint to focus generation (e.g., cbu, entity, resource)
        #[arg(long)]
        domain: Option<String>,

        /// Save generated DSL to file
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Generate custody onboarding DSL from natural language (agentic workflow)
    #[cfg(feature = "database")]
    Custody {
        /// Natural language instruction (or reads from stdin if not provided)
        #[arg(short, long)]
        instruction: Option<String>,

        /// Execute the generated DSL after validation
        #[arg(long)]
        execute: bool,

        /// Database URL (required if --execute, or use DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL")]
        db_url: Option<String>,

        /// Show plan without generating DSL
        #[arg(long)]
        plan_only: bool,

        /// Save generated DSL to file
        #[arg(long, short)]
        output: Option<PathBuf>,
    },

    /// Generate CBU from predefined templates (hedge fund, SICAV, 40 Act, SPC)
    #[cfg(feature = "database")]
    Template {
        /// Template type: hedge_fund, lux_sicav, us_40_act, spc (or 'list' to show all)
        template_type: String,

        /// Fund name (required unless listing)
        #[arg(short, long)]
        name: Option<String>,

        /// Execute the generated DSL
        #[arg(long)]
        execute: bool,

        /// Database URL (required if --execute, or use DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL")]
        db_url: Option<String>,

        /// Include KYC case with workstreams and screenings
        #[arg(long, default_value = "true")]
        kyc: bool,

        /// Include share classes / investor registry
        #[arg(long, default_value = "true")]
        share_classes: bool,

        /// Include product/service provisioning
        #[arg(long, default_value = "true")]
        products: bool,

        /// Include custody setup (universe, SSI, booking rules)
        #[arg(long)]
        custody_setup: bool,

        /// Save generated DSL to file
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Interactive REPL for incremental DSL editing with a CBU
    #[cfg(feature = "database")]
    Repl {
        /// CBU ID to load existing state (optional - creates new session if not provided)
        #[arg(short, long)]
        cbu: Option<String>,

        /// Database URL (or use DATABASE_URL env var)
        #[arg(long, env = "DATABASE_URL")]
        db_url: String,
    },
}

// =============================================================================
// MAIN
// =============================================================================

fn main() -> ExitCode {
    // Initialize tracing subscriber (controlled by RUST_LOG env var)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(true)
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Parse { file } => cmd_parse(file, cli.format),
        Commands::Validate {
            file,
            client_type,
            jurisdiction,
        } => cmd_validate(file, client_type, jurisdiction, cli.format),
        Commands::Plan { file } => cmd_plan(file, cli.format),
        Commands::Demo { scenario } => cmd_demo(&scenario, cli.format, cli.quiet),
        Commands::Verbs { domain, verbose } => cmd_verbs(domain, verbose, cli.format),
        Commands::Examples { category } => cmd_examples(&category, cli.format),
        #[cfg(feature = "database")]
        Commands::Execute {
            file,
            db_url,
            dry_run,
            client_type,
            jurisdiction,
        } => {
            // Run async runtime for database operations
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| format!("Failed to create runtime: {}", e));
            match rt {
                Ok(rt) => rt.block_on(cmd_execute(
                    file,
                    db_url,
                    dry_run,
                    client_type,
                    jurisdiction,
                    cli.format,
                )),
                Err(e) => Err(e),
            }
        }
        #[cfg(feature = "database")]
        Commands::Generate {
            instruction,
            execute,
            db_url,
            domain,
            output,
        } => {
            // Run async runtime for API calls
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| format!("Failed to create runtime: {}", e));
            match rt {
                Ok(rt) => rt.block_on(cmd_generate(
                    instruction,
                    execute,
                    db_url,
                    domain,
                    output,
                    cli.format,
                )),
                Err(e) => Err(e),
            }
        }
        #[cfg(feature = "database")]
        Commands::Custody {
            instruction,
            execute,
            db_url,
            plan_only,
            output,
        } => {
            // Run async runtime for agentic workflow
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| format!("Failed to create runtime: {}", e));
            match rt {
                Ok(rt) => rt.block_on(cmd_custody(
                    instruction,
                    execute,
                    db_url,
                    plan_only,
                    output,
                    cli.format,
                )),
                Err(e) => Err(e),
            }
        }
        #[cfg(feature = "database")]
        Commands::Template {
            template_type,
            name,
            execute,
            db_url,
            kyc,
            share_classes,
            products,
            custody_setup,
            output,
        } => {
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| format!("Failed to create runtime: {}", e));
            match rt {
                Ok(rt) => rt.block_on(cmd_template(
                    &template_type,
                    name,
                    execute,
                    db_url,
                    kyc,
                    share_classes,
                    products,
                    custody_setup,
                    output,
                    cli.format,
                )),
                Err(e) => Err(e),
            }
        }
        #[cfg(feature = "database")]
        Commands::Repl { cbu, db_url } => {
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| format!("Failed to create runtime: {}", e));
            match rt {
                Ok(rt) => rt.block_on(cmd_repl(cbu, db_url, cli.format)),
                Err(e) => Err(e),
            }
        }
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

    let ast = parse_program(&source).map_err(|e| format!("Parse error: {:?}", e))?;

    match format {
        OutputFormat::Json => {
            let output = serde_json::json!({
                "success": true,
                "statement_count": ast.statements.len(),
                "statements": ast.statements.iter().map(|s| format!("{:?}", s)).collect::<Vec<_>>(),
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&output)
                    .map_err(|e| format!("JSON serialization failed: {}", e))?
            );
        }
        OutputFormat::Text | OutputFormat::Pretty => {
            println!(
                "{} Parsed {} statement(s)",
                "OK".green(),
                ast.statements.len()
            );
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
    let ast = parse_program(&source).map_err(|e| format!("Parse error: {:?}", e))?;

    // Build context
    let mut context = ValidationContext::default();
    if let Some(ct) = client_type {
        context.client_type = Some(parse_client_type(&ct)?);
    }
    if let Some(j) = jurisdiction {
        context.jurisdiction = Some(j);
    }

    // CSG Lint (without database, uses empty rules but still checks symbols)
    let source_clone = source.clone();

    #[cfg(feature = "database")]
    let lint_result = {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create runtime: {}", e))?;
        rt.block_on(async {
            let mut linter = ob_poc::dsl_v2::CsgLinter::new_without_db();
            let _ = linter.initialize().await;
            linter.lint(ast.clone(), &context, &source_clone).await
        })
    };

    #[cfg(not(feature = "database"))]
    let lint_result = futures::executor::block_on(async {
        let mut linter = ob_poc::dsl_v2::CsgLinter::new();
        let _ = linter.initialize().await;
        linter.lint(ast.clone(), &context, &source_clone).await
    });

    // Output results
    match format {
        OutputFormat::Json => {
            let output = serde_json::json!({
                "valid": !lint_result.has_errors(),
                "errors": lint_result.diagnostics.iter()
                    .filter(|d| d.severity == Severity::Error)
                    .count(),
                "warnings": lint_result.diagnostics.iter()
                    .filter(|d| d.severity == Severity::Warning)
                    .count(),
                "diagnostics": lint_result.diagnostics.iter().map(|d| {
                    serde_json::json!({
                        "severity": format!("{:?}", d.severity),
                        "code": d.code.as_str(),
                        "message": d.message,
                        "line": d.span.line,
                        "column": d.span.column,
                    })
                }).collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        OutputFormat::Text | OutputFormat::Pretty => {
            if lint_result.diagnostics.is_empty() {
                println!(
                    "{} Validation passed ({} statements)",
                    "OK".green().bold(),
                    ast.statements.len()
                );
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
    let ast = parse_program(&source).map_err(|e| format!("Parse error: {:?}", e))?;

    // Compile to execution plan
    let plan = compile(&ast).map_err(|e| format!("Compile error: {:?}", e))?;

    match format {
        OutputFormat::Json => {
            let output = serde_json::json!({
                "success": true,
                "step_count": plan.steps.len(),
                "steps": plan.steps.iter().enumerate().map(|(i, step)| {
                    serde_json::json!({
                        "index": i,
                        "verb": format!("{}.{}", step.verb_call.domain, step.verb_call.verb),
                        "args": format!("{:?}", step.verb_call.arguments),
                        "binding": step.bind_as,
                    })
                }).collect::<Vec<_>>(),
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&output)
                    .map_err(|e| format!("JSON serialization failed: {}", e))?
            );
        }
        OutputFormat::Text | OutputFormat::Pretty => {
            println!(
                "{} Compiled to {} step(s)",
                "OK".green().bold(),
                plan.steps.len()
            );
            println!();
            for (i, step) in plan.steps.iter().enumerate() {
                println!("{}:", format!("Step {}", i).cyan().bold());
                println!("  Verb: {}.{}", step.verb_call.domain, step.verb_call.verb);
                println!("  Args: {:?}", step.verb_call.arguments);
                if let Some(ref binding) = step.bind_as {
                    println!("  Binding: @{} -> ${}", binding, i);
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

fn cmd_demo(scenario: &str, format: OutputFormat, quiet: bool) -> Result<(), String> {
    let (name, dsl, expect_fail) = match scenario {
        "onboard-individual" | "individual" => (
            "Onboard Individual Client",
            r#"
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
            false,
        ),
        "onboard-corporate" | "corporate" => (
            "Onboard Corporate Client",
            r#"
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

(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :document-type "ARTICLES_OF_INCORPORATION")

(document.catalog
    :cbu-id @cbu
    :entity-id @ubo
    :document-type "PASSPORT")
"#,
            false,
        ),
        "add-document" | "document" => (
            "Catalog Document for Entity",
            r#"
(cbu.create :name "Test CBU" :as @cbu)
(entity.create-proper-person :cbu-id @cbu :name "John" :first-name "John" :last-name "Smith" :as @person)
(document.catalog
    :cbu-id @cbu
    :entity-id @person
    :document-type "PROOF_OF_ADDRESS"
    :as @poa)
"#,
            false,
        ),
        "invalid-passport-company" | "invalid" => (
            "Invalid: Passport for Company (Should Fail CSG)",
            r#"
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
            true,
        ),
        "custody-setup" | "custody" => (
            "Custody Settlement Setup",
            r#"
;; Set up custody for a fund: trading universe, SSI, and booking rules

(cbu.ensure
    :name "Demo Pension Fund"
    :jurisdiction "US"
    :client-type "FUND"
    :as @fund)

;; Layer 1: What they trade (Universe)
(cbu-custody.add-universe
    :cbu-id @fund
    :instrument-class "EQUITY"
    :market "XNYS"
    :currencies ["USD"]
    :settlement-types ["DVP"])

;; Layer 2: SSI Data (account info)
(cbu-custody.create-ssi
    :cbu-id @fund
    :name "US Primary Safekeeping"
    :type "SECURITIES"
    :safekeeping-account "DEMO-SAFE-001"
    :safekeeping-bic "BABOROCP"
    :cash-account "DEMO-USD-001"
    :cash-bic "BABOROCP"
    :cash-currency "USD"
    :pset-bic "DTCYUS33"
    :effective-date "2024-12-01"
    :as @ssi)

(cbu-custody.activate-ssi :ssi-id @ssi)

;; Layer 3: Booking Rules (routing)
(cbu-custody.add-booking-rule
    :cbu-id @fund
    :ssi-id @ssi
    :name "US Equity DVP"
    :priority 10
    :instrument-class "EQUITY"
    :market "XNYS"
    :currency "USD"
    :settlement-type "DVP")

;; Validate coverage
(cbu-custody.validate-booking-coverage :cbu-id @fund)
"#,
            false,
        ),
        _ => {
            return Err(format!(
                "Unknown scenario: '{}'\n\nAvailable scenarios:\n  \
                onboard-individual  - Individual client with passport\n  \
                onboard-corporate   - Corporate client with UBO structure\n  \
                add-document        - Add document to existing entity\n  \
                custody-setup       - Custody settlement with universe, SSI, booking rules\n  \
                invalid             - Invalid DSL (passport for company)",
                scenario
            ));
        }
    };

    if !quiet {
        println!("{}: {}", "Demo".cyan().bold(), name);
        println!("{}", "-".repeat(60));
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

    // Parse
    let ast = parse_program(dsl).map_err(|e| format!("Parse error: {:?}", e))?;

    if !quiet {
        println!(
            "{} Parsed {} statement(s)",
            "OK".green(),
            ast.statements.len()
        );
    }

    // Validate with CSG
    let context = ValidationContext::default();

    #[cfg(feature = "database")]
    let lint_result = {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create runtime: {}", e))?;
        rt.block_on(async {
            let mut linter = ob_poc::dsl_v2::CsgLinter::new_without_db();
            let _ = linter.initialize().await;
            linter.lint(ast.clone(), &context, dsl).await
        })
    };

    #[cfg(not(feature = "database"))]
    let lint_result = futures::executor::block_on(async {
        let mut linter = ob_poc::dsl_v2::CsgLinter::new();
        let _ = linter.initialize().await;
        linter.lint(ast.clone(), &context, dsl).await
    });

    let has_errors = lint_result.has_errors();

    if !quiet {
        if lint_result.diagnostics.is_empty() {
            println!("{} CSG validation passed", "OK".green());
        } else {
            println!();
            let formatted = RustStyleFormatter::format(dsl, &lint_result.diagnostics);
            println!("{}", formatted);
        }
    }

    // Handle expected failures
    if expect_fail {
        if has_errors {
            if !quiet {
                println!();
                println!(
                    "{} Correctly rejected invalid DSL (as expected)",
                    "OK".green().bold()
                );
            }
            return Ok(());
        } else {
            return Err("Expected validation to fail but it passed!".to_string());
        }
    }

    // For normal demos, errors are failures
    if has_errors {
        return Err("Validation failed".to_string());
    }

    // Compile to plan
    let plan = compile(&ast).map_err(|e| format!("Compile error: {:?}", e))?;

    if !quiet {
        println!(
            "{} Compiled to {} execution step(s)",
            "OK".green(),
            plan.steps.len()
        );
        println!();
        println!("{} Demo complete", "OK".green().bold());
    }

    Ok(())
}

fn cmd_verbs(domain: Option<String>, verbose: bool, format: OutputFormat) -> Result<(), String> {
    let reg = registry();

    let domains_to_show: Vec<&str> = match &domain {
        Some(d) => vec![d.as_str()],
        None => reg.domains().iter().map(|s| s.as_str()).collect(),
    };

    match format {
        OutputFormat::Json => {
            let mut output = serde_json::Map::new();
            for d in &domains_to_show {
                let verbs: Vec<_> = reg
                    .verbs_for_domain(d)
                    .iter()
                    .map(|v| {
                        serde_json::json!({
                            "verb": v.full_name(),
                            "description": v.description,
                            "behavior": format!("{:?}", v.behavior),
                            "args": v.args.iter().map(|a| {
                                serde_json::json!({
                                    "name": a.name,
                                    "type": a.arg_type,
                                    "required": a.required,
                                })
                            }).collect::<Vec<_>>(),
                        })
                    })
                    .collect();
                output.insert(d.to_string(), serde_json::Value::Array(verbs));
            }
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        OutputFormat::Text | OutputFormat::Pretty => {
            for d in &domains_to_show {
                println!("{}", format!("Domain: {}", d).cyan().bold());
                println!();

                for v in reg.verbs_for_domain(d) {
                    let behavior_tag = match v.behavior {
                        VerbBehavior::Crud => "[CRUD]".dimmed(),
                        VerbBehavior::CustomOp => "[CUSTOM]".yellow(),
                        VerbBehavior::Composite => "[COMPOSITE]".blue(),
                    };

                    println!(
                        "  {}.{} {}",
                        v.domain.green(),
                        v.verb.green().bold(),
                        behavior_tag
                    );
                    if verbose {
                        println!("    {}", v.description.dimmed());
                        let required: Vec<_> = v.args.iter().filter(|a| a.required).collect();
                        let optional: Vec<_> = v.args.iter().filter(|a| !a.required).collect();
                        if !required.is_empty() {
                            let names: Vec<_> = required.iter().map(|a| a.name.as_str()).collect();
                            println!("    Required: {:?}", names);
                        }
                        if !optional.is_empty() {
                            let names: Vec<_> = optional.iter().map(|a| a.name.as_str()).collect();
                            println!("    Optional: {:?}", names);
                        }
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
        (
            "onboarding",
            "Create CBU",
            "(cbu.create :name \"Client Name\" :client-type \"individual\" :jurisdiction \"GB\" :as @cbu)",
        ),
        (
            "onboarding",
            "Create Person",
            "(entity.create-proper-person :cbu-id @cbu :name \"John Doe\" :first-name \"John\" :last-name \"Doe\" :as @person)",
        ),
        (
            "onboarding",
            "Create Company",
            "(entity.create-limited-company :cbu-id @cbu :name \"Acme Ltd\" :company-number \"12345678\" :as @company)",
        ),
        (
            "documents",
            "Catalog Passport",
            "(document.catalog :cbu-id @cbu :entity-id @person :document-type \"PASSPORT\" :as @passport)",
        ),
        (
            "documents",
            "Catalog Certificate",
            "(document.catalog :cbu-id @cbu :entity-id @company :document-type \"CERTIFICATE_OF_INCORPORATION\")",
        ),
        (
            "entities",
            "Create Trust",
            "(entity.create-trust :cbu-id @cbu :name \"Family Trust\" :jurisdiction \"GB\" :as @trust)",
        ),
        (
            "entities",
            "Create Partnership",
            "(entity.create-partnership :cbu-id @cbu :name \"Smith & Co\" :as @partnership)",
        ),
        // Custody examples
        (
            "custody",
            "Add Trading Universe",
            "(cbu-custody.add-universe :cbu-id @cbu :instrument-class \"EQUITY\" :market \"XNYS\" :currencies [\"USD\"] :settlement-types [\"DVP\"])",
        ),
        (
            "custody",
            "Create SSI",
            "(cbu-custody.create-ssi :cbu-id @cbu :name \"US Safekeeping\" :type \"SECURITIES\" :safekeeping-account \"SAFE-001\" :safekeeping-bic \"BABOROCP\" :cash-account \"CASH-001\" :cash-bic \"BABOROCP\" :cash-currency \"USD\" :pset-bic \"DTCYUS33\" :effective-date \"2024-12-01\" :as @ssi)",
        ),
        (
            "custody",
            "Activate SSI",
            "(cbu-custody.activate-ssi :ssi-id @ssi)",
        ),
        (
            "custody",
            "Add Booking Rule",
            "(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi :name \"US Equity DVP\" :priority 10 :instrument-class \"EQUITY\" :market \"XNYS\" :currency \"USD\" :settlement-type \"DVP\")",
        ),
        (
            "custody",
            "Validate Booking Coverage",
            "(cbu-custody.validate-booking-coverage :cbu-id @cbu)",
        ),
        (
            "custody",
            "Lookup SSI for Trade",
            "(cbu-custody.lookup-ssi :cbu-id @cbu :instrument-class \"EQUITY\" :market \"XNYS\" :currency \"USD\" :settlement-type \"DVP\")",
        ),
    ];

    let filtered: Vec<_> = if category == "all" {
        examples
    } else {
        examples
            .into_iter()
            .filter(|(cat, _, _)| *cat == category)
            .collect()
    };

    if filtered.is_empty() {
        return Err(format!(
            "Unknown category: '{}'\n\nAvailable: onboarding, documents, entities, custody, all",
            category
        ));
    }

    match format {
        OutputFormat::Json => {
            let output: Vec<_> = filtered
                .iter()
                .map(|(cat, name, dsl)| {
                    serde_json::json!({
                        "category": cat,
                        "name": name,
                        "dsl": dsl,
                    })
                })
                .collect();
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
        Some(path) => std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read '{}': {}", path.display(), e)),
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
        _ => Err(format!(
            "Unknown client type: '{}'. Use: individual, corporate, fund, trust",
            s
        )),
    }
}

// =============================================================================
// DATABASE EXECUTE COMMAND
// =============================================================================

#[cfg(feature = "database")]
async fn cmd_execute(
    file: Option<PathBuf>,
    db_url: String,
    dry_run: bool,
    client_type: Option<String>,
    jurisdiction: Option<String>,
    format: OutputFormat,
) -> Result<(), String> {
    use ob_poc::dsl_v2::{
        executor::{DslExecutor, ExecutionContext, ExecutionResult},
        CsgLinter,
    };

    let source = read_input(file)?;

    // 1. Connect to database
    if format == OutputFormat::Pretty {
        println!("{}", "Connecting to database...".dimmed());
    }

    let pool = sqlx::PgPool::connect(&db_url)
        .await
        .map_err(|e| format!("Database connection failed: {}", e))?;

    // 2. Parse
    if format == OutputFormat::Pretty {
        println!("{}", "Parsing DSL...".dimmed());
    }

    let ast = parse_program(&source).map_err(|e| format!("Parse error: {:?}", e))?;

    if format == OutputFormat::Pretty {
        println!(
            "{} Parsed {} statement(s)",
            "✓".green(),
            ast.statements.len()
        );
    }

    // 3. Build validation context
    let mut context = ValidationContext::default();
    if let Some(ct) = client_type {
        context.client_type = Some(parse_client_type(&ct)?);
    }
    if let Some(j) = jurisdiction {
        context.jurisdiction = Some(j);
    }

    // 4. CSG Lint with database rules
    if format == OutputFormat::Pretty {
        println!("{}", "Running CSG validation...".dimmed());
    }

    let mut linter = CsgLinter::new(pool.clone());
    linter
        .initialize()
        .await
        .map_err(|e| format!("Linter initialization failed: {}", e))?;

    let lint_result = linter.lint(ast.clone(), &context, &source).await;

    if lint_result.has_errors() {
        let formatted = RustStyleFormatter::format(&source, &lint_result.diagnostics);
        if format == OutputFormat::Json {
            let output = serde_json::json!({
                "success": false,
                "stage": "validation",
                "diagnostics": lint_result.diagnostics.iter().map(|d| {
                    serde_json::json!({
                        "severity": format!("{:?}", d.severity),
                        "code": d.code.as_str(),
                        "message": d.message,
                        "line": d.span.line,
                        "column": d.span.column,
                    })
                }).collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            eprintln!("{}", formatted);
        }
        return Err("Validation failed".to_string());
    }

    if format == OutputFormat::Pretty && lint_result.has_warnings() {
        let formatted = RustStyleFormatter::format(&source, &lint_result.diagnostics);
        eprintln!("{}", formatted);
    }

    if format == OutputFormat::Pretty {
        println!("{} CSG validation passed", "✓".green());
    }

    // 5. Compile to execution plan
    if format == OutputFormat::Pretty {
        println!("{}", "Compiling execution plan...".dimmed());
    }

    let plan = compile(&ast).map_err(|e| format!("Compile error: {:?}", e))?;

    if format == OutputFormat::Pretty {
        println!("{} Compiled {} step(s)", "✓".green(), plan.steps.len());
    }

    // 6. Dry run - stop here
    if dry_run {
        if format == OutputFormat::Json {
            let output = serde_json::json!({
                "success": true,
                "dry_run": true,
                "steps": plan.steps.len(),
                "plan": plan.steps.iter().enumerate().map(|(i, s)| {
                    serde_json::json!({
                        "step": i,
                        "verb": format!("{}.{}", s.verb_call.domain, s.verb_call.verb),
                        "binding": s.bind_as,
                        "behavior": format!("{:?}", s.behavior),
                    })
                }).collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            println!();
            println!(
                "{} Dry run complete - {} step(s) would execute:",
                "✓".green().bold(),
                plan.steps.len()
            );
            println!();
            for (i, step) in plan.steps.iter().enumerate() {
                let binding = step
                    .bind_as
                    .as_ref()
                    .map(|b| format!(" → @{}", b))
                    .unwrap_or_default();
                let behavior_tag = match step.behavior {
                    VerbBehavior::Crud => "[CRUD]".dimmed(),
                    VerbBehavior::CustomOp => "[CUSTOM]".yellow(),
                    VerbBehavior::Composite => "[COMPOSITE]".blue(),
                };
                println!(
                    "  [{}] {}.{}{} {}",
                    i,
                    step.verb_call.domain.cyan(),
                    step.verb_call.verb.cyan().bold(),
                    binding.yellow(),
                    behavior_tag
                );
            }
        }
        return Ok(());
    }

    // 7. Execute for real
    if format == OutputFormat::Pretty {
        println!();
        println!("{}", "Executing...".yellow().bold());
        println!();
    }

    let executor = DslExecutor::new(pool);
    let mut exec_ctx = ExecutionContext::default();

    // Execute the entire plan
    match executor.execute_plan(&plan, &mut exec_ctx).await {
        Ok(results) => {
            // 8. Success output
            if format == OutputFormat::Json {
                let result_data: Vec<_> = results
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        let step = &plan.steps[i];
                        serde_json::json!({
                            "step": i,
                            "verb": format!("{}.{}", step.verb_call.domain, step.verb_call.verb),
                            "success": true,
                            "result": match r {
                                ExecutionResult::Uuid(id) => serde_json::json!({"id": id.to_string()}),
                                ExecutionResult::Record(j) => j.clone(),
                                ExecutionResult::RecordSet(rs) => serde_json::json!(rs),
                                ExecutionResult::Affected(n) => serde_json::json!({"rows_affected": n}),
                                ExecutionResult::Void => serde_json::json!(null),
                            },
                        })
                    })
                    .collect();

                let bindings: std::collections::HashMap<_, _> = exec_ctx
                    .symbols
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_string()))
                    .collect();

                let output = serde_json::json!({
                    "success": true,
                    "steps_executed": results.len(),
                    "bindings": bindings,
                    "results": result_data,
                });
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                // Pretty print step results
                for (i, result) in results.iter().enumerate() {
                    let step = &plan.steps[i];
                    let verb_name = format!("{}.{}", step.verb_call.domain, step.verb_call.verb);

                    match result {
                        ExecutionResult::Uuid(id) => {
                            let binding_info = step
                                .bind_as
                                .as_ref()
                                .map(|b| format!(" @{} =", b))
                                .unwrap_or_default();
                            println!(
                                "  [{}] {}{} {} {}",
                                i,
                                verb_name.cyan(),
                                binding_info.yellow(),
                                id.to_string().dimmed(),
                                "✓".green()
                            );
                        }
                        ExecutionResult::Affected(n) => {
                            println!(
                                "  [{}] {} ({} rows) {}",
                                i,
                                verb_name.cyan(),
                                n,
                                "✓".green()
                            );
                        }
                        _ => {
                            println!("  [{}] {} {}", i, verb_name.cyan(), "✓".green());
                        }
                    }
                }

                println!();
                println!(
                    "{} Executed {} step(s) successfully",
                    "✓".green().bold(),
                    results.len()
                );

                if !exec_ctx.symbols.is_empty() {
                    println!();
                    println!("Bindings created:");
                    for (name, value) in &exec_ctx.symbols {
                        println!("  @{} = {}", name.yellow(), value.to_string().dimmed());
                    }
                }
            }

            Ok(())
        }
        Err(e) => {
            if format == OutputFormat::Json {
                let output = serde_json::json!({
                    "success": false,
                    "stage": "execution",
                    "error": e.to_string(),
                });
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                eprintln!("{} {}", "✗".red(), e.to_string().red());
            }

            Err(format!("Execution failed: {}", e))
        }
    }
}

// =============================================================================
// GENERATE COMMAND (Agent-powered DSL generation)
// Uses the same full DSL lifecycle as the execute command:
// 1. Generate (Claude API)
// 2. Parse
// 3. CSG Lint (with database rules, clippy-style output)
// 4. Compile
// 5. Execute (optional)
// =============================================================================

#[cfg(feature = "database")]
async fn cmd_generate(
    instruction: Option<String>,
    execute: bool,
    db_url: Option<String>,
    domain: Option<String>,
    output: Option<PathBuf>,
    format: OutputFormat,
) -> Result<(), String> {
    use ob_poc::dsl_v2::{
        executor::{DslExecutor, ExecutionContext, ExecutionResult},
        CsgLinter,
    };

    // Get instruction from arg or stdin
    let prompt = match instruction {
        Some(i) => i,
        None => {
            if format == OutputFormat::Pretty {
                println!("{}", "Reading instruction from stdin...".dimmed());
            }
            let mut input = String::new();
            io::stdin()
                .read_to_string(&mut input)
                .map_err(|e| format!("Failed to read stdin: {}", e))?;
            input.trim().to_string()
        }
    };

    if prompt.is_empty() {
        return Err("No instruction provided. Use --instruction or pipe to stdin.".to_string());
    }

    // Database URL is required for full validation pipeline
    let db_url = db_url
        .ok_or("--db-url or DATABASE_URL required for generate (needed for CSG validation)")?;

    // Check for API key
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| "ANTHROPIC_API_KEY environment variable not set".to_string())?;

    // =========================================================================
    // STEP 1: GENERATE DSL (Claude API)
    // =========================================================================
    if format == OutputFormat::Pretty {
        println!("{}", "Generating DSL from instruction...".dimmed());
        println!("  {}", prompt.cyan());
        println!();
    }

    // Build vocabulary for the prompt
    let vocab = build_vocab_for_generate(domain.as_deref());

    // Build system prompt
    let system_prompt = build_generation_system_prompt(&vocab);

    // Call Claude API
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 2048,
            "system": system_prompt,
            "messages": [
                {"role": "user", "content": prompt}
            ]
        }))
        .send()
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API error {}: {}", status, body));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse API response: {}", e))?;

    let generated = json["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();

    if generated.starts_with("ERROR:") {
        return Err(generated);
    }

    // Extract DSL from response (handle markdown code blocks)
    let dsl = extract_dsl_from_response(&generated);

    if format == OutputFormat::Pretty {
        println!("{} Generated DSL:", "✓".green());
        println!();
        for line in dsl.lines() {
            println!("  {}", line.cyan());
        }
        println!();
    }

    // =========================================================================
    // STEP 2: CONNECT TO DATABASE
    // =========================================================================
    if format == OutputFormat::Pretty {
        println!("{}", "Connecting to database...".dimmed());
    }

    let pool = sqlx::PgPool::connect(&db_url)
        .await
        .map_err(|e| format!("Database connection failed: {}", e))?;

    // =========================================================================
    // STEP 3: PARSE DSL
    // =========================================================================
    if format == OutputFormat::Pretty {
        println!("{}", "Parsing DSL...".dimmed());
    }

    let ast = match parse_program(&dsl) {
        Ok(ast) => {
            if format == OutputFormat::Pretty {
                println!(
                    "{} Parsed {} statement(s)",
                    "✓".green(),
                    ast.statements.len()
                );
            }
            ast
        }
        Err(e) => {
            if format == OutputFormat::Json {
                let output = serde_json::json!({
                    "success": false,
                    "stage": "parse",
                    "dsl": dsl,
                    "error": format!("Parse error: {}", e),
                });
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                eprintln!("{}: Parse error: {}", "error".red().bold(), e);
            }
            return Err(format!("Parse error: {}", e));
        }
    };

    // =========================================================================
    // STEP 4: CSG LINT (with database rules, clippy-style output)
    // =========================================================================
    if format == OutputFormat::Pretty {
        println!("{}", "Running CSG validation...".dimmed());
    }

    let mut linter = CsgLinter::new(pool.clone());
    linter
        .initialize()
        .await
        .map_err(|e| format!("Linter initialization failed: {}", e))?;

    let context = ValidationContext::default();
    let lint_result = linter.lint(ast.clone(), &context, &dsl).await;

    if lint_result.has_errors() {
        let formatted = RustStyleFormatter::format(&dsl, &lint_result.diagnostics);
        if format == OutputFormat::Json {
            let output = serde_json::json!({
                "success": false,
                "stage": "validation",
                "dsl": dsl,
                "diagnostics": lint_result.diagnostics.iter().map(|d| {
                    serde_json::json!({
                        "severity": format!("{:?}", d.severity),
                        "code": d.code.as_str(),
                        "message": d.message,
                        "line": d.span.line,
                        "column": d.span.column,
                    })
                }).collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            eprintln!("{}", formatted);
        }
        return Err("CSG validation failed".to_string());
    }

    // Show warnings if any
    if format == OutputFormat::Pretty && lint_result.has_warnings() {
        let formatted = RustStyleFormatter::format(&dsl, &lint_result.diagnostics);
        eprintln!("{}", formatted);
    }

    if format == OutputFormat::Pretty {
        println!("{} CSG validation passed", "✓".green());
    }

    // =========================================================================
    // STEP 5: COMPILE TO EXECUTION PLAN
    // =========================================================================
    if format == OutputFormat::Pretty {
        println!("{}", "Compiling execution plan...".dimmed());
    }

    let plan = match compile(&ast) {
        Ok(plan) => {
            if format == OutputFormat::Pretty {
                println!("{} Compiled {} step(s)", "✓".green(), plan.steps.len());
            }
            plan
        }
        Err(e) => {
            if format == OutputFormat::Json {
                let output = serde_json::json!({
                    "success": false,
                    "stage": "compile",
                    "dsl": dsl,
                    "error": format!("Compile error: {:?}", e),
                });
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                eprintln!("{}: Compile error: {:?}", "error".red().bold(), e);
            }
            return Err(format!("Compile error: {:?}", e));
        }
    };

    // =========================================================================
    // SAVE TO FILE (if requested)
    // =========================================================================
    if let Some(output_path) = &output {
        std::fs::write(output_path, &dsl)
            .map_err(|e| format!("Failed to write output file: {}", e))?;
        if format == OutputFormat::Pretty {
            println!("{} Saved to {}", "✓".green(), output_path.display());
        }
    }

    // =========================================================================
    // STEP 6: EXECUTE (optional)
    // =========================================================================
    if execute {
        if format == OutputFormat::Pretty {
            println!();
            println!("{}", "Executing...".yellow().bold());
            println!();
        }

        let executor = DslExecutor::new(pool);
        let mut exec_ctx = ExecutionContext::default();

        match executor.execute_plan(&plan, &mut exec_ctx).await {
            Ok(results) => {
                if format == OutputFormat::Json {
                    let result_data: Vec<_> = results
                        .iter()
                        .enumerate()
                        .map(|(i, r)| {
                            let step = &plan.steps[i];
                            serde_json::json!({
                                "step": i,
                                "verb": format!("{}.{}", step.verb_call.domain, step.verb_call.verb),
                                "success": true,
                                "result": match r {
                                    ExecutionResult::Uuid(id) => serde_json::json!({"id": id.to_string()}),
                                    ExecutionResult::Record(j) => j.clone(),
                                    ExecutionResult::RecordSet(rs) => serde_json::json!(rs),
                                    ExecutionResult::Affected(n) => serde_json::json!({"rows_affected": n}),
                                    ExecutionResult::Void => serde_json::json!(null),
                                },
                            })
                        })
                        .collect();

                    let bindings: std::collections::HashMap<_, _> = exec_ctx
                        .symbols
                        .iter()
                        .map(|(k, v)| (k.clone(), v.to_string()))
                        .collect();

                    let output = serde_json::json!({
                        "success": true,
                        "dsl": dsl,
                        "steps_executed": results.len(),
                        "bindings": bindings,
                        "results": result_data,
                    });
                    println!("{}", serde_json::to_string_pretty(&output).unwrap());
                } else {
                    // Pretty print step results
                    for (i, result) in results.iter().enumerate() {
                        let step = &plan.steps[i];
                        let verb_name =
                            format!("{}.{}", step.verb_call.domain, step.verb_call.verb);

                        match result {
                            ExecutionResult::Uuid(id) => {
                                let binding_info = step
                                    .bind_as
                                    .as_ref()
                                    .map(|b| format!(" @{} =", b))
                                    .unwrap_or_default();
                                println!(
                                    "  [{}] {}{} {} {}",
                                    i,
                                    verb_name.cyan(),
                                    binding_info.yellow(),
                                    id.to_string().dimmed(),
                                    "✓".green()
                                );
                            }
                            ExecutionResult::Affected(n) => {
                                println!(
                                    "  [{}] {} ({} rows) {}",
                                    i,
                                    verb_name.cyan(),
                                    n,
                                    "✓".green()
                                );
                            }
                            _ => {
                                println!("  [{}] {} {}", i, verb_name.cyan(), "✓".green());
                            }
                        }
                    }

                    println!();
                    println!(
                        "{} Executed {} step(s) successfully",
                        "✓".green().bold(),
                        results.len()
                    );

                    if !exec_ctx.symbols.is_empty() {
                        println!();
                        println!("Bindings created:");
                        for (name, value) in &exec_ctx.symbols {
                            println!("  @{} = {}", name.yellow(), value.to_string().dimmed());
                        }
                    }
                }
            }
            Err(e) => {
                if format == OutputFormat::Json {
                    let output = serde_json::json!({
                        "success": false,
                        "stage": "execution",
                        "dsl": dsl,
                        "error": e.to_string(),
                    });
                    println!("{}", serde_json::to_string_pretty(&output).unwrap());
                } else {
                    eprintln!("{} {}", "✗".red(), e.to_string().red());
                }
                return Err(format!("Execution failed: {}", e));
            }
        }
    } else {
        // Output DSL and validation results without execution
        if format == OutputFormat::Json {
            let output = serde_json::json!({
                "success": true,
                "dsl": dsl,
                "statement_count": ast.statements.len(),
                "step_count": plan.steps.len(),
                "validated": true,
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else if format == OutputFormat::Pretty {
            println!();
            println!(
                "{} DSL generated and validated successfully",
                "✓".green().bold()
            );
            println!("  Use --execute to run against the database");
        }
    }

    Ok(())
}

/// Build vocabulary string for generation prompt
#[cfg(feature = "database")]
fn build_vocab_for_generate(domain_filter: Option<&str>) -> String {
    let mut lines = Vec::new();
    let reg = registry();

    let domains: Vec<String> = if let Some(d) = domain_filter {
        vec![d.to_string()]
    } else {
        reg.domains().to_vec()
    };

    for domain_name in domains {
        for verb in reg.verbs_for_domain(&domain_name) {
            let required = verb.required_arg_names().join(", ");
            let optional = verb.optional_arg_names().join(", ");
            lines.push(format!(
                "{}.{}: {} [required: {}] [optional: {}]",
                verb.domain, verb.verb, verb.description, required, optional
            ));
        }
    }

    lines.join("\n")
}

/// Extract DSL from response, handling markdown code blocks
#[cfg(feature = "database")]
fn extract_dsl_from_response(response: &str) -> String {
    // Check for markdown code block
    if response.contains("```") {
        // Extract content between ``` markers
        let parts: Vec<&str> = response.split("```").collect();
        if parts.len() >= 2 {
            let code = parts[1];
            // Remove language identifier if present (e.g., "clojure\n" or "dsl\n")
            let code = code.trim();
            if let Some(idx) = code.find('\n') {
                let first_line = &code[..idx];
                // If first line looks like a language identifier, skip it
                if !first_line.contains('(') && !first_line.contains(':') {
                    return code[idx + 1..].trim().to_string();
                }
            }
            return code.to_string();
        }
    }
    response.to_string()
}

// =============================================================================
// CUSTODY COMMAND (Agentic workflow)
// Uses the agentic module for pattern-based custody DSL generation:
// 1. Extract intent from natural language
// 2. Classify pattern (SimpleEquity, MultiMarket, WithOtc)
// 3. Derive requirements (deterministic Rust code)
// 4. Generate DSL (Claude with full schemas)
// 5. Validate with retry loop
// 6. Execute (optional)
// =============================================================================

#[cfg(feature = "database")]
async fn cmd_custody(
    instruction: Option<String>,
    execute: bool,
    db_url: Option<String>,
    plan_only: bool,
    output: Option<PathBuf>,
    format: OutputFormat,
) -> Result<(), String> {
    use ob_poc::agentic::{AgentOrchestrator, RequirementPlanner};

    // Get instruction from arg or stdin
    let prompt = match instruction {
        Some(i) => i,
        None => {
            if format == OutputFormat::Pretty {
                println!("{}", "Reading instruction from stdin...".dimmed());
            }
            let mut input = String::new();
            io::stdin()
                .read_to_string(&mut input)
                .map_err(|e| format!("Failed to read stdin: {}", e))?;
            input.trim().to_string()
        }
    };

    if prompt.is_empty() {
        return Err("No instruction provided. Use --instruction or pipe to stdin.".to_string());
    }

    // Check for API key
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| "ANTHROPIC_API_KEY environment variable not set".to_string())?;

    if format == OutputFormat::Pretty {
        println!(
            "{}",
            "Custody Onboarding DSL Generator (Agentic)".cyan().bold()
        );
        println!("{}", "=".repeat(50));
        println!();
        println!("{}: {}", "Instruction".yellow(), prompt);
        println!();
    }

    // =========================================================================
    // STEP 1: CREATE ORCHESTRATOR
    // =========================================================================
    let orchestrator = if execute {
        let url = db_url.ok_or("--db-url or DATABASE_URL required for --execute".to_string())?;

        if format == OutputFormat::Pretty {
            println!("{}", "Connecting to database...".dimmed());
        }

        let pool = sqlx::PgPool::connect(&url)
            .await
            .map_err(|e| format!("Database connection failed: {}", e))?;

        ob_poc::agentic::OrchestratorBuilder::new(api_key)
            .with_pool(pool)
            .build()
            .map_err(|e| format!("Failed to create orchestrator: {}", e))?
    } else {
        AgentOrchestrator::new(api_key)
            .map_err(|e| format!("Failed to create orchestrator: {}", e))?
    };

    // =========================================================================
    // STEP 2: EXTRACT INTENT
    // =========================================================================
    if format == OutputFormat::Pretty {
        println!("{}", "Extracting intent from natural language...".dimmed());
    }

    let intent = orchestrator
        .extract_intent(&prompt)
        .await
        .map_err(|e| format!("Intent extraction failed: {}", e))?;

    if format == OutputFormat::Pretty {
        println!("{} Extracted intent:", "✓".green());
        println!(
            "  Client: {} ({:?})",
            intent.client.name.cyan(),
            intent.client.entity_type
        );
        println!(
            "  Markets: {:?}",
            intent
                .markets
                .iter()
                .map(|m| &m.market_code)
                .collect::<Vec<_>>()
        );
        println!(
            "  Instruments: {:?}",
            intent
                .instruments
                .iter()
                .map(|i| &i.class)
                .collect::<Vec<_>>()
        );
        if !intent.otc_counterparties.is_empty() {
            println!(
                "  OTC Counterparties: {:?}",
                intent
                    .otc_counterparties
                    .iter()
                    .map(|c| &c.name)
                    .collect::<Vec<_>>()
            );
        }
        println!();
    }

    // =========================================================================
    // STEP 3: CLASSIFY PATTERN AND PLAN
    // =========================================================================
    let plan = RequirementPlanner::plan(&intent);

    if format == OutputFormat::Pretty {
        println!(
            "{} Pattern: {} - {}",
            "✓".green(),
            plan.pattern.name().cyan().bold(),
            plan.pattern.description()
        );
        println!("  Required domains: {:?}", plan.pattern.required_domains());
        println!("  Universe entries: {}", plan.universe.len());
        println!("  SSIs required: {}", plan.ssis.len());
        println!("  Booking rules: {}", plan.booking_rules.len());
        if !plan.isdas.is_empty() {
            println!("  ISDA agreements: {}", plan.isdas.len());
        }
        println!();
    }

    // If plan_only, output the plan and stop
    if plan_only {
        if format == OutputFormat::Json {
            let output = serde_json::json!({
                "success": true,
                "intent": intent,
                "pattern": plan.pattern.name(),
                "plan": plan,
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            println!("{}", "Detailed Plan:".yellow().bold());
            println!();

            println!("  {}:", "CBU".cyan());
            println!("    Name: {}", plan.cbu.name);
            println!("    Jurisdiction: {}", plan.cbu.jurisdiction);
            println!("    Type: {}", plan.cbu.client_type);
            println!();

            if !plan.entities.is_empty() {
                println!("  {}:", "Entities".cyan());
                for e in &plan.entities {
                    println!("    {} ({:?}) → @{}", e.name, e.action, e.variable);
                }
                println!();
            }

            println!("  {}:", "Universe".cyan());
            for u in &plan.universe {
                let market = u.market.as_deref().unwrap_or("OTC");
                println!(
                    "    {} in {} ({:?})",
                    u.instrument_class, market, u.currencies
                );
            }
            println!();

            println!("  {}:", "SSIs".cyan());
            for s in &plan.ssis {
                println!(
                    "    {} ({}, {}) → @{}",
                    s.name, s.ssi_type, s.currency, s.variable
                );
            }
            println!();

            println!("  {}:", "Booking Rules".cyan());
            for r in &plan.booking_rules {
                println!("    [{}] {} → @{}", r.priority, r.name, r.ssi_variable);
            }

            if !plan.isdas.is_empty() {
                println!();
                println!("  {}:", "ISDA Agreements".cyan());
                for isda in &plan.isdas {
                    println!(
                        "    {} ({} law) → @{}",
                        isda.counterparty_name, isda.governing_law, isda.variable
                    );
                    if let Some(csa) = &isda.csa {
                        println!("      CSA: {} → @{}", csa.csa_type, csa.variable);
                    }
                }
            }
        }
        return Ok(());
    }

    // =========================================================================
    // STEP 4-5: GENERATE AND VALIDATE DSL
    // =========================================================================
    if format == OutputFormat::Pretty {
        println!("{}", "Generating DSL with validation...".dimmed());
    }

    let result = orchestrator
        .generate(&prompt, execute)
        .await
        .map_err(|e| format!("Generation failed: {}", e))?;

    if format == OutputFormat::Pretty {
        println!(
            "{} Generated valid DSL in {} attempt(s)",
            "✓".green(),
            result.dsl.attempts
        );
        println!();
        println!("{}:", "Generated DSL".yellow().bold());
        println!();
        for line in result.dsl.source.lines() {
            if line.trim().starts_with(';') {
                println!("  {}", line.dimmed());
            } else if !line.trim().is_empty() {
                println!("  {}", line.cyan());
            } else {
                println!();
            }
        }
        println!();
    }

    // =========================================================================
    // SAVE TO FILE (if requested)
    // =========================================================================
    if let Some(output_path) = &output {
        std::fs::write(output_path, &result.dsl.source)
            .map_err(|e| format!("Failed to write output file: {}", e))?;
        if format == OutputFormat::Pretty {
            println!("{} Saved to {}", "✓".green(), output_path.display());
        }
    }

    // =========================================================================
    // STEP 6: EXECUTION RESULTS
    // =========================================================================
    if let Some(exec_result) = &result.execution {
        if format == OutputFormat::Pretty {
            if exec_result.success {
                println!("{} Execution successful", "✓".green().bold());
                println!();
                println!("Bindings created:");
                for binding in &exec_result.bindings {
                    println!(
                        "  @{} = {}",
                        binding.variable.yellow(),
                        binding.uuid.dimmed()
                    );
                }
            } else {
                println!(
                    "{} Execution failed: {}",
                    "✗".red(),
                    exec_result.error.as_deref().unwrap_or("Unknown error")
                );
            }
        }
    }

    // Final JSON output
    if format == OutputFormat::Json {
        let output = serde_json::json!({
            "success": true,
            "intent": result.intent,
            "pattern": result.plan.pattern.name(),
            "dsl": result.dsl.source,
            "attempts": result.dsl.attempts,
            "execution": result.execution,
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else if format == OutputFormat::Pretty && !execute {
        println!();
        println!(
            "{} DSL generated and validated successfully",
            "✓".green().bold()
        );
        println!("  Use --execute to run against the database");
    }

    Ok(())
}

// =============================================================================
// TEMPLATE COMMAND - Generate CBU from predefined templates
// =============================================================================

#[cfg(feature = "database")]
async fn cmd_template(
    template_type: &str,
    name: Option<String>,
    execute: bool,
    db_url: Option<String>,
    include_kyc: bool,
    include_share_classes: bool,
    include_products: bool,
    include_custody_setup: bool,
    output: Option<PathBuf>,
    format: OutputFormat,
) -> Result<(), String> {
    use ob_poc::dsl_v2::{
        executor::{DslExecutor, ExecutionContext, ExecutionResult},
        CsgLinter,
    };
    use ob_poc::templates::{generate_template, TemplateParams, TemplateType};

    // Handle 'list' command
    if template_type == "list" || template_type == "help" {
        if format == OutputFormat::Json {
            let templates: Vec<_> = TemplateType::all()
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": t.name(),
                        "description": t.description(),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&templates).unwrap());
        } else {
            println!("{}", "Available CBU Templates:".cyan().bold());
            println!();
            for t in TemplateType::all() {
                println!("  {} - {}", t.name().green().bold(), t.description());
            }
            println!();
            println!("Usage: dsl_cli template <type> --name \"Fund Name\" [--execute]");
            println!();
            println!("Options:");
            println!("  --kyc             Include KYC case (default: true)");
            println!("  --share-classes   Include share classes (default: true)");
            println!("  --products        Include product provisioning (default: true)");
            println!("  --custody-setup   Include custody universe/SSI/rules");
        }
        return Ok(());
    }

    // Parse template type
    let template = TemplateType::from_str(template_type)
        .map_err(|_| format!(
            "Unknown template type: '{}'\n\nAvailable types: hedge_fund, lux_sicav, us_40_act, spc\nUse 'dsl_cli template list' to see descriptions.",
            template_type
        ))?;

    // Name is required
    let fund_name =
        name.ok_or_else(|| "Fund name is required. Use --name \"Your Fund Name\"".to_string())?;

    // Build params
    let params = TemplateParams {
        fund_name: fund_name.clone(),
        jurisdiction: None, // Use template defaults
        ubos: vec![],       // Use template defaults
        include_kyc,
        include_share_classes,
        include_products,
        include_custody_setup,
    };

    // Generate DSL
    if format == OutputFormat::Pretty {
        println!(
            "{} Generating {} template for \"{}\"...",
            "→".cyan(),
            template.name(),
            fund_name
        );
    }

    let dsl = generate_template(template, &params);

    // Save to file if requested
    if let Some(ref path) = output {
        std::fs::write(path, &dsl).map_err(|e| format!("Failed to write output file: {}", e))?;
        if format == OutputFormat::Pretty {
            println!("{} Saved DSL to {}", "✓".green(), path.display());
        }
    }

    // If not executing, just show the DSL
    if !execute {
        if format == OutputFormat::Json {
            let output = serde_json::json!({
                "template": template.name(),
                "fund_name": fund_name,
                "dsl": dsl,
                "line_count": dsl.lines().count(),
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            println!();
            println!("{}", "Generated DSL:".yellow().bold());
            println!("{}", "-".repeat(60));
            for line in dsl.lines() {
                if line.trim().starts_with(";;") {
                    println!("{}", line.dimmed());
                } else if !line.trim().is_empty() {
                    println!("{}", line);
                } else {
                    println!();
                }
            }
            println!("{}", "-".repeat(60));
            println!();
            println!("{} {} lines generated", "✓".green(), dsl.lines().count());
            println!("  Use --execute to run against the database");
        }
        return Ok(());
    }

    // =========================================================================
    // EXECUTE THE TEMPLATE
    // =========================================================================

    let db_url = db_url.ok_or("--db-url or DATABASE_URL required for execution")?;

    if format == OutputFormat::Pretty {
        println!("{}", "Connecting to database...".dimmed());
    }

    let pool = sqlx::PgPool::connect(&db_url)
        .await
        .map_err(|e| format!("Database connection failed: {}", e))?;

    // Parse
    if format == OutputFormat::Pretty {
        println!("{}", "Parsing DSL...".dimmed());
    }

    let ast = parse_program(&dsl).map_err(|e| format!("Parse error: {:?}", e))?;

    if format == OutputFormat::Pretty {
        println!("{} Parsed {} statements", "✓".green(), ast.statements.len());
    }

    // Validate
    if format == OutputFormat::Pretty {
        println!("{}", "Validating...".dimmed());
    }

    let mut linter = CsgLinter::new(pool.clone());
    linter
        .initialize()
        .await
        .map_err(|e| format!("Linter init failed: {}", e))?;

    let context = ValidationContext::default();
    let lint_result = linter.lint(ast.clone(), &context, &dsl).await;

    if lint_result.has_errors() {
        let formatted = RustStyleFormatter::format(&dsl, &lint_result.diagnostics);
        eprintln!("{}", formatted);
        return Err("Validation failed".to_string());
    }

    if format == OutputFormat::Pretty {
        println!("{} Validation passed", "✓".green());
    }

    // Compile
    let plan = compile(&ast).map_err(|e| format!("Compile error: {:?}", e))?;

    if format == OutputFormat::Pretty {
        println!("{} Compiled {} steps", "✓".green(), plan.steps.len());
        println!();
        println!("{}", "Executing...".yellow().bold());
        println!();
    }

    // Execute
    let executor = DslExecutor::new(pool);
    let mut exec_ctx = ExecutionContext::default();

    match executor.execute_plan(&plan, &mut exec_ctx).await {
        Ok(results) => {
            if format == OutputFormat::Json {
                let bindings: std::collections::HashMap<_, _> = exec_ctx
                    .symbols
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_string()))
                    .collect();

                let output = serde_json::json!({
                    "success": true,
                    "template": template.name(),
                    "fund_name": fund_name,
                    "steps_executed": results.len(),
                    "bindings": bindings,
                });
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                // Pretty print key results
                for (i, result) in results.iter().enumerate() {
                    let step = &plan.steps[i];
                    let verb_name = format!("{}.{}", step.verb_call.domain, step.verb_call.verb);

                    match result {
                        ExecutionResult::Uuid(id) => {
                            let binding_info = step
                                .bind_as
                                .as_ref()
                                .map(|b| format!(" @{} =", b))
                                .unwrap_or_default();
                            println!(
                                "  [{}] {}{} {}",
                                i,
                                verb_name.cyan(),
                                binding_info.yellow(),
                                id.to_string().dimmed()
                            );
                        }
                        ExecutionResult::Affected(n) => {
                            println!("  [{}] {} ({} rows)", i, verb_name.cyan(), n);
                        }
                        _ => {
                            println!("  [{}] {} {}", i, verb_name.cyan(), "✓".green());
                        }
                    }
                }

                println!();
                println!(
                    "{} Template \"{}\" executed successfully ({} steps)",
                    "✓".green().bold(),
                    fund_name,
                    results.len()
                );

                // Show key bindings
                let key_bindings: Vec<_> = exec_ctx
                    .symbols
                    .iter()
                    .filter(|(k, _)| {
                        k.contains("fund")
                            || k.contains("cbu")
                            || k.contains("case")
                            || k.contains("master")
                    })
                    .collect();

                if !key_bindings.is_empty() {
                    println!();
                    println!("Key entities created:");
                    for (name, value) in key_bindings {
                        println!("  @{} = {}", name.yellow(), value.to_string().dimmed());
                    }
                }
            }
            Ok(())
        }
        Err(e) => {
            if format == OutputFormat::Json {
                let output = serde_json::json!({
                    "success": false,
                    "error": e.to_string(),
                });
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            }
            Err(format!("Execution failed: {}", e))
        }
    }
}

/// Build system prompt for DSL generation
#[cfg(feature = "database")]
fn build_generation_system_prompt(vocab: &str) -> String {
    format!(
        r#"You are a DSL generator for a KYC/AML onboarding system.
Generate valid DSL S-expressions from natural language instructions.

AVAILABLE VERBS:
{}

DSL SYNTAX:
- Format: (domain.verb :key "value" :key2 value2)
- Strings must be quoted: "text"
- Numbers are unquoted: 42, 25.5
- References start with @: @symbol-name (hyphens allowed in symbol names)
- Use :as @name to capture results for later reference

## ONBOARDING DSL GENERATION

You can generate DSL to onboard clients to financial services products. The taxonomy is:

**Product** → **Service** → **Resource Instance**

### Available Products

| Code | Name | Description |
|------|------|-------------|
| `GLOB_CUSTODY` | Global Custody | Asset safekeeping, settlement, corporate actions |
| `FUND_ACCT` | Fund Accounting | NAV calculation, investor accounting, reporting |
| `MO_IBOR` | Middle Office IBOR | Position management, trade capture, P&L attribution |

### Service Resource Types

Service resources represent platforms/applications that deliver services:
- `service-resource.ensure` - Create/ensure a service resource type exists
- `service-resource.provision` - Provision an instance of a service resource for a CBU
- `service-resource.set-attr` - Set attributes on a provisioned instance
- `service-resource.activate` - Activate a provisioned instance
- `service-resource.suspend` - Suspend an active instance
- `service-resource.decommission` - Decommission an instance

### Entity Types

Dynamic verbs for entity creation based on type:
- `entity.create-proper-person` - Create a natural person
- `entity.create-limited-company` - Create a limited company
- `entity.create-trust-discretionary` - Create a discretionary trust
- `entity.create-partnership-limited` - Create a limited partnership

### Client Types
- `fund` - Investment fund
- `corporate` - Corporate client
- `individual` - Individual client
- `trust` - Trust structure

### Common Jurisdictions
- `US` - United States
- `UK` - United Kingdom
- `LU` - Luxembourg
- `IE` - Ireland
- `JE` - Jersey
- `KY` - Cayman Islands

### EXAMPLES

**Simple CBU Creation:**
```
(cbu.ensure :name "Acme Fund" :jurisdiction "LU" :client-type "fund" :as @fund)
```

**Corporate with UBO:**
```
(cbu.ensure :name "Acme Holdings" :jurisdiction "GB" :client-type "corporate" :as @cbu)
(entity.create-limited-company :name "Acme Holdings Ltd" :jurisdiction "GB" :as @company)
(entity.create-proper-person :first-name "John" :last-name "Smith" :nationality "GB" :as @john)
(cbu.assign-role :cbu-id @cbu :entity-id @company :role "PRINCIPAL")
(cbu.assign-role :cbu-id @cbu :entity-id @john :role "BENEFICIAL_OWNER" :target-entity-id @company :ownership-percentage 100)
```

**Service Resource Provisioning:**
```
(cbu.ensure :name "Pacific Fund" :jurisdiction "US" :client-type "fund" :as @fund)
(service-resource.provision :cbu-id @fund :resource-type "CUSTODY_ACCT" :instance-name "Pacific Custody Account" :as @custody)
(service-resource.set-attr :instance-id @custody :attr "account_number" :value "CUST-001")
(service-resource.set-attr :instance-id @custody :attr "base_currency" :value "USD")
(service-resource.activate :instance-id @custody)
```

**Document and Screening:**
```
(document.catalog :cbu-id @cbu :entity-id @john :document-type "PASSPORT")
(screening.pep :entity-id @john)
(screening.sanctions :entity-id @company)
```

## CUSTODY & SETTLEMENT DSL

The custody domain supports a three-layer model for settlement instruction routing:
- **Layer 1 (Universe)**: What instruments a CBU trades (markets, asset classes)
- **Layer 2 (SSI Data)**: Account information for securities and cash
- **Layer 3 (Booking Rules)**: ALERT-style routing rules to match trades to SSIs

### Custody Verbs

| Verb | Description |
|------|-------------|
| `cbu-custody.add-universe` | Define what a CBU trades (instrument class, market, currencies) |
| `cbu-custody.create-ssi` | Create a Standing Settlement Instruction (account info) |
| `cbu-custody.activate-ssi` | Activate an SSI |
| `cbu-custody.suspend-ssi` | Suspend an SSI |
| `cbu-custody.add-booking-rule` | Add ALERT-style booking rule for trade routing |
| `cbu-custody.validate-booking-coverage` | Validate booking rules cover trading universe |
| `cbu-custody.lookup-ssi` | Find SSI for trade characteristics |
| `cbu-custody.derive-required-coverage` | Derive required coverage from universe |

### Instrument Classes (CFI-based)
- `EQUITY` - Common stock, preferred stock
- `GOVT_BOND` - Government debt
- `CORP_BOND` - Corporate debt
- `ETF` - Exchange-traded funds
- `FUND` - Mutual funds

### Major Markets (MIC codes)
- `XNYS` - NYSE
- `XNAS` - NASDAQ
- `XLON` - London Stock Exchange
- `XPAR` - Euronext Paris
- `XFRA` - Frankfurt Stock Exchange

### Currencies (ISO 4217)
- `USD`, `EUR`, `GBP`, `CHF`, `JPY`

### Settlement Types
- `DVP` - Delivery vs Payment
- `FOP` - Free of Payment
- `RVP` - Receive vs Payment

### CUSTODY EXAMPLES

**Basic Custody Setup:**
```
(cbu.ensure :name "Pension Fund" :jurisdiction "US" :client-type "fund" :as @fund)

;; Layer 1: What they trade
(cbu-custody.add-universe :cbu-id @fund :instrument-class "EQUITY" :market "XNYS" :currencies ["USD"] :settlement-types ["DVP"])

;; Layer 2: Account info (SSI)
(cbu-custody.create-ssi :cbu-id @fund :name "US Safekeeping" :type "SECURITIES" :safekeeping-account "SAFE-001" :safekeeping-bic "BABOROCP" :cash-account "CASH-001" :cash-bic "BABOROCP" :cash-currency "USD" :pset-bic "DTCYUS33" :effective-date "2024-12-01" :as @ssi)
(cbu-custody.activate-ssi :ssi-id @ssi)

;; Layer 3: Booking rules
(cbu-custody.add-booking-rule :cbu-id @fund :ssi-id @ssi :name "US Equity DVP" :priority 10 :instrument-class "EQUITY" :market "XNYS" :currency "USD" :settlement-type "DVP")

;; Validate
(cbu-custody.validate-booking-coverage :cbu-id @fund)
```

**Multi-Market Setup:**
```
;; US Equities
(cbu-custody.add-universe :cbu-id @fund :instrument-class "EQUITY" :market "XNYS" :currencies ["USD"] :settlement-types ["DVP"])
;; UK Equities with dual currency
(cbu-custody.add-universe :cbu-id @fund :instrument-class "EQUITY" :market "XLON" :currencies ["GBP" "USD"] :settlement-types ["DVP"])
```

**Lookup SSI for Trade:**
```
(cbu-custody.lookup-ssi :cbu-id @fund :instrument-class "EQUITY" :market "XNYS" :currency "USD" :settlement-type "DVP")
```

Respond with ONLY the DSL code, no explanations or markdown. If you cannot generate valid DSL, respond with: ERROR: <reason>"#,
        vocab
    )
}

// =============================================================================
// REPL COMMAND - Interactive DSL session with CBU state
// =============================================================================

#[cfg(feature = "database")]
async fn cmd_repl(
    cbu_id: Option<String>,
    db_url: String,
    format: OutputFormat,
) -> Result<(), String> {
    use ob_poc::database::SessionRepository;
    use ob_poc::dsl_v2::{
        compile,
        config::ConfigLoader,
        emit_dsl,
        executor::{DslExecutor, ExecutionContext, ExecutionResult},
        parse_program, topological_sort,
        validation::{RustStyleFormatter, ValidationContext},
        BindingContext, BindingInfo, CsgLinter, RuntimeVerbRegistry,
    };
    use std::io::Write;
    use uuid::Uuid;

    // Connect to database
    if format == OutputFormat::Pretty {
        println!("{}", "DSL REPL - Interactive Session".cyan().bold());
        println!("{}", "=".repeat(50));
        println!();
        println!("{}", "Connecting to database...".dimmed());
    }

    let pool = sqlx::PgPool::connect(&db_url)
        .await
        .map_err(|e| format!("Database connection failed: {}", e))?;

    let repo = SessionRepository::new(pool.clone());
    let executor = DslExecutor::new(pool.clone());

    // Parse CBU ID if provided
    let cbu_uuid = if let Some(id) = &cbu_id {
        Some(Uuid::parse_str(id).map_err(|e| format!("Invalid CBU UUID: {}", e))?)
    } else {
        None
    };

    // Load or create session state
    let (session, mut binding_context) = if let Some(cbu) = cbu_uuid {
        if format == OutputFormat::Pretty {
            println!("{} Loading CBU state...", "→".cyan());
        }

        let state = repo
            .get_cbu_state(cbu)
            .await
            .map_err(|e| format!("Failed to load CBU state: {}", e))?;

        // Build binding context from existing bindings
        let mut ctx = BindingContext::new();
        for (name, uuid) in &state.bindings {
            // For now, mark all as Entity type - could be enhanced with type tracking
            ctx.insert(BindingInfo {
                name: name.clone(),
                produced_type: "entity".to_string(),
                subtype: None,
                entity_pk: *uuid,
                resolved: true,
                source_sheet_id: None,
            });
        }

        if format == OutputFormat::Pretty {
            println!(
                "{} Loaded session {} with {} bindings",
                "✓".green(),
                state.session_id.to_string().dimmed(),
                state.bindings.len()
            );
            if !state.bindings.is_empty() {
                println!();
                println!("{}:", "Available bindings".yellow());
                for (name, uuid) in &state.bindings {
                    println!("  @{} = {}", name.cyan(), uuid.to_string().dimmed());
                }
            }
        }

        (state, ctx)
    } else {
        if format == OutputFormat::Pretty {
            println!("{} No CBU specified - starting fresh session", "→".cyan());
            println!();
            println!("{}:", "Bootstrap hint".yellow());
            println!("  Start with a CBU to enable all other commands:");
            println!(
                "  {}",
                "(cbu.ensure :name \"My Fund\" :jurisdiction \"LU\" :as @fund)".cyan()
            );
            println!();
            println!("  Or attach to existing CBU: :cbu <uuid>");
        }
        (
            ob_poc::database::CbuDslState {
                cbu_id: Uuid::nil(),
                session_id: Uuid::new_v4(),
                executed_dsl: String::new(),
                bindings: std::collections::HashMap::new(),
                snapshot_count: 0,
                last_executed_at: None,
            },
            BindingContext::new(),
        )
    };

    // Initialize linter
    let mut linter = CsgLinter::new(pool.clone());
    linter
        .initialize()
        .await
        .map_err(|e| format!("Linter initialization failed: {}", e))?;

    // Load verb registry for topological sort
    // Load verb registry for topological sort
    let loader = ConfigLoader::from_env();
    let verbs_config = loader
        .load_verbs()
        .map_err(|e| format!("Failed to load verb config: {}", e))?;
    let registry = RuntimeVerbRegistry::from_config(&verbs_config);

    // Pending DSL buffer (not yet executed)
    let mut pending_dsl = String::new();
    let mut exec_ctx = ExecutionContext::default();

    // Populate exec_ctx with existing bindings
    for (name, uuid) in &session.bindings {
        exec_ctx.symbols.insert(name.clone(), *uuid);
    }

    if format == OutputFormat::Pretty {
        println!();
        println!("{}:", "Commands".yellow());
        println!("  {}    - Execute pending DSL", ":commit".green());
        println!("  {}  - Discard pending DSL", ":rollback".green());
        println!("  {}   - Show pending DSL", ":pending".green());
        println!("  {}  - Show current bindings", ":bindings".green());
        println!(
            "  {} - Reorder pending DSL by dependencies",
            ":reorder".green()
        );
        println!("  {}      - Show this help", ":help".green());
        println!("  {}      - Exit REPL", ":quit".green());
        println!();
        println!("Enter DSL statements (multi-line supported, blank line to finish input):");
        println!();
    }

    // REPL loop
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    loop {
        // Print prompt
        let prompt = if pending_dsl.is_empty() {
            "dsl> ".green().to_string()
        } else {
            "dsl+ ".yellow().to_string()
        };
        print!("{}", prompt);
        stdout.flush().map_err(|e| format!("IO error: {}", e))?;

        // Read line
        let mut line = String::new();
        match stdin.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(e) => return Err(format!("Read error: {}", e)),
        }

        let trimmed = line.trim();

        // Handle commands
        if trimmed.starts_with(':') {
            match trimmed {
                ":quit" | ":q" | ":exit" => {
                    if !pending_dsl.is_empty() {
                        println!(
                            "{} Warning: {} lines of pending DSL will be discarded",
                            "!".yellow(),
                            pending_dsl.lines().count()
                        );
                    }
                    println!("{}", "Goodbye!".cyan());
                    break;
                }

                ":help" | ":h" | ":?" => {
                    println!();
                    println!("{}:", "Commands".yellow());
                    println!("  :commit    - Execute pending DSL statements");
                    println!("  :rollback  - Discard pending DSL without executing");
                    println!("  :pending   - Show pending DSL buffer");
                    println!("  :bindings  - Show all available symbol bindings");
                    println!("  :reorder   - Topologically sort pending DSL by dependencies");
                    println!("  :clear     - Clear screen");
                    println!("  :quit      - Exit REPL");
                    println!();
                }

                ":pending" | ":p" => {
                    if pending_dsl.is_empty() {
                        println!("{}", "(no pending DSL)".dimmed());
                    } else {
                        println!();
                        println!("{}:", "Pending DSL".yellow());
                        for line in pending_dsl.lines() {
                            println!("  {}", line.cyan());
                        }
                        println!();
                    }
                }

                ":bindings" | ":b" => {
                    if exec_ctx.symbols.is_empty() {
                        println!("{}", "(no bindings)".dimmed());
                    } else {
                        println!();
                        println!("{}:", "Bindings".yellow());
                        for (name, uuid) in &exec_ctx.symbols {
                            println!("  @{} = {}", name.cyan(), uuid.to_string().dimmed());
                        }
                        println!();
                    }
                }

                ":rollback" | ":r" => {
                    if pending_dsl.is_empty() {
                        println!("{}", "(nothing to rollback)".dimmed());
                    } else {
                        let count = pending_dsl.lines().count();
                        pending_dsl.clear();
                        println!("{} Discarded {} lines", "✓".green(), count);
                    }
                }

                ":reorder" => {
                    if pending_dsl.is_empty() {
                        println!("{}", "(nothing to reorder)".dimmed());
                    } else {
                        // Parse pending DSL
                        match parse_program(&pending_dsl) {
                            Ok(ast) => {
                                // Perform topological sort
                                match topological_sort(&ast, &binding_context, &registry) {
                                    Ok(result) => {
                                        if !result.reordered {
                                            println!("{} Already in correct order", "✓".green());
                                        } else {
                                            // Emit reordered DSL
                                            let reordered = emit_dsl(&result.program);
                                            pending_dsl = reordered;
                                            println!(
                                                "{} Reordered {} statements",
                                                "✓".green(),
                                                result.program.statements.len()
                                            );
                                            println!();
                                            println!("{}:", "Reordered DSL".yellow());
                                            for line in pending_dsl.lines() {
                                                println!("  {}", line.cyan());
                                            }
                                            println!();
                                        }
                                    }
                                    Err(e) => {
                                        println!("{} Reorder failed: {:?}", "✗".red(), e);
                                    }
                                }
                            }
                            Err(e) => {
                                println!("{} Parse error: {:?}", "✗".red(), e);
                            }
                        }
                    }
                }

                ":commit" | ":c" => {
                    if pending_dsl.is_empty() {
                        println!("{}", "(nothing to commit)".dimmed());
                        continue;
                    }

                    println!();
                    println!("{}", "Validating and executing...".dimmed());

                    // 1. Parse
                    let ast = match parse_program(&pending_dsl) {
                        Ok(ast) => ast,
                        Err(e) => {
                            println!("{} Parse error: {:?}", "✗".red(), e);
                            continue;
                        }
                    };

                    // 2. CSG Lint
                    // Note: CSG linter builds its own binding context from the AST.
                    // Pre-existing bindings from previous executions are tracked in
                    // binding_context for topological sort, but linter validates
                    // self-contained DSL programs.
                    let context = ValidationContext::default();
                    let lint_result = linter.lint(ast.clone(), &context, &pending_dsl).await;

                    if lint_result.has_errors() {
                        let formatted =
                            RustStyleFormatter::format(&pending_dsl, &lint_result.diagnostics);
                        println!("{}", formatted);
                        println!("{} Validation failed - not executed", "✗".red());
                        continue;
                    }

                    if lint_result.has_warnings() {
                        let formatted =
                            RustStyleFormatter::format(&pending_dsl, &lint_result.diagnostics);
                        println!("{}", formatted);
                    }

                    // 3. Compile
                    let plan = match compile(&ast) {
                        Ok(plan) => plan,
                        Err(e) => {
                            println!("{} Compile error: {:?}", "✗".red(), e);
                            continue;
                        }
                    };

                    // 4. Execute
                    match executor.execute_plan(&plan, &mut exec_ctx).await {
                        Ok(results) => {
                            println!();
                            for (i, result) in results.iter().enumerate() {
                                let step = &plan.steps[i];
                                let verb_name =
                                    format!("{}.{}", step.verb_call.domain, step.verb_call.verb);

                                match result {
                                    ExecutionResult::Uuid(id) => {
                                        let binding_info = step
                                            .bind_as
                                            .as_ref()
                                            .map(|b| format!(" @{} =", b))
                                            .unwrap_or_default();
                                        println!(
                                            "  [{}] {}{} {}",
                                            i,
                                            verb_name.cyan(),
                                            binding_info.yellow(),
                                            id.to_string().dimmed()
                                        );

                                        // Update binding context
                                        if let Some(ref binding) = step.bind_as {
                                            binding_context.insert(BindingInfo {
                                                name: binding.clone(),
                                                produced_type: "entity".to_string(),
                                                subtype: None,
                                                entity_pk: *id,
                                                resolved: false,
                                                source_sheet_id: None,
                                            });
                                        }
                                    }
                                    ExecutionResult::Affected(n) => {
                                        println!("  [{}] {} ({} rows)", i, verb_name.cyan(), n);
                                    }
                                    _ => {
                                        println!("  [{}] {} {}", i, verb_name.cyan(), "✓".green());
                                    }
                                }
                            }

                            println!();
                            println!(
                                "{} Executed {} step(s) successfully",
                                "✓".green().bold(),
                                results.len()
                            );

                            // Clear pending buffer
                            pending_dsl.clear();
                        }
                        Err(e) => {
                            println!("{} Execution failed: {}", "✗".red(), e);
                        }
                    }
                }

                ":clear" => {
                    print!("\x1B[2J\x1B[1;1H"); // ANSI clear screen
                    stdout.flush().map_err(|e| format!("IO error: {}", e))?;
                }

                cmd if cmd.starts_with(":cbu ") => {
                    let id_str = cmd.strip_prefix(":cbu ").unwrap().trim();
                    match Uuid::parse_str(id_str) {
                        Ok(new_cbu) => {
                            println!("{} Switching to CBU {}...", "→".cyan(), new_cbu);
                            // This would require reloading state - simplified for now
                            println!(
                                "{} CBU switch not yet implemented in active session",
                                "!".yellow()
                            );
                            println!("  Restart REPL with: dsl_cli repl --cbu {}", id_str);
                        }
                        Err(e) => {
                            println!("{} Invalid UUID: {}", "✗".red(), e);
                        }
                    }
                }

                _ => {
                    println!("{} Unknown command: {}", "?".yellow(), trimmed);
                    println!("  Type :help for available commands");
                }
            }
            continue;
        }

        // Empty line - validate pending DSL inline
        if trimmed.is_empty() {
            if !pending_dsl.is_empty() {
                // Quick validation feedback
                match parse_program(&pending_dsl) {
                    Ok(ast) => {
                        println!(
                            "{} {} statement(s) parsed - use :commit to execute",
                            "✓".green(),
                            ast.statements.len()
                        );
                    }
                    Err(e) => {
                        println!("{} Parse error: {:?}", "✗".red(), e);
                    }
                }
            }
            continue;
        }

        // Add line to pending buffer
        pending_dsl.push_str(trimmed);
        pending_dsl.push('\n');
    }

    Ok(())
}
