//! CLI tool for evaluating agent pipeline against golden test cases
//!
//! Usage:
//!   cargo run --bin evaluate_agent                    # Run all tests
//!   cargo run --bin evaluate_agent -- -c pricing      # Run pricing category
//!   cargo run --bin evaluate_agent -- -i im_simple_1  # Run single case
//!   cargo run --bin evaluate_agent -- --format json   # JSON output

use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;

use ob_poc::agentic::{
    DslGenerator, EntityExtractor, EntityTypesConfig, EvaluationDataset, EvaluationReport,
    EvaluationRunner, InstrumentHierarchyConfig, IntentTaxonomy, MarketRegionsConfig,
};

#[derive(Parser)]
#[command(name = "evaluate_agent")]
#[command(about = "Evaluate agent pipeline against golden test cases")]
struct Args {
    /// Path to config directory containing YAML files
    #[arg(short = 'd', long, default_value = "config/agent")]
    config_dir: PathBuf,

    /// Run only specific category
    #[arg(short = 'c', long)]
    category: Option<String>,

    /// Run only specific case ID
    #[arg(short = 'i', long)]
    case_id: Option<String>,

    /// Output format (text, json, csv)
    #[arg(short = 'f', long, default_value = "text")]
    format: String,

    /// Verbose output - show each case result
    #[arg(short = 'v', long)]
    verbose: bool,

    /// Minimum accuracy threshold to pass (0.0-1.0)
    #[arg(long, default_value = "0.85")]
    threshold: f64,

    /// List available categories and exit
    #[arg(long)]
    list_categories: bool,
}

fn main() -> ExitCode {
    let args = Args::parse();

    // Load configuration files
    let taxonomy_path = args.config_dir.join("intent_taxonomy.yaml");
    let entity_types_path = args.config_dir.join("entity_types.yaml");
    let market_regions_path = args.config_dir.join("market_regions.yaml");
    let instrument_hierarchy_path = args.config_dir.join("instrument_hierarchy.yaml");
    let mappings_path = args.config_dir.join("parameter_mappings.yaml");
    let dataset_path = args.config_dir.join("evaluation_dataset.yaml");

    // Load dataset first to handle --list-categories
    let dataset = match EvaluationDataset::load(&dataset_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error loading evaluation dataset: {}", e);
            return ExitCode::FAILURE;
        }
    };

    if args.list_categories {
        println!("Available categories:");
        for cat in dataset.categories.keys() {
            let count = dataset.categories.get(cat).map(|c| c.len()).unwrap_or(0);
            println!("  {} ({} cases)", cat, count);
        }
        println!("\nDataset categories:");
        let mut cats: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for case in &dataset.evaluation_cases {
            cats.insert(&case.category);
        }
        for cat in cats {
            let count = dataset
                .evaluation_cases
                .iter()
                .filter(|c| c.category == cat)
                .count();
            println!("  {} ({} cases)", cat, count);
        }
        return ExitCode::SUCCESS;
    }

    // Load taxonomy
    let taxonomy = match IntentTaxonomy::load_from_file(&taxonomy_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error loading intent taxonomy: {}", e);
            return ExitCode::FAILURE;
        }
    };

    // Load entity types
    let entity_types = match EntityTypesConfig::load_from_file(&entity_types_path) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error loading entity types: {}", e);
            return ExitCode::FAILURE;
        }
    };

    // Load market regions
    let market_regions = match MarketRegionsConfig::load_from_file(&market_regions_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error loading market regions: {}", e);
            return ExitCode::FAILURE;
        }
    };

    // Load instrument hierarchy
    let instrument_hierarchy =
        match InstrumentHierarchyConfig::load_from_file(&instrument_hierarchy_path) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("Error loading instrument hierarchy: {}", e);
                return ExitCode::FAILURE;
            }
        };

    // Create entity extractor from loaded configs
    let extractor = EntityExtractor::new(entity_types, market_regions, instrument_hierarchy);

    // Load DSL generator
    let generator = match DslGenerator::from_file(&mappings_path) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error loading parameter mappings: {}", e);
            return ExitCode::FAILURE;
        }
    };

    // Create runner
    let mut runner = EvaluationRunner::new(taxonomy, extractor, generator, dataset);

    // Run evaluation
    let report = if let Some(ref case_id) = args.case_id {
        // Single case
        match runner.run_single(case_id) {
            Some(result) => {
                if args.verbose || args.format == "text" {
                    print_single_result(&result, &args.format);
                }
                // Create a minimal report for the single case
                EvaluationReport::from_results(
                    vec![result],
                    &ob_poc::agentic::evaluation::MetricsConfig {
                        intent_classification: ob_poc::agentic::evaluation::ThresholdConfig {
                            accuracy_threshold: args.threshold,
                            precision_threshold: 0.80,
                            recall_threshold: 0.80,
                        },
                        entity_extraction: ob_poc::agentic::evaluation::ThresholdConfig {
                            accuracy_threshold: 0.90,
                            precision_threshold: 0.85,
                            recall_threshold: 0.85,
                        },
                        dsl_generation: ob_poc::agentic::evaluation::DslThresholdConfig {
                            validity_threshold: 0.95,
                            completeness_threshold: 0.90,
                        },
                    },
                )
            }
            None => {
                eprintln!("Case not found: {}", case_id);
                return ExitCode::FAILURE;
            }
        }
    } else if let Some(ref category) = args.category {
        runner.run_category(category)
    } else {
        runner.run_all()
    };

    // Print results
    match args.format.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
        }
        "csv" => {
            report.print_csv();
        }
        _ => {
            if args.verbose {
                for result in &report.results {
                    print_single_result(result, "text");
                    println!();
                }
            }
            report.print_summary();
        }
    }

    // Exit with error code if below threshold
    if report.intent_accuracy < args.threshold {
        eprintln!(
            "\nFailed: Intent accuracy {:.1}% is below threshold {:.1}%",
            report.intent_accuracy * 100.0,
            args.threshold * 100.0
        );
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn print_single_result(result: &ob_poc::agentic::EvaluationResult, format: &str) {
    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(result).unwrap());
        }
        _ => {
            let status = if result.passed { "PASS" } else { "FAIL" };
            println!(
                "[{}] {} ({}/{})",
                status, result.case_id, result.category, result.difficulty
            );
            println!("  Intents: {:?}", result.classified_intents);
            println!("  Entities: {:?}", result.extracted_entities);
            if let Some(ref dsl) = result.generated_dsl {
                println!("  DSL: {}", dsl);
            }
            if !result.errors.is_empty() {
                println!("  Errors:");
                for err in &result.errors {
                    println!("    - {}", err);
                }
            }
            println!("  Latency: {}ms", result.latency_ms);
        }
    }
}
