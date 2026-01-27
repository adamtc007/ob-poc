# Claude Code Task: Unified Intent Pipeline (3-Lane Router)

## Prerequisites

**REQUIRES**: `CLAUDE_CODE_TASK_complete_schema_cleanup.md` completed first.

This task builds the router that USES the VerbSpec schemas. Without schemas, Lane 1 (S-Expr) won't work.

---

## Architecture

```
User Input
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  Intent Router                                                   │
│  ┌─────────────────┐  ┌────────────────┐  ┌──────────────────┐ │
│  │ starts with '(' │  │ 2-4 tokens,    │  │ sentence-like,   │ │
│  │ → Lane 1        │  │ command-like   │  │ questions, NL    │ │
│  │ S-Expr Parser   │  │ → Lane 2       │  │ → Lane 3         │ │
│  │                 │  │ Command Norm.  │  │ Semantic/BGE     │ │
│  └────────┬────────┘  └───────┬────────┘  └────────┬─────────┘ │
└───────────┼───────────────────┼─────────────────────┼───────────┘
            │                   │                     │
            ▼                   ▼                     ▼
┌───────────────────┐  ┌─────────────────┐  ┌──────────────────────┐
│ Schema-Guided     │  │ Synonym Maps    │  │ Entity Masking       │
│ Parse + Validate  │  │ + Reordering    │  │ + N-gram BGE         │
│ (from task 1)     │  │ + Domain Hints  │  │ + Aggregation        │
└────────┬──────────┘  └────────┬────────┘  └───────────┬──────────┘
         │                      │                       │
         └──────────────────────┼───────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│  IntentResolution                                               │
│  - Resolved(verb_fqn, canonical_ast, entities, confidence)      │
│  - Ambiguous(candidates, skeletons, reason)                     │
│  - Incomplete(expected, completions, cursor)                    │
│  - NotFound(input, suggestions)                                 │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│  Executor (existing DSL pipeline - unchanged)                   │
└─────────────────────────────────────────────────────────────────┘
```

---

## Lane 1: S-Expression Parser (Primary, Deterministic)

**Trigger**: Input starts with `(`

**Already implemented in schema cleanup task:**
- Tokenizer with spans
- Schema-guided parser
- Canonicalizer
- Validator
- Feedback (diagnostics, completions)

**This task adds:**
- Integration with intent router
- Conversion to executor-compatible AST

```rust
// In intent_router.rs

async fn resolve_sexpr(input: &str, registry: &VerbRegistry) -> IntentResolution {
    match schema::parse(input, registry) {
        ParseResult::Ok((ast, feedback)) => {
            let canonical = schema::canonicalize(&ast, registry)?;
            IntentResolution::Resolved(ResolvedIntent {
                verb_fqn: canonical.verb.clone(),
                ast: canonical,
                confidence: 1.0,  // S-expr is deterministic
                source: IntentSource::SExpr,
                feedback,
            })
        }
        ParseResult::Incomplete { partial, expected, cursor } => {
            IntentResolution::Incomplete(IncompleteIntent {
                partial,
                expected,
                cursor,
                completions: generate_completions(&partial, &expected, registry),
            })
        }
        ParseResult::Err { message, span, expected } => {
            IntentResolution::Error(IntentError {
                message,
                span,
                suggestions: generate_suggestions(&expected, registry),
            })
        }
    }
}
```

---

## Lane 2: Command Normalizer (Short Commands)

**Trigger**: 2-4 tokens, no stopwords, command-like structure

**Purpose**: Handle `drill down`, `load allianz`, `trace ownership` without s-expr syntax

### 2.1 Detection

```rust
fn is_command_like(input: &str) -> bool {
    let input = input.trim();
    
    // S-expression goes to Lane 1
    if input.starts_with('(') {
        return false;
    }
    
    let tokens: Vec<_> = input.split_whitespace().collect();
    
    // 2-4 tokens with no stopwords = command
    if tokens.len() >= 2 && tokens.len() <= 4 && !contains_stopwords(&tokens) {
        return true;
    }
    
    // 4-6 tokens but mostly keywords we recognize
    if tokens.len() <= 6 {
        let keyword_ratio = count_known_keywords(&tokens) as f32 / tokens.len() as f32;
        if keyword_ratio >= 0.6 {
            return true;
        }
    }
    
    false
}

const STOPWORDS: &[&str] = &[
    "the", "a", "an", "of", "for", "to", "in", "on", "at", "by",
    "me", "my", "this", "that", "please", "can", "you", "i", "want",
    "show", "give", "tell", "what", "who", "how", "is", "are",
];
```

