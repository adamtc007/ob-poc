# Research Workflows - Bounded Non-Determinism Architecture

> **Status:** ✅ IMPLEMENTED
> **Priority:** High - Required for UBO gap resolution
> **Created:** 2026-01-10
> **Completed:** 2026-01-10
> **Estimated Effort:** 85-100 hours
> **Dependencies:** 
>   - 019-group-taxonomy-intra-company-ownership.md (ownership graph, gaps)
>   - Existing GLEIF integration (refactor under this pattern)
>   - CLAUDE.md and annexes (review before implementation)
>   - Session/REPL/Viewport infrastructure

---

## Implementation Preamble

**Before implementing any phase of this TODO, Claude must:**

```
1. Review /CLAUDE.md for project conventions and patterns
2. Review /docs/entity-model-ascii.md for entity taxonomy
3. Review /docs/dsl-spec.md for verb definition patterns
4. Review /docs/repl-viewport.md for session/scope context
5. Review existing GLEIF implementation as reference pattern
6. Review /rust/config/verbs/*.yaml for verb YAML conventions
7. Review session mode infrastructure for agent integration
```

This ensures implementation aligns with established project architecture.

---

## System Integration Overview

**This is critical: Research verbs must wire through the existing infrastructure or they are invisible.**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    FULL SYSTEM WIRING                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   ┌─────────────────┐                                                       │
│   │   LLM Service   │ ◄── Claude API                                        │
│   │   (reasoning)   │                                                       │
│   └────────┬────────┘                                                       │
│            │ prompts, responses                                             │
│            ▼                                                                │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                    AGENT CONTROLLER                                  │   │
│   │                                                                      │   │
│   │  • Loads prompt templates                                           │   │
│   │  • Manages agent loop state                                         │   │
│   │  • Parses LLM output for DSL verbs                                  │   │
│   │  • Routes checkpoints to UI                                         │   │
│   │  • Enforces confidence thresholds                                   │   │
│   │  • Respects session scope                                           │   │
│   │                                                                      │   │
│   └───────────────────────────┬─────────────────────────────────────────┘   │
│                               │                                             │
│           ┌───────────────────┼───────────────────┐                        │
│           │                   │                   │                        │
│           ▼                   ▼                   ▼                        │
│   ┌─────────────┐     ┌─────────────┐     ┌─────────────┐                  │
│   │    REPL     │     │   Session   │     │   Viewport  │                  │
│   │             │     │             │     │    (egui)   │                  │
│   │ • Parses    │◄───►│ • Scope ctx │◄───►│             │                  │
│   │   DSL       │     │ • Variables │     │ • REPL pane │                  │
│   │ • Executes  │     │ • Mode      │     │ • Checkpoint│                  │
│   │   verbs     │     │ • Agent     │     │   dialogs   │                  │
│   │ • Returns   │     │   state     │     │ • Progress  │                  │
│   │   results   │     │             │     │   display   │                  │
│   └──────┬──────┘     └─────────────┘     └──────▲──────┘                  │
│          │                                       │                         │
│          │                                       │                         │
│          ▼                                       │                         │
│   ┌─────────────┐                                │                         │
│   │  Handlers   │                                │                         │
│   │  (Rust/Go)  │ ─────────────────────────────► │                         │
│   │             │    checkpoint events,          │                         │
│   │             │    progress updates            │                         │
│   └──────┬──────┘                                │                         │
│          │                                       │                         │
│          ▼                                       │                         │
│   ┌─────────────┐                                │                         │
│   │  Database   │                                │                         │
│   │             │ ◄──────────────────────────────┘                         │
│   │ • Entities  │    user input (selections,                               │
│   │ • Decisions │    confirmations, overrides)                             │
│   │ • Actions   │                                                          │
│   └─────────────┘                                                          │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Agent Integration

### Session Modes

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    SESSION MODE MODEL                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Session gains a MODE field:                                               │
│                                                                              │
│   MODE: MANUAL (default)                                                    │
│   ══════════════════════                                                     │
│   • User types DSL commands                                                 │
│   • REPL executes immediately                                               │
│   • Results displayed                                                       │
│   • Standard current behavior                                               │
│                                                                              │
│   MODE: AGENT                                                               │
│   ═══════════════                                                            │
│   • Agent controller running                                                │
│   • LLM generates DSL commands                                              │
│   • REPL executes on agent's behalf                                         │
│   • User supervises, responds to checkpoints                                │
│   • Can pause/resume/stop                                                   │
│                                                                              │
│   MODE: HYBRID                                                              │
│   ════════════                                                               │
│   • Agent running but user can interleave commands                          │
│   • User commands take priority                                             │
│   • Agent resumes after user command completes                              │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Session State Extension

```rust
// Extension to existing Session struct

pub struct Session {
    // Existing fields...
    pub scope: Scope,
    pub variables: HashMap<String, Value>,
    
    // New agent fields
    pub mode: SessionMode,
    pub agent_state: Option<AgentState>,
}

pub enum SessionMode {
    Manual,
    Agent,
    Hybrid,
}

pub struct AgentState {
    pub agent_session_id: Uuid,
    pub task: AgentTask,
    pub status: AgentStatus,
    pub target_entity_id: Option<Uuid>,
    pub target_group_id: Option<Uuid>,
    
    // Loop state
    pub loop_iteration: u32,
    pub max_iterations: u32,
    pub current_prompt: String,
    
    // Checkpoint state
    pub pending_checkpoint: Option<Checkpoint>,
    
    // History
    pub decisions: Vec<DecisionRef>,
    pub actions: Vec<ActionRef>,
    
    // Timing
    pub started_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
}

pub enum AgentTask {
    ResolveGaps,
    ChainResearch,
    EnrichEntity,
    EnrichGroup,
    ScreenEntities,
}

pub enum AgentStatus {
    Running,
    Paused,
    Checkpoint,  // Awaiting user input
    Complete,
    Failed,
    Cancelled,
}

pub struct Checkpoint {
    pub checkpoint_id: Uuid,
    pub checkpoint_type: CheckpointType,
    pub context: CheckpointContext,
    pub candidates: Vec<Candidate>,
    pub created_at: DateTime<Utc>,
}

pub enum CheckpointType {
    AmbiguousMatch,     // Multiple candidates, need selection
    HighStakes,         // Auto-match but context requires confirmation
    ScreeningHit,       // Sanctions/PEP match found
    ValidationFailure,  // Post-import validation failed
    SourceUnavailable,  // Preferred source failed, confirm fallback
}
```

