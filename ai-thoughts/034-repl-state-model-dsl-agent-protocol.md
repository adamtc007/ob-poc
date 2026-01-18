# REPL State Model & DSL/Agent Protocol

## Overview

This document maps the current session state model to the required REPL state model for incremental DSL assembly with proper callout/response protocol between UI, Agent, and DSL compiler stages.

---

## Part 1: Current State Model

### SessionState Enum (session.rs:383-397)

```rust
pub enum SessionState {
    New,              // Just created, no intents yet
    PendingValidation, // Has pending intents awaiting validation  
    ReadyToExecute,   // Intents validated, DSL assembled, ready
    Executing,        // Execution in progress
    Executed,         // Execution complete
    Closed,           // Session ended
}
```

### DslStatus Enum (session.rs:403-415)

```rust
pub enum DslStatus {
    Draft,      // Parsed, awaiting user confirmation
    Ready,      // User confirmed, ready to execute
    Executed,   // Successfully executed
    Cancelled,  // User declined
    Failed,     // Execution failed
}
```

### Problem: These Don't Map to Compiler Stages

Current states conflate:
- Intent capture vs DSL assembly
- Syntax errors vs semantic errors vs unresolved refs
- Disambiguation (intent) vs Resolution (entity)

---

## Part 2: Proposed REPL State Model

### New SessionState Enum

```rust
pub enum SessionState {
    // === ENTRY ===
    New,                    // Fresh session, no user input yet
    
    // === INTENT PHASE ===
    IntentCapture,          // Processing user message
    IntentClarify {         // Ambiguous intent, need user clarification
        options: Vec<IntentOption>,
        original_message: String,
    },
    
    // === DSL ASSEMBLY PHASE ===
    DslAssembly,            // Building DSL from confirmed intent
    
    // === COMPILER STAGE 1: SYNTAX ===
    SyntaxValidation,       // Parsing DSL to AST
    SyntaxError {           // Parse failed
        errors: Vec<SyntaxError>,
        partial_ast: Option<Vec<Statement>>,
    },
    
    // === COMPILER STAGE 2: SEMANTICS ===
    SemanticValidation,     // Linting AST, checking refs
    SemanticError {         // Validation failed (not unresolved refs)
        errors: Vec<SemanticError>,
    },
    UnresolvedRefs {        // Has unresolved entity references
        refs: Vec<UnresolvedRef>,
        current_index: usize,
    },
    
    // === COMPILER STAGE 3: DEPENDENCY ===
    DependencyResolution,   // Building execution order
    DependencyError {       // Circular deps, missing symbols
        errors: Vec<DependencyError>,
    },
    
    // === READY ===
    ReadyToRun,             // All stages passed, can execute
    
    // === EXECUTION ===
    Executing,              // Running against database
    Executed {              // Complete
        success: bool,
        results: Vec<ExecutionResult>,
    },
    
    // === TERMINAL ===
    Cancelled,              // User cancelled session
    Closed,                 // Session ended normally
}
```

### UnresolvedRef Structure

```rust
/// An entity reference that needs user resolution
pub struct UnresolvedRef {
    /// Unique ID for this ref in this session
    pub ref_id: String,
    
    /// Where in DSL: statement index, arg name
    pub location: RefLocation,
    
    /// The raw search value from DSL (e.g., "allianz")
    pub search_value: String,
    
    /// Entity type from verb arg definition
    pub entity_type: String,
    
    /// Resolution config from EntityGateway
    pub config: EntityResolutionConfig,
    
    /// Pre-fetched initial matches (if any)
    pub initial_matches: Vec<EntityMatch>,
    
    /// Current resolution status
    pub status: RefResolutionStatus,
}

pub struct RefLocation {
    pub statement_index: usize,
    pub verb: String,
    pub arg_name: String,
    pub span: Span,
}

pub struct EntityResolutionConfig {
    pub search_keys: Vec<SearchKeyField>,
    pub discriminators: Vec<DiscriminatorField>,
    pub resolution_mode: ResolutionModeHint,
    pub return_key_type: String, // "uuid" or "code"
}

pub enum RefResolutionStatus {
    Pending,                    // Not yet resolved
    Resolved { entity_id: String, display: String },
    Skipped,                    // User skipped
    CreatedNew { entity_id: String },
}
```

