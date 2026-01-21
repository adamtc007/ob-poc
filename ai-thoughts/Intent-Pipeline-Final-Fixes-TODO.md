# Intent Pipeline Final Fixes TODO

**Created**: 2025-01-21  
**Updated**: 2025-01-21 (post-ChatGPT review)  
**Context**: Review of Claude Code's C/K/I/J/D implementation.

---

## Current Status

| Issue | Status | Notes |
|-------|--------|-------|
| C | ✅ DONE | Real refs from parse→enrich→walker |
| K | ✅ DONE | commit-by-ref_id with dsl_hash guard |
| I | ✅ DONE | Union/dedupe of invocation_phrases + pattern_embeddings |
| J/D | ⚠️ INCOMPLETE | `normalize_candidates()` done, but LIMIT 1 in tiers 3-4 truncates candidate pool |

**Next step**: See `Intent-Pipeline-JD-Completion-TODO.md` for Claude Code prompt to wire up top-k.

---

## Original Context

Review of Claude Code's C/K/I/J/D implementation found:  
- normalize_candidates fix is correct  
- But candidate pool is truncated by LIMIT 1 before ambiguity detection can see competing verbs

---

## Claude Code Prompt Header

> **Copy this section when starting Claude Code session:**

You are completing the final fixes for the intent pipeline determinism work. The previous implementation (Issues C, K, I) is correct, but Issue J/D has a critical bug and the acceptance tests are missing.

**Hard constraints:**
- Do NOT refactor architecture — these are surgical fixes
- The dedupe/sort fix is ~15 lines in one location
- Tests must be runnable with `cargo test`

**Implementation requirements:**
1. **CRITICAL**: Fix global dedupe/sort bug in `HybridVerbSearcher::search()`
2. Expose `dsl_hash` in UI response payloads
3. Add two acceptance tests (list commit + ambiguity blocks LLM)
4. Optional: Switch user semantic to top-k method

**At the end, provide:**
- Summary of changes
- How to run the tests
- Confirmation all items are complete

---

## Issue 1: CRITICAL — Global Dedupe/Sort Bug in Verb Search

### Problem

`HybridVerbSearcher::search()` appends candidates tier-by-tier but only sorts at the end without deduping by verb. This means:

- Same verb can appear multiple times with different scores
- `candidates[0]` may not be the highest-scoring match
- `check_ambiguity()` may see duplicate verbs and give wrong results

### Fix Location

`rust/src/mcp/verb_search.rs` — add a standalone function, then call it at the end of `search()`.

### Fix Code

**Step 1**: Add this function (near `check_ambiguity()` or at module level):

```rust
use std::collections::HashMap;

/// Normalize candidate list: dedupe by verb (keep highest score), sort desc, truncate
/// 
/// Essential for J/D correctness — candidates are appended tier-by-tier during search,
/// so without this, candidates[0] is not guaranteed to be the best match.
fn normalize_candidates(mut results: Vec<VerbSearchResult>, limit: usize) -> Vec<VerbSearchResult> {
    // Deduplicate by verb (keep highest score; preserve best metadata)
    let mut by_verb: HashMap<String, VerbSearchResult> = HashMap::new();
    for r in results.drain(..) {
        by_verb
            .entry(r.verb.clone())
            .and_modify(|existing| {
                if r.score > existing.score {
                    *existing = r.clone();
                }
            })
            .or_insert(r);
    }

    let mut v: Vec<VerbSearchResult> = by_verb.into_values().collect();
    v.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    v.truncate(limit);
    v
}
```

**Step 2**: In `search()`, replace the final sort/truncate with:

```rust
let results = normalize_candidates(results, limit);
Ok(results)
```

This is cleaner than inline code and matches the pattern in `search_global_semantic_with_embedding()`.

---

## Issue 2: Expose dsl_hash in UI Response

### Problem

`PipelineResult.dsl_hash` is computed but not exposed to the UI. The `/resolve-by-ref-id` endpoint requires `dsl_hash` to prevent stale commits, but the UI has no way to get it without computing it client-side.

### Fix Location

`rust/src/api/agent_service.rs` or `rust/src/api/session.rs` — wherever `AgentChatResponse` or the disambiguation response struct is defined.

### Fix Code

Add to the response struct:

```rust
/// Hash of current DSL for resolution commit (Issue K plumbing)
#[serde(skip_serializing_if = "Option::is_none")]
pub dsl_hash: Option<String>,
```

Then populate it from `PipelineResult.dsl_hash` when building the response.

---

## Issue 3: Acceptance Test — List Commit Correctness

### Purpose

Verify that resolving one ref in a list doesn't affect others.

### Test Location

`rust/src/mcp/intent_pipeline.rs` or a new test file `rust/tests/intent_pipeline_integration.rs`

### Test Code

