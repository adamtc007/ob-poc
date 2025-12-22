# TODO: Trading Matrix Agent Intelligence
## Phase 3: Intent Understanding & DSL Generation

**Created:** December 22, 2025  
**Prerequisite:** Phase 1 (code-complete) and Phase 2 (reference data + scenarios)  
**Scope:** Agent RAG, intent classification, entity extraction, DSL generation, conversation management  
**Estimated Effort:** 4-6 weeks  

**The Goal:** An agent that genuinely understands complex trading domain requests and generates correct, executable DSL - not through keyword matching, but through semantic understanding of intent, entities, and context.

---

## The Problem Space

### Why This Is Hard

```
USER: "BlackRock will handle European equities via CTM, 
       PIMCO does our fixed income through SWIFT,
       and we need Bloomberg for pricing across the board"

EXPECTED AGENT OUTPUT:
(investment-manager.assign :cbu-id @current-cbu 
  :manager-name "BlackRock" :scope-markets ["XLON" "XETR" "XPAR" "XAMS"] 
  :scope-instrument-classes ["EQUITY"] :instruction-method CTM :priority 10)
→ @im-blackrock

(investment-manager.assign :cbu-id @current-cbu
  :manager-name "PIMCO" :scope-instrument-classes ["GOVT_BOND" "CORP_BOND"]
  :instruction-method SWIFT :priority 10)
→ @im-pimco

(pricing-config.set :cbu-id @current-cbu :instrument-class "EQUITY" 
  :source BLOOMBERG :priority 1)
(pricing-config.set :cbu-id @current-cbu :instrument-class "GOVT_BOND"
  :source BLOOMBERG :priority 1)
(pricing-config.set :cbu-id @current-cbu :instrument-class "CORP_BOND"
  :source BLOOMBERG :priority 1)
```

**Challenges in this single request:**
1. Implicit verb selection (user never said "assign" or "set")
2. Entity extraction ("BlackRock" = manager name, "European" = market scope)
3. Expansion ("European equities" → specific MIC codes)
4. Multiple intents in one utterance (2 IMs + pricing)
5. Inference ("across the board" = all instrument classes in universe)
6. Context dependency (@current-cbu must be known)
7. Parameter defaults (priority not specified, use sensible default)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         AGENT INTELLIGENCE STACK                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐  │
│  │   Intent    │───▶│   Entity    │───▶│   Context   │───▶│     DSL     │  │
│  │ Classifier  │    │  Extractor  │    │  Resolver   │    │  Generator  │  │
│  └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘  │
│         │                 │                  │                   │          │
│         ▼                 ▼                  ▼                   ▼          │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      KNOWLEDGE LAYER (RAG)                          │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐            │   │
│  │  │  Verb    │  │  Domain  │  │ Reference│  │ Convo    │            │   │
│  │  │  Index   │  │ Ontology │  │   Data   │  │ History  │            │   │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘            │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│         │                 │                  │                   │          │
│         ▼                 ▼                  ▼                   ▼          │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      GROUNDING LAYER                                │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐            │   │
│  │  │  Schema  │  │  Valid   │  │  Current │  │ Business │            │   │
│  │  │  Types   │  │  Values  │  │  State   │  │  Rules   │            │   │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘            │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Phase 19: Intent Classification System (Days 1-5)

### 19.1 Define Intent Taxonomy
**File:** `rust/config/agent/intent_taxonomy.yaml`

```yaml
# Hierarchical intent taxonomy for trading matrix domain
intent_taxonomy:
  # Level 1: Domain
  trading_matrix:
    description: "Configuration of trading capabilities"
    
    # Level 2: Sub-domain
    investment_manager:
      description: "Investment manager assignment and scope"
      
      # Level 3: Action intents
      intents:
        - intent: im_assign
          description: "Assign a new investment manager to CBU"
          canonical_verb: investment-manager.assign
          trigger_phrases:
            - "add {manager} as investment manager"
            - "{manager} will handle {scope}"
            - "assign {manager} to trade {scope}"
            - "set up {manager} for {scope}"
            - "{manager} manages {scope}"
          required_entities:
            - manager_name_or_lei
          optional_entities:
            - scope_markets
            - scope_instruments
            - instruction_method
            - priority
          default_inferences:
            priority: 100
            instruction_method: SWIFT
            
        - intent: im_update_scope
          description: "Modify an existing IM's trading scope"
          canonical_verb: investment-manager.set-scope
          trigger_phrases:
            - "expand {manager} to include {scope}"
            - "restrict {manager} to {scope}"
            - "add {scope} to {manager}'s mandate"
            - "{manager} can now trade {scope}"
          required_entities:
            - manager_reference  # Must resolve to existing IM
            - scope_modification
            
        - intent: im_change_connectivity
          description: "Change how IM sends instructions"
          canonical_verb: investment-manager.link-connectivity
          trigger_phrases:
            - "switch {manager} to {method}"
            - "{manager} will use {method} now"
            - "connect {manager} via {method}"
          required_entities:
            - manager_reference
            - instruction_method
            
        - intent: im_query
          description: "Query about IM configuration"
          canonical_verb: investment-manager.list
          trigger_phrases:
            - "who handles {scope}"
            - "which IM trades {scope}"
            - "show me the investment managers"
            - "what can {manager} trade"
          required_entities: []
          
        - intent: im_find_for_trade
          description: "Find which IM would handle a specific trade"
          canonical_verb: investment-manager.find-for-trade
          trigger_phrases:
            - "who would handle a {instrument} trade in {market}"
            - "which IM for {trade_description}"
            - "route this trade: {trade_description}"
          required_entities:
            - trade_characteristics
            
    pricing:
      description: "Pricing source configuration"
      intents:
        - intent: pricing_set
          description: "Configure pricing source for instrument class"
          canonical_verb: pricing-config.set
          trigger_phrases:
            - "use {source} for {instruments}"
            - "{source} pricing for {instruments}"
            - "price {instruments} from {source}"
            - "{source} across the board"
            - "{source} for everything"
          required_entities:
            - pricing_source
          optional_entities:
            - instrument_classes  # If omitted, infer from context
            
        - intent: pricing_query
          description: "Query pricing configuration"
          canonical_verb: pricing-config.list
          trigger_phrases:
            - "what's the pricing source for {instruments}"
            - "where do we get {instrument} prices"
            - "show pricing config"
            
    cash_management:
      description: "Cash sweep and STIF configuration"
      intents:
        - intent: sweep_configure
          description: "Set up cash sweep"
          canonical_verb: cash-sweep.configure
          trigger_phrases:
            - "sweep {currency} above {amount} to {vehicle}"
            - "set up {currency} sweep"
            - "{currency} cash to STIF"
            - "sweep idle {currency}"
          required_entities:
            - currency
          optional_entities:
            - threshold_amount
            - vehicle_type
            - sweep_time
          default_inferences:
            vehicle_type: STIF
            threshold_amount: 100000  # Varies by currency
            
    sla:
      description: "Service level agreement management"
      intents:
        - intent: sla_commit
          description: "Create SLA commitment"
          canonical_verb: sla.commit
          trigger_phrases:
            - "we need {target}% {metric}"
            - "SLA for {metric}"
            - "commit to {target} {metric}"
            - "{metric} must be {target}"
          required_entities:
            - metric_type_or_template
          optional_entities:
            - target_override
            
        - intent: sla_query
          description: "Query SLA status"
          canonical_verb: sla.list-commitments
          trigger_phrases:
            - "what are our SLAs"
            - "show SLA coverage"
            - "are we meeting SLAs"
            - "any SLA breaches"
            
    profile:
      description: "Trading profile lifecycle"
      intents:
        - intent: profile_create
          description: "Start new trading profile"
          canonical_verb: trading-profile.create
          trigger_phrases:
            - "start trading profile"
            - "new trading matrix"
            - "set up trading for {cbu}"
            - "traded instruments day"
            - "configure trading"
            
        - intent: profile_visualize
          description: "Show trading matrix visually"
          canonical_verb: trading-profile.visualize
          trigger_phrases:
            - "show me the matrix"
            - "visualize trading profile"
            - "what can we trade"
            - "trading overview"
            
        - intent: profile_validate
          description: "Check for configuration gaps"
          canonical_verb: trading-profile.validate-matrix
          trigger_phrases:
            - "check for gaps"
            - "validate the profile"
            - "is anything missing"
            - "are we complete"
            
        - intent: profile_materialize
          description: "Push profile to operational tables"
          canonical_verb: trading-profile.materialize
          trigger_phrases:
            - "activate this"
            - "push to production"
            - "materialize"
            - "make it live"
            - "we're done, save it"
            
  # Compound/Meta intents
  compound:
    intents:
      - intent: full_setup
        description: "Complete trading matrix setup"
        expands_to:
          - profile_create
          - im_assign (multiple)
          - pricing_set (multiple)
          - sweep_configure (multiple)
          - sla_commit (multiple)
          - profile_validate
        trigger_phrases:
          - "set up complete trading for {cbu}"
          - "full custody setup"
          - "onboard trading for {cbu}"
          
      - intent: im_with_connectivity
        description: "Assign IM and provision connectivity in one"
        expands_to:
          - im_assign
          - service_resource.provision
          - im_link_connectivity
        trigger_phrases:
          - "{manager} via {method}"  # Implies assign + connectivity
```

- [ ] Create comprehensive intent taxonomy
- [ ] Cover all verb domains
- [ ] Include trigger phrase patterns
- [ ] Define required vs optional entities
- [ ] Define compound intents
- [ ] Review with domain expert

### 19.2 Intent Classifier Implementation
**File:** `rust/src/agent/intent_classifier.rs`