### Agent Controller

```rust
pub struct AgentController {
    session: Arc<RwLock<Session>>,
    repl: Arc<Repl>,
    llm: Arc<LlmService>,
    prompt_loader: PromptLoader,
    event_tx: mpsc::Sender<AgentEvent>,
}

impl AgentController {
    
    /// Main entry point - starts agent for a task
    pub async fn start(&self, task: AgentTask, params: AgentParams) -> Result<Uuid> {
        let agent_session_id = Uuid::new_v4();
        
        // Update session
        {
            let mut session = self.session.write().await;
            session.mode = SessionMode::Agent;
            session.agent_state = Some(AgentState {
                agent_session_id,
                task: task.clone(),
                status: AgentStatus::Running,
                target_entity_id: params.entity_id,
                target_group_id: params.group_id,
                loop_iteration: 0,
                max_iterations: params.max_iterations.unwrap_or(50),
                current_prompt: String::new(),
                pending_checkpoint: None,
                decisions: vec![],
                actions: vec![],
                started_at: Utc::now(),
                last_activity: Utc::now(),
            });
        }
        
        // Emit start event for UI
        self.event_tx.send(AgentEvent::Started { 
            agent_session_id, 
            task: task.clone() 
        }).await?;
        
        // Spawn the loop
        let controller = self.clone();
        tokio::spawn(async move {
            controller.run_loop().await
        });
        
        Ok(agent_session_id)
    }
    
    /// The main agent loop
    async fn run_loop(&self) -> Result<AgentResult> {
        loop {
            // Check status
            let status = {
                let session = self.session.read().await;
                session.agent_state.as_ref().map(|s| s.status.clone())
            };
            
            match status {
                Some(AgentStatus::Running) => {
                    // Continue loop
                }
                Some(AgentStatus::Paused) => {
                    // Wait for resume signal
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
                Some(AgentStatus::Checkpoint) => {
                    // Wait for user response
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
                _ => break,
            }
            
            // Get current scope from session (agent respects session scope)
            let scope = {
                let session = self.session.read().await;
                session.scope.clone()
            };
            
            // Step 1: Identify gaps using DSL
            let gaps = self.execute_dsl(
                "ownership.identify-gaps(:entity-id @target)"
            ).await?;
            
            if gaps.is_empty() {
                self.complete(AgentResult::Success).await?;
                break;
            }
            
            // Step 2: Load orchestration prompt
            let context = self.build_context(&gaps, &scope).await?;
            let prompt = self.prompt_loader.load(
                "research/orchestration/resolve-gap.md",
                &context
            )?;
            
            self.update_current_prompt(&prompt).await;
            
            // Step 3: LLM reasons about strategy
            let strategy = self.llm.complete(&prompt).await?;
            
            // Step 4: Execute strategy (search, evaluate, import or checkpoint)
            match self.execute_strategy(&strategy).await? {
                StrategyResult::Imported(action_id) => {
                    self.record_action(action_id).await?;
                }
                StrategyResult::NeedsCheckpoint(checkpoint) => {
                    self.request_checkpoint(checkpoint).await?;
                    // Loop will wait at Checkpoint status
                }
                StrategyResult::NoMatch => {
                    self.try_next_source_or_skip().await?;
                }
            }
            
            // Increment and check limits
            self.increment_iteration().await?;
        }
        
        Ok(AgentResult::Success)
    }
    
    /// Execute DSL via REPL
    async fn execute_dsl(&self, dsl: &str) -> Result<Value> {
        // Emit event for UI
        self.event_tx.send(AgentEvent::Executing { 
            dsl: dsl.to_string() 
        }).await?;
        
        // Execute through REPL
        let result = self.repl.execute(dsl).await?;
        
        // Emit result
        self.event_tx.send(AgentEvent::Executed { 
            dsl: dsl.to_string(),
            result: result.clone(),
        }).await?;
        
        Ok(result)
    }
    
    /// Handle checkpoint response from user
    pub async fn respond_checkpoint(&self, response: CheckpointResponse) -> Result<()> {
        let checkpoint = {
            let mut session = self.session.write().await;
            session.agent_state.as_mut()
                .and_then(|s| s.pending_checkpoint.take())
        };
        
        if let Some(checkpoint) = checkpoint {
            match response {
                CheckpointResponse::Select(index) => {
                    let selected = &checkpoint.candidates[index];
                    
                    // Record decision
                    let decision_id = self.execute_dsl(&format!(
                        "research.workflow.record-decision(\
                            :search-query \"{}\" \
                            :source-provider \"{}\" \
                            :candidates-found {:?} \
                            :selected-key \"{}\" \
                            :confidence {} \
                            :reasoning \"User selected from checkpoint\" \
                            :decision-type \"USER_SELECTED\")",
                        checkpoint.context.search_query,
                        checkpoint.context.source,
                        checkpoint.candidates,
                        selected.key,
                        selected.score,
                    )).await?;
                    
                    // Execute import with selected key
                    self.execute_import(&checkpoint.context.source, &selected.key, decision_id).await?;
                    
                    // Resume loop
                    self.set_status(AgentStatus::Running).await;
                }
                CheckpointResponse::Reject => {
                    // Record rejection, try next source
                    self.try_next_source_or_skip().await?;
                    self.set_status(AgentStatus::Running).await;
                }
                CheckpointResponse::ManualOverride(key) => {
                    // User provided correct key manually
                    let decision_id = self.execute_dsl(&format!(
                        "research.workflow.record-decision(\
                            :search-query \"{}\" \
                            :source-provider \"{}\" \
                            :candidates-found {:?} \
                            :selected-key \"{}\" \
                            :confidence 1.0 \
                            :reasoning \"User manual override\" \
                            :decision-type \"USER_SELECTED\")",
                        checkpoint.context.search_query,
                        checkpoint.context.source,
                        checkpoint.candidates,
                        key,
                    )).await?;
                    
                    self.execute_import(&checkpoint.context.source, &key, decision_id).await?;
                    self.set_status(AgentStatus::Running).await;
                }
            }
        }
        
        Ok(())
    }
}
```

