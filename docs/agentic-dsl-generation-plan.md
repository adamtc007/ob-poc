# Agentic DSL Generation Implementation Plan

**Document**: `agentic-dsl-generation-plan.md`  
**Created**: 2025-12-01  
**Status**: Ready for Implementation  
**Priority**: HIGH - Flagship Feature

## Executive Summary

This plan implements **agentic DSL generation** for the custody/settlement domain. Unlike typical AI integrations that extract data non-deterministically, this system produces **executable, validated DSL code** from natural language requests.

**The differentiator**: User describes an onboarding scenario in plain English → Agent generates 50+ lines of syntactically valid, semantically correct DSL → DSL executes against database → Deterministic, auditable result.

This is enterprise-grade workflow automation, not chatbot theatre.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         USER REQUEST                                         │
│  "Onboard BlackRock for global equities with MS as OTC counterparty"        │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PHASE 1: INTENT EXTRACTION                                │
│  Structured extraction of entities, instruments, markets, relationships     │
│  Output: OnboardingIntent struct                                            │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PHASE 2: RAG ENRICHMENT                                   │
│  Query Qdrant for:                                                          │
│  - Verb schemas (args, valid_values, lookups)                               │
│  - Domain knowledge (market conventions, BICs, settlement cycles)           │
│  - Example workflows (similar past onboardings)                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PHASE 3: REQUIREMENT DERIVATION                           │
│  Expand intent into complete requirements:                                  │
│  - Universe entries = markets × currencies × settlement types               │
│  - SSIs = unique settlement routes                                          │
│  - Booking rules = specific + fallbacks                                     │
│  - ISDA/CSA = per OTC counterparty                                          │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PHASE 4: DSL CODE GENERATION                              │
│  Emit syntactically valid S-expressions                                     │
│  - Respect verb schemas from RAG                                            │
│  - Capture intermediate results (:as @variable)                             │
│  - Order for dependency resolution                                          │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PHASE 5: VALIDATION                                       │
│  Parse → CSG Lint → Compile                                                 │
│  If errors: feed back to agent for correction (max 3 retries)               │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PHASE 6: EXECUTION (optional)                             │
│  Execute against database                                                    │
│  Return results with created entity IDs                                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Vector Database Setup

### 1.1 Create Qdrant Collections

**Task**: Create dedicated collections for agentic RAG

**File**: `sql/seeds/qdrant_collections.sql` (documentation only - actual setup via Qdrant API)

**Collections**:

| Collection | Purpose | Vector Dimension |
|------------|---------|------------------|
| `dsl_verb_schemas` | Verb definitions from verbs.yaml | 1536 (OpenAI) or 1024 (Claude) |
| `domain_knowledge` | Market conventions, BICs, settlement rules | 1536 |
| `onboarding_examples` | Example DSL scripts with descriptions | 1536 |
| `instrument_taxonomy` | CFI/SMPG/ISDA mappings | 1536 |

**Implementation**:
```rust
// rust/src/agentic/vector_store.rs

pub struct VectorStoreConfig {
    pub qdrant_url: String,
    pub embedding_model: EmbeddingModel,
}

pub enum EmbeddingModel {
    OpenAI { model: String },      // text-embedding-3-small
    Claude { model: String },       // If Anthropic releases embeddings
    Local { model_path: PathBuf }, // sentence-transformers fallback
}

pub struct VectorStore {
    client: QdrantClient,
    embedder: Box<dyn Embedder>,
}

impl VectorStore {
    pub async fn ensure_collections(&self) -> Result<()>;
    pub async fn index_verb_schemas(&self, verbs: &VerbConfig) -> Result<usize>;
    pub async fn index_domain_knowledge(&self, docs: Vec<DomainDoc>) -> Result<usize>;
    pub async fn search(&self, collection: &str, query: &str, limit: usize) -> Result<Vec<SearchResult>>;
}
```

**Effort**: Medium (1-2 days)

---

### 1.2 Index Verb Schemas

**Task**: Parse verbs.yaml and index each verb as a searchable document

**Chunking Strategy**: One document per verb (not per domain)

**Document Structure**:
```json
{
  "id": "cbu-custody.add-booking-rule",
  "domain": "cbu-custody",
  "verb": "add-booking-rule",
  "description": "Add ALERT-style booking rule for SSI routing",
  "behavior": "crud",
  "args": [
    {
      "name": "cbu-id",
      "type": "uuid",
      "required": true,
      "description": "CBU identifier"
    },
    {
      "name": "instrument-class",
      "type": "lookup",
      "required": false,
      "lookup_table": "instrument_classes",
      "lookup_column": "code"
    }
  ],
  "valid_values": {
    "settlement-type": ["DVP", "FOP", "RVP", "DFP"]
  },
  "returns": {
    "type": "uuid",
    "name": "rule_id",
    "capture": true
  },
  "example": "(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi :name \"US Equity\" :priority 10 :instrument-class \"EQUITY\" :market \"XNYS\")",
  "text_for_embedding": "Add ALERT-style booking rule for SSI routing. Creates routing rule that matches trade characteristics to standing settlement instructions. Arguments: cbu-id (required), ssi-id (required), name, priority, instrument-class, security-type, market, currency, settlement-type, counterparty. Used for settlement instruction lookup."
}
```

