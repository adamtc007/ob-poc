# Phase 3 Agent Intelligence - Baseline Completion TODO

**Purpose:** Complete the Phase 3 agent intelligence implementation to establish a clean, working baseline capable of conversational trading matrix capture.

**Success Criteria:**
1. All defined intents can generate valid DSL
2. Generated DSL executes against the database
3. Evaluation dataset passes at 85%+ accuracy
4. End-to-end demo: Natural language → DSL → Execution → Confirmation

**Estimated Effort:** 8-12 days

---

## Phase 3.1: Wire Execution Pipeline (Priority 1)

**Goal:** Connect agent pipeline to actual DSL execution so generated DSL runs against the database.

### Task 3.1.1: Async Execution Support
**File:** `rust/src/agentic/pipeline.rs`

- [ ] Add `tokio` async runtime support to `AgentPipeline`
- [ ] Change `process()` to `async fn process()`
- [ ] Change `execute_dsl()` to `async fn execute_dsl()`
- [ ] Update `do_execute()` to actually call the DSL executor

```rust
// Current (stubbed):
fn do_execute(&self, _executor: &DslExecutor, dsl: &str) -> Result<TurnExecutionResult, PipelineError> {
    // For now, return a placeholder
    Ok(TurnExecutionResult { success: true, bindings: vec![], error: None })
}

// Required:
async fn do_execute(&self, executor: &DslExecutor, dsl: &str, pool: &PgPool) -> Result<TurnExecutionResult, PipelineError> {
    // Parse DSL
    let program = parse_program(dsl).map_err(|e| PipelineError::Execution(format!("Parse: {}", e)))?;
    
    // Compile to execution plan
    let plan = compile(&program).map_err(|e| PipelineError::Execution(format!("Compile: {}", e)))?;
    
    // Execute with database
    let mut ctx = ExecutionContext::new();
    for step in plan.steps {
        executor.execute_step(&step, &mut ctx, pool).await
            .map_err(|e| PipelineError::Execution(e.to_string()))?;
    }
    
    Ok(TurnExecutionResult {
        success: true,
        bindings: ctx.symbols.iter().map(|(k, v)| (k.clone(), v.to_string())).collect(),
        error: None,
    })
}
```

### Task 3.1.2: Database Pool Integration
**File:** `rust/src/agentic/pipeline.rs`

- [ ] Store `PgPool` in `AgentPipeline` struct
- [ ] Add `with_database(pool: PgPool)` builder method
- [ ] Ensure connection is available during execution

### Task 3.1.3: Transaction Support
**File:** `rust/src/agentic/pipeline.rs`

- [ ] Wrap multi-statement execution in a transaction
- [ ] Rollback on any statement failure
- [ ] Return partial results with error details

```rust
async fn execute_dsl_transactional(&self, dsl: &str, session_id: Uuid) -> Result<AgentResponse, PipelineError> {
    let mut tx = self.pool.begin().await.map_err(|e| PipelineError::Execution(e.to_string()))?;
    
    // Execute all statements
    let result = self.do_execute_in_tx(dsl, &mut tx).await;
    
    match result {
        Ok(r) => {
            tx.commit().await.map_err(|e| PipelineError::Execution(e.to_string()))?;
            Ok(r)
        }
        Err(e) => {
            tx.rollback().await.ok(); // Best effort rollback
            Err(e)
        }
    }
}
```

### Task 3.1.4: Binding Propagation
**File:** `rust/src/agentic/pipeline.rs`

- [ ] After successful execution, update session with created symbols
- [ ] Map database UUIDs back to DSL symbols
- [ ] Make symbols available for subsequent turns

```rust
// After execution succeeds:
if let Some(session) = self.sessions.get_mut(&session_id) {
    for (symbol, uuid) in &result.bindings {
        session.bindings.insert(symbol.clone(), uuid.clone());
        // Also update session_entities for coreference
        if symbol.starts_with("@im-") {
            session.session_entities.insert("manager_reference".to_string(), symbol.clone());
        }
    }
}
```

### Task 3.1.5: Execution Tests
**File:** `rust/src/agentic/pipeline_tests.rs`

- [ ] Add integration test with test database
- [ ] Test single statement execution
- [ ] Test multi-statement transaction
- [ ] Test rollback on failure
- [ ] Test symbol propagation across turns

---

## Phase 3.2: Complete Parameter Mappings (Priority 2)

**Goal:** Every intent in `intent_taxonomy.yaml` must have a corresponding mapping in `parameter_mappings.yaml`.

