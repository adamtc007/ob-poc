# TODO: MCP Semantic Intent → DSL Pipeline

**Priority**: HIGH  
**Created**: 2025-01-17  
**Status**: NOT STARTED

## Overview

The current `dsl_generate` MCP tool bypasses all existing semantic discovery machinery and goes straight to "LLM writes DSL text". This reintroduces the exact brittleness we built the semantic infrastructure to remove.

**Current Flow (broken)**:
```
User Intent → LLM writes raw DSL → parse/validate → hope it works
```

**Target Flow (deterministic)**:
```
User Intent → verb_search (learned + phrase + semantic) → signature lookup → 
LLM extracts arguments only → deterministic DSL assembly → validate
```

## Design Principles

1. **LLM never writes DSL syntax** — It only extracts argument values from natural language
2. **Clean replacement, not deprecation** — DELETE the old `dsl_generate`, name the new one `dsl_generate`
3. **No tool pollution** — LLMs pattern-match on what's present; if it exists, it's a valid option
4. **Single path** — The semantic pipeline *is* DSL generation now
5. **Learned phrases first** — Bypass semantic similarity entirely for known user vocabulary

---

## Architecture Summary

### Existing Components (already built)

| Component | Location | Status |
|-----------|----------|--------|
| `VerbPhraseIndex` | `ob-agentic/src/lexicon/verb_phrases.rs` | ✅ Built |
| `SemanticMatcher` | `ob-semantic-matcher/src/matcher.rs` | ✅ Built |
| `LearnedData` | `src/agent/learning/warmup.rs` | ✅ Built |
| `AgentLearningInspector` | `src/agent/learning/inspector.rs` | ✅ Built |
| `AgentEventEmitter` | `src/agent/learning/emitter.rs` | ✅ Built |
| DB Schema (`agent.*`) | `migrations/032_agent_learning.sql` | ✅ Migrated |

### New Components to Add

| Component | Purpose |
|-----------|---------|
| `verb_search` MCP tool | Expose semantic verb discovery to Claude |
| `HybridVerbSearcher` | Combine learned + phrase + semantic in priority order |
| `IntentPipeline` | Structured intent → deterministic DSL assembly |

---

## Phase 1: Add `verb_search` MCP Tool

### 1.1 Create HybridVerbSearcher

**File**: `rust/src/mcp/verb_search.rs` (NEW)

