# Claude Code Task: Verb Centroids for Semantic Lane Optimization

## Prerequisites

**WAIT UNTIL:**
1. Phrase merge is complete (V1 phrases + draft files merged into V2)
2. Re-embed complete
3. Harness baseline recorded

**Current baseline (pre-centroid):**
- Top-1: 71.7% (test_all_scenarios)
- Top-1 with optimal thresholds: 81.5%
- Confidently wrong: 0
- Ambiguity rate: 28.3%

---

## Goal

Replace noisy 10,160-pattern retrieval with:
1. **Centroid shortlist**: Query ~500 verb centroids first
2. **Pattern refinement**: Use pattern-level matches only within top-K verbs

This reduces variance and gives larger score gaps on "human" prompts.

---

## Step 1: Database Migration

Create file: `migrations/XXX_verb_centroids.sql`

```sql
-- Verb centroid vectors for semantic lane optimization
-- One stable "prototype" vector per verb (mean of normalized phrase embeddings)

CREATE TABLE IF NOT EXISTS "ob-poc".verb_centroids (
    verb_name TEXT PRIMARY KEY,
    embedding VECTOR(384) NOT NULL,
    phrase_count INT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- IVFFlat index for fast cosine similarity search
CREATE INDEX IF NOT EXISTS idx_verb_centroids_embedding_ivfflat
    ON "ob-poc".verb_centroids
    USING ivfflat (embedding vector_cosine_ops)
    WITH (lists = 100);

COMMENT ON TABLE "ob-poc".verb_centroids IS 
    'Centroid vectors per verb - mean of normalized phrase embeddings for stable semantic matching';
```

Run: `sqlx migrate run`

---

## Step 2: Centroid Computation Functions

Add to `rust/crates/ob-semantic-matcher/src/centroid.rs`:

```rust
//! Centroid computation for verb embeddings
//! 
//! Centroids provide a stable "prototype" vector per verb by averaging
//! all phrase embeddings. This reduces variance from individual phrases.

/// L2 norm of a vector
pub fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Normalize vector to unit length
pub fn normalize(v: Vec<f32>) -> Vec<f32> {
    let n = l2_norm(&v);
    if n > 0.0 {
        v.into_iter().map(|x| x / n).collect()
    } else {
        v
    }
}

/// Compute centroid from a list of embeddings.
/// 
/// Algorithm:
/// 1. Normalize each input vector (important for cosine similarity)
/// 2. Sum all normalized vectors
/// 3. Average
/// 4. Normalize final result
/// 
/// # Panics
/// Panics if vectors is empty or vectors have different dimensions.
pub fn compute_centroid(vectors: &[Vec<f32>]) -> Vec<f32> {
    assert!(!vectors.is_empty(), "centroid requires at least 1 vector");
    let dim = vectors[0].len();
    
    // Accumulator
    let mut acc = vec![0.0f32; dim];
    
    // Sum normalized vectors
    for v in vectors {
        assert_eq!(v.len(), dim, "all vectors must have same dimension");
        let normalized = normalize(v.clone());
        for (i, x) in normalized.iter().enumerate() {
            acc[i] += x;
        }
    }
    
    // Average
    let n = vectors.len() as f32;
    for x in &mut acc {
        *x /= n;
    }
    
    // Normalize final centroid
    normalize(acc)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_l2_norm() {
        let v = vec![3.0, 4.0];
        assert!((l2_norm(&v) - 5.0).abs() < 1e-6);
    }
    
    #[test]
    fn test_normalize() {
        let v = normalize(vec![3.0, 4.0]);
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);
    }
    
    #[test]
    fn test_centroid_single() {
        let vectors = vec![vec![1.0, 0.0]];
        let c = compute_centroid(&vectors);
        assert!((c[0] - 1.0).abs() < 1e-6);
        assert!((c[1] - 0.0).abs() < 1e-6);
    }
    
    #[test]
    fn test_centroid_multiple() {
        let vectors = vec![
            vec![1.0, 0.0],
            vec![0.0, 1.0],
        ];
        let c = compute_centroid(&vectors);
        // Average of [1,0] and [0,1] normalized = [0.5, 0.5] normalized = [0.707, 0.707]
        let expected = 1.0 / 2.0_f32.sqrt();
        assert!((c[0] - expected).abs() < 1e-5);
        assert!((c[1] - expected).abs() < 1e-5);
    }
}
```

