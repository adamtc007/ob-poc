# 025 Entity Disambiguation UX Design

## Problem Statement

When DSL contains unresolved entity references (e.g., `entity:create :name "John Smith"`), the system needs to:
1. Detect the ambiguous reference
2. Present a search/disambiguation UI
3. Allow progressive refinement with discriminators
4. Resolve to a specific UUID
5. Update the AST and continue

Current state: Infrastructure exists (EntityGateway, resolution API, batch resolver), but **no UI wiring**.

---

## Two Modes: Inline + Modal

**Key insight:** Different UX for different contexts.

| Mode | Trigger | Use Case | UX Pattern |
|------|---------|----------|------------|
| **Inline Popup** | Ctrl+. on squiggly, or click | Single unresolved ref | Zed/LSP code action style |
| **Batch Modal** | Compile with 3+ unresolved, or "Resolve All" | Multiple refs | Full workflow with stack |

Both share:
- Same search API (`/api/entity/search`)
- Same `EntityMatch` rendering (sized differently)
- Same resolution endpoint
- Same voice refinement parser (optional in inline)

```
                    ┌─────────────────┐
                    │  EntitySearch   │  ← shared search/render logic
                    │    Component    │
                    └────────┬────────┘
                             │
              ┌──────────────┴──────────────┐
              ▼                              ▼
    ┌───────────────────┐          ┌───────────────────┐
    │  InlineRefPopup   │          │  BatchRefModal    │
    │  (Zed-style)      │          │  (full workflow)  │
    │                   │          │                   │
    │  - compact        │          │  - DSL context    │
    │  - 5-6 results    │          │  - refinement     │
    │  - single ref     │          │  - voice          │
    │  - Ctrl+. trigger │          │  - progress stack │
    └───────────────────┘          └───────────────────┘
```

---

## Inline Popup (Zed-style)

### Trigger
- Squiggly underline on unresolved ref in DSL editor
- Click squiggly OR `Ctrl+.` (code action shortcut)

### Layout

```
entity:create :name "John Smith̲̲̲̲̲̲̲̲̲̲"  ← squiggly underline
                    ┌──────────────────────────────────┐
                    │ 🔍 John Smith                    │
                    ├──────────────────────────────────┤
                    │ John Smith (98%)                 │
                    │   UK | DOB 1965 | BlackRock      │
                    ├──────────────────────────────────┤
                    │ John Smith (72%)                 │
                    │   US | DOB 1980 | Vanguard       │
                    ├──────────────────────────────────┤
                    │ John A. Smith (65%)              │
                    │   UK | DOB 1975                  │
                    ├──────────────────────────────────┤
                    │ + Create new entity...           │
                    └──────────────────────────────────┘
```

### Behavior
- **Auto-search** on popup open (uses EntityRef.value as initial query)
- **Live filter** as user types in search box
- **Keyboard nav**: ↑↓ to move, Enter to select, Esc to close
- **Click** any row to select
- **"+ Create new"** opens minimal inline form or switches to modal

