# Intent Pipeline J/D Completion TODO

**Created**: 2025-01-21  
**Updated**: 2025-01-21 (post-ChatGPT review)  
**Context**: C/K/I are confirmed complete. J/D has a remaining bug: LIMIT 1 in tiers 3-4 truncates the candidate pool before `normalize_candidates()` runs, so ambiguity detection misses competing verbs in learned/user-learned space.

---

## Claude Code Prompt Header

> **Copy this section when starting Claude Code session:**

You are completing the J/D (ambiguity detection) fix. The previous implementation added `normalize_candidates()` and `check_ambiguity()`, but **the candidate pool is still truncated too early** by LIMIT 1 calls in tiers 3 and 4.

**The problem**: If two similar verbs exist in user-learned space (e.g., `cbu.create` @ 0.85 and `cbu.ensure` @ 0.83), only one enters the candidate pool. Ambiguity detection can't work if competing candidates never enter the pool.

**Hard constraints:**
- Do NOT refactor architecture — these are surgical wiring changes
- Existing top-k methods already exist in VerbService — just wire them in
- All existing tests must pass

**Implementation requirements:**
1. **CRITICAL**: Wire `search_user_learned_semantic_with_embedding()` to use top-k (limit=3)
2. **CRITICAL**: Wire `search_learned_semantic_with_embedding()` to use top-k OR remove it (it's redundant with step 6)
3. Source threshold from searcher config instead of hard-coded constant
4. Add missing acceptance tests

**At the end, provide:**
- Summary of changes
- `cargo test` output
- Confirmation that ambiguity detection now works across all semantic tiers

---

## Status Summary

| Issue | Status | Notes |
|-------|--------|-------|
| C | ✅ DONE | Real refs from parse→enrich→walker |
| K | ✅ DONE | commit-by-ref_id with dsl_hash guard |
| I | ✅ DONE | Union/dedupe of invocation_phrases + pattern_embeddings |
| J/D | ⚠️ INCOMPLETE | normalize_candidates done, but candidate pool truncated by LIMIT 1 |

---

## Issue 1: CRITICAL — Wire User Semantic to Top-K

### Problem

`search_user_learned_semantic_with_embedding()` calls `find_user_learned_semantic()` which returns `Option<SemanticMatch>` (LIMIT 1). This means only ONE user-learned match enters the candidate pool.

The top-k method **already exists**: `VerbService::find_user_learned_semantic_topk()` — it's just not wired in.

### Fix Location

`rust/src/mcp/verb_search.rs` — `search_user_learned_semantic_with_embedding()` method

### Current Code (BROKEN)

```rust
async fn search_user_learned_semantic_with_embedding(
    &self,
    user_id: Uuid,
    query_embedding: &[f32],
) -> Result<Option<VerbSearchResult>> {  // <-- Returns Option (single result)
    // ...
    let result = verb_service
        .find_user_learned_semantic(user_id, query_embedding, self.semantic_threshold)
        .await?;
    // ...
}
```

### Fixed Code

```rust
/// Search user-specific learned phrases by semantic similarity (top-k)
///
/// Returns multiple candidates for proper ambiguity detection.
async fn search_user_learned_semantic_with_embedding(
    &self,
    user_id: Uuid,
    query_embedding: &[f32],
    limit: usize,  // NEW: pass limit for top-k
) -> Result<Vec<VerbSearchResult>> {  // CHANGED: Vec instead of Option
    let verb_service = match &self.verb_service {
        Some(s) => s,
        None => return Ok(Vec::new()),
    };

    // Use top-k method instead of single-result
    let matches = verb_service
        .find_user_learned_semantic_topk(
            user_id,
            query_embedding,
            self.semantic_threshold,
            limit,
        )
        .await?;

    let mut results = Vec::new();
    for m in matches {
        let description = self.get_verb_description(&m.verb).await;
        let score = (m.similarity as f32) * m.confidence.unwrap_or(1.0);
        results.push(VerbSearchResult {
            verb: m.verb,
            score,
            source: VerbSearchSource::UserLearnedSemantic,
            matched_phrase: m.phrase,
            description,
        });
    }

    Ok(results)
}
```

### Update Call Site in `search()`

Change step 3 from:

```rust
// 3. User-specific learned (SEMANTIC match)
if results.is_empty() && user_id.is_some() {
    if let Some(ref embedding) = query_embedding {
        if let Some(result) = self
            .search_user_learned_semantic_with_embedding(user_id.unwrap(), embedding)
            .await?
        {
            if self.matches_domain(&result.verb, domain_filter)
                && !seen_verbs.contains(&result.verb)
            {
                seen_verbs.insert(result.verb.clone());
                results.push(result);
            }
        }
    }
}
```

To:

```rust
// 3. User-specific learned (SEMANTIC match) - top-k for ambiguity detection
if user_id.is_some() {
    if let Some(ref embedding) = query_embedding {
        let user_results = self
            .search_user_learned_semantic_with_embedding(user_id.unwrap(), embedding, 3)
            .await?;
        for result in user_results {
            if self.matches_domain(&result.verb, domain_filter)
                && !seen_verbs.contains(&result.verb)
            {
                seen_verbs.insert(result.verb.clone());
                results.push(result);
            }
        }
    }
}
```

**Note**: Remove the `if results.is_empty()` guard — we want ALL candidates for proper ambiguity detection.

---

## Issue 2: CRITICAL — Wire Global Learned Semantic to Top-K (or Remove)

### Problem

`search_learned_semantic_with_embedding()` calls `find_global_learned_semantic()` which returns `Option<SemanticMatch>` (LIMIT 1).

**However**: Step 6 (`search_global_semantic_with_embedding()`) already fetches from `agent.invocation_phrases` with top-k. So step 4 is **redundant**.

### Option A: Remove Step 4 (Recommended)

Delete the entire step 4 block in `search()`:

```rust
// 4. Global learned (SEMANTIC match) 
// REMOVED: This is redundant with step 6 which already fetches from
// invocation_phrases with top-k and merges with pattern_embeddings
```

### Option B: Convert to Top-K

If you want to keep it for "high-confidence learned matches get priority", convert it to top-k like Issue 1.

---

## Issue 3: Source Threshold from Searcher Config

### Problem

`IntentPipeline::process()` uses a hard-coded constant:

```rust
const SEMANTIC_THRESHOLD: f32 = 0.80;
let ambiguity_outcome = check_ambiguity(&candidates, SEMANTIC_THRESHOLD);
```

This should be sourced from `HybridVerbSearcher.semantic_threshold` for consistency.

### Fix

Add a getter to `HybridVerbSearcher`:

```rust
impl HybridVerbSearcher {
    /// Get the semantic threshold used for matching
    pub fn semantic_threshold(&self) -> f32 {
        self.semantic_threshold
    }
}
```

Then in `IntentPipeline::process()`:

```rust
// Use searcher's configured threshold for consistency
let threshold = self.verb_searcher.semantic_threshold();
let ambiguity_outcome = check_ambiguity(&candidates, threshold);
```

---

## Issue 4: Missing Acceptance Tests

### Test 1: List Commit Correctness

Already documented in previous TODO. Verifies that list items have unique ref_ids.

**Note**: This test depends on the enrichment fix in `EntityRef-List-Span-Fix-TODO.md`. Run that fix first.

### Test 2: Ambiguity Blocks LLM (Full Integration)

The unit tests for `check_ambiguity()` exist. For full integration:

```rust
#[tokio::test]
async fn test_user_learned_ambiguity_detected() {
    // This test verifies that ambiguous user-learned phrases are detected
    // 
    // Setup:
    // 1. Create HybridVerbSearcher with mock VerbService
    // 2. Mock find_user_learned_semantic_topk to return two close candidates
    // 3. Run search()
    // 4. Assert candidates contain both verbs
    // 5. Assert check_ambiguity returns Ambiguous
    
    // Implementation depends on your mocking infrastructure
}
```

---

## Summary Checklist

- [ ] **CRITICAL**: Change `search_user_learned_semantic_with_embedding()` to return `Vec` and use top-k
- [ ] **CRITICAL**: Update call site in `search()` step 3 to handle Vec and remove `if results.is_empty()` guard
- [ ] Either remove step 4 (global learned semantic) OR convert to top-k
- [ ] Add `semantic_threshold()` getter to `HybridVerbSearcher`
- [ ] Update `IntentPipeline::process()` to use searcher's threshold
- [ ] Verify all existing tests still pass
- [ ] Add integration test for user-learned ambiguity detection (optional)

---

## Files to Modify

| Issue | File | Changes |
|-------|------|---------|
| 1 | `rust/src/mcp/verb_search.rs` | Change user semantic method to top-k, update call site |
| 2 | `rust/src/mcp/verb_search.rs` | Remove or convert step 4 |
| 3 | `rust/src/mcp/verb_search.rs` | Add `semantic_threshold()` getter |
| 3 | `rust/src/mcp/intent_pipeline.rs` | Use searcher's threshold instead of hard-coded |

---

## Verification

After implementation, run:

```bash
cd rust
cargo test --lib verb_search
cargo test --lib intent_pipeline
cargo clippy -- -D warnings
```

All tests should pass. Then manually verify with a query that should trigger ambiguity (e.g., "create a client business unit" where both `cbu.create` and `cbu.ensure` are learned phrases).

---

## Why This Matters

Without this fix, ambiguity detection only works for:
- Cold-start pattern embeddings (step 6)

It **fails** for:
- User-learned phrases (step 3) — LIMIT 1
- Global learned phrases (step 4) — LIMIT 1

Since user-learned phrases have the **highest confidence** (they're explicitly taught), missing ambiguity there is particularly bad — the user taught two similar phrases and expects Claude to ask which one they meant.
