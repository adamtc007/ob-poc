//! Batch Test Harness CLI
//!
//! Tests batch template execution by:
//! 1. Loading a template from the registry
//! 2. Querying entities to process from the database
//! 3. Accepting shared parameters (same for all batch items)
//! 4. Expanding template and executing DSL for each entity
//! 5. (Optional) Phase 2: Agent-generated DSL for follow-up operations
//!
//! Usage:
//!   cargo run --features database,cli --bin batch_test_harness -- \
//!     --template onboard-fund-cbu \
//!     --query "SELECT entity_id, name FROM \"ob-poc\".entities WHERE name ILIKE 'Allianz%'" \
//!     --shared manco_entity="Allianz Global Investors GmbH" \
//!     --shared im_entity="Allianz Global Investors GmbH" \
//!     --shared jurisdiction=LU \
//!     --limit 5 \
//!     --dry-run
//!
//! Examples:
//!   # Dry run with 5 funds
//!   cargo run --features database,cli --bin batch_test_harness -- \
//!     --template onboard-fund-cbu \
//!     --fund-query \
//!     --limit 5 \
//!     --dry-run
//!
//!   # Execute all 205 funds
//!   cargo run --features database,cli --bin batch_test_harness -- \
//!     --template onboard-fund-cbu \
//!     --fund-query \
//!     --shared jurisdiction=LU
//!
//!   # Full real-world test: Create CBUs + add products via agent
//!   cargo run --features database,cli --bin batch_test_harness -- \
//!     --template onboard-fund-cbu \
//!     --fund-query \
//!     --shared jurisdiction=LU \
//!     --add-products "CUSTODY,FUND_ACCOUNTING"

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use clap::Parser;
use colored::Colorize;
use reqwest::Client;
use sqlx::PgPool;
use uuid::Uuid;

use ob_poc::dsl_v2::config::loader::ConfigLoader;
use ob_poc::dsl_v2::runtime_registry::RuntimeVerbRegistry;
use ob_poc::dsl_v2::{DslExecutor, ExecutionContext};
use ob_poc::templates::{ExpansionContext, TemplateExpander, TemplateRegistry};

/// Batch Test Harness for Template Execution
#[derive(Parser, Debug)]
#[command(name = "batch_test_harness")]
#[command(about = "Test batch template execution with database entities")]
struct Args {
    /// Template ID to execute (e.g., "onboard-fund-cbu")
    #[arg(long, short = 't')]
    template: String,

    /// SQL query to get batch entities (must return entity_id UUID and name TEXT)
    #[arg(long, short = 'q')]
    query: Option<String>,

    /// Use predefined query for Allianz funds
    #[arg(long)]
    fund_query: bool,

    /// Shared parameters in format key=value (can be specified multiple times)
    #[arg(long, short = 's', value_parser = parse_key_val)]
    shared: Vec<(String, String)>,

    /// Limit number of entities to process
    #[arg(long, short = 'l')]
    limit: Option<usize>,

    /// Dry run - expand templates but don't execute
    #[arg(long, short = 'n')]
    dry_run: bool,

    /// Show expanded DSL for each entity
    #[arg(long, short = 'v')]
    verbose: bool,

    /// Output results as JSON
    #[arg(long)]
    json: bool,

    /// Templates directory (default: config/verbs/templates)
    #[arg(long, short = 'd')]
    templates_dir: Option<PathBuf>,

    /// Continue on error (don't stop on first failure)
    #[arg(long, short = 'c')]
    continue_on_error: bool,

    /// Products to add via agent after CBU creation (comma-separated codes)
    /// Example: --add-products "CUSTODY,FUND_ACCOUNTING"
    #[arg(long)]
    add_products: Option<String>,

    /// Agent API URL for DSL generation (default: http://localhost:3000)
    #[arg(long, default_value = "http://localhost:3000")]
    agent_url: String,
}

/// Parse key=value pairs from command line
fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("Invalid key=value pair: {}", s))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

/// Result of processing a single batch item
#[derive(Debug, Clone, serde::Serialize)]
struct BatchItemResult {
    index: usize,
    entity_id: Uuid,
    entity_name: String,
    success: bool,
    dsl: Option<String>,
    created_cbu_id: Option<Uuid>,
    error: Option<String>,
    duration_ms: u64,
}

