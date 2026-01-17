# TODO: Learning Enhancements + pgvector Integration

**Priority**: MEDIUM (implement after foundation)  
**Estimated Effort**: 3-4 days  
**Created**: 2025-01-17  
**Status**: NOT STARTED  
**Depends On**: 
- TODO-MCP-SEMANTIC-INTENT-PIPELINE.md ✓
- TODO-MCP-INTENT-FEEDBACK-TOOL.md ✓

## Overview

This TODO extends the learning system with advanced capabilities and integrates pgvector embeddings into the learning layer for semantic generalization.

**Current state** (after foundation TODOs):
- Learned phrases are **exact match only**
- User teaches "spin up a fund" → system only recognizes "spin up a fund"
- No blocklist capability
- No user-specific learning
- No admin tools for learning management

**Target state**:
- Learned phrases have **semantic fuzzy matching** via pgvector
- User teaches "spin up a fund" → system recognizes "create a new fund", "spin up fund", etc.
- Negative feedback blocks semantic neighborhoods
- Per-user vocabulary preferences
- Admin MCP tools for learning management
- Bulk import for deployment bootstrapping

---

## Architecture: pgvector in Learning Layer

### Current Search Priority
```
1. LearnedData.resolve_phrase()     → EXACT match
2. VerbPhraseIndex.find_matches()   → Substring match  
3. SemanticMatcher (pgvector)       → Global verb patterns
```

### Enhanced Search Priority
```
1. LearnedData.resolve_phrase()           → EXACT match (in-memory)
2. LearnedSemanticSearch (pgvector)       → Learned phrase embeddings ← NEW
3. Blocklist check (pgvector)             → Block semantic neighborhood ← NEW
4. VerbPhraseIndex.find_matches()         → Substring match
5. SemanticMatcher (pgvector)             → Global verb patterns (cold start)
```

**Key insight**: Learned phrases become **semantic anchors**, not just string literals.

---

## Phase 1: pgvector Schema Extensions

### 1.1 Add Embedding Columns

**File**: `migrations/YYYYMMDD_learning_embeddings.sql`

```sql
-- Ensure pgvector extension
CREATE EXTENSION IF NOT EXISTS vector;

-- Add embedding to learned invocation phrases
ALTER TABLE "ob-poc".agent_learned_invocation_phrases
ADD COLUMN IF NOT EXISTS embedding vector(1536),
ADD COLUMN IF NOT EXISTS embedding_model TEXT DEFAULT 'text-embedding-ada-002';

-- Add embedding to learned entity aliases  
ALTER TABLE "ob-poc".agent_learned_entity_aliases
ADD COLUMN IF NOT EXISTS embedding vector(1536),
ADD COLUMN IF NOT EXISTS embedding_model TEXT DEFAULT 'text-embedding-ada-002';

-- Create phrase blocklist table
CREATE TABLE IF NOT EXISTS "ob-poc".agent_phrase_blocklist (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    phrase TEXT NOT NULL,
    blocked_verb TEXT NOT NULL,
    embedding vector(1536),
    embedding_model TEXT DEFAULT 'text-embedding-ada-002',
    reason TEXT,
    source TEXT DEFAULT 'explicit_feedback',
    user_id UUID,  -- NULL = global, set = user-specific
    created_at TIMESTAMPTZ DEFAULT now(),
    expires_at TIMESTAMPTZ,  -- Optional expiry
    
    UNIQUE(phrase, blocked_verb, user_id)
);

-- User-specific learned phrases
CREATE TABLE IF NOT EXISTS "ob-poc".agent_user_learned_phrases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL,
    phrase TEXT NOT NULL,
    verb TEXT NOT NULL,
    embedding vector(1536),
    embedding_model TEXT DEFAULT 'text-embedding-ada-002',
    occurrence_count INT DEFAULT 1,
    confidence REAL DEFAULT 1.0,
    source TEXT DEFAULT 'explicit_feedback',
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ,
    
    UNIQUE(user_id, phrase)
);

-- IVFFlat indexes for similarity search
CREATE INDEX IF NOT EXISTS idx_learned_phrases_embedding 
ON "ob-poc".agent_learned_invocation_phrases 
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

CREATE INDEX IF NOT EXISTS idx_blocklist_embedding
ON "ob-poc".agent_phrase_blocklist
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 50);

CREATE INDEX IF NOT EXISTS idx_user_phrases_embedding
ON "ob-poc".agent_user_learned_phrases
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

-- Standard indexes
CREATE INDEX IF NOT EXISTS idx_user_phrases_user 
ON "ob-poc".agent_user_learned_phrases(user_id);

CREATE INDEX IF NOT EXISTS idx_blocklist_verb
ON "ob-poc".agent_phrase_blocklist(blocked_verb);
```

### 1.2 Backfill Existing Data

**File**: `rust/src/agent/learning/migrations.rs` (or run as one-time script)

```rust
/// Backfill embeddings for existing learned phrases
pub async fn backfill_learned_embeddings(
    pool: &PgPool,
    embedder: &dyn Embedder,
) -> Result<BackfillStats> {
    let mut stats = BackfillStats::default();
    
    // Fetch phrases without embeddings
    let phrases: Vec<(Uuid, String)> = sqlx::query_as(r#"
        SELECT id, phrase 
        FROM "ob-poc".agent_learned_invocation_phrases
        WHERE embedding IS NULL
        LIMIT 100
    "#)
    .fetch_all(pool)
    .await?;

    for (id, phrase) in phrases {
        match embedder.embed(&phrase).await {
            Ok(embedding) => {
                sqlx::query(r#"
                    UPDATE "ob-poc".agent_learned_invocation_phrases
                    SET embedding = $2, embedding_model = $3
                    WHERE id = $1
                "#)
                .bind(id)
                .bind(&embedding)
                .bind(embedder.model_name())
                .execute(pool)
                .await?;
                
                stats.phrases_embedded += 1;
            }
            Err(e) => {
                tracing::warn!(phrase = %phrase, error = %e, "Failed to embed phrase");
                stats.errors += 1;
            }
        }
    }

    // Same for entity aliases...
    // Same for blocklist...

    Ok(stats)
}

#[derive(Debug, Default)]
pub struct BackfillStats {
    pub phrases_embedded: usize,
    pub aliases_embedded: usize,
    pub blocklist_embedded: usize,
    pub errors: usize,
}
```

