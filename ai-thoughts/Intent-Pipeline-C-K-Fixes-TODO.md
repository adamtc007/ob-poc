# Intent Pipeline Fixes TODO - Issues C, K, D/J, I

**Created**: 2025-01-21  
**Updated**: 2025-01-21 (incorporated Opus review feedback - final)  
**Context**: Opus review identified that quick wins (A, B, E, G, H) are done but determinism-critical fixes are incomplete.

---

## Claude Code Prompt Header

> **Copy this section when starting Claude Code session:**

You are implementing a bounded, surgical fix set described in `ai-thoughts/Intent-Pipeline-C-K-Fixes-TODO.md`.

**Hard constraints:**
- Do NOT refactor architecture or rename public APIs unless the TODO explicitly requires it.
- Keep changes minimal and reviewable: implement as a small sequence of commits (or clearly separated patch sections) per issue group: C → K → J/D → I.
- Do NOT change core DSL runtime/value types unless strictly necessary; prefer pipeline-local types where possible.
- Do NOT swallow parse/enrich errors silently—surface them in PipelineResult/validation_error.

**Implementation requirements:**
- Issue C: Remove synthetic unresolved ref generation. After assembling DSL, run parse → enrich → `find_unresolved_ref_locations` and return unresolved refs with entity_type + search_column + ref_id. Ensure option fields map correctly (no Option<Option<…>>).
- Issue K: Implement commit-by-ref_id against the enriched AST/program version. Require/verify `dsl_hash` to avoid applying commits to stale text. After commit, return updated DSL plus refreshed unresolved refs.
- Issues J/D: Add top-k semantic queries, unify thresholds, and implement ambiguity detection (margin=0.05) gated on `top >= threshold`. If ambiguous/no-match, do not call the LLM.
- Issue I: Demote/merge global semantic sources by union+dedupe at the VerbSearchResult layer, then apply threshold/margin once.

**Acceptance tests (must add or update):**
1. List commit correctness: `(batch.create :clients ["Allianz" "BlackRock"])` resolve first ref_id only → second remains unresolved.
2. Ambiguity blocks LLM: construct candidates with top-2 within margin and >= threshold → pipeline returns NeedsClarification/ambiguous and LLM is not invoked.

**At the end, provide:**
- A concise summary of changed files and why,
- How to run the tests,
- Any remaining TODOs explicitly marked (but aim for zero).

---

## Implementation Instructions

Implement these fixes as small, focused commits in dependency order: **C → K → J/D → I**. Each commit should compile and pass existing tests before moving to the next issue.

**Acceptance gates**: The two tests at the end of this document (List Commit Correctness + Ambiguity Blocks LLM) must pass before considering this work complete. Write these tests first as TDD anchors.

**Key gotchas to avoid**:
1. Don't double-wrap Option fields (e.g., `Some(loc.search_column.clone())` when `loc.search_column` is already `Option<String>`)
2. Parse/enrich can run even when compile/validate fails — still return unresolved refs for disambiguation UI
3. Commit must operate on **enriched** AST (where EntityRef nodes exist), not raw AST
4. Run `check_ambiguity()` only **after** union+dedupe+sort of all semantic sources

---

## Dependency Chain

```
K (commit by ref_id) depends on → C (real ref_ids from enriched AST)
D/J (ambiguity detection) depends on → top-k from DB (not LIMIT 1)
```

**Fix order**: C → K → J/D → I

---

## Issue C: NL Pipeline Generates Synthetic ref_ids (CRITICAL)

### Problem

In `rust/src/mcp/intent_pipeline.rs`, the NL path generates synthetic refs in `format_intent_value()`:

```rust
// Line ~705 - BAD: synthetic ref_id, no search_column
IntentArgValue::Unresolved { value, entity_type } => {
    let ref_id = format!("intent:{}", *ref_counter);  // SYNTHETIC!
    unresolved.push(UnresolvedRef {
        search_column: None,  // MISSING!
        ref_id: Some(ref_id), // UNUSABLE for commit
    });
```

But the direct DSL path correctly uses enriched AST with span-based ref_ids.

### Fix

**Step 1**: Add imports for enrichment

```rust
// In imports section, change:
use crate::dsl_v2::{compile, parse_program, registry};

// To:
use crate::dsl_v2::{compile, enrich_program, parse_program, runtime_registry_arc};
use crate::dsl_v2::ast::find_unresolved_ref_locations;  // canonical walker
```

