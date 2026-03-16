# ob-poc Architecture Review Plan

**Purpose:** Systematic "as-is" code and DB review of the ob-poc custody banking onboarding platform.  
**Model strategy:** Sonnet for all pass-by-pass review work; Opus for final cross-pillar synthesis only.  
**Key constraint:** Each Sonnet session is self-contained — scoped to fit comfortably in context without needing the full system loaded simultaneously.

---

## How To Use This Document

1. Work through phases sequentially (P1 → P2 → P3 → P4)
2. Each session has: **scope**, **input files**, **prompt**, **output deliverable**
3. Collect all session outputs into a single findings folder
4. Feed the consolidated findings to Opus in P4

**File preparation convention:**  
For each session, extract the relevant slice from the repo and paste/upload it. The prompts tell Sonnet exactly what to look at and what to produce.

---

## P1 — SemOS (Metadata Registry & Governance Layer)

> The "brain" — what the platform knows about entities, verbs, states, and how they relate.

### P1-A: Registry Structure & Coverage

**Input files:**
- DDL for: `sem_reg`, `domain_metadata`, verb registration tables, footprint tables
- Rust source: registry struct definitions (the modules that model registry entries)

**Prompt:**
```
You are reviewing the SemOS metadata registry of a custody banking
onboarding platform (ob-poc) written in Rust with PostgreSQL.

Your task: review the SemOS registry tables and Rust structs for
structural integrity, coverage, and internal consistency.

Focus areas:
1. sem_reg, domain_metadata, verb registrations, footprint tables —
   review schema for PK/FK integrity, naming consistency, orphan risks
2. Rust structs that model registry entries — do they faithfully
   mirror the DB schema? Any drift between DB and code representation?
3. Coverage: are there entity domains in the schema that have no
   corresponding sem_reg entries? Identify gaps.
4. Footprint model: is the entity→verb→footprint chain fully linked,
   or are there broken references?

Output format:
- Findings as a severity-tagged list (CLEAN / MINOR / FLAG / CRITICAL)
- Summary table: domain → verb count → footprint coverage %
- Any structural recommendations
```

**Output deliverable:** `P1-A_registry_structure.md`

---

### P1-B: Entity Lifecycle Profiles (FSM Layer)

**Input files:**
- Rust source: FSM/state machine definitions per entity
- DDL for: all status/state enum types in PostgreSQL

**Prompt:**
```
You are reviewing the state machine definitions within the SemOS
registry of ob-poc — a Rust/PostgreSQL custody banking platform.

Your task: audit every entity FSM for completeness and correctness.

Focus areas:
1. For each entity with a defined FSM: list all states, valid
   transitions, and terminal states
2. Cross-check FSM definitions against the DB enum types — any
   states defined in code but missing from schema, or vice versa?
3. Transition guards: are preconditions for state transitions
   explicitly defined, or implicit/assumed?
4. Impossible states: can you identify any state combinations that
   the FSM allows but the business logic should not?
5. Suspended/Terminated handling: is there a consistent pattern
   across entities, or ad-hoc per domain?

Output format:
- Per-entity FSM summary table (states × valid transitions)
- Gap list: missing transitions, unguarded edges, schema drift
- Pattern consistency assessment across entities
```

**Output deliverable:** `P1-B_entity_fsm_audit.md`

---

### P1-C: Verb Surface & SemOS Profiles

**Input files:**
- Verb registry data (YAML contracts or DB extract of all registered verbs)
- SemOS profile definitions (Entity, DSL, DAG, Document) for documented entities

**Prompt:**
```
You are reviewing the verb registry and SemOS profile system of
ob-poc — a Rust/PostgreSQL custody banking onboarding platform.

Your task: audit the verb surface for completeness, consistency,
and alignment with the four SemOS profile types (Entity, DSL, DAG,
Document).

Focus areas:
1. Verb inventory: total count per domain, naming convention
   adherence (domain.action pattern), any orphan verbs not bound
   to an entity
2. For each verb: does it have contracts defined (preconditions,
   postconditions, affected state transitions)?
3. Profile coverage: for the documented entities, which of the
   four SemOS profiles are populated vs missing?
4. Verb families: identify any verb families (e.g. screening.*,
   struct.*) that appear registered but lack implementation or
   have stale/placeholder definitions
5. Discrimination dimensions: do verbs carry harm_class,
   action_class, precondition_states metadata? What % coverage?

Output format:
- Verb surface summary: domain × verb count × contract coverage %
- Profile coverage matrix: entity × {Entity, DSL, DAG, Document}
- Blocked/stale verb families list with status
- Discrimination dimension coverage stats
```

