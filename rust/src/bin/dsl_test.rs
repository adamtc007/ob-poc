//! DSL Test CLI - Run onboarding DSL tests from the command line
//!
//! Usage:
//!   cargo run --bin dsl-test --features database -- --file examples/onboarding.dsl
//!   cargo run --bin dsl-test --features database -- --inline "(cbu.ensure :cbu-name "Test" :as @cbu)"

use anyhow::Result;
use clap::Parser;
use ob_poc::database::{DatabaseConfig, DatabaseManager};
use ob_poc::dsl_test_harness::{OnboardingTestHarness, OnboardingTestInput};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "dsl-test")]
#[command(about = "Test onboarding DSL validation pipeline")]
struct Cli {
    /// DSL file to test
    #[arg(short, long)]
    file: Option<PathBuf>,

    /// Inline DSL to test
    #[arg(short, long)]
    inline: Option<String>,

    /// CBU ID to use (default: random)
    #[arg(long)]
    cbu_id: Option<String>,

    /// Product codes to link (comma-separated)
    #[arg(long)]
    products: Option<String>,

    /// Output format: text, json
    #[arg(long, default_value = "text")]
    format: String,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Get DSL source
    let dsl_source = match (&cli.file, &cli.inline) {
        (Some(path), None) => std::fs::read_to_string(path)?,
        (None, Some(inline)) => inline.clone(),
        (Some(_), Some(_)) => {
            eprintln!("Error: Cannot specify both --file and --inline");
            std::process::exit(1);
        }
        (None, None) => {
            eprintln!("Error: Must specify either --file or --inline");
            std::process::exit(1);
        }
    };

    // Parse CBU ID
    let cbu_id = match &cli.cbu_id {
        Some(id) => Uuid::parse_str(id)?,
        None => Uuid::new_v4(),
    };

    // Parse product codes
    let product_codes: Vec<String> = cli
        .products
        .map(|p| p.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    // Connect to database
    let config = DatabaseConfig::default();
    let db = DatabaseManager::new(config).await?;

    // Create harness and run test
    let harness = OnboardingTestHarness::new(db.pool().clone()).await?;

    if cli.verbose {
        println!("Running DSL test...");
        println!("  CBU ID: {}", cbu_id);
        println!("  Products: {:?}", product_codes);
        println!("  DSL length: {} chars", dsl_source.len());
        println!();
    }

    let result = harness.run_test(OnboardingTestInput {
        cbu_id,
        product_codes,
        dsl_source,
    }).await?;

    // Output results
    match cli.format.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        _ => {
            println!("=== DSL Test Result ===");
            println!();
            println!("Request ID:       {}", result.request_id);
            println!("Validation:       {}", if result.validation_passed { "PASSED" } else { "FAILED" });
            println!();

            if result.validation_passed {
                println!("DSL Instance ID:  {:?}", result.dsl_instance_id);
                println!("DSL Version:      {:?}", result.dsl_version);
            } else {
                println!("Errors:");
                for err in &result.errors {
                    println!("  [{}] Line {}:{}: {}",
                        err.code, err.line, err.column, err.message);
                    if let Some(suggestion) = &err.suggestion {
                        println!("       Suggestion: {}", suggestion);
                    }
                }
            }

            println!();
            println!("=== Performance ===");
            println!("Parse time:       {}ms", result.parse_time_ms);
            println!("Validate time:    {}ms", result.validate_time_ms);
            println!("Persist time:     {}ms", result.persist_time_ms);
            println!("Total time:       {}ms", result.total_time_ms);

            println!();
            println!("=== Verification ===");
            let v = &result.verification;
            println!("Request exists:   {}", v.request_exists);
            println!("Request state:    {}", v.request_state);
            println!("Products linked:  {}/{}", v.products_linked, v.expected_products);
            println!("DSL exists:       {}", v.dsl_instance_exists);
            println!("Content matches:  {}", v.dsl_content_matches);
            println!("AST exists:       {}", v.ast_exists);
            println!("Symbol count:     {}", v.symbol_count);
            println!("Errors stored:    {}", v.errors_stored);
            println!("All checks:       {}", if v.all_checks_passed { "PASSED" } else { "FAILED" });
        }
    }

    // Exit with appropriate code
    if result.validation_passed && result.verification.all_checks_passed {
        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
}