### Task 3.2.1: Investment Manager Mappings
**File:** `rust/config/agent/parameter_mappings.yaml`

Add mappings for:

- [ ] `investment-manager.set-scope` (im_update_scope)
```yaml
investment-manager.set-scope:
  description: "Update scope for existing IM assignment"
  mappings:
    - entity_type: im_assignment_reference
      param: assignment-id
      required: true
      source: context_or_extract
    - entity_type: market_reference
      param: add-markets
      is_list: true
      condition: scope_action_is_add
    - entity_type: market_reference
      param: remove-markets
      is_list: true
      condition: scope_action_is_remove
    - entity_type: instrument_class_reference
      param: add-instrument-classes
      is_list: true
    - entity_type: instrument_class_reference
      param: remove-instrument-classes
      is_list: true
```

- [ ] `investment-manager.link-connectivity` (im_change_connectivity)
```yaml
investment-manager.link-connectivity:
  description: "Link IM to connectivity resource"
  mappings:
    - entity_type: im_assignment_reference
      param: assignment-id
      required: true
      source: context_or_extract
    - entity_type: instruction_method
      param: instruction-method
      required: true
    - entity_type: resource_reference
      param: resource-id
      source: context
      infer_from: instruction_method
      inference_rules:
        CTM: "@ctm-resource"
        FIX: "@fix-resource"
        SWIFT: "@swift-resource"
```

- [ ] `investment-manager.remove` (im_remove)
```yaml
investment-manager.remove:
  description: "Remove IM assignment"
  mappings:
    - entity_type: im_assignment_reference
      param: assignment-id
      required: true
      source: context_or_extract
    - entity_type: removal_reason
      param: reason
      default_if_missing: "CLIENT_REQUEST"
  confirmation_required: true
```

- [ ] `investment-manager.find-for-trade` (im_find_for_trade)
```yaml
investment-manager.find-for-trade:
  description: "Find which IM would handle a trade"
  mappings:
    - entity_type: cbu_reference
      param: cbu-id
      source: context
      fallback: session.current_cbu
    - entity_type: market_reference
      param: market
      required: true
    - entity_type: instrument_class_reference
      param: instrument-class
      required: true
    - entity_type: currency
      param: currency
  is_query: true
```

### Task 3.2.2: Pricing Mappings
**File:** `rust/config/agent/parameter_mappings.yaml`

- [ ] `pricing-config.list` (pricing_query)
```yaml
pricing-config.list:
  description: "List pricing configurations"
  mappings:
    - entity_type: cbu_reference
      param: cbu-id
      source: context
      fallback: session.current_cbu
    - entity_type: instrument_class_reference
      param: filter-instrument-class
    - entity_type: market_reference
      param: filter-market
  is_query: true
```

### Task 3.2.3: Cash Sweep Mappings
**File:** `rust/config/agent/parameter_mappings.yaml`

- [ ] `cash-sweep.list` (sweep_query)
```yaml
cash-sweep.list:
  description: "List cash sweep configurations"
  mappings:
    - entity_type: cbu_reference
      param: cbu-id
      source: context
      fallback: session.current_cbu
    - entity_type: currency
      param: filter-currency
  is_query: true
```

- [ ] `cash-sweep.update-timing` already exists but verify completeness

### Task 3.2.4: SLA Mappings
**File:** `rust/config/agent/parameter_mappings.yaml`

- [ ] `sla.apply-template` (sla_use_template)
```yaml
sla.apply-template:
  description: "Apply SLA template to CBU"
  mappings:
    - entity_type: cbu_reference
      param: cbu-id
      source: context
      fallback: session.current_cbu
    - entity_type: sla_template_name
      param: template-name
      required: true
    - entity_type: market_reference
      param: scope-markets
      is_list: true
    - entity_type: instrument_class_reference
      param: scope-instrument-classes
      is_list: true
  symbol_template: "@sla-{template-name}"
```

- [ ] `sla.list-commitments` (sla_query) - verify exists

### Task 3.2.5: Trading Profile Mappings
**File:** `rust/config/agent/parameter_mappings.yaml`

- [ ] `trading-profile.create` (profile_create)
```yaml
trading-profile.create:
  description: "Create new trading profile"
  mappings:
    - entity_type: cbu_reference
      param: cbu-id
      source: context
      fallback: session.current_cbu
    - entity_type: profile_name
      param: name
      default_if_missing: "Trading Profile"
    - entity_type: currency
      param: base-currency
      default_if_missing: "USD"
  defaults:
    status: DRAFT
  symbol_template: "@profile"
```