---

## Part 3: Callout/Response Protocol

### State Transition Diagram with Callouts

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                     CALLOUT / RESPONSE PROTOCOL                              │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  USER                    AGENT                    DSL COMPILER               │
│  ────                    ─────                    ────────────               │
│                                                                              │
│  ┌─────────────┐                                                             │
│  │ "show       │ ─────► ┌─────────────────┐                                 │
│  │  allianz    │        │ CALLOUT:        │                                 │
│  │  cbu"       │        │ IntentExtract   │                                 │
│  └─────────────┘        │                 │                                 │
│                         │ LLM extracts:   │                                 │
│                         │ verb: session.  │                                 │
│                         │   set-cbu       │                                 │
│                         │ params: {       │                                 │
│                         │   cbu-id:       │                                 │
│                         │   "allianz"     │                                 │
│                         │ }               │                                 │
│                         └────────┬────────┘                                 │
│                                  │                                          │
│                                  ▼                                          │
│                         ┌─────────────────┐                                 │
│                         │ RESPONSE:       │                                 │
│                         │ IntentConfirmed │ ─────► ┌─────────────────┐     │
│                         │                 │        │ CALLOUT:        │     │
│                         │ Build DSL from  │        │ Parse           │     │
│                         │ intent          │        │                 │     │
│                         └─────────────────┘        │ Input:          │     │
│                                                    │ (session.set-cbu│     │
│  DSL Editor shows:                                 │  :cbu-id        │     │
│  ┌─────────────────┐                               │  "allianz")     │     │
│  │(session.set-cbu │ ◄───────────────────────────  │                 │     │
│  │ :cbu-id         │                               └────────┬────────┘     │
│  │ "allianz")      │                                        │              │
│  └─────────────────┘                                        ▼              │
│                                                    ┌─────────────────┐     │
│                                                    │ RESPONSE:       │     │
│                                                    │ AST Valid       │     │
│                                                    │                 │     │
│                                                    │ Proceed to      │     │
│                                                    │ semantic check  │     │
│                                                    └────────┬────────┘     │
│                                                             │              │
│                                                             ▼              │
│                                                    ┌─────────────────┐     │
│                                                    │ CALLOUT:        │     │
│                                                    │ SemanticLint    │     │
│                                                    │                 │     │
│                                                    │ Walk AST,       │     │
│                                                    │ check refs      │     │
│                                                    └────────┬────────┘     │
│                                                             │              │
│                                                             ▼              │
│                                                    ┌─────────────────┐     │
│                                                    │ RESPONSE:       │     │
│                                                    │ UnresolvedRef   │     │
│                         ┌─────────────────┐ ◄───── │                 │     │
│                         │ CALLOUT:        │        │ ref_id: "ref1"  │     │
│                         │ GetEntityConfig │        │ entity_type:    │     │
│                         │                 │        │   "cbu"         │     │
│                         │ entity_type:    │        │ search_value:   │     │
│                         │   "cbu"         │        │   "allianz"     │     │
│                         └────────┬────────┘        │ verb_arg:       │     │
│                                  │                 │   session.      │     │
│                                  ▼                 │   set-cbu.      │     │
│                         ┌─────────────────┐        │   cbu-id        │     │
│                         │ RESPONSE:       │        └─────────────────┘     │
│                         │ EntityConfig    │                                 │
│                         │                 │                                 │
│                         │ search_keys:    │                                 │
│                         │  - name         │                                 │
│                         │  - jurisdiction │                                 │
│                         │  - client_type  │                                 │
│                         │ discriminators: │                                 │
│                         │  - manco_name   │                                 │
│                         │ mode: SearchModal│                                │
│                         └────────┬────────┘                                 │
│                                  │                                          │
│  ┌─────────────────┐             │                                          │
│  │ Resolution      │ ◄───────────┘                                          │
│  │ Modal Opens     │                                                        │
│  │                 │                                                        │
│  │ CBU-specific    │                                                        │
│  │ fields:         │                                                        │
│  │ [Name    ]      │                                                        │
│  │ [Jurisd ▼]      │                                                        │
│  │ [Type   ▼]      │                                                        │
│  │                 │                                                        │
│  │ Matches:        │                                                        │
│  │ 1. ALLIANZ FUNDS│                                                        │
│  │ 2. Allianz      │                                                        │
│  │    Thematica    │                                                        │
│  └────────┬────────┘                                                        │
│           │                                                                 │
│           │ User selects "ALLIANZ FUNDS"                                    │
│           ▼                                                                 │
│  ┌─────────────────┐                                                        │
│  │ CALLOUT:        │ ─────► ┌─────────────────┐                            │
│  │ ResolveRef      │        │ RESPONSE:       │                            │
│  │                 │        │ RefResolved     │                            │
│  │ ref_id: "ref1"  │        │                 │                            │
│  │ entity_id:      │        │ Update AST:     │                            │
│  │   "19eba7a4..." │        │ EntityRef.      │ ─────► ┌─────────────────┐ │
│  │ display:        │        │ resolved_key =  │        │ CALLOUT:        │ │
│  │   "ALLIANZ      │        │ "19eba7a4..."   │        │ RevalidateAST   │ │
│  │    FUNDS"       │        │                 │        │                 │ │
│  └─────────────────┘        └─────────────────┘        │ Check if more   │ │
│                                                        │ unresolved refs │ │
│  DSL Editor updates:                                   └────────┬────────┘ │
│  ┌─────────────────┐                                            │          │
│  │(session.set-cbu │                                            ▼          │
│  │ :cbu-id         │ ◄─────────────────────────────── ┌─────────────────┐ │
│  │ "19eba7a4...")  │                                  │ RESPONSE:       │ │
│  │ ; ALLIANZ FUNDS │                                  │ AllRefsResolved │ │
│  └─────────────────┘                                  │                 │ │
│                                                       │ Proceed to      │ │
│  Status: "Ready to run"                               │ dependency      │ │
│  [Run] [Edit] [Clear]                                 │ resolution      │ │
│                                                       └────────┬────────┘ │
│                                                                │          │
│                                                                ▼          │
│                                                       ┌─────────────────┐ │
│                                                       │ CALLOUT:        │ │
│                                                       │ BuildExecPlan   │ │
│                                                       │                 │ │
│                                                       │ Toposort by     │ │
│                                                       │ @symbol deps    │ │
│                                                       └────────┬────────┘ │
│                                                                │          │
│                                                                ▼          │
│                                                       ┌─────────────────┐ │
│                                                       │ RESPONSE:       │ │
│                         ┌─────────────────┐ ◄──────── │ ReadyToRun      │ │
│                         │ State:          │           │                 │ │
│                         │ ReadyToRun      │           │ Execution plan  │ │
│                         │                 │           │ built           │ │
│                         │ can_execute:    │           └─────────────────┘ │
│                         │   true          │                               │
│                         └─────────────────┘                               │
│                                                                           │
└───────────────────────────────────────────────────────────────────────────┘
```

---

## Part 4: Escape Hatches

### Valid Transitions from Any State

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           ESCAPE HATCHES                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  FROM ANY STATE:                                                            │
│  ───────────────                                                            │
│                                                                             │
│  "clear" / "start over" ─────────────────────────► New                     │
│    • Clears pending DSL                                                     │
│    • Clears pending intents                                                 │
│    • Clears resolution state                                                │
│    • Keeps conversation history (for context)                               │
│    • Keeps executed results (immutable)                                     │
│                                                                             │
│  "cancel" / "bail" ──────────────────────────────► Cancelled               │
│    • Session marked as cancelled                                            │
│    • No further actions allowed                                             │
│    • Can start new session                                                  │
│                                                                             │
│  "undo" ─────────────────────────────────────────► Previous DSL state      │
│    • Restores previous DSL source                                           │
│    • Re-runs validation from Stage 1                                        │
│    • Only available if history exists                                       │
│                                                                             │
│  Direct DSL edit (user types in editor) ─────────► SyntaxValidation        │
│    • Discards pending intents                                               │
│    • Re-parses from scratch                                                 │
│    • Agent gets "user edited DSL directly" signal                          │
│                                                                             │
│  "help" / "what can I do?" ──────────────────────► (no state change)       │
│    • Returns contextual help                                                │
│    • Based on current state                                                 │
│                                                                             │
│                                                                             │
│  FROM RESOLUTION STATE:                                                     │
│  ──────────────────────                                                     │
│                                                                             │
│  "skip" ─────────────────────────────────────────► Next unresolved ref     │
│    • Marks current ref as Skipped                                           │
│    • Advances to next ref                                                   │
│    • If last ref: proceeds to dependency stage                             │
│    • Skipped refs may cause execution errors later                         │
│                                                                             │
│  "create new" ───────────────────────────────────► Entity creation flow    │
│    • Opens entity creation sub-session                                      │
│    • On success: resolves ref with new entity ID                           │
│    • On cancel: returns to resolution                                       │
│                                                                             │
│  "try different search" ─────────────────────────► (stays in resolution)   │
│    • Clears current search fields                                           │
│    • User can try different search terms                                    │
│                                                                             │
│                                                                             │
│  FROM INTENT CLARIFY STATE:                                                 │
│  ─────────────────────────                                                  │
│                                                                             │
│  "none of these" / "something else" ─────────────► IntentCapture           │
│    • Discards intent options                                                │
│    • Agent re-prompts with broader question                                 │
│                                                                             │
│  User types new message ─────────────────────────► IntentCapture           │
│    • Treats as new intent request                                           │
│    • Previous clarification abandoned                                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Part 5: API Response Structure

### ChatResponse with State Feedback

```rust
/// Response from /api/session/:id/chat
pub struct ChatResponse {
    /// Agent's message to user
    pub message: String,
    
