# 072: Lexicon Service Implementation Plan

**Status:** Implementation Plan
**Date:** 2026-02-03
**Author:** Claude
**Related:** 
- `docs/LEXICON_PARSER_DESIGN.md` (original lexicon architecture)
- `rust/crates/ob-agentic/src/lexicon/` (existing lexicon tokenizer)
- `rust/src/mcp/verb_search.rs` (HybridVerbSearcher)
- `rust/src/mcp/intent_pipeline.rs` (IntentPipeline)

---

## Executive Summary

This document consolidates the proposed Lexicon Service architecture into a phased implementation plan that integrates with the existing `HybridVerbSearcher` pipeline. The goal is to add a **fast lexical lane** before semantic search, improving accuracy and latency while enabling better error messages and learning signals.

**Key Insight:** The existing `ob-agentic/lexicon` module is a **parallel system** designed for OTC onboarding (tokenizer → nom parser → IntentAst → DSL). The new Lexicon Service proposed here is different: it's an **in-memory keyword lookup layer** that feeds into the existing `HybridVerbSearcher` pipeline, not a replacement.

---

## Problem Statement

Current verb search relies heavily on semantic embeddings (BGE), which:
1. **Latency:** 15-30ms embedding computation per query
2. **Threshold Sensitivity:** Scores cluster at 0.55-0.75, making disambiguation hard
3. **No Structural Signal:** "load the allianz book" treats "load", "allianz", "book" as bag-of-words
4. **Hard to Debug:** Why did "spin up a fund" match `cbu.create` with 0.72? Evidence is opaque

The Lexicon Service adds a **lexical lane** that:
1. Recognizes known vocabulary (verb synonyms, entity types, domain keywords)
2. Provides structural signal (verb phrase vs entity phrase vs modifier)
3. Generates explainable evidence ("matched 'spin up' as CREATE verb synonym")
4. Enables faster exact matching before falling back to semantic

---

## Architecture: Lexical Lane in HybridVerbSearcher

```
User Input: "spin up a fund for allianz"
    │
    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  LEXICON SERVICE (NEW - in-memory, <1ms)                                    │
│                                                                              │
│  1. Segment input into tokens                                               │
│  2. Classify each token against lexicon:                                    │
│     - "spin up" → VerbSynonym(CREATE)                                       │
│     - "a" → Article (absorbed)                                              │
│     - "fund" → EntityType(FUND)                                             │
│     - "for" → Preposition                                                   │
│     - "allianz" → Unresolved (capitalized, likely entity name)             │
│                                                                              │
│  3. Generate verb candidates from structured match:                         │
│     - VerbSynonym(CREATE) + EntityType(FUND) → cbu.create (0.95)           │
│     - VerbSynonym(CREATE) + EntityType(FUND) → fund.create (0.90)          │
│                                                                              │
│  Output: Vec<VerbSearchResult> with VerbSearchSource::Lexical               │
└─────────────────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  EXISTING PIPES (macros, learned, semantic, phonetic)                       │
│  - All pipes still run (no early exit)                                      │
│  - normalize_candidates() merges results, best score wins                   │
└─────────────────────────────────────────────────────────────────────────────┘
    │
    ▼
Final VerbSearchResult with merged evidence from all channels
```

---

## Data Model

### LexiconSnapshot (In-Memory)