```rust
//! Hybrid Verb Search
//!
//! Combines multiple verb discovery strategies in priority order:
//! 1. Learned invocation phrases (from user corrections) - EXACT MATCH
//! 2. Exact phrase match from YAML invocation_phrases
//! 3. Substring phrase match
//! 4. Semantic embedding similarity (pgvector)
//!
//! Key insight: Learned phrases bypass semantic similarity entirely.
//! They're exact matches from real user vocabulary → your verbs.

use anyhow::Result;
use ob_agentic::lexicon::verb_phrases::VerbPhraseIndex;
use ob_semantic_matcher::SemanticMatcher;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashSet;

use crate::agent::learning::warmup::SharedLearnedData;

/// A unified verb search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbSearchResult {
    pub verb: String,
    pub score: f32,
    pub source: VerbSearchSource,
    pub matched_phrase: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerbSearchSource {
    /// From user corrections (highest priority, exact match)
    Learned,
    /// Exact match from YAML invocation_phrases
    PhraseExact,
    /// Substring match from YAML
    PhraseSubstring,
    /// pgvector embedding similarity
    Semantic,
}

/// Hybrid verb searcher combining all discovery strategies
pub struct HybridVerbSearcher {
    phrase_index: VerbPhraseIndex,
    semantic_matcher: Option<SemanticMatcher>,
    learned_data: Option<SharedLearnedData>,
}

impl Clone for HybridVerbSearcher {
    fn clone(&self) -> Self {
        Self {
            phrase_index: self.phrase_index.clone(),
            semantic_matcher: self.semantic_matcher.clone(),
            learned_data: self.learned_data.clone(),
        }
    }
}

impl HybridVerbSearcher {
    /// Create searcher with phrase index only (no DB required)
    pub fn phrase_only(verbs_dir: &str) -> Result<Self> {
        let phrase_index = VerbPhraseIndex::load_from_verbs_dir(verbs_dir)?;
        Ok(Self {
            phrase_index,
            semantic_matcher: None,
            learned_data: None,
        })
    }

    /// Create searcher with full capabilities
    pub async fn full(
        verbs_dir: &str,
        pool: PgPool,
        learned_data: Option<SharedLearnedData>,
    ) -> Result<Self> {
        let phrase_index = VerbPhraseIndex::load_from_verbs_dir(verbs_dir)?;
        let semantic_matcher = SemanticMatcher::new(pool).await.ok();
        
        Ok(Self {
            phrase_index,
            semantic_matcher,
            learned_data,
        })
    }

    /// Search for verbs matching user intent
    ///
    /// Priority order:
    /// 1. Learned phrases (from agent.invocation_phrases) - score 1.0
    /// 2. Exact phrase match from YAML - score 1.0  
    /// 3. Substring phrase match - score 0.7-0.9
    /// 4. Semantic similarity - score 0.5-0.95
    pub async fn search(
        &self,
        query: &str,
        domain_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<VerbSearchResult>> {
        let mut results = Vec::new();
        let mut seen_verbs: HashSet<String> = HashSet::new();
        let normalized = query.trim().to_lowercase();

        // 1. Check LEARNED invocation phrases FIRST (highest priority)
        // These bypass all fuzzy matching - they're exact user vocabulary
        if let Some(learned) = &self.learned_data {
            let guard = learned.read().await;
            if let Some(verb) = guard.resolve_phrase(&normalized) {
                if self.matches_domain(verb, domain_filter) {
                    results.push(VerbSearchResult {
                        verb: verb.to_string(),
                        score: 1.0, // Perfect score - user taught us this
                        source: VerbSearchSource::Learned,
                        matched_phrase: query.to_string(),
                        description: self.get_verb_description(verb),
                    });
                    seen_verbs.insert(verb.to_string());
                }
            }
        }

        // 2. Phrase index (exact + substring from YAML)
        let phrase_matches = self.phrase_index.find_matches(query);
        for m in phrase_matches {
            if seen_verbs.contains(&m.fq_name) {
                continue;
            }
            if !self.matches_domain(&m.fq_name, domain_filter) {
                continue;
            }
            
            let source = if m.confidence >= 1.0 {
                VerbSearchSource::PhraseExact
            } else {
                VerbSearchSource::PhraseSubstring
            };
            
            results.push(VerbSearchResult {
                verb: m.fq_name.clone(),
                score: m.confidence,
                source,
                matched_phrase: m.matched_phrase,
                description: self.get_verb_description(&m.fq_name),
            });
            seen_verbs.insert(m.fq_name);
        }

        // 3. Semantic search (fallback for novel phrases)
        if results.len() < limit {
            if let Some(matcher) = &self.semantic_matcher {
                let remaining = limit - results.len();
                if let Ok((primary, alternatives)) = matcher
                    .find_match_with_alternatives(query, remaining + 2)
                    .await
                {
                    // Add primary
                    if !seen_verbs.contains(&primary.verb_name) 
                        && self.matches_domain(&primary.verb_name, domain_filter) 
                    {
                        results.push(VerbSearchResult {
                            verb: primary.verb_name.clone(),
                            score: primary.similarity,
                            source: VerbSearchSource::Semantic,
                            matched_phrase: primary.pattern_phrase,
                            description: None,
                        });
                        seen_verbs.insert(primary.verb_name);
                    }
                    
                    // Add alternatives
                    for alt in alternatives {
                        if seen_verbs.contains(&alt.verb_name) {
                            continue;
                        }
                        if !self.matches_domain(&alt.verb_name, domain_filter) {
                            continue;
                        }
                        results.push(VerbSearchResult {
                            verb: alt.verb_name.clone(),
                            score: alt.similarity,
                            source: VerbSearchSource::Semantic,
                            matched_phrase: alt.pattern_phrase,
                            description: None,
                        });
                        seen_verbs.insert(alt.verb_name);
                    }
                }
            }
        }

        // Sort by score descending, truncate
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        Ok(results)
    }

    fn matches_domain(&self, verb: &str, filter: Option<&str>) -> bool {
        match filter {
            Some(d) => verb.starts_with(&format!("{}.", d)) || verb.starts_with(d),
            None => true,
        }
    }

    fn get_verb_description(&self, verb: &str) -> Option<String> {
        self.phrase_index
            .get_verb(verb)
            .map(|v| v.description.clone())
    }
}
```

### 1.2 Add Tool Definition