---

## Phase 2: Embedding Service

### 2.1 Embedder Trait

**File**: `rust/src/agent/learning/embedder.rs` (NEW)

```rust
use anyhow::Result;
use async_trait::async_trait;

/// Embedding vector type (matches pgvector)
pub type Embedding = Vec<f32>;

/// Trait for text embedding services
#[async_trait]
pub trait Embedder: Send + Sync {
    /// Generate embedding for text
    async fn embed(&self, text: &str) -> Result<Embedding>;
    
    /// Batch embed multiple texts (more efficient)
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>>;
    
    /// Model identifier for storage
    fn model_name(&self) -> &str;
    
    /// Embedding dimension
    fn dimension(&self) -> usize;
}

/// OpenAI Ada-002 embedder
pub struct OpenAIEmbedder {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl OpenAIEmbedder {
    pub fn new(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model: "text-embedding-ada-002".to_string(),
        }
    }
    
    pub fn with_model(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
        }
    }
}

#[async_trait]
impl Embedder for OpenAIEmbedder {
    async fn embed(&self, text: &str) -> Result<Embedding> {
        let response = self.client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({
                "model": self.model,
                "input": text
            }))
            .send()
            .await?
            .json::<EmbeddingResponse>()
            .await?;
        
        Ok(response.data[0].embedding.clone())
    }
    
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        let response = self.client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({
                "model": self.model,
                "input": texts
            }))
            .send()
            .await?
            .json::<EmbeddingResponse>()
            .await?;
        
        Ok(response.data.into_iter().map(|d| d.embedding).collect())
    }
    
    fn model_name(&self) -> &str {
        &self.model
    }
    
    fn dimension(&self) -> usize {
        1536  // ada-002
    }
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

/// Local/cached embedder for testing
pub struct CachedEmbedder {
    inner: Box<dyn Embedder>,
    cache: tokio::sync::RwLock<HashMap<String, Embedding>>,
}

impl CachedEmbedder {
    pub fn new(inner: Box<dyn Embedder>) -> Self {
        Self {
            inner,
            cache: tokio::sync::RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl Embedder for CachedEmbedder {
    async fn embed(&self, text: &str) -> Result<Embedding> {
        // Check cache
        {
            let cache = self.cache.read().await;
            if let Some(emb) = cache.get(text) {
                return Ok(emb.clone());
            }
        }
        
        // Generate and cache
        let embedding = self.inner.embed(text).await?;
        {
            let mut cache = self.cache.write().await;
            cache.insert(text.to_string(), embedding.clone());
        }
        
        Ok(embedding)
    }
    
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        // Simple implementation - could be optimized
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }
    
    fn model_name(&self) -> &str {
        self.inner.model_name()
    }
    
    fn dimension(&self) -> usize {
        self.inner.dimension()
    }
}
```

---

## Phase 3: Learned Semantic Search

### 3.1 Extend HybridVerbSearcher

**File**: `rust/src/mcp/verb_search.rs`

Add learned semantic search tier:

```rust
use crate::agent::learning::embedder::Embedder;

pub struct HybridVerbSearcher {
    phrase_index: VerbPhraseIndex,
    semantic_matcher: Option<SemanticMatcher>,
    learned_data: Option<SharedLearnedData>,
    embedder: Option<Arc<dyn Embedder>>,  // ← NEW
    pool: Option<PgPool>,                  // ← NEW (for learned semantic queries)
}

impl HybridVerbSearcher {
    /// Create searcher with full capabilities including learned embeddings
    pub async fn full_with_embeddings(
        verbs_dir: &str,
        pool: PgPool,
        learned_data: Option<SharedLearnedData>,
        embedder: Arc<dyn Embedder>,
    ) -> Result<Self> {
        let phrase_index = VerbPhraseIndex::load_from_verbs_dir(verbs_dir)?;
        let semantic_matcher = SemanticMatcher::new(pool.clone()).await.ok();
        
        Ok(Self {
            phrase_index,
            semantic_matcher,
            learned_data,
            embedder: Some(embedder),
            pool: Some(pool),
        })
    }

    /// Search with full pipeline including learned embeddings
    pub async fn search(
        &self,
        query: &str,
        user_id: Option<Uuid>,
        domain_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<VerbSearchResult>> {
        let mut results = Vec::new();
        let mut seen_verbs: HashSet<String> = HashSet::new();
        let normalized = query.trim().to_lowercase();

        // 1. User-specific learned phrases (exact match, in-memory or DB)
        if let Some(uid) = user_id {
            if let Some(result) = self.search_user_learned_exact(uid, &normalized).await? {
                if self.matches_domain(&result.verb, domain_filter) {
                    results.push(result);
                    seen_verbs.insert(results.last().unwrap().verb.clone());
                }
            }
        }

        // 2. Global learned phrases (exact match)
        if results.is_empty() {
            if let Some(learned) = &self.learned_data {
                let guard = learned.read().await;
                if let Some(verb) = guard.resolve_phrase(&normalized) {
                    if self.matches_domain(verb, domain_filter) {
                        results.push(VerbSearchResult {
                            verb: verb.to_string(),
                            score: 1.0,
                            source: VerbSearchSource::LearnedExact,
                            matched_phrase: query.to_string(),
                            description: None,
                        });
                        seen_verbs.insert(verb.to_string());
                    }
                }
            }
        }

        // 3. User-specific learned phrases (SEMANTIC match) ← NEW
        if results.is_empty() && user_id.is_some() {
            if let Some(result) = self.search_user_learned_semantic(
                user_id.unwrap(), query, 0.80
            ).await? {
                if self.matches_domain(&result.verb, domain_filter) 
                    && !seen_verbs.contains(&result.verb) 
                {
                    results.push(result);
                    seen_verbs.insert(results.last().unwrap().verb.clone());
                }
            }
        }

        // 4. Global learned phrases (SEMANTIC match) ← NEW
        if results.is_empty() {
            if let Some(result) = self.search_learned_semantic(query, 0.80).await? {
                if self.matches_domain(&result.verb, domain_filter)
                    && !seen_verbs.contains(&result.verb)
                {
                    results.push(result);
                    seen_verbs.insert(results.last().unwrap().verb.clone());
                }
            }
        }

        // 5. Check blocklist before proceeding ← NEW
        if !results.is_empty() {
            let blocked = self.check_blocklist(query, user_id, &results[0].verb).await?;
            if blocked {
                tracing::info!(
                    query = query,
                    verb = &results[0].verb,
                    "Verb blocked by blocklist, continuing search"
                );
                seen_verbs.insert(results.remove(0).verb);
            }
        }

        // 6. Phrase index (YAML invocation_phrases)
        if results.len() < limit {
            let phrase_matches = self.phrase_index.find_matches(query);
            for m in phrase_matches {
                if seen_verbs.contains(&m.fq_name) {
                    continue;
                }
                if !self.matches_domain(&m.fq_name, domain_filter) {
                    continue;
                }
                
                // Check blocklist for this candidate too
                if self.check_blocklist(query, user_id, &m.fq_name).await? {
                    seen_verbs.insert(m.fq_name.clone());
                    continue;
                }
                
                results.push(VerbSearchResult {
                    verb: m.fq_name.clone(),
                    score: m.confidence,
                    source: if m.confidence >= 1.0 {
                        VerbSearchSource::PhraseExact
                    } else {
                        VerbSearchSource::PhraseSubstring
                    },
                    matched_phrase: m.matched_phrase,
                    description: self.phrase_index.get_verb(&m.fq_name)
                        .map(|v| v.description.clone()),
                });
                seen_verbs.insert(m.fq_name);
                
                if results.len() >= limit {
                    break;
                }
            }
        }

        // 7. Global semantic search (cold start fallback)
        if results.len() < limit {
            if let Some(matcher) = &self.semantic_matcher {
                let remaining = limit - results.len();
                if let Ok((primary, alternatives)) = matcher
                    .find_match_with_alternatives(query, remaining + 2)
                    .await
                {
                    for candidate in std::iter::once(primary).chain(alternatives) {
                        if seen_verbs.contains(&candidate.verb_name) {
                            continue;
                        }
                        if !self.matches_domain(&candidate.verb_name, domain_filter) {
                            continue;
                        }
                        if self.check_blocklist(query, user_id, &candidate.verb_name).await? {
                            seen_verbs.insert(candidate.verb_name.clone());
                            continue;
                        }
                        
                        results.push(VerbSearchResult {
                            verb: candidate.verb_name.clone(),
                            score: candidate.similarity,
                            source: VerbSearchSource::Semantic,
                            matched_phrase: candidate.pattern_phrase,
                            description: None,
                        });
                        seen_verbs.insert(candidate.verb_name);
                        
                        if results.len() >= limit {
                            break;
                        }
                    }
                }
            }
        }

        // Sort by score, truncate
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        Ok(results)
    }

    /// Search learned phrases by semantic similarity
    async fn search_learned_semantic(
        &self,
        query: &str,
        threshold: f32,
    ) -> Result<Option<VerbSearchResult>> {
        let (pool, embedder) = match (&self.pool, &self.embedder) {
            (Some(p), Some(e)) => (p, e),
            _ => return Ok(None),
        };

        let query_embedding = embedder.embed(query).await?;
        
        let row = sqlx::query_as::<_, (String, String, f64)>(r#"
            SELECT phrase, verb, 1 - (embedding <=> $1::vector) as similarity
            FROM "ob-poc".agent_learned_invocation_phrases
            WHERE embedding IS NOT NULL
            ORDER BY embedding <=> $1::vector
            LIMIT 1
        "#)
        .bind(&query_embedding)
        .fetch_optional(pool)
        .await?;

        match row {
            Some((phrase, verb, similarity)) if similarity as f32 > threshold => {
                Ok(Some(VerbSearchResult {
                    verb,
                    score: similarity as f32,
                    source: VerbSearchSource::LearnedSemantic,
                    matched_phrase: phrase,
                    description: None,
                }))
            }
            _ => Ok(None),
        }
    }

    /// Search user-specific learned phrases by semantic similarity
    async fn search_user_learned_semantic(
        &self,
        user_id: Uuid,
        query: &str,
        threshold: f32,
    ) -> Result<Option<VerbSearchResult>> {
        let (pool, embedder) = match (&self.pool, &self.embedder) {
            (Some(p), Some(e)) => (p, e),
            _ => return Ok(None),
        };

        let query_embedding = embedder.embed(query).await?;
        
        let row = sqlx::query_as::<_, (String, String, f64)>(r#"
            SELECT phrase, verb, 1 - (embedding <=> $1::vector) as similarity
            FROM "ob-poc".agent_user_learned_phrases
            WHERE user_id = $2
              AND embedding IS NOT NULL
            ORDER BY embedding <=> $1::vector
            LIMIT 1
        "#)
        .bind(&query_embedding)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        match row {
            Some((phrase, verb, similarity)) if similarity as f32 > threshold => {
                Ok(Some(VerbSearchResult {
                    verb,
                    score: similarity as f32,
                    source: VerbSearchSource::UserLearnedSemantic,
                    matched_phrase: phrase,
                    description: None,
                }))
            }
            _ => Ok(None),
        }
    }

    /// Check if a verb is blocked for this query (semantic match)
    async fn check_blocklist(
        &self,
        query: &str,
        user_id: Option<Uuid>,
        verb: &str,
    ) -> Result<bool> {
        let (pool, embedder) = match (&self.pool, &self.embedder) {
            (Some(p), Some(e)) => (p, e),
            _ => return Ok(false),
        };

        let query_embedding = embedder.embed(query).await?;
        
        // Check if any blocklist entry matches semantically
        let blocked = sqlx::query_scalar::<_, bool>(r#"
            SELECT EXISTS (
                SELECT 1 FROM "ob-poc".agent_phrase_blocklist
                WHERE blocked_verb = $1
                  AND (user_id IS NULL OR user_id = $2)
                  AND (expires_at IS NULL OR expires_at > now())
                  AND embedding IS NOT NULL
                  AND 1 - (embedding <=> $3::vector) > 0.75
            )
        "#)
        .bind(verb)
        .bind(user_id)
        .bind(&query_embedding)
        .fetch_one(pool)
        .await?;

        Ok(blocked)
    }

    /// Search user learned phrases by exact match
    async fn search_user_learned_exact(
        &self,
        user_id: Uuid,
        phrase: &str,
    ) -> Result<Option<VerbSearchResult>> {
        let pool = match &self.pool {
            Some(p) => p,
            None => return Ok(None),
        };

        let row = sqlx::query_as::<_, (String, String, f32)>(r#"
            SELECT phrase, verb, confidence
            FROM "ob-poc".agent_user_learned_phrases
            WHERE user_id = $1 AND phrase = $2
        "#)
        .bind(user_id)
        .bind(phrase)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|(phrase, verb, confidence)| VerbSearchResult {
            verb,
            score: confidence,
            source: VerbSearchSource::UserLearnedExact,
            matched_phrase: phrase,
            description: None,
        }))
    }
}

/// Extended source types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerbSearchSource {
    UserLearnedExact,      // User-specific, exact match
    UserLearnedSemantic,   // User-specific, embedding similarity
    LearnedExact,          // Global, exact match
    LearnedSemantic,       // Global, embedding similarity
    PhraseExact,           // YAML phrase, exact match
    PhraseSubstring,       // YAML phrase, substring match
    Semantic,              // Global verb patterns (cold start)
    Phonetic,              // Voice/phonetic fallback
}
```

