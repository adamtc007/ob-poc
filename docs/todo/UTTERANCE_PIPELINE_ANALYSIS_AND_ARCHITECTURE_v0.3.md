# Utterance → DSL Pipeline: Root Cause Analysis & Architecture Specification v0.3

**Date:** 2026-03-06  
**Status:** Consolidated Research + Architecture Draft  
**Current accuracy:** 38.81% (52/134 test utterances)  
**Target:** ≤2 prompts to execution, 80%+ first-attempt hit rate  

---

## Part I: Root Cause Analysis

### RC-1: Verb-First Pipeline — The Foundational Error (Critical)

The pipeline treats every utterance as a verb-selection problem:

```
utterance → find verb (1,123 candidates) → extract args → DSL → execute
```

The user isn't asking for a verb. They're asking for an **outcome**. "Onboard Allianz" isn't a request to invoke `cbu.create` — it's a request to achieve a state where Allianz is onboarded, which may require 15 verbs across 4 domains, research steps, human approvals, and data that doesn't exist yet.

The verb-first pipeline forces a commitment (which verb?) before understanding the problem (what outcome?). This is why hit rates collapse as vocabulary grows — more verbs means more premature commitments to the wrong verb.

### RC-2: Data Model Is Invisible to Verb Selection (Critical)

The design document describes "Tandem Reasoning" where Data Models and Verbs jointly guide resolution. This isn't happening. The data model enters ONLY as late negative filters:

- **SemReg ContextEnvelope:** prunes verbs by ABAC/tier/entity-kind — removes options but never boosts the correct answer
- **ECIR NounIndex:** covers ~20 domain nouns — but 1,123 verbs across 134 domains means most utterances skip ECIR entirely
- **AffinityGraph:** has bidirectional verb↔data linkage but it's not wired to verb scoring at all

When the user says "calculate the UBOs for this CBU", entity linking knows the subject is a CBU, and AffinityGraph knows which verbs operate on CBUs. But this knowledge never reaches the verb ranker. Result: `ubo.calculate` gets 14% hit rate (2/14 test cases).

### RC-3: 1,123 Verbs in 384-dim Embedding Space (Critical)

BGE-small-en-v1.5 produces 384-dimensional vectors. With 15,940 intent patterns across 1,123 verbs, the embedding space is overcrowded for custody banking vocabulary. The 0.05 ambiguity margin means any utterance with general banking language hits near-tied scores across multiple domains:

- "verify it" → `allegation.verify` instead of `document.verify`
- "run the check" → `discovery.run` instead of `screening.sanctions`
- "show the structure" → `cbu.show` instead of `ubo.list-owners`
- "create a new one" → `trading-profile.create-new-version` instead of `cbu.create`

**208 verbs start with "list"**, 84 with "create". In a 384-dim space these are essentially the same vector.

### RC-4: Vocabulary Rot — 84 Exact Collisions, 343 Intra-Domain Duplicates, 134 Domains (Critical)

Systematic scan of all 1,123 verbs reveals structural vocabulary problems at every level.

**84 exact phrase collisions** — the identical phrase maps to 2-4 verbs:

- `"who owns this entity"` → 4 verbs: `graph.ancestors`, `control.identify-ubos`, `bods.list-ownership`, `ubo.list-by-subject`
- `"trace ownership"` → 3 verbs: `control.trace-chain`, `gleif.trace-ownership`, `bods.discover-ubos`
- `"subscribe cbu to product"` → 3 verbs: `contract.subscribe`, `cbu.add-product`, `product-subscription.subscribe`
- `"run sanctions screening"` → 2 verbs: `screening.sanctions`, `case-screening.run`

**343 intra-domain description-similar pairs** (>0.65 SequenceMatcher ratio):

- `movement.transfer-in` vs `transfer-out` = 0.96 description similarity
- `trading-profile` (47 verbs): 30+ near-duplicate pairs
- `entity.ensure-limited-company` / `ensure-proper-person` / `ensure-trust-discretionary` / `ensure-partnership-limited` = 0.80-0.91 — same verb with a type parameter

**185 cross-domain phrase overlap pairs** (Jaccard >0.5): reference data domains (`client-type`, `screening-type`, `settlement-type`, etc.) share 70-90% of invocation phrases because they were template-generated with the same word banks.

**Domain fragmentation:** 134 domains, 36 with ≤3 verbs. Concept fragmentation: "trace ownership" is implemented across `control`, `ownership`, `ubo`, and `gleif` domains.

**65 verbs with ≤3-word descriptions** ("Update regulator", "Deactivate a currency") — pure embedding noise.

### RC-5: No LLM in Verb Selection (High)

The LLM is sandboxed to argument extraction AFTER verb selection. The verb selection itself is pure embedding similarity + deterministic ECIR. At 100 verbs this was fine. At 1,123, embedding similarity cannot discriminate what an LLM could trivially distinguish from a 5-option multiple choice. "Run a check on this entity" → a human or LLM immediately narrows to screening/compliance. The embedding search returns hits from screening, discovery, ownership, control, and document domains.

### RC-6: Instance/Structure Observation Plane Friction (High)

