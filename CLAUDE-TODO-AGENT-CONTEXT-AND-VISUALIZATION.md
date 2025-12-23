# CLAUDE TODO: Agent Context & Journey Visualization

## The Vision

User and Agent share the same understanding of "where we are":

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│   USER SEES                          AGENT SEES                 │
│   ─────────                          ──────────                 │
│   Journey map in UI        ═══       Same context in prompt     │
│   Clickable stages                   Stage-relevant verbs       │
│   Entity status                      Gap awareness              │
│   Progress visualization             DSL selection guidance     │
│                                                                 │
│                    SHARED UNDERSTANDING                         │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**User clicks stage → Agent knows focus**
**Agent suggests action → User sees it in context**

---

## What We're Building

```
┌─────────────────────────────────────────────────────────────────┐
│  1. SESSION CONTEXT                                              │
│     CBU selected → discover linked entities → track symbols     │
├─────────────────────────────────────────────────────────────────┤
│  2. SEMANTIC STAGE MAP                                           │
│     Config: Product → Stage → Entity Type                       │
│     Runtime: Derive state from what exists                      │
├─────────────────────────────────────────────────────────────────┤
│  3. AGENT PROMPT INJECTION                                       │
│     Context + Semantic state + Relevant verbs + Examples        │
├─────────────────────────────────────────────────────────────────┤
│  4. UI JOURNEY VISUALIZATION                                     │
│     DAG of stages, progress, clickable expansion                │
└─────────────────────────────────────────────────────────────────┘
```

---

## PART 1: Session Context

### 1.1 Session State Struct

