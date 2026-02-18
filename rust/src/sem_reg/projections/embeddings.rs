//! Semantic embeddings for registry objects — text generation, staleness tracking,
//! and cosine-similarity search.
//!
//! Each registry snapshot can have an associated embedding vector.  The canonical
//! text representation is generated deterministically from the snapshot definition,
//! so staleness can be detected by comparing `version_hash` against the current
//! snapshot.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "database")]
use anyhow::Result;
#[cfg(feature = "database")]
use sqlx::PgPool;

// ── Types ────────────────────────────────────────────────────────────────────

/// The canonical text representation of a registry object, used as embedding
/// input.  Built deterministically from snapshot fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticText {
    pub snapshot_id: Uuid,
    pub object_type: String,
    /// Concatenated searchable text: FQN + name + description + aliases +
    /// taxonomy paths.
    pub text: String,
    /// SHA-256 hash of the text for staleness detection.
    pub text_hash: String,
}

/// A stored embedding record with staleness tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRecord {
    pub embedding_id: Uuid,
    pub snapshot_id: Uuid,
    pub object_type: String,
    /// The hash of the semantic text at embedding time.
    pub version_hash: String,
    /// The embedding model used (e.g., "bge-small-en-v1.5").
    pub model_id: String,
    /// Embedding dimensionality (e.g., 384).
    pub dimensions: i32,
    /// The embedding vector itself.
    pub embedding: Vec<f32>,
    pub created_at: DateTime<Utc>,
}

/// Result from a similarity search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarityResult {
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

// ── Text Generation ──────────────────────────────────────────────────────────

impl SemanticText {
    /// Generate canonical text from a snapshot definition.
    ///
    /// Concatenates: object_type, FQN (if present), name, description, aliases,
    /// and taxonomy paths into a single searchable string.
    pub fn from_definition(
        snapshot_id: Uuid,
        object_type: &str,
        definition: &serde_json::Value,
    ) -> Self {
        let mut parts: Vec<String> = Vec::new();

        // Object type as context
        parts.push(object_type.replace('_', " "));

        // FQN
        if let Some(fqn) = definition.get("fqn").and_then(|v| v.as_str()) {
            parts.push(fqn.replace('.', " "));
        }

        // Name
        if let Some(name) = definition.get("name").and_then(|v| v.as_str()) {
            parts.push(name.to_string());
        }

        // Description
        if let Some(desc) = definition.get("description").and_then(|v| v.as_str()) {
            parts.push(desc.to_string());
        }

        // Aliases
        if let Some(aliases) = definition.get("aliases").and_then(|v| v.as_array()) {
            for alias in aliases {
                if let Some(a) = alias.as_str() {
                    parts.push(a.to_string());
                }
            }
        }

        // Taxonomy paths
        if let Some(taxonomies) = definition
            .get("taxonomy_memberships")
            .and_then(|v| v.as_array())
        {
            for tax in taxonomies {
                if let Some(path) = tax.get("path").and_then(|v| v.as_str()) {
                    parts.push(path.replace('/', " "));
                }
            }
        }

        let text = parts.join(" ");
        let text_hash = format!("{:x}", md5_hash(&text));

        Self {
            snapshot_id,
            object_type: object_type.to_string(),
            text,
            text_hash,
        }
    }
}

/// Simple hash for staleness detection (not cryptographic).
fn md5_hash(input: &str) -> u128 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    input.hash(&mut hasher);
    let h1 = hasher.finish() as u128;
    // Second round for wider distribution
    format!("{}{}", input, h1).hash(&mut hasher);
    let h2 = hasher.finish() as u128;
    h1 ^ (h2 << 64)
}

// ── Store ────────────────────────────────────────────────────────────────────

pub struct EmbeddingStore;

impl EmbeddingStore {
    /// Insert or update an embedding record (versioned by snapshot_id).
    #[cfg(feature = "database")]
    pub async fn upsert_embedding(
        pool: &PgPool,
        snapshot_id: Uuid,
        object_type: &str,
        version_hash: &str,
        model_id: &str,
        dimensions: i32,
        embedding: &[f32],
    ) -> Result<Uuid> {
        let embedding_id = Uuid::new_v4();
        let embedding_json = serde_json::to_value(embedding)?;
        sqlx::query(
            r#"
            INSERT INTO sem_reg.embedding_records
                (embedding_id, snapshot_id, object_type, version_hash,
                 model_id, dimensions, embedding)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (snapshot_id) DO UPDATE SET
                version_hash = EXCLUDED.version_hash,
                model_id = EXCLUDED.model_id,
                dimensions = EXCLUDED.dimensions,
                embedding = EXCLUDED.embedding,
                updated_at = NOW()
            "#,
        )
        .bind(embedding_id)
        .bind(snapshot_id)
        .bind(object_type)
        .bind(version_hash)
        .bind(model_id)
        .bind(dimensions)
        .bind(embedding_json)
        .execute(pool)
        .await?;
        Ok(embedding_id)
    }

