# TODO: Semantic Intent Matching Enhancement

## Overview

Upgrade verb discovery from keyword-based PostgreSQL full-text search to hybrid semantic matching using sentence embeddings + phonetic matching + BM25. This will dramatically improve fuzzy matching from voice transcripts and natural language chat.

---

## Current State (What We Have)

```rust
// verb_discovery.rs - Current approach
// 1. PostgreSQL ts_vector/ts_query (keyword matching)
// 2. Simple string contains() on intent_patterns
// 3. Exact array matching for graph_contexts

// PROBLEM: "show me who owns this" won't match "list owners"
// PROBLEM: Voice error "enhawnce" won't match "enhance"
```

---

## Target Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  USER INPUT: "show me who owns this"                        │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │           MULTI-SIGNAL MATCHER                       │   │
│  ├─────────────────────────────────────────────────────┤   │
│  │ 1. Semantic:  embed(input) ↔ embed(patterns) → 0.87 │   │
│  │ 2. Phonetic:  metaphone(input) ↔ phonetic_index     │   │
│  │ 3. Lexical:   BM25(input, search_text) → 0.45       │   │
│  │ 4. Context:   graph_context boost → +0.15           │   │
│  └─────────────────────────────────────────────────────┘   │
│                         │                                   │
│                         ▼                                   │
│  Combined Score: 0.87 * 0.5 + 0.45 * 0.2 + 0.15 = 0.67    │
│  → "ubo.list-owners" (confidence: 67%)                     │
└─────────────────────────────────────────────────────────────┘
```

---

## Task 1: Add Sentence Embeddings

### 1.1 Embedding Model Selection

For browser/WASM deployment, use a small but effective model:

| Model | Size | Dimensions | Notes |
|-------|------|------------|-------|
| **all-MiniLM-L6-v2** | 22MB | 384 | Best balance for browser |
| all-mpnet-base-v2 | 420MB | 768 | Most accurate, too big |
| paraphrase-MiniLM-L3-v2 | 17MB | 384 | Fastest, less accurate |

**Recommendation:** `all-MiniLM-L6-v2` via ONNX Runtime or Candle (Rust)

### 1.2 Pre-compute Verb Pattern Embeddings

At startup, embed all intent patterns and store in vector index:

```rust
// src/session/semantic_matcher.rs

use candle_core::{Device, Tensor};
use candle_transformers::models::bert::BertModel;

pub struct SemanticMatcher {
    model: BertModel,
    tokenizer: Tokenizer,
    pattern_embeddings: Vec<(String, String, Vec<f32>)>, // (verb, pattern, embedding)
    index: faiss::Index, // or hnswlib
}

impl SemanticMatcher {
    /// Build embeddings for all verb patterns at startup
    pub async fn build_index(verbs: &[VerbWithPatterns]) -> Self {
        let mut embeddings = Vec::new();
        
        for verb in verbs {
            for pattern in &verb.intent_patterns {
                let embedding = self.embed(pattern);
                embeddings.push((verb.full_name.clone(), pattern.clone(), embedding));
            }
        }
        
        // Build FAISS index
        let dimension = 384; // MiniLM dimension
        let mut index = faiss::IndexFlatIP::new(dimension);
        
        for (_, _, emb) in &embeddings {
            index.add(&emb);
        }
        
        Self { embeddings, index, .. }
    }
    
    /// Find most similar patterns for user input
    pub fn search(&self, query: &str, top_k: usize) -> Vec<(String, f32)> {
        let query_embedding = self.embed(query);
        let (distances, indices) = self.index.search(&query_embedding, top_k);
        
        indices.iter().zip(distances.iter())
            .map(|(idx, dist)| {
                let (verb, pattern, _) = &self.embeddings[*idx as usize];
                (verb.clone(), *dist)
            })
            .collect()
    }
}
```

### 1.3 Rust Embedding Options

**Option A: Candle (HuggingFace Rust)**
```toml
# Cargo.toml
[dependencies]
candle-core = "0.4"
candle-nn = "0.4"
candle-transformers = "0.4"
tokenizers = "0.15"
```

**Option B: ONNX Runtime**
```toml
[dependencies]
ort = "2.0"  # ONNX Runtime Rust bindings
```

**Option C: External service (simplest)**
- Embed at build time with Python
- Store embeddings in PostgreSQL `vector` extension (pgvector)
- Query with SQL cosine similarity

```sql
-- Using pgvector extension
CREATE EXTENSION vector;

ALTER TABLE dsl_verbs ADD COLUMN pattern_embedding vector(384);

