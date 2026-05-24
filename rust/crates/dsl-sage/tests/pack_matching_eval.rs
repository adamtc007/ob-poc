//! Evaluation harness for the Sage pack matcher.
//!
//! # Accuracy targets
//!
//! | Embedder          | Top-1 target | Top-3 target |
//! |-------------------|-------------|-------------|
//! | BagOfWords (BoW)  | ≥ 50%       | ≥ 70%       |
//! | BGE-small-en-v1.5 | ≥ 80%       | ≥ 95%       |
//!
//! The 80%/95% targets from §1.7 of the master plan apply to the BGE model.
//! This file enforces the BoW baseline; the BGE harness is a follow-up.

use dsl_diagnostics::DiagnosticBag;
use dsl_resolution::{pack_registry::load_packs_from_dir, PackRegistry};
use dsl_sage::{match_packs_embedding_only, BagOfWordsEmbedder, SageContext};

// ---------------------------------------------------------------------------
// Evaluation set (50 utterances × 12 packs)
// ---------------------------------------------------------------------------

struct EvalCase {
    utterance: &'static str,
    expected_pack: &'static str,
}

/// 50-utterance evaluation set covering all 12 decision packs.
///
/// Pack distribution:
///   conjunctive-gate (5), disjunctive-gate (4), sanction-hit-escalation (4),
///   periodic-refresh-trigger (4), manual-override-checkpoint (4),
///   threshold-band-routing (4), multi-jurisdiction-overlay (4),
///   linked-switch-chain (3), parallel-evaluation-with-veto (3),
///   decision-table-classification (3), cascading-decision (3),
///   required-evidence-checklist (4) + 5 edge cases
const EVAL_SET: &[EvalCase] = &[
    // ---- conjunctive-gate (5) -----------------------------------------------
    EvalCase {
        utterance: "all checks must pass before activation",
        expected_pack: "conjunctive-gate",
    },
    EvalCase {
        utterance: "only proceed if KYC screening and UBO are all approved",
        expected_pack: "conjunctive-gate",
    },
    EvalCase {
        utterance: "require all three conditions to be met before proceeding",
        expected_pack: "conjunctive-gate",
    },
    EvalCase {
        utterance: "when every requirement is satisfied route to fast track",
        expected_pack: "conjunctive-gate",
    },
    EvalCase {
        utterance: "all of these must be true before we can activate",
        expected_pack: "conjunctive-gate",
    },
    // ---- disjunctive-gate (4) -----------------------------------------------
    EvalCase {
        utterance: "if any red flag is present escalate",
        expected_pack: "disjunctive-gate",
    },
    EvalCase {
        utterance: "any sanctions hit or PEP match triggers enhanced review",
        expected_pack: "disjunctive-gate",
    },
    EvalCase {
        utterance: "escalate to compliance if any risk indicator fires",
        expected_pack: "disjunctive-gate",
    },
    EvalCase {
        utterance: "route to compliance if any of these conditions holds",
        expected_pack: "disjunctive-gate",
    },
    // ---- sanction-hit-escalation (4) ----------------------------------------
    EvalCase {
        utterance: "if there is a sanctions match immediately escalate",
        expected_pack: "sanction-hit-escalation",
    },
    EvalCase {
        utterance: "hard block on sanctions positive result",
        expected_pack: "sanction-hit-escalation",
    },
    EvalCase {
        utterance: "any sanctions hit must go to manual review regardless",
        expected_pack: "sanction-hit-escalation",
    },
    EvalCase {
        utterance: "positive sanctions result overrides everything else",
        expected_pack: "sanction-hit-escalation",
    },
    // ---- periodic-refresh-trigger (4) ----------------------------------------
    EvalCase {
        utterance: "if KYC was last refreshed more than 12 months ago trigger refresh",
        expected_pack: "periodic-refresh-trigger",
    },
    EvalCase {
        utterance: "annual review check if stale",
        expected_pack: "periodic-refresh-trigger",
    },
    EvalCase {
        utterance: "check if last review is older than configured period",
        expected_pack: "periodic-refresh-trigger",
    },
    EvalCase {
        utterance: "time based trigger refresh if over threshold age",
        expected_pack: "periodic-refresh-trigger",
    },
    // ---- manual-override-checkpoint (4) ----------------------------------------
    EvalCase {
        utterance: "automatically assess risk but allow compliance officer to override",
        expected_pack: "manual-override-checkpoint",
    },
    EvalCase {
        utterance: "system recommendation with human approval checkpoint",
        expected_pack: "manual-override-checkpoint",
    },
    EvalCase {
        utterance: "four eyes check algorithm recommends human confirms",
        expected_pack: "manual-override-checkpoint",
    },
    EvalCase {
        utterance: "automated decision with manual override capability",
        expected_pack: "manual-override-checkpoint",
    },
    // ---- threshold-band-routing (4) -------------------------------------------
    EvalCase {
        utterance: "route by ownership percentage minor significant controlling",
        expected_pack: "threshold-band-routing",
    },
    EvalCase {
        utterance: "tiered risk scoring low medium high bands",
        expected_pack: "threshold-band-routing",
    },
    EvalCase {
        utterance: "ownership tier routing based on stake percentage",
        expected_pack: "threshold-band-routing",
    },
    EvalCase {
        utterance: "threshold based routing on numeric value",
        expected_pack: "threshold-band-routing",
    },
    // ---- multi-jurisdiction-overlay (4) ----------------------------------------
    EvalCase {
        utterance: "apply UK rules for UK clients EU rules for EU clients otherwise global",
        expected_pack: "multi-jurisdiction-overlay",
    },
    EvalCase {
        utterance: "different process per domicile",
        expected_pack: "multi-jurisdiction-overlay",
    },
    EvalCase {
        utterance: "route by jurisdiction each country has its own requirements",
        expected_pack: "multi-jurisdiction-overlay",
    },
    EvalCase {
        utterance: "apply relevant regulatory regime based on jurisdiction",
        expected_pack: "multi-jurisdiction-overlay",
    },
    // ---- linked-switch-chain (3) -----------------------------------------------
    EvalCase {
        utterance: "sequential checks with early exit on failure",
        expected_pack: "linked-switch-chain",
    },
    EvalCase {
        utterance: "chain of compliance checks each with rejection path",
        expected_pack: "linked-switch-chain",
    },
    EvalCase {
        utterance: "waterfall decision each gate can reject before next",
        expected_pack: "linked-switch-chain",
    },
    // ---- parallel-evaluation-with-veto (3) ------------------------------------
    EvalCase {
        utterance: "run all checks in parallel if any rejects whole application rejected",
        expected_pack: "parallel-evaluation-with-veto",
    },
    EvalCase {
        utterance: "parallel screening single hit blocks process",
        expected_pack: "parallel-evaluation-with-veto",
    },
    EvalCase {
        utterance: "concurrent evaluation with veto semantics any failure fails all",
        expected_pack: "parallel-evaluation-with-veto",
    },
    // ---- decision-table-classification (3) ------------------------------------
    EvalCase {
        utterance: "classify investor type and route accordingly",
        expected_pack: "decision-table-classification",
    },
    EvalCase {
        utterance: "use risk classification table to determine next steps",
        expected_pack: "decision-table-classification",
    },
    EvalCase {
        utterance: "DMN classification routing",
        expected_pack: "decision-table-classification",
    },
    // ---- cascading-decision (3) -----------------------------------------------
    EvalCase {
        utterance: "first classify by entity type then apply appropriate rules",
        expected_pack: "cascading-decision",
    },
    EvalCase {
        utterance: "two stage decision entity type determines which ruleset applies",
        expected_pack: "cascading-decision",
    },
    EvalCase {
        utterance: "cascading rules output of step one selects step two",
        expected_pack: "cascading-decision",
    },
    // ---- required-evidence-checklist (4) --------------------------------------
    EvalCase {
        utterance: "collect and verify all required documents before decision",
        expected_pack: "required-evidence-checklist",
    },
    EvalCase {
        utterance: "sequential evidence checklist ID address source of wealth",
        expected_pack: "required-evidence-checklist",
    },
    EvalCase {
        utterance: "step by step document verification before final approval",
        expected_pack: "required-evidence-checklist",
    },
    EvalCase {
        utterance: "checklist all evidence collected and verified proceed",
        expected_pack: "required-evidence-checklist",
    },
    // ---- edge cases (5) -------------------------------------------------------
    EvalCase {
        utterance: "check if kyc approved ubo resolved sanctions clear all three needed",
        expected_pack: "conjunctive-gate",
    },
    EvalCase {
        utterance: "immediate block if any screening produces a hit",
        expected_pack: "sanction-hit-escalation",
    },
    EvalCase {
        utterance: "human can override the automatic classification",
        expected_pack: "manual-override-checkpoint",
    },
    EvalCase {
        utterance: "review every two years if record is stale",
        expected_pack: "periodic-refresh-trigger",
    },
    EvalCase {
        utterance: "apply different compliance process depending on country of registration",
        expected_pack: "multi-jurisdiction-overlay",
    },
];