```rust
use crate::agent::embeddings::EmbeddingService;
use crate::agent::intent_taxonomy::IntentTaxonomy;

pub struct IntentClassifier {
    taxonomy: IntentTaxonomy,
    embeddings: EmbeddingService,
    intent_vectors: HashMap<String, Vec<f32>>,  // Pre-computed intent embeddings
    threshold: f32,
}

impl IntentClassifier {
    /// Classify user utterance into one or more intents
    pub async fn classify(&self, utterance: &str, context: &ConversationContext) 
        -> Result<Vec<ClassifiedIntent>, ClassifierError> 
    {
        // Step 1: Generate embedding for utterance
        let utterance_embedding = self.embeddings.embed(utterance).await?;
        
        // Step 2: Find candidate intents by semantic similarity
        let mut candidates = self.find_similar_intents(&utterance_embedding);
        
        // Step 3: Re-rank using context
        candidates = self.rerank_with_context(candidates, context);
        
        // Step 4: Check for compound intents (multiple intents in one utterance)
        let intents = self.detect_compound_intents(utterance, candidates)?;
        
        // Step 5: Filter by confidence threshold
        let confident_intents: Vec<_> = intents
            .into_iter()
            .filter(|i| i.confidence >= self.threshold)
            .collect();
        
        // Step 6: If low confidence, return NEEDS_CLARIFICATION
        if confident_intents.is_empty() {
            return Ok(vec![ClassifiedIntent {
                intent: Intent::NeedsClarification,
                confidence: 0.0,
                ambiguous_between: candidates.into_iter().take(3).collect(),
            }]);
        }
        
        Ok(confident_intents)
    }
    
    /// Detect if utterance contains multiple intents
    fn detect_compound_intents(
        &self, 
        utterance: &str, 
        candidates: Vec<ScoredIntent>
    ) -> Result<Vec<ClassifiedIntent>, ClassifierError> {
        // Use sentence segmentation + conjunction detection
        // "BlackRock for equities AND PIMCO for bonds" = 2 intents
        
        let segments = self.segment_utterance(utterance);
        
        if segments.len() == 1 {
            // Single intent
            return Ok(vec![candidates.first().map(|c| c.into()).unwrap()]);
        }
        
        // Multiple segments - classify each
        let mut intents = Vec::new();
        for segment in segments {
            let segment_embedding = self.embeddings.embed(&segment).await?;
            let segment_candidates = self.find_similar_intents(&segment_embedding);
            if let Some(best) = segment_candidates.first() {
                intents.push(ClassifiedIntent {
                    intent: best.intent.clone(),
                    confidence: best.score,
                    source_text: segment,
                    ..Default::default()
                });
            }
        }
        
        Ok(intents)
    }
    
    /// Re-rank candidates based on conversation context
    fn rerank_with_context(
        &self,
        candidates: Vec<ScoredIntent>,
        context: &ConversationContext,
    ) -> Vec<ScoredIntent> {
        candidates.into_iter().map(|mut c| {
            // Boost intents that follow naturally from recent actions
            if let Some(last_intent) = &context.last_intent {
                if self.taxonomy.is_natural_followup(&last_intent, &c.intent) {
                    c.score *= 1.2;  // 20% boost for natural flow
                }
            }
            
            // Boost intents relevant to current workflow stage
            if let Some(stage) = &context.workflow_stage {
                if self.taxonomy.intent_relevant_to_stage(&c.intent, stage) {
                    c.score *= 1.1;
                }
            }
            
            // Penalize intents that require entities not present
            let missing = self.taxonomy.missing_required_entities(&c.intent, &context.known_entities);
            c.score *= (1.0 - 0.1 * missing.len() as f32);
            
            c
        }).collect()
    }
}

#[derive(Debug, Clone)]
pub struct ClassifiedIntent {
    pub intent: Intent,
    pub confidence: f32,
    pub source_text: String,
    pub ambiguous_between: Vec<Intent>,
    pub suggested_clarification: Option<String>,
}
```

- [ ] Implement IntentClassifier struct
- [ ] Implement semantic similarity search
- [ ] Implement context-based re-ranking
- [ ] Implement compound intent detection
- [ ] Implement confidence thresholding
- [ ] Unit tests for classifier

### 19.3 Intent Embeddings Index
**File:** `rust/src/agent/intent_index.rs`

```rust
/// Build vector index of intent trigger phrases for semantic search
pub async fn build_intent_index(
    taxonomy: &IntentTaxonomy,
    embeddings: &EmbeddingService,
) -> Result<IntentIndex, IndexError> {
    let mut index = IntentIndex::new();
    
    for domain in &taxonomy.domains {
        for subdomain in &domain.subdomains {
            for intent in &subdomain.intents {
                // Embed each trigger phrase
                for phrase in &intent.trigger_phrases {
                    let embedding = embeddings.embed(phrase).await?;
                    index.add(IntentVector {
                        intent_id: intent.id.clone(),
                        phrase: phrase.clone(),
                        embedding,
                    });
                }
                
                // Also embed the description for broader matching
                let desc_embedding = embeddings.embed(&intent.description).await?;
                index.add(IntentVector {
                    intent_id: intent.id.clone(),
                    phrase: intent.description.clone(),
                    embedding: desc_embedding,
                });
            }
        }
    }
    
    // Build HNSW index for fast approximate nearest neighbor
    index.build_hnsw()?;
    
    Ok(index)
}
```

- [ ] Create intent index builder
- [ ] Embed all trigger phrases
- [ ] Build HNSW index for fast search
- [ ] Persist index to disk
- [ ] Load on startup

### 19.4 Confidence Calibration
**File:** `rust/src/agent/confidence_calibration.rs`

```rust
/// Calibrate raw similarity scores to meaningful confidence values
pub struct ConfidenceCalibrator {
    // Learned thresholds from evaluation data
    thresholds: HashMap<String, ThresholdConfig>,
}

impl ConfidenceCalibrator {
    /// Convert raw cosine similarity to calibrated confidence
    pub fn calibrate(&self, intent: &str, raw_score: f32) -> f32 {
        let config = self.thresholds.get(intent)
            .unwrap_or(&self.thresholds["_default"]);
        
        // Sigmoid calibration
        let x = (raw_score - config.midpoint) / config.temperature;
        1.0 / (1.0 + (-x).exp())
    }
    
    /// Determine if confidence is high enough to act
    pub fn should_execute(&self, intent: &str, confidence: f32) -> ExecutionDecision {
        let config = self.thresholds.get(intent)
            .unwrap_or(&self.thresholds["_default"]);
        
        if confidence >= config.execute_threshold {
            ExecutionDecision::Execute
        } else if confidence >= config.confirm_threshold {
            ExecutionDecision::ConfirmFirst
        } else if confidence >= config.suggest_threshold {
            ExecutionDecision::Suggest
        } else {
            ExecutionDecision::Clarify
        }
    }
}

pub enum ExecutionDecision {
    Execute,      // High confidence, just do it
    ConfirmFirst, // Medium confidence, confirm with user
    Suggest,      // Low confidence, suggest but ask
    Clarify,      // Too low, ask clarifying questions
}
```

- [ ] Implement confidence calibration
- [ ] Define per-intent thresholds
- [ ] Implement execution decision logic
- [ ] Create calibration dataset
- [ ] Tune thresholds empirically

---

## Phase 20: Entity Extraction Pipeline (Days 5-10)

### 20.1 Domain Entity Types
**File:** `rust/config/agent/entity_types.yaml`

```yaml
# Domain-specific entity types for trading matrix
entity_types:
  # Financial entities
  manager_reference:
    description: "Reference to an investment manager"
    patterns:
      - type: NAME
        examples: ["BlackRock", "PIMCO", "Vanguard", "State Street"]
      - type: LEI
        regex: "[A-Z0-9]{20}"
        examples: ["549300EXAMPLE00001"]
      - type: ANAPHORA
        examples: ["the first IM", "that manager", "them", "BlackRock's"]
    normalization:
      lookup_table: investment_managers
      fuzzy_match: true
      
  market_reference:
    description: "Reference to a trading market/exchange"
    patterns:
      - type: MIC
        regex: "X[A-Z]{3}"
        examples: ["XNYS", "XLON", "XETR"]
      - type: NAME
        examples: ["NYSE", "London", "Frankfurt", "Euronext"]
      - type: REGION
        examples: ["European", "Asian", "US", "APAC", "EMEA"]
        expands_to: market_list  # "European" → ["XLON", "XETR", "XPAR", ...]
    normalization:
      lookup_table: markets
      expansion_rules: market_regions
      
  instrument_class_reference:
    description: "Reference to an instrument class"
    patterns:
      - type: CODE
        examples: ["EQUITY", "GOVT_BOND", "IRS", "CDS"]
      - type: NAME
        examples: ["equities", "government bonds", "swaps", "credit derivatives"]
      - type: CATEGORY
        examples: ["fixed income", "derivatives", "alternatives"]
        expands_to: instrument_list
    normalization:
      lookup_table: instrument_classes
      hierarchy_aware: true  # "fixed income" → ["GOVT_BOND", "CORP_BOND", "MUNI_BOND"]
      
  instruction_method:
    description: "How trade instructions are delivered"
    patterns:
      - type: CODE
        valid_values: [SWIFT, CTM, FIX, API, ALERT, MANUAL]
      - type: NAME
        mappings:
          "message": SWIFT
          "matching": CTM
          "electronic": FIX
          "programmatic": API
          "Bloomberg": ALERT
          
  pricing_source:
    description: "Source of pricing data"
    patterns:
      - type: CODE
        valid_values: [BLOOMBERG, REFINITIV, MARKIT, ICE, INTERNAL]
      - type: NAME
        mappings:
          "Bloomberg": BLOOMBERG
          "BBG": BLOOMBERG
          "Reuters": REFINITIV
          "Markit": MARKIT
          "in-house": INTERNAL
          
  currency:
    description: "Currency code"
    patterns:
      - type: ISO_CODE
        regex: "[A-Z]{3}"
        examples: ["USD", "EUR", "GBP"]
      - type: NAME
        mappings:
          "dollars": USD
          "euros": EUR
          "pounds": GBP
          "sterling": GBP
          "yen": JPY
          
  amount:
    description: "Monetary amount"
    patterns:
      - type: NUMBER
        regex: "\\d+[,\\d]*"
      - type: WITH_UNIT
        regex: "(\\d+[,\\d]*)\\s*(k|m|mm|bn|K|M|B)?"
        normalization:
          k: 1000
          m: 1000000
          mm: 1000000
          bn: 1000000000
          
  time_reference:
    description: "Time of day reference"
    patterns:
      - type: TIME
        regex: "(\\d{1,2}):?(\\d{2})?\\s*(am|pm|AM|PM)?"
      - type: NAME
        mappings:
          "close": "16:00"
          "end of day": "17:00"
          "COB": "17:00"
          "market close": null  # Needs market context
          
  percentage:
    description: "Percentage value"
    patterns:
      - type: NUMBER
        regex: "(\\d+\\.?\\d*)\\s*%?"
        
  # Relationship/scope entities
  scope_expression:
    description: "Defines a trading scope"
    components:
      - markets: list[market_reference]
      - instruments: list[instrument_class_reference]
      - currencies: list[currency]
    examples:
      - "European equities"
      - "US and Canadian bonds"
      - "global fixed income except munis"
```

