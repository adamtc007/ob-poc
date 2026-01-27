# Claude Code Task: Centroid Option A - Candidate Generator Only

## Context

**Current state:**
- Centroid infrastructure exists and works (tables, computation, queries)
- Centroid path is **disabled** because it regressed accuracy (79.2% → 66.0%)
- Root cause: Centroids were used for **scoring**, not **candidate generation**

**Baseline to preserve:**
- 79.2% overall accuracy
- 0 confidently wrong

---

## The Fix: Option A

**Wrong (current disabled implementation):**
```
query → centroids → score by centroid → rank
```

**Correct (Option A):**
```
query → patterns (top-K) + centroids (top-M) → UNION candidates → rank by PATTERN score only
```

Centroids are a **recall booster** to widen the candidate set. Patterns are the **truth** for scoring.

---

## Implementation Steps

### Step 1: Change centroid path to return candidates only

The centroid stage returns verbs to consider, not scores for ranking:

```rust
#[derive(Debug, Clone)]
pub struct CentroidCandidate {
    pub verb_name: String,
    pub centroid_score: f32,  // For logging only, NOT for ranking
}
```

### Step 2: Add pattern re-scoring for centroid candidates

This is the key insight: centroid-only verbs need pattern scores to compete fairly.

Add to `VerbService`:

```rust
/// Re-score specific verbs by their best pattern match
/// 
/// Used to score centroid-only candidates on the same scale as pattern candidates.
pub async fn pattern_score_for_verbs(
    &self,
    query_embedding: &[f32],
    verb_names: &[String],
) -> Result<HashMap<String, PatternScore>, VerbServiceError> {
    if verb_names.is_empty() {
        return Ok(HashMap::new());
    }
    
    let embedding_vec = Vector::from(query_embedding.to_vec());
    
    // Get best pattern match per verb
    let rows: Vec<(String, f32, String)> = sqlx::query_as(
        r#"
        SELECT DISTINCT ON (verb_name)
            verb_name,
            1 - (embedding <=> $1::vector) as score,
            pattern_phrase
        FROM "ob-poc".verb_pattern_embeddings
        WHERE verb_name = ANY($2)
          AND embedding IS NOT NULL
        ORDER BY verb_name, embedding <=> $1::vector
        "#
    )
    .bind(&embedding_vec)
    .bind(verb_names)
    .fetch_all(&self.pool)
    .await?;
    
    let mut result = HashMap::new();
    for (verb, score, phrase) in rows {
        result.insert(verb, PatternScore {
            score,
            best_phrase: phrase,
        });
    }
    
    Ok(result)
}

#[derive(Debug, Clone)]
pub struct PatternScore {
    pub score: f32,
    pub best_phrase: String,
}
```

### Step 3: Build union candidate set

In `HybridVerbSearcher` or semantic fallback:

```rust
pub async fn search_with_centroid_recall(
    &self,
    masked_query: &str,
    query_embedding: &[f32],
) -> Result<Vec<VerbCandidate>> {
    const K_PATTERNS: usize = 10;
    const M_CENTROIDS: usize = 25;
    
    let mut candidates: HashMap<String, VerbCandidate> = HashMap::new();
    
    // A) Get pattern top-K (these have real scores)
    let pattern_topk = self.verb_service
        .search_patterns(query_embedding, K_PATTERNS as i32)
        .await?;
    
    for p in pattern_topk {
        candidates.insert(p.verb_name.clone(), VerbCandidate {
            verb_name: p.verb_name,
            pattern_score: Some(p.similarity),
            best_phrase: Some(p.phrase),
            centroid_score: None,
            source: CandidateSource::Pattern,
        });
    }
    
    // B) Get centroid shortlist (recall boost)
    let centroid_topm = self.verb_service
        .query_centroids(query_embedding, M_CENTROIDS as i32)
        .await?;
    
    // Find verbs that centroids found but patterns missed
    let missing: Vec<String> = centroid_topm
        .iter()
        .map(|c| c.verb_name.clone())
        .filter(|v| !candidates.contains_key(v))
        .collect();
    
    // C) Re-score missing verbs via patterns (so they compete fairly)
    let rescored = self.verb_service
        .pattern_score_for_verbs(query_embedding, &missing)
        .await?;
    
    // D) Merge centroid info into existing candidates
    for c in &centroid_topm {
        if let Some(cand) = candidates.get_mut(&c.verb_name) {
            cand.centroid_score = Some(c.score);
        }
    }
    
    // E) Add centroid-only candidates with their pattern rescores
    for (verb, ps) in rescored {
        candidates.insert(verb.clone(), VerbCandidate {
            verb_name: verb,
            pattern_score: Some(ps.score),
            best_phrase: Some(ps.best_phrase),
            centroid_score: centroid_topm.iter()
                .find(|c| c.verb_name == verb)
                .map(|c| c.score),
            source: CandidateSource::CentroidRecall,
        });
    }
    
    // F) Rank by pattern score ONLY
    let mut results: Vec<_> = candidates.into_values().collect();
    results.sort_by(|a, b| {
        let sa = a.pattern_score.unwrap_or(0.0);
        let sb = b.pattern_score.unwrap_or(0.0);
        sb.partial_cmp(&sa).unwrap()
    });
    
    Ok(results)
}

#[derive(Debug, Clone)]
pub struct VerbCandidate {
    pub verb_name: String,
    pub pattern_score: Option<f32>,
    pub best_phrase: Option<String>,
    pub centroid_score: Option<f32>,  // For logging only
    pub source: CandidateSource,
}

#[derive(Debug, Clone, Copy)]
pub enum CandidateSource {
    Pattern,         // Found by pattern search
    CentroidRecall,  // Found by centroid, rescored by pattern
}
```

