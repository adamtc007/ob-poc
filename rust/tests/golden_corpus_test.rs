//! Golden Corpus CI test — validates corpus files parse correctly
//! and enforces structural constraints.
//!
//! This test runs without database access (no `--features database`).
//! It validates:
//! - All YAML files in tests/golden_corpus/ parse correctly
//! - Required fields are present
//! - Expected verbs are valid FQN format (domain.action)
//! - Categories are consistent
//! - No duplicate IDs across files
//!
//! The actual verb matching accuracy tests require the database and
//! are in the replay-tuner CLI.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::Deserialize;

/// Corpus entry format for the seed.yaml (legacy flat format).
#[derive(Debug, Deserialize)]
struct SeedEntry {
    utterance: String,
    expected_verb: Option<String>,
    pack: Option<String>,
    outcome: String,
    #[allow(dead_code)]
    notes: Option<String>,
}

/// Corpus entry format for the expanded corpus files (new format with id).
#[derive(Debug, Deserialize)]
struct CorpusEntry {
    id: String,
    category: String,
    input: String,
    pack_id: Option<String>,
    expected_verb: Option<String>,
    match_mode: String,
    #[allow(dead_code)]
    tags: Option<Vec<String>>,
}

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden_corpus")
}

#[test]
fn test_seed_yaml_parses() {
    let path = corpus_dir().join("seed.yaml");
    let content = std::fs::read_to_string(&path).expect("read seed.yaml");
    let entries: Vec<SeedEntry> = serde_yaml::from_str(&content).expect("parse seed.yaml");

    assert!(
        entries.len() >= 20,
        "seed.yaml should have at least 20 entries, got {}",
        entries.len()
    );

    for (i, entry) in entries.iter().enumerate() {
        assert!(
            !entry.utterance.is_empty(),
            "Entry {} has empty utterance",
            i
        );
        assert!(
            ["matched", "ambiguous", "no_match"].contains(&entry.outcome.as_str()),
            "Entry {} has invalid outcome: {}",
            i,
            entry.outcome
        );

        // If outcome is "matched", expected_verb must be set.
        if entry.outcome == "matched" {
            assert!(
                entry.expected_verb.is_some(),
                "Entry {} outcome=matched but no expected_verb",
                i
            );
            let verb = entry.expected_verb.as_ref().unwrap();
            assert!(
                verb.contains('.'),
                "Entry {} expected_verb '{}' is not FQN (missing '.')",
                i,
                verb
            );
        }
    }
}

#[test]
fn test_expanded_corpus_files_parse() {
    let dir = corpus_dir();
    let expanded_files = [
        "kyc.yaml",
        "book_setup.yaml",
        "bootstrap.yaml",
        "pack_switching.yaml",
        "error_recovery.yaml",
        "edge_cases.yaml",
    ];

    for filename in &expanded_files {
        let path = dir.join(filename);
        if !path.exists() {
            panic!("Expected corpus file not found: {}", filename);
        }

        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", filename, e));
        let entries: Vec<CorpusEntry> = serde_yaml::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", filename, e));

        assert!(
            !entries.is_empty(),
            "{} should have at least 1 entry",
            filename
        );

        for entry in &entries {
            assert!(!entry.id.is_empty(), "{}: entry has empty id", filename);
            assert!(
                !entry.category.is_empty(),
                "{}: entry {} has empty category",
                filename,
                entry.id
            );
            assert!(
                !entry.input.is_empty() || entry.id.contains("edge-002"),
                "{}: entry {} has empty input (only edge-002 allowed)",
                filename,
                entry.id
            );
            assert!(
                ["exact", "top_three", "match_or_ambiguous"].contains(&entry.match_mode.as_str()),
                "{}: entry {} has invalid match_mode: {}",
                filename,
                entry.id,
                entry.match_mode
            );

            // If match_mode is "exact" and expected_verb is set, it must be FQN.
            if entry.match_mode == "exact" {
                if let Some(ref verb) = entry.expected_verb {
                    assert!(
                        verb.contains('.'),
                        "{}: entry {} expected_verb '{}' is not FQN",
                        filename,
                        entry.id,
                        verb
                    );
                }
            }
        }
    }
}

#[test]
fn test_no_duplicate_ids() {
    let dir = corpus_dir();
    let expanded_files = [
        "kyc.yaml",
        "book_setup.yaml",
        "bootstrap.yaml",
        "pack_switching.yaml",
        "error_recovery.yaml",
        "edge_cases.yaml",
    ];

    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut duplicates: Vec<String> = Vec::new();

    for filename in &expanded_files {
        let path = dir.join(filename);
        if !path.exists() {
            continue;
        }

        let content = std::fs::read_to_string(&path).unwrap();
        let entries: Vec<CorpusEntry> = serde_yaml::from_str(&content).unwrap();

        for entry in &entries {
            if !seen_ids.insert(entry.id.clone()) {
                duplicates.push(format!("{} in {}", entry.id, filename));
            }
        }
    }

    assert!(
        duplicates.is_empty(),
        "Duplicate IDs found: {:?}",
        duplicates
    );
}

#[test]
fn test_corpus_total_at_least_50() {
    let dir = corpus_dir();
    let all_files = [
        "seed.yaml",
        "kyc.yaml",
        "book_setup.yaml",
        "bootstrap.yaml",
        "pack_switching.yaml",
        "error_recovery.yaml",
        "edge_cases.yaml",
    ];

    let mut total = 0;

    for filename in &all_files {
        let path = dir.join(filename);
        if !path.exists() {
            continue;
        }
        let content = std::fs::read_to_string(&path).unwrap();

        if filename == &"seed.yaml" {
            let entries: Vec<SeedEntry> = serde_yaml::from_str(&content).unwrap();
            total += entries.len();
        } else {
            let entries: Vec<CorpusEntry> = serde_yaml::from_str(&content).unwrap();
            total += entries.len();
        }
    }

    assert!(
        total >= 50,
        "Total corpus entries must be >= 50, got {}",
        total
    );
}

#[test]
fn test_category_coverage() {
    let dir = corpus_dir();
    let expanded_files = [
        "kyc.yaml",
        "book_setup.yaml",
        "bootstrap.yaml",
        "pack_switching.yaml",
        "error_recovery.yaml",
        "edge_cases.yaml",
    ];

    let mut category_counts: HashMap<String, usize> = HashMap::new();

    for filename in &expanded_files {
        let path = dir.join(filename);
        if !path.exists() {
            continue;
        }
        let content = std::fs::read_to_string(&path).unwrap();
        let entries: Vec<CorpusEntry> = serde_yaml::from_str(&content).unwrap();

        for entry in &entries {
            *category_counts.entry(entry.category.clone()).or_insert(0) += 1;
        }
    }

    // Verify minimum category coverage.
    let required_categories = [
        "kyc",
        "book-setup",
        "bootstrap",
        "pack-switching",
        "error-recovery",
        "edge-cases",
    ];

    for cat in &required_categories {
        let count = category_counts.get(*cat).copied().unwrap_or(0);
        assert!(
            count >= 5,
            "Category '{}' has {} entries, need at least 5",
            cat,
            count
        );
    }
}