**File**: `rust/src/mcp/tools.rs`

Add after `dsl_signature` tool (around line 205):

```rust
Tool {
    name: "verb_search".into(),
    description: r#"Search for DSL verbs by natural language intent.

Combines multiple discovery strategies in priority order:
1. Learned phrases from user corrections (exact match, highest priority)
2. Exact phrase match from verb definitions
3. Substring phrase match  
4. Semantic embedding similarity (fallback)

Returns ranked candidates with confidence scores and sources.
ALWAYS use this before generating DSL to find the correct verb.

Examples:
- "set up ISDA agreement" → trading-profile.add-isda-config
- "add a director" → entity.assign-role
- "create Luxembourg fund" → cbu.create"#.into(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "Natural language description of desired action"
            },
            "domain": {
                "type": "string", 
                "description": "Optional domain filter (e.g., 'cbu', 'entity', 'trading-profile')"
            },
            "limit": {
                "type": "integer",
                "default": 5,
                "description": "Max results (1-20)"
            }
        },
        "required": ["query"]
    }),
},
```

### 1.3 Add Handler Implementation

**File**: `rust/src/mcp/handlers/core.rs`

**Add to imports**:
```rust
use crate::mcp::verb_search::{HybridVerbSearcher, VerbSearchResult, VerbSearchSource};
use crate::agent::learning::warmup::SharedLearnedData;
```

**Add to `ToolHandlers` struct**:
```rust
pub struct ToolHandlers {
    // ... existing fields
    verb_searcher: Option<HybridVerbSearcher>,
    learned_data: Option<SharedLearnedData>,
}
```

**Add new constructor**:
```rust
/// Create handlers with full semantic search capabilities
pub async fn with_semantic_search(
    pool: PgPool,
    sessions: SessionStore,
    cbu_sessions: CbuSessionStore,
    learned_data: SharedLearnedData,
) -> Result<Self> {
    let verb_searcher = HybridVerbSearcher::full(
        "config/verbs",
        pool.clone(),
        Some(learned_data.clone()),
    ).await.ok();
    
    Ok(Self {
        generation_log: GenerationLogRepository::new(pool.clone()),
        repo: VisualizationRepository::new(pool.clone()),
        pool,
        gateway_client: Arc::new(Mutex::new(None)),
        sessions: Some(sessions),
        cbu_sessions: Some(cbu_sessions),
        verb_searcher,
        learned_data: Some(learned_data),
    })
}
```

**Add to dispatch match**:
```rust
"verb_search" => self.verb_search(args).await,
```

**Add handler method**:
```rust
/// Search verbs by semantic intent
async fn verb_search(&self, args: Value) -> Result<Value> {
    let query = args["query"]
        .as_str()
        .ok_or_else(|| anyhow!("query required"))?;
    let domain = args["domain"].as_str();
    let limit = args["limit"].as_u64().unwrap_or(5).min(20) as usize;

    let results = if let Some(searcher) = &self.verb_searcher {
        searcher.search(query, domain, limit).await?
    } else {
        // Fallback: phrase index only (no learned data, no semantic)
        let phrase_index = VerbPhraseIndex::load_from_verbs_dir("config/verbs")?;
        phrase_index.find_matches(query)
            .into_iter()
            .take(limit)
            .map(|m| VerbSearchResult {
                verb: m.fq_name,
                score: m.confidence,
                source: VerbSearchSource::PhraseSubstring,
                matched_phrase: m.matched_phrase,
                description: None,
            })
            .collect()
    };

    // Enrich with signatures for top results
    let enriched: Vec<Value> = results.iter().map(|r| {
        let sig = self.get_verb_signature_summary(&r.verb);
        json!({
            "verb": r.verb,
            "score": r.score,
            "source": r.source,
            "matched_phrase": r.matched_phrase,
            "description": r.description,
            "signature": sig,
        })
    }).collect();

    Ok(json!({
        "query": query,
        "domain_filter": domain,
        "match_count": enriched.len(),
        "matches": enriched
    }))
}

fn get_verb_signature_summary(&self, verb_name: &str) -> Option<Value> {
    let reg = registry();
    let parts: Vec<&str> = verb_name.splitn(2, '.').collect();
    if parts.len() != 2 {
        return None;
    }
    
    reg.get_verb(parts[0], parts[1]).map(|verb| {
        let required: Vec<&str> = verb.args.iter()
            .filter(|p| p.required)
            .map(|p| p.name.as_str())
            .collect();
        let optional: Vec<&str> = verb.args.iter()
            .filter(|p| !p.required)
            .map(|p| p.name.as_str())
            .collect();
        json!({
            "required_params": required,
            "optional_params": optional,
        })
    })
}
```