**This is the friction you identified.** The pipeline has one execution model: utterance → resolve entity UUID → plug into verb args → execute against instance. This is correct for KYC and Onboarding where the user operates on specific entity instances ("onboard Allianz", "check HSBC sanctions").

But SemOS Data Management operates on a different plane entirely:

| Dimension | Instance Mode (KYC/Onboarding) | Structure Mode (SemOS Data Mgmt) |
|-----------|-------------------------------|----------------------------------|
| Subject | Specific entity (UUID) | Entity type, taxonomy, schema |
| Args | `deal-id: uuid`, `entity-id: uuid` | `entity-type: string`, `domain: string` |
| Resolution | Entity linker → UUID lookup | Type name → registry metadata |
| Verbs | `deal.get`, `cbu.create`, `screening.sanctions` | `schema.entity.describe`, `registry.list-objects` |
| State change | Modifies entity instance rows | Read-only (or modifies metadata/registry) |
| "Show me deals" | Show deal instances (needs filter/scope) | Show what a deal IS (schema, fields, verbs) |

The current pipeline patches this with `data_management_rewrite()` (orchestrator.rs lines 240-258) — a string-level hack that rewrites "show me deal record" to "describe entity schema for deal" before it enters the pipeline. This has three problems:

1. **Entity linker runs first** (line 339) and may resolve "deal" to a specific deal entity UUID, setting `dominant_entity_kind` and over-constraining SemReg to instance verbs — before the rewrite even fires
2. **The rewrite is substring matching** (`lower.contains("deal")`) — brittle, invisible to the Sage, not discoverable
3. **ECIR NounIndex maps "deal" to `deal.list`/`deal.get`** (instance verbs), not to `schema.entity.describe` — the deterministic tier fights the rewrite

The deeper issue: **the observation plane is not modeled anywhere in the type system.** The OutcomeIntent, the verb registry, the entity resolver — none of them distinguish "operating on an instance" from "operating on the structure of instances." This distinction is implicit in `stage_focus` string matching, which is fragile and can't compose with the Sage/Coder architecture.

**What's needed:** An explicit `ObservationPlane` enum that the Sage sets and the Coder respects:

```rust
pub enum ObservationPlane {
    /// Operating on specific entity instances identified by UUID.
    /// Entity linker resolves names → UUIDs. Verbs require *-id args.
    Instance,
    /// Operating on entity types, schemas, taxonomies, data models.
    /// No UUID resolution. Verbs take type names as strings.
    /// Read-only by default (metadata exploration).
    Structure,
    /// Operating on the semantic registry itself — snapshots, changesets,
    /// governance artifacts. The Stewardship Agent plane.
    Registry,
}
```

This enum propagates from the Sage (who knows the user is exploring structure vs operating on instances) through to the Coder (who picks verbs accordingly) and the entity resolver (who skips UUID resolution in Structure mode).

### RC-7: No Scribe/Sage ↔ Coder Separation (High)

The pipeline has one mode: utterance → verb → execute. No conversational confirmation step. No decomposition of complex outcomes. No separation between understanding intent and generating DSL.

The `NeedsClarification` outcome exists but only triggers on the ambiguity gate (margin < 0.05). It doesn't cover confidently-wrong verb selection (score > 0.65 but wrong verb). "This person died last month" gets routed to some verb with high confidence — but it could mean `ubo.mark-deceased`, `entity.update`, or `screening.adverse-media`.

### RC-8: Entry Point Fragmentation Below the Surface (Medium)

HTTP-level unification is done (`POST /api/session/:id/input` with 410 Gone on legacy). But:

- 3 `IntentPipeline` instantiations within a single utterance (orchestrator.rs lines 421, 668, 1455)
- No `TraceId` threading from HTTP ingress through to REPL execution
- MCP path (`mcp/handlers/core.rs:1660`) calls `handle_utterance()` directly, bypassing `agent_service.rs::build_orchestrator_context()` factory — potentially different defaults for goals, stage_focus, agent_mode

### RC-9: Intent Polarity Is Not Exploited — The Read/Write Signal (Medium-High)

A large proportion of production utterances are **read operations**: "show me the CBUs", "who are the beneficial owners?", "what documents are missing?", "list all share classes". These utterances contain deterministic surface clue words that signal the user wants to **inspect**, not **mutate**.

The current pipeline ignores this signal entirely. "Show me the CBUs" enters the same 1,123-verb search space as "create a new CBU." But read-intent utterances can only match ~429 read verbs (38% of the total) — the other 694 write verbs are impossible targets. This is a free 62% reduction in search space that fires from a prefix check on the first word.

**Clue word taxonomy (deterministic, zero-cost):**

| Intent Polarity | Clue Words | Action |
|----------------|------------|--------|
| **Inspect** | show, list, what, display, describe, view | Browse/enumerate |
| **Query** | who, which, where, how many, find, search, get | Targeted lookup |
| **Assess** | check, verify, validate, audit, analyze, review | Evaluation (may or may not mutate) |
| **Report** | report, summarize, summary, status, count, compute | Generate output |
| **Trace** | trace, discover, identify, who controls, who owns | Navigate relationships |
| **Create** | create, add, new, set up, establish, onboard, open | New entities |
| **Modify** | update, change, modify, assign, set, configure | Change existing |
| **Remove** | delete, remove, cancel, end, terminate, close, revoke | Delete/deactivate |
| **Transfer** | upload, import, export, transfer, move, send | Data movement |

