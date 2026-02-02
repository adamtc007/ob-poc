# Architecture Proposal: Entity Data Lineage & Semantic Scope Resolution

## Document Metadata

| Field | Value |
|-------|-------|
| **Status** | **FINAL - Codebase-Validated Implementation Spec (v5)** |
| **Author** | Adam / Claude |
| **Created** | 2026-02-02 |
| **Updated** | 2026-02-02 |
| **Domain** | ob-poc (Enterprise Onboarding Platform) |
| **Review Target** | Claude Code Implementation |

---

## Version History & Change Control

| Version | Date | Changes |
|---------|------|---------|
| v1 | 2026-02-02 | Initial proposal: entity lineage + semantic scope separation |
| v2 | 2026-02-02 | GPT peer review #1: Pattern A compile-time expansion, typed attribute values, split role_tags, promotion rules, learning integration |
| v3 | 2026-02-02 | GPT peer review #2: Added Pattern B runtime scope, formal utterance segmentation, session state model, metrics instrumentation |
| v4 | 2026-02-02 | GPT peer review #3: DSL materialization contract, pipeline invariants, snapshot schemas, membership confidence, cross-group as linter rule, determinism policy, Implementation Contract appendix |
| **v5** | **2026-02-02** | **Codebase Validation: Reconciled proposal with actual ob-poc implementation. Identified existing implementations (Stage 0 scope resolution, 052/055 migrations, attribute_values_typed). Added Implementation Alignment section. Minimum-diff specification for scope operands. TODO.md for Claude Code.** |

### v5 Changes Summary (Codebase-Validated)

**Key Finding**: GPT codebase validation revealed that significant portions of v3/v4 are **already implemented**. v5 reconciles the proposal with reality.

1. **Implementation Alignment Section**: Maps proposal concepts to actual code locations
2. **Existing Implementation Documentation**: Stage 0 scope resolution, semantic entity search, typed roles, attribute provenance
3. **Gap Analysis**: What still needs to be built (scope operands, snapshot tables, resolution events)
4. **Minimum-Diff Approach**: Extend existing machinery rather than duplicate
5. **Corrections**: Removed outdated claims about `client_group_entity` duplicating facts (already slim in 052)
6. **TODO.md Generation**: Phased implementation plan for Claude Code

1. **DSL Materialization Contract**: Canonical macro form + expanded form with **Option A (tuple expansion)** as default
2. **Pipeline Invariants**: Explicit Parse → Resolve → Lint → DAG → Execute with scope handling at each stage
3. **Snapshot Contract**: `scope_snapshots` and `resolution_events` DB schemas for deterministic replay + learning
4. **Membership Confidence**: Added `auto_confirmed` boolean and `confidence` score to review workflow
5. **Cross-Group Protection**: Now a **machine-enforced linter rule (S002)**, not just narrative
6. **Determinism Policy**: Explicit ordering rule, replay semantics, re-resolve semantics
7. **Implementation Contract Appendix**: EBNF grammar, Rust IR structs, DB schemas, one-page policy

---

## Executive Summary

This proposal addresses two **orthogonal concerns** that have become conflated in the current `client_group_entity` table design:

1. **Data Quality & Lineage** — Tracking the provenance of entity attributes (allegations vs. verified facts)
2. **Semantic Agent Assistance** — Resolving informal human language ("the Allianz Irish funds") to entity scope

The current implementation duplicates entity attributes in both staging and entity tables, creating data integrity issues and confusion about source of truth. This proposal separates these concerns into distinct architectural layers.

**Key Architectural Decision (v4)**: Entity scope resolution supports **TWO patterns** with a **canonical DSL materialization contract**:
- **Pattern A: Compile-Time Expansion** — For runbooks, LSP validation, deterministic replay
- **Pattern B: Runtime Scope Resolution** — For interactive sessions, conversational state, disambiguation

Both patterns converge to the same **expanded form** before execution, ensuring determinism.

---

## ⚡ Implementation Alignment (v5 - Codebase-Validated)

> **Critical**: This section maps proposal concepts to **actual implemented code**. Read this first before implementation to avoid duplicating existing machinery.

### Principle: Extend, Don't Duplicate

**DO**:
- Reuse `rust/src/mcp/scope_resolution.rs` guardrails for utterance segmentation
- Extend `client_group_entity` model (already slim) rather than replace
- Build on `attribute_values_typed` rather than parallel `entity_attribute_sources`
- Add scope operands to existing AST/compiler/DAG pipeline

**DON'T**:
- Invent new scope phrase detection (use existing prefix guards, target indicators)
- Create parallel entity membership tables
- Duplicate attribute provenance tracking
- Rebuild pipeline stages that already exist

---

### ✅ What Already Exists (Validated Against Source)

#### 1. Stage 0 Scope Resolution (HARD GATE) - IMPLEMENTED

**Location**: `rust/src/mcp/scope_resolution.rs`

```rust
// Already implemented:
- ScopeResolver::is_scope_phrase() with SCOPE_PREFIXES + TARGET_INDICATORS
- ScopeContext { client_group_id, persona } carried through pipeline
- Returns early (skipping Candle/LLM) when user setting client context
- Exact "parallel track" principle from proposal
```

**DB Layer**: `client_group_alias` resolution via `search_client_groups()`

**Implication**: v5 utterance segmentation **MUST** reuse these existing guardrails:
- `SCOPE_PREFIXES`: "work on", "working on", "switch to", "set client to", etc.
- `TARGET_INDICATORS`: "cbu", "fund", "custody", "kyc", "entity", etc.
- Single-token high-confidence rule (MIN_SINGLE_TOKEN_CONFIDENCE = 0.85)

#### 2. Semantic Bridge for Entities - IMPLEMENTED

**Location**: `migrations/052_client_group_entity_context.sql`

```sql
-- Already implemented (slim membership model):
client_group_entity = membership + provenance only (NO fact duplication!)
client_group_entity_tag = shorthand human tags (persona-scoped)
client_group_entity_tag_embedding = vector(384) embeddings (BGE-small)
search_entity_tags() = exact + trigram fuzzy search
search_entity_tags_semantic() = embedding similarity search
```

**Runtime**: `rust/src/mcp/scope_resolution.rs`
- `search_entities_in_scope()` calls `search_entity_tags()`
- `search_in_scope()` typed dispatcher ("slot type controls what Allianz can mean")

**⚠️ v5 Correction**: Earlier proposal versions claimed `client_group_entity` "duplicates facts like LEI/jurisdiction". This is **outdated**. The 052 migration already implemented the slim membership model. Remove any "slim client_group_entity migration" claims from implementation.

#### 3. Typed Roles + Relationship Lineage - IMPLEMENTED

**Location**: `migrations/055_client_group_research.sql`

```sql
-- Already implemented:
client_group_entity_roles (FK to roles table; targeted roles supported)
client_group_relationship (provisional edges with parent→child)
client_group_relationship_sources (multi-source lineage + verification + canonical flags)
v_cgr_canonical (best-available ownership view with source authority ranking)
v_cgr_discrepancies (multi-source conflict detection)
```

Matches proposal's "allegations" and "verification outcomes" as first-class. The `canonical` flag and `verification_status` enum already exist.

#### 4. Typed Attribute System with Provenance - IMPLEMENTED

**Location**: `"ob-poc".attribute_values_typed`

```sql
-- Already implemented:
- Typed columns: value_text, value_number, value_boolean, etc.
- source JSONB field for provenance
- attribute_registry with policy like "requires authoritative source"
```

**⚠️ Decision**: v4 proposed `entity_attribute_sources` table. This would **duplicate existing machinery**. v5 recommends:
- **Option A (Recommended)**: Treat `attribute_values_typed` as observation log, add canon/dispute workflow around it (views + canonical selection metadata)
- **Option B**: Explicitly replace `attribute_values_typed` (requires migration plan)

#### 5. Parse→Compile→DAG→Execute Pipeline - IMPLEMENTED

**Locations**:
```
rust/crates/dsl-core/src/parser.rs   → parse_program()
rust/crates/dsl-core/src/compiler.rs → compile() / compile_to_ops()
rust/crates/dsl-core/src/dag.rs      → DAG construction
rust/crates/dsl-core/src/ast.rs      → AST node types
```

**Key Validation**: Existing compiler expects resolved `EntityRef` inputs (single-entity). **No notion of "set/scope ref" in DSL AST or compiler ops**. This confirms the gap: need to add scope operands.

---

### ❌ What Still Needs to Be Built (Gaps)

#### 1. EntityScope as First-Class DSL Operand

**Gap**: No `@s1` scope symbols in AST  
**Gap**: No scope verbs (`scope.resolve`, `scope.narrow`, `scope.commit`)  
**Gap**: No snapshotting mechanism  

**Required**: Extend existing pipeline to support:
- New AST node types for scope operations
- Compiler ops for scope resolution
- DAG edges for scope dependencies
- `scope_snapshot` table (as proposed in v4)

#### 2. Scope Snapshot Table

**Status**: Proposed in v4, not yet implemented  
**Location**: New migration `064_scope_snapshots.sql`

#### 3. Resolution Events Learning Loop

**Status**: Proposed in v4, not yet implemented  
**Location**: New migration `065_resolution_events.sql`

**Must Integrate With**:
- Existing `ScopeResolver::record_selection()` (already reinforces aliases)
- Existing verb disambiguation "needs clarification" outcomes

---

### Minimum Diff to Support @scope in DSL