    /// Current session state (new enum)
    pub session_state: SessionState,
    
    /// DSL source (shown in editor)
    /// Present as soon as intent is confirmed, even with unresolved refs
    pub dsl_source: Option<String>,
    
    /// DSL status within the compilation pipeline
    pub dsl_status: Option<DslPipelineStatus>,
    
    /// If state is IntentClarify: options to present
    pub intent_options: Option<Vec<IntentOption>>,
    
    /// If state is UnresolvedRefs: refs needing resolution
    pub unresolved_refs: Option<Vec<UnresolvedRef>>,
    
    /// If state is *Error: errors to display
    pub errors: Option<Vec<CompilerError>>,
    
    /// Commands for UI (zoom, navigate, etc.)
    pub commands: Option<Vec<AgentCommand>>,
    
    /// Whether user can execute now
    pub can_execute: bool,
    
    /// Available escape actions from current state
    pub available_actions: Vec<EscapeAction>,
}

pub struct DslPipelineStatus {
    /// Which stage are we in?
    pub stage: CompilerStage,
    /// Progress within stage (for multi-ref resolution)
    pub progress: Option<StageProgress>,
    /// Warnings (non-blocking)
    pub warnings: Vec<CompilerWarning>,
}

pub enum CompilerStage {
    Parsing,
    SemanticValidation,
    RefResolution { current: usize, total: usize },
    DependencyResolution,
    Ready,
}