The first five rows are **read-intent** (no state mutation). The last four are **write-intent**. "Assess" is ambiguous — `check_sanctions` triggers a screening run (write) but `check_status` is read-only — so Assess needs a second signal (domain) to classify.

**Why this matters beyond search space reduction:**

1. **Read operations are safe.** The Sage can handle many reads entirely in Research mode — no Coder, no REPL, no confirmation needed. "Show me the CBUs" in data management mode? Pure Sage — query the registry, format the response, done.

2. **Combined with observation plane, this creates a 2×2 classification matrix that the Sage computes deterministically before any LLM call:**

| | Instance Plane | Structure Plane |
|---|---|---|
| **Read** | Query instance data (e.g., `deal.list`, `document.for-entity`) | Query schemas/types (e.g., `schema.entity.describe`) — **Pure Sage territory** |
| **Write** | Modify instances (e.g., `cbu.create`, `screening.sanctions`) — **Requires Coder + confirmation** | Modify registry (e.g., changeset authoring) — **Requires Coder + governance** |

The Read+Structure quadrant is the highest-volume production scenario ("what does this look like?", "show me the fields", "describe the deal schema") and it NEVER needs the Coder. The Sage handles it entirely from registry metadata and schema introspection.

3. **Read-intent verbs have less collision risk.** 208 verbs start with "list" — but within a single domain, there are typically only 2-5 list verbs. Once the Sage has both the polarity (read) and the domain, the Coder's verb lookup is near-deterministic.

**Verb distribution confirms this is impactful:**
- 429 verbs (38%) are read-only operations
- `registry` domain: 26 verbs, 100% read — entirely Sage territory
- `lifecycle`: 10/16 read (62%), `gleif`: 9/16 read (56%), `control`: 7/16 read (44%)
- Write-heavy domains like `trading-profile` (4R/43W) benefit most from polarity filtering — a read utterance against this domain drops from 47 to 4 candidates

---

## Part II: Architecture — Sage / Coder Split

### The Foundational Claim

The user is not asking for a verb. The user is asking for an **outcome** on a particular **observation plane**. The pipeline must separate:

1. **Understanding** what outcome the user wants (Sage)
2. **Executing** that outcome as DSL (Coder)

These require fundamentally different capabilities and must be architecturally separated with a hard boundary.

### Two Personas, One Hard Boundary

**The Sage** understands what the user wants. It speaks in outcomes, not verbs. It never generates DSL, never touches the REPL, never executes anything. Its job:

- **Listen** — parse the utterance in context (entity, workflow, conversation history)
- **Classify the observation plane** — is this instance-mode, structure-mode, or registry-mode?
- **Interpret** — identify the outcome, not the verb
- **Confirm** — play back the understood outcome in human terms
- **Decompose** — break complex outcomes into ordered steps
- **Research** — gather information (GLEIF, schema, registry) without REPL commitment

**The Coder** executes what the Sage decides. It speaks in DSL, verbs, arguments, and REPL state. Its job:

- **Map** — translate a confirmed OutcomeStep into the correct verb(s) + arguments
- **Resolve** — entity resolution (UUID mode) or type resolution (structure mode) based on observation plane
- **Generate** — produce syntactically valid, governance-compliant DSL
- **Execute** — run through the REPL pipeline
- **Report** — return results to the Sage

**The hard boundary** is enforced architecturally:

```
┌─────────────────────────────────────────────────────────┐
│                        SAGE                              │
│                                                          │
│  Understands outcomes. Speaks human. Never writes DSL.   │
│                                                          │
│  HAS ACCESS TO:                                          │
│    - Conversation history                                │
│    - Entity linking (read-only)                          │
│    - AffinityGraph (read-only)                           │
│    - SemReg context resolution (read-only)               │
│    - GLEIF/Companies House APIs (read-only)              │
│    - Registry metadata (read-only)                       │
│    - Schema introspection (read-only)                    │
│                                                          │
│  CANNOT ACCESS:                                          │
│    - REPL write paths                                    │
│    - DSL generation                                      │
│    - Verb argument schemas (Coder's concern)             │
│    - Execute endpoints                                   │
│                                                          │
│  ═══════════ HARD BOUNDARY ═══════════════════════════   │
│                                                          │
│                        CODER                             │
│                                                          │
│  Executes instructions. Speaks DSL. Never interprets.    │
│                                                          │
│  HAS ACCESS TO:                                          │
│    - Verb registry + argument schemas                    │
│    - Entity resolution (UUID mode)                       │
│    - Type resolution (structure mode)                    │
│    - DSL parser, compiler, REPL                          │
│    - Governance surface (SessionVerbSurface)             │
│                                                          │
│  CANNOT ACCESS:                                          │
│    - Conversation history                                │
│    - User utterance text (only OutcomeStep)              │
│    - Research tools                                      │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

### Why This Fixes the Hit Rate

The current pipeline asks one component to solve a 1,123-way verb classification problem with embedding similarity as the only signal. The Sage/Coder split decomposes this into two much simpler problems:

**Sage problem:** "What outcome in which domain on which observation plane?" — a ~20-30 way classification that LLMs are naturally good at. The Sage doesn't need to know verb FQNs. It identifies: domain_concept="screening", action=Assess, plane=Instance, subject="HSBC Holdings".

**Coder problem:** "Given domain=screening + action=Assess + params={entity, type=sanctions}, which verb?" — a deterministic lookup across 3-10 candidate verbs, using structured parameter overlap. No embedding search needed.

### Why This Fixes the Observation Plane Friction

The Sage classifies the observation plane BEFORE anything else happens:

```
User: "Show me the deal record"

