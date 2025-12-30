# UBO Tree Navigation & Ontology - Options Paper

**Date:** 2025-12-30  
**Context:** CBU as atomic container, UBO as multi-level hierarchy  
**Goal:** Agent-navigable ownership/control tree with prong separation

---

## Current State Summary

### What You Have

| Component | Status | Notes |
|-----------|--------|-------|
| `ownership_relationships` | ✅ | `owner_entity_id` → `owned_entity_id` with percentage |
| `entity_relationships` | ✅ | Generic edges: ownership, control, trust_role, employment, management |
| `ubo_registry` | ✅ | Calculated UBO determinations per CBU with verification workflow |
| `roles` table | ✅ | 60+ roles across 9 categories (OWNERSHIP_CHAIN, CONTROL_CHAIN, etc.) |
| `cbu_entity_roles` | ✅ | Junction table linking CBU ↔ Entity ↔ Role |
| Role taxonomy V2 | ✅ | Categories, UBO treatment, KYC obligation, layout hints |

### What's Missing

1. **Tree Traversal API** - No nom-based grammar for navigation commands
2. **Focus Context** - No "current node" cursor for agent navigation
3. **Prong Separation** - Ownership vs Control visualized but not navigable separately
4. **Book-Level View** - No "show all Allianz" aggregation across CBUs
5. **Person Finder** - No "where is Hans a director?" cross-CBU query

---

## The Conceptual Model

### CBU vs UBO Relationship

```
┌─────────────────────────────────────────────────────────────────────┐
│                           BOOK LEVEL                                 │
│  "Allianz" = Collection of CBUs sharing ownership/control graph     │
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │                      UBO FOREST                                 ││
│  │                                                                 ││
│  │    Allianz SE (DE) ─── UBO TERMINUS (publicly traded)          ││
│  │         │                                                       ││
│  │         ├── 100% ──▶ Allianz AM GmbH (DE)                      ││
│  │         │                 │                                     ││
│  │         │                 └── 100% ──▶ Allianz GI GmbH (DE)    ││
│  │         │                                  │                    ││
│  │         │                    ┌─────────────┼─────────────┐      ││
│  │         │                    │             │             │      ││
│  │         │              [manages]      [manages]     [manages]   ││
│  │         │                    │             │             │      ││
│  │         │                    ▼             ▼             ▼      ││
│  └─────────│────────────────────────────────────────────────────────┘│
│            │                                                         │
│  ┌─────────▼───────────────┐  ┌────────────────┐  ┌────────────────┐│
│  │        CBU #1           │  │     CBU #2     │  │     CBU #3     ││
│  │  ┌──────────────────┐  │  │                │  │                ││
│  │  │ SICAV Umbrella   │  │  │   UK OEIC      │  │   DE FCP       ││
│  │  │  ├── SubFund A   │  │  │                │  │                ││
│  │  │  ├── SubFund B   │  │  └────────────────┘  └────────────────┘│
│  │  │  └── SubFund C   │  │                                        │
│  │  └──────────────────┘  │                                        │
│  │  + Services            │                                        │
│  │  + Trading entities    │                                        │
│  └────────────────────────┘                                        │
└─────────────────────────────────────────────────────────────────────┘
```

### Key Insight

- **CBU** = Atomic trading container (one fund strategy, one onboarding)
- **UBO Forest** = Shared ownership/control graph that CBUs reference into
- **Book** = Commercial grouping of CBUs under common ownership apex

---

## Option 1: Nom-Based Tree Navigation Grammar

### Approach

Add a new Nom parser for navigation commands, separate from DSL execution.

```rust
// Navigation AST
pub enum NavCommand {
    // Book-level
    ShowBook { client_name: String },
    FocusJurisdiction { code: String },
    FocusManCo { name: String },
    
    // Tree traversal
    GoUp,                           // To parent owner
    GoDown { child_index: usize },  // To child owned entity
    GoSibling { direction: i32 },   // Left/right among siblings
    GoRoot,                         // To UBO terminus
    GoCommercialClient,             // To CBU anchor entity
    
    // Prong focus
    FocusOwnership,                 // Show ownership edges only
    FocusControl,                   // Show control edges only
    FocusBoth,                      // Default combined view
    
    // Entity search
    FindPerson { name: String },
    FindRole { role: String, person: Option<String> },
    WhereIs { entity_name: String },
    
    // Display
    ShowTree { depth: usize },
    ShowPath,                       // Current cursor to root
    ShowContext,                    // Current node details
    Zoom { level: f32 },
}
```