- [ ] Define all domain entity types
- [ ] Include pattern variations
- [ ] Define normalization rules
- [ ] Define expansion rules (region → markets)
- [ ] Include examples for training

### 20.2 Entity Extractor Implementation
**File:** `rust/src/agent/entity_extractor.rs`

```rust
pub struct EntityExtractor {
    entity_types: EntityTypeRegistry,
    reference_data: ReferenceDataCache,
    embeddings: EmbeddingService,
    coreference_resolver: CoreferenceResolver,
}

impl EntityExtractor {
    /// Extract all entities from utterance
    pub async fn extract(
        &self,
        utterance: &str,
        context: &ConversationContext,
    ) -> Result<ExtractedEntities, ExtractionError> {
        let mut entities = ExtractedEntities::new();
        
        // Step 1: Pattern-based extraction (regex, lookups)
        let pattern_entities = self.extract_by_pattern(utterance)?;
        entities.merge(pattern_entities);
        
        // Step 2: Semantic extraction (for ambiguous mentions)
        let semantic_entities = self.extract_semantic(utterance).await?;
        entities.merge(semantic_entities);
        
        // Step 3: Resolve coreferences ("them", "that manager", etc.)
        let resolved = self.coreference_resolver.resolve(&entities, context)?;
        entities.merge(resolved);
        
        // Step 4: Expand category references ("European" → market list)
        let expanded = self.expand_categories(&entities)?;
        entities.merge(expanded);
        
        // Step 5: Validate against reference data
        let validated = self.validate_entities(&entities)?;
        
        // Step 6: Infer missing entities from context
        let inferred = self.infer_from_context(&validated, context)?;
        
        Ok(inferred)
    }
    
    /// Extract entities using regex and lookup patterns
    fn extract_by_pattern(&self, utterance: &str) -> Result<ExtractedEntities, ExtractionError> {
        let mut entities = ExtractedEntities::new();
        
        for entity_type in self.entity_types.all() {
            for pattern in &entity_type.patterns {
                match pattern {
                    Pattern::Regex(re) => {
                        for capture in re.captures_iter(utterance) {
                            entities.add(ExtractedEntity {
                                entity_type: entity_type.name.clone(),
                                value: capture.get(0).unwrap().as_str().to_string(),
                                span: (capture.start(), capture.end()),
                                confidence: 0.95,  // High confidence for regex match
                                source: ExtractionSource::Pattern,
                            });
                        }
                    }
                    Pattern::Lookup(table) => {
                        // Check if any known value appears in utterance
                        for known_value in self.reference_data.get_values(table) {
                            if let Some(pos) = utterance.to_lowercase()
                                .find(&known_value.name.to_lowercase()) 
                            {
                                entities.add(ExtractedEntity {
                                    entity_type: entity_type.name.clone(),
                                    value: known_value.code.clone(),
                                    span: (pos, pos + known_value.name.len()),
                                    confidence: 0.9,
                                    source: ExtractionSource::Lookup,
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        
        Ok(entities)
    }
    
    /// Expand category references to concrete values
    fn expand_categories(&self, entities: &ExtractedEntities) 
        -> Result<ExtractedEntities, ExtractionError> 
    {
        let mut expanded = ExtractedEntities::new();
        
        for entity in entities.iter() {
            if let Some(expansion_rule) = self.entity_types
                .get(&entity.entity_type)
                .and_then(|et| et.expansion_rules.as_ref()) 
            {
                // e.g., "European" → ["XLON", "XETR", "XPAR", "XAMS", "XSWX"]
                if let Some(expanded_values) = expansion_rule.expand(&entity.value) {
                    for value in expanded_values {
                        expanded.add(ExtractedEntity {
                            entity_type: entity.entity_type.clone(),
                            value,
                            span: entity.span,
                            confidence: entity.confidence * 0.9,  // Slightly lower for expansion
                            source: ExtractionSource::Expansion,
                            derived_from: Some(entity.id),
                        });
                    }
                }
            }
        }
        
        Ok(expanded)
    }
}
```

- [ ] Implement EntityExtractor struct
- [ ] Implement pattern-based extraction
- [ ] Implement lookup-based extraction
- [ ] Implement semantic extraction (embeddings)
- [ ] Implement category expansion
- [ ] Implement validation against reference data
- [ ] Unit tests for each extraction method

### 20.3 Coreference Resolution
**File:** `rust/src/agent/coreference.rs`

```rust
/// Resolve pronouns and anaphoric references to entities
pub struct CoreferenceResolver {
    pronoun_patterns: HashMap<String, PronounType>,
}

impl CoreferenceResolver {
    /// Resolve references in current utterance using conversation context
    pub fn resolve(
        &self,
        entities: &ExtractedEntities,
        context: &ConversationContext,
    ) -> Result<ExtractedEntities, CoreferenceError> {
        let mut resolved = ExtractedEntities::new();
        
        for entity in entities.iter() {
            if let Some(pronoun_type) = self.is_pronoun(&entity.value) {
                // Find antecedent in context
                let antecedent = self.find_antecedent(
                    pronoun_type,
                    &entity.entity_type,
                    context,
                )?;
                
                if let Some(ant) = antecedent {
                    resolved.add(ExtractedEntity {
                        entity_type: entity.entity_type.clone(),
                        value: ant.value.clone(),
                        span: entity.span,
                        confidence: entity.confidence * ant.confidence,
                        source: ExtractionSource::Coreference,
                        resolved_from: Some(entity.value.clone()),
                    });
                }
            }
        }
        
        Ok(resolved)
    }
    
    fn find_antecedent(
        &self,
        pronoun_type: PronounType,
        expected_type: &str,
        context: &ConversationContext,
    ) -> Result<Option<ResolvedEntity>, CoreferenceError> {
        // Look back through conversation for matching entity
        for turn in context.history.iter().rev() {
            for entity in &turn.entities {
                if entity.entity_type == expected_type {
                    // Check pronoun agreement
                    if self.agrees(pronoun_type, entity) {
                        return Ok(Some(ResolvedEntity {
                            value: entity.value.clone(),
                            confidence: 0.85,  // Some uncertainty in resolution
                            source_turn: turn.id,
                        }));
                    }
                }
            }
        }
        
        // Also check current session's created entities
        for (symbol, entity) in &context.session_entities {
            if entity.entity_type == expected_type {
                return Ok(Some(ResolvedEntity {
                    value: entity.value.clone(),
                    confidence: 0.95,  // Higher confidence for session entities
                    source_turn: 0,
                }));
            }
        }
        
        Ok(None)
    }
}
```

- [ ] Implement CoreferenceResolver
- [ ] Handle pronouns (it, them, they)
- [ ] Handle demonstratives (that, this, those)
- [ ] Handle definite descriptions ("the manager", "the first IM")
- [ ] Look back through conversation history
- [ ] Track session entities for resolution

### 20.4 Market Region Expansion
**File:** `rust/config/agent/market_regions.yaml`

```yaml
# Map region names to specific markets
market_regions:
  European:
    description: "Major European markets"
    markets:
      - XLON  # London
      - XETR  # XETRA/Frankfurt
      - XPAR  # Euronext Paris
      - XAMS  # Euronext Amsterdam
      - XSWX  # SIX Swiss
      - XMIL  # Borsa Italiana
      - XMAD  # BME Spanish
      - XBRU  # Euronext Brussels
      - XLIS  # Euronext Lisbon
    aliases: ["Europe", "EU", "EMEA equities"]
    
  US:
    description: "US markets"
    markets:
      - XNYS  # NYSE
      - XNAS  # NASDAQ
      - XASE  # NYSE American
      - BATS  # BATS
      - ARCX  # NYSE Arca
    aliases: ["American", "United States", "North American"]
    
  Asian:
    description: "Major Asian markets"
    markets:
      - XHKG  # Hong Kong
      - XTKS  # Tokyo
      - XSES  # Singapore
      - XASX  # Australia
      - XKRX  # Korea
    aliases: ["Asia", "APAC", "Asia-Pacific"]
    
  EM:
    description: "Emerging markets"
    markets:
      - XBSP  # Brazil B3
      - XMEX  # Mexico
      - XJSE  # Johannesburg
      - XBOM  # Mumbai
      - XSHG  # Shanghai
    aliases: ["emerging", "emerging markets", "EM"]
    
  Global:
    description: "All supported markets"
    expands_to: ALL_MARKETS
    aliases: ["worldwide", "all markets", "everywhere"]
    
  DM:
    description: "Developed markets"
    union_of: [European, US, Asian]
    exclude: [EM]
    aliases: ["developed", "developed markets"]
```

- [ ] Define all region mappings
- [ ] Include aliases
- [ ] Support union/exclude logic
- [ ] Load into expansion engine
- [ ] Test expansion

### 20.5 Instrument Class Hierarchy
**File:** `rust/config/agent/instrument_hierarchy.yaml`

```yaml
# Hierarchical instrument classification for expansion
instrument_hierarchy:
  root:
    children:
      - listed_securities
      - fixed_income
      - derivatives
      - cash_equivalents
      - alternatives
      
  listed_securities:
    name: "Listed Securities"
    aliases: ["equities", "stocks", "shares", "listed"]
    children:
      - EQUITY
      - ETF
      - ADR
      - REIT
      
  fixed_income:
    name: "Fixed Income"
    aliases: ["bonds", "debt", "credit"]
    children:
      - government_bonds
      - corporate_credit
      - structured
      
  government_bonds:
    name: "Government Bonds"
    aliases: ["govvies", "sovereigns", "government debt"]
    children:
      - GOVT_BOND
      - MUNI_BOND
      - TIPS
      - GILT
      
  corporate_credit:
    name: "Corporate Credit"
    aliases: ["corporates", "corporate bonds", "credit"]
    children:
      - CORP_BOND
      - HIGH_YIELD
      - CONVERTIBLE
      
  derivatives:
    name: "Derivatives"
    aliases: ["OTC", "swaps", "options"]
    requires_isda: true
    children:
      - rates_derivatives
      - credit_derivatives
      - fx_derivatives
      - equity_derivatives
      
  rates_derivatives:
    name: "Rates Derivatives"
    aliases: ["rates", "interest rate derivatives"]
    isda_asset_class: RATES
    children:
      - IRS
      - XCCY
      - SWAPTION
      - CAP_FLOOR
      
  fx_derivatives:
    name: "FX Derivatives"
    aliases: ["FX", "forex", "currency derivatives"]
    isda_asset_class: FX
    children:
      - FX_FORWARD
      - FX_OPTION
      - FX_SWAP
```