```rust
#[test]
fn test_list_commit_resolves_single_ref() {
    use dsl_core::ast::find_unresolved_ref_locations;
    use crate::dsl_v2::{parse_program, enrich_program, runtime_registry_arc};
    
    // DSL with list of unresolved refs
    let dsl = r#"(batch.create :clients ["Allianz" "BlackRock" "Vanguard"])"#;
    
    // Parse and enrich
    let ast = parse_program(dsl).expect("parse failed");
    let registry = runtime_registry_arc();
    let enriched = enrich_program(ast, &registry);
    
    // Get unresolved refs
    let refs = find_unresolved_ref_locations(&enriched.program);
    assert_eq!(refs.len(), 3, "Expected 3 unresolved refs in list");
    
    // All should have ref_ids
    for r in &refs {
        assert!(r.ref_id.is_some(), "ref_id should be present");
    }
    
    // Verify ref_ids are distinct
    let ref_ids: Vec<_> = refs.iter().map(|r| r.ref_id.clone().unwrap()).collect();
    let unique: std::collections::HashSet<_> = ref_ids.iter().collect();
    assert_eq!(unique.len(), 3, "ref_ids should be unique");
    
    // NOTE: Full commit test requires the /resolve-by-ref-id endpoint
    // which needs a running session. This test verifies the foundation.
}
```

> **Note**: If `batch.create` doesn't exist in the registry, substitute with a verb that accepts a list parameter with lookup config (e.g., create a test verb or use an existing one).

---

## Issue 4: Acceptance Test — Ambiguity Blocks LLM

### Purpose

Verify that when verb search returns ambiguous candidates, the LLM is NOT called.

### Test Location

`rust/src/mcp/intent_pipeline.rs` in the `#[cfg(test)]` module

### Test Code

```rust
#[test]
fn test_check_ambiguity_blocks_on_close_margin() {
    use crate::mcp::verb_search::{check_ambiguity, VerbSearchOutcome, VerbSearchResult, VerbSearchSource, AMBIGUITY_MARGIN};
    
    let threshold = 0.80;
    
    // Two candidates within margin, both above threshold
    let candidates = vec![
        VerbSearchResult {
            verb: "cbu.create".to_string(),
            score: 0.85,
            source: VerbSearchSource::Semantic,
            matched_phrase: "create cbu".to_string(),
            description: None,
        },
        VerbSearchResult {
            verb: "cbu.ensure".to_string(),
            score: 0.83, // margin = 0.02 < AMBIGUITY_MARGIN (0.05)
            source: VerbSearchSource::Semantic,
            matched_phrase: "ensure cbu".to_string(),
            description: None,
        },
    ];
    
    let outcome = check_ambiguity(&candidates, threshold);
    
    match outcome {
        VerbSearchOutcome::Ambiguous { top, runner_up, margin } => {
            assert_eq!(top.verb, "cbu.create");
            assert_eq!(runner_up.verb, "cbu.ensure");
            assert!(margin < AMBIGUITY_MARGIN, "margin {} should be < {}", margin, AMBIGUITY_MARGIN);
        }
        other => panic!("Expected Ambiguous, got {:?}", other),
    }
}

#[test]
fn test_check_ambiguity_passes_on_clear_winner() {
    use crate::mcp::verb_search::{check_ambiguity, VerbSearchOutcome, VerbSearchResult, VerbSearchSource, AMBIGUITY_MARGIN};
    
    let threshold = 0.80;
    
    // Clear winner - margin > AMBIGUITY_MARGIN
    let candidates = vec![
        VerbSearchResult {
            verb: "cbu.create".to_string(),
            score: 0.92,
            source: VerbSearchSource::Semantic,
            matched_phrase: "create cbu".to_string(),
            description: None,
        },
        VerbSearchResult {
            verb: "cbu.ensure".to_string(),
            score: 0.82, // margin = 0.10 > AMBIGUITY_MARGIN (0.05)
            source: VerbSearchSource::Semantic,
            matched_phrase: "ensure cbu".to_string(),
            description: None,
        },
    ];
    
    let outcome = check_ambiguity(&candidates, threshold);
    
    match outcome {
        VerbSearchOutcome::Matched(result) => {
            assert_eq!(result.verb, "cbu.create");
        }
        other => panic!("Expected Matched, got {:?}", other),
    }
}

#[test]
fn test_check_ambiguity_no_match_below_threshold() {
    use crate::mcp::verb_search::{check_ambiguity, VerbSearchOutcome, VerbSearchResult, VerbSearchSource};
    
    let threshold = 0.80;
    
    // All candidates below threshold
    let candidates = vec![
        VerbSearchResult {
            verb: "cbu.create".to_string(),
            score: 0.75, // below threshold
            source: VerbSearchSource::Semantic,
            matched_phrase: "create cbu".to_string(),
            description: None,
        },
    ];
    
    let outcome = check_ambiguity(&candidates, threshold);
    
    assert!(matches!(outcome, VerbSearchOutcome::NoMatch));
}

#[test]
fn test_normalize_candidates_dedupes_and_sorts() {
    use crate::mcp::verb_search::{normalize_candidates, VerbSearchResult, VerbSearchSource};
    
    // Simulate tier-by-tier appending: same verb appears twice with different scores
    let candidates = vec![
        VerbSearchResult {
            verb: "cbu.create".to_string(),
            score: 0.82, // lower score, added first (tier 3)
            source: VerbSearchSource::LearnedSemantic,
            matched_phrase: "make a cbu".to_string(),
            description: None,
        },
        VerbSearchResult {
            verb: "cbu.ensure".to_string(),
            score: 0.80,
            source: VerbSearchSource::Semantic,
            matched_phrase: "ensure cbu".to_string(),
            description: None,
        },
        VerbSearchResult {
            verb: "cbu.create".to_string(),
            score: 0.91, // higher score, added later (tier 6)
            source: VerbSearchSource::PatternEmbedding,
            matched_phrase: "create cbu".to_string(),
            description: None,
        },
    ];
    
    let normalized = normalize_candidates(candidates, 5);
    
    // Should have 2 unique verbs
    assert_eq!(normalized.len(), 2);
    
    // First should be cbu.create with the HIGHER score (0.91)
    assert_eq!(normalized[0].verb, "cbu.create");
    assert!((normalized[0].score - 0.91).abs() < 0.001);
    assert!(matches!(normalized[0].source, VerbSearchSource::PatternEmbedding));
    
    // Second should be cbu.ensure
    assert_eq!(normalized[1].verb, "cbu.ensure");
}
```