---

## Phase 4: Negative Feedback / Blocklist

### 4.1 Add Blocklist MCP Tool

**File**: `rust/src/mcp/tools.rs`

```rust
Tool {
    name: "intent_block".into(),
    description: r#"Block a verb from being selected for a phrase pattern.

Use when a user explicitly says "never pick X for Y" or indicates 
persistent frustration with a wrong verb being selected.

The blocklist uses semantic matching — blocking "delete everything" 
will also block "remove all data" and similar phrases.

Options:
- global: Block for all users (default)
- user_specific: Block only for current user
- expires: Optional expiry (e.g., "30d" for 30 days)"#.into(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "phrase": {
                "type": "string",
                "description": "The phrase pattern to block"
            },
            "blocked_verb": {
                "type": "string",
                "description": "The verb to block for this phrase"
            },
            "reason": {
                "type": "string",
                "description": "Why this is being blocked"
            },
            "scope": {
                "type": "string",
                "enum": ["global", "user_specific"],
                "default": "global"
            },
            "user_id": {
                "type": "string",
                "format": "uuid",
                "description": "User ID if scope is user_specific"
            },
            "expires": {
                "type": "string",
                "description": "Optional expiry duration (e.g., '30d', '1w')"
            }
        },
        "required": ["phrase", "blocked_verb"]
    }),
},
```

### 4.2 Handler Implementation

**File**: `rust/src/mcp/handlers/core.rs`

```rust
/// Block a verb for a phrase pattern
async fn intent_block(&self, args: Value) -> Result<Value> {
    let phrase = args["phrase"]
        .as_str()
        .ok_or_else(|| anyhow!("phrase required"))?;
    let blocked_verb = args["blocked_verb"]
        .as_str()
        .ok_or_else(|| anyhow!("blocked_verb required"))?;
    let reason = args["reason"].as_str();
    let scope = args["scope"].as_str().unwrap_or("global");
    let user_id = if scope == "user_specific" {
        args["user_id"].as_str().and_then(|s| s.parse().ok())
    } else {
        None
    };
    let expires = args["expires"].as_str()
        .and_then(|s| parse_duration(s).ok())
        .map(|d| chrono::Utc::now() + d);

    let pool = self.require_pool()?;
    let embedder = self.require_embedder()?;
    
    // Generate embedding for semantic matching
    let embedding = embedder.embed(phrase).await?;

    let id = sqlx::query_scalar::<_, Uuid>(r#"
        INSERT INTO "ob-poc".agent_phrase_blocklist
            (phrase, blocked_verb, embedding, reason, user_id, expires_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (phrase, blocked_verb, user_id) DO UPDATE
        SET reason = COALESCE($4, agent_phrase_blocklist.reason),
            expires_at = $6,
            embedding = $3
        RETURNING id
    "#)
    .bind(phrase)
    .bind(blocked_verb)
    .bind(&embedding)
    .bind(reason)
    .bind(user_id)
    .bind(expires)
    .fetch_one(pool)
    .await?;

    Ok(json!({
        "blocked": true,
        "block_id": id.to_string(),
        "phrase": phrase,
        "blocked_verb": blocked_verb,
        "scope": scope,
        "expires_at": expires,
        "message": format!(
            "Blocked '{}' for phrase pattern '{}'. This will catch semantically similar phrases too.",
            blocked_verb, phrase
        )
    }))
}

/// Parse duration string like "30d", "1w", "24h"
fn parse_duration(s: &str) -> Result<chrono::Duration> {
    let len = s.len();
    if len < 2 {
        return Err(anyhow!("Invalid duration format"));
    }
    
    let (num_str, unit) = s.split_at(len - 1);
    let num: i64 = num_str.parse()?;
    
    match unit {
        "d" => Ok(chrono::Duration::days(num)),
        "w" => Ok(chrono::Duration::weeks(num)),
        "h" => Ok(chrono::Duration::hours(num)),
        _ => Err(anyhow!("Unknown duration unit: {}", unit)),
    }
}
```

---

## Phase 5: Confidence Decay

### 5.1 Decay on Correction

**File**: `rust/src/agent/learning/decay.rs` (NEW)