    /// Check if an embedding is stale (version_hash differs from current snapshot).
    #[cfg(feature = "database")]
    pub async fn check_staleness(
        pool: &PgPool,
        snapshot_id: Uuid,
        current_hash: &str,
    ) -> Result<bool> {
        let row: Option<(String,)> = sqlx::query_as(
            r#"
            SELECT version_hash
            FROM sem_reg.embedding_records
            WHERE snapshot_id = $1
            "#,
        )
        .bind(snapshot_id)
        .fetch_optional(pool)
        .await?;

        match row {
            None => Ok(true), // No embedding at all → stale
            Some((stored_hash,)) => Ok(stored_hash != current_hash),
        }
    }

    /// Search for similar embeddings using application-side cosine similarity.
    ///
    /// Since sem_reg embeddings are stored as JSONB (not pgvector), we fetch
    /// candidate embeddings and compute cosine similarity in Rust.  Results
    /// are returned sorted by descending similarity score.
    #[cfg(feature = "database")]
    pub async fn similarity_search(
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

    /// Return snapshot IDs of stale embeddings (version_hash mismatch).
    #[cfg(feature = "database")]
    pub async fn stale_snapshot_ids(pool: &PgPool) -> Result<Vec<Uuid>> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT e.snapshot_id
            FROM sem_reg.embedding_records e
            JOIN sem_reg.snapshots s ON s.snapshot_id = e.snapshot_id
            WHERE e.version_hash IS DISTINCT FROM md5(s.definition::text)
            "#,
        )
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// Count total embeddings and stale embeddings for stats.
    #[cfg(feature = "database")]
    pub async fn stats(pool: &PgPool) -> Result<(i64, i64)> {
        let row: (i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) AS total,
                COUNT(*) FILTER (
                    WHERE e.version_hash IS DISTINCT FROM
                        md5(s.definition::text)
                ) AS stale
            FROM sem_reg.embedding_records e
            JOIN sem_reg.snapshots s ON s.snapshot_id = e.snapshot_id
            "#,
        )
        .fetch_one(pool)
        .await?;
        Ok(row)
    }
}

// ── Cosine Similarity ────────────────────────────────────────────────────────

/// Compute cosine similarity between two embedding vectors.
/// Returns 0.0 if either vector has zero magnitude.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
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
    fn test_semantic_text_from_definition() {
        let def = serde_json::json!({
            "fqn": "kyc.beneficial_owner_pct",
            "name": "Beneficial Owner Percentage",
            "description": "Percentage ownership by a beneficial owner",
            "aliases": ["bo_pct", "ubo_percentage"],
            "taxonomy_memberships": [
                {"path": "kyc/ownership/beneficial"}
            ]
        });
        let st = SemanticText::from_definition(Uuid::nil(), "attribute_def", &def);
        assert!(st.text.contains("Beneficial Owner Percentage"));
        assert!(st.text.contains("bo_pct"));
        assert!(st.text.contains("kyc ownership beneficial"));
        assert!(!st.text_hash.is_empty());
    }

    #[test]
    fn test_semantic_text_minimal_definition() {
        let def = serde_json::json!({"name": "Simple"});
        let st = SemanticText::from_definition(Uuid::nil(), "verb_def", &def);
        assert!(st.text.contains("Simple"));
        assert!(st.text.contains("verb def"));
    }

    #[test]
    fn test_embedding_record_roundtrip() {
        let rec = EmbeddingRecord {
            embedding_id: Uuid::nil(),
            snapshot_id: Uuid::nil(),
            object_type: "attribute_def".into(),
            version_hash: "abc123".into(),
            model_id: "bge-small-en-v1.5".into(),
            dimensions: 384,
            embedding: vec![0.1, 0.2, 0.3],
            created_at: Utc::now(),
        };
        let json = serde_json::to_value(&rec).unwrap();
        assert_eq!(json["dimensions"], 384);
        assert_eq!(json["model_id"], "bge-small-en-v1.5");
    }

    #[test]
    fn test_staleness_detection_hash_stability() {
        let def = serde_json::json!({"name": "Test"});
        let st1 = SemanticText::from_definition(Uuid::nil(), "verb_def", &def);
        let st2 = SemanticText::from_definition(Uuid::nil(), "verb_def", &def);
        assert_eq!(
            st1.text_hash, st2.text_hash,
            "Same input should produce same hash"
        );

        let def2 = serde_json::json!({"name": "Different"});
        let st3 = SemanticText::from_definition(Uuid::nil(), "verb_def", &def2);
        assert_ne!(
            st1.text_hash, st3.text_hash,
            "Different input should produce different hash"
        );
    }

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