SAGE (data management context):
  Observation plane: Structure (no UUID, no instance targeting)
  Domain: deal
  Action: Investigate
  → OutcomeStep { plane: Structure, domain: "deal", action: Investigate }

CODER receives: plane=Structure
  → Searches ONLY schema.* / registry.* verbs
  → Entity resolver: skip UUID resolution, use type-name resolution
  → Result: schema.entity.describe :entity-type "deal"
```

Versus:

```
User: "Show me the deal record"

SAGE (KYC context, deal-id in session):
  Observation plane: Instance (session has active deal)
  Domain: deal  
  Action: Investigate
  → OutcomeStep { plane: Instance, domain: "deal", action: Investigate,
      params: { deal-id: <uuid from session> } }

CODER receives: plane=Instance
  → Searches deal.* verbs
  → Entity resolver: UUID mode
  → Result: deal.get :deal-id <uuid>
```

Same utterance, different outcome, determined by the Sage's understanding of context — not a string-level rewrite hack. The `data_management_rewrite()` function becomes unnecessary because the observation plane is a first-class dimension in the OutcomeIntent.

### The Outcome Model

```rust
/// What the user wants to achieve — domain terms, not DSL terms.
pub struct OutcomeIntent {
    /// Human-readable summary of the outcome
    pub summary: String,
    /// Which observation plane (Instance, Structure, Registry)
    pub plane: ObservationPlane,
    /// Read or Write intent (from pre-classification)
    pub polarity: IntentPolarity,
    /// Domain concept (e.g., "screening", "fund_structure", "corporate_research")
    pub domain_concept: String,
    /// Action category
    pub action: OutcomeAction,
    /// Entity context — who/what is this about
    pub subject: Option<EntityRef>,
    /// Decomposed steps (for complex outcomes)
    pub steps: Vec<OutcomeStep>,
    /// Sage's confidence in this interpretation
    pub confidence: SageConfidence,
    /// Pending clarifications before proceeding
    pub pending_clarifications: Vec<Clarification>,
}

pub enum ObservationPlane {
    /// Operating on entity instances (UUID-based). KYC, Onboarding, Deal mgmt.
    Instance,
    /// Operating on entity types, schemas, taxonomies. SemOS Data Management.
    Structure,
    /// Operating on the semantic registry. Stewardship.
    Registry,
}

pub struct OutcomeStep {
    pub description: String,
    pub plane: ObservationPlane,
    pub domain_concept: String,
    pub action: OutcomeAction,
    pub resolved_params: BTreeMap<String, ResolvedParam>,
    pub requires_confirmation: bool,
    pub depends_on: Vec<usize>,
    pub execution_mode: ExecutionMode,
}

pub enum ExecutionMode {
    /// Sage handles — read-only research, exploration, data gathering.
    /// GLEIF lookup, schema browsing, registry queries. No REPL.
    Research,
    /// Coder handles — DSL generation and REPL execution.
    Execute,
}

pub enum SageConfidence {
    High,      // Proceed (confirm if destructive)
    Medium,    // Confirm always
    Low,       // Clarify with user
    Unclear,   // Need more information
}

pub enum OutcomeAction {
    Create, Investigate, Modify, Assess, Link,
    Verify, Remove, Transfer, Configure, Report,
}
```

### Conversation Flow: Research Mode (Sage-Only)

```
User:   "What does the Allianz corporate tree look like?"

SAGE:
  Plane: Structure (exploratory, no instance targeting)
  Domain: corporate_research
  Action: Investigate
  Mode: Research (no REPL needed)
  
  → Calls GLEIF API (read-only Sage tool)
  → Formats result

SAGE → User:
  "Allianz SE has 14 entities across 3 levels:
   - Level 1: Allianz SE (DE)
   - Level 2: Allianz Global Investors GmbH (DE), ...
   - Level 3: AGI Luxembourg SA (LU), AGI UK Ltd (GB), ...
   
   Want me to import this tree into the system?"
```

No REPL touched. No verb selected. Only if user says "import it" does the Sage create an OutcomeStep for the Coder.

### Conversation Flow: Complex Multi-Step

```
User:   "Onboard the new client — Luxembourg UCITS, Pictet as ManCo"

SAGE:
  Plane: Instance (will create real entities)
  Action: Create (composite)
  Confidence: Medium (client name not provided)