- [ ] Define complete instrument hierarchy
- [ ] Include all aliases
- [ ] Mark ISDA requirements
- [ ] Include ISDA asset class mappings
- [ ] Load into expansion engine

---

## Phase 21: Context Management (Days 10-14)

### 21.1 Conversation Context Model
**File:** `rust/src/agent/context.rs`

```rust
/// Complete context for agent reasoning
#[derive(Debug, Clone)]
pub struct ConversationContext {
    // Session state
    pub session_id: Uuid,
    pub current_cbu: Option<CbuContext>,
    pub current_profile: Option<ProfileContext>,
    
    // Conversation history
    pub history: Vec<ConversationTurn>,
    pub last_intent: Option<ClassifiedIntent>,
    
    // Accumulated entities
    pub known_entities: EntityStore,
    pub session_entities: HashMap<String, CreatedEntity>,  // @symbol → entity
    
    // Workflow state
    pub workflow_stage: Option<WorkflowStage>,
    pub pending_confirmations: Vec<PendingConfirmation>,
    
    // Configuration state (what's already set up)
    pub configured_ims: Vec<ConfiguredIM>,
    pub configured_pricing: Vec<ConfiguredPricing>,
    pub configured_sweeps: Vec<ConfiguredSweep>,
    pub configured_slas: Vec<ConfiguredSLA>,
    
    // User preferences
    pub user_preferences: UserPreferences,
}

impl ConversationContext {
    /// Update context after processing a turn
    pub fn update_from_turn(&mut self, turn: &ProcessedTurn) {
        // Add to history
        self.history.push(ConversationTurn {
            id: turn.id,
            user_message: turn.user_message.clone(),
            classified_intents: turn.intents.clone(),
            extracted_entities: turn.entities.clone(),
            generated_dsl: turn.dsl.clone(),
            execution_result: turn.result.clone(),
        });
        
        // Update last intent
        self.last_intent = turn.intents.first().cloned();
        
        // Merge entities
        self.known_entities.merge(&turn.entities);
        
        // Track created entities
        for (symbol, entity) in &turn.created_entities {
            self.session_entities.insert(symbol.clone(), entity.clone());
        }
        
        // Update configuration state
        self.update_configuration_state(&turn.result);
    }
    
    /// Get relevant context for DSL generation
    pub fn get_generation_context(&self) -> GenerationContext {
        GenerationContext {
            cbu_id: self.current_cbu.as_ref().map(|c| c.cbu_id),
            profile_id: self.current_profile.as_ref().map(|p| p.profile_id),
            available_symbols: self.session_entities.keys().cloned().collect(),
            existing_ims: self.configured_ims.clone(),
            existing_pricing: self.configured_pricing.clone(),
            // ... etc
        }
    }
    
    /// Check what's missing for current workflow stage
    pub fn get_gaps(&self) -> Vec<ConfigurationGap> {
        let mut gaps = Vec::new();
        
        // Check if any instrument classes lack pricing
        if let Some(profile) = &self.current_profile {
            for ic in &profile.instrument_classes {
                if !self.configured_pricing.iter().any(|p| p.instrument_class == *ic) {
                    gaps.push(ConfigurationGap {
                        gap_type: GapType::MissingPricing,
                        detail: format!("No pricing source for {}", ic),
                        severity: GapSeverity::Warning,
                    });
                }
            }
        }
        
        // Check if OTC instruments lack ISDA
        // ... etc
        
        gaps
    }
}
```

- [ ] Implement ConversationContext struct
- [ ] Track conversation history
- [ ] Track created entities (@symbols)
- [ ] Track configuration state
- [ ] Implement gap detection
- [ ] Implement context summarization for long conversations

### 21.2 Context-Aware Default Inference
**File:** `rust/src/agent/defaults.rs`

```rust
/// Infer missing parameters from context
pub struct DefaultInferencer {
    rules: Vec<InferenceRule>,
}

impl DefaultInferencer {
    /// Infer missing required parameters for intent
    pub fn infer_defaults(
        &self,
        intent: &ClassifiedIntent,
        entities: &ExtractedEntities,
        context: &ConversationContext,
    ) -> Result<InferredDefaults, InferenceError> {
        let mut defaults = InferredDefaults::new();
        
        // Get required parameters for this intent's verb
        let verb_def = self.get_verb_definition(&intent.canonical_verb)?;
        
        for param in &verb_def.args {
            if param.required && !entities.has_for_param(&param.name) {
                // Try to infer from context
                if let Some(value) = self.infer_parameter(&param, context)? {
                    defaults.add(param.name.clone(), value);
                } else if let Some(default) = &param.default {
                    defaults.add(param.name.clone(), default.clone());
                } else {
                    // Can't infer - will need to ask user
                    defaults.mark_missing(param.name.clone());
                }
            }
        }
        
        Ok(defaults)
    }
    
    fn infer_parameter(
        &self,
        param: &VerbParameter,
        context: &ConversationContext,
    ) -> Result<Option<Value>, InferenceError> {
        match param.name.as_str() {
            "cbu-id" => {
                // Use current CBU from context
                Ok(context.current_cbu.as_ref().map(|c| json!(c.cbu_id)))
            }
            "profile-id" => {
                // Use current profile from context
                Ok(context.current_profile.as_ref().map(|p| json!(p.profile_id)))
            }
            "priority" => {
                // Infer priority based on existing IMs
                let existing_count = context.configured_ims.len();
                if existing_count == 0 {
                    Ok(Some(json!(100)))  // First IM gets default priority
                } else {
                    // Check if this looks like a specialist (non-default) IM
                    // Specialists get priority 10, default gets 100
                    Ok(Some(json!(10)))  // TODO: smarter logic
                }
            }
            "instruction-method" => {
                // Infer based on manager or instrument type
                // Derivatives often use API, equities often use CTM, default is SWIFT
                Ok(Some(json!("SWIFT")))
            }
            "source" | "pricing-source" => {
                // Default to Bloomberg if not specified
                Ok(Some(json!("BLOOMBERG")))
            }
            "vehicle-type" => {
                // Default to STIF for cash sweeps
                Ok(Some(json!("STIF")))
            }
            "sweep-timezone" => {
                // Infer from currency or CBU jurisdiction
                if let Some(currency) = context.known_entities.get_latest("currency") {
                    let tz = self.timezone_for_currency(&currency.value);
                    Ok(Some(json!(tz)))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }
    
    fn timezone_for_currency(&self, currency: &str) -> &'static str {
        match currency {
            "USD" => "America/New_York",
            "EUR" => "Europe/Luxembourg",
            "GBP" => "Europe/London",
            "JPY" => "Asia/Tokyo",
            "HKD" => "Asia/Hong_Kong",
            _ => "UTC",
        }
    }
}
```

- [ ] Implement DefaultInferencer
- [ ] Define inference rules per parameter
- [ ] Use context for cbu-id, profile-id
- [ ] Use heuristics for priority
- [ ] Use currency → timezone mappings
- [ ] Track which defaults were inferred (for explanation)

### 21.3 Session State Persistence
**File:** `rust/src/agent/session_store.rs`

```rust
/// Persist conversation context across requests
pub struct SessionStore {
    redis: RedisClient,  // Or other backing store
    ttl: Duration,
}

impl SessionStore {
    /// Save context for session
    pub async fn save(&self, session_id: Uuid, context: &ConversationContext) 
        -> Result<(), SessionError> 
    {
        let serialized = serde_json::to_string(context)?;
        self.redis.set_ex(
            format!("session:{}", session_id),
            serialized,
            self.ttl.as_secs() as usize,
        ).await?;
        Ok(())
    }
    
    /// Load context for session
    pub async fn load(&self, session_id: Uuid) 
        -> Result<Option<ConversationContext>, SessionError> 
    {
        let key = format!("session:{}", session_id);
        if let Some(serialized) = self.redis.get(&key).await? {
            let context: ConversationContext = serde_json::from_str(&serialized)?;
            Ok(Some(context))
        } else {
            Ok(None)
        }
    }
    
    /// Update specific fields without full reload
    pub async fn update_entities(
        &self, 
        session_id: Uuid, 
        new_entities: &HashMap<String, CreatedEntity>
    ) -> Result<(), SessionError> {
        // Atomic update of session entities
        // ...
    }
}
```

- [ ] Implement SessionStore
- [ ] Choose backing store (Redis, PostgreSQL, in-memory)
- [ ] Implement serialization
- [ ] Handle session expiry
- [ ] Implement atomic updates

---

## Phase 22: DSL Generation Engine (Days 14-20)

### 22.1 Generation Pipeline
**File:** `rust/src/agent/dsl_generator.rs`