### 1.4 Register Module

**File**: `rust/src/mcp/mod.rs`

```rust
pub mod verb_search;
```

---

## Phase 2: Fix Schema Drift in `dsl_lookup`

**File**: `rust/src/mcp/tools.rs`

**Find** (around line 145):
```rust
"lookup_type": {
    "type": "string",
    "enum": ["cbu", "entity", "document", "product", "service", "kyc_case", "attribute"],
```

**Replace with**:
```rust
"lookup_type": {
    "type": "string",
    "enum": [
        "cbu", 
        "entity", 
        "person", 
        "legal_entity",
        "company",
        "fund",
        "document", 
        "product", 
        "service", 
        "kyc_case", 
        "attribute",
        "role",
        "jurisdiction",
        "currency",
        "instrument_class",
        "market"
    ],
```

Update handler mapping in `core.rs`:
```rust
let nickname = match lookup_type {
    "cbu" => "cbu",
    "entity" | "person" | "legal_entity" | "company" | "fund" => "entity",
    "document" => "document",
    "product" => "product",
    "service" => "service",
    "kyc_case" => "kyc_case",
    "attribute" => "attribute",
    "role" => "role_type",
    "jurisdiction" => "jurisdiction",
    "currency" => "currency",
    "instrument_class" => "instrument_class",
    "market" => "market",
    _ => return Err(anyhow!("Unknown lookup_type: {}", lookup_type)),
};
```

---

## Phase 3: Wire Learning Loop at Startup

### 3.1 Initialize Learning Warmup

**File**: `rust/src/mcp/server.rs` (or main server init)

```rust
use crate::agent::learning::warmup::LearningWarmup;
use crate::agent::learning::drain::spawn_agent_drain_task;
use crate::agent::learning::emitter::AgentEventEmitter;

// In server initialization:

// 1. Warmup: load learned data + apply pending thresholds
let warmup = LearningWarmup::new(pool.clone());
let (learned_data, warmup_stats) = warmup.warmup().await?;

tracing::info!(
    aliases = warmup_stats.entity_aliases_loaded,
    tokens = warmup_stats.lexicon_tokens_loaded,
    phrases = warmup_stats.invocation_phrases_loaded,
    auto_applied = warmup_stats.learnings_auto_applied,
    duration_ms = warmup_stats.duration_ms,
    "Agent learning warmup complete"
);

// 2. Create event emitter + spawn drain task
let (event_tx, event_rx) = tokio::sync::mpsc::channel(1000);
let emitter = AgentEventEmitter::new(event_tx);
tokio::spawn(spawn_agent_drain_task(event_rx, pool.clone()));

// 3. Create handlers with learned data
let handlers = ToolHandlers::with_semantic_search(
    pool.clone(),
    sessions,
    cbu_sessions,
    learned_data,
).await?;
```

---

## Phase 4: DELETE `dsl_generate` and Replace with Structured Pipeline

### 4.1 Create Intent Pipeline

**File**: `rust/src/mcp/intent_pipeline.rs` (NEW)