```rust
use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

/// Confidence decay manager
pub struct ConfidenceDecay {
    pool: PgPool,
    decay_factor: f32,      // How much to decay on correction (e.g., 0.7)
    boost_factor: f32,      // How much to boost correct answer (e.g., 0.2)
    min_confidence: f32,    // Floor (e.g., 0.1)
    max_confidence: f32,    // Ceiling (e.g., 1.0)
}

impl ConfidenceDecay {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            decay_factor: 0.7,
            boost_factor: 0.2,
            min_confidence: 0.1,
            max_confidence: 1.0,
        }
    }

    /// Apply decay when a learned phrase leads to wrong verb
    pub async fn decay_wrong(
        &self,
        phrase: &str,
        wrong_verb: &str,
        user_id: Option<Uuid>,
    ) -> Result<f32> {
        // Decay in user-specific table if user_id provided
        if let Some(uid) = user_id {
            let new_conf = sqlx::query_scalar::<_, f32>(r#"
                UPDATE "ob-poc".agent_user_learned_phrases
                SET confidence = GREATEST($3, confidence * $4),
                    updated_at = now()
                WHERE user_id = $1 AND phrase = $2 AND verb = $5
                RETURNING confidence
            "#)
            .bind(uid)
            .bind(phrase)
            .bind(self.min_confidence)
            .bind(self.decay_factor)
            .bind(wrong_verb)
            .fetch_optional(&self.pool)
            .await?;
            
            if let Some(conf) = new_conf {
                return Ok(conf);
            }
        }

        // Decay in global table
        // Note: Global table doesn't have confidence column by default
        // This is handled via occurrence_count reduction instead
        sqlx::query(r#"
            UPDATE "ob-poc".agent_learned_invocation_phrases
            SET occurrence_count = GREATEST(1, occurrence_count - 1),
                updated_at = now()
            WHERE phrase = $1 AND verb = $2
        "#)
        .bind(phrase)
        .bind(wrong_verb)
        .execute(&self.pool)
        .await?;

        Ok(self.decay_factor)
    }

    /// Boost confidence when correct verb is confirmed
    pub async fn boost_correct(
        &self,
        phrase: &str,
        correct_verb: &str,
        user_id: Option<Uuid>,
    ) -> Result<f32> {
        if let Some(uid) = user_id {
            let new_conf = sqlx::query_scalar::<_, f32>(r#"
                INSERT INTO "ob-poc".agent_user_learned_phrases
                    (user_id, phrase, verb, confidence)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (user_id, phrase) DO UPDATE
                SET verb = $3,
                    confidence = LEAST($5, agent_user_learned_phrases.confidence + $4),
                    occurrence_count = agent_user_learned_phrases.occurrence_count + 1,
                    updated_at = now()
                RETURNING confidence
            "#)
            .bind(uid)
            .bind(phrase)
            .bind(correct_verb)
            .bind(self.boost_factor)
            .bind(self.max_confidence)
            .fetch_one(&self.pool)
            .await?;
            
            return Ok(new_conf);
        }

        // Boost in global table via occurrence_count
        sqlx::query(r#"
            INSERT INTO "ob-poc".agent_learned_invocation_phrases
                (phrase, verb, occurrence_count)
            VALUES ($1, $2, 1)
            ON CONFLICT (phrase) DO UPDATE
            SET verb = $2,
                occurrence_count = agent_learned_invocation_phrases.occurrence_count + 1,
                updated_at = now()
        "#)
        .bind(phrase)
        .bind(correct_verb)
        .execute(&self.pool)
        .await?;

        Ok(self.max_confidence)
    }

    /// Handle a correction event (decay wrong, boost correct)
    pub async fn handle_correction(
        &self,
        phrase: &str,
        wrong_verb: &str,
        correct_verb: &str,
        user_id: Option<Uuid>,
    ) -> Result<DecayResult> {
        let decayed = self.decay_wrong(phrase, wrong_verb, user_id).await?;
        let boosted = self.boost_correct(phrase, correct_verb, user_id).await?;
        
        Ok(DecayResult {
            phrase: phrase.to_string(),
            wrong_verb: wrong_verb.to_string(),
            wrong_new_confidence: decayed,
            correct_verb: correct_verb.to_string(),
            correct_new_confidence: boosted,
        })
    }
}

#[derive(Debug)]
pub struct DecayResult {
    pub phrase: String,
    pub wrong_verb: String,
    pub wrong_new_confidence: f32,
    pub correct_verb: String,
    pub correct_new_confidence: f32,
}
```

### 5.2 Wire into Intent Feedback Handler

**File**: `rust/src/mcp/handlers/core.rs`

In `intent_feedback` handler, after recording feedback:

```rust
// Apply confidence decay for verb corrections
if feedback_type == "verb_correction" && system_choice.is_some() {
    let decay = ConfidenceDecay::new(pool.clone());
    let result = decay.handle_correction(
        &original_input,
        system_choice.as_ref().unwrap(),
        correct_choice,
        user_id,
    ).await?;
    
    tracing::info!(
        phrase = %result.phrase,
        wrong = %result.wrong_verb,
        wrong_conf = result.wrong_new_confidence,
        correct = %result.correct_verb,
        correct_conf = result.correct_new_confidence,
        "Applied confidence decay"
    );
}
```

---

## Phase 6: Bulk Import

### 6.1 Import Tool Definition

**File**: `rust/src/mcp/tools.rs`

```rust
Tool {
    name: "learning_import".into(),
    description: r#"Bulk import phrase→verb mappings from YAML/JSON.

Used for:
- Bootstrapping new deployments with client terminology
- Loading glossaries and SOPs
- Migrating from other systems

Format (YAML):
```yaml
phrases:
  - phrase: "spin up a fund"
    verb: cbu.create
  - phrase: "add a sig"
    verb: entity.assign-role
    context:
      role: signatory
```

Embeddings are generated automatically for semantic matching."#.into(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "source": {
                "type": "string",
                "enum": ["file", "inline"],
                "description": "Import from file path or inline content"
            },
            "path": {
                "type": "string",
                "description": "File path (if source=file)"
            },
            "content": {
                "type": "string",
                "description": "YAML/JSON content (if source=inline)"
            },
            "format": {
                "type": "string",
                "enum": ["yaml", "json", "csv"],
                "default": "yaml"
            },
            "scope": {
                "type": "string",
                "enum": ["global", "user_specific"],
                "default": "global"
            },
            "user_id": {
                "type": "string",
                "format": "uuid",
                "description": "User ID if scope is user_specific"
            },
            "dry_run": {
                "type": "boolean",
                "default": false,
                "description": "Validate without importing"
            }
        },
        "required": ["source"]
    }),
},
```

### 6.2 Import Handler

**File**: `rust/src/mcp/handlers/core.rs`

