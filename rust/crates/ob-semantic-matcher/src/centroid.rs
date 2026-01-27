//! Centroid computation for verb embeddings
//!
//! Centroids provide a stable "prototype" vector per verb by averaging
//! all phrase embeddings. This reduces variance from individual phrases
//! and enables efficient two-stage semantic search:
//!
//! 1. Query centroids to get candidate verbs (fast, stable)
//! 2. Refine with pattern-level matches (precise, evidenced)

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
    fn test_l2_norm_unit() {
        let v = vec![1.0, 0.0];
        assert!((l2_norm(&v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_normalize() {
        let v = normalize(vec![3.0, 4.0]);
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_zero_vector() {
        let v = normalize(vec![0.0, 0.0]);
        assert_eq!(v, vec![0.0, 0.0]);
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
        let vectors = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let c = compute_centroid(&vectors);
        // Average of [1,0] and [0,1] normalized = [0.5, 0.5] normalized = [0.707, 0.707]
        let expected = 1.0 / 2.0_f32.sqrt();
        assert!((c[0] - expected).abs() < 1e-5);
        assert!((c[1] - expected).abs() < 1e-5);
    }

    #[test]
    fn test_centroid_is_normalized() {
        let vectors = vec![
            vec![1.0, 2.0, 3.0],
            vec![4.0, 5.0, 6.0],
            vec![7.0, 8.0, 9.0],
        ];
        let c = compute_centroid(&vectors);
        let norm = l2_norm(&c);
        assert!(
            (norm - 1.0).abs() < 1e-5,
            "centroid should be unit length, got {}",
            norm
        );
    }

    #[test]
    fn test_centroid_identical_vectors() {
        let v = vec![0.6, 0.8];
        let vectors = vec![v.clone(), v.clone(), v.clone()];
        let c = compute_centroid(&vectors);
        // Centroid of identical vectors should equal that vector (normalized)
        let expected = normalize(v);
        assert!((c[0] - expected[0]).abs() < 1e-5);
        assert!((c[1] - expected[1]).abs() < 1e-5);
    }

    #[test]
    #[should_panic(expected = "centroid requires at least 1 vector")]
    fn test_centroid_empty() {
        let vectors: Vec<Vec<f32>> = vec![];
        compute_centroid(&vectors);
    }
}