```rust
/// Compiled, immutable lexicon snapshot for fast lookup.
/// Loaded once at startup, potentially hot-reloadable via Arc swap.
pub struct LexiconSnapshot {
    /// Verb action → canonical verb class (CREATE, UPDATE, DELETE, QUERY, LINK)
    pub verb_synonyms: HashMap<String, VerbClass>,
    
    /// Entity type keywords → EntityTypeCode
    pub entity_types: HashMap<String, EntityTypeCode>,
    
    /// Domain keywords → Domain (session, cbu, kyc, trading-profile, etc.)
    pub domain_keywords: HashMap<String, String>,
    
    /// Prepositions (absorbed but tracked for structure)
    pub prepositions: HashSet<String>,
    
    /// Articles (absorbed)
    pub articles: HashSet<String>,
    
    /// Role keywords → RoleCode
    pub roles: HashMap<String, String>,
    
    /// Instrument keywords → InstrumentClass
    pub instruments: HashMap<String, String>,
    
    /// Version/hash for cache invalidation
    pub version: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VerbClass {
    Create,   // add, create, new, spin up, set up, register
    Update,   // set, update, modify, change, configure
    Delete,   // delete, remove, cancel, terminate
    Query,    // list, show, get, find, who, what
    Link,     // assign, connect, add to, link
    Navigate, // load, focus, drill, surface, zoom
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityTypeCode {
    Cbu,
    Fund,
    Entity,
    Person,
    Counterparty,
    TradingProfile,
    KycCase,
    // ... etc
}
```

### LexiconConfig (YAML Source)

```yaml
# rust/config/lexicon/lexicon.yaml

verb_synonyms:
  create:
    - create
    - add
    - new
    - spin up
    - set up
    - register
    - establish
    - onboard
  update:
    - update
    - set
    - modify
    - change
    - configure
    - edit
  delete:
    - delete
    - remove
    - cancel
    - terminate
    - drop
  query:
    - list
    - show
    - get
    - find
    - who
    - what
    - where
    - display
  link:
    - assign
    - connect
    - link
    - attach
    - add to
  navigate:
    - load
    - focus
    - drill
    - surface
    - zoom
    - open
    - work on
    - switch to

entity_types:
  cbu:
    - cbu
    - client business unit
    - structure
    - trading unit
  fund:
    - fund
    - investment fund
    - portfolio
    - vehicle
  entity:
    - entity
    - company
    - person
    - party
  counterparty:
    - counterparty
    - broker
    - dealer
  trading_profile:
    - trading profile
    - profile
    - mandate
  kyc_case:
    - kyc case
    - case
    - review

domain_keywords:
  session:
    - session
    - book
    - galaxy
    - cluster
    - scope
  cbu:
    - cbu
    - fund
    - structure
    - mandate
  kyc:
    - kyc
    - case
    - review
    - compliance
  trading:
    - trading
    - profile
    - universe
    - instrument
    - market

prepositions:
  - for
  - with
  - as
  - to
  - under
  - in
  - on
  - from

articles:
  - a
  - an
  - the
```

### VerbEvidence Extension

```rust
/// Evidence from lexical analysis (extends existing VerbEvidence)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LexicalEvidence {
    /// Verb class detected from synonyms
    pub verb_class: Option<VerbClass>,
    /// Entity type detected
    pub entity_type: Option<EntityTypeCode>,
    /// Domain inferred from keywords
    pub domain: Option<String>,
    /// Tokens that contributed to match
    pub matched_tokens: Vec<String>,
}
```

---

## Integration Points

### 1. HybridVerbSearcher: Add Lexical Pipe

```rust
// rust/src/mcp/verb_search.rs

impl HybridVerbSearcher {
    /// Add lexicon snapshot for lexical search
    pub fn with_lexicon(mut self, lexicon: Arc<LexiconSnapshot>) -> Self {
        self.lexicon = Some(lexicon);
        self
    }
    
    pub async fn search(...) -> Result<Vec<VerbSearchResult>> {
        let mut results = Vec::new();
        let mut seen_verbs: HashSet<String> = HashSet::new();
        
        // NEW: Pipe 0.5 - Lexical search (before macros, after normalization)
        // This runs BEFORE semantic embedding computation
        if let Some(lexicon) = &self.lexicon {
            let lexical_results = self.search_lexical(query, lexicon, limit);
            for result in lexical_results {
                if self.matches_domain(&result.verb, domain_filter)
                    && !seen_verbs.contains(&result.verb)
                {
                    seen_verbs.insert(result.verb.clone());
                    results.push(result);
                }
            }
        }
        
        // Existing pipes continue...
        // 0. Macros
        // 1. User learned exact
        // 2. Global learned exact
        // ... etc
    }
    
    fn search_lexical(
        &self,
        query: &str,
        lexicon: &LexiconSnapshot,
        limit: usize,
    ) -> Vec<VerbSearchResult> {
        // 1. Tokenize and classify
        let tokens = lexicon.tokenize(query);
        
        // 2. Extract structural signals
        let verb_class = tokens.iter().find_map(|t| t.verb_class());
        let entity_type = tokens.iter().find_map(|t| t.entity_type());
        let domain = tokens.iter().find_map(|t| t.domain());
        
        // 3. Generate verb candidates from structure
        self.generate_verb_candidates(verb_class, entity_type, domain, limit)
    }
}
```