**Step 2**: Split `assemble_dsl()` into string-only version

Rename `assemble_dsl()` to `assemble_dsl_string()` and change signature:

```rust
/// Assemble DSL string from structured intent (deterministic)
/// 
/// Returns string only - unresolved refs are extracted from enriched AST later (Fix C)
fn assemble_dsl_string(&self, intent: &StructuredIntent) -> Result<String> {
    let mut dsl = format!("({}", intent.verb);
    // Remove: let mut unresolved = Vec::new();
    // Remove: let mut ref_counter = 0;

    for arg in &intent.arguments {
        if matches!(arg.value, IntentArgValue::Missing { .. }) {
            continue;
        }
        // Change to use new string-only formatter
        let value_str = format_intent_value_string_only(&arg.value);
        dsl.push_str(&format!(" :{} {}", arg.name, value_str));
    }

    dsl.push(')');
    Ok(dsl)
}
```

**Step 3**: Create string-only formatter (replace `format_intent_value`)

```rust
/// Format IntentArgValue to DSL string only (Fix C - no synthetic refs)
/// 
/// Unresolved refs are extracted from the enriched AST after parsing,
/// which gives us real span-based ref_ids and search_column metadata.
fn format_intent_value_string_only(value: &IntentArgValue) -> String {
    match value {
        IntentArgValue::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
        IntentArgValue::Number(n) => n.to_string(),
        IntentArgValue::Boolean(b) => b.to_string(),
        IntentArgValue::Reference(r) => format!("@{}", r),
        IntentArgValue::Uuid(u) => format!("\"{}\"", u),
        IntentArgValue::Unresolved { value, .. } => {
            // Just emit as string - enrichment will convert to EntityRef
            // with proper span-based ref_id and search_column
            format!("\"{}\"", value.replace('"', "\\\""))
        }
        IntentArgValue::Missing { .. } => "nil".to_string(),
        IntentArgValue::List(items) => {
            let formatted: Vec<String> = items
                .iter()
                .map(format_intent_value_string_only)
                .collect();
            format!("[{}]", formatted.join(" "))
        }
        IntentArgValue::Map(entries) => {
            let formatted: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!(":{} {}", k, format_intent_value_string_only(v)))
                .collect();
            format!("{{{}}}", formatted.join(" "))
        }
    }
}
```

**Step 4**: Update `process()` to extract refs from enriched AST

> ⚠️ **IMPORTANT**: Use `find_unresolved_ref_locations()` not `extract_entity_refs_from_ast()` to avoid missing nested refs in maps/lists.

> ⚠️ **IMPORTANT**: Do NOT swallow parse/enrich errors silently. If parse fails, set validation_error with the parse error.

> ⚠️ **IMPORTANT**: Parse/enrich/walk can run even if compile fails. Unresolved refs are still returned so disambiguation UI can work even for invalid DSL. Only skip parse/enrich when parsing itself fails.

> ⚠️ **IMPORTANT**: Don't double-wrap Option fields. If `UnresolvedRefLocation` has `search_column: Option<String>` and `ref_id: Option<String>`, map them directly without wrapping in `Some()`.

```rust
// Step 5: Assemble DSL deterministically (string only)
let dsl = self.assemble_dsl_string(&intent)?;

// Step 6: Parse and enrich to extract real refs (FIX C)
// Parse → Enrich → Walk = proper span-based ref_ids + search_column
// NOTE: This runs even if compile/validate will fail - we still want unresolved refs
let (unresolved, parse_error) = match parse_program(&dsl) {
    Ok(ast) => {
        let registry = runtime_registry_arc();
        let enriched = enrich_program(ast, &registry);
        
        // Use canonical walker - handles nested maps/lists correctly
        let locations = find_unresolved_ref_locations(&enriched.program);
        
        // Map to UnresolvedRef - DON'T double-wrap Option fields
        let refs: Vec<UnresolvedRef> = locations
            .into_iter()
            .map(|loc| UnresolvedRef {
                param_name: loc.arg_key.clone(),
                search_value: loc.value.clone(),
                entity_type: loc.entity_type.clone(),      // Already Option<String>
                search_column: loc.search_column.clone(),  // Already Option<String>
                ref_id: loc.ref_id.clone(),                // Already Option<String>
            })
            .collect();
        
        (refs, None)
    }
    Err(e) => {
        // Don't swallow - surface parse error
        (vec![], Some(format!("Parse error after assembly: {:?}", e)))
    }
};

// Step 7: Validate (compile check) - runs independently of parse/enrich
let (valid, validation_error) = match &parse_error {
    Some(err) => (false, Some(err.clone())),
    None => self.validate_dsl(&dsl),
};

// Compute dsl_hash for version tracking (enables safe commit)
let dsl_hash = if dsl.is_empty() { None } else { Some(compute_dsl_hash(&dsl)) };

Ok(PipelineResult {
    intent,
    verb_candidates: candidates,
    dsl,
    dsl_hash,  // ADD THIS FIELD
    valid,
    validation_error,
    unresolved_refs: unresolved,  // Now has real refs even if valid=false
    missing_required: vec![],
    outcome: if valid {
        PipelineOutcome::Ready
    } else {
        PipelineOutcome::NeedsUserInput
    },
})
```

