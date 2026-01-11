//! Verb contract management commands
//!
//! Commands for compiling, inspecting, and diagnosing DSL verbs.

use anyhow::{Context, Result};
use sqlx::PgPool;

use dsl_core::config::ConfigLoader;
use ob_poc::dsl_v2::RuntimeVerbRegistry;
use ob_poc::session::verb_contract::VerbDiagnostics;
use ob_poc::session::verb_sync::VerbSyncService;
use ob_poc::session::verb_tiering_linter;

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

/// Check if verb configs are up-to-date (CI check)
///
/// Compares YAML config hashes to database compiled hashes.
/// Exits with code 1 if any verbs need recompilation.
pub async fn verbs_check(verbose: bool) -> Result<()> {
    println!("===========================================");
    println!("  Verb Contract Hash Check (CI)");
    println!("===========================================\n");

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    let pool = PgPool::connect(&database_url)
        .await
        .context("Failed to connect to database")?;

    // Load verb registry from YAML and compute hashes
    println!("Loading verb registry from YAML...");
    let loader = ConfigLoader::from_env();
    let verbs_config = loader.load_verbs().context("Failed to load verb config")?;
    let registry = RuntimeVerbRegistry::from_config(&verbs_config);

    // Use VerbSyncService to compute hashes (same logic as sync_all)
    let sync_service = VerbSyncService::new(pool.clone());
    let yaml_hashes = sync_service.hash_registry(&registry);

    println!("  Found {} verbs in YAML\n", yaml_hashes.len());

    // Query database for current hashes
    println!("Checking database compiled hashes...");

    #[derive(sqlx::FromRow)]
    #[allow(dead_code)]
    struct VerbHashRow {
        full_name: String,
        yaml_hash: Option<String>,
        compiled_hash: Option<Vec<u8>>,
        compiler_version: Option<String>,
    }

    let db_verbs: Vec<VerbHashRow> = sqlx::query_as(
        r#"
        SELECT full_name, yaml_hash, compiled_hash, compiler_version
        FROM "ob-poc".dsl_verbs
        ORDER BY full_name
        "#,
    )
    .fetch_all(&pool)
    .await
    .context("Failed to query verb hashes")?;

    // Compare hashes
    // The check compares:
    // - Current YAML hash (computed from disk) vs DB yaml_hash (set during last compile)
    // - If they match, the verb config is up-to-date
    // - If they differ, YAML was modified since last compile
    let mut missing_in_db: Vec<String> = Vec::new();
    let mut hash_mismatch: Vec<(String, String, String)> = Vec::new(); // (name, current_hash, db_hash)
    let mut never_synced: Vec<String> = Vec::new();
    let mut up_to_date = 0;

    // Check verbs in YAML
    for (verb_name, current_hash) in &yaml_hashes {
        if let Some(db_verb) = db_verbs.iter().find(|v| &v.full_name == verb_name) {
            if let Some(ref db_yaml_hash) = db_verb.yaml_hash {
                if db_yaml_hash != current_hash {
                    // YAML was modified since last compile
                    hash_mismatch.push((
                        verb_name.clone(),
                        current_hash[..8].to_string(),
                        db_yaml_hash[..8].to_string(),
                    ));
                } else {
                    up_to_date += 1;
                }
            } else {
                // No yaml_hash in DB means never synced
                never_synced.push(verb_name.clone());
            }
        } else {
            missing_in_db.push(verb_name.clone());
        }
    }

    // Check for verbs in DB but not in YAML (orphaned)
    let orphaned: Vec<String> = db_verbs
        .iter()
        .filter(|v| !yaml_hashes.contains_key(&v.full_name))
        .map(|v| v.full_name.clone())
        .collect();

    // Report results
    println!("\n===========================================");
    println!("  Hash Check Results");
    println!("===========================================");
    println!("  Up-to-date:      {}", up_to_date);
    println!("  Hash mismatch:   {}", hash_mismatch.len());
    println!("  Never synced:    {}", never_synced.len());
    println!("  Missing in DB:   {}", missing_in_db.len());
    println!("  Orphaned in DB:  {}", orphaned.len());

    let has_issues =
        !hash_mismatch.is_empty() || !never_synced.is_empty() || !missing_in_db.is_empty();

    if has_issues || verbose {
        if !hash_mismatch.is_empty() {
            println!("\n--- Hash Mismatches (YAML changed since compile) ---");
            for (name, yaml, db) in &hash_mismatch {
                println!("  {} : yaml={} db={}", name, yaml, db);
            }
        }

        if !never_synced.is_empty() {
            println!("\n--- Never Synced (no yaml_hash in DB) ---");
            for name in &never_synced {
                println!("  {}", name);
            }
        }

        if !missing_in_db.is_empty() {
            println!("\n--- Missing in Database ---");
            for name in &missing_in_db {
                println!("  {}", name);
            }
        }

        if !orphaned.is_empty() && verbose {
            println!("\n--- Orphaned in Database (not in YAML) ---");
            for name in &orphaned {
                println!("  {}", name);
            }
        }
    }

    if has_issues {
        println!("\n===========================================");
        println!("  FAILED: Verbs need recompilation");
        println!("  Run: cargo x verbs compile");
        println!("===========================================");
        std::process::exit(1);
    } else {
        println!("\n  All verb contracts are up-to-date.");
    }

    Ok(())
}