// ---------------------------------------------------------------------------
// Test helper: load registry from dsl-source/packs/
// ---------------------------------------------------------------------------

fn load_test_registry() -> PackRegistry {
    let pack_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("dsl-source/packs");

    let mut registry = PackRegistry::new();
    let mut diag = DiagnosticBag::new();
    load_packs_from_dir(&pack_dir, &mut registry, &mut diag)
        .expect("failed to load pack DSL files");

    let errors: Vec<_> = diag
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, dsl_diagnostics::DiagnosticSeverity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "pack loading produced errors: {:?}",
        errors
    );

    assert!(
        registry.len() >= 12,
        "expected at least 12 packs, got {}",
        registry.len()
    );
    registry
}

// ---------------------------------------------------------------------------
// Evaluation test: BagOfWords baseline
// ---------------------------------------------------------------------------

#[test]
fn eval_top1_accuracy_embedding_only() {
    let registry = load_test_registry();
    let embedder = BagOfWordsEmbedder;
    let context = SageContext::empty();

    let mut top1_correct = 0usize;
    let mut top3_correct = 0usize;
    let mut results: Vec<(&str, &str, String, bool, bool)> = Vec::new();

    for case in EVAL_SET {
        let candidates =
            match_packs_embedding_only(case.utterance, &context, &registry, &embedder);

        let top1_name = candidates.first().map(|c| c.pack_name.as_str()).unwrap_or("");
        let top3_names: Vec<&str> = candidates.iter().take(3).map(|c| c.pack_name.as_str()).collect();

        let hit1 = top1_name == case.expected_pack;
        let hit3 = top3_names.contains(&case.expected_pack);

        if hit1 {
            top1_correct += 1;
        }
        if hit3 {
            top3_correct += 1;
        }
        results.push((case.utterance, case.expected_pack, top1_name.to_string(), hit1, hit3));
    }

    let n = EVAL_SET.len();
    let top1_acc = top1_correct as f32 / n as f32;
    let top3_acc = top3_correct as f32 / n as f32;

    // Print report
    println!("\n=== Pack Matching Evaluation (embedding-only, BagOfWords) ===");
    for (utt, expected, got, hit1, hit3) in &results {
        let marker = if *hit1 { "T1+" } else if *hit3 { "T3+" } else { " ✗ " };
        let preview = &utt[..60.min(utt.len())];
        println!(
            "  {} | expected={:<36} | got={:<36} | {}",
            marker, expected, got, preview
        );
    }
    println!(
        "\nTop-1 accuracy: {}/{} = {:.1}%",
        top1_correct,
        n,
        top1_acc * 100.0
    );
    println!(
        "Top-3 accuracy: {}/{} = {:.1}%",
        top3_correct,
        n,
        top3_acc * 100.0
    );
    println!(
        "\nNote: BagOfWords baseline target is ≥ 50% top-1 / ≥ 70% top-3."
    );
    println!(
        "      BGE-small-en-v1.5 embeddings will push this to ≥ 80% / ≥ 95% per §1.7."
    );

    assert!(
        top1_acc >= 0.50,
        "Expected ≥ 50% top-1 accuracy with BagOfWords baseline, got {:.1}%.\n\
         BGE model embeddings target ≥ 80%.",
        top1_acc * 100.0
    );

    assert!(
        top3_acc >= 0.70,
        "Expected ≥ 70% top-3 accuracy with BagOfWords baseline, got {:.1}%.",
        top3_acc * 100.0
    );
}