- [ ] `trading-profile.set-universe` (profile_set_universe)
```yaml
trading-profile.set-universe:
  description: "Define tradeable universe"
  mappings:
    - entity_type: profile_reference
      param: profile-id
      source: context
      fallback: session.current_profile
    - entity_type: market_reference
      param: markets
      is_list: true
      required: true
    - entity_type: instrument_class_reference
      param: instrument-classes
      is_list: true
      required: true
    - entity_type: currency
      param: currencies
      is_list: true
```

- [ ] `trading-profile.visualize` (profile_visualize)
```yaml
trading-profile.visualize:
  description: "Generate trading matrix visualization"
  mappings:
    - entity_type: profile_reference
      param: profile-id
      source: context
      fallback: session.current_profile
    - entity_type: visualization_format
      param: format
      default_if_missing: "GRID"
  is_query: true
```

- [ ] `trading-profile.validate-matrix` (profile_validate)
```yaml
trading-profile.validate-matrix:
  description: "Validate trading profile for gaps"
  mappings:
    - entity_type: profile_reference
      param: profile-id
      source: context
      fallback: session.current_profile
  is_query: true
```

### Task 3.2.6: Custody/SSI Mappings
**File:** `rust/config/agent/parameter_mappings.yaml`

- [ ] `cbu-custody.ensure-ssi` (ssi_create)
```yaml
cbu-custody.ensure-ssi:
  description: "Create or ensure SSI exists"
  mappings:
    - entity_type: cbu_reference
      param: cbu-id
      source: context
      fallback: session.current_cbu
    - entity_type: ssi_type
      param: ssi-type
      required: true
    - entity_type: market_reference
      param: market
    - entity_type: currency
      param: currency
    - entity_type: bic
      param: custodian-bic
    - entity_type: account_number
      param: account-number
    - entity_type: settlement_type
      param: settlement-type
      default_if_missing: "DVP"
  symbol_template: "@ssi-{market}-{currency}"
  symbol_transform: lowercase_hyphenate
```

- [ ] `cbu-custody.ensure-booking-rule` (booking_rule_create)
```yaml
cbu-custody.ensure-booking-rule:
  description: "Create booking rule for SSI routing"
  mappings:
    - entity_type: cbu_reference
      param: cbu-id
      source: context
      fallback: session.current_cbu
    - entity_type: ssi_reference
      param: ssi-id
      required: true
    - entity_type: instrument_class_reference
      param: instrument-class
    - entity_type: market_reference
      param: market
    - entity_type: currency
      param: currency
    - entity_type: settlement_type
      param: settlement-type
  defaults:
    priority: 100
```

- [ ] `cbu-custody.add-universe` (universe_add)
```yaml
cbu-custody.add-universe:
  description: "Add to trading universe"
  mappings:
    - entity_type: cbu_reference
      param: cbu-id
      source: context
      fallback: session.current_cbu
    - entity_type: market_reference
      param: market
      required: true
    - entity_type: instrument_class_reference
      param: instrument-class
      required: true
    - entity_type: currency
      param: currencies
      is_list: true
    - entity_type: settlement_type
      param: settlement-types
      is_list: true
```

### Task 3.2.7: Add Missing Entity Types
**File:** `rust/config/agent/entity_types.yaml`

Add any entity types referenced in new mappings but not yet defined:

- [ ] `im_assignment_reference` - Reference to existing IM assignment
- [ ] `profile_reference` - Reference to trading profile
- [ ] `ssi_type` - Type of SSI (CUSTODY, CASH, INCOME)
- [ ] `account_number` - Account number pattern
- [ ] `removal_reason` - Reason codes for removal
- [ ] `visualization_format` - GRID, LIST, TREE
- [ ] `scope_action` - ADD, REMOVE, REPLACE
- [ ] `sla_template_name` - Names of SLA templates

### Task 3.2.8: Mapping Validation Test
**File:** `rust/src/agentic/dsl_generator.rs` (add test)

- [ ] Add test that loads real config files and verifies every intent has a mapping
```rust
#[test]
fn test_all_intents_have_mappings() {
    let taxonomy = IntentTaxonomy::load_from_file(Path::new("config/agent/intent_taxonomy.yaml")).unwrap();
    let mappings = ParameterMappingsConfig::load_from_file(Path::new("config/agent/parameter_mappings.yaml")).unwrap();
    
    let mut missing = Vec::new();
    for intent in taxonomy.all_intents() {
        if let Some(verb) = &intent.canonical_verb {
            if mappings.get_mapping(verb).is_none() {
                missing.push(format!("{} -> {}", intent.intent, verb));
            }
        }
    }
    
    assert!(missing.is_empty(), "Missing mappings:\n{}", missing.join("\n"));
}
```