```rust
//! Structured Intent Pipeline
//!
//! Extracts structured intent from natural language and assembles
//! deterministic DSL code. The LLM NEVER writes DSL syntax — it only
//! extracts argument values.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::agentic::create_llm_client;
use crate::dsl_v2::registry;
use crate::mcp::verb_search::{HybridVerbSearcher, VerbSearchResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredIntent {
    pub verb: String,
    pub arguments: Vec<IntentArgument>,
    pub confidence: f32,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentArgument {
    pub name: String,
    pub value: ArgumentValue,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArgumentValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Reference(String),
    Uuid(String),
    Unresolved(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResult {
    pub intent: StructuredIntent,
    pub verb_candidates: Vec<VerbSearchResult>,
    pub dsl: String,
    pub valid: bool,
    pub validation_error: Option<String>,
    pub unresolved_refs: Vec<UnresolvedRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedRef {
    pub param_name: String,
    pub search_value: String,
    pub entity_type: Option<String>,
}

pub struct IntentPipeline {
    verb_searcher: HybridVerbSearcher,
}

impl IntentPipeline {
    pub fn new(verb_searcher: HybridVerbSearcher) -> Self {
        Self { verb_searcher }
    }

    pub async fn process(
        &self,
        instruction: &str,
        domain_hint: Option<&str>,
    ) -> Result<PipelineResult> {
        // Step 1: Find verb candidates
        let candidates = self.verb_searcher
            .search(instruction, domain_hint, 5)
            .await?;

        if candidates.is_empty() {
            return Err(anyhow!("No matching verbs found for: {}", instruction));
        }

        let top_verb = &candidates[0].verb;
        
        // Step 2: Get verb signature
        let reg = registry();
        let parts: Vec<&str> = top_verb.splitn(2, '.').collect();
        let verb_def = reg.get_verb(parts[0], parts[1])
            .ok_or_else(|| anyhow!("Verb not in registry: {}", top_verb))?;

        // Step 3: Extract arguments via LLM (structured output only)
        let intent = self.extract_arguments(
            instruction,
            top_verb,
            verb_def,
            candidates[0].score,
        ).await?;

        // Step 4: Assemble DSL deterministically
        let (dsl, unresolved) = self.assemble_dsl(&intent)?;

        // Step 5: Validate
        let (valid, validation_error) = self.validate_dsl(&dsl);

        Ok(PipelineResult {
            intent,
            verb_candidates: candidates,
            dsl,
            valid,
            validation_error,
            unresolved_refs: unresolved,
        })
    }

    async fn extract_arguments(
        &self,
        instruction: &str,
        verb: &str,
        verb_def: &crate::dsl_v2::VerbDefinition,
        verb_confidence: f32,
    ) -> Result<StructuredIntent> {
        let llm = create_llm_client()?;

        let params_desc: Vec<String> = verb_def.args.iter().map(|p| {
            let req = if p.required { "REQUIRED" } else { "optional" };
            format!("- {}: {} ({}) - {}", p.name, p.arg_type, req, 
                p.description.as_deref().unwrap_or(""))
        }).collect();

        let system_prompt = format!(r#"You are an argument extractor for a DSL system.

Given a natural language instruction, extract argument values for the verb: {verb}

VERB PARAMETERS:
{params}

RULES:
1. Extract ONLY the values mentioned - do not invent data
2. For entity references (people, companies, CBUs), extract the name as given
3. For dates, use ISO format (YYYY-MM-DD)
4. For enums, match to closest valid value
5. If a required parameter cannot be extracted, set value to null
6. Do NOT write DSL syntax - only extract values

Respond with ONLY valid JSON:
{{
  "arguments": [
    {{"name": "param_name", "value": "extracted_value"}},
    ...
  ],
  "notes": ["any extraction notes"]
}}"#,
            verb = verb,
            params = params_desc.join("\n"),
        );

        let response = llm.chat(&system_prompt, instruction).await?;
        
        let parsed: Value = serde_json::from_str(response.trim())
            .map_err(|e| anyhow!("LLM returned invalid JSON: {}", e))?;

        let mut arguments = Vec::new();
        if let Some(args) = parsed["arguments"].as_array() {
            for arg in args {
                let name = arg["name"].as_str().unwrap_or_default().to_string();
                let value = match &arg["value"] {
                    Value::String(s) => ArgumentValue::Unresolved(s.clone()),
                    Value::Number(n) => ArgumentValue::Number(n.as_f64().unwrap_or(0.0)),
                    Value::Bool(b) => ArgumentValue::Boolean(*b),
                    Value::Null => continue,
                    _ => ArgumentValue::Unresolved(arg["value"].to_string()),
                };
                arguments.push(IntentArgument { name, value, resolved: false });
            }
        }

        let notes: Vec<String> = parsed["notes"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        Ok(StructuredIntent {
            verb: verb.to_string(),
            arguments,
            confidence: verb_confidence,
            notes,
        })
    }

    fn assemble_dsl(&self, intent: &StructuredIntent) -> Result<(String, Vec<UnresolvedRef>)> {
        let mut dsl = format!("({}", intent.verb);
        let mut unresolved = Vec::new();

        for arg in &intent.arguments {
            let value_str = match &arg.value {
                ArgumentValue::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
                ArgumentValue::Number(n) => n.to_string(),
                ArgumentValue::Boolean(b) => b.to_string(),
                ArgumentValue::Reference(r) => format!("@{}", r),
                ArgumentValue::Uuid(u) => format!("\"{}\"", u),
                ArgumentValue::Unresolved(u) => {
                    unresolved.push(UnresolvedRef {
                        param_name: arg.name.clone(),
                        search_value: u.clone(),
                        entity_type: None,
                    });
                    format!("\"{}\"", u)
                }
            };
            dsl.push_str(&format!(" :{} {}", arg.name, value_str));
        }

        dsl.push(')');
        Ok((dsl, unresolved))
    }

    fn validate_dsl(&self, dsl: &str) -> (bool, Option<String>) {
        use crate::dsl_v2::{parse_program, compile};
        
        match parse_program(dsl) {
            Ok(ast) => match compile(&ast) {
                Ok(_) => (true, None),
                Err(e) => (false, Some(format!("Compile error: {:?}", e))),
            },
            Err(e) => (false, Some(format!("Parse error: {:?}", e))),
        }
    }
}
```