**Output deliverable:** `P1-C_verb_surface_profiles.md`

---

### P1-D: StateGraph & Lane-Phase Gating

**Input files:**
- Rust source: StateGraph module (lane/phase definitions, gating logic)
- Relevant pipeline code showing how the resolver consumes StateGraph output

**Prompt:**
```
You are reviewing the StateGraph subsystem of SemOS in ob-poc —
the third leg alongside data and verbs that gates which verbs are
valid for a given entity state.

Your task: verify the StateGraph implementation reduces the verb
surface correctly per entity state.

Focus areas:
1. Graph structure: how are lanes and phases defined? Review the
   data model for completeness
2. Gating logic: trace the path from (entity, current_state) →
   valid_verb_set. Is the reduction deterministic? Any ambiguity?
3. Coverage: which entities have full StateGraph definitions vs
   partial vs none?
4. Edge cases: what happens when an entity is in a terminal state?
   Does the graph correctly return an empty or restricted verb set?
5. Integration point: how does the utterance-to-DSL pipeline
   consume the StateGraph output? Is the handoff clean?

Output format:
- StateGraph coverage table: entity → lanes defined → phases →
  estimated verb reduction ratio
- Logic trace for 2-3 representative entities through the gating
- Integration assessment with the resolver pipeline
- Gap list
```

**Output deliverable:** `P1-D_stategraph_gating.md`

---

## P2 — DSL (Custom S-Expression Language & Execution)

> The "language" — parsing, dispatch, compilation, and the NL-to-DSL bridge.

### P2-A: Parser (nom Layer)

**Input files:**
- Rust source: parser module (nom combinators)
- Existing parser tests (if any)

**Prompt:**
```
You are reviewing the DSL parser of ob-poc — a custom S-expression
language parsed with nom in Rust for a custody banking platform.

Your task: audit the parser for correctness, completeness, and
robustness.

Focus areas:
1. Grammar coverage: what constructs does the parser handle?
   Map the full grammar implicitly defined by the nom combinators
2. Error recovery: does the parser produce useful error messages
   on malformed input, or does it just fail opaquely?
3. Dead parse arms: any match branches or combinators that can
   never be reached given the grammar structure?
4. Ambiguity: are there any inputs that could parse two different
   ways? How is precedence/disambiguation handled?
5. Testing: what parser tests exist? Are edge cases covered
   (empty input, deeply nested, max-length, unicode)?

Output format:
- Informal grammar specification derived from the code
- Severity-tagged findings list
- Test coverage assessment with gap list
```

**Output deliverable:** `P2-A_parser_audit.md`

---

### P2-B: Verb Dispatch & Execution Engine

**Input files:**
- Rust source: execution/dispatch module
- Handler implementations (or at minimum the dispatch table/match arms)

**Prompt:**
```
You are reviewing the DSL verb execution engine of ob-poc — where
parsed DSL commands get dispatched to handlers in a Rust custody
banking platform.

Your task: audit the dispatch mechanism and handler implementations
for correctness and completeness.

Focus areas:
1. Dispatch table: how does a parsed verb resolve to a handler?
   Is it a static match, a registry lookup, or something else?
2. Coverage: compare the set of verbs the parser accepts vs the
   set the dispatcher can route. Any parse-but-no-handler gaps?
3. Error propagation: when a handler fails, does the error surface
   cleanly to the caller with enough context for diagnosis?
4. Transaction boundaries: which verb handlers run inside a DB
   transaction? Is the pattern consistent? Any handlers that
   should be transactional but aren't?
5. Side effects: identify handlers with external side effects
   (network calls, file I/O) — how are failures handled?

Output format:
- Dispatch coverage matrix: verb → handler → transactional? →
  side effects?
- Severity-tagged findings
- Consistency assessment
```

**Output deliverable:** `P2-B_dispatch_execution.md`

---

### P2-C: Macro Expansion & Runbook Compilation

**Input files:**
- Rust source: macro expansion module, runbook compilation pipeline
- Content-addressed ID generation code

**Prompt:**
```
You are reviewing the macro expansion and runbook compilation
pipeline of ob-poc's DSL — where raw DSL and macros get compiled
into executable runbooks.

Architectural invariant: ONLY compiled runbooks can execute.
Raw DSL and macro invocations never run directly.

Your task: verify this invariant holds and review the compilation
pipeline.

Focus areas:
1. Compilation gate: is there a single enforcement point that
   prevents raw DSL execution? Or is it convention-based?
2. Content-addressed IDs: verify the ID generation path
   (bincode + BTreeMap + SHA-256 truncated to 128 bits) — is it
   deterministic and collision-resistant for the expected scale?
3. Macro expansion: are macros expanded before or during
   compilation? Can expansion produce invalid DSL?
4. Locking: review the pessimistic entity locking with timeout —
   deadlock potential, timeout adequacy, lock release on panic
5. Replay/schema evolution: if the schema changes, can previously
   compiled runbooks still execute? What are the guardrails?

Output format:
- Invariant verification: HOLDS / BROKEN with evidence
- Pipeline stage diagram (text)
- Severity-tagged findings
- Replay risk assessment
```