**Implementation**:
```rust
// rust/src/agentic/indexer.rs

pub struct VerbDocument {
    pub id: String,
    pub domain: String,
    pub verb: String,
    pub description: String,
    pub args: Vec<ArgDoc>,
    pub valid_values: HashMap<String, Vec<String>>,
    pub example: Option<String>,
    pub text_for_embedding: String,
}

pub fn parse_verbs_yaml_to_documents(config: &VerbConfig) -> Vec<VerbDocument> {
    // Iterate domains, verbs, generate embedding text
}

pub async fn index_all_verbs(store: &VectorStore, config_path: &Path) -> Result<IndexReport> {
    let config = load_verb_config(config_path)?;
    let docs = parse_verbs_yaml_to_documents(&config);
    
    for doc in docs {
        let embedding = store.embed(&doc.text_for_embedding).await?;
        store.upsert("dsl_verb_schemas", &doc.id, embedding, doc.to_payload()).await?;
    }
    
    Ok(IndexReport { indexed: docs.len() })
}
```

**CLI Command**:
```bash
dsl_cli index verbs --config rust/config/verbs.yaml
```

**Effort**: Medium (1 day)

---

### 1.3 Index Domain Knowledge

**Task**: Create and index domain knowledge documents

**Source Documents** (create in `docs/domain_knowledge/`):

| Document | Content |
|----------|---------|
| `markets.md` | MIC codes, CSDs, timezones, settlement cycles |
| `settlement_conventions.md` | DVP vs FOP, T+1/T+2, cross-border |
| `subcustodian_bics.md` | Standard BICs for major CSDs (DTCC, Euroclear, Clearstream, CREST) |
| `isda_conventions.md` | NY vs English law, CSA types, thresholds |
| `booking_rule_patterns.md` | Priority ordering, fallback strategies, counterparty overrides |
| `instrument_classification.md` | CFI → SMPG → ISDA mapping logic |

**Document Structure**:
```json
{
  "id": "domain-markets-us",
  "category": "markets",
  "title": "US Markets Settlement",
  "content": "NYSE (XNYS) and NASDAQ (XNAS) settle via DTCC (BIC: DTCYUS33). Standard settlement is T+1 for equities. Primary currency USD. CSD participant accounts required for direct settlement...",
  "tags": ["US", "XNYS", "XNAS", "DTCC", "T+1", "USD"],
  "text_for_embedding": "US equity markets NYSE NASDAQ settlement DTCC T+1 USD..."
}
```

**Effort**: Medium (1-2 days for document creation + indexing code)

---

### 1.4 Index Example Workflows

**Task**: Create example onboarding scripts with descriptions for few-shot learning

**Source**: `docs/examples/custody_onboarding/`

**Examples to Create**:

| Example | Description |
|---------|-------------|
| `us_equity_simple.dsl` | Single market, single currency, basic SSI |
| `multi_market_equity.dsl` | US + UK + Germany, cross-currency |
| `otc_irs_with_isda.dsl` | OTC IRS setup with ISDA and CSA |
| `full_institutional.dsl` | Complete institutional setup (50+ statements) |
| `counterparty_override.dsl` | Specific SSI for one counterparty |

**Each Example Has**:
```yaml
# us_equity_simple.yaml (metadata)
id: example-us-equity-simple
title: "US Equity Simple Setup"
description: "Basic custody setup for a fund trading US equities only"
complexity: simple
domains: [cbu, cbu-custody]
markets: [XNYS]
currencies: [USD]
instruments: [EQUITY]
has_otc: false
has_isda: false
tags: [beginner, single-market, equity]
```

**Effort**: Medium (1-2 days)

---

## Phase 2: Intent Extraction

### 2.1 Define OnboardingIntent Structure

**Task**: Create structured representation of user intent