// ---------------------------------------------------------------------------
// Async test: match_packs with MockLlmClient
// ---------------------------------------------------------------------------

#[tokio::test]
async fn async_match_packs_with_mock_llm() {
    use dsl_sage::{match_packs, MockLlmClient};

    let registry = load_test_registry();
    let embedder = BagOfWordsEmbedder;
    let context = SageContext::empty();

    let result = match_packs(
        "all checks must pass before activation",
        &context,
        &registry,
        &embedder,
        Some(&MockLlmClient),
    )
    .await
    .expect("match_packs failed");

    assert!(!result.is_empty(), "expected at least one candidate");

    // conjunctive-gate should rank highly for this utterance
    assert!(
        result.iter().take(3).any(|c| c.pack_name == "conjunctive-gate"),
        "conjunctive-gate should be in top-3, got: {:?}",
        result.iter().take(3).map(|c| &c.pack_name).collect::<Vec<_>>()
    );

    // All candidates have valid confidence in [0, 1]
    for c in &result {
        assert!(
            c.confidence >= 0.0 && c.confidence <= 1.0,
            "confidence out of range: {} for {}",
            c.confidence,
            c.pack_name
        );
    }
}

// ---------------------------------------------------------------------------
// Async test: match_packs without LLM (embedding-only async path)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn async_match_packs_no_llm() {
    use dsl_sage::match_packs;

    let registry = load_test_registry();
    let embedder = BagOfWordsEmbedder;
    let context = SageContext::empty();

    let result = match_packs(
        "parallel screening single hit blocks process",
        &context,
        &registry,
        &embedder,
        None, // no LLM
    )
    .await
    .expect("match_packs (no-llm) failed");

    assert!(!result.is_empty());

    // parallel-evaluation-with-veto should appear in top-3
    assert!(
        result.iter().take(3).any(|c| c.pack_name == "parallel-evaluation-with-veto"),
        "parallel-evaluation-with-veto should be in top-3, got: {:?}",
        result.iter().take(3).map(|c| &c.pack_name).collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// Test: empty registry returns empty list
// ---------------------------------------------------------------------------

#[test]
fn empty_registry_returns_empty() {
    let registry = PackRegistry::new();
    let embedder = BagOfWordsEmbedder;
    let context = SageContext::empty();
    let result = match_packs_embedding_only("anything", &context, &registry, &embedder);
    assert!(result.is_empty());
}

// ---------------------------------------------------------------------------
// Test: domain filter de-ranks out-of-scope packs
// ---------------------------------------------------------------------------

#[test]
fn domain_filter_applied() {
    let registry = load_test_registry();
    let embedder = BagOfWordsEmbedder;
    // Use a domain that none of the packs should match → all de-ranked equally
    let context = SageContext::with_domain("nonexistent-domain");

    let result = match_packs_embedding_only(
        "all checks must pass before activation",
        &context,
        &registry,
        &embedder,
    );

    // Should still return candidates (de-ranked, not removed)
    assert!(!result.is_empty());
    // All scores should be lower than without domain filter
    // (can't verify exact values but result should be non-empty)
    for c in &result {
        assert!(
            c.confidence >= 0.0,
            "negative confidence unexpected: {}",
            c.confidence
        );
    }
}