```rust
/// Bulk import phrase mappings
async fn learning_import(&self, args: Value) -> Result<Value> {
    let source = args["source"].as_str().ok_or_else(|| anyhow!("source required"))?;
    let format = args["format"].as_str().unwrap_or("yaml");
    let scope = args["scope"].as_str().unwrap_or("global");
    let user_id = if scope == "user_specific" {
        args["user_id"].as_str().and_then(|s| s.parse().ok())
    } else {
        None
    };
    let dry_run = args["dry_run"].as_bool().unwrap_or(false);

    // Parse content
    let content = match source {
        "file" => {
            let path = args["path"].as_str().ok_or_else(|| anyhow!("path required"))?;
            std::fs::read_to_string(path)?
        }
        "inline" => {
            args["content"].as_str()
                .ok_or_else(|| anyhow!("content required"))?
                .to_string()
        }
        _ => return Err(anyhow!("Invalid source: {}", source)),
    };

    let import_data: ImportData = match format {
        "yaml" => serde_yaml::from_str(&content)?,
        "json" => serde_json::from_str(&content)?,
        "csv" => parse_csv_import(&content)?,
        _ => return Err(anyhow!("Unknown format: {}", format)),
    };

    // Validate
    let mut validation_errors = Vec::new();
    for (i, phrase) in import_data.phrases.iter().enumerate() {
        if phrase.phrase.is_empty() {
            validation_errors.push(format!("Row {}: empty phrase", i + 1));
        }
        if phrase.verb.is_empty() {
            validation_errors.push(format!("Row {}: empty verb", i + 1));
        }
        // Validate verb exists
        if !self.verb_exists(&phrase.verb) {
            validation_errors.push(format!("Row {}: unknown verb '{}'", i + 1, phrase.verb));
        }
    }

    if !validation_errors.is_empty() {
        return Ok(json!({
            "success": false,
            "validation_errors": validation_errors,
            "message": "Import failed validation"
        }));
    }

    if dry_run {
        return Ok(json!({
            "success": true,
            "dry_run": true,
            "would_import": import_data.phrases.len(),
            "message": "Validation passed, ready to import"
        }));
    }

    // Import with embeddings
    let pool = self.require_pool()?;
    let embedder = self.require_embedder()?;
    
    let mut imported = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();

    // Batch embed for efficiency
    let phrases: Vec<&str> = import_data.phrases.iter()
        .map(|p| p.phrase.as_str())
        .collect();
    let embeddings = embedder.embed_batch(&phrases).await?;

    for (phrase_data, embedding) in import_data.phrases.iter().zip(embeddings) {
        let result = if user_id.is_some() {
            sqlx::query(r#"
                INSERT INTO "ob-poc".agent_user_learned_phrases
                    (user_id, phrase, verb, embedding, source)
                VALUES ($1, $2, $3, $4, 'bulk_import')
                ON CONFLICT (user_id, phrase) DO UPDATE
                SET verb = $3, embedding = $4, updated_at = now()
            "#)
            .bind(user_id)
            .bind(&phrase_data.phrase)
            .bind(&phrase_data.verb)
            .bind(&embedding)
            .execute(pool)
            .await
        } else {
            sqlx::query(r#"
                INSERT INTO "ob-poc".agent_learned_invocation_phrases
                    (phrase, verb, embedding, source)
                VALUES ($1, $2, $3, 'bulk_import')
                ON CONFLICT (phrase) DO UPDATE
                SET verb = $2, embedding = $3, updated_at = now()
            "#)
            .bind(&phrase_data.phrase)
            .bind(&phrase_data.verb)
            .bind(&embedding)
            .execute(pool)
            .await
        };

        match result {
            Ok(_) => imported += 1,
            Err(e) => {
                errors.push(format!("{}: {}", phrase_data.phrase, e));
                skipped += 1;
            }
        }
    }

    Ok(json!({
        "success": true,
        "imported": imported,
        "skipped": skipped,
        "errors": errors,
        "scope": scope,
        "message": format!("Imported {} phrase mappings", imported)
    }))
}

#[derive(Debug, Deserialize)]
struct ImportData {
    phrases: Vec<PhraseMapping>,
}

#[derive(Debug, Deserialize)]
struct PhraseMapping {
    phrase: String,
    verb: String,
    #[serde(default)]
    context: Option<serde_json::Value>,
}

fn parse_csv_import(content: &str) -> Result<ImportData> {
    let mut phrases = Vec::new();
    for line in content.lines().skip(1) { // Skip header
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 2 {
            phrases.push(PhraseMapping {
                phrase: parts[0].trim().to_string(),
                verb: parts[1].trim().to_string(),
                context: None,
            });
        }
    }
    Ok(ImportData { phrases })
}
```

---

## Phase 7: Learning Dashboard (MCP Tools)

### 7.1 Tool Definitions

**File**: `rust/src/mcp/tools.rs`

```rust
Tool {
    name: "learning_list".into(),
    description: "List pending learning candidates for review".into(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "status": {
                "type": "string",
                "enum": ["pending", "approved", "rejected", "applied", "all"],
                "default": "pending"
            },
            "learning_type": {
                "type": "string",
                "enum": ["invocation_phrase", "entity_alias", "all"],
                "default": "all"
            },
            "min_occurrences": {
                "type": "integer",
                "default": 1,
                "description": "Filter by minimum occurrence count"
            },
            "limit": {
                "type": "integer",
                "default": 20
            }
        }
    }),
},

Tool {
    name: "learning_approve".into(),
    description: "Approve a learning candidate for application".into(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "candidate_id": {
                "type": "string",
                "format": "uuid",
                "description": "Learning candidate ID"
            },
            "apply_immediately": {
                "type": "boolean",
                "default": true,
                "description": "Apply to active learned data immediately"
            }
        },
        "required": ["candidate_id"]
    }),
},

Tool {
    name: "learning_reject".into(),
    description: "Reject a learning candidate".into(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "candidate_id": {
                "type": "string",
                "format": "uuid"
            },
            "reason": {
                "type": "string",
                "description": "Rejection reason"
            },
            "add_to_blocklist": {
                "type": "boolean",
                "default": false,
                "description": "Also add to blocklist to prevent re-learning"
            }
        },
        "required": ["candidate_id"]
    }),
},

Tool {
    name: "learning_stats".into(),
    description: "Get learning system statistics and health metrics".into(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "time_range": {
                "type": "string",
                "enum": ["day", "week", "month", "all"],
                "default": "week"
            },
            "include_top_corrections": {
                "type": "boolean",
                "default": true
            }
        }
    }),
},
```

