//! Entity linking xtask commands
//!
//! Provides CLI commands for compiling, linting, and inspecting entity snapshots.

use anyhow::{Context, Result};
use std::path::Path;

/// Get database URL from environment
fn get_database_url() -> Result<String> {
    std::env::var("DATABASE_URL").context(
        "DATABASE_URL not set. Set it to your PostgreSQL connection string, e.g.:\n\
         export DATABASE_URL=\"postgresql:///data_designer\"",
    )
}

/// Compile entity snapshot from database
pub async fn compile(output: Option<&Path>, verbose: bool) -> Result<()> {
    println!("===========================================");
    println!("  Entity Snapshot Compiler");
    println!("===========================================\n");

    let database_url = get_database_url()?;
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .context("Failed to connect to database")?;

    println!("Connected to database\n");

    if verbose {
        println!("Compiling entity snapshot...\n");
    }

    let snapshot = ob_poc::entity_linking::compile_entity_snapshot(&pool)
        .await
        .context("Failed to compile entity snapshot")?;

    let stats = snapshot.stats();
    println!("{}", stats);

    let output_path = output.unwrap_or_else(|| Path::new("assets/entity.snapshot.bin"));

    // Create parent directory if needed
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create output directory")?;
    }

    snapshot
        .save(output_path)
        .context("Failed to save snapshot")?;

    println!("\n✓ Saved to: {}", output_path.display());
    println!("✓ Hash: {}", &stats.hash[..16]);

    Ok(())
}

/// Lint entity data for quality issues
pub async fn lint(errors_only: bool) -> Result<()> {
    println!("===========================================");
    println!("  Entity Data Lint");
    println!("===========================================\n");

    let database_url = get_database_url()?;
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .context("Failed to connect to database")?;

    let warnings = ob_poc::entity_linking::lint_entity_data(&pool)
        .await
        .context("Failed to lint entity data")?;

    let mut error_count = 0;
    let mut warning_count = 0;
    let mut info_count = 0;

    for w in &warnings {
        match w.severity {
            ob_poc::entity_linking::LintSeverity::Error => error_count += 1,
            ob_poc::entity_linking::LintSeverity::Warning => warning_count += 1,
            ob_poc::entity_linking::LintSeverity::Info => info_count += 1,
        }

        // Skip info if errors_only
        if errors_only && w.severity == ob_poc::entity_linking::LintSeverity::Info {
            continue;
        }

        println!("{}\n", w);
    }

    println!("-------------------------------------------");
    println!(
        "Summary: {} errors, {} warnings, {} info",
        error_count, warning_count, info_count
    );

    if error_count > 0 {
        anyhow::bail!("Lint failed with {} errors", error_count);
    }

    if warnings.is_empty() {
        println!("\n✓ No issues found");
    }

    Ok(())
}

/// Show entity snapshot statistics
pub fn stats(snapshot_path: Option<&Path>) -> Result<()> {
    println!("===========================================");
    println!("  Entity Snapshot Statistics");
    println!("===========================================\n");

    let path = snapshot_path.unwrap_or_else(|| Path::new("assets/entity.snapshot.bin"));

    if !path.exists() {
        anyhow::bail!(
            "Snapshot not found at: {}\n\
             Run 'cargo x entity compile' first to create it.",
            path.display()
        );
    }

    let snapshot =
        ob_poc::entity_linking::EntitySnapshot::load(path).context("Failed to load snapshot")?;

    let stats = snapshot.stats();
    println!("{}", stats);

    // Show some sample data
    println!("\nSample entities (first 5):");
    for (i, e) in snapshot.entities.iter().take(5).enumerate() {
        println!(
            "  {}. {} ({}) - {}",
            i + 1,
            e.canonical_name,
            e.entity_kind,
            e.entity_id
        );
    }

    if snapshot.entities.len() > 5 {
        println!("  ... and {} more", snapshot.entities.len() - 5);
    }

    // Show kind distribution
    println!("\nEntity kinds:");
    let mut kinds: Vec<_> = snapshot.kind_index.iter().collect();
    kinds.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
    for (kind, ids) in kinds.iter().take(10) {
        println!("  {}: {} entities", kind, ids.len());
    }

    Ok(())
}
