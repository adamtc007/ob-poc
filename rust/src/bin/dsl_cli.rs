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
}

// =============================================================================
// MAIN
// =============================================================================

fn main() -> ExitCode {
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
    #[cfg(feature = "database")]
    let linter = ob_poc::dsl_v2::CsgLinter::new_without_db();
    #[cfg(not(feature = "database"))]
    let linter = ob_poc::dsl_v2::CsgLinter::new();
    let source_clone = source.clone();

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
    let linter = ob_poc::dsl_v2::CsgLinter::new_without_db();
    #[cfg(not(feature = "database"))]
    let linter = ob_poc::dsl_v2::CsgLinter::new();

    let lint_result = futures::executor::block_on(async {
        let mut linter = linter;
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
                            let names: Vec<_> = required.iter().map(|a| a.name).collect();
                            println!("    Required: {:?}", names);
                        }
                        if !optional.is_empty() {
                            let names: Vec<_> = optional.iter().map(|a| a.name).collect();
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
            "Unknown category: '{}'\n\nAvailable: onboarding, documents, entities, all",
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