/// Result of adding a product to a CBU via server batch endpoint
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ProductAddResult {
    cbu_id: Uuid,
    #[serde(default)]
    cbu_name: String,
    #[serde(alias = "product")]
    product_code: String,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    services_added: Option<i32>,
}

/// Phase 2 results summary
#[derive(Debug, Clone, serde::Serialize)]
struct Phase2Result {
    products_requested: Vec<String>,
    total_operations: usize,
    success_count: usize,
    failure_count: usize,
    duration_ms: u64,
    items: Vec<ProductAddResult>,
}

/// Overall batch execution result
#[derive(Debug, Clone, serde::Serialize)]
struct BatchResult {
    template_id: String,
    total_entities: usize,
    processed: usize,
    success_count: usize,
    failure_count: usize,
    skipped_count: usize,
    dry_run: bool,
    total_duration_ms: u64,
    items: Vec<BatchItemResult>,
    phase2: Option<Phase2Result>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if present
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .init();

    let args = Args::parse();

    // Connect to database
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPool::connect(&database_url).await?;

    // Load template registry
    let templates_dir = args.templates_dir.unwrap_or_else(|| {
        let loader = ConfigLoader::from_env();
        loader.config_dir().join("verbs").join("templates")
    });

    if !templates_dir.exists() {
        eprintln!(
            "{} Templates directory not found: {}",
            "ERROR:".red().bold(),
            templates_dir.display()
        );
        std::process::exit(1);
    }

    let registry = TemplateRegistry::load_from_dir(&templates_dir)?;

    // Get template
    let template = registry.get(&args.template).ok_or_else(|| {
        format!(
            "Template '{}' not found. Available: {:?}",
            args.template,
            registry.list_ids()
        )
    })?;

    if !args.json {
        println!("\n{} {}", "Template:".cyan().bold(), template.metadata.name);
        println!("{} {}", "ID:".cyan(), args.template);
        println!("{} {}", "Summary:".cyan(), template.metadata.summary);
    }

    // Build query
    let query = if args.fund_query {
        r#"
            SELECT e.entity_id, e.name
            FROM "ob-poc".entities e
            JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
            WHERE e.name ILIKE 'Allianz%'
              AND et.type_code IN ('fund', 'fund_subfund', 'fund_umbrella')
            ORDER BY e.name
        "#
        .to_string()
    } else if let Some(q) = args.query {
        q
    } else {
        eprintln!(
            "{} Must specify --query or --fund-query",
            "ERROR:".red().bold()
        );
        std::process::exit(1);
    };

    // Execute query to get entities
    let entities: Vec<(Uuid, String)> = sqlx::query_as(&query).fetch_all(&pool).await?;

    let entities: Vec<(Uuid, String)> = if let Some(limit) = args.limit {
        entities.into_iter().take(limit).collect()
    } else {
        entities
    };

    if !args.json {
        println!(
            "\n{} {} entities to process",
            "Found:".green().bold(),
            entities.len()
        );
    }

    if entities.is_empty() {
        if !args.json {
            println!("{} No entities found matching query", "WARNING:".yellow());
        }
        return Ok(());
    }

    // Build shared params map
    let mut shared_params: HashMap<String, String> = args.shared.into_iter().collect();

    // Resolve shared entity references to UUIDs
    resolve_entity_refs(&mut shared_params, &pool).await?;

    if !args.json {
        println!("\n{}", "Shared parameters:".cyan().bold());
        for (k, v) in &shared_params {
            // Truncate long UUIDs for display
            let display_val = if v.len() > 40 {
                format!("{}...", &v[..36])
            } else {
                v.clone()
            };
            println!("  {} = {}", k.yellow(), display_val);
        }
    }

    // Load verb registry for execution
    let loader = ConfigLoader::from_env();
    let config = loader.load_verbs()?;
    let verb_registry = Arc::new(RuntimeVerbRegistry::from_config(&config));

    // Process each entity
    let start_time = Instant::now();
    let mut results: Vec<BatchItemResult> = Vec::new();
    let mut success_count = 0;
    let mut failure_count = 0;