**Output deliverable:** `P2-C_macro_runbook_compilation.md`

---

### P2-D: Utterance-to-DSL Pipeline

**Input files:**
- Rust source: resolver pipeline (entity scope, graph walk, LLM select stages)
- Sage/Coder boundary enforcement code

**Prompt:**
```
You are reviewing the utterance-to-DSL resolution pipeline of
ob-poc — the agentic NL-to-DSL pathway in a Rust custody banking
platform.

Architecture: three deterministic steps:
  1. Entity scope resolution
  2. StateGraph walk (reduce ~650 verbs to 5-15 candidates)
  3. LLM select from numbered menu

Your task: review each stage for correctness and failure modes.

Focus areas:
1. Entity scope: how is the target entity identified from the
   utterance? What happens on ambiguity or no-match?
2. Graph walk: verify the StateGraph integration correctly prunes
   the verb space. Is the reduction logged/auditable?
3. LLM select: what's the prompt structure? Is the numbered menu
   format robust? How are LLM refusals or off-menu selections
   handled?
4. Fallback chain: if any stage fails, what's the degradation
   path? Does the user get actionable feedback?
5. Sage/Coder boundary: is the asymmetric confirmation threshold
   enforced here? Can a Coder-mode action ever execute without
   explicit confirmation?

Output format:
- Stage-by-stage review with pass/fail per focus area
- Failure mode catalogue
- Severity-tagged findings
```

**Output deliverable:** `P2-D_utterance_pipeline.md`

---

## P3 — DB Schema (PostgreSQL Foundation)

> The "ground truth" — what actually gets persisted and enforced at the storage layer.

### P3-A: Core Entity Tables

**Input files:**
- DDL for: `cbus`, `entities`, `ubos`, `kyc_cases` + all extension/relationship tables (~30-40 tables)

**Prompt:**
```
You are reviewing the PostgreSQL schema of ob-poc — a custody
banking onboarding platform. This session covers core entity tables.

Your task: structural integrity audit.

Focus areas:
1. PK strategy: UUIDs, serials, composite? Consistent across tables?
2. FK integrity: all relationships declared? Cascade/restrict
   policies appropriate for the domain?
3. Index coverage: FKs indexed? Common query patterns supported?
   Any obvious missing indexes?
4. Constraint completeness: NOT NULLs where business logic demands,
   CHECK constraints on status enums, unique constraints on natural
   keys
5. Temporal patterns: created_at/updated_at consistent? Soft delete
   pattern if used — is it consistent?
6. Enum types: defined as PG enums or as text with CHECK? Do they
   match the FSM state definitions?
7. Naming: convention adherence (snake_case, singular/plural
   consistency, prefix patterns)

Output format:
- Per-table scorecard (PK/FK/Index/Constraints/Naming)
- Cross-table consistency findings
- Severity-tagged issue list
- Index recommendation list
```

**Output deliverable:** `P3-A_core_entity_schema.md`

---

### P3-B: Service, Trading & Instrument Tables

**Input files:**
- DDL for: `service_intents`, `trading_profiles`, `srdef`, instrument matrix, booking principals, deal/execution phase tables

**Prompt:**
```
[Same checklist as P3-A, with additional focus:]

Additional focus areas:
- Cross-domain FK coherence between service layer and core entities
- Trading instrument matrix cardinality and constraint model
- Booking principals / deal lifecycle table structure — do the
  Deal phase and Execution phase tables cleanly separate?

Output format: same as P3-A
```

**Output deliverable:** `P3-B_service_trading_schema.md`

---

### P3-C: Workflow, Governance & Audit Tables

**Input files:**
- DDL for: tollgates, DAGs, runbooks, changesets, policy tables, audit/history tables

**Prompt:**
```
[Same checklist as P3-A, with additional focus:]

Additional focus areas:
- Temporal integrity for audit trail tables — are they append-only
  at the schema level?
- Changeset immutability: is the compose→publish lifecycle enforced
  by constraints, or only by application code?
- Two-stage validation pipeline: is it reflected in the schema
  structure (separate tables/status columns for each stage)?
- Advisory locking: any schema support for the 25+ error code
  system?

Output format: same as P3-A
```