**AST Extensions** (`rust/crates/dsl-core/src/ast.rs`):
```rust
enum Expr {
    // ... existing variants
    ScopeAnchor { group: String },
    ScopeResolve { desc: String, limit: u32, mode: ResolutionMode, as_symbol: String },
    ScopeNarrow { scope_symbol: String, filter: FilterExpr, as_symbol: String },
    ScopeCommit { scope_symbol: String, as_symbol: String },
    ScopeRefresh { old_symbol: String, as_symbol: String },
}

enum ArgValue {
    // ... existing variants
    ScopeRef(String),  // @s1
}

enum ResolutionMode { Exact, Fuzzy, Semantic }
```

**Compiler Ops** (`rust/crates/dsl-core/src/ops.rs`):
```rust
enum Op {
    // ... existing ops
    ResolveScope { descriptor: ScopeDescriptor, output_symbol: String },
    CommitScope { candidate_symbol: String, snapshot_id: Uuid },
}
```

**DAG Dependencies** (`rust/crates/dsl-core/src/dag.rs`):
- `scope.*` producers MUST execute before consumers
- `(verb :scope @sX)` depends on `scope.commit` that binds `@sX`

---

The DSL is inherently composite: `(action, entity...)`. The current system resolves verbs deterministically but leaves entity scope as "agent magic". This proposal makes both halves symmetric:

| Aspect | Verb Resolution | Entity Scope Resolution |
|--------|-----------------|------------------------|
| Input | Natural language phrase | Natural language phrase |
| Matching | Candle embeddings + invocation_phrases | Candle embeddings + scope_phrases |
| Validation | LSP checks verb exists, args valid | LSP checks group exists, entities resolve |
| Ambiguity | Compiler error / clarification | Compiler error / clarification |
| Output | Concrete verb with typed args | Concrete entity_id list |
| Snapshot | Verb + args stored | Scope descriptor + resolved ids stored |
| Replay | Deterministic | Deterministic (uses stored ids) |
| Learning | `agent.teach :phrase → :verb` | `agent.teach-scope :phrase → :scope` |

**Same contract. Same validation. Same determinism. Same learning mechanism.**

---

## Problem Statement

### Problem 1: The "Same Data, Two Places" Smell

The `client_group_entity` table has accumulated columns that duplicate data in `entity_limited_companies`:

```
client_group_entity              entity_limited_companies
─────────────────────            ────────────────────────
lei                      ←DUP→   lei
legal_name               ←DUP→   (via entities.name)
jurisdiction             ←DUP→   jurisdiction
is_fund                  ←DUP→   is_fund
related_via_lei          ←DUP→   manco_lei
relationship_category    ←DUP→   (should be entity_relationships)
```

**Consequence**: When an entity is "promoted" from staging to proper tables, we have two copies of the same facts with no clear authority.

### Problem 2: Allegations vs. Facts

Entity data comes from multiple sources with varying confidence:

| Source | Confidence | Example |
|--------|------------|---------|
| GLEIF | High (authoritative for LEI) | LEI, legal name, jurisdiction |
| Client allegation | Low (self-reported) | "We're 100% owned by HoldCo X" |
| LLM web scraper | Variable | "Found entity in Luxembourg" |
| Companies House | High (authoritative for UK) | Share register, officers |

**Current limitation**: The binary staging/promoted model forces us to treat data as either "unverified garbage" or "verified truth". Reality is a gradient with potential conflicts.

**Existing pattern**: The `shareholdings` + `shareholding_sources` tables already solve this for ownership data — client alleges 100% ownership, Companies House says 50/50, the conflict surfaces for review.

**Gap**: This pattern doesn't extend to other entity attributes (name, jurisdiction, fund status).

### Problem 3: Semantic Agent Scope Resolution

Users interact with the system using informal language:

```
User: "Load the Allianz Irish ETF funds"
```

This contains:
- **"Allianz"** — Not an entity! It's shorthand for a client group
- **"Irish ETF funds"** — Informal description of entity characteristics

The agent needs to:
1. Resolve "Allianz" to a session scope anchor
2. Find entities within that scope matching "Irish ETF funds"
3. Pass those entity_ids to the verb handler

**This is parallel to verb intent matching**:
- Verbs define WHAT operation ("show ownership chain" → `ubo.trace-chain`)
- Entity scope defines WHICH entities ("Allianz Irish funds" → `[entity_id, ...]`)

**Current confusion**: The `client_group_entity` table mixes semantic search helpers (role_tags) with entity facts (lei, jurisdiction), making it unclear what's authoritative.

---

## Current Architecture

### Entity Tables (Authoritative Facts)

```
entities
├── entity_id (PK)
├── name
├── entity_type_id
└── ... base fields

entity_limited_companies (1:1 extension)
├── company_id (PK)
├── entity_id (FK, UNIQUE)
├── lei (UNIQUE)
├── jurisdiction
├── is_fund
├── gleif_status, gleif_category
├── direct_parent_lei, ultimate_parent_lei
├── manco_lei, umbrella_lei
└── ... GLEIF fields

entity_relationships (graph edges)
├── from_entity_id
├── to_entity_id
├── relationship_type
└── ownership_pct
```

### Ownership Lineage (Existing Pattern)

```
shareholdings                    shareholding_sources
─────────────                    ────────────────────
subject_entity_id                shareholding_id (FK)
owner_entity_id                  source
ownership_pct ◄── Best known     ownership_pct ◄── Per source
source                           verification_status
verification_status              document_ref
```

### Client Group Tables (Current - Mixed Concerns)

```
client_group
├── id
├── canonical_name         ← "Allianz Group"
└── description

client_group_alias
├── group_id
├── alias                  ← "Allianz", "AGI", "the Germans"
├── alias_norm
└── embedding              ← For Candle semantic match

client_group_entity (PROBLEMATIC - MIXED CONCERNS)
├── group_id
├── entity_id
│
├── ─── SEMANTIC LAYER (OK) ───
├── role_tags[]            ← ['MANCO', 'LUX', 'FUND'] for search
├── membership_type        ← in_group, service_provider
├── added_by               ← gleif, manual, agent
│
├── ─── FACT LAYER (WRONG PLACE) ───
├── lei                    ← Should be entity_limited_companies
├── legal_name             ← Should be entities.name
├── jurisdiction           ← Should be entity_limited_companies
├── is_fund                ← Should be entity_limited_companies
├── relationship_category  ← Should be entity_relationships
└── related_via_lei        ← Should be entity_limited_companies.manco_lei
```

---

## Proposed Architecture

### Layer 0: DSL Materialization Contract (v4 - Critical)

**Principle**: All scope resolution — whether compile-time or runtime — MUST materialize to a canonical DSL form before execution. This is **non-negotiable** for determinism.

#### 0.1 Macro Form (Authoring / Interactive)

This is what users write or what the agent proposes:

```clojure
;; ═══════════════════════════════════════════════════════════════════════════
;; PATTERN A: Compile-time (runbooks)
;; ═══════════════════════════════════════════════════════════════════════════
(scope.define @irish_etfs
    :group "Allianz"
    :filter {:jurisdiction "IE" :is-fund true}
    :limit 50)

(ubo.trace-chain :scope @irish_etfs :depth 3)

;; ═══════════════════════════════════════════════════════════════════════════
;; PATTERN B: Runtime (interactive session)
;; ═══════════════════════════════════════════════════════════════════════════
(scope.anchor :group "Allianz")                              ; sets session anchor
(scope.resolve :desc "irish etf funds" :limit 50 :as @s1)    ; preview/resolve
(scope.commit :scope @s1 :as @s_irish_etf)                   ; freezes snapshot

(ubo.trace-chain :scope @s_irish_etf :depth 3)
```

#### 0.2 Expanded Form (Execution - Canonical)

**RULE: All execution paths converge to expanded form.**

**Option A: Tuple Expansion (DEFAULT — aligns with "DSL = action + entity tuple")**

```clojure
;; Scope resolution produces ordered entity list
;; Compiler expands to one tuple per entity

(ubo.trace-chain :entity @e_101 :depth 3)
(ubo.trace-chain :entity @e_204 :depth 3)
(ubo.trace-chain :entity @e_319 :depth 3)
```

**Option B: Vector Operand (allowed for set-native verbs ONLY)**

```clojure
;; For verbs explicitly marked `set_native: true` in their schema
;; (e.g., render, highlight, bulk annotate, zoom-to-fit)

(scope.bind :snapshot @snap_s_irish_etf :as @s_irish_etf)
(graph.highlight :scope @s_irish_etf :color "blue")
(graph.zoom-to-fit :scope @s_irish_etf)
```

**POLICY**: Option A is the default. Option B is allowed ONLY for verbs explicitly marked `set_native: true` in their YAML schema.

#### 0.3 Materialization Invariant Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  INVARIANT: All execution paths converge to expanded form                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Pattern A (Runbook)           Pattern B (Interactive)                      │
│  ─────────────────────         ───────────────────────                      │
│  scope.define @s               scope.anchor                                 │
│       │                        scope.resolve :as @s                         │
│       │                        scope.commit :as @s_final                    │
│       │                             │                                       │
│       └──────────┬─────────────────┘                                       │
│                  │                                                          │
│                  ▼                                                          │
│         ┌───────────────────┐                                              │
│         │  MACRO EXPANSION  │                                              │
│         │  (compiler phase) │                                              │
│         └─────────┬─────────┘                                              │
│                   │                                                         │
│                   ▼                                                         │
│         ┌───────────────────────────────────────────┐                      │
│         │  EXPANDED FORM (deterministic)            │                      │
│         │                                           │                      │
│         │  (verb :entity @e1 ...)                  │                      │
│         │  (verb :entity @e2 ...)                  │                      │
│         │  (verb :entity @e3 ...)                  │                      │
│         │                                           │                      │
│         │  + ScopeSnapshot record for audit/replay │                      │
│         └───────────────────────────────────────────┘                      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