### 7.2 Handler Implementations

**File**: `rust/src/mcp/handlers/core.rs`

```rust
/// List learning candidates
async fn learning_list(&self, args: Value) -> Result<Value> {
    let pool = self.require_pool()?;
    let status = args["status"].as_str().unwrap_or("pending");
    let learning_type = args["learning_type"].as_str().unwrap_or("all");
    let min_occurrences = args["min_occurrences"].as_i64().unwrap_or(1) as i32;
    let limit = args["limit"].as_i64().unwrap_or(20) as i32;

    let status_filter = if status == "all" { "%" } else { status };
    let type_filter = if learning_type == "all" { "%" } else { learning_type };

    let candidates = sqlx::query_as::<_, LearningCandidateRow>(r#"
        SELECT id, learning_type, input_pattern, suggested_output, previous_output,
               occurrence_count, risk_level, status, user_explanation, created_at
        FROM "ob-poc".agent_learning_candidates
        WHERE status LIKE $1
          AND learning_type LIKE $2
          AND occurrence_count >= $3
        ORDER BY 
            CASE status WHEN 'pending' THEN 0 ELSE 1 END,
            occurrence_count DESC,
            created_at DESC
        LIMIT $4
    "#)
    .bind(status_filter)
    .bind(type_filter)
    .bind(min_occurrences)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let items: Vec<Value> = candidates.iter().map(|c| json!({
        "id": c.id.to_string(),
        "type": c.learning_type,
        "input": c.input_pattern,
        "suggested": c.suggested_output,
        "previous": c.previous_output,
        "occurrences": c.occurrence_count,
        "risk": c.risk_level,
        "status": c.status,
        "explanation": c.user_explanation,
        "created": c.created_at.to_rfc3339(),
    })).collect();

    Ok(json!({
        "candidates": items,
        "count": items.len(),
        "filters": {
            "status": status,
            "learning_type": learning_type,
            "min_occurrences": min_occurrences
        }
    }))
}

/// Approve learning candidate
async fn learning_approve(&self, args: Value) -> Result<Value> {
    let pool = self.require_pool()?;
    let embedder = self.require_embedder()?;
    
    let candidate_id: Uuid = args["candidate_id"]
        .as_str()
        .ok_or_else(|| anyhow!("candidate_id required"))?
        .parse()?;
    let apply_immediately = args["apply_immediately"].as_bool().unwrap_or(true);

    // Get candidate
    let candidate = sqlx::query_as::<_, LearningCandidateRow>(r#"
        SELECT * FROM "ob-poc".agent_learning_candidates WHERE id = $1
    "#)
    .bind(candidate_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("Candidate not found"))?;

    // Update status
    sqlx::query(r#"
        UPDATE "ob-poc".agent_learning_candidates
        SET status = 'approved', updated_at = now()
        WHERE id = $1
    "#)
    .bind(candidate_id)
    .execute(pool)
    .await?;

    // Apply if requested
    let applied = if apply_immediately {
        let embedding = embedder.embed(&candidate.input_pattern).await?;
        
        match candidate.learning_type.as_str() {
            "invocation_phrase" | "InvocationPhrase" => {
                sqlx::query(r#"
                    INSERT INTO "ob-poc".agent_learned_invocation_phrases
                        (phrase, verb, embedding, source)
                    VALUES ($1, $2, $3, 'approved_candidate')
                    ON CONFLICT (phrase) DO UPDATE
                    SET verb = $2, embedding = $3, updated_at = now()
                "#)
                .bind(&candidate.input_pattern)
                .bind(&candidate.suggested_output)
                .bind(&embedding)
                .execute(pool)
                .await?;
                
                // Update candidate status to applied
                sqlx::query(r#"
                    UPDATE "ob-poc".agent_learning_candidates
                    SET status = 'applied', updated_at = now()
                    WHERE id = $1
                "#)
                .bind(candidate_id)
                .execute(pool)
                .await?;
                
                true
            }
            "entity_alias" | "EntityAlias" => {
                sqlx::query(r#"
                    INSERT INTO "ob-poc".agent_learned_entity_aliases
                        (alias, canonical_name, embedding, source)
                    VALUES ($1, $2, $3, 'approved_candidate')
                    ON CONFLICT (alias) DO UPDATE
                    SET canonical_name = $2, embedding = $3, updated_at = now()
                "#)
                .bind(&candidate.input_pattern)
                .bind(&candidate.suggested_output)
                .bind(&embedding)
                .execute(pool)
                .await?;
                
                sqlx::query(r#"
                    UPDATE "ob-poc".agent_learning_candidates
                    SET status = 'applied', updated_at = now()
                    WHERE id = $1
                "#)
                .bind(candidate_id)
                .execute(pool)
                .await?;
                
                true
            }
            _ => false
        }
    } else {
        false
    };

    Ok(json!({
        "approved": true,
        "applied": applied,
        "candidate_id": candidate_id.to_string(),
        "mapping": format!("'{}' → {}", candidate.input_pattern, candidate.suggested_output)
    }))
}

/// Reject learning candidate
async fn learning_reject(&self, args: Value) -> Result<Value> {
    let pool = self.require_pool()?;
    
    let candidate_id: Uuid = args["candidate_id"]
        .as_str()
        .ok_or_else(|| anyhow!("candidate_id required"))?
        .parse()?;
    let reason = args["reason"].as_str();
    let add_to_blocklist = args["add_to_blocklist"].as_bool().unwrap_or(false);

    // Get candidate before rejecting
    let candidate = sqlx::query_as::<_, LearningCandidateRow>(r#"
        SELECT * FROM "ob-poc".agent_learning_candidates WHERE id = $1
    "#)
    .bind(candidate_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("Candidate not found"))?;

    // Update status
    sqlx::query(r#"
        UPDATE "ob-poc".agent_learning_candidates
        SET status = 'rejected', 
            user_explanation = COALESCE($2, user_explanation),
            updated_at = now()
        WHERE id = $1
    "#)
    .bind(candidate_id)
    .bind(reason)
    .execute(pool)
    .await?;

    // Optionally add to blocklist
    if add_to_blocklist && candidate.learning_type.contains("phrase") {
        let embedder = self.require_embedder()?;
        let embedding = embedder.embed(&candidate.input_pattern).await?;
        
        sqlx::query(r#"
            INSERT INTO "ob-poc".agent_phrase_blocklist
                (phrase, blocked_verb, embedding, reason, source)
            VALUES ($1, $2, $3, $4, 'rejected_candidate')
            ON CONFLICT (phrase, blocked_verb, user_id) DO NOTHING
        "#)
        .bind(&candidate.input_pattern)
        .bind(&candidate.suggested_output)
        .bind(&embedding)
        .bind(reason.unwrap_or("Rejected learning candidate"))
        .execute(pool)
        .await?;
    }

    Ok(json!({
        "rejected": true,
        "candidate_id": candidate_id.to_string(),
        "added_to_blocklist": add_to_blocklist,
        "reason": reason
    }))
}

/// Get learning statistics
async fn learning_stats(&self, args: Value) -> Result<Value> {
    let pool = self.require_pool()?;
    let time_range = args["time_range"].as_str().unwrap_or("week");
    let include_top = args["include_top_corrections"].as_bool().unwrap_or(true);

    let interval = match time_range {
        "day" => "1 day",
        "week" => "7 days",
        "month" => "30 days",
        _ => "1000 years", // "all"
    };

    // Overall stats
    let stats = sqlx::query_as::<_, (i64, i64, i64, i64)>(r#"
        SELECT 
            COUNT(*) FILTER (WHERE status = 'pending'),
            COUNT(*) FILTER (WHERE status = 'approved'),
            COUNT(*) FILTER (WHERE status = 'applied'),
            COUNT(*) FILTER (WHERE status = 'rejected')
        FROM "ob-poc".agent_learning_candidates
        WHERE created_at > now() - $1::interval
    "#)
    .bind(interval)
    .fetch_one(pool)
    .await?;

    // Learned data counts
    let learned = sqlx::query_as::<_, (i64, i64, i64)>(r#"
        SELECT
            (SELECT COUNT(*) FROM "ob-poc".agent_learned_invocation_phrases),
            (SELECT COUNT(*) FROM "ob-poc".agent_learned_entity_aliases),
            (SELECT COUNT(*) FROM "ob-poc".agent_phrase_blocklist WHERE expires_at IS NULL OR expires_at > now())
    "#)
    .fetch_one(pool)
    .await?;

    let mut result = json!({
        "time_range": time_range,
        "candidates": {
            "pending": stats.0,
            "approved": stats.1,
            "applied": stats.2,
            "rejected": stats.3
        },
        "active_learned_data": {
            "invocation_phrases": learned.0,
            "entity_aliases": learned.1,
            "blocklist_entries": learned.2
        }
    });

    // Top corrections
    if include_top {
        let top = sqlx::query_as::<_, (String, String, i64)>(r#"
            SELECT input_pattern, suggested_output, occurrence_count
            FROM "ob-poc".agent_learning_candidates
            WHERE status = 'pending'
              AND learning_type LIKE '%phrase%'
              AND created_at > now() - $1::interval
            ORDER BY occurrence_count DESC
            LIMIT 10
        "#)
        .bind(interval)
        .fetch_all(pool)
        .await?;

        result["top_pending_phrases"] = json!(top.iter().map(|(input, output, count)| {
            json!({
                "phrase": input,
                "suggested_verb": output,
                "occurrences": count
            })
        }).collect::<Vec<_>>());
    }

    Ok(result)
}

#[derive(Debug, sqlx::FromRow)]
struct LearningCandidateRow {
    id: Uuid,
    learning_type: String,
    input_pattern: String,
    suggested_output: String,
    previous_output: Option<String>,
    occurrence_count: i32,
    risk_level: String,
    status: String,
    user_explanation: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}
```