```rust
pub struct DslGenerator {
    verb_registry: VerbRegistry,
    template_engine: TemplateEngine,
    validator: DslValidator,
}

impl DslGenerator {
    /// Generate DSL from classified intent and extracted entities
    pub async fn generate(
        &self,
        intents: &[ClassifiedIntent],
        entities: &ExtractedEntities,
        defaults: &InferredDefaults,
        context: &ConversationContext,
    ) -> Result<GeneratedDsl, GenerationError> {
        let mut dsl_statements = Vec::new();
        
        for intent in intents {
            let statement = self.generate_for_intent(
                intent, 
                entities, 
                defaults,
                context
            ).await?;
            dsl_statements.push(statement);
        }
        
        // Handle dependencies between statements
        let ordered = self.order_by_dependencies(&dsl_statements)?;
        
        // Validate complete DSL
        let validated = self.validator.validate(&ordered, context)?;
        
        Ok(GeneratedDsl {
            statements: validated,
            requires_confirmation: self.needs_confirmation(&validated),
            explanation: self.generate_explanation(&validated),
        })
    }
    
    async fn generate_for_intent(
        &self,
        intent: &ClassifiedIntent,
        entities: &ExtractedEntities,
        defaults: &InferredDefaults,
        context: &ConversationContext,
    ) -> Result<DslStatement, GenerationError> {
        // Get verb definition
        let verb = self.verb_registry.get(&intent.canonical_verb)?;
        
        // Map entities to verb parameters
        let mut params = HashMap::new();
        
        for arg in &verb.args {
            let value = self.resolve_parameter_value(
                &arg,
                entities,
                defaults,
                context,
            )?;
            
            if let Some(v) = value {
                params.insert(arg.name.clone(), v);
            } else if arg.required {
                return Err(GenerationError::MissingRequiredParameter(arg.name.clone()));
            }
        }
        
        // Generate symbol if verb returns entity
        let capture_symbol = if verb.returns_entity() {
            Some(self.generate_symbol(&verb, &params))
        } else {
            None
        };
        
        Ok(DslStatement {
            verb: verb.full_name(),
            params,
            capture_symbol,
            source_intent: intent.clone(),
        })
    }
    
    fn resolve_parameter_value(
        &self,
        arg: &VerbArg,
        entities: &ExtractedEntities,
        defaults: &InferredDefaults,
        context: &ConversationContext,
    ) -> Result<Option<Value>, GenerationError> {
        // Priority: explicit entity > inferred default > verb default > context
        
        // Check for matching entity
        if let Some(entity) = entities.find_for_param(&arg.name) {
            return Ok(Some(self.convert_entity_to_value(entity, arg)?));
        }
        
        // Check inferred defaults
        if let Some(value) = defaults.get(&arg.name) {
            return Ok(Some(value.clone()));
        }
        
        // Check verb default
        if let Some(default) = &arg.default {
            return Ok(Some(default.clone()));
        }
        
        // Try context
        if let Some(value) = self.get_from_context(&arg.name, context)? {
            return Ok(Some(value));
        }
        
        Ok(None)
    }
    
    fn generate_symbol(&self, verb: &VerbDefinition, params: &HashMap<String, Value>) -> String {
        // Generate meaningful symbol name
        // e.g., @im-blackrock, @pricing-equity-bbg, @sla-settlement
        
        let prefix = match verb.domain.as_str() {
            "investment-manager" => "im",
            "pricing-config" => "pricing",
            "cash-sweep" => "sweep",
            "sla" => "sla",
            _ => "ref",
        };
        
        // Add distinguishing suffix from params
        let suffix = params.get("manager-name")
            .or_else(|| params.get("instrument-class"))
            .or_else(|| params.get("currency"))
            .map(|v| v.as_str().unwrap_or("").to_lowercase())
            .unwrap_or_else(|| "default".to_string());
        
        format!("@{}-{}", prefix, suffix.replace(" ", "-"))
    }
}
```

- [ ] Implement DslGenerator struct
- [ ] Implement intent → verb mapping
- [ ] Implement entity → parameter mapping
- [ ] Implement default value resolution
- [ ] Implement symbol generation
- [ ] Implement dependency ordering
- [ ] Unit tests for generation

### 22.2 Parameter Mapping Rules
**File:** `rust/config/agent/parameter_mappings.yaml`

```yaml
# Map extracted entities to verb parameters
parameter_mappings:
  # Investment Manager verbs
  investment-manager.assign:
    mappings:
      - entity_type: manager_reference
        param: manager-name
        transform: name_or_lei
        
      - entity_type: scope_expression
        params:
          - param: scope-markets
            from: markets
          - param: scope-instrument-classes
            from: instruments
          - param: scope-currencies
            from: currencies
            
      - entity_type: instruction_method
        param: instruction-method
        
      - entity_type: priority_value
        param: priority
        default_if_missing: 100
        
  # Pricing verbs
  pricing-config.set:
    mappings:
      - entity_type: instrument_class_reference
        param: instrument-class
        # If multiple, generate multiple statements
        iterate_if_list: true
        
      - entity_type: pricing_source
        param: source
        
      - entity_type: pricing_source
        context_key: fallback
        param: fallback-source
        
  # Cash sweep verbs
  cash-sweep.configure:
    mappings:
      - entity_type: currency
        param: currency
        iterate_if_list: true
        
      - entity_type: amount
        param: threshold-amount
        
      - entity_type: sweep_vehicle
        param: vehicle-type
        default_if_missing: STIF
        
      - entity_type: time_reference
        param: sweep-time
        transform: to_24h_format
        
  # SLA verbs
  sla.commit:
    mappings:
      - entity_type: sla_template_reference
        param: template-code
        
      - entity_type: percentage
        param: override-target
        condition: differs_from_template
```

- [ ] Define mappings for all verbs
- [ ] Handle entity → parameter transforms
- [ ] Handle list expansion (iterate_if_list)
- [ ] Handle conditional mappings
- [ ] Load mappings at startup

### 22.3 Multi-Statement Generation
**File:** `rust/src/agent/multi_statement.rs`

```rust
/// Handle generation of multiple DSL statements from single intent
pub struct MultiStatementGenerator {
    single_generator: DslGenerator,
}

impl MultiStatementGenerator {
    /// Generate multiple statements when entities are lists
    pub fn expand_lists(
        &self,
        intent: &ClassifiedIntent,
        entities: &ExtractedEntities,
        context: &ConversationContext,
    ) -> Result<Vec<DslStatement>, GenerationError> {
        let verb = self.single_generator.verb_registry.get(&intent.canonical_verb)?;
        let mapping = self.get_mapping(&verb)?;
        
        // Find any list entities that should iterate
        let mut statements = Vec::new();
        let mut iteration_entities = Vec::new();
        
        for m in &mapping.mappings {
            if m.iterate_if_list {
                if let Some(entity) = entities.find_for_param(&m.param) {
                    if entity.is_list() {
                        iteration_entities.push((m.param.clone(), entity.as_list()));
                    }
                }
            }
        }
        
        if iteration_entities.is_empty() {
            // No iteration needed, single statement
            let stmt = self.single_generator.generate_for_intent(
                intent, entities, &InferredDefaults::new(), context
            )?;
            return Ok(vec![stmt]);
        }
        
        // Generate statement for each combination
        // e.g., pricing for [EQUITY, GOVT_BOND, CORP_BOND] → 3 statements
        for values in Self::cartesian_product(&iteration_entities) {
            let mut modified_entities = entities.clone();
            for (param, value) in values {
                modified_entities.set_single(&param, value);
            }
            
            let stmt = self.single_generator.generate_for_intent(
                intent, &modified_entities, &InferredDefaults::new(), context
            )?;
            statements.push(stmt);
        }
        
        Ok(statements)
    }
    
    /// Handle compound intents that expand to multiple verbs
    pub fn expand_compound(
        &self,
        intent: &ClassifiedIntent,
        entities: &ExtractedEntities,
        context: &ConversationContext,
    ) -> Result<Vec<DslStatement>, GenerationError> {
        let taxonomy = &self.single_generator.intent_taxonomy;
        
        if let Some(expansion) = taxonomy.get_expansion(&intent.intent) {
            let mut statements = Vec::new();
            
            for sub_intent_id in expansion {
                let sub_intent = ClassifiedIntent {
                    intent: taxonomy.get_intent(&sub_intent_id)?.clone(),
                    confidence: intent.confidence,
                    ..Default::default()
                };
                
                let sub_statements = self.expand_lists(
                    &sub_intent, entities, context
                )?;
                statements.extend(sub_statements);
            }
            
            Ok(statements)
        } else {
            // Not a compound intent
            self.expand_lists(intent, entities, context)
        }
    }
}
```

- [ ] Implement list expansion logic
- [ ] Implement compound intent expansion
- [ ] Handle cartesian product for multiple list params
- [ ] Preserve entity provenance in generated statements

### 22.4 DSL Validator
**File:** `rust/src/agent/dsl_validator.rs`

```rust
/// Validate generated DSL before execution
pub struct DslValidator {
    schema_validator: SchemaValidator,
    reference_data: ReferenceDataCache,
    business_rules: BusinessRuleEngine,
}

impl DslValidator {
    /// Validate DSL statements
    pub fn validate(
        &self,
        statements: &[DslStatement],
        context: &ConversationContext,
    ) -> Result<Vec<ValidatedStatement>, ValidationError> {
        let mut validated = Vec::new();
        let mut errors = Vec::new();
        
        for stmt in statements {
            match self.validate_statement(stmt, context) {
                Ok(valid) => validated.push(valid),
                Err(e) => errors.push(e),
            }
        }
        
        if !errors.is_empty() {
            return Err(ValidationError::Multiple(errors));
        }
        
        // Cross-statement validation
        self.validate_cross_statement(&validated)?;
        
        Ok(validated)
    }
    
    fn validate_statement(
        &self,
        stmt: &DslStatement,
        context: &ConversationContext,
    ) -> Result<ValidatedStatement, ValidationError> {
        // Schema validation (types, required params)
        self.schema_validator.validate(&stmt.verb, &stmt.params)?;
        
        // Reference data validation (valid market codes, etc.)
        for (param, value) in &stmt.params {
            self.validate_reference_value(param, value)?;
        }
        
        // Business rule validation
        self.business_rules.validate(stmt, context)?;
        
        Ok(ValidatedStatement {
            statement: stmt.clone(),
            validated_at: Utc::now(),
        })
    }
    
    fn validate_reference_value(&self, param: &str, value: &Value) 
        -> Result<(), ValidationError> 
    {
        match param {
            "scope-markets" => {
                if let Some(markets) = value.as_array() {
                    for market in markets {
                        let mic = market.as_str().ok_or(ValidationError::InvalidType)?;
                        if !self.reference_data.market_exists(mic) {
                            return Err(ValidationError::InvalidMarket(mic.to_string()));
                        }
                    }
                }
            }
            "scope-instrument-classes" => {
                if let Some(classes) = value.as_array() {
                    for class in classes {
                        let code = class.as_str().ok_or(ValidationError::InvalidType)?;
                        if !self.reference_data.instrument_class_exists(code) {
                            return Err(ValidationError::InvalidInstrumentClass(code.to_string()));
                        }
                    }
                }
            }
            "currency" => {
                if let Some(ccy) = value.as_str() {
                    if !self.reference_data.currency_exists(ccy) {
                        return Err(ValidationError::InvalidCurrency(ccy.to_string()));
                    }
                }
            }
            _ => {}
        }
        
        Ok(())
    }
    
    fn validate_cross_statement(&self, statements: &[ValidatedStatement]) 
        -> Result<(), ValidationError> 
    {
        // Check for conflicting IM scopes
        let im_assigns: Vec<_> = statements.iter()
            .filter(|s| s.statement.verb == "investment-manager.assign")
            .collect();
        
        // ... check for overlapping scopes with same priority
        
        Ok(())
    }
}
```