**File:** `rust/crates/entity-gateway/src/session/mod.rs`

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Session state for a user's agent interaction
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentSession {
    pub session_id: String,
    pub user_id: Option<String>,
    
    // Primary context
    pub cbu: Option<CbuContext>,
    
    // Linked contexts (discovered from CBU)
    pub linked: LinkedContexts,
    
    // What user is actively working on
    pub focus: Option<StageFocus>,
    
    // Symbols from DSL execution
    pub symbols: HashMap<String, SymbolValue>,
    
    // Last semantic state (cached for UI)
    pub semantic_state: Option<SemanticState>,
    
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuContext {
    pub id: Uuid,
    pub name: String,
    pub jurisdiction: String,
    pub category: String,
    pub products: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LinkedContexts {
    pub onboarding_request: Option<LinkedEntity>,
    pub kyc_cases: Vec<LinkedEntity>,
    pub trading_profile: Option<LinkedEntity>,
    pub isda_agreements: Vec<LinkedEntity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedEntity {
    pub id: Uuid,
    pub entity_type: String,
    pub display_name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageFocus {
    pub stage_code: String,
    pub stage_name: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolValue {
    pub name: String,
    pub entity_type: String,
    pub id: Uuid,
    pub display: String,
}
```

- [ ] Create session module
- [ ] Add to entity-gateway exports

### 1.2 Context Discovery

```rust
impl AgentSession {
    /// Called when user selects a CBU
    pub async fn load_for_cbu(pool: &PgPool, cbu_id: Uuid) -> Result<Self> {
        // Get CBU
        let cbu = sqlx::query_as!(CbuRow,
            r#"SELECT cbu_id, name, jurisdiction, cbu_category 
               FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            cbu_id
        ).fetch_one(pool).await?;
        
        // Get products
        let products = sqlx::query_scalar!(
            r#"SELECT p.product_code 
               FROM "ob-poc".cbu_product_subscriptions ps
               JOIN "ob-poc".products p ON p.product_id = ps.product_id
               WHERE ps.cbu_id = $1 AND ps.status = 'ACTIVE'"#,
            cbu_id
        ).fetch_all(pool).await?;
        
        // Get linked entities
        let linked = LinkedContexts::discover(pool, cbu_id).await?;
        
        // Derive semantic state
        let semantic_state = SemanticState::derive(pool, cbu_id, &products).await?;
        
        Ok(Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            user_id: None,
            cbu: Some(CbuContext {
                id: cbu.cbu_id,
                name: cbu.name,
                jurisdiction: cbu.jurisdiction,
                category: cbu.cbu_category,
                products,
            }),
            linked,
            focus: None,
            symbols: HashMap::new(),
            semantic_state: Some(semantic_state),
            last_updated: chrono::Utc::now(),
        })
    }
    
    /// Called after DSL execution to capture symbols
    pub fn capture_symbols(&mut self, execution_result: &ExecutionResult) {
        for (name, value) in &execution_result.symbols {
            self.symbols.insert(name.clone(), SymbolValue {
                name: name.clone(),
                entity_type: value.entity_type.clone(),
                id: value.id,
                display: value.display.clone(),
            });
        }
        self.last_updated = chrono::Utc::now();
    }
    
    /// Set focus to a stage (user clicked in UI or agent inferred)
    pub fn set_focus(&mut self, stage_code: &str, stage_name: &str) {
        self.focus = Some(StageFocus {
            stage_code: stage_code.to_string(),
            stage_name: stage_name.to_string(),
            entity_type: None,
            entity_id: None,
        });
        self.last_updated = chrono::Utc::now();
    }
}
```

- [ ] Implement load_for_cbu
- [ ] Implement capture_symbols
- [ ] Implement set_focus

---

## PART 2: Semantic Stage Map

### 2.1 Config Structure

**File:** `config/ontology/semantic_stage_map.yaml`

```yaml
# The onboarding journey structure

stages:
  - code: CLIENT_SETUP
    name: "Client Setup"
    description: "Establish the client entity"
    required_entities: [cbu]
    depends_on: []
    relevant_verbs: [cbu.create, cbu.read, cbu.update]
    
  - code: PRODUCT_SELECTION
    name: "Product Selection"
    description: "Define what services they need"
    required_entities: [cbu_product_subscription]
    depends_on: [CLIENT_SETUP]
    relevant_verbs: [cbu.add-product, cbu.remove-product, product.list]
    
  - code: KYC_REVIEW
    name: "KYC Review"
    description: "Know your customer - regulatory requirement"
    required_entities: [kyc_case, entity_workstream]
    depends_on: [CLIENT_SETUP]
    blocking: true
    relevant_verbs:
      - kyc-case.create
      - kyc-case.read
      - entity-workstream.create
      - entity-workstream.complete
      - doc-request.create
      - screening.run
    examples_path: templates/kyc/
    
  - code: INSTRUMENT_UNIVERSE
    name: "Instrument Universe"
    description: "Define what instruments they trade"
    required_entities: [trading_profile, cbu_instrument_universe]
    depends_on: [PRODUCT_SELECTION]
    relevant_verbs:
      - trading-profile.import
      - trading-profile.activate
      - cbu-custody.add-universe
    examples_path: templates/trading/
    
  - code: LIFECYCLE_RESOURCES
    name: "Lifecycle Resources"
    description: "Operational infrastructure to trade"
    required_entities: [cbu_lifecycle_instance, cbu_ssi]
    depends_on: [INSTRUMENT_UNIVERSE]
    relevant_verbs:
      - lifecycle.provision
      - lifecycle.analyze-gaps
      - cbu-custody.create-ssi
      - cbu-custody.add-booking-rule
    examples_path: templates/custody/
    
  - code: ISDA_SETUP
    name: "ISDA/CSA Setup"
    description: "Legal framework for OTC derivatives"
    required_entities: [isda_agreement, csa_agreement]
    depends_on: [KYC_REVIEW]
    conditional: has_otc_instruments
    relevant_verbs:
      - isda.create
      - isda.add-coverage
      - isda.add-csa
    examples_path: templates/isda/
    
  - code: PRICING_SETUP
    name: "Pricing Configuration"
    description: "Price sources for valuation"
    required_entities: [cbu_pricing_config]
    depends_on: [INSTRUMENT_UNIVERSE]
    relevant_verbs:
      - pricing-config.set
      - pricing-config.list

# Product → Stage requirements
product_stages:
  GLOBAL_CUSTODY:
    mandatory: [CLIENT_SETUP, PRODUCT_SELECTION, KYC_REVIEW, INSTRUMENT_UNIVERSE, LIFECYCLE_RESOURCES]
    conditional:
      - stage: ISDA_SETUP
        when: has_otc_instruments
        
  FUND_ACCOUNTING:
    mandatory: [CLIENT_SETUP, PRODUCT_SELECTION, KYC_REVIEW, INSTRUMENT_UNIVERSE, PRICING_SETUP]
    adds_to_stage:
      PRICING_SETUP:
        extra_entities: [nav_calculation_config]
        extra_verbs: [pricing-config.set-nav-frequency]
        
  PRIME_BROKERAGE:
    mandatory: [CLIENT_SETUP, PRODUCT_SELECTION, KYC_REVIEW, INSTRUMENT_UNIVERSE, LIFECYCLE_RESOURCES, ISDA_SETUP]
    adds_to_stage:
      LIFECYCLE_RESOURCES:
        extra_entities: [margin_config, stock_loan_config]

# Reverse lookup
entity_stage_mapping:
  cbu: CLIENT_SETUP
  cbu_product_subscription: PRODUCT_SELECTION
  kyc_case: KYC_REVIEW
  entity_workstream: KYC_REVIEW
  doc_request: KYC_REVIEW
  trading_profile: INSTRUMENT_UNIVERSE
  cbu_instrument_universe: INSTRUMENT_UNIVERSE
  cbu_lifecycle_instance: LIFECYCLE_RESOURCES
  cbu_ssi: LIFECYCLE_RESOURCES
  isda_agreement: ISDA_SETUP
  csa_agreement: ISDA_SETUP
  cbu_pricing_config: PRICING_SETUP
```

- [ ] Create semantic_stage_map.yaml
- [ ] Add relevant_verbs to each stage
- [ ] Add examples_path for RAG

### 2.2 State Derivation

**File:** `rust/crates/entity-gateway/src/semantic/state.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticState {
    pub stages: Vec<StageState>,
    pub progress: Progress,
    pub next_actionable: Vec<String>,
    pub blocking: Vec<String>,
    pub gaps: Vec<EntityGap>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageState {
    pub code: String,
    pub name: String,
    pub description: String,
    pub status: StageStatus,
    pub entities: Vec<EntityState>,
    pub is_blocking: bool,
    pub depends_on: Vec<String>,
    pub relevant_verbs: Vec<String>,
    pub position: StagePosition,  // For UI layout
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagePosition {
    pub level: usize,      // Depth in DAG (0 = root)
    pub column: usize,     // Horizontal position at this level
    pub parents: Vec<String>,
    pub children: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StageStatus {
    Complete,
    InProgress,
    NotStarted,
    Blocked,
    NotRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityState {
    pub entity_type: String,
    pub required: bool,
    pub exists: bool,
    pub count: usize,
    pub items: Vec<EntityInstance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityInstance {
    pub id: Uuid,
    pub display_name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityGap {
    pub entity_type: String,
    pub stage_code: String,
    pub stage_name: String,
    pub description: String,
    pub suggested_verb: String,
}

impl SemanticState {
    pub async fn derive(pool: &PgPool, cbu_id: Uuid, products: &[String]) -> Result<Self> {
        let map = load_semantic_stage_map()?;
        
        // 1. Determine required stages from products
        let required_stage_codes = compute_required_stages(&map, products);
        
        // 2. Query existing entities
        let existing = query_existing_entities(pool, cbu_id).await?;
        
        // 3. Build stage states with positions for DAG layout
        let stages = build_stage_states(&map, &required_stage_codes, &existing, products)?;
        
        // 4. Compute progress
        let complete = stages.iter().filter(|s| s.status == StageStatus::Complete).count();
        let progress = Progress {
            complete,
            total: stages.len(),
            percentage: (complete as f32 / stages.len() as f32) * 100.0,
        };
        
        // 5. Find next actionable (dependencies met, not complete)
        let next_actionable = stages.iter()
            .filter(|s| s.status == StageStatus::NotStarted || s.status == StageStatus::InProgress)
            .filter(|s| dependencies_met(&s.depends_on, &stages))
            .map(|s| s.code.clone())
            .collect();
        
        // 6. Find blocking stages
        let blocking = stages.iter()
            .filter(|s| s.is_blocking && s.status != StageStatus::Complete)
            .map(|s| s.code.clone())
            .collect();
        
        // 7. Compute gaps with suggestions
        let gaps = compute_gaps(&stages, &map);
        
        Ok(Self { stages, progress, next_actionable, blocking, gaps })
    }
}

fn build_stage_states(
    map: &SemanticStageMap,
    required: &[String],
    existing: &HashMap<String, Vec<EntityInstance>>,
    products: &[String],
) -> Result<Vec<StageState>> {
    // Topo sort for positions
    let sorted = topo_sort(&map.stages)?;
    let levels = compute_levels(&map.stages);
    
    required.iter().map(|code| {
        let def = map.stages.iter().find(|s| &s.code == code).unwrap();
        
        // Get required entities (including product extras)
        let required_entities = get_required_entities(map, code, products);
        
        // Check each entity
        let entities: Vec<EntityState> = required_entities.iter().map(|et| {
            let items = existing.get(et).cloned().unwrap_or_default();
            EntityState {
                entity_type: et.clone(),
                required: true,
                exists: !items.is_empty(),
                count: items.len(),
                items,
            }
        }).collect();
        
        // Compute status
        let all_exist = entities.iter().all(|e| e.exists);
        let any_exist = entities.iter().any(|e| e.exists);
        let status = if all_exist {
            StageStatus::Complete
        } else if any_exist {
            StageStatus::InProgress
        } else {
            StageStatus::NotStarted
        };
        
        // Position for UI
        let position = StagePosition {
            level: levels.get(code).copied().unwrap_or(0),
            column: 0, // Computed later
            parents: def.depends_on.clone(),
            children: find_children(&map.stages, code),
        };
        
        Ok(StageState {
            code: code.clone(),
            name: def.name.clone(),
            description: def.description.clone(),
            status,
            entities,
            is_blocking: def.blocking,
            depends_on: def.depends_on.clone(),
            relevant_verbs: def.relevant_verbs.clone(),
            position,
        })
    }).collect()
}
```

- [ ] Implement SemanticState::derive
- [ ] Implement position calculation for DAG layout
- [ ] Implement gap computation with suggestions

---

## PART 3: Agent Prompt Builder

### 3.1 Combined Prompt

**File:** `rust/crates/entity-gateway/src/agent/prompt_builder.rs`

```rust
pub struct AgentPromptBuilder<'a> {
    session: &'a AgentSession,
    verb_registry: &'a VerbRegistry,
}

impl<'a> AgentPromptBuilder<'a> {
    pub fn build(&self, user_message: &str) -> String {
        let mut prompt = String::new();
        
        // ═══════════════════════════════════════════════════════════════
        // SECTION 1: Context
        // ═══════════════════════════════════════════════════════════════
        prompt.push_str("# Context\n\n");
        
        if let Some(cbu) = &self.session.cbu {
            prompt.push_str(&format!(
                "Working with: **{}** ({}, {})\n",
                cbu.name, cbu.jurisdiction, cbu.category
            ));
            prompt.push_str(&format!("Products: {}\n\n", cbu.products.join(", ")));
        } else {
            prompt.push_str("No CBU selected.\n\n");
            return prompt + &format!("User: {}", user_message);
        }
        
        // ═══════════════════════════════════════════════════════════════
        // SECTION 2: Journey State
        // ═══════════════════════════════════════════════════════════════
        if let Some(state) = &self.session.semantic_state {
            prompt.push_str("# Onboarding Journey\n\n");
            prompt.push_str(&format!(
                "Progress: {}/{} stages ({:.0}%)\n\n",
                state.progress.complete,
                state.progress.total,
                state.progress.percentage
            ));
            
            // Stage summary
            prompt.push_str("## Stages\n");
            for stage in &state.stages {
                let icon = match stage.status {
                    StageStatus::Complete => "✓",
                    StageStatus::InProgress => "◐",
                    StageStatus::NotStarted => "○",
                    StageStatus::Blocked => "⊘",
                    StageStatus::NotRequired => "—",
                };
                let blocking = if stage.is_blocking && stage.status != StageStatus::Complete {
                    " ⚠️ BLOCKING"
                } else {
                    ""
                };
                prompt.push_str(&format!(
                    "{} **{}** - {}{}\n",
                    icon, stage.name, stage.description, blocking
                ));
            }
            prompt.push_str("\n");
            
            // Current focus
            if let Some(focus) = &self.session.focus {
                prompt.push_str(&format!(
                    "## Current Focus: {}\n\n",
                    focus.stage_name
                ));
            }
            
            // Next actionable
            if !state.next_actionable.is_empty() {
                prompt.push_str(&format!(
                    "## Next Actionable Stages\n{}\n\n",
                    state.next_actionable.join(", ")
                ));
            }
            
            // Gaps
            if !state.gaps.is_empty() {
                prompt.push_str("## Missing to Proceed\n");
                for gap in state.gaps.iter().take(5) {
                    prompt.push_str(&format!(
                        "- **{}** for {} → use `{}`\n",
                        gap.entity_type, gap.stage_name, gap.suggested_verb
                    ));
                }
                if state.gaps.len() > 5 {
                    prompt.push_str(&format!("- ...and {} more\n", state.gaps.len() - 5));
                }
                prompt.push_str("\n");
            }
        }
        
        // ═══════════════════════════════════════════════════════════════
        // SECTION 3: Available Actions (filtered by stage)
        // ═══════════════════════════════════════════════════════════════
        prompt.push_str("# Available Actions\n\n");
        
        let relevant_verbs = self.get_relevant_verbs();
        for (domain, verbs) in &relevant_verbs {
            prompt.push_str(&format!("## {}\n", domain));
            for verb in verbs {
                prompt.push_str(&format!("- `{}`: {}\n", verb.name, verb.description));
            }
            prompt.push_str("\n");
        }
        
        // ═══════════════════════════════════════════════════════════════
        // SECTION 4: Symbols from Session
        // ═══════════════════════════════════════════════════════════════
        if !self.session.symbols.is_empty() {
            prompt.push_str("# Available Symbols\n\n");
            prompt.push_str("These can be referenced in DSL:\n");
            for (name, sym) in &self.session.symbols {
                prompt.push_str(&format!(
                    "- `@{}` = {} ({})\n",
                    name, sym.display, sym.entity_type
                ));
            }
            prompt.push_str("\n");
        }
        
        // ═══════════════════════════════════════════════════════════════
        // SECTION 5: Examples (RAG - if focus set)
        // ═══════════════════════════════════════════════════════════════
        if let Some(focus) = &self.session.focus {
            if let Some(examples) = self.load_examples_for_stage(&focus.stage_code) {
                prompt.push_str("# Examples\n\n");
                prompt.push_str(&examples);
                prompt.push_str("\n");
            }
        }
        
        // ═══════════════════════════════════════════════════════════════
        // SECTION 6: User Message
        // ═══════════════════════════════════════════════════════════════
        prompt.push_str(&format!("# User Request\n\n{}\n", user_message));
        
        prompt
    }
    
    fn get_relevant_verbs(&self) -> HashMap<String, Vec<VerbInfo>> {
        // Get verbs relevant to current/next stages
        let stage_codes: Vec<&str> = match &self.session.focus {
            Some(focus) => vec![&focus.stage_code],
            None => self.session.semantic_state
                .as_ref()
                .map(|s| s.next_actionable.iter().map(|x| x.as_str()).collect())
                .unwrap_or_default(),
        };
        
        // Filter verb registry to those in stage_codes
        self.verb_registry.verbs_for_stages(&stage_codes)
    }
    
    fn load_examples_for_stage(&self, stage_code: &str) -> Option<String> {
        // Load from templates/{stage}/ directory
        let map = load_semantic_stage_map().ok()?;
        let stage = map.stages.iter().find(|s| s.code == stage_code)?;
        let path = stage.examples_path.as_ref()?;
        
        // Load and format examples
        load_examples_from_path(path).ok()
    }
}
```

- [ ] Implement AgentPromptBuilder
- [ ] Implement get_relevant_verbs (filter by stage)
- [ ] Implement load_examples_for_stage (RAG)

### 3.2 Wire into Agent Service

```rust
// In agent request handler

async fn handle_agent_message(
    state: &AppState,
    session: &mut AgentSession,
    user_message: &str,
) -> Result<AgentResponse> {
    // Refresh semantic state
    if let Some(cbu) = &session.cbu {
        session.semantic_state = Some(
            SemanticState::derive(&state.pool, cbu.id, &cbu.products).await?
        );
    }
    
    // Build prompt
    let prompt_builder = AgentPromptBuilder {
        session,
        verb_registry: &state.verb_registry,
    };
    let prompt = prompt_builder.build(user_message);
    
    // Call LLM
    let response = call_llm(&prompt).await?;
    
    // Parse DSL from response (if any)
    let dsl = extract_dsl(&response)?;
    
    Ok(AgentResponse {
        message: response,
        dsl,
        session_updated: true,
    })
}

// After DSL execution
async fn after_execution(
    session: &mut AgentSession,
    result: &ExecutionResult,
) {
    // Capture symbols
    session.capture_symbols(result);
    
    // Refresh semantic state (entities changed)
    if let Some(cbu) = &session.cbu {
        session.semantic_state = Some(
            SemanticState::derive(&state.pool, cbu.id, &cbu.products).await?
        );
    }
}
```

- [ ] Find existing agent handler
- [ ] Integrate prompt builder
- [ ] Add post-execution refresh

---

## PART 4: UI Journey Visualization

### 4.1 API Endpoints

**File:** `rust/crates/ob-poc-web/src/api/session.rs`

```rust
/// Get current session state (for UI)
#[get("/api/session")]
async fn get_session(
    state: web::Data<AppState>,
    session_id: web::Query<SessionId>,
) -> Result<HttpResponse> {
    let session = state.session_store.get(&session_id.id)?;
    Ok(HttpResponse::Ok().json(session))
}

/// Set CBU context
#[post("/api/session/cbu")]
async fn set_cbu(
    state: web::Data<AppState>,
    session_id: web::Query<SessionId>,
    body: web::Json<SetCbuRequest>,
) -> Result<HttpResponse> {
    let session = AgentSession::load_for_cbu(&state.pool, body.cbu_id).await?;
    state.session_store.set(&session_id.id, session.clone())?;
    Ok(HttpResponse::Ok().json(session))
}

/// Set stage focus (user clicked a stage)
#[post("/api/session/focus")]
async fn set_focus(
    state: web::Data<AppState>,
    session_id: web::Query<SessionId>,
    body: web::Json<SetFocusRequest>,
) -> Result<HttpResponse> {
    let mut session = state.session_store.get(&session_id.id)?;
    session.set_focus(&body.stage_code, &body.stage_name);
    state.session_store.set(&session_id.id, session.clone())?;
    Ok(HttpResponse::Ok().json(session))
}

/// Get journey visualization data
#[get("/api/session/journey")]
async fn get_journey(
    state: web::Data<AppState>,
    session_id: web::Query<SessionId>,
) -> Result<HttpResponse> {
    let session = state.session_store.get(&session_id.id)?;
    
    if let Some(semantic_state) = &session.semantic_state {
        let viz = JourneyVisualization::from_semantic_state(semantic_state);
        Ok(HttpResponse::Ok().json(viz))
    } else {
        Ok(HttpResponse::Ok().json(JourneyVisualization::empty()))
    }
}
```

- [ ] Create session API endpoints
- [ ] Add to router

### 4.2 Journey Visualization Data

```rust
/// Data structure for UI to render the DAG
#[derive(Debug, Clone, Serialize)]
pub struct JourneyVisualization {
    pub nodes: Vec<StageNode>,
    pub edges: Vec<StageEdge>,
    pub progress: Progress,
    pub focus: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StageNode {
    pub id: String,
    pub name: String,
    pub description: String,
    pub status: String,  // "complete", "in_progress", "not_started", "blocked"
    pub is_blocking: bool,
    pub is_focused: bool,
    pub position: NodePosition,
    pub entities: Vec<EntitySummary>,
    pub actions: Vec<ActionSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodePosition {
    pub x: f32,  // Computed for DAG layout
    pub y: f32,
    pub level: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct StageEdge {
    pub from: String,
    pub to: String,
    pub is_satisfied: bool,  // Dependency met
}

#[derive(Debug, Clone, Serialize)]
pub struct EntitySummary {
    pub entity_type: String,
    pub count: usize,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionSummary {
    pub verb: String,
    pub description: String,
}

impl JourneyVisualization {
    pub fn from_semantic_state(state: &SemanticState) -> Self {
        // Compute DAG layout positions
        let positions = compute_dag_layout(&state.stages);
        
        let nodes = state.stages.iter().map(|stage| {
            let pos = positions.get(&stage.code).unwrap();
            StageNode {
                id: stage.code.clone(),
                name: stage.name.clone(),
                description: stage.description.clone(),
                status: format!("{:?}", stage.status).to_lowercase(),
                is_blocking: stage.is_blocking,
                is_focused: false, // Set from session
                position: NodePosition {
                    x: pos.0,
                    y: pos.1,
                    level: stage.position.level,
                },
                entities: stage.entities.iter().map(|e| EntitySummary {
                    entity_type: e.entity_type.clone(),
                    count: e.count,
                    exists: e.exists,
                }).collect(),
                actions: stage.relevant_verbs.iter().map(|v| ActionSummary {
                    verb: v.clone(),
                    description: String::new(), // Filled from registry
                }).collect(),
            }
        }).collect();
        
        let edges = state.stages.iter().flat_map(|stage| {
            stage.depends_on.iter().map(|dep| StageEdge {
                from: dep.clone(),
                to: stage.code.clone(),
                is_satisfied: state.stages.iter()
                    .find(|s| &s.code == dep)
                    .map(|s| s.status == StageStatus::Complete)
                    .unwrap_or(false),
            })
        }).collect();
        
        Self {
            nodes,
            edges,
            progress: state.progress.clone(),
            focus: None,
        }
    }
}

fn compute_dag_layout(stages: &[StageState]) -> HashMap<String, (f32, f32)> {
    // Sugiyama-style layered layout
    // Level 0 at top, children below
    // Spread horizontally within each level
    
    let mut positions = HashMap::new();
    let mut level_counts: HashMap<usize, usize> = HashMap::new();
    
    for stage in stages {
        let level = stage.position.level;
        let col = *level_counts.get(&level).unwrap_or(&0);
        level_counts.insert(level, col + 1);
        
        let x = col as f32 * 200.0 + 100.0;  // 200px spacing
        let y = level as f32 * 150.0 + 50.0;  // 150px spacing
        
        positions.insert(stage.code.clone(), (x, y));
    }
    
    positions
}
```

- [ ] Create JourneyVisualization struct
- [ ] Implement DAG layout algorithm
- [ ] Add to API response

### 4.3 egui Journey Panel

**File:** `rust/crates/ob-poc-ui/src/panels/journey_panel.rs`

```rust
pub struct JourneyPanel {
    journey: Option<JourneyVisualization>,
    expanded_stage: Option<String>,
    loading: bool,
}

impl JourneyPanel {
    pub fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut AppContext) {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(25, 25, 35))
            .inner_margin(12.0)
            .show(ui, |ui| {
                // Header
                ui.horizontal(|ui| {
                    ui.heading("Onboarding Journey");
                    if let Some(j) = &self.journey {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(format!(
                                "{:.0}% complete",
                                j.progress.percentage
                            ));
                            // Progress bar
                            let progress = j.progress.percentage / 100.0;
                            ui.add(egui::ProgressBar::new(progress).desired_width(100.0));
                        });
                    }
                });
                
                ui.separator();
                
                if let Some(journey) = &self.journey {
                    // DAG visualization
                    self.render_dag(ui, journey, ctx);
                    
                    ui.separator();
                    
                    // Expanded stage detail
                    if let Some(stage_id) = &self.expanded_stage {
                        if let Some(stage) = journey.nodes.iter().find(|n| &n.id == stage_id) {
                            self.render_stage_detail(ui, stage, ctx);
                        }
                    }
                } else if self.loading {
                    ui.spinner();
                } else {
                    ui.label("Select a CBU to see onboarding journey");
                }
            });
    }
    
    fn render_dag(&mut self, ui: &mut egui::Ui, journey: &JourneyVisualization, ctx: &mut AppContext) {
        let (response, painter) = ui.allocate_painter(
            egui::vec2(ui.available_width(), 400.0),
            egui::Sense::click(),
        );
        
        let rect = response.rect;
        
        // Draw edges first (behind nodes)
        for edge in &journey.edges {
            let from_pos = journey.nodes.iter()
                .find(|n| n.id == edge.from)
                .map(|n| egui::pos2(n.position.x, n.position.y));
            let to_pos = journey.nodes.iter()
                .find(|n| n.id == edge.to)
                .map(|n| egui::pos2(n.position.x, n.position.y));
            
            if let (Some(from), Some(to)) = (from_pos, to_pos) {
                let color = if edge.is_satisfied {
                    egui::Color32::GREEN
                } else {
                    egui::Color32::GRAY
                };
                painter.line_segment(
                    [rect.min + from.to_vec2(), rect.min + to.to_vec2()],
                    egui::Stroke::new(2.0, color),
                );
            }
        }
        
        // Draw nodes
        for node in &journey.nodes {
            let center = rect.min + egui::vec2(node.position.x, node.position.y);
            let node_rect = egui::Rect::from_center_size(center, egui::vec2(160.0, 60.0));
            
            // Node color based on status
            let (bg_color, border_color) = match node.status.as_str() {
                "complete" => (egui::Color32::from_rgb(40, 80, 40), egui::Color32::GREEN),
                "in_progress" => (egui::Color32::from_rgb(80, 80, 40), egui::Color32::YELLOW),
                "blocked" => (egui::Color32::from_rgb(80, 40, 40), egui::Color32::RED),
                _ => (egui::Color32::from_rgb(50, 50, 50), egui::Color32::GRAY),
            };
            
            // Highlight if focused
            let border_width = if Some(&node.id) == self.expanded_stage.as_ref() { 3.0 } else { 1.0 };
            
            painter.rect(
                node_rect,
                4.0,
                bg_color,
                egui::Stroke::new(border_width, border_color),
            );
            
            // Status icon
            let icon = match node.status.as_str() {
                "complete" => "✓",
                "in_progress" => "◐",
                "blocked" => "⊘",
                _ => "○",
            };
            
            painter.text(
                node_rect.left_center() + egui::vec2(10.0, 0.0),
                egui::Align2::LEFT_CENTER,
                icon,
                egui::FontId::proportional(16.0),
                border_color,
            );
            
            // Stage name
            painter.text(
                node_rect.center(),
                egui::Align2::CENTER_CENTER,
                &node.name,
                egui::FontId::proportional(12.0),
                egui::Color32::WHITE,
            );
            
            // Blocking indicator
            if node.is_blocking && node.status != "complete" {
                painter.text(
                    node_rect.right_top() + egui::vec2(-5.0, 5.0),
                    egui::Align2::RIGHT_TOP,
                    "⚠️",
                    egui::FontId::proportional(10.0),
                    egui::Color32::YELLOW,
                );
            }
            
            // Click detection
            if ui.rect_contains_pointer(node_rect) && response.clicked() {
                self.expanded_stage = Some(node.id.clone());
                ctx.set_stage_focus(&node.id, &node.name);
            }
        }
    }
    
    fn render_stage_detail(&self, ui: &mut egui::Ui, stage: &StageNode, ctx: &mut AppContext) {
        ui.heading(&stage.name);
        ui.label(&stage.description);
        
        ui.add_space(8.0);
        
        // Entities
        ui.label("Entities:");
        for entity in &stage.entities {
            ui.horizontal(|ui| {
                let icon = if entity.exists { "✓" } else { "○" };
                let color = if entity.exists { egui::Color32::GREEN } else { egui::Color32::GRAY };
                ui.colored_label(color, icon);
                ui.label(&entity.entity_type);
                if entity.count > 0 {
                    ui.label(format!("({})", entity.count));
                }
            });
        }
        
        ui.add_space(8.0);
        
        // Actions
        ui.label("Actions:");
        for action in &stage.actions {
            if ui.button(&action.verb).clicked() {
                ctx.insert_verb_to_chat(&action.verb);
            }
        }
    }
}
```

- [ ] Create journey_panel.rs
- [ ] Implement DAG rendering
- [ ] Implement click-to-focus
- [ ] Implement stage detail expansion
- [ ] Wire into main UI layout

---

## PART 5: Integration

### 5.1 Wire Everything Together

```rust
// In main UI app

pub struct App {
    // Existing
    chat_panel: ChatPanel,
    graph_panel: GraphPanel,
    
    // New
    journey_panel: JourneyPanel,
    session: AgentSession,
}

impl App {
    fn update(&mut self, ctx: &egui::Context) {
        // Left: CBU selector + Journey
        egui::SidePanel::left("left_panel").show(ctx, |ui| {
            // CBU dropdown (existing)
            self.cbu_selector.ui(ui);
            
            // Journey visualization (new)
            self.journey_panel.ui(ui, &mut self.app_ctx);
        });
        
        // Center: Chat with agent
        egui::CentralPanel::default().show(ctx, |ui| {
            self.chat_panel.ui(ui, &mut self.session);
        });
        
        // Right: Entity graph (existing)
        egui::SidePanel::right("right_panel").show(ctx, |ui| {
            self.graph_panel.ui(ui);
        });
    }
    
    fn on_cbu_selected(&mut self, cbu_id: Uuid) {
        // Load session with semantic state
        self.session = AgentSession::load_for_cbu(&self.pool, cbu_id).await.unwrap();
        
        // Update journey panel
        self.journey_panel.journey = Some(
            JourneyVisualization::from_semantic_state(
                self.session.semantic_state.as_ref().unwrap()
            )
        );
    }
    
    fn on_stage_clicked(&mut self, stage_code: &str, stage_name: &str) {
        // Update session focus
        self.session.set_focus(stage_code, stage_name);
        
        // Journey panel updates expanded stage
        self.journey_panel.expanded_stage = Some(stage_code.to_string());
        
        // Agent now knows focus for next message
    }
    
    fn on_dsl_executed(&mut self, result: &ExecutionResult) {
        // Capture symbols
        self.session.capture_symbols(result);
        
        // Refresh semantic state
        if let Some(cbu) = &self.session.cbu {
            self.session.semantic_state = Some(
                SemanticState::derive(&self.pool, cbu.id, &cbu.products).await.unwrap()
            );
        }
        
        // Update journey visualization
        self.journey_panel.journey = self.session.semantic_state.as_ref().map(
            |s| JourneyVisualization::from_semantic_state(s)
        );
    }
}
```

- [ ] Wire journey panel into main app
- [ ] Handle CBU selection → load session
- [ ] Handle stage click → set focus
- [ ] Handle DSL execution → refresh state

### 5.2 Event Flow

```
┌─────────────────────────────────────────────────────────────────┐
│  USER ACTION                    SYSTEM RESPONSE                 │
├─────────────────────────────────────────────────────────────────┤
│  Select CBU dropdown     →      Load session                    │
│                          →      Derive semantic state           │
│                          →      Render journey DAG              │
│                          →      Agent prompt updated            │
├─────────────────────────────────────────────────────────────────┤
│  Click stage in DAG      →      Set focus in session            │
│                          →      Highlight stage in UI           │
│                          →      Filter verbs to stage           │
│                          →      Load stage examples             │
│                          →      Agent prompt updated            │
├─────────────────────────────────────────────────────────────────┤
│  Chat with agent         →      Prompt includes context         │
│                          →      Agent generates DSL             │
│                          →      Show DSL preview                │
├─────────────────────────────────────────────────────────────────┤
│  Execute DSL             →      Create/update entities          │
│                          →      Capture symbols                 │
│                          →      Refresh semantic state          │
│                          →      Journey DAG updates             │
│                          →      Stage status changes            │
└─────────────────────────────────────────────────────────────────┘
```

---

## PART 6: Testing

### 6.1 End-to-End Scenario

```
1. Start app, select "Alpha Fund" from CBU dropdown

2. Journey panel shows:
   ✓ Client Setup
   ✓ Product Selection (Global Custody)
   ○ KYC Review ⚠️ BLOCKING
   ○ Instrument Universe
   ⊘ Lifecycle Resources (blocked)

3. Click "KYC Review" stage
   → Stage expands showing missing entities
   → Agent prompt now focused on KYC
   → Chat shows relevant actions

4. Chat: "Start the KYC process"

5. Agent responds with DSL:
   (kyc-case.create cbu-id:@alpha-fund case-type:INITIAL)
   
6. User confirms → Execute

7. Journey updates:
   ◐ KYC Review (in progress)
   → Shows: kyc_case exists (1), entity_workstream missing

8. Session shows symbol:
   @kyc-case = CASE-2024-042

9. Chat: "Add workstream for the fund entity"

10. Agent uses @kyc-case symbol:
    (entity-workstream.create case-id:@kyc-case entity-id:@alpha-fund)

11. Execute → Journey updates → Progress increases
```

- [ ] Write integration test
- [ ] Test CBU selection flow
- [ ] Test stage click focus
- [ ] Test agent prompt content
- [ ] Test DSL execution refresh
- [ ] Test symbol capture and reuse

---

## Deliverables

### Config
- [ ] `config/ontology/semantic_stage_map.yaml`

### Rust Types
- [ ] `session/mod.rs` - AgentSession, CbuContext, StageFocus
- [ ] `semantic/state.rs` - SemanticState, StageState
- [ ] `semantic/visualization.rs` - JourneyVisualization

### Rust Logic  
- [ ] `session/loader.rs` - load_for_cbu, capture_symbols
- [ ] `semantic/derive.rs` - SemanticState::derive
- [ ] `agent/prompt_builder.rs` - AgentPromptBuilder

### API
- [ ] `/api/session` - get/set session
- [ ] `/api/session/cbu` - set CBU context
- [ ] `/api/session/focus` - set stage focus
- [ ] `/api/session/journey` - get visualization data

### UI
- [ ] `journey_panel.rs` - DAG visualization
- [ ] Wire into main app layout
- [ ] Click handlers

### Tests
- [ ] Unit tests for state derivation
- [ ] Integration test for full flow

---

## Success Criteria

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│   USER                              AGENT                       │
│   ────                              ─────                       │
│   Sees journey DAG          ═══     Knows journey state        │
│   Clicks stage to focus     ═══     Filters to relevant verbs  │
│   Sees progress update      ═══     Generates correct DSL      │
│   Sees symbols available    ═══     Uses symbols in DSL        │
│                                                                 │
│                  SHARED UNDERSTANDING                           │
│                                                                 │
│   "We both know where we are, what's next, and how to proceed" │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**The test:** User can onboard a client by clicking stages and chatting, with agent generating correct DSL each step, and both seeing progress update in real-time.