### Viewport Integration

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    VIEWPORT AGENT UI                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   REPL pane shows agent activity:                                           │
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │  REPL                                         [MODE: AGENT ▶ RUNNING]│   │
│   ├─────────────────────────────────────────────────────────────────────┤   │
│   │  > agent.resolve-gaps(:entity-id @fund-alpha)                       │   │
│   │                                                                      │   │
│   │  🤖 Agent started: RESOLVE_GAPS                                      │   │
│   │     Target: Fund Alpha                                               │   │
│   │     Scope: GROUP @allianzgi                                          │   │
│   │                                                                      │   │
│   │  [1] ownership.identify-gaps(:entity-id @fund-alpha)                │   │
│   │      → Found 2 gaps: HoldCo Ltd, Nominee X                          │   │
│   │                                                                      │   │
│   │  [2] Searching GLEIF for "HoldCo Ltd"...                            │   │
│   │      → 2 candidates found (scores: 0.85, 0.82)                      │   │
│   │                                                                      │   │
│   │  ┌───────────────────────────────────────────────────────────────┐  │   │
│   │  │ ⚠️  CHECKPOINT: Select match for "HoldCo Ltd"                  │  │   │
│   │  │                                                                 │  │   │
│   │  │  [1] HOLDCO LIMITED (LEI: 213800ABC...)                        │  │   │
│   │  │      UK | Active | Score: 0.85                                 │  │   │
│   │  │                                                                 │  │   │
│   │  │  [2] HOLDCO LTD (LEI: 213800XYZ...)                            │  │   │
│   │  │      UK | Active | Score: 0.82                                 │  │   │
│   │  │                                                                 │  │   │
│   │  │  > Enter 1, 2, N (neither), or M (manual): _                   │  │   │
│   │  └───────────────────────────────────────────────────────────────┘  │   │
│   │                                                                      │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│   Status bar: [Iteration 2/50] [Decisions: 0] [Actions: 0] [⏸ Pause] [⏹ Stop]│
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Invocation Phrases

**Critical: The LLM needs hints about when to use research verbs. These phrases trigger verb consideration.**

### agent.yaml

```yaml
domains:
  agent:
    description: "Agent mode control and automated research"
    
    invocation_hints:
      - "find the ownership"
      - "complete the chain"
      - "resolve the gaps"
      - "research this entity"
      - "who owns"
      - "trace the ownership"
      - "fill in the missing"
      - "figure out the UBO"
      - "look up the parent"
      - "find out who controls"
      - "investigate"
      - "do the research"
      - "automate the lookup"
      - "run the agent"
    
    verbs:
      start:
        description: "Start agent mode with a task"
        invocation_phrases:
          - "start the agent"
          - "run agent mode"
          - "automate this"
          - "let the agent handle"
        behavior: plugin
        handler: AgentStartOp
        args:
          - name: task
            type: string
            required: true
            valid_values: [RESOLVE_GAPS, CHAIN_RESEARCH, ENRICH_ENTITY, SCREEN_ENTITIES]
          - name: target-entity-id
            type: uuid
          - name: target-group-id
            type: uuid
          - name: options
            type: object
        effects:
          - sets session.mode to AGENT
          - spawns agent controller loop
        returns:
          type: object

      pause:
        description: "Pause agent execution"
        invocation_phrases:
          - "pause the agent"
          - "hold on"
          - "wait"
          - "stop for now"
        behavior: plugin
        handler: AgentPauseOp

      resume:
        description: "Resume paused agent"
        invocation_phrases:
          - "continue"
          - "resume"
          - "carry on"
          - "keep going"
        behavior: plugin
        handler: AgentResumeOp

      stop:
        description: "Stop agent and return to manual mode"
        invocation_phrases:
          - "stop the agent"
          - "cancel"
          - "abort"
          - "I'll do it manually"
        behavior: plugin
        handler: AgentStopOp

      status:
        description: "Get agent status"
        invocation_phrases:
          - "what's the agent doing"
          - "agent status"
          - "how's it going"
          - "progress"
        behavior: plugin
        handler: AgentStatusOp

      respond-checkpoint:
        description: "Respond to agent checkpoint"
        invocation_phrases:
          - "select the first"
          - "use that one"
          - "neither"
          - "try again"
          - "the correct one is"
        behavior: plugin
        handler: AgentRespondCheckpointOp
        args:
          - name: checkpoint-id
            type: uuid
            required: true
          - name: response
            type: string
            required: true
          - name: manual-key
            type: string
          - name: notes
            type: string

      # =====================================================================
      # TASK-SPECIFIC ENTRY POINTS
      # =====================================================================
      
      resolve-gaps:
        description: "Agent resolves ownership gaps"
        invocation_phrases:
          - "resolve the gaps"
          - "fill in the missing ownership"
          - "complete the ownership structure"
          - "fix the broken chains"
          - "find the missing parents"
          - "who are the ultimate owners"
        behavior: plugin
        handler: AgentResolveGapsOp
        args:
          - name: entity-id
            type: uuid
            required: true
          - name: max-depth
            type: integer
            default: 5
          - name: auto-confirm-threshold
            type: decimal
            default: 0.90
        returns:
          type: object

      chain-research:
        description: "Agent builds complete ownership chain"
        invocation_phrases:
          - "build the ownership chain"
          - "trace ownership to the top"
          - "find all the parents"
          - "complete chain research"
          - "who ultimately owns this"
        behavior: plugin
        handler: AgentChainResearchOp
        args:
          - name: entity-id
            type: uuid
            required: true
          - name: jurisdiction
            type: string
            required: true
          - name: max-depth
            type: integer
            default: 10

      enrich-entity:
        description: "Agent enriches entity with external data"
        invocation_phrases:
          - "enrich this entity"
          - "get more data on"
          - "fill in the details"
          - "look up details for"
          - "find information about"
        behavior: plugin
        handler: AgentEnrichEntityOp
        args:
          - name: entity-id
            type: uuid
            required: true
          - name: fields
            type: array
            default: [HIERARCHY, OFFICERS]

      enrich-group:
        description: "Agent enriches all entities in group"
        invocation_phrases:
          - "enrich the group"
          - "update all entities in the group"
          - "refresh group data"
        behavior: plugin
        handler: AgentEnrichGroupOp
        args:
          - name: group-id
            type: uuid
            required: true

      screen-entities:
        description: "Agent screens entities for sanctions/PEP"
        invocation_phrases:
          - "screen for sanctions"
          - "check PEP status"
          - "run screening"
          - "compliance check"
          - "are there any sanctions"
        behavior: plugin
        handler: AgentScreenEntitiesOp
        args:
          - name: entity-ids
            type: array
          - name: scope
            type: string
            valid_values: [ENTITY, GROUP, CBU]
          - name: scope-id
            type: uuid
```