---

## Step 3: Update populate_embeddings.rs

Add centroid computation at the end of the embedding population:

```rust
use std::collections::HashMap;
use pgvector::Vector;

/// Compute and store centroids for all verbs
/// 
/// Call this AFTER all pattern embeddings are populated.
pub async fn compute_and_store_centroids(pool: &PgPool) -> anyhow::Result<CentroidStats> {
    println!("\n📊 Computing verb centroids...");
    
    // 1) Load all pattern embeddings
    let rows: Vec<(String, Vector)> = sqlx::query_as(
        r#"
        SELECT verb_name, embedding
        FROM "ob-poc".verb_pattern_embeddings
        WHERE embedding IS NOT NULL
        "#
    )
    .fetch_all(pool)
    .await?;
    
    println!("  Loaded {} pattern embeddings", rows.len());
    
    // 2) Group by verb
    let mut map: HashMap<String, Vec<Vec<f32>>> = HashMap::new();
    for (verb_name, embedding) in rows {
        let vec: Vec<f32> = embedding.into();
        map.entry(verb_name).or_default().push(vec);
    }
    
    println!("  Found {} unique verbs", map.len());
    
    // 3) Compute + upsert centroids
    let mut inserted = 0;
    let mut updated = 0;
    
    for (verb_name, vecs) in &map {
        if vecs.is_empty() { continue; }
        
        let centroid = crate::centroid::compute_centroid(vecs);
        let phrase_count = vecs.len() as i32;
        let centroid_vec = Vector::from(centroid);
        
        let result = sqlx::query(
            r#"
            INSERT INTO "ob-poc".verb_centroids (verb_name, embedding, phrase_count, updated_at)
            VALUES ($1, $2, $3, now())
            ON CONFLICT (verb_name)
            DO UPDATE SET 
                embedding = EXCLUDED.embedding,
                phrase_count = EXCLUDED.phrase_count,
                updated_at = now()
            RETURNING (xmax = 0) as inserted
            "#
        )
        .bind(verb_name)
        .bind(&centroid_vec)
        .bind(phrase_count)
        .fetch_one(pool)
        .await?;
        
        let was_insert: bool = result.get("inserted");
        if was_insert {
            inserted += 1;
        } else {
            updated += 1;
        }
    }
    
    // 4) Cleanup orphaned centroids (verbs no longer in patterns)
    let deleted = sqlx::query_scalar::<_, i64>(
        r#"
        WITH deleted AS (
            DELETE FROM "ob-poc".verb_centroids
            WHERE verb_name NOT IN (
                SELECT DISTINCT verb_name FROM "ob-poc".verb_pattern_embeddings
            )
            RETURNING 1
        )
        SELECT COUNT(*) FROM deleted
        "#
    )
    .fetch_one(pool)
    .await?;
    
    let stats = CentroidStats {
        total_verbs: map.len(),
        inserted,
        updated,
        deleted: deleted as usize,
    };
    
    println!("  ✅ Centroids: {} inserted, {} updated, {} deleted", 
        stats.inserted, stats.updated, stats.deleted);
    
    Ok(stats)
}

#[derive(Debug)]
pub struct CentroidStats {
    pub total_verbs: usize,
    pub inserted: usize,
    pub updated: usize,
    pub deleted: usize,
}
```

Update `main()` to call this after pattern embedding:

```rust
// After populating patterns...
compute_and_store_centroids(&pool).await?;
```

---

## Step 4: Add Centroid Query to VerbService

Add to `rust/src/database/verb_service.rs`:

```rust
/// Row for centroid query results
#[derive(Debug, sqlx::FromRow)]
pub struct VerbCentroidMatch {
    pub verb_name: String,
    pub score: f32,
    pub phrase_count: i32,
}

impl VerbService {
    /// Query verb centroids for semantic shortlist
    /// 
    /// Returns top-K verbs by centroid similarity.
    /// Use this to get a candidate set, then refine with pattern-level matches.
    pub async fn query_centroids(
        &self,
        query_embedding: &[f32],
        limit: i32,
    ) -> Result<Vec<VerbCentroidMatch>, VerbServiceError> {
        let embedding_vec = Vector::from(query_embedding.to_vec());
        
        let matches = sqlx::query_as::<_, VerbCentroidMatch>(
            r#"
            SELECT 
                verb_name,
                1 - (embedding <=> $1::vector) as score,
                phrase_count
            FROM "ob-poc".verb_centroids
            ORDER BY embedding <=> $1::vector
            LIMIT $2
            "#
        )
        .bind(&embedding_vec)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(matches)
    }
    
    /// Query verb centroids with minimum score threshold
    pub async fn query_centroids_with_threshold(
        &self,
        query_embedding: &[f32],
        limit: i32,
        min_score: f32,
    ) -> Result<Vec<VerbCentroidMatch>, VerbServiceError> {
        let embedding_vec = Vector::from(query_embedding.to_vec());
        
        let matches = sqlx::query_as::<_, VerbCentroidMatch>(
            r#"
            SELECT 
                verb_name,
                1 - (embedding <=> $1::vector) as score,
                phrase_count
            FROM "ob-poc".verb_centroids
            WHERE 1 - (embedding <=> $1::vector) >= $3
            ORDER BY embedding <=> $1::vector
            LIMIT $2
            "#
        )
        .bind(&embedding_vec)
        .bind(limit)
        .bind(min_score)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(matches)
    }
}
```

---

## Step 5: Update Semantic Lane in Intent Router

Modify the semantic fallback to use centroid shortlist:

```rust
// In semantic_fallback.rs or intent_router.rs

/// Semantic search with centroid shortlist optimization
/// 
/// Strategy:
/// 1. Query centroids to get top-K candidate verbs (fast, stable)
/// 2. Query patterns only for those K verbs (precise, evidenced)
/// 3. Combine scores: centroid provides baseline, patterns provide evidence
pub async fn semantic_search_with_centroids(
    masked_input: &MaskedInput,
    embedder: &CandleEmbedder,
    verb_service: &VerbService,
) -> Result<Vec<VerbScore>> {
    // Embed the masked query
    let query_embedding = embedder.embed_query(&masked_input.masked_text).await?;
    
    // Step 1: Get centroid shortlist (top 25 verbs)
    let centroid_matches = verb_service
        .query_centroids(&query_embedding, 25)
        .await?;
    
    if centroid_matches.is_empty() {
        return Ok(vec![]);
    }
    
    // Step 2: Get pattern-level evidence for shortlisted verbs only
    let shortlist_verbs: Vec<&str> = centroid_matches
        .iter()
        .map(|m| m.verb_name.as_str())
        .collect();
    
    let pattern_matches = verb_service
        .search_patterns_for_verbs(&query_embedding, &shortlist_verbs, 10)
        .await?;
    
    // Step 3: Combine scores
    // - Centroid score: stable baseline
    // - Pattern score: specific evidence
    // - Weight: 0.4 * centroid + 0.6 * best_pattern (tunable)
    const CENTROID_WEIGHT: f32 = 0.4;
    const PATTERN_WEIGHT: f32 = 0.6;
    
    let mut verb_scores: HashMap<String, VerbScore> = HashMap::new();
    
    for cm in &centroid_matches {
        verb_scores.insert(cm.verb_name.clone(), VerbScore {
            verb_fqn: cm.verb_name.clone(),
            centroid_score: cm.score,
            pattern_score: 0.0,
            best_pattern: None,
            combined_score: cm.score * CENTROID_WEIGHT, // baseline
        });
    }
    
    for pm in &pattern_matches {
        if let Some(vs) = verb_scores.get_mut(&pm.verb_name) {
            if pm.similarity > vs.pattern_score {
                vs.pattern_score = pm.similarity;
                vs.best_pattern = Some(pm.phrase.clone());
            }
            vs.combined_score = 
                CENTROID_WEIGHT * vs.centroid_score + 
                PATTERN_WEIGHT * vs.pattern_score;
        }
    }
    
    // Sort by combined score
    let mut results: Vec<_> = verb_scores.into_values().collect();
    results.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
    
    Ok(results)
}

#[derive(Debug, Clone)]
pub struct VerbScore {
    pub verb_fqn: String,
    pub centroid_score: f32,
    pub pattern_score: f32,
    pub best_pattern: Option<String>,
    pub combined_score: f32,
}
```