SAGE → User:
  "I'll set up a Luxembourg UCITS with Pictet as ManCo. Fund name?"

User:   "Pictet Total Return Diversified"

SAGE decomposes:
  1. [Research, Structure] Check GLEIF for Pictet entities
  2. [Execute, Instance] Create client group from GLEIF data
  3. [Execute, Instance] Create CBU: "Pictet Total Return Diversified", LU
  4. [Execute, Instance] Set up UCITS fund hierarchy (macro expansion)
  5. [Execute, Instance] Assign Pictet as ManCo
  6. [Execute, Instance] Open KYC case
  7. [Execute, Instance] Generate document requirements
  8. [Research, Instance] Run initial sanctions screening

SAGE → User:
  "Here's the onboarding plan:
   1. Import Pictet corporate tree from GLEIF
   2. Create client group and link entities
   3-7. Create CBU, fund structure, roles, KYC case, documents
   8. Run initial screening
   
   Steps 1 and 8 are research only (no system changes).
   Steps 2-7 create records. Proceed?"
```

Each step crosses the hard boundary individually. The Sage tracks progress. The Coder executes mechanically.

### Conversation Flow: Same Utterance, Different Plane

```
User (in Data Management mode): "Show me documents"

SAGE:
  Plane: Structure (data management context, no instance targeting)
  → OutcomeStep { plane: Structure, domain: "document",
      action: Investigate }

CODER:
  → plane=Structure → search schema.* verbs only
  → schema.entity.describe :entity-type "document"
  → Returns: fields, relationships, available verbs for document entity type
```

```
User (in KYC mode, entity "HSBC" in scope): "Show me documents"

SAGE:
  Plane: Instance (KYC context, entity in scope)
  → OutcomeStep { plane: Instance, domain: "document",
      action: Investigate, subject: HSBC entity }

CODER:
  → plane=Instance → search document.* verbs
  → document.for-entity :entity-id <HSBC uuid>
  → Returns: actual document list for HSBC
```

The observation plane eliminates the `data_management_rewrite()` hack because the Sage understands context.

### The Sage's Three-Signal Pre-Classification (Deterministic, Before LLM)

Before the Sage calls the LLM for outcome classification, it computes three signals deterministically from the utterance surface and session context. These three signals together eliminate 50-80% of the verb space at zero cost:

**Signal 1: Observation Plane** (from session context + instance-targeting heuristics)
```
Instance | Structure | Registry
```

**Signal 2: Intent Polarity** (from utterance prefix / clue words)
```
Read (inspect/query/report/trace) | Write (create/modify/remove/transfer) | Ambiguous (assess)
```

**Signal 3: Domain Hint** (from ECIR noun extraction + entity_kind + stage_focus goals)
```
screening | cbu | deal | fund | document | ... (~25-30 domain concepts)
```

These compose into a **pre-classification cube** that the Sage computes in microseconds:

```
                    Read                    Write
              ┌──────────────────┬──────────────────┐
  Instance    │ Query data       │ Modify data      │
              │ deal.list        │ cbu.create       │
              │ document.for-*   │ screening.run    │
              │ ~200 verbs       │ ~500 verbs       │
              ├──────────────────┼──────────────────┤
  Structure   │ SAGE-ONLY ZONE   │ Registry authoring│
              │ schema.describe  │ changeset.create │
              │ registry.search  │ governance.submit│
              │ ~50 verbs        │ ~30 verbs        │
              │ No Coder needed  │ Coder + gates    │
              └──────────────────┴──────────────────┘
```

**The Read+Structure quadrant is pure Sage territory.** This is the highest-volume production scenario — users exploring data models, asking "what does this look like?", browsing schemas. The Sage answers from registry metadata and schema introspection without ever touching the Coder.

**The Read+Instance quadrant** still needs the Coder (to query live data), but the search space is halved — only read verbs in the detected domain. A read utterance against `trading-profile` (47 verbs) drops to 4 candidates.

**The Write+Instance quadrant** is where the full Sage confirmation → Coder execution flow applies. This is the minority of production utterances but the most important to get right.

**Clue word taxonomy (deterministic, utterance prefix):**

| Intent Polarity | Clue Words | Production Examples |
|----------------|------------|---------------------|
| **Inspect** | show, list, what, display, describe, view | "show me the CBUs", "what's the fund hierarchy?" |
| **Query** | who, which, where, how many, find, search, get | "who are the beneficial owners?", "find HSBC" |
| **Report** | report, summarize, status, count, compute | "what's the status of this case?" |
| **Trace** | trace, discover, who controls, who owns | "trace the ownership chain" |
| **Create** | create, add, new, set up, establish, onboard, open | "create a CBU", "onboard Allianz" |
| **Modify** | update, change, modify, assign, set, configure | "assign Goldman as custodian" |
| **Remove** | delete, remove, cancel, end, terminate, close, revoke | "cancel this deal" |
| **Transfer** | upload, import, export, transfer, move | "import from GLEIF", "upload passport" |
| **Ambiguous** | check, verify, run, process, handle | "check sanctions" (write) vs "check status" (read) |

The first four rows are **Read polarity** — no state mutation. The next four are **Write polarity**. Ambiguous needs a second signal (domain context) to classify: "run a check" in the screening domain = write; "run a report" = read.

**Pre-classification type:**
```rust
pub struct SagePreClassification {
    pub plane: ObservationPlane,     // Signal 1: from session context
    pub polarity: IntentPolarity,     // Signal 2: from utterance clue words
    pub domain_hints: Vec<String>,    // Signal 3: from ECIR + entity_kind + goals
    pub clue_word: Option<String>,    // Which word triggered polarity
    pub sage_only: bool,              // True if Read+Structure → skip Coder entirely
}