### 2.2 Synonym Maps

```rust
lazy_static! {
    /// Verb synonyms: input → canonical verb root
    static ref VERB_SYNONYMS: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        
        // Navigation
        m.insert("drill", "drill");
        m.insert("dive", "drill");
        m.insert("expand", "drill");
        m.insert("deeper", "drill");
        m.insert("into", "drill");
        m.insert("zoom-in", "drill");
        
        m.insert("surface", "surface");
        m.insert("back", "surface");
        m.insert("up", "surface");
        m.insert("parent", "surface");
        m.insert("out", "surface");
        m.insert("zoom-out", "surface");
        
        m.insert("trace", "trace");
        m.insert("follow", "trace");
        m.insert("track", "trace");
        m.insert("path", "trace");
        
        // CRUD
        m.insert("create", "create");
        m.insert("add", "create");
        m.insert("new", "create");
        m.insert("make", "create");
        
        m.insert("list", "list");
        m.insert("show", "list");
        m.insert("display", "list");
        m.insert("get", "get");
        m.insert("fetch", "get");
        
        m.insert("update", "update");
        m.insert("edit", "update");
        m.insert("modify", "update");
        
        m.insert("delete", "delete");
        m.insert("remove", "delete");
        
        // Session
        m.insert("load", "load");
        m.insert("open", "load");
        m.insert("switch", "load");
        m.insert("select", "select");
        
        // Compute
        m.insert("compute", "compute");
        m.insert("calculate", "compute");
        m.insert("derive", "compute");
        
        m
    };

    /// Domain hints: nouns that suggest specific verb domains
    static ref DOMAIN_HINTS: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        
        m.insert("ownership", "ownership");
        m.insert("owners", "ownership");
        m.insert("owned", "ownership");
        
        m.insert("ubo", "ubo");
        m.insert("ubos", "ubo");
        m.insert("beneficial", "ubo");
        m.insert("chain", "ubo");
        m.insert("chains", "ubo");
        
        m.insert("control", "control");
        m.insert("controller", "control");
        m.insert("voting", "control");
        
        m.insert("fund", "fund");
        m.insert("funds", "fund");
        m.insert("subfund", "fund");
        m.insert("umbrella", "fund");
        
        m.insert("investor", "registry");
        m.insert("investors", "registry");
        m.insert("holding", "registry");
        
        m.insert("graph", "graph");
        m.insert("tree", "graph");
        m.insert("structure", "graph");
        
        m
    };

    /// Entity-type hints: prevent collision (these are NOT verb domains)
    static ref ENTITY_TYPE_HINTS: HashSet<&'static str> = {
        let mut s = HashSet::new();
        s.insert("fund");
        s.insert("company");
        s.insert("trust");
        s.insert("person");
        s.insert("partnership");
        s.insert("sicav");
        s.insert("icav");
        s.insert("llc");
        s.insert("ltd");
        s
    };
}
```

### 2.3 Noun-Verb Reordering

```rust
fn normalize_command(input: &str) -> NormalizedCommand {
    let mut tokens: Vec<String> = input
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.trim_matches(|c: char| c.is_ascii_punctuation()))
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    
    // Handle noun-first: "fund load" → "load fund"
    if tokens.len() >= 2 {
        let first_is_noun = DOMAIN_HINTS.contains_key(tokens[0].as_str())
            || ENTITY_TYPE_HINTS.contains(&tokens[0].as_str());
        let second_is_verb = VERB_SYNONYMS.contains_key(tokens[1].as_str());
        
        if first_is_noun && second_is_verb {
            tokens.swap(0, 1);
        }
    }
    
    // Normalize verb to canonical
    if let Some(first) = tokens.first_mut() {
        if let Some(canonical) = VERB_SYNONYMS.get(first.as_str()) {
            *first = canonical.to_string();
        }
    }
    
    NormalizedCommand { tokens, original: input.to_string() }
}
```

