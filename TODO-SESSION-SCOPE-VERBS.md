# TODO: Session Scope Verbs & Entity Resolution UI

> **Priority:** CRITICAL - Blocking Allianz UAT
> **Phase 1:** Debug DSL pipeline + entity resolution popup
> **Phase 2:** SESSION scope verbs implementation
> **Status:** Required for agent-only UI to function

---

## Phase 1: Debug New UI DSL Pipeline

### Problem Statement

The re-skinned 3-panel UI (Viewport + Session State + Agent Chat) has a broken DSL execution path. Agent commands aren't reaching the viewport.

### Debugging Checklist

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      DSL PIPELINE DEBUG CHECKLIST                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  [ ] 1. Agent chat → DSL generation                                         │
│      - Is agent producing valid DSL?                                        │
│      - Check agent response contains DSL expressions                        │
│      - Log raw agent output before parsing                                  │
│                                                                             │
│  [ ] 2. DSL parsing                                                         │
│      - Is parser receiving the DSL string?                                  │
│      - Are viewport verbs being recognized?                                 │
│      - Log parse results (success/failure + AST)                            │
│                                                                             │
│  [ ] 3. Entity resolution                                                   │
│      - Are entity refs being resolved?                                      │
│      - Is resolver connected to database?                                   │
│      - Log resolution attempts + results                                    │
│                                                                             │
│  [ ] 4. DSL execution                                                       │
│      - Is executor receiving parsed verbs?                                  │
│      - Is ViewportState being mutated?                                      │
│      - Log pre/post execution state                                         │
│                                                                             │
│  [ ] 5. State propagation to UI                                             │
│      - Is mutated state being sent to frontend?                             │
│      - Is WebSocket/SSE connection alive?                                   │
│      - Log state updates sent to UI                                         │
│                                                                             │
│  [ ] 6. egui rendering                                                      │
│      - Is egui receiving new ViewportState?                                 │
│      - Is render loop checking for state changes?                           │
│      - Log state received by renderer                                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Add Debug Logging

```rust
// Add to each pipeline stage:

// 1. Agent output
tracing::debug!(target: "dsl_pipeline", stage = "agent_output", raw = %response);

// 2. Parse
tracing::debug!(target: "dsl_pipeline", stage = "parse", input = %dsl_string, success = %result.is_ok());

// 3. Resolution  
tracing::debug!(target: "dsl_pipeline", stage = "resolve", entity_ref = %ref_str, resolved = ?resolved);

// 4. Execution
tracing::debug!(target: "dsl_pipeline", stage = "execute", verb = ?verb, pre_state = ?state_before, post_state = ?state_after);

// 5. State propagation
tracing::debug!(target: "dsl_pipeline", stage = "propagate", state_hash = %hash, sent = %success);

// 6. Render
tracing::debug!(target: "dsl_pipeline", stage = "render", state_hash = %hash, focus = ?state.focus);
```

### Likely Failure Points

| Symptom | Likely Cause | Fix |
|---------|--------------|-----|
| Agent doesn't generate DSL | Agent prompt missing DSL instructions | Update system prompt |
| DSL not parsed | Verb not registered in parser | Add to grammar/parser |
| Entity not resolved | Resolver not connected to DB | Wire up resolver service |
| State not mutating | Executor not calling state methods | Check executor dispatch |
| UI not updating | WebSocket disconnected / no re-render trigger | Check connection + render loop |
| Render shows old state | State not being read from correct source | Check state binding |

---

## Phase 1b: Entity Resolution Search Popup

### UX Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│  User: "look up Allianz"                                                    │
│                                                                             │
│  Agent: Searching for "Allianz"...                                          │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  ENTITY RESOLUTION - 47 matches for "Allianz"                       │   │
│  │  ─────────────────────────────────────────────────────────────────  │   │
│  │                                                                     │   │
│  │  ● Allianz SE                                        [Select]       │   │
│  │    LEI: 529900K9WJHPHV2Q2L79 · Munich, DE · Active                 │   │
│  │    Ultimate parent · 47 subsidiaries                                │   │
│  │                                                                     │   │
│  │  ○ Allianz Global Investors GmbH                     [Select]       │   │
│  │    LEI: 5299009QHR3EKO2FYC82 · Frankfurt, DE · Active              │   │
│  │    Subsidiary of Allianz SE                                         │   │
│  │                                                                     │   │
│  │  ○ Allianz Investment Management SE                  [Select]       │   │
│  │    LEI: 549300KUBU8D9JQNEJ87 · Munich, DE · Active                 │   │
│  │    Subsidiary of Allianz SE                                         │   │
│  │                                                                     │   │
│  │  ○ Allianz Life Insurance Company of North America   [Select]       │   │
│  │    LEI: 549300OPVCF2PJY8UW05 · Minneapolis, US · Active            │   │
│  │                                                                     │   │
│  │  [Show 43 more...]                                                  │   │
│  │                                                                     │   │
│  │  [Cancel]                              [Select Best Match: Allianz SE]  │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Popup Behavior

