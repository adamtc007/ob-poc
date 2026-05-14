//! Mention extraction from utterances
//!
//! Extracts entity mention spans from natural language utterances using
//! n-gram scanning against the entity snapshot. This enables multi-entity
//! resolution from prompts like "Set up Goldman Sachs and Morgan Stanley".

use super::normalize::tokenize;
use super::snapshot::{EntityId, EntitySnapshot};
use smallvec::SmallVec;
use std::collections::HashMap;

/// A candidate mention span in the original utterance
#[derive(Debug, Clone)]
pub struct MentionSpan {
    /// Start character position in original text
    pub start: usize,
    /// End character position in original text (exclusive)
    pub end: usize,
    /// Original text of the mention
    pub text: String,
    /// Normalized form for matching
    pub normalized: String,
    /// Tokens in the mention
    pub tokens: Vec<String>,
    /// Candidate entity IDs from snapshot lookup
    pub candidate_ids: SmallVec<[EntityId; 8]>,
    /// Score from initial lookup (before disambiguation)
    pub score: f32,
}

/// Configuration for mention extraction
#[derive(Debug, Clone)]
pub(crate) struct MentionExtractorConfig {
    /// Maximum n-gram size to consider
    pub max_ngram: usize,
    /// Minimum score threshold to keep a span
    pub min_score: f32,
    /// Minimum token overlap ratio for fuzzy matching
    pub min_overlap: f32,
}

impl Default for MentionExtractorConfig {
    fn default() -> Self {
        Self {
            max_ngram: 5,
            min_score: 0.30,
            min_overlap: 0.34,
        }
    }
}

/// Extracts entity mention spans from utterances
#[derive(Default)]
pub struct MentionExtractor {
    config: MentionExtractorConfig,
}

impl MentionExtractor {
    /// Extract non-overlapping mention spans from utterance
    pub fn extract(&self, utterance: &str, snapshot: &EntitySnapshot) -> Vec<MentionSpan> {
        let words = self.tokenize_with_positions(utterance);
        if words.is_empty() {
            return vec![];
        }

        let mut candidates: Vec<MentionSpan> = Vec::new();

        // Generate n-gram spans
        for start_idx in 0..words.len() {
            for ngram_len in 1..=self.config.max_ngram.min(words.len() - start_idx) {
                let end_idx = start_idx + ngram_len;

                let span_words: Vec<_> = words[start_idx..end_idx].to_vec();
                let (char_start, _, _) = span_words.first().unwrap();
                let (_, char_end, _) = span_words.last().unwrap();

                let text: String = utterance[*char_start..*char_end].to_string();
                let tokens: Vec<String> = span_words.iter().map(|(_, _, t)| t.clone()).collect();
                let normalized = tokens.join(" ");

                // Score this span against snapshot
                let (candidate_ids, score) = self.score_span(&normalized, &tokens, snapshot);

                if score >= self.config.min_score && !candidate_ids.is_empty() {
                    candidates.push(MentionSpan {
                        start: *char_start,
                        end: *char_end,
                        text,
                        normalized,
                        tokens,
                        candidate_ids,
                        score,
                    });
                }
            }
        }

        // Select best non-overlapping spans (greedy by score, prefer longer)
        self.select_non_overlapping(candidates)
    }

    /// Tokenize with character positions: Vec<(start, end, token_norm)>
    fn tokenize_with_positions(&self, s: &str) -> Vec<(usize, usize, String)> {
        let mut result = Vec::new();
        let mut in_word = false;
        let mut word_start = 0;

        for (i, c) in s.char_indices() {
            if c.is_alphanumeric() {
                if !in_word {
                    word_start = i;
                    in_word = true;
                }
            } else if in_word {
                let token = s[word_start..i].to_lowercase();
                result.push((word_start, i, token));
                in_word = false;
            }
        }

        // Handle trailing word
        if in_word {
            let token = s[word_start..].to_lowercase();
            result.push((word_start, s.len(), token));
        }

        result
    }

    /// Score a span against the snapshot
    fn score_span(
        &self,
        normalized: &str,
        tokens: &[String],
        snapshot: &EntitySnapshot,
    ) -> (SmallVec<[EntityId; 8]>, f32) {
        // Fast path: exact alias match
        if let Some(ids) = snapshot.lookup_by_alias(normalized) {
            return (ids.iter().copied().collect(), 1.0);
        }

        // Fast path: exact canonical name match
        if let Some(id) = snapshot.lookup_by_name(normalized) {
            let mut ids = SmallVec::new();
            ids.push(id);
            return (ids, 1.0);
        }

        // Slower path: token overlap via token_index
        let mut entity_token_hits: HashMap<EntityId, usize> = HashMap::new();

        for token in tokens {
            if let Some(ids) = snapshot.lookup_by_token(token) {
                for id in ids.iter().take(50) {
                    *entity_token_hits.entry(*id).or_insert(0) += 1;
                }
            }
        }

        if entity_token_hits.is_empty() {
            return (SmallVec::new(), 0.0);
        }

        // Compute overlap scores and collect top candidates
        let mut scored: Vec<(EntityId, f32)> = entity_token_hits
            .into_iter()
            .filter_map(|(id, hits)| {
                let entity = snapshot.get(&id)?;
                let entity_tokens = tokenize(&entity.canonical_name_norm);
                if entity_tokens.is_empty() {
                    return None;
                }

                // Jaccard-ish overlap: intersection / min(query_len, entity_len)
                let min_len = tokens.len().min(entity_tokens.len());
                let overlap = hits as f32 / min_len as f32;

                // Filter by minimum overlap threshold
                if overlap < self.config.min_overlap {
                    return None;
                }

                Some((id, overlap))
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(8);

        let top_score = scored.first().map(|(_, s)| *s).unwrap_or(0.0);
        let ids: SmallVec<[EntityId; 8]> = scored.into_iter().map(|(id, _)| id).collect();

        (ids, top_score)
    }

    /// Greedy non-overlapping selection by score, preferring longer spans
    fn select_non_overlapping(&self, mut candidates: Vec<MentionSpan>) -> Vec<MentionSpan> {
        // Sort by score descending, then by length descending (prefer longer matches)
        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| (b.end - b.start).cmp(&(a.end - a.start)))
        });