### 4.2 DELETE Old `dsl_generate` and Replace

**File**: `rust/src/mcp/tools.rs`

**DELETE** the entire old `dsl_generate` tool definition (lines ~207-225).

**ADD** the new tool definition in its place:

```rust
Tool {
    name: "dsl_generate".into(),
    description: r#"Generate DSL from natural language using structured intent extraction.

Pipeline:
1. verb_search finds candidate verbs (learned → phrase → semantic)
2. LLM extracts structured arguments (JSON only, never DSL syntax)
3. Arguments assembled into DSL deterministically
4. DSL validated before return

This is RELIABLE because the LLM only extracts argument values - 
it does NOT write DSL syntax. The verb and syntax are determined
by semantic search and deterministic assembly.

Returns:
- intent: Extracted structured intent with verb and arguments
- verb_candidates: Verbs considered (ranked by confidence)
- dsl: Generated DSL code
- valid: Whether DSL passed validation
- unresolved_refs: Entity references needing resolution via dsl_lookup

If unresolved_refs is non-empty, use dsl_lookup to resolve entity names
to UUIDs, then call dsl_execute with the resolved DSL."#.into(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "instruction": {
                "type": "string",
                "description": "Natural language description (e.g., 'Add John Smith as director of Apex Fund')"
            },
            "context": {
                "type": "object",
                "description": "Optional context hints",
                "properties": {
                    "cbu_id": { "type": "string", "format": "uuid" },
                    "domain": { "type": "string" }
                }
            },
            "execute": {
                "type": "boolean",
                "default": false,
                "description": "If true and valid with no unresolved refs, execute immediately"
            }
        },
        "required": ["instruction"]
    }),
},
```

### 4.3 DELETE Old Handler and Replace

**File**: `rust/src/mcp/handlers/core.rs`

**DELETE** the entire old `dsl_generate` method (lines ~1004-1097).

**REPLACE** with:

```rust
/// Generate DSL using structured intent pipeline
///
/// LLM extracts arguments only - never writes DSL syntax.
/// Verb selection is done by semantic search.
/// DSL assembly is deterministic.
async fn dsl_generate(&self, args: Value) -> Result<Value> {
    use crate::mcp::intent_pipeline::IntentPipeline;

    let instruction = args["instruction"]
        .as_str()
        .ok_or_else(|| anyhow!("instruction required"))?;
    let domain = args["context"]["domain"].as_str();
    let execute = args["execute"].as_bool().unwrap_or(false);

    let searcher = self.verb_searcher.as_ref()
        .ok_or_else(|| anyhow!("Semantic search not initialized"))?;
    
    let pipeline = IntentPipeline::new(searcher.clone());
    let result = pipeline.process(instruction, domain).await?;

    // If valid, no unresolved refs, and execute requested → run it
    if result.valid && execute && result.unresolved_refs.is_empty() {
        let exec_result = self.dsl_execute(json!({
            "source": result.dsl,
            "intent": instruction
        })).await?;
        
        return Ok(json!({
            "intent": result.intent,
            "verb_candidates": result.verb_candidates,
            "dsl": result.dsl,
            "valid": result.valid,
            "executed": true,
            "execution_result": exec_result
        }));
    }

    Ok(json!({
        "intent": result.intent,
        "verb_candidates": result.verb_candidates,
        "dsl": result.dsl,
        "valid": result.valid,
        "validation_error": result.validation_error,
        "unresolved_refs": result.unresolved_refs,
        "next_steps": if !result.unresolved_refs.is_empty() {
            "Use dsl_lookup to resolve entity references, then call dsl_execute"
        } else if !result.valid {
            "Review validation error and adjust instruction"
        } else {
            "Review DSL and call dsl_execute when ready"
        }
    }))
}
```