| Condition | Behavior |
|-----------|----------|
| 0 matches | Show "No results" + suggest alternatives |
| 1 match (high confidence) | Auto-select, no popup |
| 1 match (low confidence) | Show popup for confirmation |
| 2-10 matches | Show popup with all options |
| 10+ matches | Show top 10 + "Show N more..." |

### Popup Data Model

```rust
pub struct EntityResolutionPopup {
    pub query: String,
    pub matches: Vec<EntityMatch>,
    pub total_count: usize,
    pub best_match: Option<usize>,  // Index of recommended selection
    pub state: PopupState,
}

pub struct EntityMatch {
    pub entity_id: Uuid,
    pub name: String,
    pub lei: Option<String>,
    pub entity_type: EntityType,
    pub jurisdiction: String,
    pub status: EntityStatus,
    pub parent_name: Option<String>,
    pub subsidiary_count: Option<usize>,
    pub confidence: f32,  // Match confidence 0.0-1.0
    pub match_reason: String,  // "Exact name match", "LEI match", "Fuzzy match"
}

pub enum PopupState {
    Open,
    Selected(usize),
    Cancelled,
}
```

### Integration with Agent Chat

```rust
// When agent executes search verb:
fn handle_search_result(&mut self, result: SearchResult) {
    match result.matches.len() {
        0 => {
            self.chat.add_message("No entities found for that query.");
        }
        1 if result.matches[0].confidence > 0.95 => {
            // Auto-select high confidence single match
            self.session.set_entity(result.matches[0].entity_id);
            self.chat.add_message(format!("Found: {}", result.matches[0].name));
        }
        _ => {
            // Show popup for user selection
            self.entity_popup = Some(EntityResolutionPopup::new(result));
        }
    }
}

// When user selects from popup:
fn handle_popup_selection(&mut self, index: usize) {
    if let Some(popup) = &self.entity_popup {
        let selected = &popup.matches[index];
        
        // Execute session verb to set context
        self.execute_dsl(format!(
            "(SESSION.set-entity \"{}\")", 
            selected.entity_id
        ));
        
        self.chat.add_message(format!("Selected: {}", selected.name));
        self.entity_popup = None;
    }
}
```

---

## Phase 2: SESSION Scope Verbs

### Verb Definitions

```yaml
# =============================================================================
# SESSION.set-client
# =============================================================================
SESSION.set-client:
  description: "Set the top-level client context for this session"
  params:
    - name: client_ref
      type: string
      description: "Client name, ID, or reference"
  effect: "Sets session.client to resolved client"
  returns: "Confirmation with client details"

# =============================================================================
# SESSION.set-cbu
# =============================================================================
SESSION.set-cbu:
  description: "Set CBU context, optionally for a specific scope (KYC/TRADING/PRODUCT)"
  params:
    - name: cbu_ref
      type: string
      description: "CBU name, ID, LEI, or @reference"
    - name: scope
      type: enum [KYC, TRADING, PRODUCT]
      optional: true
      default: "inferred from cbu type or KYC"
  effect: "Sets session.cbu[scope] to resolved CBU"
  returns: "Confirmation with CBU details"

# =============================================================================
# SESSION.set-entity
# =============================================================================
SESSION.set-entity:
  description: "Set focused entity within current CBU context"
  params:
    - name: entity_ref
      type: string
      description: "Entity name, ID, LEI, or @reference"
  effect: "Sets session.entity to resolved entity"
  returns: "Confirmation with entity details"

# =============================================================================
# SESSION.set-jurisdiction
# =============================================================================
SESSION.set-jurisdiction:
  description: "Set jurisdiction for compliance gates and regulatory checks"
  params:
    - name: jurisdiction
      type: enum [EU, UK, US, AU, HK, OTHER]
  effect: "Sets session.jurisdiction"
  returns: "Confirmation"

# =============================================================================
# SESSION.show-context / SESSION.context
# =============================================================================
SESSION.show-context:
  description: "Display current session context state"
  params: none
  effect: none (read-only)
  returns: "Formatted session state summary"

# =============================================================================
# SESSION.clear-context
# =============================================================================
SESSION.clear-context:
  description: "Reset session context (all or specific scope)"
  params:
    - name: scope
      type: enum [ALL, CLIENT, CBU, ENTITY, JURISDICTION]
      optional: true
      default: ALL
  effect: "Clears specified scope(s)"
  returns: "Confirmation"

# =============================================================================
# SESSION.use-cbu
# =============================================================================
SESSION.use-cbu:
  description: "Switch which CBU scope is 'current' (without changing assignments)"
  params:
    - name: scope
      type: enum [KYC, TRADING, PRODUCT]
  effect: "Sets session.active_cbu_scope"
  returns: "Confirmation with new active CBU"
```