---

## Step 6: Add Pattern Query for Shortlisted Verbs

Add to `VerbService`:

```rust
/// Search patterns only for specific verbs (for centroid refinement)
pub async fn search_patterns_for_verbs(
    &self,
    query_embedding: &[f32],
    verb_names: &[&str],
    limit_per_verb: i32,
) -> Result<Vec<PatternMatch>, VerbServiceError> {
    if verb_names.is_empty() {
        return Ok(vec![]);
    }
    
    let embedding_vec = Vector::from(query_embedding.to_vec());
    
    // Use ANY for verb filter
    let matches = sqlx::query_as::<_, PatternMatch>(
        r#"
        SELECT 
            verb_name,
            pattern_phrase as phrase,
            1 - (embedding <=> $1::vector) as similarity
        FROM "ob-poc".verb_pattern_embeddings
        WHERE verb_name = ANY($2)
          AND embedding IS NOT NULL
        ORDER BY embedding <=> $1::vector
        LIMIT $3
        "#
    )
    .bind(&embedding_vec)
    .bind(verb_names)
    .bind(limit_per_verb * verb_names.len() as i32)
    .fetch_all(&self.pool)
    .await?;
    
    Ok(matches)
}
```

---

## Step 7: Run Harness and Compare

After implementation:

```bash
# Re-run harness
cargo test --package ob-poc verb_search -- --nocapture

# Or if you have the xtask:
cargo x intent eval
```

**Record these metrics (before vs after centroids):**

| Metric | Before | After |
|--------|--------|-------|
| Top-1 accuracy | | |
| Top-3 accuracy | | |
| Ambiguity rate | | |
| Collision cases | | |
| Confidently wrong | | |

---

## Tuning Parameters

If results need adjustment:

```rust
// Centroid shortlist size (default: 25)
const CENTROID_SHORTLIST: i32 = 25;

// Score combination weights (default: 0.4/0.6)
const CENTROID_WEIGHT: f32 = 0.4;
const PATTERN_WEIGHT: f32 = 0.6;

// Minimum centroid score to consider (optional filter)
const MIN_CENTROID_SCORE: f32 = 0.3;
```

---

## Success Criteria

- [ ] Migration runs without error
- [ ] `verb_centroids` table populated with ~500-1000 verbs
- [ ] `phrase_count` per verb looks reasonable (no verb with 1 phrase unless expected)
- [ ] Centroid query returns results in <10ms
- [ ] Harness shows improvement OR no regression in:
  - Top-1 accuracy
  - Confidently wrong (must stay 0)
  - Score gaps (top-1 vs top-2) should increase

---

## Files to Create/Modify

| File | Action |
|------|--------|
| `migrations/XXX_verb_centroids.sql` | CREATE |
| `crates/ob-semantic-matcher/src/centroid.rs` | CREATE |
| `crates/ob-semantic-matcher/src/lib.rs` | ADD `mod centroid;` |
| `crates/ob-semantic-matcher/src/bin/populate_embeddings.rs` | MODIFY |
| `src/database/verb_service.rs` | MODIFY |
| `src/mcp/semantic_fallback.rs` or `intent_router.rs` | MODIFY |
