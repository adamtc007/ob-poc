//! Lexicon service xtask commands
//!
//! Commands for compiling, linting, and benchmarking the lexicon snapshot.

use anyhow::{Context, Result};
use std::path::Path;

/// Compile lexicon YAML files into a binary snapshot.
pub fn compile(config_root: Option<&Path>, output: Option<&Path>, verbose: bool) -> Result<()> {
    use ob_poc::lexicon::LexiconCompiler;

    println!("===========================================");
    println!("  Lexicon Compile");
    println!("===========================================\n");

    // Default paths
    let root = super::project_root()?;
    let config_dir = config_root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| root.join("rust/config"));
    let output_path = output
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| root.join("rust/assets/lexicon.snapshot.bin"));

    println!("Config root: {}", config_dir.display());
    println!("Output:      {}", output_path.display());
    println!();

    // Create output directory if needed
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Build the snapshot
    let compiler = LexiconCompiler::new(&config_dir);
    let snapshot = compiler
        .build()
        .context("Failed to build lexicon snapshot")?;

    // Save binary
    snapshot
        .save_binary(&output_path)
        .context("Failed to save snapshot")?;

    println!("Lexicon snapshot compiled successfully!");
    println!();
    println!("  Hash:         {}", snapshot.hash);
    println!("  Verbs:        {}", snapshot.verb_meta.len());
    println!("  Entity types: {}", snapshot.entity_types.len());
    println!("  Domains:      {}", snapshot.domains.len());
    println!("  Labels:       {}", snapshot.label_to_concepts.len());
    println!("  Tokens:       {}", snapshot.token_to_concepts.len());

    if verbose {
        println!();
        println!("Label index sample (first 10):");
        for (label, concepts) in snapshot.label_to_concepts.iter().take(10) {
            println!("  {:30} → {:?}", label, concepts.as_slice());
        }
    }

    println!();
    println!("===========================================");
    println!("  Compile complete");
    println!("===========================================");

    Ok(())
}

/// Lint lexicon YAML files for consistency.
pub fn lint(config_root: Option<&Path>, errors_only: bool) -> Result<()> {
    println!("===========================================");
    println!("  Lexicon Lint");
    println!("===========================================\n");

    let root = super::project_root()?;
    let config_dir = config_root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| root.join("rust/config"));

    let lexicon_dir = config_dir.join("lexicon");
    println!("Checking: {}", lexicon_dir.display());
    println!();

    if !lexicon_dir.exists() {
        println!("WARNING: Lexicon directory does not exist yet.");
        println!("         Create YAML files in: {}", lexicon_dir.display());
        println!();
        println!("Expected files:");
        println!("  - verb_concepts.yaml");
        println!("  - entity_types.yaml");
        println!("  - domains.yaml");
        println!("  - schemes.yaml (optional)");
        return Ok(());
    }

    let mut errors = 0;
    let mut warnings = 0;

    // Check each expected file
    let expected_files = [
        ("verb_concepts.yaml", true),
        ("entity_types.yaml", true),
        ("domains.yaml", true),
        ("schemes.yaml", false),
    ];

    for (filename, required) in expected_files {
        let path = lexicon_dir.join(filename);
        if !path.exists() {
            if required {
                println!("[ERROR] Missing required file: {}", filename);
                errors += 1;
            } else if !errors_only {
                println!("[WARN]  Missing optional file: {}", filename);
                warnings += 1;
            }
            continue;
        }

        // Try to parse YAML
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", filename))?;

        match serde_yaml::from_str::<serde_yaml::Value>(&content) {
            Ok(doc) => {
                if !errors_only {
                    let count = match &doc {
                        serde_yaml::Value::Mapping(m) => m.len(),
                        serde_yaml::Value::Sequence(s) => s.len(),
                        _ => 1,
                    };
                    println!("[OK]    {} ({} entries)", filename, count);
                }
            }
            Err(e) => {
                println!("[ERROR] {} - YAML parse error: {}", filename, e);
                errors += 1;
            }
        }
    }

    println!();
    println!("===========================================");
    println!("  Lint Summary");
    println!("===========================================");
    println!("Errors:   {}", errors);
    if !errors_only {
        println!("Warnings: {}", warnings);
    }

    if errors > 0 {
        anyhow::bail!("Lint failed with {} error(s)", errors);
    }

    println!("\nLexicon lint passed!");
    Ok(())
}

/// Benchmark lexicon search performance.
pub fn bench(snapshot_path: Option<&Path>, iterations: usize) -> Result<()> {
    use ob_poc::lexicon::{LexiconService, LexiconServiceImpl, LexiconSnapshot};
    use std::sync::Arc;
    use std::time::Instant;

    println!("===========================================");
    println!("  Lexicon Benchmark");
    println!("===========================================\n");

    let root = super::project_root()?;
    let path = snapshot_path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| root.join("rust/assets/lexicon.snapshot.bin"));

    if !path.exists() {
        anyhow::bail!(
            "Snapshot not found: {}\nRun `cargo xtask lexicon compile` first.",
            path.display()
        );
    }

    println!("Loading: {}", path.display());
    let snapshot =
        Arc::new(LexiconSnapshot::load_binary(&path).context("Failed to load snapshot")?);
    println!("  Hash: {}", snapshot.hash);
    println!("  Verbs: {}", snapshot.verb_meta.len());
    println!();

    let service = LexiconServiceImpl::new(snapshot);

    // Test queries representing real-world usage
    let queries = [
        "create fund",
        "load allianz",
        "list cbus",
        "assign role",
        "show ownership",
        "trace money",
        "open kyc case",
        "add trading profile",
    ];

    println!(
        "Running {} iterations with {} queries...\n",
        iterations,
        queries.len()
    );

    let start = Instant::now();
    for _ in 0..iterations {
        for q in &queries {
            let _ = service.search_verbs(q, None, 5);
        }
    }
    let elapsed = start.elapsed();

    let total_calls = iterations * queries.len();
    let avg_us = elapsed.as_micros() as f64 / total_calls as f64;
    let ops_per_sec = (total_calls as f64 / elapsed.as_secs_f64()) as u64;

    println!("Results:");
    println!("  Total calls:    {}", total_calls);
    println!("  Total time:     {:?}", elapsed);
    println!("  Avg per call:   {:.2}µs", avg_us);
    println!("  Throughput:     {} ops/sec", ops_per_sec);
    println!();

    // Check against target
    let target_us = 100.0;
    if avg_us < target_us {
        println!("✓ PASS: {:.2}µs < {}µs target", avg_us, target_us);
    } else {
        println!("✗ FAIL: {:.2}µs > {}µs target", avg_us, target_us);
        println!();
        println!("Performance is below target. Consider:");
        println!("  - Reducing token index size");
        println!("  - Using smaller SmallVec capacities");
        println!("  - Profiling hot paths");
    }

    println!();
    println!("===========================================");
    println!("  Benchmark complete");
    println!("===========================================");

    Ok(())
}