### research.gleif.yaml

```yaml
domains:
  research.gleif:
    description: "GLEIF - Global LEI Foundation"
    
    invocation_hints:
      - "LEI"
      - "GLEIF"
      - "legal entity identifier"
      - "global LEI"
      - "parent company"
      - "ultimate parent"
      - "corporate hierarchy"
      - "who owns"
    
    verbs:
      import-entity:
        description: "Import entity by LEI"
        invocation_phrases:
          - "import from GLEIF"
          - "get the LEI record"
          - "fetch LEI"
          - "load from GLEIF"
        behavior: plugin
        handler: GleifImportEntityOp
        requires_key: true
        key_type: LEI
        key_validation: "^[A-Z0-9]{20}$"
        args:
          - name: lei
            type: string
            required: true
          - name: target-entity-id
            type: uuid
          - name: decision-id
            type: uuid

      import-hierarchy:
        description: "Import ownership hierarchy by LEI"
        invocation_phrases:
          - "import the GLEIF hierarchy"
          - "get parent chain from GLEIF"
          - "fetch GLEIF ownership"
          - "load corporate structure"
          - "who owns this according to GLEIF"
        behavior: plugin
        handler: GleifImportHierarchyOp
        requires_key: true
        key_type: LEI
        args:
          - name: lei
            type: string
            required: true
          - name: direction
            type: string
            default: "UP"
            valid_values: [UP, DOWN, BOTH]
          - name: max-depth
            type: integer
            default: 5
          - name: create-missing-entities
            type: boolean
            default: true
          - name: decision-id
            type: uuid

      validate-lei:
        description: "Validate LEI status"
        invocation_phrases:
          - "check if LEI is valid"
          - "validate this LEI"
          - "is the LEI current"
        behavior: plugin
        handler: GleifValidateLeiOp
        args:
          - name: lei
            type: string
            required: true
          - name: entity-id
            type: uuid

      refresh-entity:
        description: "Refresh entity data from GLEIF"
        invocation_phrases:
          - "refresh from GLEIF"
          - "update GLEIF data"
          - "sync with GLEIF"
        behavior: plugin
        handler: GleifRefreshEntityOp
        args:
          - name: entity-id
            type: uuid
            required: true
```

### research.companies-house.yaml

```yaml
domains:
  research.companies-house:
    description: "UK Companies House registry"
    
    invocation_hints:
      - "Companies House"
      - "UK company"
      - "British company"
      - "company number"
      - "UK directors"
      - "PSC"
      - "persons with significant control"
    
    verbs:
      import-company:
        description: "Import company by company number"
        invocation_phrases:
          - "import from Companies House"
          - "get UK company"
          - "fetch from CH"
          - "load company"
        behavior: plugin
        handler: CompaniesHouseImportCompanyOp
        requires_key: true
        key_type: COMPANY_NUMBER
        key_validation: "^[A-Z0-9]{8}$"
        args:
          - name: company-number
            type: string
            required: true
          - name: target-entity-id
            type: uuid
          - name: decision-id
            type: uuid

      import-officers:
        description: "Import officers/directors"
        invocation_phrases:
          - "get the directors"
          - "import officers"
          - "who are the directors"
          - "fetch board composition"
        behavior: plugin
        handler: CompaniesHouseImportOfficersOp
        requires_key: true
        key_type: COMPANY_NUMBER
        args:
          - name: company-number
            type: string
            required: true
          - name: entity-id
            type: uuid
            required: true
          - name: include-resigned
            type: boolean
            default: false
          - name: decision-id
            type: uuid

      import-psc:
        description: "Import Persons with Significant Control (UBO)"
        invocation_phrases:
          - "get the PSCs"
          - "import UBOs from Companies House"
          - "who controls this UK company"
          - "significant control"
          - "fetch PSC records"
        behavior: plugin
        handler: CompaniesHouseImportPscOp
        requires_key: true
        key_type: COMPANY_NUMBER
        args:
          - name: company-number
            type: string
            required: true
          - name: entity-id
            type: uuid
            required: true
          - name: decision-id
            type: uuid
```

### research.sec.yaml

```yaml
domains:
  research.sec:
    description: "US SEC EDGAR filings"
    
    invocation_hints:
      - "SEC"
      - "EDGAR"
      - "US company"
      - "American company"
      - "CIK"
      - "13F"
      - "13D"
      - "13G"
      - "beneficial owner"
      - "institutional holder"
    
    verbs:
      import-company:
        description: "Import company by CIK"
        invocation_phrases:
          - "import from SEC"
          - "get SEC filing"
          - "fetch from EDGAR"
        behavior: plugin
        handler: SecImportCompanyOp
        requires_key: true
        key_type: CIK
        args:
          - name: cik
            type: string
            required: true
          - name: target-entity-id
            type: uuid
          - name: decision-id
            type: uuid

      import-13f-holders:
        description: "Import institutional holders from 13F"
        invocation_phrases:
          - "get 13F holders"
          - "import institutional holders"
          - "who are the institutional investors"
          - "fetch 13F filings"
        behavior: plugin
        handler: SecImport13FOp
        requires_key: true
        key_type: CIK
        args:
          - name: cik
            type: string
            required: true
          - name: entity-id
            type: uuid
            required: true
          - name: as-of-quarter
            type: string
          - name: threshold-pct
            type: decimal
            default: 0
          - name: decision-id
            type: uuid

      import-13dg-owners:
        description: "Import beneficial owners from 13D/13G"
        invocation_phrases:
          - "get 13D owners"
          - "get 13G owners"
          - "who are the beneficial owners"
          - "activist investors"
        behavior: plugin
        handler: SecImport13DGOp
        requires_key: true
        key_type: CIK
        args:
          - name: cik
            type: string
            required: true
          - name: entity-id
            type: uuid
            required: true
          - name: decision-id
            type: uuid
```