### Step 4: Acceptance gate uses pattern score only

**No changes needed** - existing gate already uses pattern scores:

```rust
// Accept if:
// pattern_top >= semantic_threshold AND 
// (pattern_top - pattern_2nd) >= AMBIGUITY_MARGIN

let top = results.first().and_then(|r| r.pattern_score).unwrap_or(0.0);
let second = results.get(1).and_then(|r| r.pattern_score).unwrap_or(0.0);

if top >= SEMANTIC_THRESHOLD && (top - second) >= AMBIGUITY_MARGIN {
    // Accept top candidate
} else {
    // Ambiguous - return top-N for clarification
}
```

### Step 5: Add instrumentation

Behind `OB_INTENT_TRACE=1`:

```rust
if std::env::var("OB_INTENT_TRACE").is_ok() {
    eprintln!("=== Centroid Recall Trace ===");
    eprintln!("Query: {}", masked_query);
    
    eprintln!("\nPattern top-5:");
    for (i, c) in results.iter().filter(|c| c.source == CandidateSource::Pattern).take(5).enumerate() {
        eprintln!("  {}. {} (pattern={:.3}) \"{}\"", 
            i+1, c.verb_name, c.pattern_score.unwrap_or(0.0), 
            c.best_phrase.as_deref().unwrap_or(""));
    }
    
    eprintln!("\nCentroid shortlist top-5:");
    for (i, c) in centroid_topm.iter().take(5).enumerate() {
        eprintln!("  {}. {} (centroid={:.3})", i+1, c.verb_name, c.score);
    }
    
    eprintln!("\nCentroid-only candidates added: {}", missing.len());
    for (verb, ps) in &rescored {
        eprintln!("  - {} rescored to pattern={:.3}", verb, ps.score);
    }
    
    eprintln!("\nFinal ranking (by pattern score):");
    for (i, c) in results.iter().take(5).enumerate() {
        let src = match c.source {
            CandidateSource::Pattern => "P",
            CandidateSource::CentroidRecall => "C",
        };
        eprintln!("  {}. [{}] {} (pattern={:.3}, centroid={:.3})",
            i+1, src, c.verb_name,
            c.pattern_score.unwrap_or(0.0),
            c.centroid_score.unwrap_or(0.0));
    }
}
```

---

## Parameters

```rust
const K_PATTERNS: usize = 10;      // Pattern top-K (existing)
const M_CENTROIDS: usize = 25;     // Centroid shortlist size
// Thresholds unchanged from baseline
```

---

## Success Criteria

| Metric | Baseline | After Option A | Pass? |
|--------|----------|----------------|-------|
| Overall accuracy | 79.2% | ≥79.2% | |
| Confidently wrong | 0 | 0 | |
| Top-3 contains | 67.9% | ≥67.9% | |

**Expected improvement:**
- Some "no match" cases become correct because centroid shortlist injects the right verb
- No regression because pattern scores are the sole ranking signal

**If accuracy drops:**
- Check instrumentation: are centroid-only candidates replacing pattern top-K?
- Verify pattern rescoring is working (scores should be on same scale)

---

## Files to Modify

| File | Change |
|------|--------|
| `src/database/verb_service.rs` | Add `pattern_score_for_verbs()` |
| `src/mcp/verb_search.rs` or `hybrid_verb_searcher.rs` | Implement union + rank logic |

---

## Test Commands

```bash
# Run harness
cargo test --package ob-poc verb_search -- --nocapture

# With trace logging
OB_INTENT_TRACE=1 cargo test --package ob-poc test_specific_scenario -- --nocapture
```