### Nom Parser

```rust
use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_while1},
    character::complete::{space0, space1, digit1},
    combinator::{map, opt, value},
    sequence::{preceded, tuple},
    IResult,
};

fn parse_nav_command(input: &str) -> IResult<&str, NavCommand> {
    alt((
        parse_show_book,
        parse_focus_jurisdiction,
        parse_focus_manco,
        parse_go_up,
        parse_go_down,
        parse_find_person,
        parse_where_is,
        parse_focus_prong,
    ))(input)
}

fn parse_show_book(input: &str) -> IResult<&str, NavCommand> {
    let (input, _) = alt((
        tag_no_case("show book"),
        tag_no_case("show me the"),
        tag_no_case("show"),
    ))(input)?;
    let (input, _) = space1(input)?;
    let (input, name) = parse_identifier(input)?;
    let (input, _) = opt(tag_no_case(" book"))(input)?;
    Ok((input, NavCommand::ShowBook { client_name: name.to_string() }))
}

fn parse_focus_jurisdiction(input: &str) -> IResult<&str, NavCommand> {
    let (input, _) = tag_no_case("focus on")(input)?;
    let (input, _) = space1(input)?;
    let (input, code) = alt((
        tag_no_case("lux"),
        tag_no_case("luxembourg"),
        tag_no_case("lu"),
        tag_no_case("ireland"),
        tag_no_case("ie"),
        tag_no_case("uk"),
        tag_no_case("gb"),
        tag_no_case("germany"),
        tag_no_case("de"),
    ))(input)?;
    let normalized = match code.to_lowercase().as_str() {
        "lux" | "luxembourg" | "lu" => "LU",
        "ireland" | "ie" => "IE",
        "uk" | "gb" => "GB",
        "germany" | "de" => "DE",
        _ => code,
    };
    Ok((input, NavCommand::FocusJurisdiction { code: normalized.to_string() }))
}

fn parse_where_is(input: &str) -> IResult<&str, NavCommand> {
    let (input, _) = alt((
        tag_no_case("where is"),
        tag_no_case("find"),
        tag_no_case("show me where"),
    ))(input)?;
    let (input, _) = space1(input)?;
    let (input, name) = parse_quoted_or_identifier(input)?;
    let (input, _) = opt(tuple((space1, tag_no_case("director"))))(input)?;
    Ok((input, NavCommand::WhereIs { entity_name: name.to_string() }))
}
```

### Pros/Cons

| Pros | Cons |
|------|------|
| Natural language-ish commands | Another parser to maintain |
| Type-safe AST | Learning curve for navigation syntax |
| Composable with DSL engine | Needs state management (cursor) |
| Can generate visualization updates | |

---

## Option 2: Graph Query Language Extension

### Approach

Extend existing DSL with graph traversal verbs.

```lisp
;; Book-level queries
(ubo.show-book :client "Allianz")
(ubo.filter-jurisdiction :code "LU")
(ubo.filter-manco :name "Allianz Global Investors GmbH")

;; Traversal from current focus
(ubo.go-up)                                 ; -> parent owner
(ubo.go-down :child 0)                      ; -> first child
(ubo.go-to :entity @allianz_gi)             ; -> specific entity

;; Prong queries
(ubo.show-ownership-chain :from @fund :depth 5)
(ubo.show-control-chain :from @operating_co)
(ubo.show-combined :from @fund)

;; Person/role queries
(ubo.find-roles :person "Hans Mueller")     ; -> all Hans's roles across book
(ubo.find-directors :entity @allianz_gi)    ; -> all directors of entity
(ubo.find-by-role :role "CIO" :scope :book) ; -> all CIOs in book

;; Context
(ubo.show-path)                             ; -> current entity to UBO terminus
(ubo.show-context)                          ; -> current entity details + roles
```

### Implementation

Add to `rust/config/verbs/ubo-nav.yaml`:

```yaml
domain: ubo
verbs:
  show-book:
    description: Display all CBUs under a commercial client's ownership
    behavior: query
    query:
      type: graph_aggregate
      aggregation: client_book
    args:
      - name: client
        type: string
        required: true
        description: Commercial client name or identifier
    returns:
      - cbu_ids: array[uuid]
      - entity_graph: object
      - ownership_tree: object
      - control_overlay: object

  go-up:
    description: Navigate to parent owner entity
    behavior: plugin
    plugin: ubo_nav
    args: []
    context:
      requires_focus: true
    returns:
      - entity: object
      - ownership_edge: object
      - depth_change: integer

  show-ownership-chain:
    description: Display ownership chain from entity to UBO terminus
    behavior: query
    query:
      type: recursive_cte
      base_table: entity_relationships
      filter: relationship_type = 'ownership'
    args:
      - name: from
        type: entity_ref
        required: true
      - name: depth
        type: integer
        required: false
        default: 10
    returns:
      - chain: array[entity]
      - total_percentage: number
      - terminus_entity: object
      - terminus_reason: string

  find-roles:
    description: Find all roles held by a person across the book
    behavior: query
    query:
      type: cross_cbu_search
      join:
        - table: cbu_entity_roles
        - table: entities
        - table: roles
    args:
      - name: person
        type: string
        required: true
      - name: scope
        type: string
        valid_values: [cbu, book, global]
        default: book
    returns:
      - roles: array[object]
      - cbus: array[uuid]
      - entities: array[uuid]
```

### Pros/Cons

| Pros | Cons |
|------|------|
| Reuses existing DSL infrastructure | Heavier syntax for simple navigation |
| No new parser needed | Less natural for conversational UI |
| Full power of verb system | Requires session state for focus |
| Recordable/replayable | |

---

## Option 3: Hybrid - Navigation State Machine + DSL

### Approach

Separate navigation state from execution, use both.

```rust
/// Navigation context - maintained by agent
pub struct UboNavigator {
    /// Current focus entity (if any)
    focus: Option<Uuid>,
    
    /// Current prong filter
    prong: ProngFilter,
    
    /// Current book scope
    book_scope: Option<BookScope>,
    
    /// Navigation history for back/forward
    history: Vec<NavigationFrame>,
    
    /// Cached graph for current scope
    cached_graph: Option<UboGraph>,
}

pub enum ProngFilter {
    Both,
    OwnershipOnly,
    ControlOnly,
}

pub struct BookScope {
    /// Root entity (e.g., Allianz SE)
    apex_entity_id: Uuid,
    /// Jurisdictions in scope
    jurisdictions: Option<Vec<String>>,
    /// ManCos in scope
    mancos: Option<Vec<Uuid>>,
}

impl UboNavigator {
    /// Process natural language navigation command
    pub fn process_command(&mut self, cmd: &str, db: &impl UboRepository) -> NavResult {
        // Use lightweight nom parser for command recognition
        match parse_nav_command(cmd) {
            Ok((_, NavCommand::ShowBook { client_name })) => {
                self.load_book(&client_name, db)
            }
            Ok((_, NavCommand::GoUp)) => {
                self.navigate_to_parent(db)
            }
            Ok((_, NavCommand::FocusOwnership)) => {
                self.prong = ProngFilter::OwnershipOnly;
                self.refresh_view(db)
            }
            Ok((_, NavCommand::FindPerson { name })) => {
                self.search_person(&name, db)
            }
            // ... etc
            Err(_) => {
                // Fall back to DSL execution for complex queries
                NavResult::FallbackToDsl(cmd.to_string())
            }
        }
    }
}
```

### Agent Conversation Flow

```
User: show me the whole Allianz book
Agent: [calls ubo.show-book :client "Allianz"]
       Loaded 47 CBUs under Allianz ownership. Apex: Allianz SE (DE).
       Showing ownership pyramid with 3 ManCos across LU, IE, DE.

User: focus on Lux
Agent: [updates prong filter, re-renders]
       Filtered to Luxembourg. 23 CBUs, 1 ManCo (Allianz GI GmbH - LU Branch).
       205 sub-funds under 2 SICAV umbrellas.

User: focus on the AI fund
Agent: [searches for "AI" in fund names]
       Found: Allianz Global Artificial Intelligence
       ├── Umbrella: Allianz Global Investors Fund (SICAV)
       ├── ManCo: Allianz Global Investors GmbH
       └── 8 share classes (A-EUR, IT-USD, WT-USD, ...)

User: show me the control prong
Agent: [switches prong = ControlOnly]
       Directors of Allianz GI GmbH:
       ├── Hans Mueller (Chairman, since 2019)
       ├── Maria Schmidt (Director, since 2021)
       └── Klaus Weber (Director, since 2020)
       
       Board of SICAV:
       └── Independent Directors: 3

User: where is Hans a director?
Agent: [cross-CBU search]
       Hans Mueller holds DIRECTOR role in:
       ├── Allianz Global Investors GmbH (DE) - Chairman
       ├── Allianz Global Investors Fund SICAV (LU) - Director
       └── AllianzGI Managed Accounts (UK) - Non-Exec Director

User: go up from the ManCo
Agent: [navigates ownership chain]
       Allianz GI GmbH (DE)
       └── 100% owned by: Allianz SE (DE) [UBO TERMINUS - publicly traded]
```