---

## RAG Metadata - Comprehensive Helper Phrases

> **CRITICAL:** These verbs are highly exposed to natural language. Include every colloquialism, shorthand, and variation you can think of.

```rust
// =============================================================================
// CONTEXT-SETTING LEAD-IN PATTERNS
// These words/phrases signal "I'm about to set context" - apply to all SESSION.set-* verbs
// =============================================================================
//
// ACTION VERBS:        using, set, targeting, viewing, for, with, on, about, regarding
// SELECTION:           picking, choosing, selecting, grabbing, taking, going with
// FOCUS:               focusing, zooming, drilling, looking, examining, inspecting
// DIRECTIONAL:         switching to, moving to, going to, turning to, navigating to
// ASSIGNMENT:          assigning, making it, setting it to, that's, it's
// POSSESSION:          my, our, the, this, that, their
// TEMPORAL:            now, currently, at the moment, right now, today
// CONVERSATIONAL:      let's talk about, discussing, regarding, concerning, re:
// IMPLICIT:            [just entity name] + context clues from conversation
//
// =============================================================================

// =============================================================================
// SESSION.set-client
// =============================================================================
m.insert(
    "session.set-client",
    vec![
        // Direct commands
        "set client",
        "set the client",
        "switch client",
        "change client",
        "use client",
        "select client",
        "pick client",
        "choose client",
        
        // Working with patterns
        "working with",
        "working on",
        "work with",
        "work on",
        "i'm working with",
        "i'm working on",
        "we're working with",
        "we're working on",
        "let's work with",
        "let's work on",
        
        // Context setting
        "client is",
        "the client is",
        "client context",
        "set client context",
        "client scope",
        
        // Onboarding patterns
        "onboarding",
        "i'm onboarding",
        "we're onboarding",
        "start onboarding",
        "begin onboarding",
        "onboard",
        
        // Focus patterns
        "focus on client",
        "focusing on",
        "let's focus on",
        
        // Colloquial
        "this is for",
        "this one's for",
        "dealing with",
        "handling",
        "looking at client",
        
        // Lead-in patterns (using, set, targeting, viewing, for...)
        "using client",
        "targeting client",
        "viewing client",
        "for client",
        "with client",
        "on client",
        "about client",
        "regarding client",
        "concerning client",
        "re client",
        
        // Selection patterns
        "picking client",
        "choosing client",
        "selecting client",
        "grabbing client",
        "taking client",
        "going with client",
        
        // Directional
        "switching to client",
        "moving to client",
        "going to client",
        "turning to client",
        "navigating to client",
        
        // Assignment
        "client is",
        "that's the client",
        "it's client",
        "make it client",
        "the client's",
        
        // Temporal
        "now working with",
        "currently on",
        "at the moment",
        "right now",
        "today we're",
        "today's client",
        
        // Conversational
        "let's talk about client",
        "discussing client",
        "let's discuss",
        "about this client",
        "regarding this client",
        
        // Shorthand
        "client:",
        "for:",
        "re:",
    ],
);

// =============================================================================
// SESSION.set-cbu
// =============================================================================
m.insert(
    "session.set-cbu",
    vec![
        // Direct commands
        "set cbu",
        "set the cbu",
        "switch cbu",
        "change cbu",
        "use cbu",
        "select cbu",
        "pick cbu",
        "choose cbu",
        
        // CBU terminology variations
        "set business unit",
        "set client business unit",
        "use business unit",
        "switch business unit",
        
        // Scope-specific
        "set kyc cbu",
        "set trading cbu",
        "set product cbu",
        "kyc entity is",
        "trading entity is",
        "use for kyc",
        "use for trading",
        "for kyc purposes",
        "for trading purposes",
        
        // Working patterns
        "working with cbu",
        "work with cbu",
        "cbu is",
        "the cbu is",
        "using cbu",
        
        // Legal entity patterns
        "set legal entity",
        "use legal entity",
        "the legal entity is",
        "working with entity",
        "anchor entity is",
        "set anchor",
        "use anchor",
        
        // Focus patterns
        "focus on cbu",
        "focus cbu",
        "cbu context",
        "set cbu context",
        
        // Colloquial
        "this cbu",
        "that cbu",
        "switch to",
        "go to cbu",
        "open cbu",
        "load cbu",
        
        // Lead-in patterns
        "using cbu",
        "using business unit",
        "targeting cbu",
        "targeting entity",
        "viewing cbu",
        "for cbu",
        "with cbu",
        "on cbu",
        "about cbu",
        "regarding cbu",
        
        // Selection patterns
        "picking cbu",
        "choosing cbu",
        "selecting cbu",
        "grabbing cbu",
        "going with cbu",
        "take cbu",
        "taking cbu",
        
        // Directional
        "switching to cbu",
        "moving to cbu",
        "going to cbu",
        "navigating to cbu",
        "turning to cbu",
        "jump to cbu",
        "jumping to",
        
        // Assignment
        "that's the cbu",
        "it's cbu",
        "make it cbu",
        "the cbu's",
        "cbu:",
        
        // Temporal
        "now on cbu",
        "currently cbu",
        "right now cbu",
        
        // Conversational
        "let's talk about cbu",
        "discussing cbu",
        "about this cbu",
        "regarding this cbu",
        "concerning cbu",
        
        // Implicit possession
        "my cbu",
        "our cbu",
        "their cbu",
        "the cbu",
        "this cbu",
        "that cbu",
    ],
);

// =============================================================================
// SESSION.set-entity
// =============================================================================
m.insert(
    "session.set-entity",
    vec![
        // Direct commands
        "set entity",
        "set the entity",
        "switch entity",
        "change entity",
        "use entity",
        "select entity",
        "pick entity",
        "choose entity",
        "select this one",
        "pick this one",
        "choose this one",
        "use this one",
        "that one",
        "this one",
        
        // Focus patterns
        "focus on",
        "focus entity",
        "focusing on",
        "let's focus on",
        "zoom in on",
        "drill into",
        "look at",
        "looking at",
        "let's look at",
        "show me",
        "pull up",
        "bring up",
        "open",
        "load",
        
        // Selection from list
        "select the first",
        "select the second",
        "select the third",
        "pick the first",
        "pick the second",
        "use the first",
        "the first one",
        "the second one",
        "the top one",
        "the best match",
        "select number",
        "pick number",
        "option",
        "choice",
        
        // Working patterns
        "working with",
        "work with",
        "entity is",
        "the entity is",
        "using entity",
        
        // Company/organization patterns
        "set company",
        "use company",
        "select company",
        "the company is",
        "set organization",
        "use organization",
        "set org",
        
        // Person patterns (for UBO work)
        "set person",
        "select person",
        "the person is",
        "focus on person",
        
        // Colloquial
        "go with",
        "let's go with",
        "i want",
        "i'll take",
        "that's the one",
        "yeah that one",
        "yes that one",
        "correct one",
        "right one",
        
        // Lead-in patterns
        "using",
        "using entity",
        "targeting",
        "targeting entity",
        "viewing",
        "viewing entity",
        "for entity",
        "with entity",
        "on entity",
        "about entity",
        "regarding entity",
        "concerning",
        
        // Selection patterns
        "picking",
        "picking entity",
        "choosing",
        "choosing entity",
        "selecting",
        "grabbing",
        "taking",
        "going with",
        "gonna use",
        "wanna use",
        "want to use",
        
        // Directional
        "switching to",
        "moving to",
        "going to",
        "navigating to",
        "turning to",
        "jump to",
        "jumping to",
        "hop to",
        
        // Assignment/Confirmation
        "that's it",
        "it's that one",
        "make it",
        "set it to",
        "yep",
        "yes",
        "yeah",
        "correct",
        "right",
        "bingo",
        "exactly",
        "that one",
        "this guy",
        "that guy",
        
        // Temporal
        "now looking at",
        "currently on",
        "right now",
        
        // Conversational
        "let's talk about",
        "discussing",
        "about this",
        "regarding this",
        "tell me about",
        "show me",
        "give me",
        "get me",
        
        // Implicit possession
        "my entity",
        "our entity",
        "their entity",
        "the entity",
        "this entity",
        "that entity",
        
        // Numbered selection (critical for popup)
        "one",
        "two",
        "three",
        "four",
        "five",
        "1",
        "2",
        "3",
        "4",
        "5",
        "first",
        "second",
        "third",
        "fourth",
        "fifth",
        "top",
        "bottom",
        "last",
        "next",
        "previous",
        "other",
        "another",
    ],
);

// =============================================================================
// SESSION.set-jurisdiction
// =============================================================================
m.insert(
    "session.set-jurisdiction",
    vec![
        // Direct commands
        "set jurisdiction",
        "set the jurisdiction",
        "switch jurisdiction",
        "change jurisdiction",
        "use jurisdiction",
        "select jurisdiction",
        
        // Country/region patterns
        "set country",
        "set region",
        "use country",
        "use region",
        "country is",
        "region is",
        "jurisdiction is",
        
        // Specific jurisdictions
        "set eu",
        "use eu",
        "for eu",
        "in eu",
        "european",
        "europe",
        "set uk",
        "use uk",
        "for uk",
        "in uk",
        "british",
        "britain",
        "united kingdom",
        "set us",
        "use us",
        "for us",
        "in us",
        "american",
        "america",
        "united states",
        "set australia",
        "use australia",
        "for australia",
        "in australia",
        "australian",
        "aussie",
        "set hong kong",
        "use hong kong",
        "for hong kong",
        "in hong kong",
        "hk",
        
        // Regulatory context
        "for compliance",
        "compliance jurisdiction",
        "regulatory jurisdiction",
        "reporting jurisdiction",
        "mifir jurisdiction",
        "emir jurisdiction",
        
        // Colloquial
        "we're in",
        "they're in",
        "based in",
        "operating in",
        "trading in",
        "booking in",
        
        // Lead-in patterns
        "using jurisdiction",
        "targeting jurisdiction",
        "for jurisdiction",
        "with jurisdiction",
        "under jurisdiction",
        
        // Regulatory context expanded
        "under mifir",
        "under emir",
        "under sftr",
        "under cftc",
        "mifir rules",
        "emir rules",
        "eu rules",
        "uk rules",
        "us rules",
        "regulatory",
        "regulation",
        "compliant with",
        "compliance for",
        
        // Shorthand country references
        "eu",
        "uk",
        "us",
        "usa",
        "au",
        "hk",
        "emea",
        "apac",
        "amer",
        "americas",
        "asia",
        "asia pac",
        "europe",
        "european union",
        "britain",
        "great britain",
        "england",
        "germany",
        "german",
        "france",
        "french",
        "singapore",
        "japan",
        "japanese",
        "swiss",
        "switzerland",
        "luxembourg",
        "ireland",
        "irish",
        "cayman",
        "caymans",
        "bermuda",
        "jersey",
        "guernsey",
        "dubai",
        "uae",
        
        // Booking/Trading location patterns
        "booking location",
        "trading location",
        "execution venue",
        "booked in",
        "traded in",
        "executed in",
        "clearing in",
        "settled in",
        "custodied in",
        "domiciled in",
        "incorporated in",
        "registered in",
        "headquartered in",
        "hq in",
        "home market",
        
        // Assignment
        "jurisdiction is",
        "that's jurisdiction",
        "jur:",
        "loc:",
    ],
);

// =============================================================================
// SESSION.show-context / SESSION.context
// =============================================================================
m.insert(
    "session.show-context",
    vec![
        // Direct commands
        "show context",
        "show session",
        "show state",
        "show current context",
        "show current state",
        "show session state",
        "display context",
        "display session",
        "display state",
        
        // Question patterns
        "what context",
        "what's the context",
        "what is the context",
        "what's current",
        "what is current",
        "what's selected",
        "what is selected",
        "what's set",
        "what is set",
        "what am i looking at",
        "what are we looking at",
        "what's loaded",
        "what is loaded",
        
        // Current state queries
        "current client",
        "current cbu",
        "current entity",
        "current jurisdiction",
        "which client",
        "which cbu",
        "which entity",
        "which jurisdiction",
        
        // Where am I patterns
        "where am i",
        "where are we",
        "what's active",
        "what's in scope",
        "scope",
        "context",
        "session",
        "status",
        
        // Colloquial
        "remind me",
        "what was it",
        "what did i set",
        "what did we set",
        "show me context",
        "tell me context",
        "give me context",
        
        // Uncertainty patterns
        "wait what",
        "hold on",
        "hang on",
        "wait",
        "lost track",
        "forgot",
        "can't remember",
        "don't remember",
        "confused",
        "which one again",
        "what was that",
        
        // Verification patterns
        "is it",
        "is that",
        "are we on",
        "am i on",
        "do i have",
        "do we have",
        "confirm",
        "verify",
        "check",
        "double check",
        "sanity check",
        
        // Summary requests
        "summary",
        "overview",
        "recap",
        "rundown",
        "where we at",
        "where are we at",
        "where we're at",
        "state of play",
        "current state",
        "current setup",
        "what's configured",
        "what's set up",
    ],
);

// =============================================================================
// SESSION.clear-context
// =============================================================================
m.insert(
    "session.clear-context",
    vec![
        // Direct commands
        "clear context",
        "clear session",
        "clear state",
        "reset context",
        "reset session",
        "reset state",
        "clear all",
        "reset all",
        
        // Start fresh patterns
        "start fresh",
        "start over",
        "start again",
        "begin again",
        "fresh start",
        "clean slate",
        "new session",
        "new start",
        
        // Unset patterns
        "unset",
        "unset client",
        "unset cbu",
        "unset entity",
        "clear client",
        "clear cbu",
        "clear entity",
        "remove context",
        
        // Colloquial
        "forget that",
        "forget everything",
        "never mind",
        "scratch that",
        "wipe",
        "wipe it",
        "blank slate",
        
        // Undo patterns
        "undo",
        "undo that",
        "go back",
        "back",
        "revert",
        "rollback",
        "take it back",
        
        // Dismissal patterns
        "nope",
        "no",
        "nah",
        "wrong",
        "not that",
        "not that one",
        "cancel",
        "abort",
        "stop",
        "hold",
        "wait no",
        "actually no",
        "oops",
        "my bad",
        "mistake",
        
        // Reset idioms
        "from scratch",
        "from the top",
        "from zero",
        "zero out",
        "null it",
        "clear it out",
        "dump it",
        "trash it",
        "toss it",
        "ditch it",
        "lose it",
        "drop it",
        "let go",
        "release",
    ],
);

// =============================================================================
// SESSION.use-cbu (scope switching)
// =============================================================================
m.insert(
    "session.use-cbu",
    vec![
        // Direct commands
        "use cbu scope",
        "switch cbu scope",
        "change cbu scope",
        "switch to kyc",
        "switch to trading",
        "switch to product",
        "use kyc",
        "use trading",
        "use product",
        
        // Mode patterns
        "kyc mode",
        "trading mode",
        "product mode",
        "go to kyc mode",
        "go to trading mode",
        
        // Scope patterns
        "kyc scope",
        "trading scope",
        "product scope",
        "set scope to",
        "change scope to",
        
        // Activity patterns
        "for kyc work",
        "for trading work",
        "doing kyc",
        "doing trading",
        "working on kyc",
        "working on trading",
        
        // Colloquial
        "let's do kyc",
        "let's do trading",
        "back to kyc",
        "back to trading",
        
        // Purpose patterns
        "for kyc",
        "for trading",
        "for product",
        "kyc purposes",
        "trading purposes",
        "product purposes",
        "need kyc",
        "need trading",
        "want kyc",
        "want trading",
        
        // Activity patterns  
        "doing kyc now",
        "doing trading now",
        "kyc time",
        "trading time",
        "kyc stuff",
        "trading stuff",
        
        // Switching idioms
        "flip to kyc",
        "flip to trading",
        "swap to kyc",
        "swap to trading",
        "toggle kyc",
        "toggle trading",
    ],
);

// =============================================================================
// Generic context/scope patterns (map to show-context or set-*)
// These are LEAD-IN words that signal context-setting intent
// NLU needs to determine which specific verb based on what follows
// =============================================================================
m.insert(
    "session.generic",
    vec![
        // === PRIMARY LEAD-INS ===
        // These almost always precede a context-setting operation
        "using",
        "set",
        "targeting",
        "viewing",
        "for",
        "with",
        "on",
        "about",
        "regarding",
        "concerning",
        "re",
        
        // === SELECTION LEAD-INS ===
        "picking",
        "choosing",
        "selecting",
        "grabbing",
        "taking",
        "going with",
        
        // === FOCUS LEAD-INS ===
        "focusing",
        "focus",
        "zooming",
        "zoom",
        "drilling",
        "drill",
        "looking",
        "look",
        "examining",
        "examine",
        "inspecting",
        "inspect",
        
        // === DIRECTIONAL LEAD-INS ===
        "switching",
        "switch",
        "moving",
        "move",
        "going",
        "go",
        "turning",
        "turn",
        "navigating",
        "navigate",
        "jumping",
        "jump",
        "hopping",
        "hop",
        
        // === ASSIGNMENT LEAD-INS ===
        "that's",
        "it's",
        "this is",
        "make it",
        "set it",
        
        // === POSSESSION LEAD-INS ===
        "my",
        "our",
        "the",
        "this",
        "that",
        "their",
        
        // === TEMPORAL LEAD-INS ===
        "now",
        "currently",
        "right now",
        "at the moment",
        "today",
        
        // === CONVERSATIONAL LEAD-INS ===
        "let's",
        "let us",
        "can we",
        "could we",
        "shall we",
        "we should",
        "i want",
        "i need",
        "i'd like",
        "we want",
        "we need",
        "we'd like",
        
        // === QUESTION LEAD-INS (often lead to show-context) ===
        "what",
        "which",
        "where",
        "who",
        "how",
        "is it",
        "are we",
        "am i",
        "do we",
        "do i",
        
        // === SCOPE NOUNS ===
        "context",
        "scope",
        "session",
        "state",
        "current",
        "active",
        "selected",
        "focused",
        "target",
        "working",
    ],
);

// =============================================================================
// ENTITY NAME PATTERNS
// When user just says an entity name, often means "set that as context"
// These help recognize bare entity references
// =============================================================================
m.insert(
    "session.entity_name_patterns",
    vec![
        // Company suffixes (recognize as entity names)
        "inc",
        "inc.",
        "incorporated",
        "corp",
        "corp.",
        "corporation",
        "llc",
        "l.l.c.",
        "ltd",
        "ltd.",
        "limited",
        "plc",
        "p.l.c.",
        "gmbh",
        "ag",
        "sa",
        "s.a.",
        "nv",
        "n.v.",
        "bv",
        "b.v.",
        "se",  // Societas Europaea
        "kg",
        "ohg",
        "co",
        "co.",
        "company",
        "group",
        "holdings",
        "holding",
        "partners",
        "partnership",
        "lp",
        "l.p.",
        "llp",
        "l.l.p.",
        "fund",
        "funds",
        "trust",
        "bank",
        "banking",
        "capital",
        "asset",
        "assets",
        "management",
        "investment",
        "investments",
        "insurance",
        "assurance",
        "securities",
        "financial",
        "services",
        
        // LEI patterns
        "lei",
        "lei:",
        
        // After these words, next thing is likely an entity name
        "called",
        "named",
        "known as",
        "aka",
        "a.k.a.",
    ],
);
```