---

## Testing Checklist

### pgvector Integration
- [ ] Embeddings generated on phrase insert
- [ ] Semantic search returns similar phrases (>0.8 threshold)
- [ ] IVFFlat index improves query performance
- [ ] Backfill migration works on existing data

### Negative Feedback
- [ ] `intent_block` creates blocklist entry with embedding
- [ ] Blocked verbs filtered in `verb_search`
- [ ] Semantic blocking catches paraphrases
- [ ] Expiry works correctly

### Confidence Decay
- [ ] Wrong verb confidence decays on correction
- [ ] Correct verb confidence boosts
- [ ] Min/max confidence bounds enforced
- [ ] User-specific decay isolated from global

### User-Specific Learning
- [ ] User phrases checked before global
- [ ] User semantic search isolated
- [ ] Global fallback when no user match

### Bulk Import
- [ ] YAML/JSON/CSV parsing works
- [ ] Validation catches bad verbs
- [ ] Embeddings generated for all imports
- [ ] Dry run mode works

### Dashboard Tools
- [ ] `learning_list` filters correctly
- [ ] `learning_approve` applies with embedding
- [ ] `learning_reject` optionally adds to blocklist
- [ ] `learning_stats` returns accurate counts

---

## Files Summary

| File | Changes |
|------|---------|
| `migrations/learning_embeddings.sql` | NEW - pgvector schema extensions |
| `rust/src/agent/learning/embedder.rs` | NEW - Embedder trait + OpenAI impl |
| `rust/src/agent/learning/decay.rs` | NEW - Confidence decay logic |
| `rust/src/agent/learning/migrations.rs` | NEW - Backfill utilities |
| `rust/src/mcp/verb_search.rs` | Extended with semantic learned search |
| `rust/src/mcp/tools.rs` | Add 5 new tools |
| `rust/src/mcp/handlers/core.rs` | Add 5 new handlers |

---

## Success Criteria

1. **Semantic generalization**: One learned phrase catches 5-10 paraphrases
2. **Blocklist effective**: Blocked verbs don't appear even with paraphrased queries
3. **User isolation**: User A's vocabulary doesn't affect User B
4. **Bulk import coverage**: 50 imported phrases → 200+ effective coverage
5. **Dashboard usable**: Admin can review/approve/reject via Claude conversation
6. **Hit rate improvement**: +15-20% vs exact-match-only baseline

---

## Dependencies

- `pgvector` PostgreSQL extension (already installed)
- OpenAI API key (for ada-002 embeddings)
- `reqwest` for API calls
- `async-trait` for Embedder trait