### Pros/Cons

| Pros | Cons |
|------|------|
| Natural conversational flow | More complex implementation |
| State maintained for context | Two systems to coordinate |
| Fallback to full DSL power | Session management needed |
| Best agent UX | |

---

## Option 4: Database-Centric with Materialized Views

### Approach

Push tree navigation logic into PostgreSQL with CTEs and materialized views.

```sql
-- Materialized view: Full ownership forest
CREATE MATERIALIZED VIEW mv_ownership_forest AS
WITH RECURSIVE ownership_tree AS (
    -- Start from UBO termini (no parent with ownership relationship)
    SELECT 
        e.entity_id,
        e.name as entity_name,
        e.entity_type,
        e.jurisdiction,
        NULL::uuid as parent_entity_id,
        NULL::numeric as ownership_pct,
        0 as depth,
        ARRAY[e.entity_id] as path,
        e.entity_id as root_entity_id
    FROM "ob-poc".entities e
    WHERE NOT EXISTS (
        SELECT 1 FROM "ob-poc".entity_relationships er
        WHERE er.from_entity_id = e.entity_id
        AND er.relationship_type = 'ownership'
        AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
    )
    AND EXISTS (
        SELECT 1 FROM "ob-poc".entity_relationships er2
        WHERE er2.to_entity_id = e.entity_id
        AND er2.relationship_type = 'ownership'
    )
    
    UNION ALL
    
    -- Recurse down ownership chain
    SELECT
        child.entity_id,
        child.name,
        child.entity_type,
        child.jurisdiction,
        parent.entity_id as parent_entity_id,
        er.percentage as ownership_pct,
        parent.depth + 1,
        parent.path || child.entity_id,
        parent.root_entity_id
    FROM ownership_tree parent
    JOIN "ob-poc".entity_relationships er 
        ON er.from_entity_id = parent.entity_id
        AND er.relationship_type = 'ownership'
        AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
    JOIN "ob-poc".entities child 
        ON child.entity_id = er.to_entity_id
    WHERE child.entity_id != ALL(parent.path)  -- Prevent cycles
    AND parent.depth < 20  -- Safety limit
)
SELECT * FROM ownership_tree;

-- Function: Navigate to parent
CREATE OR REPLACE FUNCTION ubo_go_up(p_entity_id uuid)
RETURNS TABLE (
    entity_id uuid,
    entity_name text,
    ownership_pct numeric,
    depth int
) AS $$
    SELECT 
        ot.parent_entity_id,
        pe.name,
        ot.ownership_pct,
        ot.depth - 1
    FROM mv_ownership_forest ot
    JOIN "ob-poc".entities pe ON pe.entity_id = ot.parent_entity_id
    WHERE ot.entity_id = p_entity_id
    LIMIT 1;
$$ LANGUAGE sql;

-- Function: Show ownership chain to apex
CREATE OR REPLACE FUNCTION ubo_show_chain(p_entity_id uuid)
RETURNS TABLE (
    entity_id uuid,
    entity_name text,
    entity_type text,
    jurisdiction varchar,
    ownership_pct numeric,
    depth int,
    is_terminus boolean
) AS $$
    WITH RECURSIVE chain AS (
        SELECT 
            entity_id, entity_name, entity_type, jurisdiction,
            ownership_pct, depth, parent_entity_id
        FROM mv_ownership_forest
        WHERE entity_id = p_entity_id
        
        UNION ALL
        
        SELECT 
            parent.entity_id, parent.entity_name, parent.entity_type,
            parent.jurisdiction, parent.ownership_pct, parent.depth,
            parent.parent_entity_id
        FROM mv_ownership_forest parent
        JOIN chain ON chain.parent_entity_id = parent.entity_id
    )
    SELECT 
        entity_id, entity_name, entity_type, jurisdiction,
        ownership_pct, depth,
        parent_entity_id IS NULL as is_terminus
    FROM chain
    ORDER BY depth ASC;
$$ LANGUAGE sql;

-- Function: Find person's roles across book
CREATE OR REPLACE FUNCTION ubo_find_person_roles(p_person_name text)
RETURNS TABLE (
    entity_id uuid,
    entity_name text,
    role_name text,
    role_category text,
    cbu_id uuid,
    cbu_name text
) AS $$
    SELECT 
        e.entity_id,
        e.name as entity_name,
        r.name as role_name,
        r.role_category,
        c.cbu_id,
        c.name as cbu_name
    FROM "ob-poc".entities e
    JOIN "ob-poc".cbu_entity_roles cer ON cer.entity_id = e.entity_id
    JOIN "ob-poc".roles r ON r.role_id = cer.role_id
    JOIN "ob-poc".cbus c ON c.cbu_id = cer.cbu_id
    WHERE e.name ILIKE '%' || p_person_name || '%'
    AND e.entity_type = 'proper_person'
    ORDER BY c.name, r.role_category, r.name;
$$ LANGUAGE sql;
```