pub enum IntentPolarity {
    /// No state mutation. Show, list, describe, who, find, trace...
    Read,
    /// Mutates state. Create, update, delete, import, assign...
    Write,
    /// Could be either — needs domain context to classify.
    /// "Check sanctions" = write. "Check status" = read.
    Ambiguous,
}
```

This fires BEFORE the Sage's LLM call, BEFORE entity linking, BEFORE embedding search. It constrains the LLM prompt: instead of "classify this utterance against everything", the prompt becomes "given this is a READ operation in the SCREENING domain, what specific outcome?" — a much smaller classification problem.

**Why this matters for production volumes:** In a real custody banking operation, users spend most of their time *looking at things* before acting — checking status, browsing structures, querying ownership, reviewing documents. The read:write ratio in production will be 60:40 or 70:30, not the test suite's 28:72. The Sage handling Read+Structure utterances entirely — no Coder, no REPL, no verb selection — eliminates the most common source of user frustration (asking "what is X?" and getting routed to a write operation or asked for a UUID).

**The pre-classification also composes with the Outcome Model.** Read+Instance with a detected domain reduces the `OutcomeAction` to `Investigate` or `Report`, which the Coder maps to `*.list`, `*.get`, `*.for-entity`, `*.search` verbs only — typically 2-5 per domain. The 1,123-verb search problem disappears.

### Design Principle: Asymmetric Risk — Read Is Cheap, Write Is Expensive

Intent polarity creates an asymmetric risk profile that the Sage must exploit:

**Read operations are low-risk.** If the Sage gets a read wrong — shows the user the wrong list, describes the wrong schema, queries the wrong entity — the consequence is trivial. The user sees incorrect information, says "no, I meant X", and the Sage corrects. No state was modified. No data was corrupted. No audit trail was polluted. No governance boundary was crossed.

**Write operations are high-risk.** If the Sage gets a write wrong — creates the wrong CBU, runs sanctions screening against the wrong entity, assigns the wrong role — the consequence ranges from annoying (undo required) to serious (compliance violation, incorrect audit record, downstream workflow triggered on bad data).

**This means the Sage should operate with different confidence thresholds by polarity:**

```
READ + High confidence   → Execute immediately, no confirmation needed
READ + Medium confidence → Execute with light confirmation ("Showing deal schema — is that right?")
READ + Low confidence    → Ask which entity/domain, then execute
READ + Unclear           → Ask what they want to see

WRITE + High confidence  → Confirm before execution ("I'll create CBU for Allianz in LU. Proceed?")  
WRITE + Medium confidence→ Confirm with alternatives ("Create CBU or update existing? For Allianz?")
WRITE + Low confidence   → Full clarification before proceeding
WRITE + Unclear          → Full clarification, never guess
```

In practice this means: **the Sage should be aggressive on reads and cautious on writes.** A read outcome with 60% confidence? Execute it — worst case, the user sees the wrong list and says "no." A write outcome with 60% confidence? Always confirm. This asymmetry alone will dramatically reduce the perceived prompt count for the most common user flows (inspecting, browsing, exploring), because the Sage stops asking for confirmation on operations where being wrong doesn't matter.

The current pipeline treats all verb matches identically — the same 0.65 threshold and 0.05 margin whether the user is asking to view a list or delete a cascade. That's architecturally wrong. Read and write have fundamentally different risk profiles and should have fundamentally different confirmation behaviour.

### Design Principle: Polarity as Early Outcome Narrowing

Intent polarity is not just a filtering signal — it's an **early clue that narrows the field of possible outcomes** before domain classification or LLM reasoning even begins.

Consider: if the Sage detects Read polarity from "show me...", the space of possible OutcomeActions collapses from 10 to 4:

```
All actions:  Create | Investigate | Modify | Assess | Link | Verify | Remove | Transfer | Configure | Report
                                                                                                        
Read signal:          Investigate                                                                Report
              (browse, list, describe, query)                            (summarise, status, compute)
              
              + possibly: Assess (if read-only assessment like "check status")
              + possibly: Trace  (navigate relationships — always read)
```

That's a 60% reduction in the outcome classification space. The Sage's LLM call — if it's needed at all — is now choosing between "are they investigating, reporting, assessing, or tracing?" instead of the full 10-way action classification. This makes the LLM faster, cheaper, and more accurate.

Similarly, Write polarity collapses the action space:

```
Write signal: Create | Modify | Link | Remove | Transfer | Configure
              + possibly: Assess (if it triggers a mutation like screening.sanctions)
              + possibly: Verify (if it records a verification outcome)
