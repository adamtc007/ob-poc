//! Semantic embeddings for registry objects — text generation, staleness tracking,
//! and cosine-similarity search.
//!
//! Each registry snapshot can have an associated embedding vector.  The canonical
//! text representation is generated deterministically from the snapshot definition,
//! so staleness can be detected by comparing `version_hash` against the current
//! snapshot.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "database")]
use anyhow::Result;
#[cfg(feature = "database")]
use sqlx::PgPool;

// ── Types ────────────────────────────────────────────────────────────────────

/// Result from a similarity search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SimilarityResult {
    pub snapshot_id: Uuid,
    pub object_type: String,
    pub object_id: Uuid,
    pub score: f64,
    pub name: Option<String>,
    /// Whether the embedding is stale (version_hash mismatch with current snapshot).
    #[serde(default)]
    pub stale: bool,
}

/// Internal row type for similarity search queries.
#[cfg(feature = "database")]
#[derive(sqlx::FromRow)]
struct EmbeddingSearchRow {
    snapshot_id: Uuid,
    object_type: String,
    object_id: Uuid,
    embedding: serde_json::Value,
    version_hash: String,
    name: Option<String>,
    current_hash: String,
}

// ── Store ────────────────────────────────────────────────────────────────────

pub(crate) struct EmbeddingStore;

impl EmbeddingStore {
    /// Search for similar embeddings using application-side cosine similarity.
    ///
    /// Since sem_reg embeddings are stored as JSONB (not pgvector), we fetch
    /// candidate embeddings and compute cosine similarity in Rust.  Results
    /// are returned sorted by descending similarity score.
    #[cfg(feature = "database")]
    pub(crate) async fn similarity_search(
        pool: &PgPool,
        query_embedding: &[f32],
        object_type_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SimilarityResult>> {
        // Fetch candidate embeddings (with snapshot metadata for the response)
        let rows: Vec<EmbeddingSearchRow> = if let Some(ot) = object_type_filter {
            sqlx::query_as(
                r#"
                SELECT e.snapshot_id, e.object_type, s.object_id,
                       e.embedding, e.version_hash,
                       s.definition->>'name' AS name,
                       md5(s.definition::text) AS current_hash
                FROM sem_reg.embedding_records e
                JOIN sem_reg.snapshots s ON s.snapshot_id = e.snapshot_id
                WHERE s.status = 'active' AND s.effective_until IS NULL
                  AND e.object_type = $1
                "#,
            )
            .bind(ot)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT e.snapshot_id, e.object_type, s.object_id,
                       e.embedding, e.version_hash,
                       s.definition->>'name' AS name,
                       md5(s.definition::text) AS current_hash
                FROM sem_reg.embedding_records e
                JOIN sem_reg.snapshots s ON s.snapshot_id = e.snapshot_id
                WHERE s.status = 'active' AND s.effective_until IS NULL
                "#,
            )
            .fetch_all(pool)
            .await?
        };

        // Compute cosine similarity in Rust
        let mut scored: Vec<SimilarityResult> = rows
            .into_iter()
            .filter_map(|row| {
                let stored: Vec<f32> = serde_json::from_value(row.embedding).ok()?;
                let score = cosine_similarity(query_embedding, &stored);
                Some(SimilarityResult {
                    snapshot_id: row.snapshot_id,
                    object_type: row.object_type,
                    object_id: row.object_id,
                    score,
                    name: row.name,
                    stale: row.version_hash != row.current_hash,
                })
            })
            .collect();

        // Sort by score descending, take top N
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(limit);
        Ok(scored)
    }
}

// ── Cosine Similarity ────────────────────────────────────────────────────────

/// Compute cosine similarity between two embedding vectors.
/// Returns 0.0 if either vector has zero magnitude.
pub(crate) fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0_f64;
    let mut mag_a = 0.0_f64;
    let mut mag_b = 0.0_f64;
    for (x, y) in a.iter().zip(b.iter()) {
        let x = *x as f64;
        let y = *y as f64;
        dot += x * y;
        mag_a += x * x;
        mag_b += y * y;
    }
    let denom = mag_a.sqrt() * mag_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let score = cosine_similarity(&a, &b);
        assert!((score - 1.0).abs() < 1e-6, "Identical vectors → 1.0");
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let score = cosine_similarity(&a, &b);
        assert!(score.abs() < 1e-6, "Orthogonal vectors → 0.0");
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let score = cosine_similarity(&a, &b);
        assert!((score + 1.0).abs() < 1e-6, "Opposite vectors → -1.0");
    }

    #[test]
    fn test_cosine_similarity_empty() {
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
    }

    #[test]
    fn test_cosine_similarity_length_mismatch() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_similarity_result_stale_field_serialization() {
        let r = SimilarityResult {
            snapshot_id: Uuid::nil(),
            object_type: "attribute_def".into(),
            object_id: Uuid::nil(),
            score: 0.95,
            name: Some("Test".into()),
            stale: true,
        };
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["stale"], true);
        assert_eq!(json["score"], 0.95);
    }
}