### research.screening.yaml

```yaml
domains:
  research.screening:
    description: "Sanctions, PEP, and adverse media screening"
    
    invocation_hints:
      - "sanctions"
      - "PEP"
      - "politically exposed"
      - "adverse media"
      - "screening"
      - "compliance check"
      - "OFAC"
      - "EU sanctions"
      - "watchlist"
    
    verbs:
      record-sanctions-check:
        description: "Record sanctions screening result"
        invocation_phrases:
          - "record sanctions result"
          - "log sanctions check"
          - "save screening outcome"
        behavior: plugin
        handler: ScreeningRecordSanctionsOp
        args:
          - name: entity-id
            type: uuid
            required: true
          - name: provider
            type: string
            required: true
          - name: lists-checked
            type: array
            required: true
          - name: result
            type: string
            required: true
            valid_values: [CLEAR, POTENTIAL_MATCH, CONFIRMED_MATCH]
          - name: matches
            type: array
          - name: decision-id
            type: uuid

      record-pep-check:
        description: "Record PEP screening result"
        invocation_phrases:
          - "record PEP result"
          - "log PEP check"
        behavior: plugin
        handler: ScreeningRecordPepOp
        args:
          - name: person-entity-id
            type: uuid
            required: true
          - name: provider
            type: string
            required: true
          - name: result
            type: string
            required: true
            valid_values: [NOT_PEP, PEP, RCA, FORMER_PEP]
          - name: pep-details
            type: object
          - name: decision-id
            type: uuid

      record-adverse-media:
        description: "Record adverse media result"
        invocation_phrases:
          - "record adverse media"
          - "log media screening"
        behavior: plugin
        handler: ScreeningRecordAdverseMediaOp
        args:
          - name: entity-id
            type: uuid
            required: true
          - name: provider
            type: string
            required: true
          - name: result
            type: string
            required: true
          - name: mentions
            type: array
          - name: decision-id
            type: uuid
```

### research.generic.yaml

```yaml
domains:
  research.generic:
    description: "Generic import for discovered/pluggable sources"
    
    invocation_hints:
      - "import from"
      - "I found"
      - "registry shows"
      - "according to"
      - "data from"
    
    verbs:
      import-entity:
        description: "Import entity from any source using normalized structure"
        invocation_phrases:
          - "import this entity"
          - "load this data"
          - "save what I found"
          - "create entity from"
        behavior: plugin
        handler: GenericImportEntityOp
        args:
          - name: source-name
            type: string
            required: true
            description: "Human-readable source name"
          - name: source-url
            type: string
            description: "URL of source (for audit)"
          - name: source-key
            type: string
            required: true
            description: "Identifier from source"
          - name: source-key-type
            type: string
            required: true
            description: "Type of identifier"
          - name: extracted-data
            type: object
            required: true
            description: "Normalized entity data"
          - name: raw-response
            type: string
            description: "Original response for audit"
          - name: decision-id
            type: uuid

      import-hierarchy:
        description: "Import hierarchy from any source"
        invocation_phrases:
          - "import this hierarchy"
          - "load ownership structure"
          - "create these relationships"
        behavior: plugin
        handler: GenericImportHierarchyOp
        args:
          - name: source-name
            type: string
            required: true
          - name: entities
            type: array
            required: true
            description: "Array of normalized entity objects"
          - name: relationships
            type: array
            required: true
            description: "Array of relationship objects"
          - name: decision-id
            type: uuid

      import-officers:
        description: "Import officers from any source"
        invocation_phrases:
          - "import these directors"
          - "add these officers"
          - "load board composition"
        behavior: plugin
        handler: GenericImportOfficersOp
        args:
          - name: entity-id
            type: uuid
            required: true
          - name: source-name
            type: string
            required: true
          - name: officers
            type: array
            required: true
          - name: decision-id
            type: uuid
```

### research.workflow.yaml

```yaml
domains:
  research.workflow:
    description: "Research workflow and decision management"
    
    invocation_hints:
      - "research trigger"
      - "gap"
      - "decision"
      - "correction"
      - "audit trail"
    
    verbs:
      list-triggers:
        description: "List research triggers"
        invocation_phrases:
          - "show research triggers"
          - "what gaps need work"
          - "pending research"
          - "what needs to be done"
        behavior: crud
        crud:
          operation: select
          table: ownership_research_triggers
          schema: kyc

      create-trigger:
        description: "Create research trigger"
        invocation_phrases:
          - "create a trigger"
          - "flag for research"
          - "mark as needing work"
        behavior: crud
        crud:
          operation: insert
          table: ownership_research_triggers
          schema: kyc
          returning: trigger_id

      record-decision:
        description: "Record a research decision"
        invocation_phrases:
          - "record the decision"
          - "log the selection"
          - "save decision"
        behavior: crud
        crud:
          operation: insert
          table: research_decisions
          schema: kyc
          returning: decision_id
        args:
          - name: trigger-id
            type: uuid
          - name: target-entity-id
            type: uuid
          - name: search-query
            type: string
            required: true
          - name: source-provider
            type: string
            required: true
          - name: candidates-found
            type: array
            required: true
          - name: selected-key
            type: string
          - name: selected-key-type
            type: string
          - name: confidence
            type: decimal
          - name: reasoning
            type: string
            required: true
          - name: decision-type
            type: string
            required: true

      confirm-decision:
        description: "User confirms ambiguous decision"
        invocation_phrases:
          - "confirm this"
          - "yes that's right"
          - "use that one"
        behavior: plugin
        handler: WorkflowConfirmDecisionOp

      reject-decision:
        description: "User rejects suggestion"
        invocation_phrases:
          - "no that's wrong"
          - "reject"
          - "not that one"
        behavior: plugin
        handler: WorkflowRejectDecisionOp

      record-correction:
        description: "Record correction to previous decision"
        invocation_phrases:
          - "correct this"
          - "fix the mistake"
          - "wrong entity was selected"
          - "undo that selection"
        behavior: crud
        crud:
          operation: insert
          table: research_corrections
          schema: kyc
          returning: correction_id

      audit-trail:
        description: "Get research audit trail"
        invocation_phrases:
          - "show audit trail"
          - "what research was done"
          - "history of decisions"
          - "how did we get this data"
        behavior: plugin
        handler: WorkflowAuditTrailOp
        args:
          - name: entity-id
            type: uuid
            required: true
```

