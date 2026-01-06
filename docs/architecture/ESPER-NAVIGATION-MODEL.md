# Blade Runner Esper Navigation Model

## The Esper Pattern

From the film: Deckard navigates a photograph using voice commands:
- "Enhance 224 to 176" (zoom to coordinates)
- "Track right... stop" (pan and hold)
- "Enhance... give me a hard copy" (zoom further, then commit)

**Key characteristics:**
1. **Continuous navigation** - not discrete page jumps
2. **Soft focus** - preview/highlight without commitment
3. **Hard commit** - "dive in" / "enter" / "hard copy"
4. **Voice-driven** - natural language commands
5. **Reversible** - "pull back", "track left"
6. **Nested** - can keep enhancing deeper

---

## Current Model Gap

**What we have:** 4 discrete LOD levels with hard transitions

```
UNIVERSE ──click──→ CLUSTER ──click──→ CBU ──click──→ ENTITY
    ←──back────────    ←──back────────   ←──back────
```

**What Esper needs:** Continuous zoom with inline expansion

```
UNIVERSE
    │
    ├─ [soft] "enhance Luxembourg" → cluster preview overlay
    │       │
    │       ├─ [soft] "show CBUs" → inline CBU list expansion
    │       │
    │       └─ [hard] "dive in" → CLUSTER view (full transition)
    │
    └─ [soft] "inspect Allianz" → client preview overlay
            │
            └─ [hard] "enter" → CLIENT BOOK view
```

---

## Navigation Command Vocabulary

### Movement Commands (soft - reversible)

| Command | Action | Scope |
|---------|--------|-------|
| `enhance <target>` | Soft focus + expand preview | Any node |
| `inspect <target>` | Show detail overlay without navigation | Any node |
| `track to <target>` | Pan/highlight without zoom | Same level |
| `pull back` | Collapse current expansion | Any |
| `track left/right/up/down` | Move selection | Same level |
| `stop` | Hold current position | Any |
| `show <aspect>` | Expand specific sub-element | Focused node |

### Commitment Commands (hard - navigation)

| Command | Action | Result |
|---------|--------|--------|
| `dive in` / `enter` | Navigate into focused element | Level change |
| `surface` / `exit` | Navigate out one level | Level change |
| `jump to <target>` | Direct navigation | Level change |
| `hard copy` | Snapshot/export current view | - |

### Query Commands (no navigation)

| Command | Action | Result |
|---------|--------|--------|
| `show ownership` | Expand ownership sub-tree inline | Inline expansion |
| `show documents` | Expand document list inline | Inline expansion |
| `show history` | Expand temporal view | Inline expansion |
| `show connections` | Highlight related nodes | Visual only |
| `as of <date>` | Temporal filter | Re-render |

---

## Inline Expansion Model

### Node Expansion States

```rust
pub enum NodeExpansionState {
    /// Collapsed - shows summary only
    Collapsed,
    
    /// Soft focus - highlighted, shows preview overlay
    SoftFocus {
        preview_data: Option<PreviewData>,
    },
    
    /// Expanded - shows inline children
    Expanded {
        children: Vec<ExpandedChild>,
        expansion_type: ExpansionType,
    },
    
    /// Deep expanded - multiple levels inline
    DeepExpanded {
        depth: usize,
        expansions: Vec<ExpansionType>,
    },
}

pub enum ExpansionType {
    Ownership,      // Show ownership chain inline
    Roles,          // Show roles in CBU
    Documents,      // Show document list
    Appearances,    // Show CBU appearances (for entity)
    Children,       // Generic children (for cluster → CBUs)
    TradingProfile, // Show trading matrix inline
    History,        // Show temporal changes
}
```

### Expansion by Level

#### Universe Level Expansions

| Node | Expandable Aspects | Inline Content |
|------|-------------------|----------------|
| `JurisdictionCluster` | CBUs | Top N CBU cards inside cluster orb |
| `JurisdictionCluster` | Risk breakdown | Risk distribution pie/bar |
| `JurisdictionCluster` | Clients | Client logos/names in cluster |
| `ClientCluster` | CBUs | CBU cards grouped by jurisdiction |
| `ClientCluster` | Shared entities | Entity nodes with connection lines |