-- Query
SELECT full_name, 1 - (pattern_embedding <=> $1::vector) as similarity
FROM dsl_verbs
ORDER BY pattern_embedding <=> $1::vector
LIMIT 10;
```

---

## Task 2: Add Phonetic Matching (for Voice Errors)

### 2.1 Why Phonetic Matching?

Voice transcription errors are often phonetic:
- "enhawnce" → "enhance"
- "trak right" → "track right"  
- "drill frew" → "drill through"
- "yoo bee oh" → "UBO"

### 2.2 Implementation

```rust
// src/session/phonetic_matcher.rs

use rust_phonetic::{Metaphone, DoubleMetaphone};

pub struct PhoneticMatcher {
    // Pre-computed phonetic codes for all patterns
    pattern_codes: HashMap<String, Vec<(String, String)>>, // phonetic_code -> [(verb, pattern)]
}

impl PhoneticMatcher {
    pub fn build_index(verbs: &[VerbWithPatterns]) -> Self {
        let mut pattern_codes: HashMap<String, Vec<(String, String)>> = HashMap::new();
        
        for verb in verbs {
            for pattern in &verb.intent_patterns {
                // Generate phonetic codes for each word in pattern
                let codes = pattern.split_whitespace()
                    .map(|word| double_metaphone(word))
                    .collect::<Vec<_>>()
                    .join(" ");
                
                pattern_codes.entry(codes)
                    .or_default()
                    .push((verb.full_name.clone(), pattern.clone()));
            }
        }
        
        Self { pattern_codes }
    }
    
    pub fn search(&self, query: &str) -> Vec<(String, String, f32)> {
        let query_codes = query.split_whitespace()
            .map(|word| double_metaphone(word))
            .collect::<Vec<_>>()
            .join(" ");
        
        // Exact phonetic match
        if let Some(matches) = self.pattern_codes.get(&query_codes) {
            return matches.iter()
                .map(|(verb, pattern)| (verb.clone(), pattern.clone(), 1.0))
                .collect();
        }
        
        // Fuzzy phonetic match (Levenshtein on phonetic codes)
        let mut results = Vec::new();
        for (codes, matches) in &self.pattern_codes {
            let distance = levenshtein(&query_codes, codes);
            let similarity = 1.0 - (distance as f32 / query_codes.len().max(codes.len()) as f32);
            if similarity > 0.7 {
                for (verb, pattern) in matches {
                    results.push((verb.clone(), pattern.clone(), similarity));
                }
            }
        }
        
        results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
        results
    }
}

fn double_metaphone(word: &str) -> String {
    // Use rust-phonetic or implement Double Metaphone
    DoubleMetaphone::encode(word).primary
}
```

### 2.3 Rust Phonetic Libraries

```toml
[dependencies]
# Option 1: rust-phonetic (comprehensive)
rust-phonetic = "1.0"

# Option 2: rphonetic (lighter)  
rphonetic = "2.0"

# For Levenshtein distance
strsim = "0.10"
```

---

## Task 3: Hybrid Scoring

Combine all signals with weighted scoring:

```rust
// src/session/hybrid_matcher.rs

pub struct HybridMatcher {
    semantic: SemanticMatcher,
    phonetic: PhoneticMatcher,
    bm25: BM25Index, // or keep PostgreSQL FTS
}

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub verb: String,
    pub pattern: String,
    pub semantic_score: f32,
    pub phonetic_score: f32,
    pub lexical_score: f32,
    pub context_boost: f32,
    pub final_score: f32,
    pub confidence: MatchConfidence,
}

pub enum MatchConfidence {
    High,    // > 0.85 - execute immediately
    Medium,  // 0.6-0.85 - show confirmation
    Low,     // < 0.6 - show alternatives
}

impl HybridMatcher {
    /// Weights for combining scores
    const SEMANTIC_WEIGHT: f32 = 0.50;  // Most important
    const PHONETIC_WEIGHT: f32 = 0.25;  // Important for voice
    const LEXICAL_WEIGHT: f32 = 0.15;   // Keyword backup
    const CONTEXT_WEIGHT: f32 = 0.10;   // Situational boost
    