### 4.4 Register Intent Pipeline Module

**File**: `rust/src/mcp/mod.rs`

```rust
pub mod intent_pipeline;
```

---

## Phase 5: Cleanup Checklist

After implementation, verify complete removal:

- [ ] **DELETE** old `dsl_generate` tool definition in `tools.rs` (the one mentioning "LLM writes DSL")
- [ ] **DELETE** old `dsl_generate` handler in `core.rs` (the one with `vocab.join("\n")`)
- [ ] **DELETE** dispatch arm if not replaced: `"dsl_generate" => ...`
- [ ] **VERIFY** no `_v2` suffixes anywhere
- [ ] **VERIFY** no "deprecated" warnings in tool descriptions
- [ ] **VERIFY** `cargo check` passes
- [ ] **VERIFY** `cargo clippy` passes

**Dead code that should be removed if no other users:**
- `reg.all_verbs().take(50)` pattern (was only for vocab prompt)

---

## The Flywheel

```
Day 1: YAML invocation_phrases + semantic embeddings (cold start)
       ↓
       verb_search gets ~70-80% hit rate
       ↓
User corrects: "No, I meant X"
       ↓
Claude calls intent_feedback (see companion TODO)
       ↓
Learning candidate created in agent.learning_candidates
       ↓
After 3 occurrences OR explicit approval → applied to agent.invocation_phrases
       ↓
warmup loads into LearnedData.invocation_phrases
       ↓
verb_search checks learned FIRST (hot path, exact match)
       ↓
Day 30: 90%+ hit rate, corrections rare
```

**Key insight**: Learned phrases bypass semantic similarity entirely. They're exact matches from real user vocabulary → your verbs. The embeddings are just the fallback for novel phrases.

---

## Testing Checklist

### Unit Tests

- [ ] `HybridVerbSearcher::search` returns learned phrases first
- [ ] `HybridVerbSearcher::search` respects domain filter
- [ ] `IntentPipeline::extract_arguments` handles all param types
- [ ] `IntentPipeline::assemble_dsl` produces valid syntax
- [ ] `IntentPipeline::validate_dsl` catches common errors

### Integration Tests

- [ ] `verb_search` returns results for known phrases
- [ ] `verb_search` falls back to semantic when no phrase match
- [ ] `dsl_generate` produces valid DSL for simple instructions
- [ ] `dsl_generate` identifies unresolved entity references
- [ ] Learning warmup loads data correctly at startup

### End-to-End Tests

- [ ] Claude uses `verb_search` → `dsl_signature` → `dsl_execute` flow
- [ ] Claude uses `dsl_generate` → `dsl_lookup` → `dsl_execute` flow
- [ ] Learned phrases override phrase index correctly
- [ ] **No tool shows deprecated warnings or `_v2` suffixes**

---

## Files Modified Summary

| File | Action |
|------|--------|
| `rust/src/mcp/mod.rs` | ADD `verb_search`, `intent_pipeline` modules |
| `rust/src/mcp/tools.rs` | ADD `verb_search`; **DELETE+REPLACE** `dsl_generate`; fix schema drift |
| `rust/src/mcp/handlers/core.rs` | ADD `verb_search` handler; **DELETE+REPLACE** `dsl_generate` handler |
| `rust/src/mcp/verb_search.rs` | NEW file |
| `rust/src/mcp/intent_pipeline.rs` | NEW file |
| `rust/src/mcp/server.rs` | Wire learning warmup + drain task |

---

## Success Criteria

1. **`verb_search`** returns learned phrases with score 1.0 when available
2. **`dsl_generate`** produces valid DSL for 95% of well-formed instructions
3. **No tool pollution** — only ONE `dsl_generate` tool exists
4. **LLM never writes DSL** — all DSL assembly is deterministic
5. **Schema drift** eliminated — all gateway-supported types in `dsl_lookup`
6. **Learning loop** wired — warmup loads, drain persists, flywheel spins
7. **Clean namespace** — no `_v2` suffixes, no deprecated warnings