---

## Phase 3.3: Add LLM Fallback (Priority 3)

**Goal:** When pattern matching fails, use LLM to classify intent and extract entities.

### Task 3.3.1: LLM Client Abstraction
**File:** `rust/src/agentic/llm_client.rs`

- [ ] Verify `LlmClient` trait is complete
- [ ] Add method for structured extraction:
```rust
#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(&self, prompt: &str, system: Option<&str>) -> Result<String, LlmError>;
    
    async fn extract_structured<T: DeserializeOwned>(
        &self, 
        prompt: &str, 
        schema: &str,
        system: Option<&str>
    ) -> Result<T, LlmError>;
}
```

### Task 3.3.2: Intent Classification Prompt
**File:** `rust/src/agentic/prompts/intent_classification.md` (new file)

- [ ] Create prompt template for LLM intent classification:
```markdown
# Intent Classification

You are classifying user utterances for a trading matrix configuration system.

## Available Intents
{{#each intents}}
- **{{this.intent}}**: {{this.description}}
  Examples: {{#each this.trigger_phrases}}{{this}}{{#unless @last}}, {{/unless}}{{/each}}
{{/each}}

## User Utterance
"{{utterance}}"

## Task
Classify this utterance into one or more intents. Return JSON:
```json
{
  "intents": [
    {"intent_id": "...", "confidence": 0.0-1.0, "reasoning": "..."}
  ],
  "needs_clarification": boolean,
  "clarification_question": "..." // if needs_clarification
}
```
```

### Task 3.3.3: Entity Extraction Prompt
**File:** `rust/src/agentic/prompts/entity_extraction.md` (new file)

- [ ] Create prompt template for LLM entity extraction:
```markdown
# Entity Extraction

Extract domain entities from this utterance for a trading matrix configuration system.

## Entity Types
{{#each entity_types}}
- **{{@key}}**: {{this.description}}
  Valid patterns: {{#each this.patterns}}{{this.type}}{{#unless @last}}, {{/unless}}{{/each}}
{{/each}}

## Utterance
"{{utterance}}"

## Classified Intent
{{intent_id}}

## Task
Extract all entities. Return JSON:
```json
{
  "entities": [
    {
      "type": "entity_type",
      "value": "normalized_value",
      "original_text": "as found in utterance",
      "confidence": 0.0-1.0
    }
  ]
}
```
```

### Task 3.3.4: Hybrid Classifier
**File:** `rust/src/agentic/intent_classifier.rs`

- [ ] Add `HybridIntentClassifier` that tries patterns first, then LLM:
```rust
pub struct HybridIntentClassifier {
    pattern_classifier: IntentClassifier,
    llm_client: Arc<dyn LlmClient>,
    fallback_threshold: f32, // Use LLM if pattern confidence below this
}

impl HybridIntentClassifier {
    pub async fn classify(&self, utterance: &str, context: &ConversationContext) -> ClassificationResult {
        // Try pattern matching first
        let pattern_result = self.pattern_classifier.classify(utterance, context);
        
        // If high confidence, return pattern result
        if !pattern_result.intents.is_empty() && 
           pattern_result.intents[0].confidence >= self.fallback_threshold {
            return pattern_result;
        }
        
        // Fall back to LLM
        match self.classify_with_llm(utterance, context).await {
            Ok(llm_result) => {
                // Merge results, preferring higher confidence
                self.merge_results(pattern_result, llm_result)
            }
            Err(e) => {
                log::warn!("LLM classification failed: {}, using pattern result", e);
                pattern_result
            }
        }
    }
    
    async fn classify_with_llm(&self, utterance: &str, context: &ConversationContext) -> Result<ClassificationResult, LlmError> {
        // Build prompt from template
        // Call LLM
        // Parse response
        todo!()
    }
}
```

### Task 3.3.5: Hybrid Entity Extractor
**File:** `rust/src/agentic/entity_extractor.rs`