- [ ] Implement DslValidator
- [ ] Schema validation (types, required params)
- [ ] Reference data validation (valid codes)
- [ ] Business rule validation
- [ ] Cross-statement validation
- [ ] Return actionable error messages

### 22.5 Explanation Generator
**File:** `rust/src/agent/explainer.rs`

```rust
/// Generate natural language explanation of generated DSL
pub struct DslExplainer {
    templates: ExplanationTemplates,
}

impl DslExplainer {
    /// Generate explanation of what the DSL will do
    pub fn explain(&self, dsl: &GeneratedDsl) -> String {
        let mut parts = Vec::new();
        
        for stmt in &dsl.statements {
            let explanation = self.explain_statement(stmt);
            parts.push(explanation);
        }
        
        if dsl.statements.len() > 1 {
            format!("I'll execute {} actions:\n\n{}", 
                    dsl.statements.len(), 
                    parts.join("\n\n"))
        } else {
            parts.join("")
        }
    }
    
    fn explain_statement(&self, stmt: &DslStatement) -> String {
        let template = self.templates.get(&stmt.verb)
            .unwrap_or(&self.templates.default);
        
        // Fill in template with parameter values
        let mut explanation = template.clone();
        
        for (param, value) in &stmt.params {
            let display_value = self.format_value(param, value);
            explanation = explanation.replace(&format!("{{{}}}", param), &display_value);
        }
        
        // Add symbol capture explanation
        if let Some(symbol) = &stmt.capture_symbol {
            explanation.push_str(&format!("\n(This will be referenced as `{}`)", symbol));
        }
        
        explanation
    }
    
    fn format_value(&self, param: &str, value: &Value) -> String {
        match value {
            Value::Array(arr) => {
                let items: Vec<_> = arr.iter()
                    .map(|v| v.as_str().unwrap_or("?"))
                    .collect();
                items.join(", ")
            }
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => value.to_string(),
        }
    }
}
```

- [ ] Implement explanation generator
- [ ] Create templates for each verb
- [ ] Format values for readability
- [ ] Explain symbol captures
- [ ] Support multi-statement explanations

---

## Phase 23: RAG Knowledge Layer (Days 20-25)

### 23.1 Vector Store Setup
**File:** `rust/src/agent/vector_store.rs`

```rust
use qdrant_client::prelude::*;

pub struct AgentVectorStore {
    client: QdrantClient,
    embeddings: EmbeddingService,
}

impl AgentVectorStore {
    /// Initialize collections for agent knowledge
    pub async fn initialize(&self) -> Result<(), VectorStoreError> {
        // Collection: verb_examples
        // Stores example utterances mapped to DSL
        self.create_collection_if_not_exists(
            "verb_examples",
            384,  // Dimension for embedding model
        ).await?;
        
        // Collection: domain_ontology
        // Stores domain concept definitions
        self.create_collection_if_not_exists(
            "domain_ontology",
            384,
        ).await?;
        
        // Collection: conversation_history
        // Stores successful conversation turns for few-shot learning
        self.create_collection_if_not_exists(
            "conversation_history",
            384,
        ).await?;
        
        // Collection: error_corrections
        // Stores user corrections for learning
        self.create_collection_if_not_exists(
            "error_corrections",
            384,
        ).await?;
        
        Ok(())
    }
    
    /// Search for similar examples
    pub async fn search_examples(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<VerbExample>, VectorStoreError> {
        let query_embedding = self.embeddings.embed(query).await?;
        
        let results = self.client.search_points(&SearchPoints {
            collection_name: "verb_examples".to_string(),
            vector: query_embedding,
            limit: limit as u64,
            with_payload: Some(true.into()),
            ..Default::default()
        }).await?;
        
        results.result.into_iter()
            .map(|r| VerbExample::from_payload(r.payload))
            .collect()
    }
}
```

- [ ] Set up Qdrant (or alternative vector DB)
- [ ] Create collections for each knowledge type
- [ ] Implement search functions
- [ ] Implement upsert functions
- [ ] Configure embedding model

### 23.2 Verb Example Index
**File:** `rust/src/agent/verb_examples.rs`

```rust
/// Index of example utterances → DSL for few-shot learning
pub struct VerbExampleIndex {
    store: AgentVectorStore,
}

impl VerbExampleIndex {
    /// Load examples from YAML files
    pub async fn load_examples(&self, path: &Path) -> Result<usize, IndexError> {
        let mut count = 0;
        
        for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
            if entry.path().extension() == Some(OsStr::new("yaml")) {
                let examples: Vec<VerbExample> = serde_yaml::from_reader(
                    File::open(entry.path())?
                )?;
                
                for example in examples {
                    self.store.upsert_example(&example).await?;
                    count += 1;
                }
            }
        }
        
        Ok(count)
    }
    
    /// Get few-shot examples for prompt
    pub async fn get_few_shot_examples(
        &self,
        utterance: &str,
        intent: &Intent,
        limit: usize,
    ) -> Result<Vec<FewShotExample>, IndexError> {
        // Search for similar utterances
        let similar = self.store.search_examples(utterance, limit * 2).await?;
        
        // Filter to matching intent and high quality
        let filtered: Vec<_> = similar.into_iter()
            .filter(|ex| ex.intent == *intent || ex.quality_score > 0.9)
            .take(limit)
            .map(|ex| FewShotExample {
                user: ex.utterance,
                assistant_thinking: ex.reasoning,
                dsl: ex.dsl,
            })
            .collect();
        
        Ok(filtered)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerbExample {
    pub utterance: String,
    pub intent: String,
    pub entities: Vec<ExtractedEntity>,
    pub dsl: String,
    pub reasoning: String,  // Chain of thought
    pub quality_score: f32,
}
```

- [ ] Implement example loading from YAML
- [ ] Implement similarity search
- [ ] Implement few-shot example selection
- [ ] Create initial example dataset

### 23.3 Domain Ontology Index
**File:** `rust/src/agent/ontology_index.rs`

```rust
/// Index of domain concepts for grounding
pub struct DomainOntologyIndex {
    store: AgentVectorStore,
}

impl DomainOntologyIndex {
    /// Load ontology from schema and documentation
    pub async fn load_ontology(&self) -> Result<(), IndexError> {
        // Index verb definitions
        for verb in self.verb_registry.all() {
            self.store.upsert_concept(&OntologyConcept {
                concept_type: "verb",
                name: verb.full_name(),
                description: verb.description.clone(),
                related_terms: verb.trigger_phrases.clone(),
                schema: serde_json::to_string(&verb.args)?,
            }).await?;
        }
        
        // Index entity types
        for entity_type in self.entity_types.all() {
            self.store.upsert_concept(&OntologyConcept {
                concept_type: "entity_type",
                name: entity_type.name.clone(),
                description: entity_type.description.clone(),
                related_terms: entity_type.aliases.clone(),
                examples: entity_type.examples.clone(),
            }).await?;
        }
        
        // Index reference data (markets, instruments, etc.)
        // ...
        
        Ok(())
    }
    
    /// Retrieve relevant ontology context
    pub async fn get_context(
        &self,
        utterance: &str,
        intents: &[ClassifiedIntent],
    ) -> Result<OntologyContext, IndexError> {
        let mut context = OntologyContext::new();
        
        // Get verb definitions for classified intents
        for intent in intents {
            let verb = self.store.search_concept(
                &intent.canonical_verb, 
                "verb", 
                1
            ).await?;
            if let Some(v) = verb.first() {
                context.add_verb(v);
            }
        }
        
        // Get relevant entity type definitions
        let entity_context = self.store.search_concepts(
            utterance,
            "entity_type",
            5,
        ).await?;
        context.add_entity_types(entity_context);
        
        Ok(context)
    }
}
```

- [ ] Implement ontology loading
- [ ] Index verb definitions
- [ ] Index entity types
- [ ] Index reference data summaries
- [ ] Implement context retrieval

### 23.4 Conversation History Learning
**File:** `rust/src/agent/history_learner.rs`

```rust
/// Learn from successful conversations
pub struct ConversationLearner {
    store: AgentVectorStore,
}

impl ConversationLearner {
    /// Record successful conversation turn for learning
    pub async fn record_success(
        &self,
        turn: &ProcessedTurn,
        user_feedback: Option<Feedback>,
    ) -> Result<(), LearnerError> {
        // Only learn from successful executions
        if !turn.execution_succeeded() {
            return Ok(());
        }
        
        // Calculate quality score
        let quality = self.calculate_quality(turn, user_feedback);
        
        // Store as example
        let example = HistoricalExample {
            utterance: turn.user_message.clone(),
            intents: turn.intents.clone(),
            entities: turn.entities.clone(),
            generated_dsl: turn.dsl.clone(),
            quality_score: quality,
            timestamp: Utc::now(),
        };
        
        self.store.upsert_history(&example).await?;
        
        // If high quality, also add to verb examples
        if quality > 0.9 {
            self.promote_to_example(&example).await?;
        }
        
        Ok(())
    }
    
    /// Learn from user corrections
    pub async fn record_correction(
        &self,
        original: &ProcessedTurn,
        corrected_dsl: &str,
        correction_type: CorrectionType,
    ) -> Result<(), LearnerError> {
        let correction = Correction {
            original_utterance: original.user_message.clone(),
            original_intents: original.intents.clone(),
            original_dsl: original.dsl.clone(),
            corrected_dsl: corrected_dsl.to_string(),
            correction_type,
            timestamp: Utc::now(),
        };
        
        self.store.upsert_correction(&correction).await?;
        
        // Analyze correction to update model
        self.analyze_correction(&correction).await?;
        
        Ok(())
    }
    
    fn calculate_quality(
        &self,
        turn: &ProcessedTurn,
        feedback: Option<Feedback>,
    ) -> f32 {
        let mut score = 0.5;  // Base score
        
        // Boost for successful execution
        if turn.execution_succeeded() {
            score += 0.2;
        }
        
        // Boost for positive feedback
        if let Some(Feedback::Positive) = feedback {
            score += 0.2;
        }
        
        // Boost for no corrections needed
        if !turn.was_corrected {
            score += 0.1;
        }
        
        score.min(1.0)
    }
}
```

