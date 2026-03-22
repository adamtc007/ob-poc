//! Embedding-based pre-screening for generated utterances.

use anyhow::Result;
use std::collections::HashMap;

use crate::agent::learning::embedder::{CandleEmbedder, Embedder};

use super::types::{EmbeddingPreScreen, GeneratedUtterance, PreScreenStratum};

/// Pre-screen utterances using the live Candle/BGE embedder.
///
/// # Examples
/// ```rust,no_run
/// use ob_poc::calibration::{pre_screen_utterances, CalibrationScenario, CalibrationExecutionShape};
///
/// # async fn demo(embedder: ob_poc::agent::learning::embedder::CandleEmbedder, scenario: CalibrationScenario) -> anyhow::Result<()> {
/// let results = pre_screen_utterances(&[], &scenario, &embedder).await?;
/// assert!(results.is_empty());
/// # Ok(())
/// # }
/// ```
pub async fn pre_screen_utterances(
    utterances: &[GeneratedUtterance],
    scenario: &super::types::CalibrationScenario,
    embedder: &CandleEmbedder,
) -> Result<Vec<EmbeddingPreScreen>> {
    let target_embedding = embedder.embed_target(&scenario.target_verb).await?;
    let mut neighbour_embeddings = HashMap::<String, Vec<f32>>::new();
    for neighbour in &scenario.near_neighbour_verbs {
        let embedding = embedder.embed_target(&neighbour.verb_id).await?;
        neighbour_embeddings.insert(neighbour.verb_id.clone(), embedding);
    }

    let mut results = Vec::with_capacity(utterances.len());
    for utterance in utterances {
        let utterance_embedding = embedder.embed_query(&utterance.text).await?;
        let target_distance = cosine_distance(&utterance_embedding, &target_embedding);

        let mut nearest_neighbour_verb = String::new();
        let mut nearest_neighbour_distance = f32::MAX;
        for (verb_id, embedding) in &neighbour_embeddings {
            let distance = cosine_distance(&utterance_embedding, embedding);
            if distance < nearest_neighbour_distance {
                nearest_neighbour_distance = distance;
                nearest_neighbour_verb = verb_id.clone();
            }
        }

        if nearest_neighbour_verb.is_empty() {
            nearest_neighbour_verb = scenario.target_verb.clone();
            nearest_neighbour_distance = target_distance;
        }

        let margin = nearest_neighbour_distance - target_distance;
        let stratum = if target_distance < 0.15 && margin > 0.10 {
            PreScreenStratum::ClearMatch {
                distance: target_distance,
            }
        } else if nearest_neighbour_distance < target_distance {
            PreScreenStratum::NeighbourPreferred {
                preferred_verb: nearest_neighbour_verb.clone(),
                preferred_distance: nearest_neighbour_distance,
            }
        } else if margin.abs() < 0.08 {
            PreScreenStratum::BoundaryCase { margin }
        } else if target_distance > 0.40 {
            PreScreenStratum::ClearNonMatch {
                distance: target_distance,
            }
        } else {
            PreScreenStratum::BoundaryCase { margin }
        };

        results.push(EmbeddingPreScreen {
            utterance: utterance.text.clone(),
            target_verb_distance: target_distance,
            nearest_neighbour_distance,
            nearest_neighbour_verb,
            margin,
            stratum,
        });
    }

    Ok(results)
}

fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    1.0 - (dot / (norm_a * norm_b))
}