**File**: `rust/src/agentic/intent.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingIntent {
    /// The client being onboarded
    pub client: ClientIntent,
    
    /// Instruments they will trade
    pub instruments: Vec<InstrumentIntent>,
    
    /// Markets they will access
    pub markets: Vec<MarketIntent>,
    
    /// OTC counterparty relationships
    pub otc_counterparties: Vec<CounterpartyIntent>,
    
    /// Explicit requirements mentioned
    pub explicit_requirements: Vec<String>,
    
    /// Inferred requirements
    pub inferred_requirements: Vec<String>,
    
    /// Timeline/urgency
    pub timeline: Option<String>,
    
    /// Original natural language
    pub original_request: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientIntent {
    pub name: String,
    pub entity_type: Option<String>,  // fund, corporate, individual
    pub jurisdiction: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketIntent {
    pub market_code: String,          // XNYS, XLON, XFRA
    pub currencies: Vec<String>,      // USD, GBP, EUR
    pub settlement_types: Vec<String>, // DVP, FOP
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentIntent {
    pub class: String,                // EQUITY, GOVT_BOND, OTC_IRS
    pub specific_types: Vec<String>,  // ADR, ETF, etc.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterpartyIntent {
    pub name: String,
    pub instruments: Vec<String>,     // What they trade with this CP
    pub isda_required: bool,
    pub governing_law: Option<String>,
    pub csa_required: bool,
}
```

**Effort**: Small (0.5 day)

---

### 2.2 Implement Intent Extractor

**Task**: Use Claude to extract structured intent from natural language

**File**: `rust/src/agentic/intent_extractor.rs`

```rust
pub struct IntentExtractor {
    client: AnthropicClient,
    system_prompt: String,
}

impl IntentExtractor {
    pub fn new(client: AnthropicClient) -> Self {
        Self {
            client,
            system_prompt: include_str!("prompts/intent_extraction.txt").to_string(),
        }
    }
    
    pub async fn extract(&self, user_request: &str) -> Result<OnboardingIntent> {
        let response = self.client.complete(
            &self.system_prompt,
            user_request,
            ExtractedIntent::schema(), // JSON schema for structured output
        ).await?;
        
        serde_json::from_str(&response)
    }
}
```

**Prompt Template** (`rust/src/agentic/prompts/intent_extraction.txt`):
```
You are an expert custody onboarding analyst. Extract structured information from the user's onboarding request.

CONTEXT:
- This is for a custody bank onboarding a new client
- Clients trade securities (equities, bonds) and/or OTC derivatives
- Each market/currency combination needs settlement instructions (SSIs)
- OTC derivatives require ISDA agreements with counterparties

EXTRACTION RULES:
1. Identify the client entity (name, type if mentioned, jurisdiction if mentioned)
2. Identify all markets mentioned or implied (XNYS=US, XLON=UK, XFRA=Germany)
3. For each market, identify currencies (local + cross-currency if mentioned)
4. Identify instrument classes (EQUITY, GOVT_BOND, CORP_BOND, OTC_IRS, etc.)
5. Identify OTC counterparties and what instruments they trade
6. Infer ISDA requirement if OTC instruments mentioned
7. Note any explicit timeline requirements

OUTPUT: JSON matching the OnboardingIntent schema.

Example input: "Onboard BlackRock for global equities in US and UK, settling in local currencies plus USD cross-currency"

Example output:
{
  "client": {"name": "BlackRock", "entity_type": "fund"},
  "markets": [
    {"market_code": "XNYS", "currencies": ["USD"], "settlement_types": ["DVP"]},
    {"market_code": "XLON", "currencies": ["GBP", "USD"], "settlement_types": ["DVP"]}
  ],
  "instruments": [{"class": "EQUITY", "specific_types": []}],
  "otc_counterparties": [],
  "inferred_requirements": ["SSIs for 3 currency routes", "Booking rules with fallbacks"],
  "original_request": "..."
}
```

**Effort**: Medium (1 day)

---

## Phase 3: Requirement Derivation

### 3.1 Implement Requirement Planner

**Task**: Expand intent into complete set of DSL operations needed

**File**: `rust/src/agentic/planner.rs`