### Pros/Cons

| Pros | Cons |
|------|------|
| Fast queries via materialized views | View refresh latency |
| SQL-native, no Rust parsing | Less flexible for ad-hoc navigation |
| Easy to extend with new functions | Agent needs to call functions |
| Works with any client | Can't maintain cursor state |

---

## Recommendation

**Go with Option 3: Hybrid Navigation State Machine + DSL**

### Rationale

1. **Agent UX is king** - You want conversational navigation ("focus on Lux", "where is Hans")
2. **CBU/UBO are different beasts** - CBU is atomic, UBO is a forest; treating them separately is correct
3. **Nom is already in your stack** - Adding a lightweight navigation grammar is trivial
4. **Fallback to DSL** - Complex queries still work via existing verb system
5. **State is necessary** - "go up" implies current position; you need a cursor

### Implementation Phases

| Phase | Deliverable | Effort |
|-------|-------------|--------|
| 1 | `UboNavigator` struct + state management | 2 days |
| 2 | Nom parser for 10 core navigation commands | 2 days |
| 3 | Database functions for tree traversal | 1 day |
| 4 | Agent integration (process_command → viz update) | 2 days |
| 5 | Prong separation (ownership vs control views) | 1 day |
| 6 | Book-level aggregation | 1 day |
| 7 | Person/role finder across CBUs | 1 day |

### Schema Additions Needed

```sql
-- Book-level grouping (optional - could be derived from ownership apex)
CREATE TABLE IF NOT EXISTS "ob-poc".commercial_books (
    book_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    book_name varchar(255) NOT NULL,
    apex_entity_id uuid REFERENCES "ob-poc".entities(entity_id),
    jurisdiction varchar(10),
    created_at timestamptz DEFAULT now()
);

-- Link CBUs to books (derived from ownership chain to apex)
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_book_membership (
    cbu_id uuid REFERENCES "ob-poc".cbus(cbu_id),
    book_id uuid REFERENCES "ob-poc".commercial_books(book_id),
    derived_at timestamptz DEFAULT now(),
    ownership_path uuid[],  -- Entity IDs from CBU anchor to apex
    PRIMARY KEY (cbu_id, book_id)
);
```

---

## Agent Command Reference (Target)

| Command | Example | Action |
|---------|---------|--------|
| Show book | "show me the Allianz book" | Load all CBUs under Allianz SE |
| Focus jurisdiction | "focus on Lux" | Filter to LU entities |
| Focus ManCo | "focus on the Lux ManCo" | Filter to AllianzGI-managed CBUs |
| Focus entity | "focus on the AI fund" | Set cursor to entity |
| Go up | "go up" / "parent" | Navigate to owner |
| Go down | "go down to subfund A" | Navigate to owned child |
| Show ownership | "show ownership prong" | Filter to ownership edges |
| Show control | "show control prong" | Filter to control edges |
| Where is X | "where is Hans a director" | Cross-CBU role search |
| Show path | "show path to UBO" | Display chain to terminus |
| Show context | "show me this entity" | Display current focus details |

---

## Questions for You

1. **Book derivation**: Should books be explicitly created, or derived from ownership apex?
2. **Cross-CBU entities**: An entity (e.g., Hans) in multiple CBUs - single or multiple nodes in viz?
3. **Temporal navigation**: Should "go up" respect effective dates, or always show current?
4. **Control overlay**: Directors on the same node as the company, or offset like V2 spec?
5. **Save/load views**: Should filtered views be persistable per user?

Let me know which direction you want to take, and I'll start on the implementation.