**Visual:** Cluster orb expands to show content, other clusters fade/shrink

```
Before:                          After "enhance Luxembourg":
                                 
   ○ LU    ○ IE                    ┌─────────────────────┐
                                   │     LUXEMBOURG      │
      ○ DE                         │  ┌────┐ ┌────┐     │    ○ IE (faded)
                                   │  │FundA│ │FundB│    │
                                   │  └────┘ └────┘     │
                                   │  ┌────┐ +174 more  │    ○ DE (faded)
                                   │  │FundC│           │
                                   └─────────────────────┘
```

#### Cluster Detail Level Expansions

| Node | Expandable Aspects | Inline Content |
|------|-------------------|----------------|
| `CbuCard` | Entities | Mini entity badges inside card |
| `CbuCard` | Roles | Role tags with entity names |
| `CbuCard` | Risk factors | Risk indicator breakdown |
| `SharedEntityMarker` | CBU connections | Highlight all connected CBU cards |
| `SharedEntityMarker` | Entity detail | Entity preview overlay |

**Visual:** CBU card expands vertically to show content

```
Before:                          After "inspect Fund A":

┌─────────────┐                  ┌─────────────────────────────┐
│ Fund A  [●] │                  │ Fund A                  [●] │
│ UCITS  LU   │                  │ UCITS  LU                   │
│ 23 entities │                  │─────────────────────────────│
└─────────────┘                  │ Roles:                      │
                                 │  ManCo: Allianz GI GmbH     │
                                 │  IM: PIMCO Europe           │
                                 │  Custodian: BNY Mellon      │
                                 │─────────────────────────────│
                                 │ UBOs: J.Smith (25%), ...    │
                                 │─────────────────────────────│
                                 │ [Dive In →]                 │
                                 └─────────────────────────────┘
```

#### CBU Graph Level Expansions

| Node | Expandable Aspects | Inline Content |
|------|-------------------|----------------|
| `LegalEntity` | Ownership chain | Upstream/downstream owners inline |
| `LegalEntity` | Roles | All roles this entity plays |
| `LegalEntity` | Documents | Document icons with status |
| `LegalEntity` | Other CBUs | Mini badges for other appearances |
| `NaturalPerson` | Positions | All directorships/roles |
| `NaturalPerson` | PEP/Sanctions | Screening results |
| `TradingProfile` | Full matrix | Instrument classes + markets |
| `InstrumentClass` | Markets | Expanded market list |
| Edge (any) | History | Temporal changes to relationship |

**Visual:** Node expands with sub-tree, edges re-route

```
Before:                          After "show ownership" on ManCo:

    ┌───────┐                        ┌───────────────────────┐
    │ ManCo │                        │ ManCo                 │
    └───┬───┘                        │─────────────────────  │
        │                            │ ↑ Owned by:           │
    ┌───┴───┐                        │   Allianz SE (100%)   │
    │ Fund  │                        │     └─ Ultimate       │
    └───────┘                        │─────────────────────  │
                                     │ Roles in this CBU:    │
                                     │   MANAGEMENT_COMPANY  │
                                     └───────────┬───────────┘
                                                 │
                                             ┌───┴───┐
                                             │ Fund  │
                                             └───────┘
```

#### Entity Detail Level Expansions

| Node | Expandable Aspects | Inline Content |
|------|-------------------|----------------|
| `EntityHeader` | Full attributes | All entity fields |
| `CbuAppearance` | Role details | Effective dates, authorities |
| `OwnershipLink` | Chain continuation | Next level of owners |
| `DocumentLink` | Document preview | Thumbnail/summary |
| `DocumentLink` | Verification status | Who verified, when |

---

## Focus Model

### Focus Stack (not Navigation Stack)

The focus stack tracks **soft focus** within the current view level:

```rust
pub struct FocusStack {
    /// Current view level (hard navigation)
    level: ViewLevel,
    
    /// Stack of focused elements within this level
    focus_path: Vec<FocusFrame>,
    
    /// Currently expanded nodes
    expansions: HashMap<String, NodeExpansionState>,
}

pub struct FocusFrame {
    /// Node ID that has focus
    node_id: String,
    
    /// What aspect is expanded
    expansion: Option<ExpansionType>,
    
    /// Child focus (for nested expansion)
    child_focus: Option<Box<FocusFrame>>,
}
```

### Focus vs Navigation

| Action | Focus Stack | Navigation Stack | View Change |
|--------|-------------|------------------|-------------|
| `enhance X` | Push focus on X | No change | Expand inline |
| `pull back` | Pop focus | No change | Collapse inline |
| `dive in` | Clear focus | Push new level | Full transition |
| `surface` | Clear focus | Pop level | Full transition |
| `track to Y` | Replace focus | No change | Move selection |

---

## Server Support Required

### Preview Endpoint

```
GET /api/node/:id/preview?type=<expansion_type>
```

Returns lightweight preview data for inline expansion without full navigation.

**Response:**
```json
{
  "node_id": "cluster:LU",
  "preview_type": "children",
  "preview_data": {
    "total_count": 177,
    "preview_items": [
      { "id": "cbu:fund-a", "name": "Fund A", "risk": "LOW" },
      { "id": "cbu:fund-b", "name": "Fund B", "risk": "MEDIUM" },
      { "id": "cbu:fund-c", "name": "Fund C", "risk": "LOW" }
    ],
    "has_more": true
  }
}
```

### Expansion Endpoints by Type

| Expansion Type | Endpoint | Returns |
|---------------|----------|---------|
| `ownership` | `GET /api/entity/:id/ownership-chain` | Upstream + downstream |
| `roles` | `GET /api/entity/:id/roles` | All role assignments |
| `documents` | `GET /api/entity/:id/documents` | Document list |
| `appearances` | `GET /api/entity/:id/cbu-appearances` | CBU list with roles |
| `children` | `GET /api/cluster/:id/preview?limit=5` | Top N children |
| `history` | `GET /api/node/:id/history?from=&to=` | Temporal changes |

---

## Voice Command Mapping

### DSL Verbs for Esper Navigation

```lisp
;; Soft focus commands
(esper.enhance :target "Luxembourg")
(esper.inspect :target @fund-a)
(esper.track :direction :right)
(esper.track :to @manco)
(esper.pull-back)
(esper.stop)

;; Expansion commands
(esper.show :aspect :ownership)
(esper.show :aspect :documents)
(esper.show :aspect :roles)
(esper.show :aspect :history :from "2024-01-01")

;; Hard navigation commands
(esper.dive-in)
(esper.surface)
(esper.jump :to @entity-uuid)

;; Query/filter commands
(esper.as-of :date "2024-06-30")
(esper.filter :risk :high)
(esper.highlight :connections)

;; Output commands
(esper.hard-copy)  ;; Export/screenshot
(esper.narrate)    ;; Describe current view
```

### Natural Language → DSL

| Voice Input | DSL |
|-------------|-----|
| "Enhance Luxembourg" | `(esper.enhance :target "Luxembourg")` |
| "Show me the CBUs" | `(esper.show :aspect :children)` |
| "Track right" | `(esper.track :direction :right)` |
| "Inspect the ManCo" | `(esper.inspect :target @manco)` |
| "Show ownership chain" | `(esper.show :aspect :ownership)` |
| "Pull back" | `(esper.pull-back)` |
| "Dive in" | `(esper.dive-in)` |
| "As of last December" | `(esper.as-of :date "2024-12-31")` |
| "Give me a hard copy" | `(esper.hard-copy)` |

---

## Rendering Implications

### Transition Types