    for (index, (entity_id, entity_name)) in entities.iter().enumerate() {
        let item_start = Instant::now();

        if !args.json {
            print!(
                "\n[{}/{}] {} {}... ",
                index + 1,
                entities.len(),
                "Processing:".blue(),
                entity_name
            );
        }

        // Build params for this entity
        let mut params = shared_params.clone();
        params.insert("fund_entity".to_string(), entity_id.to_string());
        params.insert("fund_entity.name".to_string(), entity_name.clone());

        // Create expansion context
        let context = ExpansionContext::new();

        // Expand template
        let expansion = TemplateExpander::expand(template, &params, &context);

        if !expansion.missing_params.is_empty() {
            let missing: Vec<_> = expansion.missing_params.iter().map(|p| &p.name).collect();
            let error = format!("Missing required params: {:?}", missing);

            if !args.json {
                println!("{}", "SKIP".yellow());
                println!("  {}", error);
            }

            results.push(BatchItemResult {
                index,
                entity_id: *entity_id,
                entity_name: entity_name.clone(),
                success: false,
                dsl: Some(expansion.dsl),
                created_cbu_id: None,
                error: Some(error),
                duration_ms: item_start.elapsed().as_millis() as u64,
            });

            if !args.continue_on_error {
                break;
            }
            continue;
        }

        if args.verbose && !args.json {
            println!();
            println!("  {}", "Expanded DSL:".cyan());
            for line in expansion.dsl.lines() {
                println!("    {}", line.dimmed());
            }
        }

        if args.dry_run {
            if !args.json {
                println!("{}", "DRY-RUN".cyan());
            }
            success_count += 1;
            results.push(BatchItemResult {
                index,
                entity_id: *entity_id,
                entity_name: entity_name.clone(),
                success: true,
                dsl: Some(expansion.dsl),
                created_cbu_id: None,
                error: None,
                duration_ms: item_start.elapsed().as_millis() as u64,
            });
            continue;
        }

        // Parse and execute DSL
        match execute_dsl(&expansion.dsl, &pool, verb_registry.clone()).await {
            Ok(created_id) => {
                if !args.json {
                    println!("{}", "OK".green());
                    if let Some(id) = created_id {
                        println!("  Created CBU: {}", id.to_string().dimmed());
                    }
                }
                success_count += 1;
                results.push(BatchItemResult {
                    index,
                    entity_id: *entity_id,
                    entity_name: entity_name.clone(),
                    success: true,
                    dsl: if args.verbose {
                        Some(expansion.dsl)
                    } else {
                        None
                    },
                    created_cbu_id: created_id,
                    error: None,
                    duration_ms: item_start.elapsed().as_millis() as u64,
                });
            }
            Err(e) => {
                if !args.json {
                    println!("{}", "FAIL".red());
                    println!("  Error: {}", e.to_string().red());
                }
                failure_count += 1;
                results.push(BatchItemResult {
                    index,
                    entity_id: *entity_id,
                    entity_name: entity_name.clone(),
                    success: false,
                    dsl: Some(expansion.dsl),
                    created_cbu_id: None,
                    error: Some(e.to_string()),
                    duration_ms: item_start.elapsed().as_millis() as u64,
                });

                if !args.continue_on_error {
                    break;
                }
            }
        }
    }

    let phase1_duration = start_time.elapsed();