### Layer 0.5: Pipeline Invariants (v4 - Critical)

**Principle**: Each pipeline stage has explicit responsibilities for scope handling. Implementation **cannot drift** from these invariants.

#### Stage 1: Parse

```
┌────────────────────────────────────────────────────────────────────────────┐
│  PARSE                                                                     │
├────────────────────────────────────────────────────────────────────────────┤
│  Responsibilities:                                                         │
│  ├── Add AST nodes: ScopeAnchor, ScopeDefine, ScopeResolve, ScopeNarrow,  │
│  │                  ScopeCommit, ScopeRefresh                             │
│  ├── Parse verb args: :entity @eX and :scope @sX                          │
│  ├── Validate syntax (not semantics)                                       │
│  └── Output: Unresolved AST with scope nodes                              │
│                                                                            │
│  New AST Node Types:                                                       │
│  ──────────────────                                                        │
│  ScopeAnchor   { group: String }                                          │
│  ScopeDefine   { name: Symbol, group: Option<String>, filter: ScopeFilter,│
│                  limit: u32 }                                              │
│  ScopeResolve  { desc: String, limit: u32, mode: Mode, as_name: Symbol }  │
│  ScopeNarrow   { source: Symbol, filter: ScopeFilter, as_name: Symbol }   │
│  ScopeCommit   { scope: Symbol, as_name: Symbol }                         │
│  ScopeRefresh  { old_scope: Symbol, as_name: Symbol }                     │
│  VerbCall      { verb: String, args: Args }  // args may include :scope   │
└────────────────────────────────────────────────────────────────────────────┘
```

#### Stage 2: Resolve (Symbol Binding)

```
┌────────────────────────────────────────────────────────────────────────────┐
│  RESOLVE                                                                   │
├────────────────────────────────────────────────────────────────────────────┤
│  Responsibilities:                                                         │
│  ├── Bind @symbols to their definitions                                   │
│  ├── Type-check symbol references (@sX must be Scope type)                │
│  ├── DO NOT execute scope resolution (no DB queries here)                 │
│  └── Output: Resolved AST with typed symbol table                         │
│                                                                            │
│  Symbol Table Entry:                                                       │
│  ───────────────────                                                       │
│  {                                                                         │
│    name: "@s1",                                                            │
│    type: Scope,                                                            │
│    defined_at: NodeId,                                                     │
│    used_at: [NodeId...],                                                   │
│    depends_on: [Symbol...]                                                │
│  }                                                                         │
└────────────────────────────────────────────────────────────────────────────┘
```

#### Stage 3: Lint (v4 - Scope-Aware Rules)

```
┌────────────────────────────────────────────────────────────────────────────┐
│  LINT                                                                      │
├────────────────────────────────────────────────────────────────────────────┤
│  Responsibilities:                                                         │
│  ├── Enforce determinism + safety rules                                   │
│  ├── Emit warnings/errors for policy violations                           │
│  └── Output: Diagnostics + validated AST                                  │
│                                                                            │
│  ═══════════════════════════════════════════════════════════════════════  │
│  SCOPE LINT RULES (machine-enforced)                                       │
│  ═══════════════════════════════════════════════════════════════════════  │
│                                                                            │
│  S001  :limit REQUIRED on scope.resolve/define                            │
│        Must be ≤ POLICY_MAX (default: 100)                                │
│        ERROR: "scope.resolve requires :limit, max 100"                    │
│                                                                            │
│  S002  CROSS-GROUP SCOPE FORBIDDEN unless :global true                    │
│        scope.resolve searches only within anchored group                  │
│        ERROR: "cross-group scope requires explicit :global true"          │
│                                                                            │
│  S003  scope.commit REQUIRED before scope consumption in :strict mode     │
│        ERROR: "scope @s1 used before scope.commit in strict mode"         │
│                                                                            │
│  S004  Ambiguous scope MUST route to clarification UX in :interactive     │
│        WARN: "ambiguous scope will prompt for clarification"              │
│                                                                            │
│  S005  Entity reference @eX must exist in resolved scope                  │
│        ERROR: "entity @e42 not found in scope @s1"                        │
│                                                                            │
│  S006  Scope symbol @sX must be defined before use                        │
│        ERROR: "undefined scope symbol @s1"                                 │
│                                                                            │
│  S007  Circular scope dependencies forbidden                               │
│        ERROR: "circular dependency: @s1 → @s2 → @s1"                      │
│                                                                            │
│  ═══════════════════════════════════════════════════════════════════════  │
│  AMBIGUITY MODES                                                           │
│  ═══════════════════════════════════════════════════════════════════════  │
│                                                                            │
│  :mode strict      → Fail compilation if scope doesn't resolve uniquely   │
│  :mode interactive → Emit clarification request, await scope.commit       │
│  :mode greedy      → Take top-1 match (audit logged, not recommended)     │
│                                                                            │
│  Default: :interactive for chat, :strict for runbooks                     │
└────────────────────────────────────────────────────────────────────────────┘
```

#### Stage 4: Macro Expand (v4 - Scope Expansion)

```
┌────────────────────────────────────────────────────────────────────────────┐
│  MACRO EXPAND                                                              │
├────────────────────────────────────────────────────────────────────────────┤
│  Responsibilities:                                                         │
│  ├── Execute scope resolution (DB queries happen here)                    │
│  ├── Expand (verb :scope @sX) to tuple form                              │
│  ├── Create ScopeSnapshot records                                         │
│  └── Output: Expanded AST with concrete entity references                 │
│                                                                            │
│  Expansion Process:                                                        │
│  ──────────────────                                                        │
│  1. For each scope.define / scope.commit:                                 │
│     a. Execute resolution query against DB                                │
│     b. Create ScopeSnapshot { descriptor, entity_ids, scoring }          │
│     c. Bind snapshot to symbol                                            │
│                                                                            │
│  2. For each (verb :scope @sX ...):                                       │
│     a. Look up bound snapshot                                             │
│     b. Check verb schema for set_native flag                              │
│     c. If verb.set_native == true:                                        │
│        emit (verb :scope @snapshot_ref ...)                               │
│     d. Else (DEFAULT):                                                    │
│        expand to N tuples: (verb :entity @eN ...)                        │
│                                                                            │
│  3. Emit expanded AST + snapshot manifest                                 │
└────────────────────────────────────────────────────────────────────────────┘
```

#### Stage 5: DAG / Topological Sort

```
┌────────────────────────────────────────────────────────────────────────────┐
│  DAG / TOPO SORT                                                           │
├────────────────────────────────────────────────────────────────────────────┤
│  Responsibilities:                                                         │
│  ├── Order nodes respecting dependencies                                  │
│  ├── scope.* producers MUST execute before consumers                      │
│  ├── Detect cycles                                                        │
│  └── Output: Execution-ordered node list                                  │
│                                                                            │
│  Dependency Rules:                                                         │
│  ─────────────────                                                         │
│  scope.anchor       → No dependencies                                     │
│  scope.define       → Depends on scope.anchor (if no explicit :group)    │
│  scope.resolve      → Depends on scope.anchor                             │
│  scope.narrow       → Depends on source scope symbol                      │
│  scope.commit       → Depends on scope.resolve that produces it          │
│  scope.refresh      → Depends on old scope symbol                         │
│  (verb :scope @sX)  → Depends on scope.commit that binds @sX             │
│  (verb :entity @eX) → Depends on scope that contains @eX                 │
└────────────────────────────────────────────────────────────────────────────┘
```

#### Stage 6: Execute

```
┌────────────────────────────────────────────────────────────────────────────┐
│  EXECUTE                                                                   │
├────────────────────────────────────────────────────────────────────────────┤
│  Responsibilities:                                                         │
│  ├── Execute nodes in DAG order                                           │
│  ├── Two-phase scope execution (see below)                                │
│  ├── Log ResolutionEvent for learning                                     │
│  └── Output: Execution results                                            │
│                                                                            │
│  ═══════════════════════════════════════════════════════════════════════  │
│  TWO-PHASE SCOPE EXECUTION                                                │
│  ═══════════════════════════════════════════════════════════════════════  │
│                                                                            │
│  PHASE 1: Scope Resolution                                                │
│  ─────────────────────────                                                │
│  a. Execute scope.resolve/narrow → candidate sets                        │
│  b. If ambiguous + :mode interactive → PAUSE, emit clarification         │
│  c. Await user selection → user issues scope.commit                       │
│                                                                            │
│  PHASE 2: Scope Commitment                                                │
│  ────────────────────────                                                 │
│  a. Execute scope.commit → freeze resolved list into ScopeSnapshot       │
│  b. ScopeSnapshot is IMMUTABLE after commit                              │
│  c. Write snapshot to scope_snapshots table                               │
│                                                                            │
│  PHASE 3: Verb Execution                                                  │
│  ───────────────────────                                                  │
│  a. Execute expanded verbs against snapshot entity_ids                   │
│  b. Retrieve entity facts from authoritative tables                       │
│     (entities, entity_limited_companies — NOT client_group_entity)       │
│  c. Log ResolutionEvent with full top-k and outcomes                     │
└────────────────────────────────────────────────────────────────────────────┘
```

---

### Layer 0.75: Snapshot Contract (v4 - Non-Negotiable)

**Principle**: Every scope resolution produces an immutable snapshot. This enables deterministic replay and learning without poisoning production data.