```rust
pub struct RequirementPlanner {
    vector_store: VectorStore,
}

#[derive(Debug, Clone)]
pub struct OnboardingPlan {
    /// Entities to create or lookup
    pub entities: Vec<EntityPlan>,
    
    /// CBU to create
    pub cbu: CbuPlan,
    
    /// Universe entries (Layer 1)
    pub universe: Vec<UniversePlan>,
    
    /// SSIs to create (Layer 2)
    pub ssis: Vec<SsiPlan>,
    
    /// Booking rules (Layer 3)
    pub booking_rules: Vec<BookingRulePlan>,
    
    /// ISDA agreements
    pub isdas: Vec<IsdaPlan>,
    
    /// Validation steps
    pub validations: Vec<String>,
}

impl RequirementPlanner {
    pub async fn plan(&self, intent: &OnboardingIntent) -> Result<OnboardingPlan> {
        let mut plan = OnboardingPlan::default();
        
        // 1. Plan entities
        plan.entities = self.plan_entities(intent).await?;
        
        // 2. Plan CBU
        plan.cbu = self.plan_cbu(intent)?;
        
        // 3. Plan universe (markets × currencies × settlement types)
        plan.universe = self.derive_universe(intent)?;
        
        // 4. Plan SSIs (one per unique settlement route)
        plan.ssis = self.derive_ssis(&plan.universe)?;
        
        // 5. Plan booking rules (specific + fallbacks)
        plan.booking_rules = self.derive_booking_rules(&plan.universe, &plan.ssis)?;
        
        // 6. Plan ISDA/CSA
        plan.isdas = self.plan_isdas(intent)?;
        
        // 7. Add validation steps
        plan.validations = vec![
            "cbu-custody.validate-booking-coverage".to_string(),
        ];
        
        Ok(plan)
    }
    
    fn derive_universe(&self, intent: &OnboardingIntent) -> Result<Vec<UniversePlan>> {
        let mut universe = Vec::new();
        
        for market in &intent.markets {
            for instrument in &intent.instruments {
                universe.push(UniversePlan {
                    instrument_class: instrument.class.clone(),
                    market: Some(market.market_code.clone()),
                    currencies: market.currencies.clone(),
                    settlement_types: market.settlement_types.clone(),
                });
            }
        }
        
        // Add OTC universe entries (no market, counterparty-specific)
        for cp in &intent.otc_counterparties {
            for instr in &cp.instruments {
                universe.push(UniversePlan {
                    instrument_class: instr.clone(),
                    market: None,
                    currencies: vec!["USD".to_string()], // Default, could be smarter
                    settlement_types: vec!["DVP".to_string()],
                    counterparty: Some(cp.name.clone()),
                });
            }
        }
        
        Ok(universe)
    }
    
    fn derive_booking_rules(&self, universe: &[UniversePlan], ssis: &[SsiPlan]) -> Result<Vec<BookingRulePlan>> {
        let mut rules = Vec::new();
        let mut priority = 10;
        
        // Specific rules for each universe entry
        for entry in universe {
            rules.push(BookingRulePlan {
                name: format!("{} {} {}", 
                    entry.instrument_class,
                    entry.market.as_deref().unwrap_or("OTC"),
                    entry.currencies.first().unwrap_or(&"USD".to_string())
                ),
                priority,
                instrument_class: Some(entry.instrument_class.clone()),
                market: entry.market.clone(),
                currency: entry.currencies.first().cloned(),
                settlement_type: entry.settlement_types.first().cloned(),
                ssi_name: self.match_ssi(entry, ssis),
            });
            priority += 5;
        }
        
        // Currency fallback rules
        let currencies: HashSet<_> = universe.iter()
            .flat_map(|u| u.currencies.iter())
            .collect();
        
        for currency in currencies {
            rules.push(BookingRulePlan {
                name: format!("{} Fallback", currency),
                priority: 50,
                instrument_class: None,
                market: None,
                currency: Some(currency.clone()),
                settlement_type: None,
                ssi_name: format!("{} Primary", currency),
            });
        }
        
        // Ultimate fallback
        rules.push(BookingRulePlan {
            name: "Ultimate Fallback".to_string(),
            priority: 100,
            instrument_class: None,
            market: None,
            currency: None,
            settlement_type: None,
            ssi_name: "Default SSI".to_string(),
        });
        
        Ok(rules)
    }
}
```

**Effort**: Large (2-3 days)

---

## Phase 4: DSL Code Generation

### 4.1 Implement DSL Generator

**Task**: Convert OnboardingPlan to valid DSL source code

**File**: `rust/src/agentic/generator.rs`

```rust
pub struct DslGenerator {
    vector_store: VectorStore,
}

impl DslGenerator {
    pub async fn generate(&self, plan: &OnboardingPlan) -> Result<String> {
        let mut output = String::new();
        
        // Header comment
        output.push_str(&self.generate_header(plan));
        
        // Entity lookups/creation
        output.push_str("\n;; --- Entities ---\n");
        for entity in &plan.entities {
            output.push_str(&self.generate_entity(entity).await?);
        }
        
        // CBU creation
        output.push_str("\n;; --- CBU ---\n");
        output.push_str(&self.generate_cbu(&plan.cbu)?);
        
        // Layer 1: Universe
        output.push_str("\n;; --- Layer 1: Universe ---\n");
        for entry in &plan.universe {
            output.push_str(&self.generate_universe_entry(entry)?);
        }
        
        // Layer 2: SSIs
        output.push_str("\n;; --- Layer 2: SSIs ---\n");
        for ssi in &plan.ssis {
            output.push_str(&self.generate_ssi(ssi)?);
        }
        
        // Activate SSIs
        output.push_str("\n;; Activate SSIs\n");
        for ssi in &plan.ssis {
            output.push_str(&format!(
                "(cbu-custody.activate-ssi :ssi-id @{})\n",
                ssi.variable_name
            ));
        }
        
        // Layer 3: Booking Rules
        output.push_str("\n;; --- Layer 3: Booking Rules ---\n");
        for rule in &plan.booking_rules {
            output.push_str(&self.generate_booking_rule(rule)?);
        }
        
        // ISDA/CSA
        if !plan.isdas.is_empty() {
            output.push_str("\n;; --- ISDA Agreements ---\n");
            for isda in &plan.isdas {
                output.push_str(&self.generate_isda(isda)?);
            }
        }
        
        // Validation
        output.push_str("\n;; --- Validation ---\n");
        output.push_str("(cbu-custody.validate-booking-coverage :cbu-id @cbu)\n");
        
        Ok(output)
    }
    
    fn generate_booking_rule(&self, rule: &BookingRulePlan) -> Result<String> {
        let mut args = vec![
            format!(":cbu-id @cbu"),
            format!(":ssi-id @{}", rule.ssi_variable()),
            format!(":name \"{}\"", rule.name),
            format!(":priority {}", rule.priority),
        ];
        
        // Only add non-None criteria (wildcards)
        if let Some(ref class) = rule.instrument_class {
            args.push(format!(":instrument-class \"{}\"", class));
        }
        if let Some(ref market) = rule.market {
            args.push(format!(":market \"{}\"", market));
        }
        if let Some(ref currency) = rule.currency {
            args.push(format!(":currency \"{}\"", currency));
        }
        if let Some(ref st) = rule.settlement_type {
            args.push(format!(":settlement-type \"{}\"", st));
        }
        
        Ok(format!(
            "(cbu-custody.add-booking-rule {})\n",
            args.join(" ")
        ))
    }
}
```