### 2.4 Candidate Generation

```rust
fn generate_candidates(
    normalized: &NormalizedCommand,
    registry: &VerbRegistry,
) -> Vec<IntentCandidate> {
    let tokens = &normalized.tokens;
    if tokens.is_empty() {
        return vec![];
    }
    
    let verb_token = &tokens[0];
    let noun_tokens: Vec<&str> = tokens[1..].iter().map(|s| s.as_str()).collect();
    
    // Collect domain hints
    let domain_hints: Vec<&str> = noun_tokens.iter()
        .filter_map(|t| DOMAIN_HINTS.get(t).copied())
        .collect();
    
    let mut candidates = Vec::new();
    
    // Strategy 1: Direct domain.verb match
    for domain in &domain_hints {
        let fqn = format!("{}.{}", domain, verb_token);
        if let Some(spec) = registry.get(&fqn) {
            candidates.push(IntentCandidate {
                verb_fqn: fqn,
                score: 0.95,
                match_type: MatchType::DirectDomainVerb,
                spec: spec.clone(),
            });
        }
    }
    
    // Strategy 2: Navigation verbs → view.*
    if is_navigation_verb(verb_token) {
        let fqn = format!("view.{}", verb_token);
        if let Some(spec) = registry.get(&fqn) {
            candidates.push(IntentCandidate {
                verb_fqn: fqn,
                score: 0.90,
                match_type: MatchType::NavigationVerb,
                spec: spec.clone(),
            });
        }
    }
    
    // Strategy 3: Session verbs with entity-like nouns
    if matches!(verb_token.as_str(), "load" | "open" | "switch" | "select") {
        if noun_tokens.iter().any(|t| looks_like_entity_name(t)) {
            if let Some(spec) = registry.get("session.load-client-group") {
                candidates.push(IntentCandidate {
                    verb_fqn: "session.load-client-group".to_string(),
                    score: 0.85,
                    match_type: MatchType::SessionLoad,
                    spec: spec.clone(),
                });
            }
        }
    }
    
    // Strategy 4: Fuzzy match on registry
    if candidates.is_empty() {
        let pattern = tokens.join(" ");
        candidates.extend(registry.fuzzy_search(&pattern, 5));
    }
    
    // Dedupe and sort
    candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    candidates.dedup_by(|a, b| a.verb_fqn == b.verb_fqn);
    
    candidates
}
```

### 2.5 Confidence Evaluation

```rust
fn evaluate_command_confidence(candidates: &[IntentCandidate]) -> CommandResult {
    if candidates.is_empty() {
        return CommandResult::Unknown;
    }
    
    let top = &candidates[0];
    
    // Single clear winner with high score
    if candidates.len() == 1 && top.score >= 0.85 {
        return CommandResult::Confident(top.clone());
    }
    
    // Big gap between top and second
    if candidates.len() >= 2 {
        let gap = top.score - candidates[1].score;
        if top.score >= 0.80 && gap >= 0.15 {
            return CommandResult::Confident(top.clone());
        }
    }
    
    // Multiple plausible candidates
    if top.score >= 0.60 {
        return CommandResult::Ambiguous(candidates[..3.min(candidates.len())].to_vec());
    }
    
    // Low confidence - fall through to semantic
    CommandResult::LowConfidence(candidates.to_vec())
}
```

---

## Lane 3: Semantic Fallback (Natural Language)

**Trigger**: Sentence-like input, questions, or when Lane 2 returns low confidence

**Purpose**: Handle "show me the ownership structure for Allianz" via BGE

### 3.1 Entity Masking