**Step 5**: Add `dsl_hash` to `PipelineResult`

```rust
pub struct PipelineResult {
    // ... existing fields ...
    /// Hash of DSL for version tracking (enables safe commit)
    pub dsl_hash: Option<String>,
}
```

**Step 6**: Delete old `format_intent_value()` function

Remove the old function entirely (around line 521-570).

### Verification

After fix, NL pipeline refs should have:
- `ref_id`: `"0:15-30"` format (span-based, not `"intent:0"`)
- `search_column`: `Some("name")` (from YAML lookup config)
- `entity_type`: `Some("entity")` (from enrichment)

---

## Issue K: Commit Resolution by ref_id

### Problem

Commit currently doesn't target by ref_id. Even after C is fixed, the commit endpoint needs to use ref_id to update the correct AST node.

### Prerequisites

- Issue C must be complete first (real ref_ids exist)

### Design Constraints

> ⚠️ **IMPORTANT**: Commit must apply to the session's current DSL/program version (same version the unresolved refs came from). Include a `dsl_hash` in the commit request to prevent applying a resolution to stale/different text.

> ⚠️ **CRITICAL**: Commit must parse→enrich first, then update `AstNode::EntityRef.resolved_key` by ref_id. The raw AST from parse doesn't have EntityRef nodes for the NL path — only the enriched AST does.

### API Surface

**Request**:
```rust
pub struct ResolveEntityRequest {
    /// ref_id from UnresolvedRef (e.g., "0:15-30")
    pub ref_id: String,
    /// The resolved UUID/key to commit
    pub resolved_key: String,
    /// Hash of DSL this resolution applies to (prevents race conditions)
    pub dsl_hash: String,
}
```

**Response**:
```rust
pub struct ResolveEntityResponse {
    /// Updated DSL with resolved ref
    pub dsl: String,
    /// New hash for the updated DSL
    pub dsl_hash: String,
    /// Remaining unresolved refs (so UI can continue without round-trip)
    pub remaining_unresolved: Vec<UnresolvedRef>,
    /// Whether all refs are now resolved
    pub fully_resolved: bool,
}
```

### Fix Location

`rust/src/api/agent_service.rs` - the disambiguation/commit flow

### Implementation Steps

1. Add `ref_id`, `entity_type`, `search_column` to `DisambiguationItem::EntityMatch`:
   ```rust
   DisambiguationItem::EntityMatch {
       param: String,
       search_text: String,
       matches: Vec<EntityCandidate>,
       entity_type: Option<String>,    // ADD
       search_column: Option<String>,  // ADD
       ref_id: Option<String>,         // ADD - for commit targeting
   }
   ```

2. Plumb `ref_id` through from `PipelineResult.unresolved_refs`

3. Add `dsl_hash` to disambiguation response (from `PipelineResult.dsl_hash`)

4. Implement commit-by-ref_id handler:
   ```rust
   pub async fn commit_resolution(&self, req: ResolveEntityRequest) -> Result<ResolveEntityResponse> {
       // 1. Verify dsl_hash matches current session DSL
       let current_hash = compute_dsl_hash(&self.session.current_dsl);
       if current_hash != req.dsl_hash {
           return Err(anyhow!("DSL has changed since disambiguation was generated"));
       }
       
       // 2. Parse AND ENRICH current DSL (EntityRef nodes only exist after enrichment!)
       let ast = parse_program(&self.session.current_dsl)?;
       let registry = runtime_registry_arc();
       let enriched = enrich_program(ast, &registry);
       
       // 3. Walk ENRICHED AST to find EntityRef node with matching ref_id
       // 4. Update that node's resolved_key
       let updated_ast = update_ref_by_id(&enriched.program, &req.ref_id, &req.resolved_key)?;
       
       // 5. Re-serialize AST to DSL
       let updated_dsl = emit_dsl(&updated_ast);
       let new_hash = compute_dsl_hash(&updated_dsl);
       
       // 6. Re-enrich and extract remaining unresolved
       let re_enriched = enrich_program(parse_program(&updated_dsl)?, &registry);
       let remaining = find_unresolved_ref_locations(&re_enriched.program);
       
       Ok(ResolveEntityResponse {
           dsl: updated_dsl,
           dsl_hash: new_hash,
           remaining_unresolved: remaining.into_iter().map(to_unresolved_ref).collect(),
           fully_resolved: remaining.is_empty(),
       })
   }
   ```