**Effort**: Large (2-3 days)

---

### 4.2 RAG-Enhanced Generation

**Task**: Use retrieved verb schemas to ensure correct argument names and valid values

**Enhancement to Generator**:

```rust
impl DslGenerator {
    async fn generate_verb_call(&self, domain: &str, verb: &str, args: &HashMap<String, Value>) -> Result<String> {
        // Retrieve verb schema from vector store
        let schema = self.vector_store
            .search("dsl_verb_schemas", &format!("{}.{}", domain, verb), 1)
            .await?
            .first()
            .ok_or_else(|| anyhow!("Unknown verb: {}.{}", domain, verb))?;
        
        let verb_def: VerbDocument = serde_json::from_value(schema.payload.clone())?;
        
        // Validate and format arguments
        let mut formatted_args = Vec::new();
        for arg_def in &verb_def.args {
            if let Some(value) = args.get(&arg_def.name) {
                // Check valid_values if defined
                if let Some(ref valid) = verb_def.valid_values.get(&arg_def.name) {
                    let str_val = value.as_str().unwrap_or_default();
                    if !valid.contains(&str_val.to_string()) {
                        return Err(anyhow!(
                            "Invalid value '{}' for {}. Valid: {:?}",
                            str_val, arg_def.name, valid
                        ));
                    }
                }
                formatted_args.push(self.format_arg(&arg_def.name, value, &arg_def.arg_type)?);
            } else if arg_def.required {
                return Err(anyhow!("Missing required argument: {}", arg_def.name));
            }
        }
        
        Ok(format!("({}.{} {})\n", domain, verb, formatted_args.join(" ")))
    }
}
```

**Effort**: Medium (1 day)

---

## Phase 5: Validation & Feedback Loop

### 5.1 Implement Validator Integration

**Task**: Validate generated DSL before execution, with retry capability

**File**: `rust/src/agentic/validator.rs`

```rust
pub struct AgentValidator {
    parser: DslParser,
    linter: CsgLinter,
    compiler: ExecutionPlanner,
}

#[derive(Debug)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
    pub execution_plan: Option<ExecutionPlan>,
}

#[derive(Debug)]
pub struct ValidationError {
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub suggestion: Option<String>,
}

impl AgentValidator {
    pub fn validate(&self, dsl_source: &str) -> ValidationResult {
        // Parse
        let ast = match self.parser.parse(dsl_source) {
            Ok(ast) => ast,
            Err(e) => return ValidationResult {
                is_valid: false,
                errors: vec![ValidationError {
                    line: e.line,
                    column: e.column,
                    message: e.message,
                    suggestion: self.suggest_parse_fix(&e),
                }],
                warnings: vec![],
                execution_plan: None,
            },
        };
        
        // Lint
        let lint_result = self.linter.lint(&ast);
        if !lint_result.errors.is_empty() {
            return ValidationResult {
                is_valid: false,
                errors: lint_result.errors.iter().map(|e| ValidationError {
                    line: e.line,
                    column: 0,
                    message: e.message.clone(),
                    suggestion: self.suggest_lint_fix(e),
                }).collect(),
                warnings: lint_result.warnings,
                execution_plan: None,
            };
        }
        
        // Compile
        match self.compiler.compile(&ast) {
            Ok(plan) => ValidationResult {
                is_valid: true,
                errors: vec![],
                warnings: lint_result.warnings,
                execution_plan: Some(plan),
            },
            Err(e) => ValidationResult {
                is_valid: false,
                errors: vec![ValidationError {
                    line: 0,
                    column: 0,
                    message: e.to_string(),
                    suggestion: None,
                }],
                warnings: lint_result.warnings,
                execution_plan: None,
            },
        }
    }
}
```