    // =========================================================================
    // PHASE 2: Add products via agent-generated DSL
    // =========================================================================
    let phase2_result = if let Some(ref products_str) = args.add_products {
        if args.dry_run {
            if !args.json {
                println!("\n{}", "═".repeat(60));
                println!(
                    "{} (skipped - dry run)",
                    "PHASE 2: AGENT PRODUCT ADDITION".magenta().bold()
                );
            }
            None
        } else {
            let products: Vec<String> = products_str
                .split(',')
                .map(|s| s.trim().to_uppercase())
                .collect();

            if !args.json {
                println!("\n{}", "═".repeat(60));
                println!("{}", "PHASE 2: AGENT PRODUCT ADDITION".magenta().bold());
                println!("{}", "═".repeat(60));
                println!("Products to add: {}", products.join(", ").yellow());
            }

            // Collect successfully created CBU IDs with their names (for display)
            let created_cbus: Vec<(Uuid, String)> = results
                .iter()
                .filter_map(|r| {
                    if r.success {
                        r.created_cbu_id.map(|id| (id, r.entity_name.clone()))
                    } else {
                        None
                    }
                })
                .collect();

            if created_cbus.is_empty() {
                if !args.json {
                    println!("{} No CBUs created in Phase 1", "WARNING:".yellow());
                }
                None
            } else {
                let phase2_start = Instant::now();
                let http_client = Client::new();

                // Build lookup map for CBU names
                let cbu_name_map: HashMap<Uuid, String> = created_cbus.iter().cloned().collect();
                let cbu_ids: Vec<Uuid> = created_cbus.iter().map(|(id, _)| *id).collect();

                if !args.json {
                    println!(
                        "\nCalling server batch endpoint for {} CBUs × {} products...",
                        cbu_ids.len(),
                        products.len()
                    );
                }

                // Single server-side batch call - no LLM, no chunking needed
                match call_batch_add_products(&http_client, &args.agent_url, &cbu_ids, &products)
                    .await
                {
                    Ok(batch_response) => {
                        if !args.json {
                            println!(
                                "Server processed {} operations in {}ms",
                                batch_response.total_operations.to_string().cyan(),
                                batch_response.duration_ms
                            );
                            println!(
                                "  Success: {}, Failed: {}",
                                batch_response.success_count.to_string().green(),
                                batch_response.failure_count.to_string().red()
                            );
                        }

                        // Enrich results with CBU names for display
                        let product_results: Vec<ProductAddResult> = batch_response
                            .results
                            .into_iter()
                            .map(|r| ProductAddResult {
                                cbu_id: r.cbu_id,
                                cbu_name: cbu_name_map.get(&r.cbu_id).cloned().unwrap_or_default(),
                                product_code: r.product,
                                success: r.success,
                                error: r.error,
                                services_added: r.services_added,
                            })
                            .collect();

                        // Print individual failures if verbose
                        if args.verbose && !args.json {
                            for r in &product_results {
                                if !r.success {
                                    println!(
                                        "    {} {} + {}: {}",
                                        "FAIL".red(),
                                        r.cbu_name,
                                        r.product_code,
                                        r.error.as_deref().unwrap_or("unknown error")
                                    );
                                }
                            }
                        }

                        Some(Phase2Result {
                            products_requested: products,
                            total_operations: product_results.len(),
                            success_count: batch_response.success_count,
                            failure_count: batch_response.failure_count,
                            duration_ms: phase2_start.elapsed().as_millis() as u64,
                            items: product_results,
                        })
                    }
                    Err(e) => {
                        if !args.json {
                            println!("{} Batch endpoint failed: {}", "ERROR:".red(), e);
                        }
                        // Return all as failed
                        let error_msg = format!("Batch endpoint error: {}", e);
                        let mut product_results: Vec<ProductAddResult> = Vec::new();
                        for cbu_id in &cbu_ids {
                            for product in &products {
                                product_results.push(ProductAddResult {
                                    cbu_id: *cbu_id,
                                    cbu_name: cbu_name_map.get(cbu_id).cloned().unwrap_or_default(),
                                    product_code: product.clone(),
                                    success: false,
                                    error: Some(error_msg.clone()),
                                    services_added: None,
                                });
                            }
                        }

                        Some(Phase2Result {
                            products_requested: products,
                            total_operations: product_results.len(),
                            success_count: 0,
                            failure_count: product_results.len(),
                            duration_ms: phase2_start.elapsed().as_millis() as u64,
                            items: product_results,
                        })
                    }
                }
            }
        }
    } else {
        None
    };

    let total_duration = start_time.elapsed();

    let batch_result = BatchResult {
        template_id: args.template.clone(),
        total_entities: entities.len(),
        processed: results.len(),
        success_count,
        failure_count,
        skipped_count: entities.len() - results.len(),
        dry_run: args.dry_run,
        total_duration_ms: total_duration.as_millis() as u64,
        items: results,
        phase2: phase2_result,
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&batch_result)?);
    } else {
        // Print Phase 1 summary
        println!("\n{}", "═".repeat(60));
        println!("{}", "PHASE 1: TEMPLATE EXECUTION SUMMARY".cyan().bold());
        println!("{}", "═".repeat(60));
        println!("Template:     {}", batch_result.template_id.yellow().bold());
        println!("Total:        {}", batch_result.total_entities);
        println!("Processed:    {}", batch_result.processed);
        println!(
            "Success:      {}",
            batch_result.success_count.to_string().green()
        );
        println!(
            "Failed:       {}",
            batch_result.failure_count.to_string().red()
        );
        if batch_result.skipped_count > 0 {
            println!(
                "Skipped:      {}",
                batch_result.skipped_count.to_string().yellow()
            );
        }
        println!("Duration:     {:.2}s", phase1_duration.as_secs_f64());

        // Print Phase 2 summary if applicable
        if let Some(ref p2) = batch_result.phase2 {
            println!("\n{}", "═".repeat(60));
            println!(
                "{}",
                "PHASE 2: AGENT PRODUCT ADDITION SUMMARY".magenta().bold()
            );
            println!("{}", "═".repeat(60));
            println!(
                "Products:     {}",
                p2.products_requested.join(", ").yellow()
            );
            println!("Operations:   {}", p2.total_operations);
            println!("Success:      {}", p2.success_count.to_string().green());
            println!("Failed:       {}", p2.failure_count.to_string().red());
            println!("Duration:     {:.2}s", p2.duration_ms as f64 / 1000.0);
        }

        println!("\n{}", "═".repeat(60));
        println!("{}", "TOTAL".bold());
        println!("{}", "═".repeat(60));
        println!("Total Duration: {:.2}s", total_duration.as_secs_f64());

        if batch_result.dry_run {
            println!("\n{}", "DRY RUN - no changes made".cyan().bold());
        }
    }

    // Exit with error code if any failures
    let phase2_failures = batch_result
        .phase2
        .as_ref()
        .map(|p| p.failure_count)
        .unwrap_or(0);
    if failure_count > 0 || phase2_failures > 0 {
        std::process::exit(1);
    }

    Ok(())
}