```rust
pub enum TransitionType {
    /// Hard navigation between levels
    LevelChange {
        from_level: ViewLevel,
        to_level: ViewLevel,
        animation: LevelTransitionAnimation,
    },
    
    /// Soft focus within level
    Focus {
        node_id: String,
        animation: FocusAnimation,
    },
    
    /// Inline expansion
    Expand {
        node_id: String,
        expansion_type: ExpansionType,
        animation: ExpandAnimation,
    },
    
    /// Collapse expansion
    Collapse {
        node_id: String,
        animation: CollapseAnimation,
    },
}

pub enum LevelTransitionAnimation {
    ZoomIn,      // Camera flies into focused element
    ZoomOut,     // Camera pulls back
    CrossFade,   // Dissolve between views
}

pub enum FocusAnimation {
    Highlight,   // Glow/border on focused node
    Spotlight,   // Dim everything else
    Track,       // Camera pans to center on node
}

pub enum ExpandAnimation {
    Unfold,      // Node grows to show children
    Reveal,      // Children fade in below node
    Tree,        // Branch lines animate out
}
```

### Camera Behavior

| Command | Camera Action |
|---------|--------------|
| `enhance` | Zoom toward target, center on it |
| `track to` | Pan to center on target (no zoom) |
| `pull back` | Zoom out slightly, widen view |
| `dive in` | Aggressive zoom into target, then transition |
| `surface` | Pull back to parent level bounds |

---

## Implementation Priority

### Phase 1: Focus Model (Client)

1. Add `FocusStack` to `AppState`
2. Implement `enhance` / `pull-back` for cluster level
3. Add highlight/dim rendering for soft focus
4. Wire to existing `GalaxyAction`

### Phase 2: Inline Expansion (Server + Client)

1. Add `/api/node/:id/preview` endpoint
2. Implement `NodeExpansionState` rendering
3. Add `show :aspect` handlers for ownership, roles, documents
4. Animate expansion/collapse

### Phase 3: Voice Commands

1. Map voice input to DSL verbs
2. Add `esper.*` verb family to DSL parser
3. Wire to focus/expansion actions

### Phase 4: Temporal Navigation

1. Add `as_of` parameter to all graph endpoints
2. Implement history timeline UI
3. Add temporal scrubbing controls

---

## Example Sequence

**Scenario:** Find who controls Allianz Luxembourg funds

```
[Universe View]
Voice: "Enhance Luxembourg"
→ LU cluster expands, other clusters fade
→ Preview shows top 5 CBUs

Voice: "Show Allianz funds"
→ Filter to Allianz commercial client
→ 35 CBU cards highlighted

Voice: "Track to Fund Alpha"
→ Camera pans to Fund Alpha card
→ Card highlighted with glow

Voice: "Inspect"
→ Card expands showing:
   - ManCo: Allianz GI GmbH
   - IM: PIMCO Europe
   - Directors: J.Smith, M.Jones
   - UBOs: Person A (25%), Person B (15%)

Voice: "Show ownership chain for the ManCo"
→ Ownership sub-tree expands inline:
   Allianz GI GmbH
     ↑ 100% owned by
   Allianz Asset Mgmt
     ↑ 100% owned by
   Allianz SE (Ultimate)

Voice: "Dive in to Allianz SE"
→ Hard transition to Entity Detail view for Allianz SE
→ Shows all 47 CBU appearances
→ Shows full ownership structure

Voice: "Hard copy"
→ Export/screenshot current view
```

---

## Summary: What's New

| Concept | Old Model | Esper Model |
|---------|-----------|-------------|
| Navigation | Hard transitions only | Soft focus + hard dive |
| Expansion | None | Inline expansion per node |
| Commands | Click/double-click | Voice-driven vocabulary |
| Focus | Selection only | Focus stack with preview |
| Temporal | Single point-in-time | Scrubbing + "as of" |
| Camera | Fit-to-bounds | Cinematic zoom/pan/track |

The taxonomy doc needs to be extended with:
1. `NodeExpansionState` definitions
2. Preview data structures
3. Expansion endpoints
4. Voice command → DSL mapping
5. Camera/animation specs