pub enum EscapeAction {
    Clear,
    Undo,
    Cancel,
    Skip,      // Only in resolution
    CreateNew, // Only in resolution
    Edit,      // Direct DSL edit
}
```

---

## Part 6: Implementation Changes Required

### 1. Modify resolve_all() Behavior

**Current**: Returns `NeedsDisambiguation` before DSL is built
**New**: Build DSL with placeholder refs, let semantic stage detect unresolved

```rust
// In resolve_all():
// Instead of:
if !suggestions.is_empty() {
    disambiguations.push(DisambiguationItem::EntityMatch { ... });
}

// Do:
if matches.len() == 1 {
    // Single match - resolve immediately
    inject_resolved_id(intent, param, matches[0].id);
} else {
    // Multiple or zero matches - leave as string literal
    // Semantic validator will flag as UnresolvedRef
    intent.params.insert(param, ParamValue::UnresolvedLiteral {
        search_value: search_text,
        entity_type: ref_type.to_string(),
    });
}
```

### 2. Add UnresolvedRef Detection in Semantic Validator

**File**: `rust/src/dsl_v2/semantic_validator.rs`

```rust
impl SemanticValidator {
    pub fn validate(&self, ast: &[Statement]) -> ValidationResult {
        let mut errors = vec![];
        let mut unresolved_refs = vec![];
        
        for (stmt_idx, stmt) in ast.iter().enumerate() {
            self.validate_statement(stmt, stmt_idx, &mut errors, &mut unresolved_refs);
        }
        
        ValidationResult {
            valid: errors.is_empty() && unresolved_refs.is_empty(),
            errors,
            unresolved_refs,  // NEW: separate from errors
        }
    }
    