- [ ] Implement success recording
- [ ] Implement correction recording
- [ ] Implement quality scoring
- [ ] Implement promotion to examples
- [ ] Implement correction analysis

---

## Phase 24: Clarification & Confirmation (Days 25-28)

### 24.1 Clarification Generator
**File:** `rust/src/agent/clarification.rs`

```rust
/// Generate clarifying questions when intent/entities are ambiguous
pub struct ClarificationGenerator {
    templates: ClarificationTemplates,
}

impl ClarificationGenerator {
    /// Generate clarification for ambiguous intent
    pub fn clarify_intent(
        &self,
        ambiguous: &[ClassifiedIntent],
        utterance: &str,
    ) -> ClarificationQuestion {
        if ambiguous.len() == 2 {
            // Binary choice
            ClarificationQuestion {
                question: format!(
                    "I want to make sure I understand. Are you asking me to {} or {}?",
                    self.describe_intent(&ambiguous[0]),
                    self.describe_intent(&ambiguous[1]),
                ),
                options: ambiguous.iter()
                    .map(|i| ClarificationOption {
                        label: self.describe_intent(i),
                        resolves_to: i.clone(),
                    })
                    .collect(),
                question_type: QuestionType::Choice,
            }
        } else {
            // More complex ambiguity
            ClarificationQuestion {
                question: "I'm not sure what you'd like me to do. Could you clarify?".to_string(),
                options: vec![],
                question_type: QuestionType::OpenEnded,
            }
        }
    }
    
    /// Generate clarification for missing required entity
    pub fn clarify_entity(
        &self,
        missing: &str,
        intent: &ClassifiedIntent,
        context: &ConversationContext,
    ) -> ClarificationQuestion {
        let question = match missing {
            "manager-name" | "manager-lei" => {
                "Which investment manager should I assign? (You can provide a name or LEI)"
            }
            "scope-markets" => {
                "Which markets should this IM be allowed to trade in? \
                 (e.g., 'European markets', 'NYSE and NASDAQ', specific MIC codes)"
            }
            "scope-instrument-classes" => {
                "What types of instruments? \
                 (e.g., 'equities', 'fixed income', 'derivatives')"
            }
            "currency" => {
                "Which currency?"
            }
            "threshold-amount" => {
                "What's the threshold amount for the sweep? (e.g., '100k', '50000')"
            }
            _ => {
                &format!("What value should I use for {}?", missing)
            }
        };
        
        ClarificationQuestion {
            question: question.to_string(),
            options: self.suggest_options(missing, context),
            question_type: QuestionType::FreeTextWithSuggestions,
        }
    }
    
    fn suggest_options(
        &self,
        param: &str,
        context: &ConversationContext,
    ) -> Vec<ClarificationOption> {
        // Suggest based on context
        match param {
            "scope-markets" => {
                vec![
                    ClarificationOption::quick("All markets"),
                    ClarificationOption::quick("European markets"),
                    ClarificationOption::quick("US markets"),
                    ClarificationOption::quick("APAC markets"),
                ]
            }
            "instruction-method" => {
                vec![
                    ClarificationOption::quick("SWIFT"),
                    ClarificationOption::quick("CTM"),
                    ClarificationOption::quick("FIX"),
                    ClarificationOption::quick("API"),
                ]
            }
            _ => vec![],
        }
    }
}
```

- [ ] Implement intent clarification
- [ ] Implement entity clarification
- [ ] Create clarification templates
- [ ] Implement suggestion generation
- [ ] Track clarification → resolution for learning

### 24.2 Confirmation Generator
**File:** `rust/src/agent/confirmation.rs`

```rust
/// Generate confirmation requests before execution
pub struct ConfirmationGenerator {
    explainer: DslExplainer,
}

impl ConfirmationGenerator {
    /// Generate confirmation for DSL execution
    pub fn generate_confirmation(
        &self,
        dsl: &GeneratedDsl,
        context: &ConversationContext,
    ) -> ConfirmationRequest {
        let explanation = self.explainer.explain(dsl);
        
        let impact = self.assess_impact(dsl, context);
        
        ConfirmationRequest {
            message: format!(
                "I'm about to:\n\n{}\n\n{}",
                explanation,
                self.impact_warning(&impact),
            ),
            dsl_preview: dsl.to_display_string(),
            impact,
            options: vec![
                ConfirmOption::Confirm,
                ConfirmOption::Edit,
                ConfirmOption::Cancel,
            ],
        }
    }
    
    fn assess_impact(
        &self,
        dsl: &GeneratedDsl,
        context: &ConversationContext,
    ) -> Impact {
        let mut impact = Impact::default();
        
        for stmt in &dsl.statements {
            match stmt.verb.as_str() {
                "investment-manager.assign" => {
                    impact.creates_im = true;
                }
                "investment-manager.terminate" => {
                    impact.terminates_im = true;
                    impact.is_destructive = true;
                }
                "trading-profile.materialize" => {
                    impact.affects_production = true;
                }
                "sla.commit" => {
                    impact.creates_sla = true;
                }
                _ => {}
            }
        }
        
        impact
    }
    
    fn impact_warning(&self, impact: &Impact) -> &'static str {
        if impact.is_destructive {
            "⚠️ This action cannot be easily undone."
        } else if impact.affects_production {
            "⚡ This will update operational systems."
        } else {
            "Shall I proceed?"
        }
    }
}
```

- [ ] Implement confirmation generation
- [ ] Implement impact assessment
- [ ] Define confirmation thresholds
- [ ] Handle user responses

### 24.3 Conversation Flow Controller
**File:** `rust/src/agent/flow_controller.rs`

```rust
/// Control conversation flow state
pub struct FlowController {
    clarifier: ClarificationGenerator,
    confirmer: ConfirmationGenerator,
}

impl FlowController {
    /// Determine next action in conversation
    pub fn determine_action(
        &self,
        classification: &ClassificationResult,
        extraction: &ExtractionResult,
        generation: &GenerationResult,
        context: &ConversationContext,
    ) -> FlowAction {
        // Check for clarification needs
        if classification.needs_clarification() {
            return FlowAction::Clarify(
                self.clarifier.clarify_intent(
                    &classification.ambiguous_intents,
                    &classification.utterance,
                )
            );
        }
        
        // Check for missing entities
        if let Some(missing) = extraction.missing_required() {
            return FlowAction::ClarifyEntity(
                self.clarifier.clarify_entity(
                    &missing,
                    &classification.best_intent(),
                    context,
                )
            );
        }
        
        // Check for validation errors
        if let Some(errors) = generation.validation_errors() {
            return FlowAction::ReportErrors(errors);
        }
        
        // Check if confirmation needed
        let confidence = classification.best_confidence();
        let impact = generation.impact();
        
        if self.needs_confirmation(confidence, impact) {
            return FlowAction::Confirm(
                self.confirmer.generate_confirmation(
                    &generation.dsl,
                    context,
                )
            );
        }
        
        // Ready to execute
        FlowAction::Execute(generation.dsl.clone())
    }
    
    fn needs_confirmation(&self, confidence: f32, impact: &Impact) -> bool {
        // Always confirm destructive actions
        if impact.is_destructive {
            return true;
        }
        
        // Confirm production changes
        if impact.affects_production {
            return true;
        }
        
        // Confirm if confidence is medium
        if confidence < 0.85 {
            return true;
        }
        
        false
    }
}

pub enum FlowAction {
    Clarify(ClarificationQuestion),
    ClarifyEntity(ClarificationQuestion),
    Confirm(ConfirmationRequest),
    Execute(GeneratedDsl),
    ReportErrors(Vec<ValidationError>),
}
```

- [ ] Implement flow controller
- [ ] Define action determination logic
- [ ] Handle state transitions
- [ ] Track conversation flow for debugging

---

## Phase 25: Integration & Testing (Days 28-35)

### 25.1 Agent Pipeline Integration
**File:** `rust/src/agent/pipeline.rs`

```rust
/// Complete agent processing pipeline
pub struct AgentPipeline {
    intent_classifier: IntentClassifier,
    entity_extractor: EntityExtractor,
    default_inferencer: DefaultInferencer,
    dsl_generator: DslGenerator,
    flow_controller: FlowController,
    context_manager: ContextManager,
    executor: DslExecutor,
    learner: ConversationLearner,
}

impl AgentPipeline {
    /// Process user message through complete pipeline
    pub async fn process(
        &self,
        message: &str,
        session_id: Uuid,
    ) -> Result<AgentResponse, PipelineError> {
        // Load conversation context
        let mut context = self.context_manager.load(session_id).await?;
        
        // Step 1: Classify intent
        let classification = self.intent_classifier
            .classify(message, &context)
            .await?;
        
        // Step 2: Extract entities
        let extraction = self.entity_extractor
            .extract(message, &context)
            .await?;
        
        // Step 3: Infer defaults
        let defaults = self.default_inferencer
            .infer_defaults(&classification.best_intent(), &extraction, &context)?;
        
        // Step 4: Generate DSL
        let generation = self.dsl_generator
            .generate(
                &classification.intents,
                &extraction.entities,
                &defaults,
                &context,
            )
            .await?;
        
        // Step 5: Determine flow action
        let action = self.flow_controller.determine_action(
            &classification,
            &extraction,
            &generation,
            &context,
        );
        
        // Step 6: Execute action
        let response = match action {
            FlowAction::Clarify(question) => {
                AgentResponse::clarification(question)
            }
            FlowAction::ClarifyEntity(question) => {
                AgentResponse::clarification(question)
            }
            FlowAction::Confirm(request) => {
                context.pending_confirmation = Some(request.clone());
                AgentResponse::confirmation(request)
            }
            FlowAction::Execute(dsl) => {
                let result = self.executor.execute(&dsl, &mut context).await?;
                
                // Learn from success
                self.learner.record_success(&ProcessedTurn {
                    user_message: message.to_string(),
                    intents: classification.intents,
                    entities: extraction.entities,
                    dsl: dsl.clone(),
                    result: result.clone(),
                    ..Default::default()
                }, None).await?;
                
                AgentResponse::execution(result, &dsl)
            }
            FlowAction::ReportErrors(errors) => {
                AgentResponse::errors(errors)
            }
        };
        
        // Update context
        context.update_from_turn(&ProcessedTurn {
            user_message: message.to_string(),
            // ... etc
        });
        self.context_manager.save(session_id, &context).await?;
        
        Ok(response)
    }
}
```

