//! Verb contract management commands
//!
//! Commands for compiling, inspecting, and diagnosing DSL verbs.

use anyhow::{Context, Result};
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write;
use std::path::PathBuf;

use dsl_core::config::types::{SourceOfTruth, VerbBehavior, VerbScope, VerbTier};
use dsl_core::config::ConfigLoader;
use ob_poc::domain_ops::CustomOperationRegistry;
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
pub async fn verbs_lint(errors_only: bool, verbose: bool, tier: &str) -> Result<()> {
    // Parse lint tier
    let lint_tier: verb_tiering_linter::LintTier =
        tier.parse().map_err(|e| anyhow::anyhow!("{}", e))?;

    println!("===========================================");
    println!("  Verb Tiering Lint Report (tier: {})", lint_tier);
    println!("===========================================\n");

    // Load verb config from YAML
    println!("Loading verb definitions from YAML...");
    let loader = ConfigLoader::from_env();
    let verbs_config = loader.load_verbs().context("Failed to load verb config")?;

    // Run the tiering linter with specified tier
    let config = verb_tiering_linter::LintConfig {
        tier: lint_tier,
        fail_on_warning: false,
        issues_only: false,
    };
    let report = verb_tiering_linter::lint_all_verbs_with_config(&verbs_config.domains, &config);

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

/// Generate verb inventory report
///
/// Creates a comprehensive markdown report of all verbs grouped by domain, tier, and noun.
pub fn verbs_inventory(
    output: Option<PathBuf>,
    update_claude_md: bool,
    show_untagged: bool,
) -> Result<()> {
    use chrono::Utc;
    use std::fmt::Write;
    use std::fs;

    println!("===========================================");
    println!("  Verb Inventory Generation");
    println!("===========================================\n");

    // Load verb config from YAML
    println!("Loading verb definitions from YAML...");
    let loader = ConfigLoader::from_env();
    let verbs_config = loader.load_verbs().context("Failed to load verb config")?;

    // Collect statistics
    let mut total_verbs = 0;
    let mut verbs_with_metadata = 0;
    let mut tier_counts: HashMap<String, usize> = HashMap::new();
    let mut source_counts: HashMap<String, usize> = HashMap::new();
    let mut scope_counts: HashMap<String, usize> = HashMap::new();
    let mut noun_counts: HashMap<String, usize> = HashMap::new();
    let mut domain_counts: HashMap<String, usize> = HashMap::new();
    let mut untagged_verbs: Vec<(String, String)> = Vec::new(); // (domain, verb_name)

    // Domain verb details for the report
    #[allow(dead_code)]
    struct VerbInfo {
        name: String,
        description: String,
        tier: Option<VerbTier>,
        source: Option<SourceOfTruth>,
        scope: Option<VerbScope>,
        noun: Option<String>,
        internal: bool,
    }

    let mut domain_verbs: HashMap<String, Vec<VerbInfo>> = HashMap::new();

    for (domain_name, domain_config) in &verbs_config.domains {
        let mut verbs: Vec<VerbInfo> = Vec::new();

        for (verb_name, verb_config) in &domain_config.verbs {
            total_verbs += 1;
            *domain_counts.entry(domain_name.clone()).or_insert(0) += 1;

            let info = if let Some(ref metadata) = verb_config.metadata {
                verbs_with_metadata += 1;

                let tier_name = metadata
                    .tier
                    .as_ref()
                    .map(|t| format!("{:?}", t).to_lowercase())
                    .unwrap_or_else(|| "unset".to_string());
                *tier_counts.entry(tier_name).or_insert(0) += 1;

                let source_name = metadata
                    .source_of_truth
                    .as_ref()
                    .map(|s| format!("{:?}", s).to_lowercase())
                    .unwrap_or_else(|| "unset".to_string());
                *source_counts.entry(source_name).or_insert(0) += 1;

                let scope_name = metadata
                    .scope
                    .as_ref()
                    .map(|s| format!("{:?}", s).to_lowercase())
                    .unwrap_or_else(|| "unset".to_string());
                *scope_counts.entry(scope_name).or_insert(0) += 1;

                if let Some(ref noun) = metadata.noun {
                    *noun_counts.entry(noun.clone()).or_insert(0) += 1;
                }

                VerbInfo {
                    name: verb_name.clone(),
                    description: verb_config.description.clone(),
                    tier: metadata.tier,
                    source: metadata.source_of_truth,
                    scope: metadata.scope,
                    noun: metadata.noun.clone(),
                    internal: metadata.internal,
                }
            } else {
                untagged_verbs.push((domain_name.clone(), verb_name.clone()));
                *tier_counts.entry("no_metadata".to_string()).or_insert(0) += 1;

                VerbInfo {
                    name: verb_name.clone(),
                    description: verb_config.description.clone(),
                    tier: None,
                    source: None,
                    scope: None,
                    noun: None,
                    internal: false,
                }
            };

            verbs.push(info);
        }

        // Sort verbs by name
        verbs.sort_by(|a, b| a.name.cmp(&b.name));
        domain_verbs.insert(domain_name.clone(), verbs);
    }

    // Print summary
    println!("  Total verbs:        {}", total_verbs);
    println!("  With metadata:      {}", verbs_with_metadata);
    println!(
        "  Missing metadata:   {}",
        total_verbs - verbs_with_metadata
    );
    println!("  Domains:            {}", domain_counts.len());

    // Show untagged if requested
    if show_untagged && !untagged_verbs.is_empty() {
        println!("\n--- Verbs Missing Metadata ---");
        for (domain, verb) in &untagged_verbs {
            println!("  {}.{}", domain, verb);
        }
    }

    // Generate markdown report
    let mut md = String::new();
    let now = Utc::now();

    writeln!(md, "# Verb Inventory\n").unwrap();
    writeln!(md, "> Auto-generated by `cargo x verbs inventory`").unwrap();
    writeln!(md, "> Generated: {}\n", now.format("%Y-%m-%d %H:%M UTC")).unwrap();

    // Summary section
    writeln!(md, "## Summary\n").unwrap();
    writeln!(md, "| Metric | Count |").unwrap();
    writeln!(md, "|--------|-------|").unwrap();
    writeln!(md, "| Total verbs | {} |", total_verbs).unwrap();
    writeln!(md, "| With metadata | {} |", verbs_with_metadata).unwrap();
    writeln!(
        md,
        "| Missing metadata | {} |",
        total_verbs - verbs_with_metadata
    )
    .unwrap();
    writeln!(md, "| Domains | {} |", domain_counts.len()).unwrap();

    // Tier distribution
    writeln!(md, "\n## Tier Distribution\n").unwrap();
    writeln!(md, "| Tier | Count |").unwrap();
    writeln!(md, "|------|-------|").unwrap();
    let mut tiers: Vec<_> = tier_counts.iter().collect();
    tiers.sort_by(|a, b| b.1.cmp(a.1));
    for (tier, count) in tiers {
        writeln!(md, "| {} | {} |", tier, count).unwrap();
    }

    // Source of truth distribution
    writeln!(md, "\n## Source of Truth Distribution\n").unwrap();
    writeln!(md, "| Source | Count |").unwrap();
    writeln!(md, "|--------|-------|").unwrap();
    let mut sources: Vec<_> = source_counts.iter().collect();
    sources.sort_by(|a, b| b.1.cmp(a.1));
    for (source, count) in sources {
        writeln!(md, "| {} | {} |", source, count).unwrap();
    }

    // Domain summary
    writeln!(md, "\n## Domain Summary\n").unwrap();
    writeln!(md, "| Domain | Verbs |").unwrap();
    writeln!(md, "|--------|-------|").unwrap();
    let mut domains: Vec<_> = domain_counts.iter().collect();
    domains.sort_by(|a, b| b.1.cmp(a.1));
    for (domain, count) in domains {
        writeln!(md, "| {} | {} |", domain, count).unwrap();
    }

    // Noun distribution (top 20)
    writeln!(md, "\n## Top Nouns\n").unwrap();
    writeln!(md, "| Noun | Count |").unwrap();
    writeln!(md, "|------|-------|").unwrap();
    let mut nouns: Vec<_> = noun_counts.iter().collect();
    nouns.sort_by(|a, b| b.1.cmp(a.1));
    for (noun, count) in nouns.iter().take(20) {
        writeln!(md, "| {} | {} |", noun, count).unwrap();
    }

    // Detailed domain sections
    writeln!(md, "\n---\n").unwrap();
    writeln!(md, "## Verbs by Domain\n").unwrap();

    let mut sorted_domains: Vec<_> = domain_verbs.keys().collect();
    sorted_domains.sort();

    for domain_name in sorted_domains {
        let verbs = domain_verbs.get(domain_name).unwrap();
        writeln!(md, "### {}\n", domain_name).unwrap();
        writeln!(md, "| Verb | Tier | Source | Description |").unwrap();
        writeln!(md, "|------|------|--------|-------------|").unwrap();

        for verb in verbs {
            let tier_str = verb
                .tier
                .as_ref()
                .map(|t| format!("{:?}", t).to_lowercase())
                .unwrap_or_else(|| "-".to_string());
            let source_str = verb
                .source
                .as_ref()
                .map(|s| format!("{:?}", s).to_lowercase())
                .unwrap_or_else(|| "-".to_string());
            let internal_marker = if verb.internal { " (internal)" } else { "" };
            let desc = verb.description.replace('|', "\\|");
            let desc_short = if desc.len() > 60 {
                format!("{}...", &desc[..57])
            } else {
                desc
            };

            writeln!(
                md,
                "| {}{} | {} | {} | {} |",
                verb.name, internal_marker, tier_str, source_str, desc_short
            )
            .unwrap();
        }
        writeln!(md).unwrap();
    }

    // Write output file
    let output_path = output.unwrap_or_else(|| PathBuf::from("docs/verb-inventory.md"));

    // Ensure parent directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).context("Failed to create output directory")?;
    }

    fs::write(&output_path, &md).context("Failed to write inventory file")?;
    println!("\nInventory written to: {}", output_path.display());

    // Update CLAUDE.md if requested
    if update_claude_md {
        update_claude_md_stats(total_verbs, domain_counts.len())?;
    }

    Ok(())
}