**Effort**: Medium (1 day)

---

### 5.2 Implement Feedback Loop

**Task**: If validation fails, send errors back to agent for correction

**File**: `rust/src/agentic/feedback.rs`

```rust
pub struct FeedbackLoop {
    generator: DslGenerator,
    validator: AgentValidator,
    client: AnthropicClient,
    max_retries: usize,
}

impl FeedbackLoop {
    pub async fn generate_valid_dsl(&self, plan: &OnboardingPlan) -> Result<ValidatedDsl> {
        let mut attempts = 0;
        let mut current_dsl = self.generator.generate(plan).await?;
        
        loop {
            let validation = self.validator.validate(&current_dsl);
            
            if validation.is_valid {
                return Ok(ValidatedDsl {
                    source: current_dsl,
                    execution_plan: validation.execution_plan.unwrap(),
                    attempts,
                });
            }
            
            attempts += 1;
            if attempts >= self.max_retries {
                return Err(anyhow!(
                    "Failed to generate valid DSL after {} attempts. Errors: {:?}",
                    attempts,
                    validation.errors
                ));
            }
            
            // Ask agent to fix
            current_dsl = self.request_fix(&current_dsl, &validation.errors).await?;
        }
    }
    
    async fn request_fix(&self, dsl: &str, errors: &[ValidationError]) -> Result<String> {
        let prompt = format!(
            r#"The following DSL has validation errors. Please fix them.

DSL:
```
{}
```

Errors:
{}

Output ONLY the corrected DSL, nothing else."#,
            dsl,
            errors.iter()
                .map(|e| format!("- Line {}: {} (Suggestion: {})", 
                    e.line, 
                    e.message, 
                    e.suggestion.as_deref().unwrap_or("none")))
                .collect::<Vec<_>>()
                .join("\n")
        );
        
        self.client.complete(&prompt).await
    }
}
```

**Effort**: Medium (1 day)

---

## Phase 6: Integration & API

### 6.1 Create Agent Orchestrator

**Task**: Main entry point that orchestrates the full pipeline

**File**: `rust/src/agentic/orchestrator.rs`

```rust
pub struct AgentOrchestrator {
    intent_extractor: IntentExtractor,
    planner: RequirementPlanner,
    feedback_loop: FeedbackLoop,
    executor: DslExecutor,
}

pub struct GenerationResult {
    pub intent: OnboardingIntent,
    pub plan: OnboardingPlan,
    pub dsl: ValidatedDsl,
    pub execution_result: Option<ExecutionResult>,
}

impl AgentOrchestrator {
    pub async fn generate(&self, request: &str, execute: bool) -> Result<GenerationResult> {
        // Phase 1: Extract intent
        let intent = self.intent_extractor.extract(request).await?;
        
        // Phase 3: Plan requirements
        let plan = self.planner.plan(&intent).await?;
        
        // Phase 4 & 5: Generate and validate DSL
        let dsl = self.feedback_loop.generate_valid_dsl(&plan).await?;
        
        // Phase 6: Execute if requested
        let execution_result = if execute {
            Some(self.executor.execute(&dsl.execution_plan).await?)
        } else {
            None
        };
        
        Ok(GenerationResult {
            intent,
            plan,
            dsl,
            execution_result,
        })
    }
}
```

**Effort**: Medium (1 day)

---

### 6.2 Extend API Routes

**Task**: Add new API endpoints for agentic generation

**File**: `rust/src/api/agent_routes.rs` (extend existing)

```rust
// New routes
pub fn agent_routes() -> Router<AppState> {
    Router::new()
        .route("/api/agent/generate", post(generate_dsl))         // Existing
        .route("/api/agent/generate/custody", post(generate_custody_dsl)) // New: domain-specific
        .route("/api/agent/plan", post(plan_onboarding))          // New: show plan without generating
        .route("/api/agent/execute", post(generate_and_execute))  // New: full pipeline
}

#[derive(Deserialize)]
pub struct CustodyGenerateRequest {
    pub instruction: String,
    pub execute: Option<bool>,
    pub validate_only: Option<bool>,
}

#[derive(Serialize)]
pub struct CustodyGenerateResponse {
    pub intent: OnboardingIntent,
    pub plan: OnboardingPlan,
    pub dsl: String,
    pub validation: ValidationResult,
    pub execution: Option<ExecutionResult>,
}

async fn generate_custody_dsl(
    State(state): State<AppState>,
    Json(request): Json<CustodyGenerateRequest>,
) -> Result<Json<CustodyGenerateResponse>, ApiError> {
    let orchestrator = state.agent_orchestrator();
    let result = orchestrator.generate(&request.instruction, request.execute.unwrap_or(false)).await?;
    
    Ok(Json(CustodyGenerateResponse {
        intent: result.intent,
        plan: result.plan,
        dsl: result.dsl.source,
        validation: ValidationResult { is_valid: true, errors: vec![], warnings: vec![], execution_plan: None },
        execution: result.execution_result,
    }))
}
```