```rust
async fn mask_entities(
    input: &str,
    entity_gateway: &EntityGateway,
) -> MaskedInput {
    let mut masked = input.to_string();
    let mut entities = Vec::new();
    
    // 1. Quoted strings (high confidence)
    for cap in QUOTED_REGEX.captures_iter(input) {
        let span = cap.get(1).unwrap();
        entities.push(DetectedEntity {
            text: span.as_str().to_string(),
            span: (span.start(), span.end()),
            entity_type: None,
            confidence: 0.9,
        });
    }
    
    // 2. Title case sequences (proper nouns)
    for candidate in extract_title_case_sequences(input) {
        if let Some(entity) = entity_gateway.fuzzy_match(&candidate, 0.8).await {
            entities.push(DetectedEntity {
                text: candidate,
                span: find_span(input, &candidate),
                entity_type: Some(entity.entity_type.clone()),
                confidence: 0.95,
            });
        }
    }
    
    // 3. Known collision words in entity-like context
    // "load the Allianz Fund" - "Fund" is collision word, mask it
    
    // Sort by position descending, replace from end
    entities.sort_by(|a, b| b.span.0.cmp(&a.span.0));
    for e in &entities {
        let replacement = match &e.entity_type {
            Some(t) => format!("<ENTITY:{}>", t),
            None => "<ENTITY>".to_string(),
        };
        masked.replace_range(e.span.0..e.span.1, &replacement);
    }
    
    MaskedInput { masked_text: masked, entities, original: input.to_string() }
}
```

### 3.2 N-gram Extraction + Batch Embedding

```rust
async fn semantic_search(
    masked: &MaskedInput,
    embedder: &CandleEmbedder,
    pool: &PgPool,
) -> Vec<VerbScore> {
    // Extract n-grams (2-5 tokens, skip 1-grams)
    let ngrams = extract_ngrams(&masked.masked_text, 2, 5);
    
    if ngrams.is_empty() {
        return vec![];
    }
    
    // Batch embed all n-grams
    let embeddings = embedder.embed_batch(&ngrams).await?;
    
    // Aggregate scores by verb
    let mut verb_evidence: HashMap<String, VerbEvidence> = HashMap::new();
    
    for (ngram, embedding) in ngrams.iter().zip(embeddings.iter()) {
        let n_tokens = ngram.split_whitespace().count();
        let len_weight = (1.0 + 0.1 * (n_tokens as f32 - 2.0)).min(1.4);
        
        // Vector search
        let matches = vector_search(pool, embedding, 10).await?;
        
        for m in matches {
            let weighted_score = m.similarity * len_weight;
            
            verb_evidence
                .entry(m.verb_fqn.clone())
                .and_modify(|e| {
                    if weighted_score > e.max_score {
                        e.max_score = weighted_score;
                        e.best_ngram = ngram.clone();
                    }
                })
                .or_insert(VerbEvidence {
                    verb_fqn: m.verb_fqn,
                    max_score: weighted_score,
                    best_ngram: ngram.clone(),
                });
        }
    }
    
    let mut results: Vec<_> = verb_evidence.into_values()
        .map(|e| VerbScore {
            verb_fqn: e.verb_fqn,
            score: e.max_score,
            evidence: e,
        })
        .collect();
    
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    results
}
```

### 3.3 Semantic Thresholds

```rust
const SEM_ACCEPT: f32 = 0.70;    // Accept if top >= this
const SEM_GAP: f32 = 0.10;       // Accept if gap >= this
const SEM_MINIMUM: f32 = 0.55;   // Below this = not found
```

---

## Unified Router (`rust/src/mcp/intent_router.rs`)