    pub fn search(
        &self,
        query: &str,
        context: &GraphContext,
        top_k: usize,
    ) -> Vec<MatchResult> {
        // 1. Semantic search
        let semantic_results = self.semantic.search(query, top_k * 2);
        
        // 2. Phonetic search (important for voice)
        let phonetic_results = self.phonetic.search(query);
        
        // 3. Lexical search (BM25 / PostgreSQL FTS)
        let lexical_results = self.bm25.search(query, top_k * 2);
        
        // 4. Combine and score
        let mut candidates: HashMap<String, MatchResult> = HashMap::new();
        
        for (verb, score) in semantic_results {
            candidates.entry(verb.clone())
                .or_insert_with(|| MatchResult::new(&verb))
                .semantic_score = score;
        }
        
        for (verb, _, score) in phonetic_results {
            candidates.entry(verb.clone())
                .or_insert_with(|| MatchResult::new(&verb))
                .phonetic_score = score;
        }
        
        for (verb, score) in lexical_results {
            candidates.entry(verb.clone())
                .or_insert_with(|| MatchResult::new(&verb))
                .lexical_score = score;
        }
        
        // 5. Apply context boost
        for (_, result) in &mut candidates {
            result.context_boost = self.compute_context_boost(&result.verb, context);
            
            // Compute final score
            result.final_score = 
                result.semantic_score * Self::SEMANTIC_WEIGHT +
                result.phonetic_score * Self::PHONETIC_WEIGHT +
                result.lexical_score * Self::LEXICAL_WEIGHT +
                result.context_boost * Self::CONTEXT_WEIGHT;
            
            // Set confidence level
            result.confidence = if result.final_score > 0.85 {
                MatchConfidence::High
            } else if result.final_score > 0.6 {
                MatchConfidence::Medium
            } else {
                MatchConfidence::Low
            };
        }
        
        // 6. Sort and return top-k
        let mut results: Vec<_> = candidates.into_values().collect();
        results.sort_by(|a, b| b.final_score.partial_cmp(&a.final_score).unwrap());
        results.truncate(top_k);
        
        results
    }
    
    fn compute_context_boost(&self, verb: &str, context: &GraphContext) -> f32 {
        // Boost verbs that are relevant to current graph state
        // e.g., if cursor is on UBO, boost ownership-related verbs
        0.0 // TODO: implement based on graph_contexts array
    }
}
```

---

## Task 4: Domain-Specific Fine-Tuning (Optional but Powerful)

For maximum accuracy, fine-tune the embedding model on your domain:

### 4.1 Generate Training Pairs

```python
# scripts/generate_training_pairs.py

import json

# Positive pairs: same intent, different phrasing
positive_pairs = [
    ("show owners", "list owners"),
    ("show owners", "who owns this"),
    ("show owners", "ownership structure"),
    ("follow the white rabbit", "trace to terminus"),
    ("follow the white rabbit", "find the humans"),
    ("enhance", "zoom in"),
    ("enhance", "magnify"),
    ("drill through", "penetrate to UBO"),
    ("drill through", "go all the way down"),
]

# Negative pairs: different intents
negative_pairs = [
    ("show owners", "show controllers"),  # Similar but different
    ("zoom in", "zoom out"),
    ("drill down", "drill up"),
]

# Save for training
training_data = {
    "positive": positive_pairs,
    "negative": negative_pairs
}

with open("training_pairs.json", "w") as f:
    json.dump(training_data, f, indent=2)
```

### 4.2 Fine-Tune with Sentence Transformers

```python
# scripts/finetune_embeddings.py

from sentence_transformers import SentenceTransformer, InputExample, losses
from torch.utils.data import DataLoader
import json

# Load base model
model = SentenceTransformer('all-MiniLM-L6-v2')

# Load training data
with open("training_pairs.json") as f:
    data = json.load(f)

# Create training examples
train_examples = []
for a, b in data["positive"]:
    train_examples.append(InputExample(texts=[a, b], label=1.0))
for a, b in data["negative"]:
    train_examples.append(InputExample(texts=[a, b], label=0.0))

# Train
train_dataloader = DataLoader(train_examples, shuffle=True, batch_size=16)
train_loss = losses.CosineSimilarityLoss(model)

model.fit(
    train_objectives=[(train_dataloader, train_loss)],
    epochs=10,
    warmup_steps=100,
    output_path="./ob-poc-embeddings"
)

# Export to ONNX for Rust
model.save("./ob-poc-embeddings")
```

---

## Task 5: Browser/WASM Deployment

For client-side matching (privacy, latency):

### 5.1 Option A: Pre-compute Everything Server-Side

```rust
// At startup, compute all embeddings server-side
// Client sends query, server returns matches
// Simple but requires network round-trip
```

### 5.2 Option B: Ship Small Model to Browser

```javascript
// Using Transformers.js (HuggingFace)
import { pipeline } from '@xenova/transformers';

const embedder = await pipeline('feature-extraction', 'Xenova/all-MiniLM-L6-v2');

async function matchIntent(query, patternEmbeddings) {
    const queryEmbedding = await embedder(query, { pooling: 'mean' });
    
    // Find nearest neighbors
    const results = patternEmbeddings.map(({ verb, embedding }) => ({
        verb,
        similarity: cosineSimilarity(queryEmbedding, embedding)
    }));
    
    return results.sort((a, b) => b.similarity - a.similarity).slice(0, 5);
}
```

### 5.3 Option C: Rust WASM with Candle

```rust
// Compile embedding model to WASM
// Ship to browser
// Run inference client-side