---

## Session State Data Model

```rust
/// Complete session state - source of truth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Session ID
    pub id: Uuid,
    
    /// Top-level client (optional - may work without)
    pub client: Option<ClientRef>,
    
    /// CBU scopes - can have multiple simultaneously
    pub cbu_scopes: CbuScopes,
    
    /// Active CBU scope (which one is "current")
    pub active_cbu_scope: CbuScopeType,
    
    /// Focused entity within current CBU
    pub entity: Option<EntityRef>,
    
    /// Jurisdiction for compliance gates
    pub jurisdiction: Option<Jurisdiction>,
    
    /// Temporal scope (for historical queries)
    pub as_of: AsOfDate,
    
    /// Viewport state (focus, enhance, view)
    pub viewport: ViewportState,
    
    /// Last modified timestamp
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuScopes {
    pub kyc: Option<CbuRef>,
    pub trading: Option<CbuRef>,
    pub product: Option<CbuRef>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum CbuScopeType {
    #[default]
    Kyc,
    Trading,
    Product,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AsOfDate {
    Latest,
    Specific(NaiveDate),
}

impl SessionState {
    /// Get the currently active CBU
    pub fn current_cbu(&self) -> Option<&CbuRef> {
        match self.active_cbu_scope {
            CbuScopeType::Kyc => self.cbu_scopes.kyc.as_ref(),
            CbuScopeType::Trading => self.cbu_scopes.trading.as_ref(),
            CbuScopeType::Product => self.cbu_scopes.product.as_ref(),
        }
    }
    
    /// Format for display in session panel
    pub fn format_display(&self) -> String {
        let mut lines = vec![];
        
        if let Some(client) = &self.client {
            lines.push(format!("Client: {}", client.name));
        }
        
        lines.push("CBU Scopes:".to_string());
        if let Some(cbu) = &self.cbu_scopes.kyc {
            let marker = if matches!(self.active_cbu_scope, CbuScopeType::Kyc) { "✓" } else { " " };
            lines.push(format!("  KYC: {} {}", cbu.name, marker));
        }
        if let Some(cbu) = &self.cbu_scopes.trading {
            let marker = if matches!(self.active_cbu_scope, CbuScopeType::Trading) { "✓" } else { " " };
            lines.push(format!("  TRADING: {} {}", cbu.name, marker));
        }
        if let Some(cbu) = &self.cbu_scopes.product {
            let marker = if matches!(self.active_cbu_scope, CbuScopeType::Product) { "✓" } else { " " };
            lines.push(format!("  PRODUCT: {} {}", cbu.name, marker));
        }
        
        if let Some(entity) = &self.entity {
            lines.push(format!("Entity: {}", entity.name));
            if let Some(lei) = &entity.lei {
                lines.push(format!("  LEI: {}", lei));
            }
        }
        
        if let Some(j) = &self.jurisdiction {
            lines.push(format!("Jurisdiction: {:?}", j));
        }
        
        lines.push(format!("As-of: {:?}", self.as_of));
        
        lines.join("\n")
    }
}
```

