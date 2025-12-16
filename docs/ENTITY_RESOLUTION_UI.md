# Entity Resolution UI Design

## Problem Statement

Entity resolution is the most time-consuming part of the DSL pipeline. When an agent generates DSL or a template is expanded, entity references (names like "Allianz Global Investors GmbH") need to be resolved to UUIDs before execution.

Currently this happens:
1. **Silently** - User doesn't see what's being resolved
2. **All-or-nothing** - If one entity can't resolve, the whole batch fails
3. **Without user input** - Ambiguous matches can't be disambiguated

The key insight: **This DSL is a hybrid of function calls and data references**. Unlike a pure programming language where all identifiers are code-defined, our DSL references live database entities. This requires a human-in-the-loop resolution experience.

## Design Goals

1. **Integrated into agent chat** - Resolution UI is part of the REPL session, not a separate modal
2. **Batch-aware** - Show all unresolved entities from current DSL at once
3. **Search-schema-driven** - Each entity type has its own search pattern (s-expression based)
4. **Progressive resolution** - User can resolve entities one by one, seeing DSL update
5. **Agent-invokable** - Agent can trigger resolution panel with specific entities

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           CHAT PANEL                                        │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │ User: "Add Allianz Global Investors as IM for all Allianz funds"      │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │ Agent: Generated DSL with 3 unresolved entities...                    │ │
│  │ [Resolve Entities]                                                     │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                             │
│  ┌─────────────────── RESOLUTION PANEL ───────────────────────────────────┐ │
│  │ Unresolved References (3)                                              │ │
│  │ ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ │ │
│  │                                                                        │ │
│  │ 1. "Allianz Global Investors" (entity - LIMITED_COMPANY)              │ │
│  │    ┌──────────────────────────────────────────┐                       │ │
│  │    │ 🔍 Allianz Global                        │                       │ │
│  │    └──────────────────────────────────────────┘                       │ │
│  │    Matches:                                                           │ │
│  │    ○ Allianz Global Investors GmbH (DE) ← 95% match                  │ │
│  │    ○ Allianz Global Investors Luxembourg S.A. (LU) ← 92% match       │ │
│  │    ○ Allianz Global Investors UK Limited (GB) ← 88% match            │ │
│  │    [Select] [Skip] [Create New]                                       │ │
│  │                                                                        │ │
│  │ 2. "Pacific Growth Fund" (cbu)                                        │ │
│  │    ⚠ No matches found                                                 │ │
│  │    [Create New CBU] [Enter UUID manually]                             │ │
│  │                                                                        │ │
│  │ 3. "DIRECTOR" (role)                                                  │ │
│  │    ✓ Auto-resolved → DIRECTOR                                         │ │
│  │                                                                        │ │
│  │ ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ │ │
│  │ [Resolve All] [Cancel]                              Resolved: 1/3     │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ 1. AGENT GENERATES DSL                                                      │
│    Template expansion or natural language → DSL with entity names           │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 2. ENRICHMENT PASS                                                          │
│    Parser → Enrichment converts strings to EntityRef based on verb YAML     │
│    EntityRef { entity_type, value, resolved_key: None }                     │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 3. COLLECT UNRESOLVED                                                       │
│    Walk AST, extract all EntityRef where resolved_key = None                │
│    Group by entity_type, include verb YAML search_key config                │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 4. RESOLUTION REQUEST → UI                                                  │
│    POST /api/session/:id/resolution/start                                   │
│    { unresolved: [{ ref_id, entity_type, search_value, search_schema }] }   │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 5. UI RENDERS RESOLUTION PANEL                                              │
│    For each unresolved ref:                                                 │
│    - Show search input pre-filled with value                                │
│    - Fetch matches from EntityGateway                                       │
│    - Display ranked results with discriminators                             │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 6. USER RESOLVES                                                            │
│    - Selects match → resolved_key = UUID                                    │
│    - Or refines search → new query to EntityGateway                         │
│    - Or creates new → mini-form for entity creation                         │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 7. RESOLUTION COMMIT                                                        │
│    POST /api/session/:id/resolution/commit                                  │
│    { resolutions: [{ ref_id, resolved_key, display }] }                     │
│    Server updates AST EntityRefs with resolved_key                          │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 8. DSL READY FOR EXECUTION                                                  │
│    All EntityRefs now have resolved_key → can execute                       │
└─────────────────────────────────────────────────────────────────────────────┘
```

## API Design

### Start Resolution Session

```
POST /api/session/:id/resolution/start
```

Request: (empty - server extracts unresolved from current session DSL)

Response:
```json
{
  "resolution_id": "uuid",
  "unresolved": [
    {
      "ref_id": "ref-001",
      "entity_type": "entity",
      "entity_subtype": "LIMITED_COMPANY",
      "search_value": "Allianz Global Investors",
      "search_schema": {
        "primary_field": "name",
        "discriminators": [
          { "field": "jurisdiction", "selectivity": 0.8 }
        ]
      },
      "context": {
        "verb": "cbu.assign-role",
        "arg_name": "entity-id",
        "statement_index": 2
      },
      "initial_matches": [
        {
          "id": "uuid-1",
          "display": "Allianz Global Investors GmbH",
          "score": 0.95,
          "discriminators": { "jurisdiction": "DE" }
        }
      ]
    }
  ],
  "auto_resolved": [
    {
      "ref_id": "ref-002",
      "entity_type": "role",
      "search_value": "DIRECTOR",
      "resolved_key": "DIRECTOR",
      "display": "Director"
    }
  ]
}
```

### Search for Entity

```
POST /api/session/:id/resolution/search
```

Request:
```json
{
  "ref_id": "ref-001",
  "query": "Allianz GmbH",
  "discriminators": {
    "jurisdiction": "DE"
  }
}
```

Response:
```json
{
  "matches": [
    {
      "id": "6e594583-c7b3-4e9f-b243-07229adeedda",
      "display": "Allianz Global Investors GmbH",
      "score": 0.98,
      "discriminators": {
        "jurisdiction": "DE",
        "registration_number": "HRB 9340"
      }
    }
  ]
}
```

### Commit Resolutions

```
POST /api/session/:id/resolution/commit
```

Request:
```json
{
  "resolutions": [
    {
      "ref_id": "ref-001",
      "resolved_key": "6e594583-c7b3-4e9f-b243-07229adeedda",
      "display": "Allianz Global Investors GmbH"
    }
  ]
}
```

Response:
```json
{
  "success": true,
  "resolved_count": 3,
  "remaining_unresolved": 0,
  "updated_dsl": "(cbu.assign-role :cbu-id @fund :entity-id (\"entity\" \"Allianz Global Investors GmbH\" \"6e594583-...\") ...)"
}
```

## Key Types

### UnresolvedRef

```rust
/// An unresolved entity reference from the AST
pub struct UnresolvedRef {
    /// Unique ID for this resolution task
    pub ref_id: String,
    /// Entity type from verb YAML lookup config
    pub entity_type: String,
    /// Entity subtype if applicable (e.g., LIMITED_COMPANY)
    pub entity_subtype: Option<String>,
    /// The search value from the DSL
    pub search_value: String,
    /// Search schema from verb YAML (s-expression parsed)
    pub search_schema: SearchSchema,
    /// Context: which verb/arg this came from
    pub context: RefContext,
    /// Initial matches from EntityGateway (pre-fetched)
    pub initial_matches: Vec<EntityMatch>,
}

