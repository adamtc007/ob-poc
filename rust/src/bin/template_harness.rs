//! Template Test Harness CLI
//!
//! Tests all templates by loading, expanding, parsing, compiling, and optionally executing.
//!
//! Usage:
//!   cargo run --bin template_harness
//!   cargo run --bin template_harness -- --verbose
//!   cargo run --bin template_harness -- --json
//!   cargo run --features database --bin template_harness -- --execute

use std::env;
use std::path::PathBuf;

use ob_poc::templates::{run_harness_no_db, HarnessResult};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    let verbose = args.iter().any(|a| a == "--verbose" || a == "-v");
    let json_output = args.iter().any(|a| a == "--json");
    let execute = args.iter().any(|a| a == "--execute" || a == "-e");

    // Find templates directory
    let templates_dir = args
        .iter()
        .position(|a| a == "--templates-dir" || a == "-d")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("config/templates"));

    // Check templates directory exists
    if !templates_dir.exists() {
        eprintln!("Templates directory not found: {}", templates_dir.display());
        eprintln!("Run from the rust/ directory or specify --templates-dir PATH");
        std::process::exit(1);
    }

    if !json_output {
        println!("Loading templates from: {}", templates_dir.display());
    }

    // Run harness
    let result = if execute {
        #[cfg(feature = "database")]
        {
            let database_url =
                env::var("DATABASE_URL").expect("DATABASE_URL required for --execute");
            let pool = sqlx::PgPool::connect(&database_url).await?;
            ob_poc::templates::harness::run_harness(&templates_dir, true, Some(&pool)).await?
        }
        #[cfg(not(feature = "database"))]
        {
            eprintln!("Error: --execute requires database feature");
            eprintln!("Build with: cargo build --features database --bin template_harness");
            std::process::exit(1);
        }
    } else {
        run_harness_no_db(&templates_dir).await?
    };

    // Output results
    if json_output {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_results(&result, verbose);
    }

    // Exit with error code if any failures
    if result.parse_failed > 0 || result.compile_failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

fn print_results(result: &HarnessResult, verbose: bool) {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║              TEMPLATE TEST HARNESS RESULTS                   ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // Summary stats
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ Summary                                                     │");
    println!("├─────────────────────────────────────────────────────────────┤");
    println!(
        "│ Total templates:      {:>4}                                 │",
        result.total_templates
    );
    println!(
        "│ Expansion complete:   {:>4}  Incomplete: {:>4}              │",
        result.expansion_complete, result.expansion_incomplete
    );
    println!(
        "│ Parse success:        {:>4}  Failed: {:>4}                  │",
        result.parse_success, result.parse_failed
    );
    println!(
        "│ Compile success:      {:>4}  Failed: {:>4}                  │",
        result.compile_success, result.compile_failed
    );
    if result.execution_success > 0 || result.execution_failed > 0 {
        println!(
            "│ Execution success:    {:>4}  Failed: {:>4}                  │",
            result.execution_success, result.execution_failed
        );
    }
    println!("└─────────────────────────────────────────────────────────────┘\n");

    // Per-template results
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ Per-Template Results                                        │");
    println!("├─────────────────────────────────────────────────────────────┤");

    for r in &result.results {
        let status = if r.compile_success {
            "✓"
        } else if r.parse_success {
            "⚠"
        } else {
            "✗"
        };

        let expansion_status = if r.expansion_complete {
            "complete"
        } else {
            "partial"
        };

        println!(
            "│ {} {:<25} {:>8} {:>2} steps             │",
            status, r.template_id, expansion_status, r.step_count
        );

        if !r.missing_params.is_empty() {
            let missing = r.missing_params.join(", ");
            let pad = 47usize.saturating_sub(missing.len());
            println!("│   Missing: {}{} │", missing, " ".repeat(pad));
        }

        if let Some(ref err) = r.parse_error {
            let err_short = if err.len() > 50 { &err[..50] } else { err };
            let pad = 44usize.saturating_sub(err_short.len());
            println!("│   Parse error: {}{} │", err_short, " ".repeat(pad));
        }

        if let Some(ref err) = r.compile_error {
            let err_short = if err.len() > 50 { &err[..50] } else { err };
            let pad = 42usize.saturating_sub(err_short.len());
            println!("│   Compile error: {}{} │", err_short, " ".repeat(pad));
        }

        if verbose {
            if let Some(ref dsl) = r.dsl {
                println!("│   DSL:                                                      │");
                for line in dsl.lines().take(10) {
                    let line_short = if line.len() > 55 { &line[..55] } else { line };
                    let pad = 55usize.saturating_sub(line_short.len());
                    println!("│     {}{} │", line_short, " ".repeat(pad));
                }
                if dsl.lines().count() > 10 {
                    println!(
                        "│     ... ({} more lines)                                     │",
                        dsl.lines().count() - 10
                    );
                }
            }
        }
    }

    println!("└─────────────────────────────────────────────────────────────┘\n");

    // Legend
    println!("Legend: ✓ = compile success, ⚠ = parse ok/compile fail, ✗ = parse fail");
    println!("\nOptions: --verbose (-v), --json, --execute (-e), --templates-dir (-d) PATH");
}