#### ScopeSnapshot Schema

```sql
-- Migration: 064_scope_snapshots.sql

CREATE TABLE scope_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- ═══════════════════════════════════════════════════════════════════
    -- DESCRIPTOR (what was requested)
    -- ═══════════════════════════════════════════════════════════════════
    group_id UUID NOT NULL REFERENCES client_group(id),
    group_anchor_name TEXT NOT NULL,
    description TEXT,                        -- "irish etf funds"
    filter_applied JSONB,                    -- {jurisdiction: "IE", tags: [...]}
    limit_requested INTEGER NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('strict', 'interactive', 'greedy')),
    
    -- ═══════════════════════════════════════════════════════════════════
    -- RESOLVED LIST (deterministic output)
    -- ═══════════════════════════════════════════════════════════════════
    selected_entity_ids UUID[] NOT NULL,     -- Ordered deterministically (see policy)
    entity_count INTEGER NOT NULL,
    
    -- ═══════════════════════════════════════════════════════════════════
    -- SCORING SUMMARY (for learning/debug)
    -- ═══════════════════════════════════════════════════════════════════
    top_k_candidates JSONB NOT NULL,         -- [{entity_id, name, score, method}, ...]
    resolution_method TEXT NOT NULL,         -- 'exact_phrase', 'role_tag', 'semantic', 'authoritative'
    overall_confidence DECIMAL(3,2),
    
    -- ═══════════════════════════════════════════════════════════════════
    -- INDEX FINGERPRINTS (drift detection)
    -- ═══════════════════════════════════════════════════════════════════
    scope_phrases_hash TEXT,                 -- Hash of scope_phrases table state
    embedder_version TEXT,                   -- e.g., 'bge-small-en-v1.5-20260201'
    role_tags_hash TEXT,                     -- Hash of role_tags at resolution time
    
    -- ═══════════════════════════════════════════════════════════════════
    -- LINEAGE (for scope.refresh)
    -- ═══════════════════════════════════════════════════════════════════
    parent_snapshot_id UUID REFERENCES scope_snapshots(id),
    
    -- ═══════════════════════════════════════════════════════════════════
    -- AUDIT
    -- ═══════════════════════════════════════════════════════════════════
    session_id UUID,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    created_by TEXT NOT NULL                 -- 'agent', 'user', 'runbook:{id}'
);

-- Indexes
CREATE INDEX idx_ss_session ON scope_snapshots(session_id);
CREATE INDEX idx_ss_group ON scope_snapshots(group_id);
CREATE INDEX idx_ss_created ON scope_snapshots(created_at DESC);
CREATE INDEX idx_ss_parent ON scope_snapshots(parent_snapshot_id) 
    WHERE parent_snapshot_id IS NOT NULL;

-- Snapshots are IMMUTABLE — enforce via trigger
CREATE OR REPLACE FUNCTION prevent_snapshot_update() RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'scope_snapshots are immutable after creation';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER snapshot_immutable
    BEFORE UPDATE ON scope_snapshots
    FOR EACH ROW EXECUTE FUNCTION prevent_snapshot_update();
```

#### ResolutionEvent Schema (Learning Loop Payload)

```sql
-- Migration: 065_resolution_events.sql

CREATE TABLE resolution_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID,
    
    -- ═══════════════════════════════════════════════════════════════════
    -- SEGMENTATION OUTPUT
    -- ═══════════════════════════════════════════════════════════════════
    raw_input TEXT NOT NULL,
    segmentation JSONB NOT NULL,
    -- {
    --   verb_phrase: {text, span, confidence, match_source},
    --   group_phrase: {text, span, confidence, match_source},
    --   scope_phrase: {text, span, confidence, match_source},
    --   residual_terms: [...],
    --   overall_confidence: 0.91
    -- }
    
    -- ═══════════════════════════════════════════════════════════════════
    -- VERB RESOLUTION (top-k + chosen)
    -- ═══════════════════════════════════════════════════════════════════
    verb_top_k JSONB NOT NULL,               -- [{verb, score}, {verb, score}, ...]
    verb_chosen TEXT,
    verb_confidence DECIMAL(3,2),
    
    -- ═══════════════════════════════════════════════════════════════════
    -- GROUP RESOLUTION (top-k + chosen)
    -- ═══════════════════════════════════════════════════════════════════
    group_top_k JSONB,                       -- [{group_id, name, score}, ...]
    group_chosen_id UUID,
    group_confidence DECIMAL(3,2),
    
    -- ═══════════════════════════════════════════════════════════════════
    -- SCOPE RESOLUTION (top-k + chosen)
    -- ═══════════════════════════════════════════════════════════════════
    scope_top_k JSONB,                       -- [{entity_id, name, score, method}, ...]
    scope_snapshot_id UUID REFERENCES scope_snapshots(id),
    scope_confidence DECIMAL(3,2),
    
    -- ═══════════════════════════════════════════════════════════════════
    -- USER INTERACTION (what the user did)
    -- ═══════════════════════════════════════════════════════════════════
    was_ambiguous BOOLEAN DEFAULT false,
    clarification_shown BOOLEAN DEFAULT false,
    user_override_verb BOOLEAN DEFAULT false,
    user_override_group BOOLEAN DEFAULT false,
    user_narrowed_scope BOOLEAN DEFAULT false,
    user_widened_scope BOOLEAN DEFAULT false,
    user_confirmed BOOLEAN,
    
    -- ═══════════════════════════════════════════════════════════════════
    -- OUTCOME
    -- ═══════════════════════════════════════════════════════════════════
    execution_success BOOLEAN,
    error_message TEXT,
    
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Indexes for learning queries
CREATE INDEX idx_re_session ON resolution_events(session_id);
CREATE INDEX idx_re_ambiguous ON resolution_events(was_ambiguous) 
    WHERE was_ambiguous = true;
CREATE INDEX idx_re_overrides ON resolution_events(user_override_verb, user_override_group);
CREATE INDEX idx_re_narrowed ON resolution_events(user_narrowed_scope) 
    WHERE user_narrowed_scope = true;
CREATE INDEX idx_re_created ON resolution_events(created_at DESC);
```

#### Determinism Policy (v4)

```
┌────────────────────────────────────────────────────────────────────────────┐
│  DETERMINISM POLICY                                                        │
├────────────────────────────────────────────────────────────────────────────┤
│                                                                            │
│  1. ORDERING RULE                                                          │
│     ─────────────                                                          │
│     Entity IDs in selected_entity_ids are ordered by:                     │
│       a. Primary: resolution_score DESC                                   │
│       b. Secondary: entity_id ASC (lexicographic UUID for stability)     │
│                                                                            │
│     This ensures identical inputs produce identical ordered outputs.      │
│                                                                            │
│  2. REPLAY SEMANTICS                                                       │
│     ─────────────────                                                      │
│     Replay ALWAYS uses snapshot.selected_entity_ids                       │
│     Never re-resolves against current index state                         │
│     Snapshot is IMMUTABLE after creation (enforced by DB trigger)        │
│                                                                            │
│     Example:                                                               │
│       replay_runbook(runbook_id: "xyz", mode: "replay")                  │
│       → Loads snapshot, executes against stored entity_ids                │
│       → Index changes since snapshot time are IGNORED                     │
│                                                                            │
│  3. RE-RESOLVE SEMANTICS                                                   │
│     ────────────────────                                                   │
│     Re-resolution requires EXPLICIT DSL operation:                        │
│                                                                            │
│       (scope.refresh @s_old :as @s_new)                                   │
│                                                                            │
│     This creates a NEW snapshot linked via parent_snapshot_id            │
│     Old snapshot remains for audit trail                                  │
│     Cannot refresh without explicit user action                           │
│                                                                            │
│  4. DRIFT DETECTION                                                        │
│     ───────────────                                                        │
│     On replay, compare current index fingerprints to snapshot:           │
│       - scope_phrases_hash                                                │
│       - embedder_version                                                  │
│       - role_tags_hash                                                    │
│                                                                            │
│     If mismatch:                                                          │
│       - Emit WARNING to user/log                                          │
│       - Execute with snapshot data (not current index)                   │
│       - Log drift event for monitoring                                    │
│                                                                            │
│     Drift does NOT block execution — it's informational.                 │
│                                                                            │
└────────────────────────────────────────────────────────────────────────────┘
```

---

### Layer 1: Dual-Pattern Scope Resolution

**Principle**: Entity scope resolution supports TWO complementary patterns that share validation logic and learning feedback.

#### Why Two Patterns?

| Aspect | Pattern A (Compile-Time) | Pattern B (Runtime) |
|--------|-------------------------|---------------------|
| Use case | Runbooks, batch, LSP validation | Interactive chat sessions |
| Scope resolution | At compile/save time | At execution time |
| Session state | N/A (self-contained) | Required (group anchor, scopes) |
| Disambiguation | Compiler error | Interactive selection |
| Conversational | No | Yes (scope builds across turns) |
| Snapshot | Original + expanded tuples | Original + resolved entity_ids |
| Replay | Use expanded tuples | Use snapshotted entity_ids |

**Key insight**: In chat, scope is conversational state, not a single DSL statement.