/// Lint verbs for tiering rule compliance
///
/// Validates verb metadata against tiering rules:
/// - Projection verbs must be internal
/// - Intent verbs cannot write to operational tables
/// - Diagnostics verbs must be read-only
/// - etc.
pub async fn verbs_lint(errors_only: bool, verbose: bool) -> Result<()> {
    println!("===========================================");
    println!("  Verb Tiering Lint Report");
    println!("===========================================\n");

    // Load verb config from YAML
    println!("Loading verb definitions from YAML...");
    let loader = ConfigLoader::from_env();
    let verbs_config = loader.load_verbs().context("Failed to load verb config")?;

    // Run the tiering linter
    let report = verb_tiering_linter::lint_all_verbs(&verbs_config.domains);

    println!("  Scanned {} verbs\n", report.total_verbs);

    // Print summary
    println!("===========================================");
    println!("  Summary");
    println!("===========================================");
    println!("  Total verbs:          {}", report.total_verbs);
    println!("  Verbs with errors:    {}", report.verbs_with_errors);
    println!("  Verbs with warnings:  {}", report.verbs_with_warnings);
    println!("  Missing metadata:     {}", report.verbs_missing_metadata);

    // Print details
    let issues = report.issues_only();
    if !issues.is_empty() {
        println!("\n===========================================");
        println!("  Issues");
        println!("===========================================\n");

        for result in issues {
            if errors_only && !result.has_errors() {
                continue;
            }

            println!("{}:", result.full_name);

            for error in &result.diagnostics.errors {
                println!("  ERROR [{}]: {}", error.code, error.message);
                if let Some(ref path) = error.path {
                    println!("        at: {}", path);
                }
                if let Some(ref hint) = error.hint {
                    println!("        hint: {}", hint);
                }
            }

            if !errors_only {
                for warning in &result.diagnostics.warnings {
                    println!("  WARN  [{}]: {}", warning.code, warning.message);
                    if let Some(ref path) = warning.path {
                        println!("        at: {}", path);
                    }
                    if verbose {
                        if let Some(ref hint) = warning.hint {
                            println!("        hint: {}", hint);
                        }
                    }
                }
            }

            println!();
        }
    } else {
        println!("\n  No tiering issues found.");
    }

    // Print tier distribution if verbose
    if verbose {
        println!("\n===========================================");
        println!("  Tier Distribution");
        println!("===========================================");

        let mut tier_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut source_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for (domain_name, domain_config) in &verbs_config.domains {
            for (verb_name, verb_config) in &domain_config.verbs {
                if let Some(ref metadata) = verb_config.metadata {
                    let tier_name = metadata
                        .tier
                        .as_ref()
                        .map(|t| format!("{:?}", t))
                        .unwrap_or_else(|| "unset".to_string());
                    *tier_counts.entry(tier_name).or_insert(0) += 1;

                    let source_name = metadata
                        .source_of_truth
                        .as_ref()
                        .map(|s| format!("{:?}", s))
                        .unwrap_or_else(|| "unset".to_string());
                    *source_counts.entry(source_name).or_insert(0) += 1;
                } else {
                    *tier_counts.entry("no_metadata".to_string()).or_insert(0) += 1;
                    *source_counts.entry("no_metadata".to_string()).or_insert(0) += 1;
                }

                // Suppress unused variable warnings
                let _ = (domain_name, verb_name);
            }
        }

        println!("\n  By Tier:");
        let mut tiers: Vec<_> = tier_counts.into_iter().collect();
        tiers.sort_by(|a, b| b.1.cmp(&a.1));
        for (tier, count) in tiers {
            println!("    {:15} {}", tier, count);
        }

        println!("\n  By Source of Truth:");
        let mut sources: Vec<_> = source_counts.into_iter().collect();
        sources.sort_by(|a, b| b.1.cmp(&a.1));
        for (source, count) in sources {
            println!("    {:15} {}", source, count);
        }
    }

    // Exit with error code if there are errors
    if report.has_errors() {
        println!("\n===========================================");
        println!(
            "  FAILED: {} verbs have tiering errors",
            report.verbs_with_errors
        );
        println!("===========================================");
        std::process::exit(1);
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