---

## Pluggable Source Model

### Source Tiers

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    THREE-TIER SOURCE MODEL                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   TIER 1: BUILT-IN SOURCES (optimized, pre-built handlers)                  │
│   ═════════════════════════════════════════════════════════                  │
│                                                                              │
│   • GLEIF                    research.gleif.import-*                        │
│   • Companies House          research.companies-house.import-*              │
│   • SEC EDGAR                research.sec.import-*                          │
│                                                                              │
│   Characteristics:                                                          │
│   • Dedicated prompt templates with API details                             │
│   • Dedicated handlers with schema mapping                                  │
│   • Specific verbs                                                          │
│   • Optimized parsing                                                       │
│                                                                              │
│   ───────────────────────────────────────────────────────────────────────   │
│                                                                              │
│   TIER 2: REGISTERED SOURCES (semi-known, pluggable)                        │
│   ═══════════════════════════════════════════════════                        │
│                                                                              │
│   • Singapore ACRA           registered, base URL known                     │
│   • Hong Kong CR             registered, API documented                     │
│   • German Handelsregister   registered, no API (web scraping)              │
│                                                                              │
│   Characteristics:                                                          │
│   • Source registered in discovered_sources table                           │
│   • LLM has hints (base URL, notes from previous use)                       │
│   • Uses research.generic.import-* verbs                                    │
│   • LLM adapts to API/format                                                │
│                                                                              │
│   ───────────────────────────────────────────────────────────────────────   │
│                                                                              │
│   TIER 3: DISCOVERED SOURCES (ad-hoc, LLM figures it out)                   │
│   ════════════════════════════════════════════════════════                   │
│                                                                              │
│   • User says "check the Cayman registry"                                   │
│   • LLM web searches for API/access                                         │
│   • LLM reads docs, makes calls, parses response                            │
│   • LLM extracts to normalized structure                                    │
│   • Uses research.generic.import-* verbs                                    │
│   • Optionally registers as Tier 2 for future use                           │
│                                                                              │
│   The LLM is the universal API adapter                                      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Normalized Data Structure

```yaml
# Contract between LLM exploration and deterministic import

extracted_entity:
  required:
    name: string
    source_key: string
    source_name: string
  
  optional:
    jurisdiction: string        # ISO country code
    entity_type: string         # Mapped to our taxonomy
    status: string              # ACTIVE, DISSOLVED, etc.
    incorporated_date: date
    dissolved_date: date
    registered_address: object
    lei: string
    tax_id: string
    registration_number: string
    
  nested:
    officers:
      - name: string
        role: string            # DIRECTOR, SECRETARY, CEO
        appointed_date: date
        resigned_date: date
        nationality: string
        
    shareholders:
      - name: string
        entity_type: string     # PERSON, COMPANY
        shares: number
        share_class: string
        percentage: decimal
        source_key: string      # If identifiable
        
    parents:
      - name: string
        relationship_type: string
        ownership_pct: decimal
        source_key: string
```

### Source Discovery Prompt

```markdown
# /prompts/research/sources/discover-source.md

## Context
You need to find corporate/ownership data but we don't have a pre-built 
integration for the relevant registry. You must find and use the source.

## Input
- entity_name: {{entity_name}}
- jurisdiction: {{jurisdiction}}
- data_needed: {{data_needed}}

## Your Task

1. IDENTIFY data sources for {{jurisdiction}}:
   - Official company registry
   - Securities regulator
   - Tax authority records
   - Commercial databases

2. FIND API or access method:
   - Search for "[jurisdiction] company registry API"
   - Check if public API exists
   - Check authentication requirements
   - Note rate limits

3. SEARCH for the entity and extract data

4. NORMALIZE to our standard structure:
```json
{
  "source": {
    "name": "...",
    "url": "...",
    "accessed_at": "..."
  },
  "entity": { ... },
  "officers": [ ... ],
  "shareholders": [ ... ]
}
```

5. EMIT the import verb:
   research.generic.import-entity(
     :source-name "..."
     :source-key "..."
     :extracted-data { ... }
   )

## Rules
- NEVER fabricate data
- Include raw response snippet for audit
- Flag uncertain field mappings
- If API unavailable, note it and suggest alternatives
```

### Discovered Sources Table

```sql
CREATE TABLE IF NOT EXISTS kyc.discovered_sources (
    source_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Identity
    source_name VARCHAR(100) NOT NULL UNIQUE,
    source_type VARCHAR(30) NOT NULL,
    
    -- Coverage
    jurisdictions TEXT[] NOT NULL,
    data_provides TEXT[],
    
    -- Access
    base_url VARCHAR(500),
    api_documentation_url VARCHAR(500),
    requires_auth BOOLEAN DEFAULT false,
    auth_type VARCHAR(30),
    
    -- LLM learned details
    api_notes TEXT,
    example_request TEXT,
    example_response TEXT,
    parsing_notes TEXT,
    
    -- Quality tracking
    times_used INTEGER DEFAULT 0,
    success_count INTEGER DEFAULT 0,
    last_used_at TIMESTAMPTZ,
    last_success_at TIMESTAMPTZ,
    reliability_score DECIMAL(3,2) GENERATED ALWAYS AS (
        CASE WHEN times_used > 0 
        THEN success_count::DECIMAL / times_used 
        ELSE 0 END
    ) STORED,
    
    -- Status
    is_active BOOLEAN DEFAULT true,
    
    -- Audit
    discovered_at TIMESTAMPTZ DEFAULT NOW(),
    discovered_in_session UUID,
    
    CONSTRAINT chk_source_type CHECK (
        source_type IN ('REGISTRY', 'REGULATOR', 'COMMERCIAL', 'AGGREGATOR', 'OTHER')
    )
);
```

---

## Core Architecture: Bounded Non-Determinism