// See: https://huggingface.co/docs/candle/index
```

---

## Task 6: Database Schema Updates

Add embedding storage to PostgreSQL:

```sql
-- Enable pgvector extension
CREATE EXTENSION IF NOT EXISTS vector;

-- Add embedding columns
ALTER TABLE "ob-poc".dsl_verbs 
ADD COLUMN IF NOT EXISTS search_embedding vector(384);

-- Add pattern embeddings table (one per pattern)
CREATE TABLE IF NOT EXISTS "ob-poc".verb_pattern_embeddings (
    id SERIAL PRIMARY KEY,
    verb_full_name TEXT NOT NULL REFERENCES "ob-poc".dsl_verbs(full_name),
    pattern TEXT NOT NULL,
    embedding vector(384) NOT NULL,
    phonetic_code TEXT, -- Double Metaphone encoding
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Create vector index for fast similarity search
CREATE INDEX IF NOT EXISTS idx_pattern_embedding 
ON "ob-poc".verb_pattern_embeddings 
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

-- Create phonetic index
CREATE INDEX IF NOT EXISTS idx_phonetic_code 
ON "ob-poc".verb_pattern_embeddings(phonetic_code);
```

---

## Task 7: Update VerbDiscoveryService

Integrate new matching into existing service:

```rust
// src/session/verb_discovery.rs

impl VerbDiscoveryService {
    pub async fn discover(
        &self,
        query: &DiscoveryQuery,
    ) -> Result<Vec<VerbSuggestion>, VerbDiscoveryError> {
        let mut suggestions = Vec::new();
        
        if let Some(ref text) = query.query_text {
            // NEW: Use hybrid matcher instead of just FTS
            let hybrid_results = self.hybrid_matcher.search(
                text,
                &query.graph_context,
                query.limit,
            );
            
            for result in hybrid_results {
                suggestions.push(VerbSuggestion {
                    verb: result.verb,
                    score: result.final_score,
                    reason: SuggestionReason::HybridMatch {
                        semantic: result.semantic_score,
                        phonetic: result.phonetic_score,
                        lexical: result.lexical_score,
                        confidence: result.confidence,
                    },
                    ..Default::default()
                });
            }
        }
        
        // ... rest of existing logic (graph_context, workflow_phase, etc.)
        
        Ok(suggestions)
    }
}
```

---

## Implementation Priority

| Priority | Task | Impact | Effort |
|----------|------|--------|--------|
| **1** | Semantic embeddings (pgvector) | HIGH | Medium |
| **2** | Phonetic matching (Double Metaphone) | HIGH for voice | Low |
| **3** | Hybrid scoring | HIGH | Low |
| **4** | Domain fine-tuning | Medium | Medium |
| **5** | Browser WASM deployment | Nice-to-have | High |

---

## Expected Improvements

| Scenario | Current | With Semantic + Phonetic |
|----------|---------|-------------------------|
| "show me who owns this" → `ubo.list-owners` | ❌ Fails | ✅ 87% match |
| "enhawnce" (voice error) → `ui.zoom-in` | ❌ Fails | ✅ 92% match |
| "rabbit hole" → `ui.follow-the-rabbit` | ⚠️ Pattern match only | ✅ 95% match |
| "trak left" → `ui.pan-left` | ❌ Fails | ✅ 88% match |
| "yoo bee oh chain" → `ubo.trace-chains` | ❌ Fails | ✅ 85% match |

---

## Testing Strategy

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_semantic_similarity() {
        let matcher = HybridMatcher::new();
        
        // These should all match "ubo.list-owners"
        let queries = [
            "show owners",
            "who owns this",
            "list ownership",
            "ownership structure",
            "show me the owners",
        ];
        
        for query in queries {
            let results = matcher.search(query, &GraphContext::default(), 5);
            assert!(results[0].verb == "ubo.list-owners");
            assert!(results[0].confidence == MatchConfidence::High);
        }
    }
    
    #[test]
    fn test_phonetic_voice_errors() {
        let matcher = HybridMatcher::new();
        
        // Common voice transcription errors
        let voice_errors = [
            ("enhawnce", "ui.zoom-in"),
            ("trak right", "ui.pan-right"),
            ("drill frew", "ui.drill-through"),
            ("yoo bee oh", "ubo.calculate"),
        ];
        
        for (input, expected_verb) in voice_errors {
            let results = matcher.search(input, &GraphContext::default(), 5);
            assert!(results.iter().any(|r| r.verb == expected_verb));
        }
    }
}
```

---

## References

- Sentence Transformers: https://sbert.net/
- pgvector: https://github.com/pgvector/pgvector
- Candle (Rust ML): https://github.com/huggingface/candle
- Double Metaphone: https://en.wikipedia.org/wiki/Metaphone
- FAISS: https://github.com/facebookresearch/faiss