        let mut selected: Vec<MentionSpan> = Vec::new();

        for candidate in candidates {
            let overlaps = selected
                .iter()
                .any(|s| !(candidate.end <= s.start || candidate.start >= s.end));

            if !overlaps {
                selected.push(candidate);
            }
        }

        // Sort by position for output
        selected.sort_by_key(|s| s.start);
        selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity_linking::snapshot::{EntityRow, EntitySnapshot, SNAPSHOT_VERSION};
    use smallvec::smallvec;

    fn make_test_snapshot() -> EntitySnapshot {
        let apple_id = uuid::Uuid::new_v4();
        let ford_id = uuid::Uuid::new_v4();
        let goldman_id = uuid::Uuid::new_v4();

        let mut alias_index = HashMap::new();
        alias_index.insert("apple".to_string(), smallvec![apple_id]);
        alias_index.insert("ford".to_string(), smallvec![ford_id]);
        alias_index.insert("goldman sachs".to_string(), smallvec![goldman_id]);
        alias_index.insert("goldman".to_string(), smallvec![goldman_id]);

        let mut name_index = HashMap::new();
        name_index.insert("apple inc".to_string(), apple_id);
        name_index.insert("ford motor company".to_string(), ford_id);
        name_index.insert("goldman sachs group inc".to_string(), goldman_id);

        let mut token_index: HashMap<String, SmallVec<[EntityId; 8]>> = HashMap::new();
        token_index.insert("apple".to_string(), smallvec![apple_id]);
        for token in ["ford", "motor"] {
            token_index.insert(token.to_string(), smallvec![ford_id]);
        }
        for token in ["goldman", "sachs", "group"] {
            token_index.insert(token.to_string(), smallvec![goldman_id]);
        }

        EntitySnapshot {
            version: SNAPSHOT_VERSION,
            hash: "test".to_string(),
            entities: vec![
                EntityRow {
                    entity_id: apple_id,
                    entity_kind: "company".to_string(),
                    canonical_name: "Apple Inc.".to_string(),
                    canonical_name_norm: "apple inc".to_string(),
                },
                EntityRow {
                    entity_id: ford_id,
                    entity_kind: "company".to_string(),
                    canonical_name: "Ford Motor Company".to_string(),
                    canonical_name_norm: "ford motor company".to_string(),
                },
                EntityRow {
                    entity_id: goldman_id,
                    entity_kind: "company".to_string(),
                    canonical_name: "Goldman Sachs Group Inc.".to_string(),
                    canonical_name_norm: "goldman sachs group inc".to_string(),
                },
            ],
            alias_index,
            name_index,
            token_index,
            concept_links: HashMap::new(),
            kind_index: HashMap::new(),
        }
    }

    #[test]
    fn test_single_mention() {
        let extractor = MentionExtractor::default();
        let snapshot = make_test_snapshot();

        let mentions = extractor.extract("I want to invest in Apple", &snapshot);
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0].text, "Apple");
        assert!(!mentions[0].candidate_ids.is_empty());
    }

    #[test]
    fn test_multi_mention() {
        let extractor = MentionExtractor::default();
        let snapshot = make_test_snapshot();

        let mentions = extractor.extract("Compare Apple and Ford", &snapshot);
        assert_eq!(mentions.len(), 2);
        assert!(mentions.iter().any(|m| m.text == "Apple"));
        assert!(mentions.iter().any(|m| m.text == "Ford"));
    }

    #[test]
    fn test_multi_word_mention() {
        let extractor = MentionExtractor::default();
        let snapshot = make_test_snapshot();

        let mentions = extractor.extract("Set up Goldman Sachs for trading", &snapshot);
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0].text, "Goldman Sachs");
    }

    #[test]
    fn test_no_mentions() {
        let extractor = MentionExtractor::default();
        let snapshot = make_test_snapshot();

        let mentions = extractor.extract("Hello world", &snapshot);
        assert!(mentions.is_empty());
    }

    #[test]
    fn test_position_tracking() {
        let extractor = MentionExtractor::default();
        let snapshot = make_test_snapshot();

        let mentions = extractor.extract("The Apple company", &snapshot);
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0].start, 4);
        assert_eq!(mentions[0].end, 9);
        assert_eq!(&"The Apple company"[4..9], "Apple");
    }
}
