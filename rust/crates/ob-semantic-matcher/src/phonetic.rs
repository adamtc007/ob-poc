//! Phonetic matching using Double Metaphone
//!
//! Handles misheard words like "enhawnce" → "enhance" by comparing
//! phonetic representations rather than exact spelling.

use rphonetic::DoubleMetaphone;

/// Phonetic matcher using Double Metaphone algorithm
///
/// Double Metaphone is particularly good for English words and handles
/// many edge cases better than original Metaphone or Soundex.
pub struct PhoneticMatcher {
    encoder: DoubleMetaphone,
}

impl Default for PhoneticMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PhoneticMatcher {
    /// Create a new phonetic matcher
    pub fn new() -> Self {
        Self {
            encoder: DoubleMetaphone::default(),
        }
    }

    /// Encode a word to its phonetic representation
    ///
    /// Returns both primary and alternate codes for better matching.
    pub fn encode(&self, word: &str) -> Vec<String> {
        let word = word.trim().to_lowercase();
        if word.is_empty() {
            return vec![];
        }

        let result = self.encoder.double_metaphone(&word);
        let primary = result.primary();
        let alternate = result.alternate();

        let mut codes = vec![primary.to_string()];
        if !alternate.is_empty() && alternate != primary {
            codes.push(alternate.to_string());
        }

        codes
    }

    /// Encode a phrase (multiple words) to phonetic codes
    ///
    /// Returns codes for each word in the phrase.
    pub fn encode_phrase(&self, phrase: &str) -> Vec<String> {
        phrase
            .split_whitespace()
            .flat_map(|word| self.encode(word))
            .collect()
    }

    /// Check if two words match phonetically
    pub fn matches(&self, word1: &str, word2: &str) -> bool {
        let codes1 = self.encode(word1);
        let codes2 = self.encode(word2);

        // Any overlap in codes means a phonetic match
        codes1.iter().any(|c1| codes2.contains(c1))
    }

    /// Calculate phonetic similarity between two phrases
    ///
    /// Returns a score between 0.0 and 1.0 based on overlapping phonetic codes.
    pub fn phrase_similarity(&self, phrase1: &str, phrase2: &str) -> f32 {
        let codes1 = self.encode_phrase(phrase1);
        let codes2 = self.encode_phrase(phrase2);

        if codes1.is_empty() || codes2.is_empty() {
            return 0.0;
        }

        let matches = codes1.iter().filter(|c| codes2.contains(c)).count();
        let max_possible = codes1.len().max(codes2.len());

        matches as f32 / max_possible as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_word() {
        let matcher = PhoneticMatcher::new();

        // Basic encoding
        let codes = matcher.encode("enhance");
        assert!(!codes.is_empty());

        // Misspelling should match
        let codes_misspelled = matcher.encode("enhawnce");
        assert!(
            codes.iter().any(|c| codes_misspelled.contains(c)),
            "enhance and enhawnce should have overlapping codes"
        );
    }

    #[test]
    fn test_matches() {
        let matcher = PhoneticMatcher::new();

        // Should match phonetically similar words
        assert!(matcher.matches("enhance", "enhawnce"));
        assert!(matcher.matches("track", "trak"));
        assert!(matcher.matches("zoom", "zuum"));

        // Should not match phonetically different words
        assert!(!matcher.matches("zoom", "enhance"));
    }

    #[test]
    fn test_phrase_similarity() {
        let matcher = PhoneticMatcher::new();

        // High similarity
        let sim1 = matcher.phrase_similarity("zoom in", "zuum in");
        assert!(sim1 > 0.5, "Expected high similarity, got {}", sim1);

        // Lower similarity
        let sim2 = matcher.phrase_similarity("zoom in", "pan left");
        assert!(
            sim2 < sim1,
            "zoom/pan should be less similar than zoom/zuum"
        );
    }

    #[test]
    fn test_voice_misrecognition_cases() {
        let matcher = PhoneticMatcher::new();

        // Common voice recognition errors
        assert!(matcher.matches("rabbit", "rabit"));
        assert!(matcher.matches("white", "wite"));
        assert!(matcher.matches("follow", "fallow"));
        assert!(matcher.matches("ownership", "ownershp"));
    }
}