/// Search schema parsed from verb YAML search_key s-expression
pub struct SearchSchema {
    /// Primary search field (e.g., "name", "search_name")
    pub primary_field: String,
    /// Discriminator fields with selectivity
    pub discriminators: Vec<Discriminator>,
    /// Minimum confidence threshold
    pub min_confidence: Option<f64>,
}

/// A discriminator field for narrowing search
pub struct Discriminator {
    pub field: String,
    pub selectivity: f64,
    /// Maps to DSL arg name if different
    pub from_arg: Option<String>,
}

/// Context about where this ref appears in the DSL
pub struct RefContext {
    pub verb: String,
    pub arg_name: String,
    pub statement_index: usize,
    pub span: SourceSpan,
}
```

### Resolution Session State

```rust
/// Resolution session attached to agent session
pub struct ResolutionSession {
    pub id: Uuid,
    pub session_id: Uuid,
    /// All refs needing resolution
    pub unresolved: Vec<UnresolvedRef>,
    /// Refs that auto-resolved (exact match)
    pub auto_resolved: Vec<ResolvedRef>,
    /// User resolutions (in progress)
    pub pending_resolutions: HashMap<String, ResolvedRef>,
    /// State
    pub state: ResolutionState,
}

pub enum ResolutionState {
    /// Collecting resolutions
    Active,
    /// All resolved, ready to commit
    Complete,
    /// User cancelled
    Cancelled,
    /// Committed to session
    Committed,
}
```

## UI Components

### ResolutionPanel (egui/WASM)

```rust
pub struct ResolutionPanel {
    /// Current resolution session
    session: Option<ResolutionSession>,
    /// Search state per ref
    search_states: HashMap<String, SearchState>,
    /// Currently focused ref
    focused_ref: Option<String>,
}

struct SearchState {
    query: String,
    matches: Vec<EntityMatch>,
    loading: bool,
    selected: Option<usize>,
}

