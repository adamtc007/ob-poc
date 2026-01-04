//! Verb contract management commands
//!
//! Commands for compiling, inspecting, and diagnosing DSL verbs.

use anyhow::{Context, Result};
use sqlx::PgPool;

use dsl_core::config::ConfigLoader;
use ob_poc::dsl_v2::RuntimeVerbRegistry;
use ob_poc::session::verb_contract::VerbDiagnostics;
use ob_poc::session::verb_sync::VerbSyncService;

/// Compile all verbs from YAML and sync to database
pub async fn verbs_compile(verbose: bool) -> Result<()> {
    println!("===========================================");
    println!("  Verb Contract Compilation");
    println!("===========================================\n");

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    let pool = PgPool::connect(&database_url)
        .await
        .context("Failed to connect to database")?;

    // Load verb registry from YAML
    println!("Loading verb registry from YAML...");
    let loader = ConfigLoader::from_env();
    let verbs_config = loader.load_verbs().context("Failed to load verb config")?;
    let registry = RuntimeVerbRegistry::from_config(&verbs_config);
    let verb_count = registry.all_verbs().count();
    println!("  Found {} verbs\n", verb_count);

    // Sync to database with contract compilation
    println!("Syncing verbs to database...");
    let sync_service = VerbSyncService::new(pool.clone());
    let result = sync_service
        .sync_all(&registry)
        .await
        .context("Failed to sync verbs")?;

    println!("\n===========================================");
    println!("  Compilation Summary");
    println!("===========================================");
    println!("  Added:     {}", result.verbs_added);
    println!("  Updated:   {}", result.verbs_updated);
    println!("  Unchanged: {}", result.verbs_unchanged);
    println!("  Removed:   {}", result.verbs_removed);

    // Query for diagnostics summary
    let diag_stats: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE jsonb_array_length(diagnostics_json->'errors') > 0),
            COUNT(*) FILTER (WHERE jsonb_array_length(diagnostics_json->'warnings') > 0)
        FROM "ob-poc".dsl_verbs
        WHERE diagnostics_json IS NOT NULL
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap_or((0, 0));

    println!("\n  Verbs with errors:   {}", diag_stats.0);
    println!("  Verbs with warnings: {}", diag_stats.1);

    if verbose && (diag_stats.0 > 0 || diag_stats.1 > 0) {
        println!("\nRun 'cargo x verbs diagnostics' to see details.");
    }

    Ok(())
}

/// Show compiled contract for a specific verb
pub async fn verbs_show(verb_name: &str) -> Result<()> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    let pool = PgPool::connect(&database_url)
        .await
        .context("Failed to connect to database")?;

    #[derive(sqlx::FromRow)]
    #[allow(dead_code)]
    struct VerbRow {
        domain: String,
        verb_name: String,
        description: Option<String>,
        behavior: Option<String>,
        compiled_json: Option<serde_json::Value>,
        effective_config_json: Option<serde_json::Value>,
        diagnostics_json: Option<serde_json::Value>,
        compiled_hash: Option<Vec<u8>>,
    }

    // Try exact match first, then partial match
    let row: Option<VerbRow> = sqlx::query_as(
        r#"
        SELECT domain, verb_name, description, behavior,
               compiled_json, effective_config_json, diagnostics_json, compiled_hash
        FROM "ob-poc".dsl_verbs
        WHERE verb_name = $1
           OR CONCAT(domain, '.', REPLACE(verb_name, CONCAT(domain, '.'), '')) = $1
        LIMIT 1
        "#,
    )
    .bind(verb_name)
    .fetch_optional(&pool)
    .await
    .context("Failed to query verb")?;

    match row {
        Some(row) => {
            println!("===========================================");
            println!("  Verb: {}", row.verb_name);
            println!("===========================================\n");

            println!("Domain:      {}", row.domain);
            println!(
                "Description: {}",
                row.description.as_deref().unwrap_or("(none)")
            );
            println!(
                "Behavior:    {}",
                row.behavior.as_deref().unwrap_or("(none)")
            );

            if let Some(hash) = &row.compiled_hash {
                let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
                println!("Hash:        {}", &hex[..16]); // Show first 16 chars
            }

            if let Some(ref compiled) = row.compiled_json {
                println!("\n--- Compiled Contract ---");
                println!(
                    "{}",
                    serde_json::to_string_pretty(compiled).unwrap_or_default()
                );
            } else {
                println!("\n(No compiled contract - run 'cargo x verbs compile' first)");
            }

            if let Some(ref diag) = row.diagnostics_json {
                let diagnostics: VerbDiagnostics =
                    serde_json::from_value(diag.clone()).unwrap_or_default();
                if !diagnostics.errors.is_empty() || !diagnostics.warnings.is_empty() {
                    println!("\n--- Diagnostics ---");
                    for e in &diagnostics.errors {
                        println!("  ERROR [{}]: {}", e.code, e.message);
                        if let Some(ref path) = e.path {
                            println!("        at: {}", path);
                        }
                        if let Some(ref hint) = e.hint {
                            println!("        hint: {}", hint);
                        }
                    }
                    for w in &diagnostics.warnings {
                        println!("  WARN  [{}]: {}", w.code, w.message);
                        if let Some(ref path) = w.path {
                            println!("        at: {}", path);
                        }
                        if let Some(ref hint) = w.hint {
                            println!("        hint: {}", hint);
                        }
                    }
                }
            }
        }
        None => {
            println!("Verb not found: {}", verb_name);
            println!("\nTip: Use the full verb name (e.g., 'cbu.ensure' or 'entity.create-proper-person')");

            // Suggest similar verbs
            let similar: Vec<(String,)> = sqlx::query_as(
                r#"
                SELECT verb_name
                FROM "ob-poc".dsl_verbs
                WHERE verb_name ILIKE '%' || $1 || '%'
                LIMIT 5
                "#,
            )
            .bind(verb_name)
            .fetch_all(&pool)
            .await
            .unwrap_or_default();

            if !similar.is_empty() {
                println!("\nDid you mean:");
                for (name,) in similar {
                    println!("  - {}", name);
                }
            }
        }
    }

    Ok(())
}