#### Pattern A: Compile-Time Expansion (Runbooks/LSP)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  RUNBOOK AUTHORING                                                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  (scope.define @irish_etfs                                                  │
│      :group "Allianz"                                                       │
│      :filter {:jurisdiction "IE" :is-fund true}                            │
│      :limit 50)                                                             │
│                                                                             │
│  (ubo.trace-chain :scope @irish_etfs)                                      │
│                                                                             │
│  → LSP validates at authoring time                                          │
│  → Compiler expands to concrete tuples at save/execute                      │
│  → Snapshot stores both: original scope + resolved entity_ids              │
│                                                                             │
│  EXPANDED (what gets executed):                                             │
│  (ubo.trace-chain :entity-id "abc-123")                                    │
│  (ubo.trace-chain :entity-id "def-456")                                    │
│  (ubo.trace-chain :entity-id "ghi-789")                                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Pattern B: Runtime Scope Resolution (Interactive Sessions)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  INTERACTIVE SESSION                                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Turn 1: "Work on Allianz"                                                  │
│          → (scope.anchor :group "Allianz")                                  │
│          → session.anchor_group_id = "..."                                  │
│          → Agent: "Session anchored to Allianz Group (47 entities)"        │
│                                                                             │
│  Turn 2: "Show me the Irish funds"                                          │
│          → (scope.resolve :desc "Irish funds" :limit 50 :as @s1)           │
│          → session.scopes["@s1"] = {entity_ids, resolved_at, ...}          │
│          → Agent: "Found 12 Irish funds..."                                 │
│                                                                             │
│  Turn 3: "Trace UBO for those"                                              │
│          → (scope.commit :scope @s1 :as @s1_final)                         │
│          → (ubo.trace-chain :scope @s1_final)                              │
│          → Executes against snapshotted entity_ids                          │
│          → Agent: "UBO chain for 12 entities..."                           │
│                                                                             │
│  Turn 4: "Just the ETF ones"                                                │
│          → (scope.narrow @s1_final :filter {:tags ["ETF"]} :as @s2)        │
│          → Narrows previous scope                                           │
│          → Agent: "Narrowed to 4 ETF funds..."                             │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Scope DSL Primitives (Unified)

```yaml
# In verbs/scope.yaml
domain: scope
description: "Entity scope resolution and management"

verbs:
  # === PATTERN A: Compile-time ===
  - verb: define
    description: "Define a named entity scope (compile-time resolution)"
    behavior: compiler_directive
    invocation_phrases:
      - "define scope"
      - "create scope"
    args:
      - name: name
        type: symbol              # @my_scope
        required: true
      - name: group
        type: string
        required: false           # Optional if session has anchor
        lookup:
          lookup_type: client_group
          search_key: alias
      - name: filter
        type: scope_filter
        required: false
      - name: entities
        type: array               # Explicit entity list (alternative to filter)
        items: uuid
        required: false
      - name: limit
        type: integer
        required: true
        validation:
          min: 1
          max: 100                # POLICY_MAX
    validation:
      one_of_required: [filter, entities]

  # === PATTERN B: Runtime ===
  - verb: anchor
    description: "Anchor session to a client group"
    behavior: runtime
    invocation_phrases:
      - "work on"
      - "switch to"
      - "focus on"
    args:
      - name: group
        type: string
        required: true
        lookup:
          lookup_type: client_group
          search_key: alias
    effects:
      - "Sets session.anchor_group_id"
      - "Clears session.scopes"

  - verb: resolve
    description: "Resolve entity scope at runtime"
    behavior: runtime
    invocation_phrases:
      - "show me the"
      - "find the"
      - "get the"
    args:
      - name: desc
        type: string              # Natural language: "Irish ETF funds"
        required: true
      - name: as
        type: symbol              # @s1
        required: true
      - name: limit
        type: integer
        required: true
        validation:
          min: 1
          max: 100
      - name: mode
        type: enum
        values: [strict, interactive, greedy]
        default: interactive
      - name: group
        type: string
        required: false           # Uses session anchor if not specified
      - name: global
        type: boolean
        default: false            # Must be true to search across groups
    effects:
      - "Resolves entity_ids"
      - "Stores in session.scopes[name]"
      - "Snapshots resolved_at timestamp"

  - verb: narrow
    description: "Narrow an existing scope"
    behavior: runtime
    invocation_phrases:
      - "just the"
      - "only the"
      - "filter to"
    args:
      - name: scope
        type: symbol              # @s1 (existing scope)
        required: true
      - name: filter
        type: scope_filter
        required: true
      - name: as
        type: symbol              # @s2 (new scope)
        required: true

  - verb: commit
    description: "Freeze a scope resolution into an immutable snapshot"
    behavior: runtime
    invocation_phrases:
      - "confirm"
      - "use these"
      - "select these"
    args:
      - name: scope
        type: symbol
        required: true
      - name: as
        type: symbol
        required: true
    effects:
      - "Creates ScopeSnapshot record"
      - "Snapshot becomes immutable"
      - "Logs learning event"

  - verb: refresh
    description: "Re-resolve a scope against current index state"
    behavior: runtime
    invocation_phrases:
      - "refresh scope"
      - "update scope"
    args:
      - name: scope
        type: symbol              # Old scope to refresh
        required: true
      - name: as
        type: symbol              # New scope symbol
        required: true
    effects:
      - "Creates new ScopeSnapshot with parent_snapshot_id link"
      - "Old snapshot preserved for audit"
```

#### Session State Model (Pattern B)

```rust
/// Runtime session state for interactive conversations
pub struct SessionContext {
    pub session_id: Uuid,
    
    // === Group Anchoring ===
    pub anchor_group_id: Option<Uuid>,
    pub anchor_group_name: Option<String>,
    pub anchor_set_at: Option<DateTime<Utc>>,
    
    // === Resolved Scopes (keyed by symbol) ===
    pub scopes: HashMap<String, ResolvedScope>,
    
    // === Conversational References ===
    pub last_scope_id: Option<String>,      // For "those" / "them" references
    pub last_entity_ids: Option<Vec<Uuid>>, // For "that one" / "the first one"
    
    // === Disambiguation State ===
    pub pending_disambiguation: Option<DisambiguationState>,
}

#[derive(Clone, Debug)]
pub struct ResolvedScope {
    pub scope_id: String,               // @s1, @irish_etfs
    pub description: String,            // "Irish ETF funds"
    pub group_id: Uuid,
    pub entity_ids: Vec<Uuid>,          // Resolved concrete IDs (ordered)
    pub resolved_at: DateTime<Utc>,
    pub resolution_method: ScopeResolutionMethod,
    pub filter_applied: Option<ScopeFilter>,
    pub snapshot_id: Option<Uuid>,      // Set after scope.commit
    pub is_committed: bool,             // True after scope.commit
}

#[derive(Clone, Debug)]
pub struct DisambiguationState {
    pub resolution_id: String,
    pub original_phrase: String,
    pub candidates: Vec<ScopeCandidate>,
    pub awaiting_selection: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct ScopeCandidate {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub score: f32,
    pub match_reason: String,           // "exact_phrase", "semantic", "role_tag"
}
```

---

### Layer 2: Formal Utterance Segmentation

**Principle**: "Extract entity descriptors from input" is where systems fail. We need an **explicit segmentation contract** — essentially a lightweight domain-specific NER.

#### Segmentation Output Contract

```rust
/// The output of utterance segmentation — everything tagged by role
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtteranceSegmentation {
    pub raw_input: String,
    
    // === Segmented Components ===
    pub verb_phrase: Option<SegmentedPhrase>,     // "trace UBO", "show ownership chain"
    pub group_phrase: Option<SegmentedPhrase>,    // "Allianz", "the BlackRock group"
    pub scope_phrase: Option<SegmentedPhrase>,    // "Irish ETF funds", "the ManCos"
    pub residual_terms: Vec<String>,              // Unclassified tokens for later
    
    // === Confidence & Diagnostics ===
    pub segmentation_confidence: f32,             // Overall confidence
    pub ambiguities: Vec<SegmentationAmbiguity>,  // Detected ambiguities
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentedPhrase {
    pub text: String,
    pub span: (usize, usize),          // Character offsets in raw_input
    pub confidence: f32,
    pub match_source: MatchSource,     // How we identified this
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchSource {
    ExactPhrase(String),               // Matched "trace UBO" exactly
    SemanticMatch { 
        score: f32,
        matched_phrase: String,
    },
    AliasMatch(String),                // Matched group alias "Allianz"
    Heuristic,                         // Stopword removal / position-based
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentationAmbiguity {
    pub text: String,
    pub possible_roles: Vec<(String, f32)>,  // [("verb", 0.7), ("scope", 0.5)]
    pub span: (usize, usize),
}
```