---

## Files to Create/Modify

### Phase 1 (Debug + Popup)

| File | Action | Description |
|------|--------|-------------|
| `rust/crates/ob-poc-ui/src/app.rs` | MODIFY | Add debug logging to DSL pipeline |
| `rust/crates/ob-poc-ui/src/panels/chat.rs` | MODIFY | Wire agent output to DSL executor |
| `rust/crates/ob-poc-ui/src/panels/entity_popup.rs` | CREATE | Entity resolution popup widget |
| `rust/crates/ob-poc-ui/src/panels/mod.rs` | MODIFY | Export entity_popup module |
| `rust/src/api/agent_service.rs` | MODIFY | Add debug logging |
| `rust/src/dsl_v2/executor.rs` | MODIFY | Add debug logging |

### Phase 2 (SESSION Verbs)

| File | Action | Description |
|------|--------|-------------|
| `rust/crates/dsl-core/src/ast.rs` | MODIFY | Add SessionVerb AST nodes |
| `rust/crates/dsl-core/src/session_parser.rs` | CREATE | Parser for SESSION verbs |
| `rust/crates/dsl-core/src/lib.rs` | MODIFY | Export session_parser |
| `rust/src/session/session_state.rs` | CREATE | SessionState struct |
| `rust/src/session/session_executor.rs` | CREATE | SESSION verb execution |
| `rust/src/session/mod.rs` | MODIFY | Export new modules |
| `rust/src/session/verb_rag_metadata.rs` | MODIFY | Add SESSION verb helper phrases |