- [ ] Add LLM fallback for entity extraction:
```rust
impl EntityExtractor {
    pub async fn extract_with_llm_fallback(
        &mut self,
        utterance: &str,
        context: &ConversationContext,
        intent: &ClassifiedIntent,
        llm_client: &dyn LlmClient,
    ) -> ExtractedEntities {
        // First try pattern extraction
        let mut entities = self.extract(utterance, context);
        
        // Check if we have required entities for the intent
        let missing = self.find_missing_required(intent, &entities);
        
        if !missing.is_empty() {
            // Try LLM extraction for missing entities
            if let Ok(llm_entities) = self.extract_with_llm(utterance, &missing, llm_client).await {
                entities.merge(llm_entities);
            }
        }
        
        entities
    }
}
```

### Task 3.3.6: Pipeline Integration
**File:** `rust/src/agentic/pipeline.rs`

- [ ] Update `AgentPipeline` to use hybrid classifiers
- [ ] Add configuration for LLM fallback enablement
- [ ] Add metrics for pattern vs LLM classification rates

### Task 3.3.7: LLM Fallback Tests
**File:** `rust/src/agentic/pipeline_tests.rs`

- [ ] Test pattern classification (no LLM needed)
- [ ] Test LLM fallback triggers on low confidence
- [ ] Test LLM extraction for missing entities
- [ ] Mock LLM client for deterministic testing

---

## Phase 3.4: Evaluation Harness (Priority 4)

**Goal:** Build test runner for `evaluation_dataset.yaml` to measure system accuracy.

### Task 3.4.1: Evaluation Runner
**File:** `rust/src/agentic/evaluation.rs` (new file)

- [ ] Create evaluation runner:
```rust
pub struct EvaluationRunner {
    pipeline: AgentPipeline,
    dataset: EvaluationDataset,
}

#[derive(Debug, Deserialize)]
pub struct EvaluationDataset {
    pub version: String,
    pub evaluation_cases: Vec<EvaluationCase>,
    pub metrics: MetricsConfig,
}

#[derive(Debug, Deserialize)]
pub struct EvaluationCase {
    pub id: String,
    pub category: String,
    pub difficulty: String,
    pub input: String,
    pub expected_intents: Vec<String>,
    pub expected_entities: HashMap<String, serde_json::Value>,
    pub expected_dsl_contains: Vec<String>,
    pub expected_dsl_not_contains: Vec<String>,
    pub context: Option<TestContext>,
    pub notes: Option<String>,
}

#[derive(Debug)]
pub struct EvaluationResult {
    pub case_id: String,
    pub intent_correct: bool,
    pub entities_correct: bool,
    pub dsl_valid: bool,
    pub dsl_contains_check: bool,
    pub errors: Vec<String>,
    pub latency_ms: u64,
}

impl EvaluationRunner {
    pub async fn run_all(&mut self) -> EvaluationReport {
        let mut results = Vec::new();
        for case in &self.dataset.evaluation_cases {
            results.push(self.run_case(case).await);
        }
        EvaluationReport::from_results(results, &self.dataset.metrics)
    }
    
    pub async fn run_category(&mut self, category: &str) -> EvaluationReport {
        let cases: Vec<_> = self.dataset.evaluation_cases
            .iter()
            .filter(|c| c.category == category)
            .collect();
        // ...
    }
    
    async fn run_case(&mut self, case: &EvaluationCase) -> EvaluationResult {
        let start = std::time::Instant::now();
        
        // Set up context if provided
        let session_id = Uuid::new_v4();
        if let Some(ctx) = &case.context {
            self.apply_test_context(session_id, ctx);
        }
        
        // Run pipeline
        let response = self.pipeline.process(&case.input, session_id).await;
        
        // Evaluate results
        let intent_correct = self.check_intents(&response, &case.expected_intents);
        let entities_correct = self.check_entities(&response, &case.expected_entities);
        let dsl_valid = self.check_dsl_valid(&response);
        let dsl_contains = self.check_dsl_contains(&response, &case.expected_dsl_contains, &case.expected_dsl_not_contains);
        
        EvaluationResult {
            case_id: case.id.clone(),
            intent_correct,
            entities_correct,
            dsl_valid,
            dsl_contains_check: dsl_contains,
            errors: vec![], // Collect specific errors
            latency_ms: start.elapsed().as_millis() as u64,
        }
    }
}
```

### Task 3.4.2: Evaluation Report
**File:** `rust/src/agentic/evaluation.rs`