    fn validate_statement(&self, stmt: &Statement, ...) {
        // For each arg, check if it's an unresolved literal
        // that should be an entity ref based on verb definition
        let verb_def = registry().get_verb(&stmt.verb);
        for arg in &stmt.args {
            if let Some(arg_def) = verb_def.get_arg(&arg.name) {
                if arg_def.has_lookup() {
                    if let AstNode::StringLiteral { value, .. } = &arg.value {
                        // This should be a resolved entity, but it's still a string
                        unresolved_refs.push(UnresolvedRef {
                            ref_id: generate_ref_id(),
                            location: RefLocation {
                                statement_index: stmt_idx,
                                verb: stmt.verb.clone(),
                                arg_name: arg.name.clone(),
                            },
                            search_value: value.clone(),
                            entity_type: arg_def.lookup.entity_type.clone(),
                            config: fetch_entity_config(&arg_def.lookup.entity_type),
                            ...
                        });
                    }
                }
            }
        }
    }
}
```

### 3. Update AgentService Pipeline

```rust
impl AgentService {
    pub async fn process_chat(&self, session: &mut AgentSession, req: &AgentChatRequest) 
        -> Result<AgentChatResponse> 
    {
        // ... intent extraction (LLM call) ...
        
        // Build DSL immediately (with unresolved refs as string literals)
        let dsl_source = build_dsl_program(&intents);
        
        // Parse (Stage 1)
        let ast = match parse_dsl(&dsl_source) {
            Ok(ast) => ast,
            Err(errors) => {
                return Ok(AgentChatResponse {
                    session_state: SessionState::SyntaxError { errors },
                    dsl_source: Some(dsl_source),
                    ...
                });
            }
        };
        
        // Semantic validation (Stage 2) - detects unresolved refs
        let validation = self.semantic_validator.validate(&ast);
        
        if !validation.unresolved_refs.is_empty() {
            // Fetch entity configs for each unresolved ref
            let refs_with_config = self.enrich_with_entity_config(
                &validation.unresolved_refs
            ).await;
            
            return Ok(AgentChatResponse {
                session_state: SessionState::UnresolvedRefs {
                    refs: refs_with_config,
                    current_index: 0,
                },
                dsl_source: Some(dsl_source), // DSL visible in editor!
                can_execute: false,
                ...
            });
        }
        
        // Dependency resolution (Stage 3)
        let exec_plan = match build_execution_plan(&ast) {
            Ok(plan) => plan,
            Err(errors) => {
                return Ok(AgentChatResponse {
                    session_state: SessionState::DependencyError { errors },
                    dsl_source: Some(dsl_source),
                    ...
                });
            }
        };
        
        // All good - ready to run
        Ok(AgentChatResponse {
            session_state: SessionState::ReadyToRun,
            dsl_source: Some(dsl_source),
            can_execute: true,
            ...
        })
    }
}
```

### 4. Handle Resolution Response

```rust
impl AgentService {
    pub async fn handle_resolution_response(
        &self, 
        session: &mut AgentSession,
        ref_id: &str,
        resolution: RefResolution,
    ) -> Result<AgentChatResponse> {
        // Update the AST with resolved entity
        let ast = session.pending.as_mut()
            .ok_or("No pending DSL")?;
        
        self.apply_resolution_to_ast(ast, ref_id, &resolution)?;
        
        // Regenerate DSL source from updated AST
        let dsl_source = ast_to_dsl_string(&ast.ast);
        
        // Re-run semantic validation (Stage 2)
        // This will detect if there are more unresolved refs
        let validation = self.semantic_validator.validate(&ast.ast);
        
        if !validation.unresolved_refs.is_empty() {
            // More refs to resolve
            return Ok(AgentChatResponse {
                session_state: SessionState::UnresolvedRefs {
                    refs: validation.unresolved_refs,
                    current_index: 0,
                },
                dsl_source: Some(dsl_source),
                ...
            });
        }
        
        // All refs resolved - proceed to dependency stage
        // ... same as above ...
    }
}
```

---

## Part 7: UI State Handling

### AppState Changes

```rust
// In ob-poc-ui/src/state.rs