impl ResolutionPanel {
    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &ResolutionContext) {
        // Header with progress
        ui.horizontal(|ui| {
            ui.heading("Entity Resolution");
            let resolved = self.session.as_ref().map(|s| s.pending_resolutions.len()).unwrap_or(0);
            let total = self.session.as_ref().map(|s| s.unresolved.len()).unwrap_or(0);
            ui.label(format!("{}/{} resolved", resolved, total));
        });

        // Scrollable list of unresolved refs
        egui::ScrollArea::vertical().show(ui, |ui| {
            for unresolved in &self.session.as_ref().unwrap().unresolved {
                self.show_ref_resolver(ui, unresolved, ctx);
            }
        });

        // Actions
        ui.horizontal(|ui| {
            if ui.button("Resolve All").clicked() {
                ctx.commit_resolutions();
            }
            if ui.button("Cancel").clicked() {
                ctx.cancel_resolution();
            }
        });
    }

    fn show_ref_resolver(&mut self, ui: &mut egui::Ui, unresolved: &UnresolvedRef, ctx: &ResolutionContext) {
        let state = self.search_states.entry(unresolved.ref_id.clone()).or_default();
        
        // Entity type badge + search value
        ui.horizontal(|ui| {
            ui.label(format!("{}", unresolved.entity_type));
            if let Some(subtype) = &unresolved.entity_subtype {
                ui.small(subtype);
            }
        });

        // Search input
        let response = ui.text_edit_singleline(&mut state.query);
        if response.changed() {
            ctx.trigger_search(&unresolved.ref_id, &state.query);
        }

        // Matches list
        for (i, m) in state.matches.iter().enumerate() {
            let selected = state.selected == Some(i);
            if ui.selectable_label(selected, &m.display).clicked() {
                state.selected = Some(i);
                ctx.select_resolution(&unresolved.ref_id, &m.id);
            }
            // Show discriminators
            for (k, v) in &m.discriminators {
                ui.small(format!("{}: {}", k, v));
            }
        }

        // No matches action
        if state.matches.is_empty() && !state.loading {
            ui.label("No matches found");
            if ui.button("Create New").clicked() {
                ctx.create_entity(&unresolved.entity_type, &state.query);
            }
        }
    }
}
```

## Integration Points

### 1. Agent Chat Integration

The agent can trigger resolution via a special message type:

```rust
pub enum ChatResponseContent {
    Text(String),
    Dsl { source: String, status: DslStatus },
    ResolutionRequired {
        dsl_id: Uuid,
        unresolved_count: usize,
        message: String,
    },
}
```

When agent returns `ResolutionRequired`, the UI:
1. Shows the message in chat
2. Opens the resolution panel inline
3. Pauses further agent interaction until resolved

### 2. Session State Integration

The resolution session is attached to the agent session:

```rust
pub struct AgentSession {
    pub id: Uuid,
    pub state: SessionState,
    pub pending_dsl: Vec<PendingDsl>,
    pub executed_dsl: Vec<ExecutedDsl>,
    // NEW: Resolution state
    pub resolution: Option<ResolutionSession>,
}
```

### 3. Verb YAML Search Schema

Each verb argument with a `lookup:` block provides the search schema:

```yaml
args:
  - name: entity-id
    type: uuid
    required: true
    lookup:
      entity_type: entity
      search_key: "(name (jurisdiction :selectivity 0.8))"
      primary_key: entity_id
      resolution_mode: entity  # triggers search modal, not dropdown
```

The `search_key` s-expression is parsed to `SearchSchema` for the UI.

## Implementation Phases

### Phase 1: Core API (Backend)

1. Add `ResolutionSession` to session state
2. Implement `/resolution/start` - extract unresolved refs from AST
3. Implement `/resolution/search` - proxy to EntityGateway with schema
4. Implement `/resolution/commit` - update AST EntityRefs

### Phase 2: UI Panel (WASM/egui)

1. Create `ResolutionPanel` component
2. Integrate with chat panel (inline expansion)
3. Implement search with debounce
4. Implement match selection and commit

### Phase 3: Agent Integration

1. Add `ResolutionRequired` response type
2. Agent detects unresolved refs before returning DSL
3. Agent can describe entities to help user resolve

### Phase 4: Progressive Enhancement

1. Bulk resolution (auto-resolve high-confidence matches)
2. Resolution history (remember user choices)
3. Create-in-place (mini entity creation form)
4. Discriminator refinement UI (add DOB, jurisdiction filters)

## Benefits

1. **Transparency** - User sees exactly what's being resolved
2. **Control** - User can disambiguate, not just accept/reject
3. **Efficiency** - Batch resolution, not one-at-a-time errors
4. **Learning** - User learns entity naming conventions
5. **Hybrid power** - Combines agent intelligence with human judgment