- [ ] Create report generation:
```rust
#[derive(Debug)]
pub struct EvaluationReport {
    pub total_cases: usize,
    pub passed: usize,
    pub failed: usize,
    pub intent_accuracy: f64,
    pub entity_accuracy: f64,
    pub dsl_validity_rate: f64,
    pub avg_latency_ms: f64,
    pub by_category: HashMap<String, CategoryMetrics>,
    pub by_difficulty: HashMap<String, CategoryMetrics>,
    pub failures: Vec<FailureDetail>,
}

impl EvaluationReport {
    pub fn print_summary(&self) {
        println!("=== Evaluation Report ===");
        println!("Total: {} | Passed: {} | Failed: {}", self.total_cases, self.passed, self.failed);
        println!("Intent Accuracy: {:.1}%", self.intent_accuracy * 100.0);
        println!("Entity Accuracy: {:.1}%", self.entity_accuracy * 100.0);
        println!("DSL Validity: {:.1}%", self.dsl_validity_rate * 100.0);
        println!("Avg Latency: {:.0}ms", self.avg_latency_ms);
        
        println!("\nBy Category:");
        for (cat, metrics) in &self.by_category {
            println!("  {}: {}/{} ({:.1}%)", cat, metrics.passed, metrics.total, metrics.pass_rate * 100.0);
        }
        
        if !self.failures.is_empty() {
            println!("\nFailures:");
            for f in &self.failures {
                println!("  {} - {}: {}", f.case_id, f.category, f.reason);
            }
        }
    }
}
```

### Task 3.4.3: CLI for Evaluation
**File:** `rust/src/bin/evaluate_agent.rs` (new file)

- [ ] Create CLI tool:
```rust
use clap::Parser;

#[derive(Parser)]
struct Args {
    /// Path to evaluation dataset
    #[arg(short, long, default_value = "config/agent/evaluation_dataset.yaml")]
    dataset: PathBuf,
    
    /// Run only specific category
    #[arg(short, long)]
    category: Option<String>,
    
    /// Run only specific case ID
    #[arg(short = 'i', long)]
    case_id: Option<String>,
    
    /// Output format (text, json, csv)
    #[arg(short, long, default_value = "text")]
    format: String,
    
    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // Load configs and create pipeline
    let pipeline = AgentPipeline::from_config_dir(Path::new("config/agent"))?;
    let dataset = EvaluationDataset::load(&args.dataset)?;
    
    let mut runner = EvaluationRunner::new(pipeline, dataset);
    
    let report = if let Some(cat) = &args.category {
        runner.run_category(cat).await
    } else if let Some(id) = &args.case_id {
        runner.run_single(id).await
    } else {
        runner.run_all().await
    };
    
    match args.format.as_str() {
        "json" => println!("{}", serde_json::to_string_pretty(&report)?),
        "csv" => report.print_csv(),
        _ => report.print_summary(),
    }
    
    // Exit with error code if below threshold
    if report.intent_accuracy < 0.85 {
        std::process::exit(1);
    }
    
    Ok(())
}
```

### Task 3.4.4: Add to Cargo.toml
**File:** `rust/Cargo.toml`

- [ ] Add binary target:
```toml
[[bin]]
name = "evaluate_agent"
path = "src/bin/evaluate_agent.rs"
```

### Task 3.4.5: Expand Evaluation Dataset
**File:** `rust/config/agent/evaluation_dataset.yaml`

- [ ] Add more test cases for edge cases
- [ ] Add regression test cases as bugs are found
- [ ] Add performance baseline cases

---

## Phase 3.5: Query Intent Implementation (Priority 5)

**Goal:** Make all query intents return useful information (not just DSL).

### Task 3.5.1: Query Response Type
**File:** `rust/src/agentic/pipeline.rs`

- [ ] Add query result handling distinct from mutation:
```rust
pub enum QueryResult {
    InvestmentManagers(Vec<ImAssignmentSummary>),
    PricingConfig(Vec<PricingConfigSummary>),
    CashSweeps(Vec<CashSweepSummary>),
    SlaCommitments(Vec<SlaCommitmentSummary>),
    TradingMatrix(TradingMatrixView),
    ValidationGaps(Vec<ConfigurationGap>),
}

impl AgentPipeline {
    async fn execute_query(&self, dsl: &str, session_id: Uuid) -> Result<QueryResult, PipelineError> {
        // Parse and identify query type
        // Execute against database
        // Format results
    }
}
```

### Task 3.5.2: IM Query Handler
**File:** `rust/src/agentic/query_handlers.rs` (new file)