### 2. Verb → (VerbClass, EntityType, Domain) Mapping

The lexicon needs a mapping from structural signals to concrete verbs:

```yaml
# rust/config/lexicon/verb_mapping.yaml

# Maps (verb_class, entity_type?) → candidate verbs with base scores
mappings:
  - verb_class: create
    entity_type: cbu
    verbs:
      - { verb: "cbu.create", score: 0.95 }
      - { verb: "cbu.ensure", score: 0.85 }
      
  - verb_class: create
    entity_type: fund
    verbs:
      - { verb: "cbu.create", score: 0.95 }  # fund is CBU subtype
      - { verb: "entity.create", score: 0.70 }
      
  - verb_class: navigate
    domain: session
    verbs:
      - { verb: "session.load-galaxy", score: 0.90 }
      - { verb: "session.load-cbu", score: 0.85 }
      - { verb: "session.load-cluster", score: 0.85 }
      
  - verb_class: query
    entity_type: cbu
    verbs:
      - { verb: "cbu.list", score: 0.95 }
      - { verb: "cbu.info", score: 0.80 }
```

### 3. Startup Loading

```rust
// rust/crates/ob-poc-web/src/main.rs

// Load lexicon during startup
let lexicon_config = LexiconConfig::load("config/lexicon/lexicon.yaml")?;
let verb_mapping = VerbMapping::load("config/lexicon/verb_mapping.yaml")?;
let lexicon_snapshot = Arc::new(LexiconSnapshot::compile(lexicon_config, verb_mapping)?);

// Pass to verb searcher
let verb_searcher = HybridVerbSearcher::new(verb_service, learned_data)
    .with_embedder(embedder)
    .with_macro_registry(macro_registry)
    .with_lexicon(lexicon_snapshot);  // NEW
```

---

## Phased Implementation

### Phase A0: Proof of Concept (2-3 hours)

**Goal:** Validate lexical search improves accuracy on existing test harness.

1. Create `rust/src/lexicon/mod.rs` with minimal types
2. Hardcode a small vocabulary (create/list/load + cbu/fund/entity)
3. Add `search_lexical()` to `HybridVerbSearcher`
4. Run verb search test harness, measure delta

**Success Criteria:**
- No regression on existing tests
- At least 5% improvement on hard cases (vague queries)

### Phase A: Core Types & Tokenizer (Half day)

1. `rust/src/lexicon/types.rs` - VerbClass, EntityTypeCode, Token
2. `rust/src/lexicon/snapshot.rs` - LexiconSnapshot with HashMap lookups
3. `rust/src/lexicon/tokenizer.rs` - Basic tokenizer (whitespace + phrase matching)
4. `rust/config/lexicon/lexicon.yaml` - Initial vocabulary
5. Unit tests for tokenization

### Phase B: Verb Mapping & Candidate Generation (Half day)

1. `rust/src/lexicon/mapping.rs` - VerbMapping from YAML
2. `rust/config/lexicon/verb_mapping.yaml` - Initial mappings
3. `generate_verb_candidates()` implementation
4. Integration test: tokenize → classify → generate candidates

### Phase C: HybridVerbSearcher Integration (Half day)