**Output deliverable:** `P3-C_workflow_governance_schema.md`

---

### P3-D: SemOS Metadata & Infrastructure Tables

**Input files:**
- DDL for: `sem_reg`, `domain_metadata`, verb registration tables, footprints, GLEIF, screening, SSI, document tables

**Prompt:**
```
[Same checklist as P3-A, with additional focus:]

Additional focus areas:
- Self-referential integrity of metadata tables (sem_reg pointing
  to itself for hierarchical domain relationships)
- Document polymorphism schema: do the three planes (artifact,
  proof assertions, context acceptance) have clean table
  boundaries, or are they conflated?
- GLEIF integration tables: external ID handling, staleness
  detection patterns

Output format: same as P3-A
```

**Output deliverable:** `P3-D_metadata_infrastructure_schema.md`

---

### P3-E: Schema Cross-Cut (FK Graph Only)

**Input files:**
- Output of `fk_graph_extract.sql` (see companion script)

**Prompt:**
```
You are reviewing the complete FK dependency graph of ob-poc's
PostgreSQL schema (~312 tables).

You are NOT reviewing individual table structure — that's done
in separate sessions. This session is purely topological.

Input: a list of (parent_table, child_table, fk_column,
on_delete, on_update) tuples.

Your task:
1. Orphan tables: any tables with zero FK references in or out?
2. Circular dependencies: any FK cycles? If so, are they
   intentional and documented?
3. Hub tables: which tables have the highest FK fan-in? Are these
   the expected domain anchors (cbus, entities, etc)?
4. Cascade risk: any DELETE CASCADE chains longer than 2 hops?
   Map the blast radius.
5. Schema partitioning: can you identify natural domain boundaries
   from the FK graph alone? Do they match the expected domains?

Output format:
- Orphan table list
- Cycle list (if any)
- Top-10 hub tables by FK fan-in
- Cascade chain risk map
- Suggested schema domain boundaries
```

**Output deliverable:** `P3-E_fk_graph_topology.md`

---

## P4 — Opus Synthesis (Single Session)

> Cross-pillar integration — the only session that needs Opus.

**Input files:**
- All deliverables from P1 (A-D), P2 (A-D), P3 (A-E)

**Prompt:**
```
You are performing an architectural synthesis of ob-poc — a Rust/
PostgreSQL custody banking onboarding platform with a custom DSL
and SemOS metadata registry.

Attached are the review findings from 13 Sonnet sessions across
three pillars: SemOS (P1), DSL (P2), and DB Schema (P3).

Your task:
1. Cross-pillar coherence: do the three pillars align? Where do
   schema definitions drift from SemOS metadata? Where does the
   DSL assume schema structures that don't exist?
2. Systemic patterns: identify recurring themes across findings
   (e.g. "inconsistent error handling" appearing in multiple
   pillars)
3. Architectural risks: what are the top 5 risks that only emerge
   from cross-cutting analysis?
4. Remediation backlog: prioritised list (P0/P1/P2) with estimated
   scope (small/medium/large)
5. SemOS completeness: given the findings, what % of the platform
   is genuinely governed by SemOS vs operating outside it?

Output: single consolidated architecture review document with
executive summary, detailed findings, and prioritised action plan.
```

**Output deliverable:** `P4_ARCHITECTURE_SYNTHESIS.md`

---

## Progress Tracker

| Session | Status | Findings Count | Critical | Date |
|---------|--------|---------------|----------|------|
| P1-A Registry Structure | ☐ TODO | | | |
| P1-B Entity FSM | ☐ TODO | | | |
| P1-C Verb Surface | ☐ TODO | | | |
| P1-D StateGraph | ☐ TODO | | | |
| P2-A Parser | ☐ TODO | | | |
| P2-B Dispatch/Execution | ☐ TODO | | | |
| P2-C Macro/Runbook | ☐ TODO | | | |
| P2-D Utterance Pipeline | ☐ TODO | | | |
| P3-A Core Entity Schema | ☐ TODO | | | |
| P3-B Service/Trading Schema | ☐ TODO | | | |
| P3-C Workflow/Governance Schema | ☐ TODO | | | |
| P3-D Metadata/Infra Schema | ☐ TODO | | | |
| P3-E FK Graph Topology | ☐ TODO | | | |
| P4 Opus Synthesis | ☐ TODO | | | |

---

## Companion Scripts

- `fk_graph_extract.sql` — extracts the FK topology for P3-E
- `enum_extract.sql` — extracts all PG enum types for P1-B cross-check
- `table_inventory.sql` — produces the full table list with row counts for scoping