- [ ] Implement IM list query:
```rust
pub async fn handle_im_list(
    pool: &PgPool,
    cbu_id: &str,
    filter: Option<&ImFilter>,
) -> Result<Vec<ImAssignmentSummary>, QueryError> {
    let assignments = sqlx::query_as!(
        ImAssignmentRow,
        r#"
        SELECT ia.*, im.name as manager_name
        FROM cbu_im_assignment ia
        JOIN investment_manager im ON ia.manager_id = im.id
        WHERE ia.cbu_id = $1
        ORDER BY ia.priority
        "#,
        cbu_id
    )
    .fetch_all(pool)
    .await?;
    
    // Format into summaries
    Ok(assignments.into_iter().map(|a| ImAssignmentSummary {
        manager_name: a.manager_name,
        scope_markets: a.scope_markets,
        scope_instruments: a.scope_instrument_classes,
        instruction_method: a.instruction_method,
        priority: a.priority,
    }).collect())
}
```

### Task 3.5.3: Pricing Query Handler
- [ ] Implement pricing config list query

### Task 3.5.4: Trading Matrix Visualization Query
- [ ] Implement matrix visualization query (returns structured data for UI)

### Task 3.5.5: Gap Analysis Query
- [ ] Implement validation/gap query that returns missing configurations

---

## Phase 3.6: Coreference Improvements (Priority 6)

**Goal:** Better pronoun and anaphora resolution.

### Task 3.6.1: Entity Salience Tracking
**File:** `rust/src/agentic/entity_extractor.rs`

- [ ] Track entity salience (recency + frequency) in session:
```rust
#[derive(Debug, Clone)]
pub struct SalientEntity {
    pub entity_type: String,
    pub value: String,
    pub symbol: Option<String>,
    pub last_mentioned: usize, // Turn number
    pub mention_count: usize,
    pub salience_score: f32,
}

impl SessionContext {
    pub fn update_salience(&mut self, entities: &ExtractedEntities, turn_number: usize) {
        for entity in entities.iter() {
            let entry = self.salient_entities
                .entry((entity.entity_type.clone(), entity.value.clone()))
                .or_insert(SalientEntity {
                    entity_type: entity.entity_type.clone(),
                    value: entity.value.clone(),
                    symbol: None,
                    last_mentioned: turn_number,
                    mention_count: 0,
                    salience_score: 0.0,
                });
            entry.mention_count += 1;
            entry.last_mentioned = turn_number;
            entry.salience_score = Self::calculate_salience(entry, turn_number);
        }
    }
    
    pub fn resolve_pronoun(&self, pronoun: &str, entity_type: &str) -> Option<&SalientEntity> {
        // Find most salient entity of the expected type
        self.salient_entities
            .values()
            .filter(|e| e.entity_type == entity_type)
            .max_by(|a, b| a.salience_score.partial_cmp(&b.salience_score).unwrap())
    }
}
```

### Task 3.6.2: Possessive Resolution
**File:** `rust/src/agentic/entity_extractor.rs`

- [ ] Handle possessives ("BlackRock's scope", "their markets"):
```rust
fn resolve_possessive(&self, text: &str, context: &ConversationContext) -> Option<ExtractedEntity> {
    // Match patterns like "BlackRock's", "their", "its"
    let possessive_re = Regex::new(r"(\w+)'s|their|its").unwrap();
    
    if let Some(cap) = possessive_re.captures(text) {
        let owner = cap.get(1).map(|m| m.as_str());
        // Resolve owner to entity
        // Return entity representing the possessed thing
    }
    None
}
```

### Task 3.6.3: Definite Reference Resolution
- [ ] Handle "the IM", "the first manager", "the European assignment"

---

## Phase 3.7: Error Handling & Recovery (Priority 7)

### Task 3.7.1: Validation Error Recovery
**File:** `rust/src/agentic/pipeline.rs`

- [ ] When DSL validation fails, attempt to fix common issues:
```rust
async fn handle_validation_errors(
    &self,
    dsl: &str,
    errors: &[ValidationError],
    entities: &ExtractedEntities,
) -> Result<String, PipelineError> {
    let mut fixed_dsl = dsl.to_string();
    
    for error in errors {
        match &error.error_type {
            ValidationErrorType::MissingRequiredParam(param) => {
                // Try to infer from context or ask for clarification
                if let Some(value) = self.infer_missing_param(param, entities) {
                    fixed_dsl = self.inject_param(&fixed_dsl, param, &value);
                }
            }
            ValidationErrorType::InvalidValue(param, value) => {
                // Try to normalize or suggest alternatives
            }
            _ => {}
        }
    }
    
    Ok(fixed_dsl)
}
```

### Task 3.7.2: Execution Error Recovery
- [ ] On execution failure, provide actionable error messages
- [ ] Suggest corrections based on error type