1. Add `with_lexicon()` builder method
2. Add lexical pipe in `search()` (before semantic embedding)
3. Add `VerbSearchSource::Lexical` variant
4. Merge lexical evidence with existing evidence
5. Integration tests with full pipeline

### Phase D: Startup & Hot Reload (2-3 hours)

1. Load lexicon at startup in `ob-poc-web`
2. Add `cargo x lexicon lint` command (validate YAML)
3. Add `cargo x lexicon bench` command (measure lookup latency)
4. Consider Arc swap pattern for hot reload (optional)

### Phase E: Test Harness & Tuning (Half day)

1. Add lexical-specific test scenarios
2. Tune vocabulary based on test harness failures
3. Add `OB_LEXICON_TRACE=1` debug logging
4. Document vocabulary governance

### Phase F: Learning Integration (Optional, 2-3 hours)

1. When user selects verb from disambiguation, extract lexical signals
2. Auto-suggest new vocabulary entries for review
3. Add `cargo x lexicon suggest` command

---

## File Structure

```
rust/
├── src/
│   └── lexicon/
│       ├── mod.rs           # Module exports
│       ├── types.rs         # VerbClass, EntityTypeCode, Token
│       ├── snapshot.rs      # LexiconSnapshot (compiled, in-memory)
│       ├── tokenizer.rs     # Tokenize input, classify tokens
│       ├── mapping.rs       # VerbMapping (verb_class + entity_type → verbs)
│       └── loader.rs        # Load from YAML
├── config/
│   └── lexicon/
│       ├── lexicon.yaml     # Vocabulary definitions
│       └── verb_mapping.yaml # Structural → verb mappings
└── xtask/
    └── src/
        └── lexicon.rs       # lint, bench, suggest commands
```

---

## Relationship to Existing ob-agentic/lexicon

The existing `ob-agentic/lexicon` module is a **different system**:

| Aspect | ob-agentic/lexicon | New Lexicon Service |
|--------|-------------------|---------------------|
| Purpose | OTC onboarding intent classification | Verb discovery vocabulary layer |
| Output | IntentAst (domain-specific AST) | VerbSearchResult candidates |
| Integration | Standalone pipeline | Pipe in HybridVerbSearcher |
| Grammar | Nom parser for structured intents | Simple keyword lookup |
| Coverage | OTC domain (ISDA, CSA, counterparty) | All domains (session, cbu, kyc, etc.) |

**These systems can coexist.** The ob-agentic lexicon is used for complex OTC onboarding flows, while the new Lexicon Service is a lightweight vocabulary layer for general verb discovery.

---

## Migration: Hardcoded Convergence Maps

The existing `rust/src/mcp/convergence.rs` has hardcoded domain inference:

```rust
// BEFORE (convergence.rs)
fn infer_domain(input: &str) -> Option<String> {
    if input.contains("kyc") || input.contains("case") {
        return Some("kyc".to_string());
    }
    // ... more hardcoded rules
}
```

**Migration:** Replace with lexicon lookup:

```rust
// AFTER (uses lexicon)
fn infer_domain(input: &str, lexicon: &LexiconSnapshot) -> Option<String> {
    let tokens = lexicon.tokenize(input);
    tokens.iter().find_map(|t| t.domain())
}
```

This is Phase G work (after core lexicon is proven).

---

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Test harness strict pass | 84.9% | 90%+ |
| Vague query handling | Suggest path | Clear match or helpful suggestions |
| Latency (lexical lookup) | N/A | <1ms |
| Latency (full search) | 15-30ms | 10-20ms (skip semantic on lexical hit) |
| Evidence explainability | Score only | "Matched 'create' + 'fund' → cbu.create" |

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Vocabulary explosion | Strict governance, lint on startup |
| False positives | Lexical candidates feed into final ranking, don't bypass |
| Maintenance burden | Auto-suggest from learning, periodic audit |
| Latency regression | Benchmark on every PR, <1ms SLA |

---

## Next Steps

1. **Immediate:** Implement Phase A0 (PoC) to validate approach
2. Review this plan with stakeholders
3. Decide on hot-reload requirement (Phase D)
4. Decide on learning integration (Phase F)