**Effort**: Small (0.5 day)

---

### 6.3 Extend CLI

**Task**: Add CLI commands for agentic generation

**File**: `rust/src/bin/dsl_cli.rs` (extend existing)

```rust
#[derive(Subcommand)]
enum Commands {
    // Existing...
    
    /// Generate custody onboarding DSL from natural language
    Custody {
        /// Natural language instruction
        #[arg(short, long)]
        instruction: String,
        
        /// Execute after generation
        #[arg(long)]
        execute: bool,
        
        /// Show plan without generating DSL
        #[arg(long)]
        plan_only: bool,
        
        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    
    /// Index verb schemas into vector store
    Index {
        #[command(subcommand)]
        target: IndexTarget,
    },
}

#[derive(Subcommand)]
enum IndexTarget {
    /// Index verb schemas from verbs.yaml
    Verbs {
        #[arg(long, default_value = "rust/config/verbs.yaml")]
        config: PathBuf,
    },
    /// Index domain knowledge documents
    Domain {
        #[arg(long, default_value = "docs/domain_knowledge")]
        path: PathBuf,
    },
    /// Index example workflows
    Examples {
        #[arg(long, default_value = "docs/examples/custody_onboarding")]
        path: PathBuf,
    },
}
```

**Usage Examples**:
```bash
# Generate DSL from natural language
dsl_cli custody -i "Onboard BlackRock for US and UK equities with Morgan Stanley as IRS counterparty"

# Generate and execute
dsl_cli custody -i "..." --execute

# Show plan only
dsl_cli custody -i "..." --plan-only

# Index verb schemas
dsl_cli index verbs

# Index domain knowledge
dsl_cli index domain
```

**Effort**: Small (0.5 day)

---

## Phase 7: Testing

### 7.1 Unit Tests

**File**: `rust/src/agentic/tests/`

| Test Module | Coverage |
|-------------|----------|
| `intent_tests.rs` | Intent extraction from various phrasings |
| `planner_tests.rs` | Requirement derivation logic |
| `generator_tests.rs` | DSL output formatting |
| `validator_tests.rs` | Validation and error detection |
| `feedback_tests.rs` | Retry loop behavior |

**Effort**: Medium (1-2 days)

---

### 7.2 Integration Tests

**File**: `rust/tests/agentic_integration.rs`

**Test Scenarios**:

| Scenario | Description |
|----------|-------------|
| `simple_us_equity` | Single market, single currency |
| `multi_market_equity` | US + UK + Germany with cross-currency |
| `with_otc_counterparty` | OTC IRS with ISDA/CSA |
| `counterparty_override` | Specific SSI for one counterparty |
| `full_institutional` | Complete complex setup |
| `validation_failure_recovery` | Intentionally bad input, verify retry works |

**Example Test**:
```rust
#[tokio::test]
async fn test_multi_market_equity() {
    let orchestrator = test_orchestrator().await;
    
    let result = orchestrator.generate(
        "Onboard Pacific Fund for US and UK equities, settling in USD and GBP",
        false
    ).await.unwrap();
    
    // Check intent extraction
    assert_eq!(result.intent.markets.len(), 2);
    assert!(result.intent.markets.iter().any(|m| m.market_code == "XNYS"));
    assert!(result.intent.markets.iter().any(|m| m.market_code == "XLON"));
    
    // Check plan
    assert!(result.plan.universe.len() >= 2);
    assert!(result.plan.ssis.len() >= 2);
    assert!(result.plan.booking_rules.len() >= 4); // Specific + fallbacks
    
    // Check DSL validity
    assert!(result.dsl.source.contains("cbu-custody.add-universe"));
    assert!(result.dsl.source.contains("cbu-custody.create-ssi"));
    assert!(result.dsl.source.contains("cbu-custody.add-booking-rule"));
    assert!(result.dsl.source.contains("cbu-custody.validate-booking-coverage"));
}
```

**Effort**: Medium (1-2 days)

---

### 7.3 Demo Scenarios

**File**: `docs/demos/agentic_custody.md`