#### Segmentation Algorithm (Multi-Pass, Greedy)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  INPUT: "Trace the UBO chain for the Allianz Irish ETF funds"              │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  PASS 1: VERB EXTRACTION (highest priority)                                 │
│                                                                             │
│  Score n-grams against verb invocation_phrases corpus:                      │
│    "trace" → partial match                                                  │
│    "trace the UBO" → partial match                                          │
│    "trace the UBO chain" → EXACT MATCH (ubo.trace-chain, score: 0.98)      │
│                                                                             │
│  Greedy: Take longest match                                                 │
│  verb_phrase = SegmentedPhrase {                                            │
│      text: "Trace the UBO chain",                                           │
│      span: (0, 19),                                                         │
│      confidence: 0.98,                                                      │
│      match_source: ExactPhrase("trace the UBO chain")                      │
│  }                                                                          │
│  remaining = "for the Allianz Irish ETF funds"                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  PASS 2: GROUP EXTRACTION                                                   │
│                                                                             │
│  Score tokens against client_group_alias corpus:                            │
│    "for" → stopword, skip                                                   │
│    "the" → stopword, skip                                                   │
│    "Allianz" → EXACT MATCH on alias_norm (score: 1.0)                      │
│                                                                             │
│  group_phrase = SegmentedPhrase {                                           │
│      text: "Allianz",                                                       │
│      span: (28, 35),                                                        │
│      confidence: 1.0,                                                       │
│      match_source: AliasMatch("Allianz")                                   │
│  }                                                                          │
│  remaining = "for the Irish ETF funds"                                     │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  PASS 3: SCOPE EXTRACTION                                                   │
│                                                                             │
│  Remove stopwords ("for", "the"):                                           │
│    remaining = "Irish ETF funds"                                            │
│                                                                             │
│  Check scope_phrases corpus:                                                │
│    "Irish ETF funds" → no exact match                                       │
│    Semantic search → score 0.72 against "Irish funds"                       │
│                                                                             │
│  scope_phrase = SegmentedPhrase {                                           │
│      text: "Irish ETF funds",                                               │
│      span: (40, 55),                                                        │
│      confidence: 0.72,                                                      │
│      match_source: SemanticMatch { score: 0.72, matched: "Irish funds" }   │
│  }                                                                          │
│  residual_terms = []                                                        │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  OUTPUT:                                                                    │
│  UtteranceSegmentation {                                                    │
│      raw_input: "Trace the UBO chain for the Allianz Irish ETF funds",     │
│      verb_phrase: Some(...),      // → ubo.trace-chain                     │
│      group_phrase: Some(...),     // → group_id                             │
│      scope_phrase: Some(...),     // → ScopeFilter to resolve              │
│      residual_terms: [],                                                    │
│      segmentation_confidence: 0.90,                                         │
│      ambiguities: [],                                                       │
│  }                                                                          │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### The Segmenter Implementation

```rust
pub struct UtteranceSegmenter {
    verb_corpus: VerbCorpus,           // invocation_phrases + embeddings
    group_corpus: GroupAliasCorpus,    // client_group_alias + embeddings
    scope_corpus: ScopePhraseCorpus,   // scope_phrases + embeddings
    stopwords: HashSet<String>,
    
    // Thresholds
    verb_exact_threshold: f32,         // 0.95 for exact match
    verb_semantic_threshold: f32,      // 0.75 for semantic match
    group_semantic_threshold: f32,     // 0.80 for group alias
    scope_semantic_threshold: f32,     // 0.60 for scope (more lenient)
}

impl UtteranceSegmenter {
    /// Main entry point — segments raw input into tagged components
    pub fn segment(&self, input: &str) -> UtteranceSegmentation {
        let mut remaining = input.to_string();
        let mut ambiguities = vec![];
        
        // Pass 1: Extract verb phrase (greedy, longest match)
        let (verb_phrase, remaining) = self.extract_verb(&remaining);
        
        // Pass 2: Extract group phrase (exact > semantic)
        let (group_phrase, remaining) = self.extract_group(&remaining);
        
        // Pass 3: Extract scope phrase (remainder minus stopwords)
        let (scope_phrase, residual) = self.extract_scope(&remaining);
        
        UtteranceSegmentation {
            raw_input: input.to_string(),
            verb_phrase,
            group_phrase,
            scope_phrase,
            residual_terms: residual,
            segmentation_confidence: self.compute_overall_confidence(
                &verb_phrase, &group_phrase, &scope_phrase
            ),
            ambiguities,
        }
    }
    
    /// Session-aware segmentation — uses session state for implicit group
    pub fn segment_with_context(
        &self, 
        input: &str, 
        session: &SessionContext
    ) -> UtteranceSegmentation {
        let mut seg = self.segment(input);
        
        // If no group extracted, use session anchor
        if seg.group_phrase.is_none() {
            if let Some(anchor_name) = &session.anchor_group_name {
                seg.group_phrase = Some(SegmentedPhrase {
                    text: format!("(session: {})", anchor_name),
                    span: (0, 0),
                    confidence: 1.0,
                    match_source: MatchSource::Heuristic,
                });
            }
        }
        
        // Handle "those" / "them" references
        if seg.scope_phrase.as_ref().map(|s| 
            ["those", "them", "these", "it"].contains(&s.text.to_lowercase().as_str())
        ).unwrap_or(false) {
            if let Some(last_scope) = &session.last_scope_id {
                seg.scope_phrase = Some(SegmentedPhrase {
                    text: format!("(ref: {})", last_scope),
                    span: (0, 0),
                    confidence: 1.0,
                    match_source: MatchSource::Heuristic,
                });
            }
        }
        
        seg
    }
}
```

#### Fallback Chain (No Group Anchor)

```
When no explicit group is detected in the utterance:

1. session.anchor_group_id (most recent anchor)
2. Direct entity lookup (LEI match, exact name match)
3. Return Ambiguous with candidate groups

Example:
  User: "Show UBO for 549300ABCDEFG"
  → No group phrase detected
  → LEI lookup finds entity
  → Find which groups contain this entity
  → If single group: auto-anchor
  → If multiple groups: "This entity belongs to Allianz and BlackRock. Which context?"
```

---

### Layer 3: Entity Facts + Source Lineage

**Principle**: All entity attributes live on entity tables, with optional source lineage for attributes that may be alleged/disputed.

#### 3.1 Entity Tables (Unchanged Structure)

```sql
-- Core entity (unchanged)
entities (
    entity_id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    entity_type_id UUID REFERENCES entity_types
);

-- Company extension (unchanged structure, facts are authoritative)
entity_limited_companies (
    company_id UUID PRIMARY KEY,
    entity_id UUID UNIQUE REFERENCES entities,
    lei VARCHAR(20) UNIQUE,
    jurisdiction VARCHAR(3),
    is_fund BOOLEAN,
    manco_lei VARCHAR(20),
    -- ... existing GLEIF fields
);
```

#### 3.2 Entity Attribute Sources (Lineage with Typed Values)

Extends the shareholding_sources pattern to ALL entity attributes that may have multiple sources or disputed values.

```sql
CREATE TABLE entity_attribute_sources (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
    
    -- What attribute
    attribute_name TEXT NOT NULL,       -- 'name', 'jurisdiction', 'lei', 'is_fund'
    
    -- Typed values (exactly ONE populated based on value_type)
    attribute_value_text TEXT,
    attribute_value_bool BOOLEAN,
    attribute_value_num NUMERIC,
    attribute_value_date DATE,
    attribute_value_jsonb JSONB,
    value_type TEXT NOT NULL CHECK (value_type IN ('text', 'bool', 'num', 'date', 'jsonb')),
    
    -- Source provenance
    source TEXT NOT NULL,               -- 'gleif', 'client', 'companies_house', 'llm_scraper'
    source_document_id UUID,
    source_record_id TEXT,
    source_url TEXT,
    
    -- Confidence & verification
    confidence DECIMAL(3,2),
    verification_status TEXT NOT NULL DEFAULT 'alleged'
        CHECK (verification_status IN ('alleged', 'corroborated', 'verified', 'disputed', 'superseded')),
    
    verified_at TIMESTAMPTZ,
    verified_by TEXT,
    
    -- Lifecycle
    is_current BOOLEAN DEFAULT true,
    superseded_by UUID REFERENCES entity_attribute_sources(id),
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(entity_id, attribute_name, source, value_type)
);

CREATE INDEX idx_eas_entity_current 
    ON entity_attribute_sources(entity_id, attribute_name) 
    WHERE is_current = true;

CREATE INDEX idx_eas_disputed 
    ON entity_attribute_sources(entity_id) 
    WHERE verification_status = 'disputed';
```

#### 3.3 Best Known Promotion Rule

| Status | Promotion Behavior |
|--------|-------------------|
| VERIFIED (authoritative source) | Immediately promoted to entity table |
| CORROBORATED (multiple sources agree) | Promoted to entity table |
| ALLEGED (single non-authoritative) | NOT promoted, stays in lineage only |
| DISPUTED (sources conflict) | BLOCKS promotion, requires review |

---

### Layer 4: Semantic Agent Assistance (Client Group)

**Principle**: Client group tables exist ONLY for semantic resolution — mapping informal human language to entity scope. They contain NO authoritative facts.

#### 4.1 Client Group (Unchanged)

```sql
CREATE TABLE client_group (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    canonical_name TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

#### 4.2 Client Group Aliases

```sql
CREATE TABLE client_group_alias (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES client_group(id) ON DELETE CASCADE,
    
    alias TEXT NOT NULL,
    alias_norm TEXT NOT NULL,
    
    embedding VECTOR(384),
    embedder_id TEXT,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(group_id, alias_norm)
);

CREATE INDEX idx_cga_alias_norm ON client_group_alias(alias_norm);
CREATE INDEX idx_cga_embedding ON client_group_alias 
    USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);