/// Update CLAUDE.md header stats with verb count
fn update_claude_md_stats(verb_count: usize, file_count: usize) -> Result<()> {
    use regex::Regex;
    use std::fs;

    // CLAUDE.md is in the project root (one level up from rust/)
    let claude_md_path = PathBuf::from("../CLAUDE.md");
    if !claude_md_path.exists() {
        println!(
            "CLAUDE.md not found at {:?}, skipping update",
            claude_md_path
        );
        return Ok(());
    }

    let content = fs::read_to_string(&claude_md_path).context("Failed to read CLAUDE.md")?;

    // Update verb count line
    let verb_re = Regex::new(r#">\s*\*\*Verb count:\*\*[^\n]+"#).unwrap();
    let updated = verb_re.replace(
        &content,
        format!(
            "> **Verb count:** ~{} verbs across {}+ YAML files",
            verb_count, file_count
        ),
    );

    if updated != content {
        fs::write(&claude_md_path, updated.as_ref()).context("Failed to write CLAUDE.md")?;
        println!("Updated CLAUDE.md verb count: ~{} verbs", verb_count);
    } else {
        println!("CLAUDE.md verb count already up-to-date");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Verb Atlas
// ---------------------------------------------------------------------------

/// Severity for atlas findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "UPPERCASE")]
enum Severity {
    Error,
    Warn,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "ERROR"),
            Severity::Warn => write!(f, "WARN"),
            Severity::Info => write!(f, "INFO"),
        }
    }
}