### The Two-Phase Pattern

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PHASE 1 vs PHASE 2                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   PHASE 1: LLM EXPLORATION                                                  │
│   ════════════════════════                                                   │
│   • Fuzzy name matching                                                     │
│   • Context reasoning                                                       │
│   • Source selection                                                        │
│   • API discovery                                                           │
│   • Disambiguation                                                          │
│   • Confidence scoring                                                      │
│                                                                              │
│   Executed via: PROMPT TEMPLATES                                            │
│   Output: IDENTIFIER (LEI, company number, CIK) + normalized data           │
│                                                                              │
│   Non-deterministic but AUDITABLE                                           │
│                                                                              │
│   ───────────────────────────────────────────────────────────────────────   │
│                                                                              │
│   PHASE 2: DSL EXECUTION                                                    │
│   ══════════════════════                                                     │
│   • Validate normalized structure                                           │
│   • Map to entity schema                                                    │
│   • Create/update entities                                                  │
│   • Create relationships                                                    │
│   • Record in audit trail                                                   │
│                                                                              │
│   Executed via: DSL VERBS                                                   │
│   Input: IDENTIFIER + normalized data (from Phase 1)                        │
│                                                                              │
│   Deterministic, reproducible, idempotent                                   │
│                                                                              │
│   ═══════════════════════════════════════════════════════════════════════   │
│                                                                              │
│   THE IDENTIFIER IS THE BRIDGE                                              │
│                                                                              │
│   Phase 1 finds the key → Phase 2 uses the key                              │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Confidence Thresholds

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    CONFIDENCE-BASED ROUTING                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Score >= 0.90 (auto_proceed_threshold)                                    │
│   ═══════════════════════════════════════                                    │
│   → AUTO_SELECTED                                                           │
│   → Proceed to Phase 2 immediately                                          │
│   → Log decision with reasoning                                             │
│                                                                              │
│   Score 0.70-0.90 (ambiguous range)                                         │
│   ═════════════════════════════════                                          │
│   → CHECKPOINT                                                              │
│   → Present candidates to user                                              │
│   → Wait for selection                                                      │
│   → Then proceed to Phase 2                                                 │
│                                                                              │
│   Score < 0.70 (reject_threshold)                                           │
│   ═══════════════════════════════                                            │
│   → NO_MATCH                                                                │
│   → Try next source in priority list                                        │
│   → Or flag for manual research                                             │
│                                                                              │
│   FORCED CHECKPOINTS (regardless of score):                                 │
│   ═════════════════════════════════════════                                  │
│   • Screening hits (sanctions, PEP)                                         │
│   • High-stakes context (NEW_CLIENT, MATERIAL_HOLDING)                      │
│   • Correction to previous decision                                         │
│   • Multiple equally-scored candidates                                      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Decision Audit Schema

```sql
-- =============================================================================
-- RESEARCH DECISIONS (Phase 1 audit)
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.research_decisions (
    decision_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Context
    trigger_id UUID REFERENCES kyc.ownership_research_triggers(trigger_id),
    target_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    session_id UUID,
    
    -- Search
    search_query TEXT NOT NULL,
    search_context JSONB,
    
    -- Source
    source_provider VARCHAR(30) NOT NULL,
    source_tier VARCHAR(10),  -- BUILT_IN, REGISTERED, DISCOVERED
    
    -- Candidates
    candidates_found JSONB NOT NULL,
    candidates_count INTEGER NOT NULL,
    
    -- Selection
    selected_key VARCHAR(100),
    selected_key_type VARCHAR(20),
    selection_confidence DECIMAL(3,2),
    selection_reasoning TEXT NOT NULL,
    
    -- Decision
    decision_type VARCHAR(20) NOT NULL,
    
    -- Verification
    auto_selected BOOLEAN NOT NULL DEFAULT false,
    verified_by UUID,
    verified_at TIMESTAMPTZ,
    
    -- Link to action
    resulting_action_id UUID,
    
    -- Audit
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT chk_decision_type CHECK (decision_type IN (
        'AUTO_SELECTED', 'USER_SELECTED', 'USER_CONFIRMED',
        'NO_MATCH', 'AMBIGUOUS', 'REJECTED'
    ))
);

-- =============================================================================
-- RESEARCH ACTIONS (Phase 2 audit)
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.research_actions (
    action_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Context
    target_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    decision_id UUID REFERENCES kyc.research_decisions(decision_id),
    session_id UUID,
    
    -- Action
    action_type VARCHAR(50) NOT NULL,
    source_provider VARCHAR(30) NOT NULL,
    source_key VARCHAR(100) NOT NULL,
    source_key_type VARCHAR(20) NOT NULL,
    
    -- DSL
    verb_domain VARCHAR(30) NOT NULL,
    verb_name VARCHAR(50) NOT NULL,
    verb_args JSONB NOT NULL,
    
    -- Outcome
    success BOOLEAN NOT NULL,
    entities_created INTEGER DEFAULT 0,
    entities_updated INTEGER DEFAULT 0,
    relationships_created INTEGER DEFAULT 0,
    
    -- Errors
    error_code VARCHAR(50),
    error_message TEXT,
    
    -- Performance
    duration_ms INTEGER,
    
    -- Audit
    executed_at TIMESTAMPTZ DEFAULT NOW(),
    executed_by UUID
);

-- =============================================================================
-- RESEARCH CORRECTIONS
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.research_corrections (
    correction_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    original_decision_id UUID NOT NULL REFERENCES kyc.research_decisions(decision_id),
    original_action_id UUID REFERENCES kyc.research_actions(action_id),
    
    correction_type VARCHAR(20) NOT NULL,
    wrong_key VARCHAR(100),
    correct_key VARCHAR(100),
    new_action_id UUID REFERENCES kyc.research_actions(action_id),
    
    correction_reason TEXT NOT NULL,
    
    corrected_at TIMESTAMPTZ DEFAULT NOW(),
    corrected_by UUID NOT NULL,
    
    CONSTRAINT chk_correction_type CHECK (correction_type IN (
        'WRONG_ENTITY', 'WRONG_JURISDICTION', 'STALE_DATA', 'MERGE_REQUIRED', 'UNLINK'
    ))
);
```

---

## Prompt Templates

### Directory Structure