```

The polarity signal acts as a **prior** for the Sage's reasoning — it doesn't determine the outcome, but it makes the correct outcome much more likely to be identified quickly. Combined with the domain hint (Signal 3), the Sage often has enough information to classify the outcome without an LLM call at all:

```
Read + "deal" domain   → Investigate deals (show, list, describe)
Read + "screening"     → Report screening results (show alerts, list hits)
Write + "cbu" domain   → Create or modify CBU
Write + "screening"    → Run screening check (assess)
```

For a human reading a user utterance, these clue words are the first thing you'd notice. "Show" — OK, they want to see something. "Create" — OK, they want to make something. The Sage should reason the same way: read the polarity signal first, then narrow the field, then classify within the reduced space.

---

## Part III: Implementation Plan

### Phase 0: Vocabulary Rationalization (Week 0-1)

**Target: Reduce 1,123 verbs / 134 domains to ~500-600 verbs / ~50 domains. YAML-only changes.**

**0A. Merge type-parameterized verb families** (~60-80 verbs eliminated):
- `entity.create-*` → `entity.create` + `entity-type` arg
- `entity.ensure-*` → `entity.ensure` + `entity-type` arg  
- `ubo.end-*` → `ubo.end-relationship` + `relationship-type` arg
- `ubo.delete-*` → `ubo.delete-relationship` + `relationship-type` arg
- `sla.bind-to-*` → `sla.bind` + `target-type` arg
- `trading-profile.add-*/remove-*` → `trading-profile.add-component`/`remove-component` + `component-type` arg

**0B. Merge overlapping domains** (~40-60 verbs, ~15-20 domains consolidated):
- Ownership tracing: 4 verbs across `control`/`gleif`/`bods`/`ownership` → single `ownership.trace`
- UBO identification: 4 verbs → single `ubo.identify`
- Product subscription: 3 verbs → single `cbu.subscribe-product`
- Lifecycle + service-resource overlap: consolidate into `service-resource`

**0C. De-template reference data phrases** — replace generic "get details", "fetch info" with domain-specific phrases, or consolidate all reference CRUD into `refdata.*` with domain parameter (~50-60 verbs eliminated).

**0D. Fix 84 exact phrase collisions** — deduplicate or differentiate each shared phrase.

**0E. Enrich 65 vague descriptions** — add WHEN/WHY context, not just WHAT.

### Phase 1: Sage Skeleton + Observation Plane (Week 1-2)

**Target: ~60% hit rate. Sage runs in parallel with existing pipeline for comparison.**

```
rust/src/sage/
  mod.rs              — SageEngine trait + public API
  outcome.rs          — OutcomeIntent, OutcomeStep, ObservationPlane types
  classifier.rs       — LLM-based outcome classification
  plane_classifier.rs — ObservationPlane detection from context
  decomposer.rs       — Complex outcome → step sequence
  context.rs          — SageContext (conversation history, entity state)
  research.rs         — Read-only research tool dispatch
```

Key implementation: the Sage's LLM prompt is NOT "pick a verb" — it's "identify the outcome", pre-constrained by the three deterministic signals:

```
You are a custody banking operations specialist. Given the user's 
request and context, identify the outcome.

PRE-CLASSIFICATION (already determined):
  Observation plane: {plane}
  Intent polarity: {polarity}
  Domain hints: {domain_hints}

Given these constraints, identify:

1. OUTCOME: What does the user want to achieve? (one sentence)
2. DOMAIN: Confirm or refine the domain hint. Which business area?
   (onboarding/kyc/fund_structure/screening/deal_management/
   data_management/corporate_research/regulatory/trading/other)
3. ACTION: What type? (create/investigate/modify/assess/link/verify/
   remove/transfer/configure/report)
4. PARAMETERS: What specific values did the user provide?
5. CONFIDENCE: How certain? (high/medium/low/unclear)
6. STEPS: If complex, what are the sub-steps?