/// Resolve entity name references to UUIDs
async fn resolve_entity_refs(
    params: &mut HashMap<String, String>,
    pool: &PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    // List of params that should be resolved to entity UUIDs
    let entity_params = ["manco_entity", "im_entity"];

    for param in entity_params {
        if let Some(value) = params.get(param).cloned() {
            // Skip if already a UUID
            if Uuid::try_parse(&value).is_ok() {
                continue;
            }

            // Try to resolve by name
            let result: Option<(Uuid,)> = sqlx::query_as(
                r#"SELECT entity_id FROM "ob-poc".entities WHERE name = $1 LIMIT 1"#,
            )
            .bind(&value)
            .fetch_optional(pool)
            .await?;

            if let Some((entity_id,)) = result {
                params.insert(param.to_string(), entity_id.to_string());
            } else {
                return Err(
                    format!("Could not resolve entity '{}' for param '{}'", value, param).into(),
                );
            }
        }
    }

    Ok(())
}

/// Execute DSL and return created CBU ID if any
async fn execute_dsl(
    dsl: &str,
    pool: &PgPool,
    _verb_registry: Arc<RuntimeVerbRegistry>,
) -> Result<Option<Uuid>, Box<dyn std::error::Error>> {
    use ob_poc::dsl_v2::{compile, parse_program};

    // Parse
    let ast = parse_program(dsl).map_err(|e| format!("Parse error: {:?}", e))?;

    // Compile
    let plan = compile(&ast).map_err(|e| format!("Compile error: {:?}", e))?;

    // Execute
    let executor = DslExecutor::new(pool.clone());
    let mut ctx = ExecutionContext::new();

    executor
        .execute_plan(&plan, &mut ctx)
        .await
        .map_err(|e| format!("Execution error: {}", e))?;

    // Look for @cbu binding
    let cbu_id = ctx.symbols.get("cbu").copied();

    Ok(cbu_id)
}

/// Call server batch endpoint to add products to CBUs
/// This is server-side DSL generation - no LLM needed
async fn call_batch_add_products(
    client: &Client,
    agent_url: &str,
    cbu_ids: &[Uuid],
    products: &[String],
) -> Result<BatchAddProductsResponse, Box<dyn std::error::Error>> {
    #[derive(serde::Serialize)]
    struct BatchAddProductsRequest {
        cbu_ids: Vec<Uuid>,
        products: Vec<String>,
    }

    let url = format!("{}/api/batch/add-products", agent_url);
    let req = BatchAddProductsRequest {
        cbu_ids: cbu_ids.to_vec(),
        products: products.to_vec(),
    };

    let response = client
        .post(&url)
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("HTTP error {}: {}", status, body).into());
    }

    let resp: BatchAddProductsResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(resp)
}

/// Server batch response type
#[derive(Debug, serde::Deserialize)]
struct BatchAddProductsResponse {
    total_operations: usize,
    success_count: usize,
    failure_count: usize,
    duration_ms: u64,
    results: Vec<ServerBatchProductResult>,
}

/// Individual result from server batch endpoint
#[derive(Debug, serde::Deserialize)]
struct ServerBatchProductResult {
    cbu_id: Uuid,
    product: String,
    success: bool,
    error: Option<String>,
    services_added: Option<i32>,
}