### Compact Design Rules
- Max 5-6 visible results (scroll if more)
- No refinement input (just type more specific name)
- No voice button (use modal for voice)
- No DSL context (you're already looking at it)
- Width: ~300px, anchored below the squiggly

### On Select
1. Update `EntityRef.resolved_key` in AST
2. Replace squiggly with resolved indicator (green underline? checkmark?)
3. Close popup
4. If more unresolved refs exist, show subtle "2 more unresolved" indicator

---

## Batch Modal (Full Workflow)

### Trigger
- Compile/validate with 3+ unresolved EntityRefs
- Click "Resolve All" button in toolbar
- Voice: "Resolve all entities"

---

## Design Principles

1. **Inline context** - Show the offending DSL, not just the entity name
2. **Progressive disclosure** - Start simple, add discriminators on demand
3. **Fast feedback** - Incremental search as you type (debounced)
4. **Multimodal** - Keyboard, click, AND voice refinement
5. **Escape hatches** - "Create new" or "Skip for now" options
6. **Batch awareness** - Handle multiple unresolved refs efficiently

---

## UI Component: `EntityRefModal`

### Trigger Conditions

Modal opens when:
- Semantic validator finds unresolved `EntityRef` nodes
- User clicks inline "?" marker on unresolved ref in DSL editor
- Voice command: "resolve John Smith" or "who is John Smith?"

### Modal Layout

```
┌─────────────────────────────────────────────────────────────┐
│  Resolve Entity Reference                              [X]  │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  DSL Context:                                               │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ entity:create                                        │   │
│  │   :name "John Smith"  ← unresolved                  │   │
│  │   :entity-type "natural-person"                     │   │
│  │   :jurisdiction "UK"                                │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  Search: [John Smith________________] 🔍  [🎤]             │
│                                                             │
│  ┌─ Refinements (optional) ─────────────────────────────┐  │
│  │ Nationality: [____]  DOB: [____]  Role: [____]       │  │
│  │                                                       │  │
│  │ 💡 "UK citizen, director at BlackRock"               │  │
│  │    → Parsed: nationality=GB, role contains BlackRock │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  Results (3 matches):                                       │
│  ┌───────────────────────────────────────────────────────┐ │
│  │ [Select] John Smith (98%)                             │ │
│  │          DOB: 1965-03-15 | UK | Director, BlackRock   │ │
│  ├───────────────────────────────────────────────────────┤ │
│  │ [Select] John Smith (72%)                             │ │
│  │          DOB: 1980-01-20 | US | Manager, Vanguard     │ │
│  ├───────────────────────────────────────────────────────┤ │
│  │ [Select] John A. Smith (65%)                          │ │
│  │          DOB: 1975-07-08 | UK | Analyst               │ │
│  └───────────────────────────────────────────────────────┘ │
│                                                             │
│  [+ Create New Entity]        [Skip]        [Cancel]        │
└─────────────────────────────────────────────────────────────┘
```

### Key Features

#### 1. DSL Context Display
- Show 3-5 lines around the unresolved reference
- Highlight the specific argument with "← unresolved"
- Shows what we already know (entity-type, jurisdiction from same statement)

#### 2. Search Box with Voice
- Auto-populated with the `value` from EntityRef
- Debounced search (300ms) as user types
- 🎤 button for voice input (or always-on listening mode)
- Minimum 2 characters to trigger search

#### 3. Natural Language Refinement
- Single text input for disambiguation hints
- Examples:
  - "UK citizen" → `nationality=GB`
  - "born 1965" → `dob=1965`
  - "director at BlackRock" → fuzzy match on role/company
  - "the one in London" → `jurisdiction=GB` (if ambiguous)
- Parser extracts structured discriminators from free text
- Shows parsed interpretation: "→ Parsed: nationality=GB, role contains BlackRock"

#### 4. Results List
- Score-colored (green >90%, yellow >70%, red <70%)
- Key discriminator fields visible (DOB, nationality, role/company)
- Single-click to select
- Keyboard navigation (↑↓ to move, Enter to select)

#### 5. Actions
- **Select** → Resolve EntityRef, update AST, close modal (or next unresolved)
- **Create New Entity** → Open entity creation flow, then use new UUID
- **Skip** → Mark as "review later", continue to next
- **Cancel** → Close modal, leave unresolved

---

## Voice Integration

### Voice-Triggered Resolution

When user says (in agent chat or via mic button):
- "Resolve John Smith" → Opens modal for that entity
- "Who is John Smith?" → Same as above
- "The UK director" → Adds refinement to current search

### Voice Refinement Flow

```
User: "John Smith"
System: [Shows 3 matches]

User: "The UK citizen"
System: [Filters to 2 matches, highlights nationality=UK]

User: "Director at BlackRock"
System: [Filters to 1 match, auto-selects if confidence >95%]

User: "Yes" or "Select"
System: [Confirms selection, resolves EntityRef]
```

### Auto-Resolve on High Confidence

If voice refinement narrows to single match with score >95%:
- Brief confirmation: "Selecting John Smith (DOB 1965-03-15, BlackRock)?"
- Voice "Yes" or 2-second timeout → auto-confirm
- Voice "No" → stay in modal

---

## State Machine

```
┌──────────────┐
│   Closed     │
└──────┬───────┘
       │ open(ref)
       ▼
┌──────────────┐
│  Searching   │◄────────────────┐
└──────┬───────┘                 │
       │ results                 │
       ▼                         │
┌──────────────┐    refine       │
│  Results     │─────────────────┘
└──────┬───────┘
       │ select / create / skip
       ▼
┌──────────────┐
│  Resolved    │──► next ref or close
└──────────────┘
```

---

## Data Structures

### UI State (ob-poc-ui)

```rust
pub struct EntityRefModalState {
    /// Is modal open?
    pub open: bool,
    
    /// The unresolved EntityRef being resolved
    pub current_ref: Option<UnresolvedRef>,
    
    /// Queue of remaining unresolved refs
    pub pending_refs: Vec<UnresolvedRef>,
    
    /// Search query (may differ from original value)
    pub query: String,
    
    /// Natural language refinement input
    pub refinement: String,
    
    /// Parsed discriminators from refinement
    pub discriminators: Discriminators,
    
    /// Search results from server
    pub results: Option<Vec<EntityMatch>>,
    
    /// Search in progress
    pub searching: bool,
    
    /// Voice listening active
    pub voice_active: bool,
}

pub struct UnresolvedRef {
    /// Statement index in AST
    pub stmt_index: usize,
    
    /// Argument name (e.g., "name")
    pub arg_name: String,
    
    /// Entity type from YAML lookup config
    pub entity_type: String,
    
    /// Original value (e.g., "John Smith")
    pub value: String,
    
    /// DSL context lines for display
    pub context_lines: Vec<String>,
    
    /// Line number of the unresolved ref
    pub line_number: usize,
}

pub struct Discriminators {
    pub nationality: Option<String>,
    pub date_of_birth: Option<String>,
    pub jurisdiction: Option<String>,
    pub role_hint: Option<String>,
    pub company_hint: Option<String>,
}
```

### Actions (returned from modal)

```rust
pub enum EntityRefAction {
    /// Search with current query + discriminators
    Search {
        query: String,
        discriminators: Discriminators,
    },
    
    /// User selected a match
    Select {
        ref_index: usize,
        resolved_key: String,
        display: String,
    },
    
    /// User wants to create new entity
    CreateNew {
        entity_type: String,
        initial_name: String,
    },
    
    /// Skip this ref, move to next
    Skip,
    
    /// Close modal entirely
    Close,
    
    /// Voice input received
    VoiceInput {
        transcript: String,
        confidence: f32,
    },
}
```

---

## API Integration

### Existing Endpoints (no changes needed)

1. **`GET /api/entity/search`** - Fuzzy search with discriminators
   ```
   GET /api/entity/search?type=entity&q=John+Smith&nationality=GB&dob=1965&limit=10
   ```

2. **`POST /api/session/:id/resolution/select`** - Update AST with resolved key
   ```json
   { "ref_index": 0, "resolved_key": "uuid-here" }
   ```

### New Endpoint: Parse Refinement

```
POST /api/entity/parse-refinement

Request:
{ "text": "UK citizen, born 1965, director at BlackRock" }

Response:
{
  "discriminators": {
    "nationality": "GB",
    "date_of_birth": "1965",
    "role_hint": "director",
    "company_hint": "BlackRock"
  },
  "interpretation": "nationality=GB, dob=1965, role contains 'director', company contains 'BlackRock'"
}
```

This could use simple pattern matching or LLM for complex cases.

---

## Implementation Phases

### Phase 1: Shared EntitySearch Component (3-4 hours)
- Extract search logic from `cbu_search_modal`
- Create `EntitySearchResults` widget (reusable)
- Parameterize by entity_type
- Wire to `/api/entity/search`
- Keyboard navigation (↑↓ Enter Esc)

### Phase 2: Inline Popup (4-5 hours)
- Squiggly underline rendering for unresolved EntityRefs in DSL editor
- Popup positioning (anchored below squiggly)
- Ctrl+. trigger / click trigger
- Compact layout (300px, 5-6 results)
- Select → update AST, close popup
- "2 more unresolved" indicator

### Phase 3: Batch Modal (4-5 hours)
- Full modal with DSL context display
- Stack-based workflow (current + pending queue)
- Progress indicator: "Resolving 3 of 7"
- Skip / Skip All / Cancel actions
- "Resolve All" toolbar button trigger

### Phase 4: Natural Language Refinement (4-5 hours)
- Refinement text input in modal
- Implement `/api/entity/parse-refinement`
- Show parsed interpretation
- Auto-apply discriminators to search

### Phase 5: Voice Integration (3-4 hours)
- Wire voice transcript to refinement input
- Add "listening" indicator in modal
- Auto-resolve on high confidence (>95%)
- Voice "select" / "yes" / "no" commands

### Phase 6: Polish (2-3 hours)
- Resolved indicator (green underline / checkmark)
- "Create new entity" flow
- Auto-resolve high confidence batch button
- Edge cases (no results, API errors)

---

## Design Decisions (Resolved)

1. **Inline vs Modal?** → **Both!**
   - Inline popup for single ref (Zed-style, Ctrl+.)
   - Batch modal for 3+ refs with full workflow

2. **When to trigger?**
   - Squiggly underline always visible on unresolved refs
   - Click or Ctrl+. → inline popup
   - Compile with 3+ unresolved → batch modal (sub-session)
   - "Resolve All" button → batch modal (sub-session)

3. **Agent-driven resolution (not forms)**
   - Resolution happens in scoped agent sub-session
   - User stays in conversational flow
   - Sub-session pops back to main chat on completion
   - See `docs/strategy-patterns.md` §3 Window Stack Architecture

---

## Sub-Session Architecture

Resolution uses the **window stack** pattern (see `docs/strategy-patterns.md`).

```
Main Agent Chat
    │
    │ Agent detects unresolved refs
    ▼
Agent: "I found 2 entities to confirm."
       [Open Resolution Assistant →]
    │
    │ Click button (pushes to window_stack)
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  Resolution Assistant (Layer 2 sub-session)    [← Back] [X]    │
│  ─────────────────────────────────────────────────────────────  │
│  Context: entity:link :parent "BlackRock UK" :child "John..."  │
│                                                                 │
│  Agent: Which BlackRock UK?                                     │
│         1. BlackRock Fund Managers (95%)                        │
│         2. BlackRock UK Holdings (78%)                          │
│                                                                 │
│  User: "The fund managers one" [voice or text]                  │
│                                                                 │
│  Agent: ✓ Selected. Now, which John Smith?                     │
│         Found 3 matches. Any details?                           │
│                                                                 │
│  User: "UK citizen, born 1965"                                  │
│                                                                 │
│  Agent: ✓ John Smith (DOB 1965, UK) selected.                  │
│         All resolved. Ready to execute?                         │
│                                                                 │
│  User: "Yes"                                                    │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
    │
    │ Completion (pops from window_stack)
    ▼
Main Agent Chat: [Resolution complete, DSL updated with UUIDs]
```

**Key properties:**
- Sub-session is a **scoped agent conversation**, not a form
- Has its own chat history (ephemeral)
- Voice works naturally ("the first one", "UK citizen")
- ESC or [← Back] closes and returns to main chat
- On completion, merges resolved refs back to main session AST

---

## Open Questions

1. **Entity creation flow?**
   - Separate modal? Inline expansion?
   - Pre-fill from discriminators?
   - Recommendation: Minimal inline form, expand to full modal if needed

2. **Voice always-on?**
   - Push-to-talk button only?
   - Wake word ("Hey OB")?
   - Recommendation: Button for now, always-on as opt-in setting

3. **Threshold for batch vs inline?**
   - Currently: 3+ unresolved → batch modal
   - Should user be able to force inline for batch? (resolve one-by-one)

---

## Success Criteria

1. [ ] User can resolve "John Smith" to correct person in <10 seconds
2. [ ] Voice refinement works without touching keyboard
3. [ ] 3+ unresolved refs can be batch-resolved efficiently
4. [ ] No false-positive auto-resolutions (confidence threshold respected)
5. [ ] "Create new" flow is seamless, not jarring
6. [ ] Mobile-friendly (if ever needed) - large touch targets

---

## References

- Existing modal pattern: `rust/crates/ob-poc-ui/src/panels/cbu_search.rs`
- Entity search API: `rust/src/api/entity_routes.rs`
- Resolution API: `rust/src/api/resolution_routes.rs`
- EntityGateway search: `rust/crates/entity-gateway/src/search_engine.rs`
- Voice bridge: `rust/crates/ob-poc-ui/src/voice_bridge.rs`