---

## Issue D/J: Top-K Semantic + Ambiguity Detection

### Problem

`verb_service.rs` uses `LIMIT 1` for semantic lookups - can't detect ambiguity.

### Fix Location

`rust/src/database/verb_service.rs` and `rust/src/mcp/verb_search.rs`

### Implementation Steps

**Step 1**: Add top-k variants for semantic lookups in `verb_service.rs`

```rust
/// Find user-learned phrases by semantic similarity (top-k)
pub async fn find_user_learned_semantic_topk(
    &self,
    user_id: Uuid,
    query_embedding: &[f32],
    threshold: f32,
    limit: usize,
) -> Result<Vec<SemanticMatch>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, f32, f64)>(
        r#"
        SELECT phrase, verb, confidence, 1 - (embedding <=> $1::vector) as similarity
        FROM agent.user_learned_phrases
        WHERE user_id = $2
          AND embedding IS NOT NULL
          AND 1 - (embedding <=> $1::vector) > $4
        ORDER BY embedding <=> $1::vector
        LIMIT $3
        "#,
    )
    .bind(query_embedding)
    .bind(user_id)
    .bind(limit as i32)
    .bind(threshold)
    .fetch_all(&self.pool)
    .await?;

    Ok(rows.into_iter().map(|(phrase, verb, confidence, similarity)| {
        SemanticMatch { phrase, verb, similarity, confidence: Some(confidence), category: None }
    }).collect())
}

/// Find global learned phrases by semantic similarity (top-k)
pub async fn find_global_learned_semantic_topk(
    &self,
    query_embedding: &[f32],
    threshold: f32,
    limit: usize,
) -> Result<Vec<SemanticMatch>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, f64)>(
        r#"
        SELECT phrase, verb, 1 - (embedding <=> $1::vector) as similarity
        FROM agent.invocation_phrases
        WHERE embedding IS NOT NULL
          AND 1 - (embedding <=> $1::vector) > $3
        ORDER BY embedding <=> $1::vector
        LIMIT $2
        "#,
    )
    .bind(query_embedding)
    .bind(limit as i32)
    .bind(threshold)
    .fetch_all(&self.pool)
    .await?;

    Ok(rows.into_iter().map(|(phrase, verb, similarity)| {
        SemanticMatch { phrase, verb, similarity, confidence: None, category: None }
    }).collect())
}
```

**Step 2**: Add `VerbSearchOutcome` enum in `verb_search.rs`

```rust
pub enum VerbSearchOutcome {
    /// Clear winner - proceed with LLM extraction
    Matched(VerbSearchResult),
    /// Top candidates too close - need user clarification
    Ambiguous { 
        top: VerbSearchResult, 
        runner_up: VerbSearchResult, 
        margin: f32 
    },
    /// Nothing matched threshold
    NoMatch,
}
```

**Step 3**: Implement margin policy with threshold guard

> ⚠️ **IMPORTANT**: Ambiguity rule must apply only when `top >= threshold`. Don't flag ambiguity for low-confidence matches.

> ⚠️ **IMPORTANT**: Run `check_ambiguity()` only **after** union+dedupe+sort (Issue I), so margin reflects the true best alternatives across all semantic sources. Do not run ambiguity check on individual source results.