pub struct AppState {
    // ... existing fields ...
    
    /// Current compiler pipeline stage
    pub pipeline_stage: Option<CompilerStage>,
    
    /// Unresolved refs (if in resolution state)
    pub unresolved_refs: Vec<UnresolvedRef>,
    
    /// Current ref being resolved
    pub current_ref_index: usize,
    
    /// DSL source (always shown once intent confirmed)
    pub dsl_source: Option<String>,
    
    /// Whether DSL was edited directly by user
    pub dsl_user_edited: bool,
}
```

### UI Response to State Changes

```rust
// In app.rs update()

fn process_chat_response(&mut self, response: ChatResponse) {
    // Always update DSL source if present
    if let Some(dsl) = response.dsl_source {
        self.state.buffers.dsl_editor = dsl;
        self.state.dsl_user_edited = false;
    }
    
    match response.session_state {
        SessionState::UnresolvedRefs { refs, current_index } => {
            // Store refs and open resolution modal
            self.state.unresolved_refs = refs;
            self.state.current_ref_index = current_index;
            
            // Populate resolution UI with first ref's config
            if let Some(ref first) = self.state.unresolved_refs.first() {
                self.state.resolution_ui.current_entity_type = 
                    Some(first.entity_type.clone());
                self.state.resolution_ui.search_keys = 
                    first.config.search_keys.clone();
                self.state.resolution_ui.discriminator_fields = 
                    first.config.discriminators.clone();
                self.state.resolution_ui.resolution_mode = 
                    first.config.resolution_mode.clone();
            }
            
            // Open resolution modal
            self.state.window_stack.push(WindowEntry {
                window_type: WindowType::Resolution,
                ...
            });
        }
        
        SessionState::ReadyToRun => {
            // Close resolution modal if open
            self.state.window_stack.close_by_type(WindowType::Resolution);
            // Show "Ready to run" status
        }
        
        // ... handle other states ...
    }
}
```

---

## Summary

The key changes are:

1. **Build DSL immediately** after intent extraction, even with unresolved refs
2. **Semantic validator detects unresolved refs** (not resolve_all)
3. **Entity type comes from verb arg definition**, not from pre-resolution
4. **Resolution modal gets config from EntityGateway** based on entity type
5. **DSL always visible in editor** - user sees what will execute
6. **Clear escape hatches** at every state
7. **Re-validation after each resolution** to detect if more refs remain

This separates:
- **Intent disambiguation** (what verb?) - happens before DSL
- **Entity resolution** (which entity?) - happens after DSL, driven by verb arg defs