```
/prompts/research/
├── sources/
│   ├── gleif/
│   │   ├── search.md
│   │   └── disambiguate.md
│   ├── companies-house/
│   │   ├── search.md
│   │   └── search-officer.md
│   ├── sec-edgar/
│   │   ├── search.md
│   │   └── parse-13f.md
│   └── discover-source.md          # For Tier 3 discovery
│
├── screening/
│   ├── interpret-sanctions.md
│   └── interpret-pep.md
│
├── documents/
│   ├── extract-ownership.md
│   └── extract-directors.md
│
└── orchestration/
    ├── resolve-gap.md              # Strategy for single gap
    ├── chain-research.md           # Full chain strategy
    └── select-source.md            # Pick best source
```

### Example: GLEIF Search Prompt

```markdown
# /prompts/research/sources/gleif/search.md

## Context
Search GLEIF for entity: {{entity_name}}
Jurisdiction hint: {{jurisdiction}}
Context: {{context}}

## GLEIF API
Fuzzy search: https://api.gleif.org/api/v1/fuzzycompletions?field=fulltext&q={query}
Exact search: https://api.gleif.org/api/v1/lei-records?filter[entity.legalName]={name}

## Strategy
1. Try exact name
2. Remove legal suffixes (Ltd, GmbH, LLC)
3. Filter by jurisdiction
4. Only ISSUED status (not LAPSED)

## Scoring
- Exact name match: +0.3
- Jurisdiction match: +0.2  
- Active status: +0.2
- Recent registration: +0.1
- Has parent data: +0.1

## Output
If score >= 0.90:
  {"status": "found", "lei": "...", "confidence": 0.95, "reasoning": "..."}

If 0.70-0.90:
  {"status": "ambiguous", "candidates": [...]}

If < 0.70:
  {"status": "not_found", "suggestion": "Try Companies House"}

## Then Emit
research.workflow.record-decision(:search-query "..." :source-provider "gleif" ...)

If found:
  research.gleif.import-hierarchy(:lei "..." :decision-id @decision_id)
```

---

## Implementation Phases

### Phase 1: Agent Infrastructure (15h)
- [ ] 1.1 Review CLAUDE.md and session/REPL docs
- [ ] 1.2 Extend Session with mode and agent_state
- [ ] 1.3 Implement AgentController struct
- [ ] 1.4 Implement agent loop skeleton
- [ ] 1.5 Wire agent events to viewport
- [ ] 1.6 Implement checkpoint UI pattern

### Phase 2: Agent Verbs (10h)
- [ ] 2.1 Implement agent.start verb
- [ ] 2.2 Implement agent.pause/resume/stop
- [ ] 2.3 Implement agent.respond-checkpoint
- [ ] 2.4 Implement agent.resolve-gaps
- [ ] 2.5 Implement agent.chain-research
- [ ] 2.6 Add invocation phrases to verb YAML

### Phase 3: Audit Schema (8h)
- [ ] 3.1 Create research_decisions table
- [ ] 3.2 Create research_actions table
- [ ] 3.3 Create research_corrections table
- [ ] 3.4 Create discovered_sources table
- [ ] 3.5 Create confidence_config table

### Phase 4: Prompt Templates (10h)
- [ ] 4.1 Create prompt directory structure
- [ ] 4.2 Write GLEIF search/disambiguate prompts
- [ ] 4.3 Write Companies House prompts
- [ ] 4.4 Write orchestration prompts
- [ ] 4.5 Write discover-source prompt
- [ ] 4.6 Implement PromptLoader

### Phase 5: GLEIF Refactor (8h)
- [ ] 5.1 Move existing GLEIF under research module
- [ ] 5.2 Add decision_id parameter
- [ ] 5.3 Wire audit trail logging
- [ ] 5.4 Add invocation phrases
- [ ] 5.5 Test end-to-end

### Phase 6: Companies House (10h)
- [ ] 6.1 Implement CH API client
- [ ] 6.2 Implement import-company verb
- [ ] 6.3 Implement import-officers verb
- [ ] 6.4 Implement import-psc verb
- [ ] 6.5 Add prompt templates
- [ ] 6.6 Test with real data

### Phase 7: Generic Import Path (8h)
- [ ] 7.1 Define normalized structure schema
- [ ] 7.2 Implement research.generic.import-entity
- [ ] 7.3 Implement research.generic.import-hierarchy
- [ ] 7.4 Implement source registration logic
- [ ] 7.5 Test with discovered source

### Phase 8: Screening (8h)
- [ ] 8.1 Implement record-sanctions-check
- [ ] 8.2 Implement record-pep-check
- [ ] 8.3 Implement interpret prompts
- [ ] 8.4 Wire forced checkpoint for hits

### Phase 9: Workflow Verbs (6h)
- [ ] 9.1 Implement record-decision
- [ ] 9.2 Implement confirm/reject-decision
- [ ] 9.3 Implement record-correction
- [ ] 9.4 Implement audit-trail query

### Phase 10: Integration Testing (10h)
- [ ] 10.1 Test full agent loop
- [ ] 10.2 Test checkpoint flow
- [ ] 10.3 Test correction workflow
- [ ] 10.4 Test discovered source flow
- [ ] 10.5 Test scope inheritance
- [ ] 10.6 Update CLAUDE.md

---

## Estimated Effort

| Phase | Effort |
|-------|--------|
| 1. Agent Infrastructure | 15h |
| 2. Agent Verbs | 10h |
| 3. Audit Schema | 8h |
| 4. Prompt Templates | 10h |
| 5. GLEIF Refactor | 8h |
| 6. Companies House | 10h |
| 7. Generic Import | 8h |
| 8. Screening | 8h |
| 9. Workflow Verbs | 6h |
| 10. Testing | 10h |
| **Total** | **~93h** |

---

## Success Criteria

1. **Agent wired to session** - Mode, state visible in session
2. **Agent wired to REPL** - DSL emitted and executed
3. **Agent wired to viewport** - Progress, checkpoints displayed
4. **Invocation phrases work** - LLM triggers correct verbs
5. **Decisions audited** - All Phase 1 selections logged
6. **Actions audited** - All Phase 2 imports logged
7. **Pluggable sources** - Discovered source can be used and registered
8. **Checkpoints enforced** - User confirms ambiguous/high-stakes
9. **Corrections tracked** - Mistakes can be fixed with audit trail

---

Generated: 2026-01-10