Respond in JSON.
```

Note: for Read+Structure utterances, the LLM call may be skippable entirely — the pre-classification plus domain hint is often sufficient for the Sage to dispatch directly to `schema.entity.describe` or `registry.search` without LLM involvement. This is the fast path for production "show me X" queries.

### Phase 2: Coder Verb Resolution Rewrite (Week 2-3)

**Target: ~75% hit rate. Coder receives OutcomeStep, does structured lookup instead of embedding search.**

```rust
fn resolve_verb(step: &OutcomeStep, registry: &VerbRegistry) -> Result<ResolvedVerb> {
    // 1. Plane filter: Structure → schema.*/registry.* only,
    //                  Instance → exclude schema.*/registry.*
    let plane_verbs = registry.verbs_for_plane(step.plane);
    
    // 2. Polarity filter: Read → list/get/search/for-* verbs only,
    //                     Write → create/update/delete/add/remove verbs only
    let polarity_verbs = plane_verbs.filter_polarity(step.polarity);
    
    // 3. Domain filter: narrow to domain concept
    let domain_verbs = polarity_verbs.filter_domain(&step.domain_concept);
    
    // 4. Action + param overlap scoring
    let scored = domain_verbs
        .score_by_action_and_params(&step.action, &step.resolved_params);
    
    // 5. Deterministic selection (typically 1-3 candidates after filtering)
    select_best(scored)
}
```

### Phase 3: Data-Aware Scoring (Week 2-3, parallel with Phase 2)

Wire existing data model knowledge into the pipeline:

**3A. AffinityGraph boost** — verbs linked to the subject's entity type get scoring boost.

**3B. ECIR expansion** — generate noun_index entries for all rationalized domains from verb YAML metadata.

**3C. Domain gate** — two-stage funnel: classify domain first (ECIR + entity_kind + goals), then search within domain only.

### Phase 4: Wire Sage → Coder with Hard Boundary (Week 3-4)

**Target: ~85% hit rate.**

Replace orchestrator's `handle_utterance()` with Sage-first pipeline. Both pipelines run behind feature flag during transition. Sage classifies → confirms → hands to Coder. Hard boundary enforced: Sage has no write access, Coder has no conversation access.

### Phase 5: Entry Point Consolidation (Week 3-4, parallel)

**5A. Single pipeline instantiation** — create IntentPipeline ONCE in the orchestrator, not 3 times.

**5B. TraceId threading** — `trace_id: Uuid` on OrchestratorContext, propagated through IntentTrace, logged at every tier.

**5C. Shared OrchestratorContext factory** — both `agent_service.rs` and `mcp/handlers/core.rs` use the same builder. Same inputs, same outputs.

### Phase 6: Embedding Upgrade + Active Learning (Ongoing)

Replace BGE-small with domain-finetuned model. Active learning from corrections. Cluster-based disambiguation. This phase matters less once the Sage is handling outcome classification (the Coder's verb lookup may not need embeddings at all).

---

## Part IV: Expected Impact

| Phase | Accuracy | Key Change | Effort |
|-------|----------|------------|--------|
| Current | 38.8% | — | — |
| Phase 0 (Vocab) | ~50% | Eliminate collisions, merge duplicates, consolidate domains | 3-5 days |
| Phase 1 (Sage) | ~60% | Outcome classification replaces verb-first selection | 5-7 days |
| Phase 2 (Coder) | ~75% | Structured verb lookup replaces embedding search | 3-5 days |
| Phase 3 (Data-aware) | ~80% | AffinityGraph + ECIR + domain gate | 3-5 days |
| Phase 4 (Wire) | ~85% | End-to-end Sage→Coder with hard boundary | 3-5 days |
| Phase 5 (Entry) | — | Tracing, consolidation (quality-of-life, not accuracy) | 2-3 days |
| Phase 6 (Embeddings) | ~90%+ | Domain-finetuned embeddings, active learning | Ongoing |

---

## Part V: Key Architectural Decisions Pending

1. **Outcome taxonomy:** How many domain concepts? Derive from `phase_tags` and `metadata.tags`? Rough estimate: ~25-30.

2. **Observation plane defaults:** What's the default plane when no `stage_focus` is set? Instance (current behavior) is safest. Structure requires explicit opt-in via workflow selection.

3. **Sage LLM model:** Same model as arg extraction, or lighter? Outcome classification is structured output — may work with smaller/faster model.

4. **Conversation history window:** How much does the Sage need? Last 3 turns covers "run another one" scenarios. Full session for complex workflows.

5. **Research tool boundary:** Classify all ~102 MCP tools as Sage-accessible (read-only) vs Coder-only (write). The `db_introspect` tools are Sage-side. All `*.create`, `*.update`, `*.delete` are Coder-side.

6. **Backward compatibility:** Feature flag for Sage-first vs verb-first pipeline during migration. Both run, results compared, Sage replaces verb-first when consistently better.

7. **Vocabulary rationalization authority:** For the 84 exact collisions, which verb is canonical? E.g., "who owns this entity" → `ubo.list-by-subject` (UBO-focused) vs `graph.ancestors` (graph-focused)? This is a domain modelling decision, not engineering.

8. **Entity linker suppression in Structure mode:** In the current pipeline, entity linking runs unconditionally (orchestrator.rs line 339). In Structure mode, entity linking should be suppressed or its results ignored — "deal" is a type name, not an entity to resolve. The Sage's observation plane classification must happen BEFORE entity linking.

9. **Read+Structure fast path:** Can the Sage dispatch Read+Structure utterances entirely without an LLM call? If pre-classification gives plane=Structure + polarity=Read + domain_hint="deal", the Sage could deterministically dispatch to `schema.entity.describe :entity-type "deal"` with zero LLM cost. This would be the fastest path for the most common production queries. How much coverage does this fast path need before it's worth implementing as a hardcoded branch vs always going through LLM classification?

10. **Clue word ambiguity resolution:** "Check", "verify", "run" are ambiguous polarity — "check sanctions" is write, "check status" is read. The domain context resolves this (screening domain → write, session domain → read). Should the Sage resolve ambiguous polarity deterministically from domain, or always defer to LLM? Given the cost difference, deterministic resolution with a small exception list is probably correct.

---

*This document consolidates the root cause analysis, vocabulary audit, Sage/Coder architecture, and observation plane model into a single specification. It serves as the basis for generating per-phase Claude Code TODO.md files.*