---

## Acceptance Criteria

### Phase 1

- [ ] Debug logging shows complete pipeline trace
- [ ] Can identify exactly where DSL execution fails
- [ ] Entity resolution popup appears for ambiguous searches
- [ ] Single high-confidence result auto-selects (no popup)
- [ ] User selection from popup sets session entity
- [ ] Session state panel shows current entity after selection

### Phase 2

- [ ] `SESSION.set-cbu` resolves and sets CBU context
- [ ] `SESSION.set-entity` resolves and sets entity focus
- [ ] `SESSION.set-jurisdiction` sets compliance jurisdiction
- [ ] `SESSION.show-context` displays formatted session state
- [ ] `SESSION.clear-context` resets session
- [ ] All colloquial phrases in RAG metadata route to correct verb
- [ ] "working with Allianz" → `SESSION.set-client "Allianz"`
- [ ] "the cbu is Allianz SE" → `SESSION.set-cbu "Allianz SE"`
- [ ] "use this one" (after search) → `SESSION.set-entity @result.selected`
- [ ] "what's the context" → `SESSION.show-context`
- [ ] Session state panel updates in real-time

---

## Testing Scenarios

```
# Scenario 1: Basic entity search and selection
User: "look up Allianz"
→ Search executes, popup shows 47 matches
User: [clicks "Allianz SE"]
→ Entity selected, session shows "Entity: Allianz SE"
→ Viewport focuses on entity

# Scenario 2: Natural language context setting
User: "I'm working with Allianz SE for KYC"
→ Agent: (SESSION.set-cbu "Allianz SE" :scope KYC)
→ Session shows "CBU (KYC): Allianz SE ✓"

# Scenario 3: Jurisdiction for compliance
User: "this is for EU trading"
→ Agent: (SESSION.set-jurisdiction EU)
         (SESSION.use-cbu TRADING)
→ Session shows "Jurisdiction: EU", "Active: TRADING"

# Scenario 4: Context check
User: "what am I looking at"
→ Agent: (SESSION.show-context)
→ Chat shows formatted session state

# Scenario 5: Reset
User: "start fresh"
→ Agent: (SESSION.clear-context)
→ Session clears all scopes
```