/// Show all verbs with diagnostics (errors or warnings)
pub async fn verbs_diagnostics(errors_only: bool) -> Result<()> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    let pool = PgPool::connect(&database_url)
        .await
        .context("Failed to connect to database")?;

    println!("===========================================");
    println!("  Verb Diagnostics Report");
    println!("===========================================\n");

    #[derive(sqlx::FromRow)]
    struct DiagRow {
        verb_name: String,
        diagnostics_json: Option<serde_json::Value>,
    }

    let query = if errors_only {
        r#"
        SELECT verb_name, diagnostics_json
        FROM "ob-poc".dsl_verbs
        WHERE diagnostics_json IS NOT NULL
          AND jsonb_array_length(diagnostics_json->'errors') > 0
        ORDER BY verb_name
        "#
    } else {
        r#"
        SELECT verb_name, diagnostics_json
        FROM "ob-poc".dsl_verbs
        WHERE diagnostics_json IS NOT NULL
          AND (jsonb_array_length(diagnostics_json->'errors') > 0
               OR jsonb_array_length(diagnostics_json->'warnings') > 0)
        ORDER BY verb_name
        "#
    };

    let rows: Vec<DiagRow> = sqlx::query_as(query)
        .fetch_all(&pool)
        .await
        .context("Failed to query diagnostics")?;

    if rows.is_empty() {
        if errors_only {
            println!("No verbs with errors found.");
        } else {
            println!("No verbs with diagnostics found.");
        }
        return Ok(());
    }

    let mut total_errors = 0;
    let mut total_warnings = 0;

    for row in &rows {
        if let Some(ref diag_json) = row.diagnostics_json {
            let diag: VerbDiagnostics =
                serde_json::from_value(diag_json.clone()).unwrap_or_default();

            if diag.errors.is_empty() && (errors_only || diag.warnings.is_empty()) {
                continue;
            }

            println!("{}:", row.verb_name);

            for e in &diag.errors {
                println!("  ERROR [{}]: {}", e.code, e.message);
                if let Some(ref path) = e.path {
                    println!("        at: {}", path);
                }
                total_errors += 1;
            }

            if !errors_only {
                for w in &diag.warnings {
                    println!("  WARN  [{}]: {}", w.code, w.message);
                    if let Some(ref path) = w.path {
                        println!("        at: {}", path);
                    }
                    total_warnings += 1;
                }
            }

            println!();
        }
    }

    println!("-------------------------------------------");
    println!(
        "Total: {} errors, {} warnings",
        total_errors, total_warnings
    );

    Ok(())
}