```rust
pub async fn resolve_intent(
    raw_input: &str,
    ctx: &IntentContext,
) -> Result<IntentResolution> {
    let input = raw_input.trim();
    
    // ========================================
    // Lane 1: S-Expression (deterministic)
    // ========================================
    if input.starts_with('(') {
        return resolve_sexpr(input, &ctx.verb_registry).await;
    }
    
    // ========================================
    // Lane 2: Command Normalizer
    // ========================================
    if is_command_like(input) {
        let normalized = normalize_command(input);
        let candidates = generate_candidates(&normalized, &ctx.verb_registry);
        
        match evaluate_command_confidence(&candidates) {
            CommandResult::Confident(intent) => {
                // Generate canonical s-expr
                let sexpr = generate_sexpr(&intent, &normalized);
                return Ok(IntentResolution::Resolved(ResolvedIntent {
                    verb_fqn: intent.verb_fqn,
                    sexpr,
                    confidence: intent.score,
                    source: IntentSource::CommandNormalizer,
                }));
            }
            CommandResult::Ambiguous(candidates) => {
                return Ok(IntentResolution::Ambiguous(AmbiguousIntent {
                    candidates,
                    skeletons: generate_skeletons(&candidates, input),
                    reason: AmbiguityReason::CloseScores,
                    source: IntentSource::CommandNormalizer,
                }));
            }
            CommandResult::LowConfidence(_) | CommandResult::Unknown => {
                // Fall through to Lane 3
            }
        }
    }
    
    // ========================================
    // Lane 3: Semantic Fallback (BGE)
    // ========================================
    let masked = mask_entities(input, &ctx.entity_gateway).await;
    let results = semantic_search(&masked, &ctx.embedder, &ctx.pool).await?;
    
    if results.is_empty() {
        return Ok(IntentResolution::NotFound(NotFoundIntent {
            input: input.to_string(),
            suggestion: "Try: (verb :arg value)".to_string(),
        }));
    }
    
    let top = &results[0];
    let gap = if results.len() > 1 {
        top.score - results[1].score
    } else {
        1.0
    };
    
    if top.score >= SEM_ACCEPT && gap >= SEM_GAP {
        let sexpr = generate_sexpr_from_semantic(&top, &masked);
        Ok(IntentResolution::Resolved(ResolvedIntent {
            verb_fqn: top.verb_fqn.clone(),
            sexpr,
            confidence: top.score,
            source: IntentSource::Semantic,
        }))
    } else if top.score >= SEM_MINIMUM {
        Ok(IntentResolution::Ambiguous(AmbiguousIntent {
            candidates: results[..3.min(results.len())].to_vec(),
            skeletons: generate_skeletons_from_semantic(&results, input),
            reason: AmbiguityReason::CloseScores,
            source: IntentSource::Semantic,
        }))
    } else {
        Ok(IntentResolution::NotFound(NotFoundIntent {
            input: input.to_string(),
            suggestion: format!("Closest match: {} (score: {:.2})", top.verb_fqn, top.score),
        }))
    }
}
```

---

## Files to Create

```
rust/src/mcp/
├── intent_router.rs           # Main router (3 lanes)
├── command_normalizer.rs      # Lane 2: synonym maps, reordering
├── semantic_fallback.rs       # Lane 3: entity masking, BGE
├── intent_types.rs            # Shared types
└── synonym_maps.rs            # Verb/domain/entity synonyms
```

---

## Test Cases

```yaml
# Lane 1: S-Expression
- input: "(view.drill :entity \"Allianz\")"
  expected_lane: sexpr
  expected_verb: view.drill

- input: "(drill \"Allianz\")"
  expected_lane: sexpr
  expected_verb: view.drill  # alias resolved

# Lane 2: Command Normalizer
- input: "drill down"
  expected_lane: command
  expected_verb: view.drill

- input: "load allianz"
  expected_lane: command
  expected_verb: session.load-client-group  # NOT fund.*

- input: "fund load"
  expected_lane: command
  expected_verb: session.load-client-group  # reordered

- input: "trace ownership"
  expected_lane: command
  expected_verb: ownership.trace-chain

# Lane 3: Semantic
- input: "show me the ownership structure for this entity"
  expected_lane: semantic
  expected_verb: ownership.* or graph.*

- input: "who are the ultimate beneficial owners"
  expected_lane: semantic
  expected_verb: ubo.list-ubos
```

---

## Success Criteria

| Lane | Input Type | Target Accuracy |
|------|------------|-----------------|
| 1 | S-Expression | 100% (deterministic) |
| 2 | Short command | 85% top-1 |
| 3 | Natural language | 75% top-1, 90% top-3 |

---

## Execution Order

1. Implement `intent_types.rs` (shared types)
2. Implement `synonym_maps.rs` (extracted from analysis)
3. Implement `command_normalizer.rs` (Lane 2)
4. Implement `semantic_fallback.rs` (Lane 3)
5. Implement `intent_router.rs` (orchestration)
6. Write tests
7. Integration with existing pipeline