```rust
const AMBIGUITY_MARGIN: f32 = 0.05;

fn check_ambiguity(candidates: &[VerbSearchResult], threshold: f32) -> VerbSearchOutcome {
    match candidates.first() {
        None => VerbSearchOutcome::NoMatch,
        Some(top) if top.score < threshold => VerbSearchOutcome::NoMatch,
        Some(top) => {
            match candidates.get(1) {
                // Only one candidate above threshold
                None => VerbSearchOutcome::Matched(top.clone()),
                Some(runner_up) if runner_up.score < threshold => {
                    // Runner-up below threshold - clear winner
                    VerbSearchOutcome::Matched(top.clone())
                }
                Some(runner_up) => {
                    let margin = top.score - runner_up.score;
                    if margin < AMBIGUITY_MARGIN {
                        VerbSearchOutcome::Ambiguous {
                            top: top.clone(),
                            runner_up: runner_up.clone(),
                            margin,
                        }
                    } else {
                        VerbSearchOutcome::Matched(top.clone())
                    }
                }
            }
        }
    }
}
```

**Step 4**: Update pipeline to handle ambiguity

In `intent_pipeline.rs`, after verb search:

```rust
let outcome = check_ambiguity(&candidates, semantic_threshold);
match outcome {
    VerbSearchOutcome::Matched(verb_result) => {
        // Continue with LLM extraction using verb_result.verb
    }
    VerbSearchOutcome::Ambiguous { top, runner_up, margin } => {
        // DO NOT call LLM - return for user clarification
        return Ok(PipelineResult {
            intent: StructuredIntent::empty(),
            verb_candidates: vec![top, runner_up],
            dsl: String::new(),
            dsl_hash: None,
            valid: false,
            validation_error: Some(format!(
                "Ambiguous verb match (margin={:.3}). Did you mean '{}' or '{}'?",
                margin, top.verb, runner_up.verb
            )),
            unresolved_refs: vec![],
            missing_required: vec![],
            outcome: PipelineOutcome::NeedsClarification,
        });
    }
    VerbSearchOutcome::NoMatch => {
        return Ok(PipelineResult {
            outcome: PipelineOutcome::NoMatch,
            dsl_hash: None,
            // ...
        });
    }
}
```

---

## Issue I: Union/Dedupe Global Semantic Sources

### Problem

Two competing global semantic sources:
- `agent.invocation_phrases` (learned)
- `"ob-poc".verb_pattern_embeddings` (cold start)

### Design Constraint

> ⚠️ **IMPORTANT**: Union/dedupe at `VerbSearchResult` layer, not `SemanticMatch`. Preserve source tier, category/domain, and evidence phrase for debugging and UI display.

> ⚠️ **IMPORTANT**: Run `check_ambiguity()` only after union+dedupe+sort, so margin reflects the true best alternatives. This prevents the bug where invocation_phrases yields ambiguous pair but pattern_embeddings would have provided a clear winner.

### Implementation

In `HybridVerbSearcher::search_semantic()`:

```rust
// 1. Fetch top-k from BOTH sources
let learned = verb_service
    .find_global_learned_semantic_topk(embedding, threshold, 5)
    .await?;
let patterns = verb_service
    .search_verb_patterns_semantic(embedding, 5, threshold)
    .await?;

// 2. Convert to VerbSearchResult (preserving source metadata)
let learned_results: Vec<VerbSearchResult> = learned
    .into_iter()
    .map(|m| VerbSearchResult {
        verb: m.verb,
        score: m.similarity as f32,
        source: VerbSearchSource::GlobalLearned,
        matched_phrase: m.phrase,
        description: None,
        category: m.category,
    })
    .collect();

let pattern_results: Vec<VerbSearchResult> = patterns
    .into_iter()
    .map(|m| VerbSearchResult {
        verb: m.verb,
        score: m.similarity as f32,
        source: VerbSearchSource::PatternEmbedding,
        matched_phrase: m.phrase,
        description: None,
        category: m.category,
    })
    .collect();

// 3. Union and dedupe by verb, keeping highest score + most informative metadata
let mut combined: HashMap<String, VerbSearchResult> = HashMap::new();
for result in learned_results.into_iter().chain(pattern_results) {
    combined
        .entry(result.verb.clone())
        .and_modify(|existing| {
            if result.score > existing.score {
                *existing = result.clone();
            }
        })
        .or_insert(result);
}

// 4. Sort by score descending
let mut sorted: Vec<VerbSearchResult> = combined.into_values().collect();
sorted.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

// 5. Apply ambiguity check on UNIFIED list (not earlier!)
let outcome = check_ambiguity(&sorted, threshold);
```

---

## Required Tests

> ⚠️ **TDD**: Write these tests FIRST as acceptance gates. Implementation is complete when both pass.

### Test 1: List Commit Correctness

Verifies that resolving one ref in a list doesn't affect others.