### Integration Test (Optional but Recommended)

For a full integration test that verifies the LLM is never called:

```rust
#[tokio::test]
async fn test_ambiguous_verb_does_not_call_llm() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    
    // This test requires mocking the LLM client
    // If you have a MockLlmClient, use it here:
    //
    // let call_count = Arc::new(AtomicUsize::new(0));
    // let mock_llm = MockLlmClient::with_counter(call_count.clone());
    // let pipeline = IntentPipeline::with_llm(searcher, Arc::new(mock_llm));
    // 
    // // Force ambiguous search results somehow (e.g., mock searcher)
    // let result = pipeline.process("ambiguous query", None).await.unwrap();
    // 
    // assert_eq!(result.outcome, PipelineOutcome::NeedsClarification);
    // assert_eq!(call_count.load(Ordering::SeqCst), 0, "LLM should not be called");
    
    // For now, the unit tests above verify the logic.
    // Full integration test requires mock infrastructure.
}
```

---

## Issue 5: Recommended — Use Top-K for User Semantic

### Problem

`find_user_learned_semantic_topk()` exists but `search_user_learned_semantic_with_embedding()` still uses the old single-result method. This means user-learned phrases can't contribute to ambiguity detection.

### Fix Location

`rust/src/mcp/verb_search.rs` — `search_user_learned_semantic_with_embedding()` method

### Fix

Change signature from `-> Result<Option<VerbSearchResult>>` to `-> Result<Vec<VerbSearchResult>>`.

Then call `find_user_learned_semantic_topk(..., limit: 3)` instead of the single-result method.

Even `top_k=3` is enough for ambiguity detection to work properly from learned sources.

**This is recommended** but not blocking — the global `normalize_candidates()` fix is the priority.

---

## Summary Checklist

### Completed (verified in review)
- [x] **CRITICAL**: Add `normalize_candidates()` function and call at end of `search()`
- [x] Expose `dsl_hash` in `AgentChatResponse` or disambiguation payload
- [x] Add `test_normalize_candidates_dedupes_and_sorts` (tests the critical fix)
- [x] Add `test_check_ambiguity_blocks_on_close_margin`
- [x] Add `test_check_ambiguity_passes_on_clear_winner`
- [x] Add `test_check_ambiguity_no_match_below_threshold`

### Still Required (J/D not fully complete)
- [ ] **Wire up top-k for user semantic** — `search_user_learned_semantic_with_embedding()` still uses LIMIT 1, change to call `find_user_learned_semantic_topk()` and return `Vec<VerbSearchResult>`
- [ ] **Remove or convert step 4** — `search_learned_semantic_with_embedding()` is now redundant (global semantic already merges invocation_phrases). Either delete it or convert to top-k.
- [ ] **Source threshold from searcher config** — `IntentPipeline.process()` uses hard-coded `SEMANTIC_THRESHOLD = 0.80`. Should use `searcher.semantic_threshold` for consistency.
- [ ] Add `test_list_commit_resolves_single_ref` (requires list span fix first)

## Files to Modify

| Issue | File | Changes |
|-------|------|---------|
| 1 (CRITICAL) | `rust/src/mcp/verb_search.rs` | Add `normalize_candidates()` fn, call at end of `search()` |
| 2 | `rust/src/api/agent_service.rs` | Add `dsl_hash` field to response struct |
| 3-4 | `rust/src/mcp/verb_search.rs` | Add acceptance tests in `#[cfg(test)]` module |
| 5 (recommended) | `rust/src/mcp/verb_search.rs` | Change `search_user_learned_semantic_with_embedding` to use top-k |

## Verification

After implementation, run:

```bash
cd rust
cargo test normalize_candidates
cargo test test_check_ambiguity
cargo test test_list_commit
```

Or run all verb_search tests:

```bash
cargo test --lib verb_search
```

All tests should pass. The `normalize_candidates` test specifically guards against the tier-ordering bug.