- [ ] Implement complete pipeline
- [ ] Wire all components together
- [ ] Handle errors at each stage
- [ ] Implement response formatting

### 25.2 Comprehensive Test Suite
**File:** `rust/tests/agent_tests.rs`

```rust
#[tokio::test]
async fn test_simple_im_assignment() {
    let pipeline = create_test_pipeline().await;
    
    let response = pipeline.process(
        "Add BlackRock as our investment manager for European equities using CTM",
        test_session_id(),
    ).await.unwrap();
    
    assert!(response.is_execution());
    let dsl = response.executed_dsl();
    assert!(dsl.contains("investment-manager.assign"));
    assert!(dsl.contains("BlackRock"));
    assert!(dsl.contains("CTM"));
    assert!(dsl.contains("XLON") || dsl.contains("European"));
}

#[tokio::test]
async fn test_multi_intent_utterance() {
    let pipeline = create_test_pipeline().await;
    
    let response = pipeline.process(
        "BlackRock handles equities, PIMCO does fixed income, and use Bloomberg for pricing",
        test_session_id(),
    ).await.unwrap();
    
    // Should generate 3+ DSL statements
    let dsl = response.executed_dsl();
    assert!(dsl.matches("investment-manager.assign").count() >= 2);
    assert!(dsl.contains("pricing-config.set"));
}

#[tokio::test]
async fn test_clarification_on_ambiguity() {
    let pipeline = create_test_pipeline().await;
    
    let response = pipeline.process(
        "Set up the manager",  // Ambiguous - which manager? What kind of setup?
        test_session_id(),
    ).await.unwrap();
    
    assert!(response.is_clarification());
}

#[tokio::test]
async fn test_entity_expansion() {
    let pipeline = create_test_pipeline().await;
    
    let response = pipeline.process(
        "Allow trading in all European markets",
        test_session_id(),
    ).await.unwrap();
    
    let dsl = response.executed_dsl();
    // Should expand "European" to specific markets
    assert!(dsl.contains("XLON"));
    assert!(dsl.contains("XETR"));
}

#[tokio::test]
async fn test_context_continuity() {
    let pipeline = create_test_pipeline().await;
    let session = test_session_id();
    
    // First turn - establish IM
    pipeline.process(
        "Add BlackRock as our equity IM",
        session,
    ).await.unwrap();
    
    // Second turn - reference previous
    let response = pipeline.process(
        "Now connect them via CTM",  // "them" should resolve to BlackRock
        session,
    ).await.unwrap();
    
    let dsl = response.executed_dsl();
    assert!(dsl.contains("BlackRock") || dsl.contains("@im-blackrock"));
}

#[tokio::test]
async fn test_validation_error() {
    let pipeline = create_test_pipeline().await;
    
    let response = pipeline.process(
        "Set up IM for trading in INVALID_MARKET",
        test_session_id(),
    ).await.unwrap();
    
    assert!(response.is_error());
    assert!(response.errors()[0].message.contains("market"));
}
```

- [ ] Create comprehensive test suite
- [ ] Test simple single-intent cases
- [ ] Test multi-intent utterances
- [ ] Test clarification triggering
- [ ] Test entity expansion
- [ ] Test context continuity
- [ ] Test validation errors
- [ ] Test confirmation flow

### 25.3 Evaluation Dataset
**File:** `rust/config/agent/evaluation_dataset.yaml`

```yaml
# Golden dataset for agent evaluation
evaluation_cases:
  - id: simple_im_1
    category: investment_manager
    difficulty: easy
    input: "Add Vanguard as our US equity manager"
    expected_intents: [im_assign]
    expected_entities:
      manager_reference: "Vanguard"
      scope_expression:
        markets: ["XNYS", "XNAS"]
        instruments: ["EQUITY"]
    expected_dsl_contains:
      - "investment-manager.assign"
      - "Vanguard"
      - "EQUITY"
      
  - id: multi_im_1
    category: investment_manager
    difficulty: medium
    input: "BlackRock for European equities via CTM, PIMCO for bonds via SWIFT"
    expected_intents: [im_assign, im_assign]
    expected_entities:
      - manager_reference: "BlackRock"
        scope_expression: { markets: ["XLON", "XETR", "XPAR"], instruments: ["EQUITY"] }
        instruction_method: "CTM"
      - manager_reference: "PIMCO"
        scope_expression: { instruments: ["GOVT_BOND", "CORP_BOND"] }
        instruction_method: "SWIFT"
        
  - id: complex_setup_1
    category: compound
    difficulty: hard
    input: |
      Set up our Luxembourg fund with three managers:
      - European equities handled by AllianzGI via CTM
      - US handled by Vanguard via SWIFT
      - Everything else by our internal team
      Use Bloomberg for all pricing and sweep EUR/USD to STIF.
    expected_intents: [im_assign, im_assign, im_assign, pricing_set, sweep_configure, sweep_configure]
    # ... detailed expectations
```

- [ ] Create evaluation dataset (100+ cases)
- [ ] Cover all intent types
- [ ] Include edge cases
- [ ] Include difficult cases
- [ ] Run automated evaluation
- [ ] Track metrics over time

### 25.4 Performance Metrics
**File:** `rust/src/agent/metrics.rs`

```rust
pub struct AgentMetrics {
    // Intent classification metrics
    intent_accuracy: f32,
    intent_precision: HashMap<String, f32>,
    intent_recall: HashMap<String, f32>,
    
    // Entity extraction metrics
    entity_f1: HashMap<String, f32>,
    
    // Generation metrics
    dsl_validity_rate: f32,
    execution_success_rate: f32,
    
    // User experience metrics
    clarification_rate: f32,
    correction_rate: f32,
    turns_to_completion: f32,
}

impl AgentMetrics {
    pub fn evaluate(&mut self, dataset: &EvaluationDataset) {
        for case in &dataset.cases {
            let result = self.pipeline.process(&case.input, test_session()).await;
            
            // Check intent accuracy
            self.check_intents(&result, &case.expected_intents);
            
            // Check entity extraction
            self.check_entities(&result, &case.expected_entities);
            
            // Check DSL generation
            self.check_dsl(&result, &case.expected_dsl_contains);
        }
        
        self.compute_aggregates();
    }
}
```

- [ ] Define metrics to track
- [ ] Implement evaluation harness
- [ ] Create baseline measurements
- [ ] Set up continuous evaluation
- [ ] Dashboard for metrics

---

## Phase 26: Documentation & Deployment (Days 35-40)

### 26.1 Agent Architecture Documentation
**File:** `docs/AGENT_ARCHITECTURE.md`

Document:
- [ ] Overall architecture diagram
- [ ] Component responsibilities
- [ ] Data flow through pipeline
- [ ] Configuration options
- [ ] Extension points

### 26.2 Intent Taxonomy Documentation
**File:** `docs/AGENT_INTENTS.md`

Document:
- [ ] All supported intents
- [ ] Example utterances per intent
- [ ] Required vs optional entities
- [ ] Related intents

### 26.3 Tuning Guide
**File:** `docs/AGENT_TUNING.md`

Document:
- [ ] How to add new intents
- [ ] How to add new entity types
- [ ] How to add verb examples
- [ ] How to tune confidence thresholds
- [ ] How to analyze errors

### 26.4 Deployment Configuration
**File:** `rust/config/agent/production.yaml`

```yaml
agent:
  # Embedding service
  embeddings:
    model: "text-embedding-3-small"
    batch_size: 32
    cache_ttl: 3600
    
  # Vector store
  vector_store:
    type: qdrant
    url: ${QDRANT_URL}
    api_key: ${QDRANT_API_KEY}
    
  # Classification
  classification:
    confidence_threshold: 0.75
    confirm_threshold: 0.85
    max_ambiguous_intents: 3
    
  # Session management
  sessions:
    store_type: redis
    ttl_seconds: 3600
    max_history_turns: 50
    
  # Learning
  learning:
    enabled: true
    min_quality_for_promotion: 0.9
    correction_analysis_enabled: true
```

- [ ] Create production configuration
- [ ] Document all configuration options
- [ ] Create deployment scripts
- [ ] Set up monitoring

---

## Success Criteria

Phase 3 (Agent Intelligence) is complete when:

1. ✅ Intent classifier achieves >90% accuracy on evaluation dataset
2. ✅ Entity extractor achieves >85% F1 on all entity types
3. ✅ DSL generator produces valid DSL >95% of the time
4. ✅ Agent handles multi-intent utterances correctly
5. ✅ Context continuity works across conversation turns
6. ✅ Clarification triggers appropriately on ambiguity
7. ✅ Confirmation triggers appropriately on high-impact actions
8. ✅ Learning from corrections demonstrably improves accuracy
9. ✅ Response latency <2 seconds for typical requests
10. ✅ Can complete full trading matrix setup through conversation

---

## Notes for Claude Code

**Critical Path:**
1. Intent taxonomy (19.1) → Intent classifier (19.2) - Can't classify without taxonomy
2. Entity types (20.1) → Entity extractor (20.2) - Can't extract without types
3. Both above → DSL generator (22.x) - Generation needs both
4. All above → Pipeline integration (25.1)

**Start Simple:**
- Begin with pattern-based intent matching, add embeddings later
- Begin with regex entity extraction, add semantic later
- Get pipeline working end-to-end, then improve each component

**Evaluation-Driven:**
- Create evaluation dataset early (25.3)
- Run evaluations frequently
- Let metrics guide optimization

**Embedding Model:**
- Start with OpenAI `text-embedding-3-small` or Anthropic embeddings
- Can switch to local model later if needed

**Vector Store:**
- Qdrant is good choice, but can start with in-memory HNSW
- Only need persistence for production

**The Key Insight:**
The agent's job is to translate natural language → DSL. Everything else (clarification, confirmation, learning) exists to make that translation more accurate and trustworthy.