---

## Appendix: Token Classification Algorithm

```rust
impl LexiconSnapshot {
    pub fn tokenize(&self, input: &str) -> Vec<Token> {
        let normalized = input.trim().to_lowercase();
        let mut tokens = Vec::new();
        let mut remaining = normalized.as_str();
        
        while !remaining.is_empty() {
            remaining = remaining.trim_start();
            if remaining.is_empty() {
                break;
            }
            
            // Try multi-word phrase match (longest first)
            if let Some((phrase, token_type, len)) = self.try_phrase_match(remaining) {
                tokens.push(Token {
                    text: phrase,
                    token_type,
                    span: (0, len), // TODO: track actual spans
                });
                remaining = &remaining[len..];
                continue;
            }
            
            // Single word
            let word_end = remaining
                .find(|c: char| c.is_whitespace())
                .unwrap_or(remaining.len());
            let word = &remaining[..word_end];
            
            let token_type = self.classify_word(word);
            tokens.push(Token {
                text: word.to_string(),
                token_type,
                span: (0, word_end),
            });
            
            remaining = &remaining[word_end..];
        }
        
        tokens
    }
    
    fn try_phrase_match(&self, input: &str) -> Option<(String, TokenType, usize)> {
        // Check verb synonyms (multi-word like "spin up", "set up")
        for (phrase, verb_class) in &self.verb_synonyms {
            if input.starts_with(phrase.as_str()) {
                let next_char = input.chars().nth(phrase.len());
                if next_char.map(|c| c.is_whitespace()).unwrap_or(true) {
                    return Some((
                        phrase.clone(),
                        TokenType::VerbSynonym(*verb_class),
                        phrase.len(),
                    ));
                }
            }
        }
        
        // Check entity types (multi-word like "client business unit")
        for (phrase, entity_type) in &self.entity_types {
            if input.starts_with(phrase.as_str()) {
                let next_char = input.chars().nth(phrase.len());
                if next_char.map(|c| c.is_whitespace()).unwrap_or(true) {
                    return Some((
                        phrase.clone(),
                        TokenType::EntityType(entity_type.clone()),
                        phrase.len(),
                    ));
                }
            }
        }
        
        None
    }
    
    fn classify_word(&self, word: &str) -> TokenType {
        // Check single-word matches
        if let Some(verb_class) = self.verb_synonyms.get(word) {
            return TokenType::VerbSynonym(*verb_class);
        }
        if let Some(entity_type) = self.entity_types.get(word) {
            return TokenType::EntityType(entity_type.clone());
        }
        if let Some(domain) = self.domain_keywords.get(word) {
            return TokenType::DomainKeyword(domain.clone());
        }
        if self.prepositions.contains(word) {
            return TokenType::Preposition;
        }
        if self.articles.contains(word) {
            return TokenType::Article;
        }
        
        // Unknown - might be entity name
        TokenType::Unknown
    }
}
```

---

## Appendix: Ensemble Mode Evidence

When `OB_VERB_ENSEMBLE_MODE=1`, the lexical pipe contributes evidence:

```rust
VerbSearchResult {
    verb: "cbu.create",
    score: 0.95,
    source: VerbSearchSource::Lexical,
    matched_phrase: "create fund",
    evidence: vec![
        VerbEvidence {
            source: VerbSearchSource::Lexical,
            score: 0.95,
            matched_phrase: "create fund",
            // Extended fields for lexical:
            // verb_class: Some(VerbClass::Create),
            // entity_type: Some(EntityTypeCode::Fund),
        },
        VerbEvidence {
            source: VerbSearchSource::PatternEmbedding,
            score: 0.72,
            matched_phrase: "create a new fund",
        },
    ],
}
```

This enables:
- **Explainability:** "Matched because 'create' maps to CREATE verb class and 'fund' maps to FUND entity type"
- **Calibration:** Compare lexical vs semantic scores across queries
- **Debugging:** See which channel contributed most