```rust
#[tokio::test]
async fn test_list_commit_resolves_single_ref() {
    // DSL with list of unresolved refs
    let dsl = r#"(batch.create :clients ["Allianz" "BlackRock" "Vanguard"])"#;
    
    // Parse and enrich (EntityRef nodes created here)
    let ast = parse_program(dsl).unwrap();
    let registry = runtime_registry_arc();
    let enriched = enrich_program(ast, &registry);
    
    // Get unresolved refs
    let refs = find_unresolved_ref_locations(&enriched.program);
    assert_eq!(refs.len(), 3);
    
    // Commit resolution for first ref only
    let first_ref_id = refs[0].ref_id.clone().unwrap();
    let resolved_uuid = "550e8400-e29b-41d4-a716-446655440000";
    
    // Update must operate on ENRICHED AST
    let updated_ast = update_ref_by_id(&enriched.program, &first_ref_id, resolved_uuid).unwrap();
    
    // Re-check unresolved
    let remaining = find_unresolved_ref_locations(&updated_ast);
    assert_eq!(remaining.len(), 2);  // Two still unresolved
    
    // Verify first is now resolved
    // (walk AST and check resolved_key is set for first item only)
}
```

### Test 2: Ambiguity Blocks LLM

Verifies that ambiguous verb matches don't trigger LLM extraction.

```rust
#[tokio::test]
async fn test_ambiguity_blocks_llm_call() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    
    // Mock LLM that counts calls
    let llm_call_count = Arc::new(AtomicUsize::new(0));
    let mock_llm = MockLlmClient::new(llm_call_count.clone());
    
    // Create pipeline with mock LLM
    let searcher = HybridVerbSearcher::with_forced_ambiguity(
        // Returns two candidates within AMBIGUITY_MARGIN (both above threshold)
        vec![
            VerbSearchResult { verb: "cbu.create".into(), score: 0.85, .. },
            VerbSearchResult { verb: "cbu.ensure".into(), score: 0.83, .. },  // margin = 0.02 < 0.05
        ]
    );
    let pipeline = IntentPipeline::with_llm(searcher, Arc::new(mock_llm));
    
    // Process instruction
    let result = pipeline.process("create a new cbu", None).await.unwrap();
    
    // Verify outcome
    assert_eq!(result.outcome, PipelineOutcome::NeedsClarification);
    assert_eq!(result.verb_candidates.len(), 2);
    
    // Verify LLM was NOT called
    assert_eq!(llm_call_count.load(Ordering::SeqCst), 0);
}
```

---

## Summary Checklist

- [ ] **C**: NL refs from enriched AST using `find_unresolved_ref_locations()` (unlocks K)
- [ ] **C**: Don't swallow parse/enrich errors - surface in validation_error
- [ ] **C**: Parse/enrich runs even if compile fails - still return unresolved refs
- [ ] **C**: Don't double-wrap Option fields in mapping
- [ ] **C**: Add `dsl_hash` to `PipelineResult`
- [ ] **K**: Commit by ref_id on ENRICHED AST (not raw)
- [ ] **K**: Verify dsl_hash matches before commit
- [ ] **K**: Return updated dsl + remaining_unresolved
- [ ] **J/D**: Top-k semantic from DB (`*_topk()` methods)
- [ ] **J/D**: Ambiguity gate with threshold guard (only when top >= threshold)
- [ ] **J/D**: Run ambiguity check AFTER union+dedupe+sort
- [ ] **I**: Union/dedupe at VerbSearchResult layer, preserving source metadata
- [ ] **Test**: List commit correctness (TDD anchor)
- [ ] **Test**: Ambiguity blocks LLM (TDD anchor)

## Files to Modify

| Issue | File | Changes |
|-------|------|---------|
| C | `rust/src/mcp/intent_pipeline.rs` | Split assemble_dsl, use `find_unresolved_ref_locations()`, add dsl_hash |
| C | `rust/src/dsl_v2/ast.rs` | Ensure `find_unresolved_ref_locations()` exists and handles nested structures |
| K | `rust/src/api/agent_service.rs` | commit-by-ref_id endpoint, verify dsl_hash |
| J/D | `rust/src/database/verb_service.rs` | Add `*_topk()` methods |
| J/D | `rust/src/mcp/verb_search.rs` | Add VerbSearchOutcome, check_ambiguity with threshold guard |
| I | `rust/src/mcp/verb_search.rs` | Union/dedupe at VerbSearchResult layer, run ambiguity after merge |