### Task 3.7.3: Partial Success Handling
- [ ] When multi-statement execution partially fails, report what succeeded
- [ ] Allow user to retry failed statements

---

## Phase 3.8: Documentation & Polish

### Task 3.8.1: Architecture Documentation
**File:** `rust/docs/AGENT_ARCHITECTURE.md` (new file)

- [ ] Document pipeline stages
- [ ] Document configuration files
- [ ] Document extension points

### Task 3.8.2: Intent Catalog
**File:** `rust/docs/INTENT_CATALOG.md` (new file)

- [ ] Document all intents with examples
- [ ] Document trigger phrases
- [ ] Document required vs optional entities

### Task 3.8.3: Troubleshooting Guide
**File:** `rust/docs/AGENT_TROUBLESHOOTING.md` (new file)

- [ ] Common issues and solutions
- [ ] How to add new intents
- [ ] How to tune confidence thresholds

### Task 3.8.4: Code Cleanup
- [ ] Remove TODO comments that are now done
- [ ] Add missing doc comments
- [ ] Run clippy and fix warnings
- [ ] Format with rustfmt

---

## Verification Checklist

Before marking Phase 3 Baseline Complete:

### Functional Requirements
- [ ] All 25+ intents in taxonomy have parameter mappings
- [ ] Pattern classification works for all trigger phrases
- [ ] LLM fallback activates on low confidence
- [ ] Entity extraction handles all defined entity types
- [ ] Market region expansion works (European → 5 MICs)
- [ ] Instrument hierarchy expansion works (fixed income → 2 classes)
- [ ] Coreference resolves "them", "that manager", etc.
- [ ] Generated DSL parses without errors
- [ ] Generated DSL executes against database
- [ ] Query intents return formatted results
- [ ] Multi-statement execution is transactional
- [ ] Symbols propagate across conversation turns

### Quality Requirements
- [ ] Evaluation dataset passes at 85%+ accuracy
- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] No clippy warnings
- [ ] Documentation complete

### Demo Requirements
- [ ] Can onboard an IM through conversation
- [ ] Can configure pricing through conversation
- [ ] Can set up cash sweep through conversation
- [ ] Can query "who handles European equities"
- [ ] Can handle multi-intent utterance
- [ ] Can handle follow-up with coreference

---

## Execution Order

1. **Week 1 (Days 1-5)**
   - Phase 3.1: Wire Execution (Tasks 3.1.1-3.1.5)
   - Phase 3.2: Parameter Mappings (Tasks 3.2.1-3.2.4)

2. **Week 2 (Days 6-10)**
   - Phase 3.2: Parameter Mappings (Tasks 3.2.5-3.2.8)
   - Phase 3.4: Evaluation Harness (Tasks 3.4.1-3.4.5)
   - Run evaluation, identify gaps

3. **Week 3 (Days 11-15)** (if needed)
   - Phase 3.3: LLM Fallback (Tasks 3.3.1-3.3.7)
   - Phase 3.5: Query Implementation (Tasks 3.5.1-3.5.5)

4. **Week 4 (Days 16-20)** (polish)
   - Phase 3.6: Coreference Improvements
   - Phase 3.7: Error Handling
   - Phase 3.8: Documentation

---

## Notes for Claude Code

1. **Test as you go** - Run `cargo test` after each task
2. **Use real configs** - Integration tests should load actual YAML files
3. **Preserve existing code** - Don't break what works, extend it
4. **Log decisions** - Add comments explaining non-obvious choices
5. **Ask for clarification** - If a task is ambiguous, check before implementing

## Files to Create
- `rust/src/agentic/evaluation.rs`
- `rust/src/agentic/query_handlers.rs`
- `rust/src/agentic/prompts/intent_classification.md`
- `rust/src/agentic/prompts/entity_extraction.md`
- `rust/src/bin/evaluate_agent.rs`
- `rust/docs/AGENT_ARCHITECTURE.md`
- `rust/docs/INTENT_CATALOG.md`
- `rust/docs/AGENT_TROUBLESHOOTING.md`

## Files to Modify
- `rust/src/agentic/pipeline.rs` (execution wiring)
- `rust/src/agentic/intent_classifier.rs` (hybrid classifier)
- `rust/src/agentic/entity_extractor.rs` (LLM fallback, coreference)
- `rust/src/agentic/mod.rs` (exports)
- `rust/config/agent/parameter_mappings.yaml` (add all mappings)
- `rust/config/agent/entity_types.yaml` (add missing types)
- `rust/Cargo.toml` (add binary)