**Demo 1: Simple US Equity**
```
User: "Set up a new hedge fund called Apex Capital for US equity trading"

Generated DSL:
;; =============================================================================
;; APEX CAPITAL - US EQUITY ONBOARDING
;; Generated by: Custody Onboarding Agent
;; =============================================================================

;; --- CBU ---
(cbu.ensure :name "Apex Capital" :jurisdiction "US" :client-type "FUND" :as @cbu)

;; --- Layer 1: Universe ---
(cbu-custody.add-universe :cbu-id @cbu :instrument-class "EQUITY" :market "XNYS" :currencies ["USD"] :settlement-types ["DVP"])

;; --- Layer 2: SSIs ---
(cbu-custody.create-ssi :cbu-id @cbu :name "US Primary" :type "SECURITIES" 
  :safekeeping-account "APEX-SAFE-001" :safekeeping-bic "BABOROCP"
  :cash-account "APEX-CASH-001" :cash-bic "BABOROCP" :cash-currency "USD"
  :pset-bic "DTCYUS33" :effective-date "2024-12-01" :as @ssi-us)
(cbu-custody.activate-ssi :ssi-id @ssi-us)

;; --- Layer 3: Booking Rules ---
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "US Equity DVP" :priority 10 :instrument-class "EQUITY" :market "XNYS" :currency "USD" :settlement-type "DVP")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "USD Fallback" :priority 50 :currency "USD")

;; --- Validation ---
(cbu-custody.validate-booking-coverage :cbu-id @cbu)
```

**Demo 2: Full Institutional**
```
User: "Onboard BlackRock Asset Management for global equities trading in US, UK, and Germany. They settle in local currencies plus USD cross-currency. They have OTC IRS exposure with Morgan Stanley and Goldman Sachs under NY law ISDA. Include CSA for variation margin. Full setup for T+1 go-live."

Generated DSL: [50+ lines covering complete setup]
```

**Effort**: Small (0.5 day)

---

## Implementation Summary

### File Structure

```
rust/src/agentic/
├── mod.rs
├── vector_store.rs        # Qdrant integration
├── indexer.rs             # Verb/domain indexing
├── intent.rs              # OnboardingIntent struct
├── intent_extractor.rs    # NL → structured intent
├── planner.rs             # Intent → requirements
├── generator.rs           # Requirements → DSL
├── validator.rs           # DSL validation
├── feedback.rs            # Retry loop
├── orchestrator.rs        # Main pipeline
├── prompts/
│   ├── intent_extraction.txt
│   ├── dsl_generation.txt
│   └── error_correction.txt
└── tests/
    ├── intent_tests.rs
    ├── planner_tests.rs
    ├── generator_tests.rs
    └── integration_tests.rs

docs/
├── domain_knowledge/
│   ├── markets.md
│   ├── settlement_conventions.md
│   ├── subcustodian_bics.md
│   ├── isda_conventions.md
│   └── booking_rule_patterns.md
└── examples/
    └── custody_onboarding/
        ├── us_equity_simple.dsl
        ├── multi_market_equity.dsl
        ├── otc_irs_with_isda.dsl
        └── full_institutional.dsl
```

### Task Summary

| Phase | Tasks | Total Effort |
|-------|-------|--------------|
| 1. Vector DB Setup | 4 tasks | 4-6 days |
| 2. Intent Extraction | 2 tasks | 1.5 days |
| 3. Requirement Derivation | 1 task | 2-3 days |
| 4. DSL Generation | 2 tasks | 3-4 days |
| 5. Validation & Feedback | 2 tasks | 2 days |
| 6. Integration & API | 3 tasks | 2 days |
| 7. Testing | 3 tasks | 2.5-4 days |
| **Total** | **17 tasks** | **17-23 days** |

### Priority Order

1. **Vector store + verb indexing** (foundation for RAG)
2. **Intent extraction** (parse user requests)
3. **Planner** (derive complete requirements)
4. **Generator** (emit valid DSL)
5. **Validation + feedback loop** (ensure correctness)
6. **API/CLI integration** (expose to users)
7. **Testing + demos** (prove it works)

### Dependencies

- Qdrant running (already in place)
- OpenAI API key (for embeddings) OR local sentence-transformers
- Anthropic API key (already in place for Claude)
- Custody schema implemented (done)
- Custody DSL verbs implemented (done)

---

## Success Criteria

1. **Deterministic Output**: Same input produces structurally equivalent DSL
2. **Validation Pass Rate**: >95% of generated DSL passes validation on first attempt
3. **Retry Success**: 100% success within 3 retries for valid requests
4. **Coverage**: Generated DSL includes all required universe/SSI/rule coverage
5. **Execution**: Generated DSL executes successfully against database
6. **Performance**: Full pipeline <10 seconds for typical request

---

## Notes for Claude Code

1. **Start with Phase 1** - Vector store is foundation for everything else
2. **Use existing infrastructure** - Qdrant client, Anthropic client already exist
3. **Leverage verbs.yaml** - It's the source of truth for DSL syntax
4. **Test incrementally** - Each phase should have working tests before moving on
5. **Keep prompts external** - Store in `prompts/` directory for easy iteration
6. **Log everything** - This is complex, visibility is critical for debugging

---

*End of Plan*