```

#### 4.3 Client Group Entity (v4 - With Confidence)

```sql
CREATE TABLE client_group_entity (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES client_group(id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
    
    -- Role tags (split per GPT recommendation)
    role_tags_derived TEXT[] NOT NULL DEFAULT '{}',  -- Auto-computed from entity facts
    role_tags_curated TEXT[] NOT NULL DEFAULT '{}',  -- Manual/agent-added
    
    -- Membership context
    membership_type TEXT NOT NULL DEFAULT 'in_group',
    added_by TEXT NOT NULL DEFAULT 'manual',
    
    -- ═══════════════════════════════════════════════════════════════════
    -- REVIEW WORKFLOW (v4: added confidence + auto_confirmed)
    -- ═══════════════════════════════════════════════════════════════════
    review_status TEXT NOT NULL DEFAULT 'pending'
        CHECK (review_status IN ('pending', 'confirmed', 'rejected', 'auto_confirmed')),
    confidence DECIMAL(3,2),              -- 0.00 to 1.00
    confirmed_by_human BOOLEAN DEFAULT false,
    reviewed_by TEXT,
    reviewed_at TIMESTAMPTZ,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(group_id, entity_id)
);

-- Effective tags view
CREATE VIEW client_group_entity_searchable AS
SELECT 
    *,
    role_tags_derived || role_tags_curated AS role_tags_effective
FROM client_group_entity;

CREATE INDEX idx_cge_role_tags_derived 
    ON client_group_entity USING GIN (role_tags_derived);
CREATE INDEX idx_cge_role_tags_curated 
    ON client_group_entity USING GIN (role_tags_curated);
CREATE INDEX idx_cge_review_status
    ON client_group_entity(review_status) WHERE review_status = 'pending';
```

#### 4.4 Client Group Entity Tags

```sql
CREATE TABLE client_group_entity_tag (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES client_group(id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES entities(entity_id) ON DELETE CASCADE,
    
    tag TEXT NOT NULL,
    tag_norm TEXT NOT NULL,
    persona TEXT,                       -- NULL=universal, 'kyc', 'trading'
    
    embedding VECTOR(384),
    embedder_id TEXT,
    confidence DECIMAL(3,2) DEFAULT 1.0,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(group_id, entity_id, tag_norm, persona)
);

CREATE INDEX idx_cget_tag_norm ON client_group_entity_tag(group_id, tag_norm);
CREATE INDEX idx_cget_embedding ON client_group_entity_tag 
    USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);
```

---

### Layer 5: Agent Integration

#### 5.1 Unified Resolution Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  USER INPUT                                                                 │
│  "Show the ownership chain for the Allianz Irish ETF funds"                 │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  STAGE 1: UTTERANCE SEGMENTATION (formal parser)                            │
│                                                                             │
│  → verb_phrase:  "Show the ownership chain" → ubo.trace-chain              │
│  → group_phrase: "Allianz" → group_id                                       │
│  → scope_phrase: "Irish ETF funds" → ScopeFilter                           │
│  → residual:     []                                                         │
│  → confidence:   0.91                                                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┴───────────────┐
                    │                               │
                    ▼                               ▼
┌─────────────────────────────────┐ ┌─────────────────────────────────────────┐
│  PATTERN A (Runbook)            │ │  PATTERN B (Interactive)                │
│                                 │ │                                         │
│  Compile-time expansion:        │ │  Runtime resolution:                    │
│  → LSP validates                │ │  → Store in session.scopes              │
│  → Expands to concrete tuples   │ │  → Await scope.commit                   │
│  → Snapshots original+resolved  │ │  → Snapshot entity_ids                  │
│                                 │ │                                         │
└─────────────────────────────────┘ └─────────────────────────────────────────┘
                    │                               │
                    └───────────────┬───────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  STAGE 4: SCOPE RESOLUTION (shared logic)                                   │
│                                                                             │
│  Resolution order:                                                          │
│  1. Exact phrase match → scope_phrases.phrase_norm                         │
│  2. Role tag filter → role_tags_effective @> ARRAY[...]                    │
│  3. Semantic phrase match → scope_phrases.embedding                        │
│  4. Authoritative filter → Query entity tables directly                    │
│                                                                             │
│  Output: entity_ids = [uuid1, uuid2, uuid3, ...]                           │
│  If ambiguous: return ScopeResolution::Ambiguous                           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  STAGE 5: VERB EXECUTION                                                    │
│                                                                             │
│  Execute verb for each entity in resolved scope                             │
│  Retrieve facts from entity tables (NOT client_group_entity)               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  STAGE 6: LEARNING FEEDBACK (instrumented)                                  │
│                                                                             │
│  Create ResolutionEvent with:                                               │
│  - Segmentation output                                                      │
│  - Verb top-k + chosen                                                      │
│  - Group top-k + chosen                                                     │
│  - Scope top-k + chosen                                                     │
│  - User interaction flags                                                   │
│  - Outcome                                                                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### 5.2 Metrics Instrumentation

```rust
/// Structured logging for hit-rate analysis
#[derive(Debug, Serialize)]
pub struct ResolutionMetrics {
    pub session_id: Uuid,
    pub timestamp: DateTime<Utc>,
    
    // Input
    pub raw_input: String,
    pub segmentation: UtteranceSegmentation,
    
    // Verb resolution
    pub verb_resolved: Option<String>,
    pub verb_confidence: f32,
    pub verb_alternatives: Vec<(String, f32)>,
    
    // Scope resolution
    pub scope_resolved: bool,
    pub scope_entity_count: usize,
    pub scope_method: String,
    pub scope_confidence: f32,
    
    // Outcome
    pub was_ambiguous: bool,
    pub required_clarification: bool,
    pub user_accepted: bool,
    pub user_narrowed_scope: bool,
    pub user_corrected_verb: bool,
}

impl ResolutionMetrics {
    /// Key metrics to trend
    pub fn intent_success(&self) -> bool {
        self.verb_resolved.is_some() 
            && self.scope_resolved 
            && self.user_accepted
            && !self.user_corrected_verb
    }
    
    pub fn scope_precision(&self) -> bool {
        self.scope_resolved && !self.user_narrowed_scope
    }
    
    pub fn required_clarification(&self) -> bool {
        self.was_ambiguous || self.required_clarification
    }
}
```

---

## Migration Plan

### Phase 0: Audit Current State

Before any code changes:
- [ ] Audit `client_group_entity` columns currently in use
- [ ] Identify handlers reading fact columns (lei, jurisdiction, etc.)
- [ ] Document current learning infrastructure
- [ ] Verify role_tags GIN index exists

### Phase 1: Add Entity Attribute Sources

**Migration: `060_entity_attribute_sources.sql`**

- Create `entity_attribute_sources` table with typed values
- Backfill from GLEIF data with source='gleif', status='verified'
- Create lineage view

**No breaking changes** — additive only.

### Phase 2: Slim Client Group Entity

**Migration: `061_slim_client_group_entity.sql`**

- Remove fact columns
- Split role_tags into derived/curated
- Add confidence + auto_confirmed fields
- Create `client_group_entity_searchable` view

**Breaking change**: Update handlers.

### Phase 3: Backfill Role Tags

**Migration: `062_backfill_role_tags.sql`**

- Populate role_tags_derived from entity facts

### Phase 4: Add Scope Learning Tables

**Migration: `063_scope_learning_tables.sql`**

- Create `scope_phrases`, `scope_resolution_log`, `scope_examples`
- Add embedding indexes

### Phase 5: Add Snapshot Tables

**Migration: `064_scope_snapshots.sql`**

- Create `scope_snapshots` table with immutability trigger
- Create `resolution_events` table

### Phase 6: Implement Utterance Segmenter

- Create `UtteranceSegmenter` with formal contract
- Add verb/group/scope corpora
- Integrate with Candle embeddings

### Phase 7: Implement Dual-Pattern Scope Resolution

- Add `scope.define` (Pattern A) compiler directive
- Add `scope.anchor`, `scope.resolve`, `scope.narrow`, `scope.commit`, `scope.refresh` (Pattern B)
- Implement `SessionContext` with scope storage
- Add linter rules S001-S007

### Phase 8: Update Verb Handlers

- Update handlers to use `role_tags_effective`
- Join entity tables for facts
- Add scope resolution to intent pipeline

### Phase 9: Test Harness & Metrics

- Create `ScopeTestScenario` structure
- Add metrics instrumentation
- Integrate with xtask test runner
- Add training data export

---

## Open Questions (Resolved)

| Question | Resolution |
|----------|------------|
| Role Tags Derivation | Split: `role_tags_derived` (auto) + `role_tags_curated` (manual) |
| Dispute Resolution Workflow | Flag in agent response + review queue |
| Scope Caching | Immutable snapshots after scope.commit |
| Cross-Group Entities | Linter rule S002: forbidden unless `:global true` |
| Lineage Granularity | Track for sensitive attributes + any disputed |
| Best Known Promotion | VERIFIED/CORROBORATED → promote, ALLEGED → no, DISPUTED → blocks |
| Ambiguity Handling | Typed modes: `:strict`, `:interactive`, `:greedy` |
| Dual Pattern (v3) | Both compile-time (runbooks) and runtime (interactive) |
| Utterance Segmentation (v3) | Formal parser with explicit contract |
| **DSL Materialization (v4)** | Macro form → expanded form (Option A default) |
| **Pipeline Invariants (v4)** | Parse → Resolve → Lint → DAG → Execute |
| **Snapshot Contract (v4)** | `scope_snapshots` + `resolution_events` tables |
| **Membership Confidence (v4)** | `confidence` + `auto_confirmed` + `confirmed_by_human` |
| **Determinism Policy (v4)** | Ordering rule, replay semantics, refresh semantics |

---

## Appendix A: Table Summary

| Table | Layer | Purpose | Contains Facts? |
|-------|-------|---------|-----------------|
| `entities` | Entity | Core entity record | YES (authoritative) |
| `entity_limited_companies` | Entity | Company attributes | YES (authoritative) |
| `entity_attribute_sources` | Entity | Source lineage | Tracks sources of facts |
| `shareholdings` | Entity | Ownership facts | YES (with multi-source) |
| `entity_relationships` | Entity | Graph edges | YES (authoritative) |
| `client_group` | Semantic | Brand/grouping | NO (metadata only) |
| `client_group_alias` | Semantic | Name resolution | NO (search index) |
| `client_group_entity` | Semantic | Membership + search | NO (search tags only) |
| `client_group_entity_tag` | Semantic | Phrase resolution | NO (search index) |
| `scope_phrases` | Learning | Phrase→filter mapping | NO (learning data) |
| `scope_resolution_log` | Learning | Resolution audit | NO (learning data) |
| `scope_examples` | Learning | Training data | NO (learning data) |
| `scope_snapshots` | Snapshot | Immutable resolution records | NO (audit/replay) |
| `resolution_events` | Learning | Full decision log | NO (learning data) |

---

## Appendix B: Comparison with Existing Patterns

### Shareholdings (Current Best Practice)

```
shareholdings                    shareholding_sources
─────────────                    ────────────────────
subject_entity_id                shareholding_id (FK)
owner_entity_id                  source
ownership_pct ◄── Best known     ownership_pct ◄── Per source
source                           verification_status
verification_status              document_ref
```

### Proposed Entity Attributes (Extends Pattern)

```
entity_limited_companies         entity_attribute_sources
────────────────────────         ────────────────────────
entity_id                        entity_id (FK)
lei ◄── Best known               attribute_name
jurisdiction ◄── Best known      attribute_value_* ◄── Per source (typed)
is_fund ◄── Best known           source, confidence
                                 verification_status
```

---

## Appendix C: Agent Conversation Example (v4)

```
User: "Work on Allianz"
Agent: [scope.anchor :group "Allianz"]
       [Sets session.anchor_group_id]
       "Session anchored to Allianz Group (47 entities). What would you like to do?"

User: "Show me the Irish funds"
Agent: [Segmentation: verb="show", group=(session), scope="Irish funds"]
       [scope.resolve :desc "Irish funds" :limit 50 :as @s1]
       [Finds 12 entities via role_tags_effective @> '{IRISH,FUND}']
       "Found 12 Irish funds in the Allianz group:
        - Allianz Ireland UCITS Platform (LEI: 549300...)
        - Allianz Dublin ETF Fund (LEI: 549300...)
        ..."

User: "Just the ETFs"
Agent: [scope.narrow @s1 :filter {:tags ["ETF"]} :as @s2]
       "Narrowed to 4 ETF funds..."

User: "Trace UBO for those"
Agent: [scope.commit :scope @s2 :as @s2_final]  ← Creates immutable snapshot
       [Segmentation: verb="trace UBO", scope="those" → @s2_final]
       [ubo.trace-chain :scope @s2_final]
       [MACRO EXPAND → 4 tuples]
       [Retrieves facts from entity_limited_companies]
       "UBO chains for 4 ETF funds:
        
        Allianz Dublin ETF Fund (IE):
        └── 100% Allianz Ireland Holdings (IE)
            └── 100% Allianz Asset Management (DE)
                └── 100% Allianz SE (DE) ← Ultimate Parent
        ..."

User: "Companies House says the Ireland Holdings is only 50% owned"
Agent: [Records in entity_attribute_sources with source='user_report']
       [Marks existing ownership as 'disputed']
       "I've noted the discrepancy. The ownership of Allianz Ireland Holdings 
        is now marked as disputed:
        - GLEIF says: 100% owned by Allianz Asset Management
        - Companies House (reported): 50%
        Would you like me to create a review task?"
```

---

## Appendix D: Prompt Learning Integration

### DSL Verbs for Scope Teaching

```yaml
verbs:
  - verb: teach-scope
    description: "Teach the agent a new scope phrase"
    behavior: plugin
    handler: AgentTeachScopeOp
    invocation_phrases:
      - "remember that ... means"
      - "when I say ... I mean"
      - "teach scope"
    args:
      - name: phrase
        type: string
        required: true
      - name: group
        type: string
        required: false
      - name: filter
        type: scope_filter
        required: false
      - name: entities
        type: array
        items: uuid
        required: false
    effects:
      - "Adds entry to scope_phrases table"
      - "Queues embedding computation"
      - "Logs to scope_resolution_log"

  - verb: unteach-scope
    description: "Remove a learned scope phrase"
    behavior: plugin
    handler: AgentUnteachScopeOp
    args:
      - name: phrase
        type: string
        required: true
      - name: group
        type: string
        required: false
      - name: reason
        type: string
        required: false
```

### Test Harness Integration

```rust
pub struct ScopeTestScenario {
    pub name: String,
    pub description: String,
    pub group_context: Option<String>,
    pub test_cases: Vec<ScopeTestCase>,
}

pub enum ScopeExpectation {
    ExactEntities(Vec<Uuid>),
    FilterApplied { min_entities: usize, required_jurisdictions: Option<Vec<String>> },
    Ambiguous { min_candidates: usize },
    Unresolvable,
}
```

---

## Appendix E: Implementation Contract (v4)

This appendix provides the concrete artifacts needed for Claude Code implementation.

### E.1 EBNF Grammar Additions

```ebnf
(* Scope verb extensions *)
scope_stmt     ::= scope_anchor | scope_define | scope_resolve 
                 | scope_narrow | scope_commit | scope_refresh

scope_anchor   ::= '(' 'scope.anchor' ':group' string ')'

scope_define   ::= '(' 'scope.define' symbol 
                       ':group' string? 
                       ':filter' filter_expr 
                       ':limit' integer ')'

scope_resolve  ::= '(' 'scope.resolve' 
                       ':desc' string 
                       ':limit' integer 
                       ':mode'? mode_enum
                       ':global'? boolean
                       ':as' symbol ')'

scope_narrow   ::= '(' 'scope.narrow' symbol 
                       ':filter' filter_expr 
                       ':as' symbol ')'

scope_commit   ::= '(' 'scope.commit' ':scope' symbol ':as' symbol ')'

scope_refresh  ::= '(' 'scope.refresh' symbol ':as' symbol ')'

(* Verb arg extensions *)
verb_arg       ::= ... | ':scope' symbol | ':entity' symbol

(* Types *)
symbol         ::= '@' identifier
mode_enum      ::= 'strict' | 'interactive' | 'greedy'
filter_expr    ::= '{' (filter_key filter_value)* '}'
filter_key     ::= ':jurisdiction' | ':is-fund' | ':tags' | ':role-tags' | ...
```

### E.2 Rust IR Structs

```rust
// === AST Nodes ===

#[derive(Debug, Clone)]
pub enum ScopeNode {
    Anchor { group: String },
    Define { 
        name: Symbol, 
        group: Option<String>, 
        filter: ScopeFilter,
        limit: u32,
    },
    Resolve {
        desc: String,
        limit: u32,
        mode: ResolutionMode,
        global: bool,
        as_name: Symbol,
    },
    Narrow {
        source: Symbol,
        filter: ScopeFilter,
        as_name: Symbol,
    },
    Commit {
        scope: Symbol,
        as_name: Symbol,
    },
    Refresh {
        old_scope: Symbol,
        as_name: Symbol,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum ResolutionMode {
    Strict,
    Interactive,
    Greedy,
}

// === Scope Descriptor (what was requested) ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeDescriptor {
    pub group_id: Uuid,
    pub group_anchor_name: String,
    pub description: Option<String>,
    pub filter_applied: Option<ScopeFilter>,
    pub limit_requested: u32,
    pub mode: ResolutionMode,
}

// === Scope Snapshot Reference ===

#[derive(Debug, Clone)]
pub struct ScopeSnapshotRef {
    pub snapshot_id: Uuid,
    pub entity_ids: Vec<Uuid>,           // Ordered deterministically
    pub created_at: DateTime<Utc>,
    pub is_committed: bool,
}

// === Resolution Event (learning payload) ===

#[derive(Debug, Clone, Serialize)]
pub struct ResolutionEventPayload {
    pub session_id: Uuid,
    pub raw_input: String,
    pub segmentation: UtteranceSegmentation,
    
    pub verb_top_k: Vec<(String, f32)>,
    pub verb_chosen: Option<String>,
    
    pub group_top_k: Vec<(Uuid, String, f32)>,
    pub group_chosen_id: Option<Uuid>,
    
    pub scope_top_k: Vec<ScopeCandidate>,
    pub scope_snapshot_id: Option<Uuid>,
    
    pub was_ambiguous: bool,
    pub user_overrides: UserOverrides,
    pub execution_success: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct UserOverrides {
    pub verb: bool,
    pub group: bool,
    pub narrowed_scope: bool,
    pub widened_scope: bool,
    pub confirmed: bool,
}
```

### E.3 DB Schema Summary

| Migration | Table | Purpose |
|-----------|-------|---------|
| 060 | `entity_attribute_sources` | Typed lineage for entity attributes |
| 061 | `client_group_entity` (ALTER) | Add confidence, auto_confirmed |
| 062 | (backfill) | Populate role_tags_derived |
| 063 | `scope_phrases`, `scope_resolution_log`, `scope_examples` | Learning tables |
| 064 | `scope_snapshots` | Immutable resolution records |
| 065 | `resolution_events` | Full decision log for learning |

### E.4 Determinism Policy Summary

| Rule | Specification |
|------|---------------|
| **Ordering** | `selected_entity_ids` ordered by: (1) score DESC, (2) entity_id ASC |
| **Replay** | Always uses `snapshot.selected_entity_ids`, never re-resolves |
| **Refresh** | Explicit `scope.refresh` required; creates new snapshot with `parent_snapshot_id` |
| **Drift** | Compare fingerprints; emit WARNING but execute with snapshot data |
| **Immutability** | `scope_snapshots` UPDATE blocked by DB trigger |

---

*End of Architecture Proposal v4*