/// A single finding from the atlas lint pass.
#[derive(Debug, Clone, serde::Serialize)]
struct Finding {
    code: String,
    severity: Severity,
    verb_fqn: String,
    message: String,
}

/// One row in the atlas table — one per verb FQN.
#[derive(Debug, Clone, serde::Serialize)]
struct AtlasRow {
    fqn: String,
    domain: String,
    action: String,
    tier: Option<String>,
    source_of_truth: Option<String>,
    scope: Option<String>,
    behavior: String,
    description: String,
    phrase_count: usize,
    phrases: Vec<String>,
    pack_membership: Vec<String>,
    pack_forbidden: Vec<String>,
    handler_exists: bool,
    in_lexicon: bool,
    has_preconditions: bool,
    has_lifecycle: bool,
    internal: bool,
    dangerous: bool,
    status: String,
}

/// Collision between two or more verbs sharing the same normalised phrase.
#[derive(Debug, Clone, serde::Serialize)]
struct PhraseCollision {
    normalized_phrase: String,
    verbs: Vec<String>,
    collision_type: String, // "EXACT" or "NEAR"
}

/// Normalize a phrase for collision detection: lowercase, trim, collapse whitespace, strip punctuation.
fn normalize_phrase(phrase: &str) -> String {
    let lower = phrase.to_lowercase();
    let cleaned: String = lower
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Truncate a string to at most `max_chars` characters (Unicode-safe), appending "..." if truncated.
fn truncate_utf8(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}

/// Jaccard similarity between two token sets.
fn jaccard_similarity(a: &HashSet<&str>, b: &HashSet<&str>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let intersection = a.intersection(b).count();
    let union = a.union(b).count();
    if union == 0 {
        return 0.0;
    }
    intersection as f64 / union as f64
}

/// Load pack manifests from YAML files.
fn load_packs(config_dir: &std::path::Path) -> Result<Vec<PackInfo>> {
    let packs_dir = config_dir.join("packs");
    if !packs_dir.exists() {
        return Ok(Vec::new());
    }

    let mut packs = Vec::new();
    for entry in std::fs::read_dir(&packs_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .map(|e| e == "yaml" || e == "yml")
            .unwrap_or(false)
        {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read pack: {}", path.display()))?;
            let pack: PackInfo = serde_yaml::from_str(&content)
                .with_context(|| format!("Failed to parse pack: {}", path.display()))?;
            packs.push(pack);
        }
    }
    Ok(packs)
}

/// Minimal pack info for atlas (avoids pulling in full PackManifest).
#[derive(Debug, Clone, serde::Deserialize)]
struct PackInfo {
    id: String,
    #[serde(default)]
    allowed_verbs: Vec<String>,
    #[serde(default)]
    forbidden_verbs: Vec<String>,
}

/// Load verb concepts from the lexicon YAML.
fn load_verb_concepts(config_dir: &std::path::Path) -> Result<HashSet<String>> {
    let path = config_dir.join("lexicon/verb_concepts.yaml");
    if !path.exists() {
        return Ok(HashSet::new());
    }
    let content = std::fs::read_to_string(&path)?;
    let map: BTreeMap<String, serde_yaml::Value> = serde_yaml::from_str(&content)?;
    Ok(map.keys().cloned().collect())
}

/// Control-plane verb prefixes that should not appear in business pack allowed_verbs.
const CONTROL_PREFIXES: &[&str] = &["session.", "runbook.", "agent.", "view.", "nav."];

/// Generate the verb atlas.
pub fn verbs_atlas(output_dir: Option<PathBuf>, lint_only: bool, verbose: bool) -> Result<()> {
    println!("===========================================");
    println!("  Verb Atlas Generator");
    println!("===========================================\n");

    // -----------------------------------------------------------------------
    // 1. Load all verb data
    // -----------------------------------------------------------------------
    println!("Loading verb definitions from YAML...");
    let loader = ConfigLoader::from_env();
    let verbs_config = loader.load_verbs().context("Failed to load verb config")?;

    // Resolve config directory for packs / lexicon
    let config_dir = resolve_config_dir();

    // Load packs
    println!("Loading pack manifests...");
    let packs = load_packs(&config_dir)?;
    println!("  Found {} pack(s)", packs.len());

    // Load lexicon verb concepts
    println!("Loading verb concepts lexicon...");
    let lexicon_verbs = load_verb_concepts(&config_dir)?;
    println!("  Found {} concept entries", lexicon_verbs.len());

    // Build handler registry (inventory-based)
    println!("Building handler registry...");
    let custom_ops = CustomOperationRegistry::new();
    let handler_list: HashSet<String> = custom_ops
        .list()
        .iter()
        .map(|(domain, verb, _)| format!("{}.{}", domain, verb))
        .collect();
    println!("  Found {} registered handlers", handler_list.len());

    // Build pack membership indices
    let mut pack_allowed: HashMap<String, Vec<String>> = HashMap::new(); // verb_fqn → pack_ids
    let mut pack_forbidden: HashMap<String, Vec<String>> = HashMap::new();
    for pack in &packs {
        for verb in &pack.allowed_verbs {
            pack_allowed
                .entry(verb.clone())
                .or_default()
                .push(pack.id.clone());
        }
        for verb in &pack.forbidden_verbs {
            pack_forbidden
                .entry(verb.clone())
                .or_default()
                .push(pack.id.clone());
        }
    }

    // -----------------------------------------------------------------------
    // 2. Build atlas rows
    // -----------------------------------------------------------------------
    println!("\nBuilding atlas rows...");
    let mut rows: Vec<AtlasRow> = Vec::new();
    let mut all_findings: Vec<Finding> = Vec::new();
    let mut all_collisions: Vec<PhraseCollision> = Vec::new();

    // Phrase → list of verb FQNs (for collision detection)
    let mut phrase_to_verbs: HashMap<String, Vec<String>> = HashMap::new();

    for (domain_name, domain_config) in &verbs_config.domains {
        for (verb_name, verb_config) in &domain_config.verbs {
            let fqn = format!("{}.{}", domain_name, verb_name);

            let (
                tier_str,
                source_str,
                scope_str,
                internal,
                dangerous,
                status_str,
                has_preconditions,
                has_lifecycle,
            ) = if let Some(ref meta) = verb_config.metadata {
                (
                    meta.tier
                        .as_ref()
                        .map(|t| format!("{:?}", t).to_lowercase()),
                    meta.source_of_truth
                        .as_ref()
                        .map(|s| format!("{:?}", s).to_lowercase()),
                    meta.scope
                        .as_ref()
                        .map(|s| format!("{:?}", s).to_lowercase()),
                    meta.internal,
                    meta.dangerous,
                    format!("{:?}", meta.status).to_lowercase(),
                    !verb_config
                        .lifecycle
                        .as_ref()
                        .map(|l| l.precondition_checks.is_empty())
                        .unwrap_or(true),
                    verb_config.lifecycle.is_some(),
                )
            } else {
                (
                    None,
                    None,
                    None,
                    false,
                    false,
                    "active".to_string(),
                    false,
                    false,
                )
            };

            let behavior_str = match verb_config.behavior {
                VerbBehavior::Crud => "crud",
                VerbBehavior::Plugin => "plugin",
                VerbBehavior::GraphQuery => "graph_query",
            }
            .to_string();

            let handler_exists = match verb_config.behavior {
                VerbBehavior::Plugin => handler_list.contains(&fqn),
                _ => true, // CRUD and GraphQuery don't need custom handlers
            };

            // Index phrases for collision detection
            for phrase in &verb_config.invocation_phrases {
                let norm = normalize_phrase(phrase);
                if !norm.is_empty() {
                    phrase_to_verbs.entry(norm).or_default().push(fqn.clone());
                }
            }

            let row = AtlasRow {
                fqn: fqn.clone(),
                domain: domain_name.clone(),
                action: verb_name.clone(),
                tier: tier_str,
                source_of_truth: source_str,
                scope: scope_str,
                behavior: behavior_str,
                description: verb_config.description.clone(),
                phrase_count: verb_config.invocation_phrases.len(),
                phrases: verb_config.invocation_phrases.clone(),
                pack_membership: pack_allowed.get(&fqn).cloned().unwrap_or_default(),
                pack_forbidden: pack_forbidden.get(&fqn).cloned().unwrap_or_default(),
                handler_exists,
                in_lexicon: lexicon_verbs.contains(&fqn),
                has_preconditions,
                has_lifecycle,
                internal,
                dangerous,
                status: status_str,
            };
            rows.push(row);
        }
    }

    rows.sort_by(|a, b| a.fqn.cmp(&b.fqn));
    println!("  Built {} atlas rows", rows.len());

    // -----------------------------------------------------------------------
    // 3. Collision detection
    // -----------------------------------------------------------------------
    println!("Running collision detection...");

    // Build verb→domain lookup for collision severity
    let verb_domain: HashMap<&str, &str> = rows
        .iter()
        .map(|r| (r.fqn.as_str(), r.domain.as_str()))
        .collect();

    // 3a. Exact collisions: same normalized phrase → multiple verbs
    // Same-domain collisions are ERROR (must fix), cross-domain collisions are WARN
    // (acceptable — pack/scope scoring separates them).
    for (norm_phrase, verbs) in &phrase_to_verbs {
        // Deduplicate verb FQNs (same verb can register the same phrase multiple times)
        let unique_verbs: BTreeSet<_> = verbs.iter().collect();
        if unique_verbs.len() > 1 {
            // Determine if all colliding verbs are in the same domain
            let domains: BTreeSet<_> = unique_verbs
                .iter()
                .filter_map(|v| verb_domain.get(v.as_str()).copied())
                .collect();
            let same_domain = domains.len() == 1;

            all_collisions.push(PhraseCollision {
                normalized_phrase: norm_phrase.clone(),
                verbs: unique_verbs.into_iter().cloned().collect(),
                collision_type: "EXACT".to_string(),
            });

            for verb_fqn in verbs.iter().collect::<BTreeSet<_>>() {
                all_findings.push(Finding {
                    code: "COLLISION".to_string(),
                    severity: if same_domain {
                        Severity::Error
                    } else {
                        Severity::Warn
                    },
                    verb_fqn: verb_fqn.clone(),
                    message: format!(
                        "Exact phrase collision on \"{}\" — shared with: {}{}",
                        norm_phrase,
                        verbs
                            .iter()
                            .filter(|v| v.as_str() != verb_fqn.as_str())
                            .map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                        if same_domain {
                            ""
                        } else {
                            " (cross-domain, scope-separated)"
                        }
                    ),
                });
            }
        }
    }

    // 3b. Near-collisions: Jaccard > 0.80 between verbs in same domain or pack
    {
        // Build per-verb normalized phrase token sets (owned strings for lifetime safety)
        let verb_phrase_sets: BTreeMap<String, Vec<(String, HashSet<String>)>> = rows
            .iter()
            .map(|r| {
                let sets: Vec<(String, HashSet<String>)> = r
                    .phrases
                    .iter()
                    .map(|p| {
                        let norm = normalize_phrase(p);
                        let tokens: HashSet<String> =
                            norm.split_whitespace().map(String::from).collect();
                        (norm, tokens)
                    })
                    .collect();
                (r.fqn.clone(), sets)
            })
            .collect();

        // Group verbs by domain
        let mut domain_groups: HashMap<String, Vec<String>> = HashMap::new();
        for row in &rows {
            domain_groups
                .entry(row.domain.clone())
                .or_default()
                .push(row.fqn.clone());
        }

        let mut near_collision_set: HashSet<(String, String)> = HashSet::new();

        for verbs_in_group in domain_groups.values() {
            for i in 0..verbs_in_group.len() {
                for j in (i + 1)..verbs_in_group.len() {
                    let v1 = &verbs_in_group[i];
                    let v2 = &verbs_in_group[j];
                    let sets1 = verb_phrase_sets.get(v1);
                    let sets2 = verb_phrase_sets.get(v2);

                    if let (Some(s1), Some(s2)) = (sets1, sets2) {
                        for (norm1, tokens1) in s1 {
                            for (norm2, tokens2) in s2 {
                                if norm1 == norm2 {
                                    continue; // Already caught as exact collision
                                }
                                let t1_ref: HashSet<&str> =
                                    tokens1.iter().map(|s| s.as_str()).collect();
                                let t2_ref: HashSet<&str> =
                                    tokens2.iter().map(|s| s.as_str()).collect();
                                let sim = jaccard_similarity(&t1_ref, &t2_ref);
                                if sim > 0.80 {
                                    let key = if v1 < v2 {
                                        (v1.clone(), v2.clone())
                                    } else {
                                        (v2.clone(), v1.clone())
                                    };
                                    if near_collision_set.insert(key.clone()) {
                                        all_collisions.push(PhraseCollision {
                                            normalized_phrase: format!(
                                                "\"{}\" ↔ \"{}\" (Jaccard={:.2})",
                                                norm1, norm2, sim
                                            ),
                                            verbs: vec![v1.clone(), v2.clone()],
                                            collision_type: "NEAR".to_string(),
                                        });
                                        all_findings.push(Finding {
                                            code: "NEAR_COLLISION".to_string(),
                                            severity: Severity::Warn,
                                            verb_fqn: v1.clone(),
                                            message: format!(
                                                "Near-collision with {} — \"{}\" ↔ \"{}\" (Jaccard={:.2})",
                                                v2, norm1, norm2, sim
                                            ),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    println!(
        "  Found {} exact collisions, {} near-collisions",
        all_collisions
            .iter()
            .filter(|c| c.collision_type == "EXACT")
            .count(),
        all_collisions
            .iter()
            .filter(|c| c.collision_type == "NEAR")
            .count(),
    );

    // -----------------------------------------------------------------------
    // 4. Lint checks
    // -----------------------------------------------------------------------
    println!("Running lint checks...");

    for row in &rows {
        // GHOST_VERB: tier=intent + 0 phrases + not internal
        if row.tier.as_deref() == Some("intent") && row.phrase_count == 0 && !row.internal {
            all_findings.push(Finding {
                code: "GHOST_VERB".to_string(),
                severity: Severity::Error,
                verb_fqn: row.fqn.clone(),
                message: "Intent verb with 0 invocation phrases — invisible to semantic search"
                    .to_string(),
            });
        }

        // MISSING_TIER: no tier at all
        if row.tier.is_none() {
            all_findings.push(Finding {
                code: "MISSING_TIER".to_string(),
                severity: Severity::Error,
                verb_fqn: row.fqn.clone(),
                message: "No metadata.tier set".to_string(),
            });
        }

        // CONTROL_IN_PACK: control-plane verb in a business pack's allowed_verbs
        // Exclude session.load-* (scoping verbs legitimately used in business packs)
        // and packs whose id starts with "session" (dedicated session packs).
        if !row.pack_membership.is_empty() {
            let is_control = CONTROL_PREFIXES.iter().any(|p| row.fqn.starts_with(p));
            let is_scoping_verb = row.fqn.starts_with("session.load-");
            let only_session_packs = row.pack_membership.iter().all(|p| p.starts_with("session"));
            if is_control && !is_scoping_verb && !only_session_packs {
                all_findings.push(Finding {
                    code: "CONTROL_IN_PACK".to_string(),
                    severity: Severity::Error,
                    verb_fqn: row.fqn.clone(),
                    message: format!(
                        "Control-plane verb in business pack allowed_verbs: [{}]",
                        row.pack_membership.join(", ")
                    ),
                });
            }
        }

        // NO_HANDLER: plugin behavior but no handler found
        if row.behavior == "plugin" && !row.handler_exists {
            all_findings.push(Finding {
                code: "NO_HANDLER".to_string(),
                severity: Severity::Error,
                verb_fqn: row.fqn.clone(),
                message: "Plugin verb with no registered CustomOperation handler".to_string(),
            });
        }

        // MISSING_CONCEPT: intent verb not in verb_concepts lexicon
        if row.tier.as_deref() == Some("intent") && !row.in_lexicon && !row.internal {
            all_findings.push(Finding {
                code: "MISSING_CONCEPT".to_string(),
                severity: Severity::Warn,
                verb_fqn: row.fqn.clone(),
                message: "Intent verb not in verb_concepts.yaml lexicon".to_string(),
            });
        }

        // DEAD_VERB: no handler + no pack membership + status active + not internal
        if !row.handler_exists
            && row.pack_membership.is_empty()
            && row.status == "active"
            && !row.internal
            && row.behavior == "plugin"
        {
            all_findings.push(Finding {
                code: "DEAD_VERB".to_string(),
                severity: Severity::Warn,
                verb_fqn: row.fqn.clone(),
                message: "No handler, no pack membership — delete candidate".to_string(),
            });
        }

        // MISSING_PRECONDITIONS: intent verb with lifecycle but no precondition_checks
        if row.tier.as_deref() == Some("intent")
            && row.has_lifecycle
            && !row.has_preconditions
            && !row.internal
        {
            all_findings.push(Finding {
                code: "MISSING_PRECONDITIONS".to_string(),
                severity: Severity::Info,
                verb_fqn: row.fqn.clone(),
                message: "Intent verb with lifecycle block but no precondition_checks".to_string(),
            });
        }
    }

    // ORPHAN_CONCEPT: lexicon entry maps to nonexistent verb
    let all_fqns: HashSet<String> = rows.iter().map(|r| r.fqn.clone()).collect();
    for concept_fqn in &lexicon_verbs {
        if !all_fqns.contains(concept_fqn) {
            all_findings.push(Finding {
                code: "ORPHAN_CONCEPT".to_string(),
                severity: Severity::Warn,
                verb_fqn: concept_fqn.clone(),
                message: "Lexicon verb_concepts entry maps to nonexistent verb".to_string(),
            });
        }
    }

    // Sort findings by severity (errors first), then by verb_fqn
    all_findings.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then(a.verb_fqn.cmp(&b.verb_fqn))
    });

    let error_count = all_findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warn_count = all_findings
        .iter()
        .filter(|f| f.severity == Severity::Warn)
        .count();
    let info_count = all_findings
        .iter()
        .filter(|f| f.severity == Severity::Info)
        .count();

    println!(
        "  {} ERRORs, {} WARNs, {} INFOs",
        error_count, warn_count, info_count
    );

    // -----------------------------------------------------------------------
    // 5. Print summary
    // -----------------------------------------------------------------------
    println!("\n===========================================");
    println!("  Atlas Summary");
    println!("===========================================");
    println!("  Total verbs:          {}", rows.len());
    println!(
        "  Total phrases:        {}",
        rows.iter().map(|r| r.phrase_count).sum::<usize>()
    );
    println!(
        "  Verbs with phrases:   {}",
        rows.iter().filter(|r| r.phrase_count > 0).count()
    );
    println!(
        "  Verbs without phrases:{}",
        rows.iter().filter(|r| r.phrase_count == 0).count()
    );
    println!(
        "  Plugin verbs:         {}",
        rows.iter().filter(|r| r.behavior == "plugin").count()
    );
    println!(
        "  CRUD verbs:           {}",
        rows.iter().filter(|r| r.behavior == "crud").count()
    );
    println!(
        "  Pack-assigned verbs:  {}",
        rows.iter()
            .filter(|r| !r.pack_membership.is_empty())
            .count()
    );
    println!(
        "  In lexicon:           {}",
        rows.iter().filter(|r| r.in_lexicon).count()
    );
    println!(
        "  Exact collisions:     {}",
        all_collisions
            .iter()
            .filter(|c| c.collision_type == "EXACT")
            .count()
    );
    println!(
        "  Near-collisions:      {}",
        all_collisions
            .iter()
            .filter(|c| c.collision_type == "NEAR")
            .count()
    );
    println!("  Findings — ERRORs:    {}", error_count);
    println!("  Findings — WARNs:     {}", warn_count);
    println!("  Findings — INFOs:     {}", info_count);

    // If lint-only, exit with non-zero on errors
    if lint_only {
        if error_count > 0 {
            println!("\n  FAILED: {} errors found", error_count);
            // Print errors only
            for f in &all_findings {
                if f.severity == Severity::Error {
                    println!(
                        "  {} [{}] {}: {}",
                        f.severity, f.code, f.verb_fqn, f.message
                    );
                }
            }
            std::process::exit(1);
        } else {
            println!("\n  PASSED: 0 errors");
            return Ok(());
        }
    }

    // -----------------------------------------------------------------------
    // 6. Write output files
    // -----------------------------------------------------------------------
    let out_dir = output_dir.unwrap_or_else(|| PathBuf::from("docs/generated"));
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("Failed to create output dir: {}", out_dir.display()))?;

    // 6a. verb_atlas.json
    let json_path = out_dir.join("verb_atlas.json");
    let json_output = serde_json::json!({
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "verb_count": rows.len(),
        "phrase_count": rows.iter().map(|r| r.phrase_count).sum::<usize>(),
        "rows": rows,
        "findings": all_findings,
        "collisions": all_collisions,
    });
    std::fs::write(&json_path, serde_json::to_string_pretty(&json_output)?)
        .context("Failed to write verb_atlas.json")?;
    println!("\n  Written: {}", json_path.display());

    // 6b. verb_atlas.md
    let md_path = out_dir.join("verb_atlas.md");
    let atlas_md = generate_atlas_md(&rows);
    std::fs::write(&md_path, &atlas_md).context("Failed to write verb_atlas.md")?;
    println!("  Written: {}", md_path.display());

    // 6c. verb_findings.md
    let findings_path = out_dir.join("verb_findings.md");
    let findings_md = generate_findings_md(&all_findings, error_count, warn_count, info_count);
    std::fs::write(&findings_path, &findings_md).context("Failed to write verb_findings.md")?;
    println!("  Written: {}", findings_path.display());

    // 6d. verb_phrase_collisions.md
    let collisions_path = out_dir.join("verb_phrase_collisions.md");
    let collisions_md = generate_collisions_md(&all_collisions);
    std::fs::write(&collisions_path, &collisions_md)
        .context("Failed to write verb_phrase_collisions.md")?;
    println!("  Written: {}", collisions_path.display());

    if verbose {
        println!("\n--- Findings Detail ---\n");
        for f in &all_findings {
            println!(
                "  {} [{}] {}: {}",
                f.severity, f.code, f.verb_fqn, f.message
            );
        }
    }

    if error_count > 0 {
        println!(
            "\n  {} errors need attention (see verb_findings.md)",
            error_count
        );
    }

    Ok(())
}

/// Resolve the config directory for packs and lexicon.
fn resolve_config_dir() -> PathBuf {
    // Try DSL_CONFIG_DIR first
    if let Ok(dir) = std::env::var("DSL_CONFIG_DIR") {
        return PathBuf::from(dir);
    }

    // Try common locations
    for candidate in &["config", "../config", "rust/config"] {
        let path = PathBuf::from(candidate);
        if path.join("verbs").exists() {
            return path;
        }
    }

    // Fallback: use CARGO_MANIFEST_DIR to find rust/config
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let path = PathBuf::from(manifest_dir);
        // xtask is in rust/xtask, so config is at rust/config
        let config_path = path.parent().unwrap_or(&path).join("config");
        if config_path.join("verbs").exists() {
            return config_path;
        }
    }

    PathBuf::from("config")
}

fn generate_atlas_md(rows: &[AtlasRow]) -> String {
    let mut md = String::new();
    let now = chrono::Utc::now();

    writeln!(md, "# Verb Atlas\n").unwrap();
    writeln!(md, "> Auto-generated by `cargo x verbs atlas`").unwrap();
    writeln!(md, "> Generated: {}", now.format("%Y-%m-%d %H:%M UTC")).unwrap();
    writeln!(md, "> Verb count: {}\n", rows.len()).unwrap();

    // Summary by domain
    let mut domain_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for row in rows {
        *domain_counts.entry(&row.domain).or_insert(0) += 1;
    }
    writeln!(md, "## Domain Summary\n").unwrap();
    writeln!(md, "| Domain | Count |").unwrap();
    writeln!(md, "|--------|-------|").unwrap();
    for (domain, count) in &domain_counts {
        writeln!(md, "| {} | {} |", domain, count).unwrap();
    }

    // Full table
    writeln!(md, "\n## Full Verb Table\n").unwrap();
    writeln!(
        md,
        "| FQN | Tier | Behavior | Phrases | Packs | Handler | Lexicon | Description |"
    )
    .unwrap();
    writeln!(
        md,
        "|-----|------|----------|---------|-------|---------|---------|-------------|"
    )
    .unwrap();

    for row in rows {
        let tier = row.tier.as_deref().unwrap_or("-");
        let packs = if row.pack_membership.is_empty() {
            "-".to_string()
        } else {
            row.pack_membership.join(", ")
        };
        let handler = if row.behavior == "plugin" {
            if row.handler_exists {
                "yes"
            } else {
                "**NO**"
            }
        } else {
            "n/a"
        };
        let lexicon = if row.in_lexicon { "yes" } else { "-" };
        let desc = row.description.replace('|', "\\|");
        let desc_short = truncate_utf8(&desc, 50);

        writeln!(
            md,
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            row.fqn, tier, row.behavior, row.phrase_count, packs, handler, lexicon, desc_short
        )
        .unwrap();
    }

    md
}

fn generate_findings_md(
    findings: &[Finding],
    error_count: usize,
    warn_count: usize,
    info_count: usize,
) -> String {
    let mut md = String::new();
    let now = chrono::Utc::now();

    writeln!(md, "# Verb Atlas Findings\n").unwrap();
    writeln!(md, "> Auto-generated by `cargo x verbs atlas`").unwrap();
    writeln!(md, "> Generated: {}", now.format("%Y-%m-%d %H:%M UTC")).unwrap();
    writeln!(
        md,
        "> {} ERRORs, {} WARNs, {} INFOs\n",
        error_count, warn_count, info_count
    )
    .unwrap();

    if findings.is_empty() {
        writeln!(md, "No findings. All verbs are clean.").unwrap();
        return md;
    }

    // Group findings by code
    let mut by_code: BTreeMap<&str, Vec<&Finding>> = BTreeMap::new();
    for f in findings {
        by_code.entry(&f.code).or_default().push(f);
    }

    for (code, items) in &by_code {
        let severity = items[0].severity;
        writeln!(md, "## {} — {} ({} items)\n", code, severity, items.len()).unwrap();

        writeln!(md, "| Verb | Message |").unwrap();
        writeln!(md, "|------|---------|").unwrap();
        for f in items {
            writeln!(md, "| {} | {} |", f.verb_fqn, f.message.replace('|', "\\|")).unwrap();
        }
        writeln!(md).unwrap();
    }

    md
}

fn generate_collisions_md(collisions: &[PhraseCollision]) -> String {
    let mut md = String::new();
    let now = chrono::Utc::now();

    writeln!(md, "# Verb Phrase Collisions\n").unwrap();
    writeln!(md, "> Auto-generated by `cargo x verbs atlas`").unwrap();
    writeln!(md, "> Generated: {}\n", now.format("%Y-%m-%d %H:%M UTC")).unwrap();

    let exact: Vec<_> = collisions
        .iter()
        .filter(|c| c.collision_type == "EXACT")
        .collect();
    let near: Vec<_> = collisions
        .iter()
        .filter(|c| c.collision_type == "NEAR")
        .collect();

    writeln!(md, "## Summary\n").unwrap();
    writeln!(md, "- Exact collisions: {}", exact.len()).unwrap();
    writeln!(md, "- Near-collisions: {}\n", near.len()).unwrap();

    if !exact.is_empty() {
        writeln!(md, "## Exact Collisions\n").unwrap();
        writeln!(
            md,
            "Two or more verbs share the exact same normalized phrase.\n"
        )
        .unwrap();
        writeln!(md, "| Phrase | Verbs |").unwrap();
        writeln!(md, "|--------|-------|").unwrap();
        for c in &exact {
            writeln!(md, "| {} | {} |", c.normalized_phrase, c.verbs.join(", ")).unwrap();
        }
        writeln!(md).unwrap();
    }

    if !near.is_empty() {
        writeln!(md, "## Near-Collisions (Jaccard > 0.80)\n").unwrap();
        writeln!(
            md,
            "Verb pairs within the same domain with highly similar phrases.\n"
        )
        .unwrap();
        writeln!(md, "| Comparison | Verbs |").unwrap();
        writeln!(md, "|------------|-------|").unwrap();
        for c in &near {
            writeln!(md, "| {} | {} |", c.normalized_phrase, c.verbs.join(", ")).unwrap();
        }
        writeln!(md).unwrap();
    }

    if collisions.is_empty() {
        writeln!(md, "No phrase collisions detected.").unwrap();
    }

    md
}
