# REPL Loop Re-Engineering: Pack-Guided Runbook Architecture

**Document:** Unified Vision & Implementation Specification  
**Version:** 3.0 — February 2026  
**Status:** Proposed  
**Author:** Enterprise Onboarding Platform — ob-poc  
**Lineage:** v1 engineering blueprint → v2 merged with product vision → v3 review redlines applied

---

## 1. Prime Directive

> **If it isn't in the runbook, it didn't happen.**

No side effects, no "helpful hidden actions," no alternate execution paths. The runbook is the *only* durable output of the REPL loop.

---

## 2. Vision

The REPL is the primary interface between a business user and the onboarding platform. It should feel like talking to an expert colleague who already knows the canonical shape of your task — not like filling out forms or whispering verbs into a search engine.

**Four requirements drive the redesign:**

1. **Journey Packs, not free-form discovery.** The agent always operates inside a Journey Pack — a structured, versioned manifest that knows what questions to ask, what verbs to use, and what "done" looks like. The agent never "invents a journey." This single rule eliminates most trapdoors and makes the system explainable.

2. **Conversation, not forms.** Every clarification, disambiguation, and confirmation is a spoken sentence — never a modal picker, never a dropdown. If business users see forms, adoption fails. No modal pickers are required for correctness; UI suggestions (autocomplete, chips) are optional affordances that supplement the conversational flow.

3. **Sentence playback equals understanding.** After each intent is matched, the agent plays back a natural-language sentence derived from **dedicated sentence templates** (distinct from search phrases) and extracted arguments. "So you are adding IRS, CDS, and Equity products to the Allianz Lux CBU." The user confirms, corrects, or abandons.

4. **Runbooks are the deliverable.** The session is always building a runbook (even for a single command). Sentences accumulate into a reviewable sheet. Execution requires explicit user intent — `run` is never implicit.

---

## 3. Packs Are the Product API

Journey Packs are not an implementation detail — they are the **product-level interface** between the user and the platform. They sit above atomic DSL verbs and above single-entity operations:

> "I want to onboard Allianz Lux CBU book…"
> …is not 40 verbs. It's **one pack + one template**, with a small, guided Q/A to fill gaps, producing a runbook the user can read and approve.

**Packs define UX. Verbs define execution primitives.**

### 3.1 Why Packs Make or Break Adoption

Users will not "trust" a free-form agent if it feels like random questions, unpredictable next actions, verb soup, or unclear progress. Packs solve this:

- **Predictability**: "this is the onboarding journey; these are the steps"
- **Speed**: template-first planning (fast path)
- **Safety**: explicit scope limits + required fields
- **Trust**: coherent playback ("what we think you're doing") at pack and step level
- **Completeness**: definition-of-done and progress tracking

### 3.2 Adoption Sequencing (What to Build First)

Shortest path to user acceptance:
1. **Journey selection + templates** (packs make it feel like "shorthand for the whole job")
2. **Playback engine** (sentences) + runbook viewer/editor (trust and readability)
3. **Only then** expand broad "intent → verb discovery" beyond pack routing

Reason: users adopt the system when it feels **structured (packs)** and **self-explaining (playback)** — not when it feels "smart."

### 3.3 Operational Rule

> The agent never "invents a journey." It must always operate *inside a pack* — or ask the user to pick one.

Pack selection can always be forced by the user: "Use the onboarding journey."

### 3.4 De-Scoping Rule

> One runbook = one pack. No multi-pack runbooks.

If the user's work spans multiple packs, each pack produces its own runbook. Context from a completed runbook can be forwarded to the next via **pack handoff** (see §7.6). This keeps audit boundaries clean and makes each runbook self-explanatory.

---

## 4. Pack Manifest Contract

A pack is a versioned manifest (YAML) shipped as a new top-level config type alongside `verbs.yaml`. The orchestrator loads packs from the `RuntimeRegistry` the same way it loads verb definitions.

### 4.1 Full Schema

```yaml
# packs/onboarding-request.yaml
pack:
  id: "onboarding-request"
  name: "Onboarding Request"
  version: "1.0"
  description: "Onboard a CBU/book to products, trading matrix, and counterparties"

  # --- Routing ---
  invocation_phrases:
    - "onboard {cbu-name}"
    - "set up onboarding for {cbu-name}"
    - "onboarding request for {cbu-name}"
    - "prepare {cbu-name} for production"
  required_context:
    - client_group    # must be set before pack activates
  optional_context:
    - book_id         # if known, skip book selection
    - case_id

  # --- Scope & Safety ---
  allowed_verbs:
    - "cbu.assign-product"
    - "trading-profile.create-matrix"
    - "trading-profile.add-counterparty"
    - "trading-profile.add-instrument"
    - "entity.ensure-*"          # wildcard: all entity creation verbs
    - "kyc.open-case"
    - "onboarding.create-request"
  forbidden_verbs:
    - "admin.*"
    - "system.*"
  risk_policy:
    require_confirm_before_execute: true
    max_steps_without_confirm: 5

  # --- Question Policy (Conversational Semantics) ---
  #
  # These describe WHAT the agent needs to learn, not HOW to render it.
  # The primary interaction is natural language. UI may render optional
  # affordances (autocomplete, suggestion chips) but correctness never
  # depends on a picker or form widget.
  required_questions:
    - field: "products"
      prompt: "Which products do you want to add?"
      answer_kind: list              # list | boolean | string | entity_ref | enum
      options_source: "cbu.assign-product.product"  # verb.arg path for valid values
    - field: "trading_matrix"
      prompt: "Do you need a trading matrix?"
      answer_kind: boolean
      default: true
  optional_questions:
    - field: "counterparties"
      prompt: "Which counterparties for the trading matrix?"
      answer_kind: list
      ask_when: "trading_matrix == true"
  stop_rules:
    - "all required_questions answered"
    - "template can be filled with defaults"

  # --- Templates (Canonical Runbook Skeletons) ---
  templates:
    - template_id: "standard-onboarding"
      when_to_use: "Default onboarding with products + trading matrix"
      steps:
        - verb: "cbu.assign-product"
          args:
            cbu-name: "{context.cbu_name}"
            product: "{answers.products}"
          repeat_for: "answers.products"
        - verb: "trading-profile.create-matrix"
          args:
            cbu-name: "{context.cbu_name}"
            markets: "{answers.markets | default: ['EU', 'US']}"
          when: "answers.trading_matrix == true"
        - verb: "onboarding.create-request"
          args:
            cbu-name: "{context.cbu_name}"
          execution_mode: "human_gate"

    - template_id: "minimal-onboarding"
      when_to_use: "Products only, no trading matrix"
      steps:
        - verb: "cbu.assign-product"
          args:
            cbu-name: "{context.cbu_name}"
            product: "{answers.products}"
          repeat_for: "answers.products"

  # --- Playback ---
  pack_summary_template: >
    You're onboarding {context.cbu_name} with {answers.products | count} products
    {if answers.trading_matrix}and a trading matrix for {answers.markets | join}{/if},
    ready for a production onboarding request.
  section_layout:
    - chapter: "Products"
      verbs: ["cbu.assign-product"]
    - chapter: "Trading Setup"
      verbs: ["trading-profile.*"]
    - chapter: "Readiness"
      verbs: ["onboarding.create-request"]

  # --- Completion ---
  definition_of_done:
    - "at least 1 product assigned"
    - "onboarding.create-request step present"
  progress_signals:
    - label: "Products"
      condition: "count(steps where verb == 'cbu.assign-product' and status == 'Completed')"
    - label: "Trading Matrix"
      condition: "exists(steps where verb == 'trading-profile.create-matrix')"
    - label: "Request Filed"
      condition: "exists(steps where verb == 'onboarding.create-request' and status == 'Completed')"
```

### 4.2 Pack Selection (Deterministic)

The orchestrator chooses a pack via:

1. **Explicit user selection** (preferred) — user picks from a list or says "Use the onboarding journey"
2. **Pack routing** — user input matched against pack `invocation_phrases` using Candle BGE embeddings (same infrastructure as verb search, pointed at pack phrases instead)
3. **Fallback** — ask: "Are you doing Book Setup, Onboarding Request, or KYC Case?"

Pack routing reuses the existing `HybridVerbSearcher` infrastructure. Pack invocation phrases are indexed alongside verb invocation phrases, with a `pack:` prefix for disambiguation. The semantic search returns both verb and pack candidates; the orchestrator prefers pack matches when the user is in `JourneySelection` state.

### 4.3 Pack Interaction with Verb Registry

- `allowed_verbs[]` filters the `RuntimeRegistry` at pack activation. The orchestrator only proposes verbs within the allow-list.
- Pack `templates[].steps[].args` reference verb arg schemas. The template system validates that template arg names match verb `required` / `optional` arg definitions.
- Pack `required_questions[].options_source` pulls valid values from the referenced verb's arg schema (e.g., valid products from `cbu.assign-product`'s product enum).
- Pack `arg_inference_rules` (when present) provide defaults or derivations that supplement the verb's own `lookup` config.

### 4.4 Starter Packs (Phase 1 Deliverables)

Three starter packs ship with the initial implementation:

| Pack | Key Verbs | Templates |
|---|---|---|
| **Book Setup** (Lux/UK patterns) | `entity.ensure-*`, `cbu.create-*`, `cbu.assign-manco` | Lux SICAV canonical, UK OEIC canonical |
| **Onboarding Request** | `cbu.assign-product`, `trading-profile.*`, `onboarding.create-request` | Standard onboarding, minimal onboarding |
| **KYC Case** | `kyc.open-case`, `kyc.request-docs`, `kyc.review-gate` | New case, renewal case |

---

## 5. What Is Wrong Today (Code Forensics)

A forensic reading of the current source (~21K lines across 15 files) reveals a system that works but has accumulated structural debt from multiple iterations. The problems cluster into six categories.

### 5.1 Multiple Competing Pipelines

Input can flow through at least three distinct pipelines that partially overlap:

| Pipeline | Entry point | What it does |
|---|---|---|
| `IntentPipeline` (`mcp/intent_pipeline.rs`, 2011 lines) | `process()` / `process_with_scope()` | Scope resolution → Candle verb search → LLM arg extraction → DSL assembly → validation. The heavyweight path. |
| `HybridIntentMatcher` (`repl/intent_matcher.rs`, 518 lines) | `match_intent()` | Candle verb search → entity linking → LLM DSL generation. Designed as a "pure service" for the REPL orchestrator. |
| `LexiconPipeline` (`ob-agentic/lexicon/pipeline.rs`, 852 lines) | `process()` | Tokenizer → nom parser → IntentAst → DSL render. A formal grammar approach that predates the semantic search path. |

These three pipelines share concepts (verb search, entity resolution, DSL generation) but diverge in types, error handling, and state contracts. The orchestrator calls `IntentMatcher`, the MCP handlers call `IntentPipeline`, and the lexicon pipeline sits alongside both. A user message entering via the REPL API takes a different code path than the same message entering via MCP `tools/process-intent`.

### 5.2 Trapdoors and Side Exits

The orchestrator's `handle_message` method has multiple early-return branches that short-circuit the loop:

- **Direct DSL bypass** (`is_direct_dsl`): Input starting with `(` skips intent matching entirely and lands in `DslReady`. Useful, but it is a trapdoor out of the conversational flow — no sentence playback, no runbook staging.
- **Auto-execute** (`can_auto_execute`): Navigation verbs jump straight from match to execution, skipping confirmation. This creates an invisible category of commands that behave differently.
- **Macro expansion** (in `IntentPipeline`): Macro verbs trigger expansion inline and return a different `PipelineOutcome::MacroExpanded`. The orchestrator has no awareness of this.
- **Scope resolution consuming input** (Stage 0): When scope resolution matches, it returns early from the pipeline. Correct behaviour, but it is a hard gate implemented inside `IntentPipeline` via `#[cfg(feature = "database")]` conditionals rather than as an explicit state in the loop.

### 5.3 No Sentence Playback

Every verb already carries `invocation_phrases` in its YAML config, and `phrase_gen.rs` generates additional phrases from synonym dictionaries. But these phrases are used exclusively for semantic search indexing — they are never played back to the user as confirmation sentences. They are also not suitable for playback: search phrases like "add counterparty" and "create counterparty" are keyword fragments, not sentence templates with arg slots.

The current flow is: match verb → generate DSL → show DSL to user → ask "run?". The user sees `(entity.ensure-limited-company :name "Allianz" :jurisdiction "LU")` instead of "Create Allianz as a limited company in Luxembourg." This is a fundamental UX failure for non-technical users.

### 5.4 Runbook Model Fragmentation

Four separate runbook/sheet models exist:

| Model | Location | Status |
|---|---|---|
| `DslSheet` | `session/dsl_sheet.rs` | **Deprecated** (header says so). Batch execution with DAG phasing. |
| `StagedRunbook` / `StagedCommand` | `repl/staged_runbook.rs` | Active. Database-backed, entity footprinting, resolution tracking. Operated by `RunbookService`. |
| `RunSheet` / `RunSheetEntry` | `session/unified.rs` | Newer replacement for `DslSheet`, embedded in `UnifiedSession`. |
| `LedgerEntry` | `repl/types.rs` | Orchestrator's model. Tracks inputs and outcomes but doesn't connect to any runbook. |

Each has its own status enum, its own resolution tracking, and its own execution path.

### 5.5 State Machine Gaps

The `ReplState` enum has five states: `Idle`, `IntentMatching`, `Clarifying(...)`, `DslReady`, `Executing`. Missing are:

- **SentenceConfirmation** — no state for "agent played back a sentence, waiting for user to confirm"
- **RunbookBuilding** — no state for "we have N sentences accumulated, what's next?"
- **JourneySelection** — no state for "user has context but hasn't chosen a journey pack"
- **ScopeSelection** — scope selection is modelled as a sub-variant of `ClarifyingState::ClientGroupSelection`, but it is conceptually Stage 0, before clarification

The `ClarifyingState` enum has six variants (VerbSelection, ScopeSelection, EntityResolution, Confirmation, IntentTier, ClientGroupSelection). Several have TODO comments in their handlers, indicating incomplete wiring.

### 5.6 No Journey Structure

There is no concept of a "Journey Pack" in the current codebase. Every interaction is free-form verb discovery. The user must know what verbs exist and ask for them one at a time. There is no template system, no definition-of-done, no question policy, and no pack-level playback. This is the root cause of the "40 rounds of verb-by-verb Q&A" problem.

---

## 6. The New System Boundary (Three Concerns)

### 6.1 Hard Split

1. **REPL Orchestrator (new)**
   Owns: state machine, journey pack routing, Q/A policy, proposal generation, sentence playback, runbook editing.

2. **Runbook Store (new)**
   Owns: persistence, versioning, diffing, audit trail, event stream.

3. **Compiler/Executor (existing, treated as black box)**
   Owns: compile, validate, execute, produce outcomes/events.

**Key discipline:** the orchestrator never "does work directly." It only **writes runbook steps** and requests compile/execute.

### 6.2 vNext Architecture (Greenfield Discipline)

The target architecture is designed as if the old code does not exist. This means:
- new types do not import or extend legacy types
- new state machine does not accommodate legacy state variants
- new runbook model does not wrap `DslSheet`, `StagedRunbook`, or `RunSheet`
- new pipeline does not share internal types with `IntentPipeline` or `HybridIntentMatcher`

The clean separation means the vNext code is testable in isolation and can serve as the reference implementation. Engineering does not compromise the clean model to preserve legacy types.

### 6.3 Integration & Rollout (Feature Flags)

This is a working system in active development with 90+ database tables, a running MCP server, and an API surface the UI consumes. The migration must be incremental.

**Approach:** New code is built alongside old code. Feature flags control which path is active. Each phase adds capability without removing existing capability. Deprecation happens only after the new path is proven.

```
Phase 0–2:  New types and skeleton coexist with old types (separate modules)
Phase 3–4:  New orchestrator behind feature flag (both paths active)
Phase 5:    New orchestrator becomes default, old is fallback
Phase 6:    Old code deprecated/removed
```

The vNext architecture (§6.2) is the design target. The rollout plan (above) is the delivery strategy. They are separate concerns.

---

## 7. Runbook Contract (The Only Durable Truth)

### 7.1 Runbook Model

One model. One status enum. One execution path. Supersedes `DslSheet`, `StagedRunbook`, `RunSheet`, and `LedgerEntry`.

```rust
pub struct Runbook {
    pub id: Uuid,
    pub session_id: Uuid,
    pub client_group_id: Option<Uuid>,

    // --- Pack provenance ---
    pub pack_id: Option<String>,
    pub pack_version: Option<String>,
    pub pack_manifest_hash: Option<String>,   // SHA-256 of manifest at creation time

    // --- Template provenance ---
    pub template_id: Option<String>,
    pub template_hash: Option<String>,        // hash of template skeleton used

    pub status: RunbookStatus,
    pub entries: Vec<RunbookEntry>,
    pub audit: Vec<RunbookEvent>,             // event-sourced history
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct RunbookEntry {
    pub id: Uuid,
    pub sequence: i32,

    // --- What the user sees ---
    pub sentence: String,                     // "add IRS product to Allianz Lux"
    pub labels: HashMap<String, String>,      // UI display labels

    // --- What the engine executes ---
    pub dsl: String,                          // "(cbu.assign-product :cbu-name ...)"
    pub verb: String,                         // "cbu.assign-product"
    pub args: HashMap<String, String>,

    // --- Provenance ---
    pub slot_provenance: SlotProvenance,       // which args were user-provided vs defaulted
    pub arg_extraction_audit: Option<ArgExtractionAudit>,  // LLM audit trail

    // --- Execution ---
    pub status: EntryStatus,
    pub execution_mode: ExecutionMode,
    pub confirm_policy: ConfirmPolicy,         // how this step requires confirmation
    pub unresolved_refs: Vec<UnresolvedRef>,
    pub depends_on: Vec<Uuid>,                // DAG edges
    pub result: Option<ExecutionResult>,
}

pub enum RunbookStatus { Draft, Building, Ready, Executing, Completed, Parked, Aborted }
pub enum EntryStatus { Proposed, Confirmed, Resolved, Executing, Completed, Failed, Parked }
pub enum ExecutionMode { Sync, Durable, HumanGate }
pub enum ConfirmPolicy { Always, QuickConfirm, PackConfigured }
```

### 7.2 Slot Provenance

When a runbook is created from a template, every arg slot records its source:

```rust
pub struct SlotProvenance {
    pub slots: HashMap<String, SlotSource>,
}

pub enum SlotSource {
    UserProvided,           // user explicitly said "IRS"
    TemplateDefault,        // template defined a default value
    InferredFromContext,    // derived from client context, pack rules, etc.
    CopiedFromPrevious,    // pack handoff from prior runbook
}
```

This is essential for explaining "why this plan looks like this."

### 7.3 Arg Extraction Audit

Every runbook entry that involved LLM arg extraction stores the evidence:

```rust
pub struct ArgExtractionAudit {
    pub model_id: String,          // e.g. "claude-sonnet-4-5-20250929"
    pub prompt_hash: String,       // SHA-256 of the prompt sent
    pub user_input: String,        // raw user message
    pub extracted_args: HashMap<String, String>,
    pub confidence: Option<f32>,   // model's self-reported confidence if available
    pub timestamp: DateTime<Utc>,
}
```

This is a compliance requirement. Determinism claims require that LLM calls are auditable and replayable for debugging.

### 7.4 Visibility Policy (DSL vs Sentences)

The runbook stores both sentences and DSL. The **visibility policy** governs what the user sees:

| Context | Shown | Hidden |
|---|---|---|
| Default conversational UX | Sentences, labels, pack summary | DSL, arg maps, provenance |
| Runbook review/edit | Sentences, step status, progress | DSL (available on expand) |
| Power-user mode (opt-in) | Sentences + DSL side-by-side | Nothing hidden |
| Audit/compliance export | Everything | Nothing |
| Error diagnostics | Sentence + DSL + arg extraction audit | Nothing |

**Rule:** DSL is never shown unless the user explicitly opts in (power-user toggle) or an error makes it necessary for diagnosis. The default experience is sentences-only.

### 7.5 What Must Be Persisted (and Why)

| Field | Why |
|---|---|
| `pack_id` + `pack_version` + `pack_manifest_hash` | Reproducibility: re-run months later with same pack |
| `template_id` + `template_hash` | Explain "why this plan shape" |
| `sentence` per entry | Human-readable audit trail |
| `dsl` per entry | Machine-executable record |
| `slot_provenance` per entry | Explain "user said this, system defaulted that" |
| `arg_extraction_audit` per entry | LLM compliance trail |
| `audit[]` events on runbook | Event-sourced history of all changes |

### 7.6 Pack Handoff

Users will start Book Setup then immediately want Onboarding Request. Since one runbook = one pack, we need a first-class **handoff**:

```rust
pub struct PackHandoff {
    pub source_runbook_id: Uuid,
    pub target_pack_id: String,
    pub forwarded_context: HashMap<String, String>,  // e.g. client_group, cbu_name
    pub forwarded_outcomes: Vec<Uuid>,                // entry IDs whose results carry forward
}
```

**Handoff is a standard system action**, not a special case. The orchestrator creates a new runbook for the target pack, populates its context from the source runbook's completed outcomes, and marks the forwarded slots with `SlotSource::CopiedFromPrevious`.

### 7.7 Outcome Events

Compiler/executor emits events the orchestrator consumes:

```rust
pub enum OutcomeEvent {
    ValidationError { step_id: Uuid, message: String },
    ExecutionResult { step_id: Uuid, result: Value },
    DurableTaskStarted { step_id: Uuid, task_id: String },
    ExternalSignalReceived { step_id: Uuid, signal: Value },
    HumanReviewRequired { step_id: Uuid, reason: String },
    StepCompleted { step_id: Uuid },
}
```

---

## 8. The Golden Loop (Non-Negotiable)

### 8.1 The Loop (Single Mental Model)

```
1. SELECT CLIENT GROUP CONTEXT
2. SELECT JOURNEY PACK (book setup / onboarding / KYC case / etc.)
3. Q/A TO RESOLVE INTENT (minimal questions; stop as soon as we can act safely)
4. PROPOSE RUNBOOK STEP(S) (candidate steps with evidence + "why")
5. SENTENCE PLAYBACK (verb sentence templates assembled into plain English)
6. USER EDITS / CONFIRMS
7. COMPILE → EXECUTE (sync or durable)
8. OBSERVE OUTCOMES (results + next questions)
9. APPEND / REFINE RUNBOOK
10. REPEAT UNTIL DONE / HAND-OFF
```

### 8.2 The Loop as a Stage Diagram

```
┌──────────────────────────────────────────────────────────────┐
│                       THE LOOP                                │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ STAGE 0 — SCOPE GATE                                   │  │
│  │                                                        │  │
│  │ Q: "What client group are you working on?"             │  │
│  │                                                        │  │
│  │ Resolved      → set context, proceed to Stage 1        │  │
│  │ Candidates    → conversational picker, stay Stage 0    │  │
│  │ Not scope     → proceed to Stage 1 (scope may be set)  │  │
│  │ No scope set  → ask, stay Stage 0                      │  │
│  └────────────────────────────────────────────────────────┘  │
│                          │                                    │
│                          ▼                                    │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ STAGE 1 — JOURNEY SELECTION                            │  │
│  │                                                        │  │
│  │ Pack routing (invocation phrases + context)            │  │
│  │                                                        │  │
│  │ Pack matched   → load template, proceed Stage 2        │  │
│  │ Ambiguous      → ask: "Book setup, Onboarding, KYC?"   │  │
│  │ No match       → ask user, stay Stage 1                │  │
│  │ Pack selected  → already in context, skip              │  │
│  └────────────────────────────────────────────────────────┘  │
│                          │                                    │
│                          ▼                                    │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ STAGE 2 — PACK-GUIDED Q/A                              │  │
│  │                                                        │  │
│  │ Within the selected pack:                              │  │
│  │  - Ask required questions (slot-filling, not open)     │  │
│  │  - Semantic verb search for ad-hoc intent              │  │
│  │  - Template fast-path: fill skeleton from user input   │  │
│  │  - Fallback: verb search + LLM arg extraction          │  │
│  │                                                        │  │
│  │ High confidence  → extract args, propose step          │  │
│  │ Ambiguous        → clarifying Q, stay Stage 2          │  │
│  │ Direct DSL       → wrap in sentence, propose step      │  │
│  └────────────────────────────────────────────────────────┘  │
│                          │                                    │
│                          ▼                                    │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ STAGE 3 — SENTENCE PLAYBACK                            │  │
│  │                                                        │  │
│  │ Generate sentence from verb sentence templates + args  │  │
│  │ Play back: "So you are adding Allianz as counterparty  │  │
│  │   for IRS and CDS products..."                         │  │
│  │                                                        │  │
│  │ Pack playback: one-paragraph summary of the whole plan │  │
│  │                                                        │  │
│  │ User confirms   → proceed Stage 4                      │  │
│  │ User corrects   → re-extract args, stay Stage 3        │  │
│  │ User abandons   → return to Stage 2                    │  │
│  └────────────────────────────────────────────────────────┘  │
│                          │                                    │
│                          ▼                                    │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ STAGE 4 — RUNBOOK EDITING                              │  │
│  │                                                        │  │
│  │ Step added to runbook. Show accumulated sentences.     │  │
│  │                                                        │  │
│  │ "What else?"    → return to Stage 2                    │  │
│  │ "Ready"/"Run"   → proceed Stage 5                      │  │
│  │ "Remove #3"     → edit runbook, stay Stage 4           │  │
│  │ "Reorder"       → reorder, stay Stage 4                │  │
│  │ Edit step args  → modify + re-playback, stay Stage 4   │  │
│  └────────────────────────────────────────────────────────┘  │
│                          │                                    │
│                          ▼                                    │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ STAGE 5 — EXECUTION                                    │  │
│  │                                                        │  │
│  │ Compile: validate all steps, DAG dependencies          │  │
│  │ Execute: phased, with per-step status reporting        │  │
│  │                                                        │  │
│  │ Complete → show results + outcomes, return Stage 2/4   │  │
│  │ Failure  → show error, allow retry/edit                │  │
│  │ Durable  → park runbook, resume on signal              │  │
│  │ Human gate → park, notify, await approval              │  │
│  └────────────────────────────────────────────────────────┘  │
│                          │                                    │
│                          ▼                                    │
│              Return to Stage 2 (within pack)                  │
│              or Stage 1 (new journey / pack handoff)          │
└──────────────────────────────────────────────────────────────┘
```

### 8.3 The State Machine (Rust)

```rust
pub enum ReplState {
    /// Stage 0: No scope set, or switching scope
    ScopeGate {
        pending_input: Option<String>,
    },

    /// Stage 1: Selecting or routing to a journey pack
    JourneySelection {
        candidates: Option<Vec<PackCandidate>>,
    },

    /// Stage 2: Inside a pack, accepting input, matching intent
    InPack {
        pack_id: String,
        required_slots_remaining: Vec<String>,   // unanswered required questions
        last_proposal_id: Option<Uuid>,          // most recent step proposal
    },

    /// Stage 2→3 transition: agent asking clarifying Q
    Clarifying {
        question: String,
        candidates: Vec<VerbCandidate>,
        original_input: String,
    },

    /// Stage 3: Sentence played back, awaiting confirmation
    SentencePlayback {
        sentence: String,
        verb: String,
        dsl: String,
        args: HashMap<String, String>,
    },

    /// Stage 4: Building runbook, waiting for next step or execute
    RunbookEditing,

    /// Stage 5: Executing runbook
    Executing {
        runbook_id: Uuid,
        progress: ExecutionProgress,
    },
}
```

Key changes from current codebase:
- `Idle` and `IntentMatching` collapse into `InPack` — the orchestrator always knows which pack it's in, what questions remain, and what was last proposed
- `ClarifyingState` with 6 variants collapses into a single `Clarifying` with a question string (the agent asks questions in natural language, not via typed modals)
- `DslReady` is replaced by `SentencePlayback` (user never sees DSL by default)
- `JourneySelection` is new — the pack routing state
- `RunbookEditing` is new — the accumulation and editing state
- `ScopeGate` is promoted to a first-class state (not a sub-variant of clarifying)

### 8.4 Confirmation Policy

Confirmation is policy-driven, not a code bypass. Every step still creates a runbook entry regardless of confirmation mode.

```rust
pub enum ConfirmPolicy {
    /// Always require explicit "yes" before adding to runbook
    Always,
    /// Play back sentence, auto-confirm after brief pause unless user interrupts.
    /// Used for high-confidence navigation: "Loading the Allianz book."
    QuickConfirm,
    /// Defer to the pack's risk_policy configuration
    PackConfigured,
}
```

**All flows create a runbook entry.** `QuickConfirm` is a UX affordance (shorter pause, less prominent confirmation prompt), not a path that skips the runbook. The `ConfirmPolicy` is set per-verb in the verb registry, overridable per-step in the pack template.

### 8.5 Clarification Policy (Minimal Questions)

The orchestrator asks questions only when:
- the next step would be **unsafe** or **meaningfully ambiguous**
- the journey pack declares a field as **required** (and it's not yet answered)
- the executor returns a validation error that can be fixed by user input

Stop criteria:
- we can propose a safe step with defaults, OR
- we can propose multiple options with clear tradeoffs

---

## 9. Verb Sentence Templates (Separate from Search Phrases)

### 9.1 The Problem with Reusing Search Phrases

The existing `invocation_phrases` on each verb are optimised for embedding similarity search. They are keyword fragments like "add counterparty", "create counterparty", "assign product to cbu". Using these for sentence playback produces awkward English — "add IRS product to Allianz Lux" is tolerable but "assign product to cbu Allianz Lux" is not. This is a long-term UX tax.

### 9.2 Separated Field Structure

Every verb carries **two distinct sets of phrases**, each serving a different purpose:

```yaml
# In verb YAML config
verb:
  id: "cbu.assign-product"
  # ...existing fields...

  # --- Search (embeddings index) ---
  search_phrases:
    - "add product"
    - "assign product to cbu"
    - "add {product} to {cbu-name}"
    - "register product for book"

  # --- Sentence Templates (playback) ---
  sentences:
    step:
      - "Add {product} product to {cbu-name}"
      - "Register {product} for the {cbu-name} book"
    summary:
      - "adding {product} to {cbu-name}"
    clarify:
      product: "Which product — IRS, CDS, Equity, Fixed Income, or something else?"
      cbu-name: "Which CBU should this product be added to?"
```

**Search phrases** drive semantic search (Candle BGE embeddings). They may be fragments, duplicated, keyword-heavy.

**Sentence templates** drive playback and clarification. They must produce grammatically correct English when args are substituted. `step` templates are used for per-entry playback. `summary` templates are used for pack-level plan summaries. `clarify` templates are used for conversational clarification when an arg is missing.

### 9.3 Migration Path

Phase 0–1: The `SentenceGenerator` first checks for `sentences.step[]` templates. If none exist, it falls back to `search_phrases` (existing `invocation_phrases`) with best-effort template selection. This means playback works immediately for all verbs, with quality improving as `sentences` fields are populated.

Phase 2: All verbs used by starter packs get proper `sentences` fields. The verb registry schema is extended accordingly.

---

## 10. Single Unified Pipeline

The three existing pipelines merge into one service with clean internal phases.

### 10.1 IntentService

```rust
pub struct IntentService {
    scope_resolver: ScopeResolver,
    pack_router: PackRouter,                   // NEW: pack selection
    verb_searcher: HybridVerbSearcher,
    arg_extractor: ArgExtractor,               // LLM-based
    sentence_generator: SentenceGenerator,     // template-based, no LLM
}

impl IntentService {
    /// Phase 0: Is this a scope phrase?
    pub async fn try_resolve_scope(&self, input: &str, pool: &PgPool)
        -> ScopeOutcome;

    /// Phase 1: Route to a journey pack
    pub async fn route_pack(&self, input: &str, context: &SessionContext)
        -> PackRouteOutcome;

    /// Phase 2: Find the verb (within pack's allowed_verbs)
    pub async fn match_verb(&self, input: &str, pack: &PackManifest, context: &SessionContext)
        -> VerbMatchOutcome;

    /// Phase 3: Extract arguments (LLM call)
    pub async fn extract_args(&self, verb: &str, input: &str, context: &SessionContext)
        -> ArgExtractionOutcome;

    /// Phase 4: Generate confirmation sentence (NO LLM — template only)
    pub fn generate_sentence(&self, verb: &str, args: &HashMap<String, String>)
        -> String;

    /// Phase 5: Assemble DSL from verb + resolved args
    pub fn assemble_dsl(&self, verb: &str, args: &HashMap<String, ArgValue>)
        -> Result<String>;
}
```

The orchestrator calls each phase explicitly, inspects the outcome, and decides the state transition. No phase has side effects on session state. The orchestrator owns all mutations.

### 10.2 Sentence Generator

The sentence generator is **deterministic and LLM-free**. It uses verb `sentences.step[]` templates (falling back to `search_phrases` during migration).

```rust
pub struct SentenceGenerator {
    registry: Arc<RuntimeRegistry>,
}

impl SentenceGenerator {
    /// Generate a natural sentence from verb + args.
    ///
    /// Example:
    ///   verb = "cbu.assign-product"
    ///   args = { "cbu-name": "Allianz Lux", "product": "IRS" }
    ///   template = "Add {product} product to {cbu-name}"
    ///   output = "Add IRS product to Allianz Lux"
    pub fn generate(&self, verb: &str, args: &HashMap<String, String>) -> String {
        let verb_def = self.registry.get_verb(verb);
        // 1. Try sentences.step[] templates first
        // 2. Fall back to search_phrases (invocation_phrases) if no sentence templates
        // 3. Fall back to phrase_gen::generate_phrases() as last resort
        let template = self.best_template(verb_def, args);
        self.substitute(template, args)
    }

    /// Select the template with the most arg placeholders that can be filled.
    fn best_template(&self, verb_def: &RuntimeVerb, args: &HashMap<String, String>) -> &str;

    /// Substitute {arg-name} placeholders. Unfilled placeholders render as "...".
    fn substitute(&self, template: &str, args: &HashMap<String, String>) -> String;

    /// Handle list args: join with Oxford comma ("IRS, CDS, and Equity")
    fn format_list(&self, values: &[String]) -> String;
}
```

### 10.3 Pack-Level Playback

In addition to per-step sentences, the pack provides summary playback:

```rust
impl PackPlayback {
    /// Generate a one-paragraph summary of the entire runbook within this pack.
    ///
    /// Uses pack_summary_template + section_layout to produce:
    /// "You're onboarding Allianz Lux with 3 products (IRS, CDS, Equity)
    ///  and a trading matrix for EU and US markets, ready for a
    ///  production onboarding request."
    pub fn summarize(&self, pack: &PackManifest, runbook: &Runbook) -> String;

    /// Generate chapter headings grouped by section_layout.
    ///
    /// "Products:
    ///   1. Add IRS to Allianz Lux
    ///   2. Add CDS to Allianz Lux
    ///   3. Add Equity to Allianz Lux
    ///  Trading Setup:
    ///   4. Create trading matrix for EU, US markets
    ///  Readiness:
    ///   5. File onboarding request"
    pub fn chapter_view(&self, pack: &PackManifest, runbook: &Runbook) -> String;
}
```

### 10.4 Conversational Clarification (Not Forms)

Today, disambiguation produces a picker modal. Tomorrow it produces a question using the verb's `sentences.clarify` templates.

**Current (form-like):**
```
Agent: Multiple actions match. Did you mean:
  [1] cbu.assign-product
  [2] trading-profile.add-instrument
  [3] custody.add-product
```

**Target (conversational):**
```
Agent: I can see a few options. Are you adding a product to a CBU,
       setting up a trading instrument, or adding a custody product?
User:  Adding a product to the CBU
Agent: Got it. Which product — IRS, CDS, Equity, or something else?
User:  IRS and CDS
Agent: So you are adding IRS and CDS to the Allianz Lux CBU. Correct?
User:  Yes
```

---

## 11. Client & Journey Context

### 11.1 Client Context

A stable object that answers: "what client group am I working on?"

```rust
pub struct ClientContext {
    pub client_group_id: Uuid,
    pub client_group_name: String,
    pub default_cbu: Option<Uuid>,
    pub default_book: Option<Uuid>,
    pub permissions: Vec<Permission>,
}
```

### 11.2 Journey Context

```rust
pub struct JourneyContext {
    pub pack: Arc<PackManifest>,
    pub pack_manifest_hash: String,              // pinned at activation
    pub answers: HashMap<String, AnswerValue>,   // accumulated Q/A answers
    pub template: Option<String>,                // selected template_id
    pub progress: PackProgress,                  // definition-of-done tracking
    pub handoff_source: Option<PackHandoff>,      // if this journey started from a handoff
}
```

---

## 12. Agent Q&A Journeys: Concrete Flows

### 12.1 New Session — Full Journey (Onboarding Pack)

```
Agent:  Welcome. What client group are you working on?           [Stage 0: ScopeGate]
User:   Allianz
Agent:  Working with Allianz (12 entities, 45 CBUs).             [Scope set]
        What would you like to do?                               [Stage 1: JourneySelection]
User:   I need to onboard the Lux CBU with some products
Agent:  Sounds like an onboarding request. I'll use the          [Pack matched]
        Onboarding Request journey.
        Which CBU — the Allianz Lux Umbrella or the Allianz      [Stage 2: InPack Q/A]
        Lux Sub-Fund?
User:   The umbrella
Agent:  Which products? IRS, CDS, Equity, Fixed Income,          [Pack required question]
        Fund Accounting, or others?
User:   IRS, CDS, and Equity
Agent:  Do you need a trading matrix?                            [Pack required question]
User:   Yes, EU and US markets
Agent:  Got it. Here's the plan:                                 [Stage 3: Playback]

        You're onboarding Allianz Lux Umbrella with 3 products
        (IRS, CDS, Equity) and a trading matrix for EU and US
        markets, ready for a production onboarding request.

        Steps:
          1. Add IRS to Allianz Lux Umbrella
          2. Add CDS to Allianz Lux Umbrella
          3. Add Equity to Allianz Lux Umbrella
          4. Create trading matrix for EU, US markets
          5. File onboarding request [requires approval]

        Does this look right?
User:   Yes                                                       [Stage 4: RunbookEditing]
Agent:  Runbook confirmed. Ready to execute, or any changes?
User:   Run it                                                    [Stage 5: Execution]
Agent:  Executing 5 steps...
          ✓ 1. IRS product added
          ✓ 2. CDS product added
          ✓ 3. Equity product added
          ✓ 4. Trading matrix created
          ⏸ 5. Onboarding request filed — awaiting approval
        4 of 5 complete. Step 5 is parked for human approval.
        Anything else?                                            [→ Stage 2]
```

### 12.2 Returning User — Scope Already Set

```
Agent:  [Scope: Allianz] What would you like to do?
User:   Open a KYC case for the main manco
Agent:  I'll use the KYC Case journey.                           [Pack matched]
        So you are opening a KYC case for Allianz Global         [Playback]
        Investors GmbH. Is that right?
User:   Yes
Agent:  Added to runbook. Ready to execute, or anything else?
User:   Run it
Agent:  ✓ KYC case KYC-2026-0042 created for Allianz Global
        Investors GmbH.
```

### 12.3 Book Setup — Template Fast Path

```
User:   Set up a Lux SICAV with a manco, an SPV, and
        an investment manager
Agent:  I'll use the Book Setup journey, Lux SICAV template.     [Pack + template matched]
        Here's what I'll set up:

          1. Create Lux SICAV umbrella
          2. Create ManCo entity
          3. Create SPV entity
          4. Create Investment Manager entity
          5. Assign ManCo to SICAV
          6. Link SPV and IM to structure

        I need a few details:
        - Name for the SICAV?
User:   Allianz Luxembourg Fund III
Agent:  - ManCo name?
User:   Use the existing Allianz Global manco
Agent:  - SPV name?
User:   Allianz Lux SPV III
Agent:  Updated plan:                                             [Re-playback]
          1. Create Allianz Luxembourg Fund III as Lux SICAV
          2. Link existing ManCo: Allianz Global Investors GmbH
          3. Create Allianz Lux SPV III as SPV
          4. Create Investment Manager (name TBD)
          ...
        Confirm?
```

### 12.4 Pack Shorthand (The Adoption Test)

```
User:   Onboard Allianz Lux CBU book
Agent:  Routing to Onboarding Request.                           [Direct pack match]
        Using the Allianz Lux Umbrella CBU.

        Which products? And do you need a trading matrix?         [Only slot-filling Qs]
User:   IRS and CDS, with trading matrix
Agent:  You're onboarding Allianz Lux Umbrella with 2 products   [Pack-level playback]
        (IRS, CDS) and a standard trading matrix, ready for
        production onboarding.

        4 steps in your runbook. Confirm?
```

### 12.5 Pack Handoff

```
Agent:  ✓ Book setup complete. Allianz Luxembourg Fund III       [Book Setup pack done]
        is ready with ManCo, SPV, and IM linked.
        Would you like to start onboarding for this book?
User:   Yes
Agent:  Starting Onboarding Request for Allianz Luxembourg       [Handoff → new pack]
        Fund III. Context carried forward from book setup.
        Which products?                                           [Slot-filling in new pack]
```

---

## 13. Technical Decisions

### 13.1 Dedicated Sentence Templates, Not Search Phrases

**Decision:** Sentence playback uses `sentences.step[]` templates, which are separate from `search_phrases` (the renamed `invocation_phrases`).

**Rationale:** Search phrases are optimised for embedding similarity (keyword fragments, duplicates, abbreviations). Sentence templates must produce grammatically correct English. Using search phrases for playback is a long-term UX tax that produces awkward, robotic output. The fallback path (use search phrases when no sentence templates exist) enables incremental migration.

### 13.2 Templates over LLM for Sentence Generation

**Decision:** Sentence playback uses verb sentence templates, not LLM generation.

**Rationale:** Deterministic, sub-millisecond, zero cost, auditable. The LLM is used only for arg extraction (Stage 2), where non-determinism is acceptable and validated by sentence playback (Stage 3). If the LLM extracts wrong args, the user catches it in the played-back sentence and corrects.

### 13.3 Always Runbook, Even for One Command

**Decision:** Every interaction builds a runbook. A single command is a 1-entry runbook.

**Rationale:** Consistent UX. No special-casing. Users learn one workflow. Compliance gets one audit model. The "auto-execute for navigation verbs" trapdoor is removed — navigation verbs still go through sentence playback but use `ConfirmPolicy::QuickConfirm` (a UX affordance, not a code bypass). All flows create a runbook entry.

### 13.4 Direct DSL as Power-User Escape

**Decision:** Direct DSL input `(verb.name :arg value)` is preserved but wraps into the sentence flow.

**Rationale:** Power users need direct DSL access. But instead of a trapdoor to `DslReady`, direct DSL is parsed, a sentence is generated from the parsed verb + args, and it enters the normal Stage 3 → Stage 4 flow. This means direct DSL still gets sentence confirmation and runbook tracking.

### 13.5 Lexicon Pipeline as Fast Path

**Decision:** The lexicon/grammar pipeline becomes an optional fast path within `IntentService`, not a separate pipeline.

**Rationale:** The lexicon tokenizer is excellent for exact-match patterns ("create entity Allianz as limited company in LU"). When it matches, it can skip the LLM arg extraction entirely — the grammar has already parsed the args. This is faster and cheaper. But it must still produce a sentence and flow through Stage 3.

### 13.6 Scope Resolution as State, Not Conditional

**Decision:** Scope resolution is the `ScopeGate` state, not a `#[cfg(feature)]` conditional inside a pipeline.

**Rationale:** Scope is a hard prerequisite for entity resolution. Making it a state means the UI can render it explicitly, the session always knows whether scope is set, and there is no way to accidentally skip it. The orchestrator checks `session.has_scope()` before dispatching to Stage 1.

### 13.7 Always Inside a Pack

**Decision:** The agent never operates outside a journey pack. Free-form verb discovery is only available within a pack's `allowed_verbs`.

**Rationale:** This is the single rule that eliminates most trapdoors. If the user asks for something outside any pack, the agent asks "which journey are you working on?" rather than attempting free-form discovery. This makes the system predictable, explainable, and auditable.

---

## 14. Implementation Roadmap

### Phase 0 — Lay the Tracks (2–4 days)

**Deliverable:** A working REPL skeleton that can:
- select client group
- select journey pack (from hardcoded list)
- create an empty runbook
- append steps manually (verb + args → sentence + DSL)
- compile/validate via a stub executor
- render playback via sentence generator

**Files created:**
- `rust/src/repl/sentence_generator.rs` — template-based sentence generation
- `rust/src/repl/runbook.rs` — unified runbook model (with provenance, audit fields)
- `rust/src/journey/pack.rs` — pack manifest types + loader
- `rust/src/journey/handoff.rs` — pack handoff types

**Files modified:**
- `rust/src/repl/types.rs` — new `ReplState` enum (7 states, `InPack` replaces `Listening`)
- `rust/src/repl/orchestrator.rs` — new dispatch skeleton

**Success condition:** One clean "golden loop" exists end-to-end, even if verbs are fake and execution is stubbed.

### Phase 1 — Journey Packs + Starter Packs + Sentence Library (Week 1–2)

**Deliverable:** The Journey Pack framework plus **3 starter packs** that users can actually run:
1. **Book setup** (Lux/UK patterns)
2. **Onboarding request** (CBU + products + trading matrix)
3. **KYC case** (open case / request docs / review gates)

Each pack ships with:
- pack manifest YAML (scope, definition-of-done, question policy with `answer_kind` semantics)
- **at least 2 templates** (canonical runbook skeletons)
- `sentences.step[]` and `sentences.clarify` templates for all verbs used by those templates

**Key engineering:**
- Pack YAML loader integrated with `RuntimeRegistry`
- Pack routing via `HybridVerbSearcher` (reuse Candle BGE, index pack invocation phrases)
- Template instantiation: skeleton → runbook entries with arg substitution + slot provenance
- Pack-level playback from `pack_summary_template`
- Arg extraction audit stored per entry

**Definition of Done (Phase 1):**

Phase 1 is "done" only if:
- a user can say: "Onboard Allianz Lux CBU book"
- the system routes to Onboarding pack (or asks to choose pack)
- asks < 5 slot-filling questions
- produces a readable plan (pack-level summary + chapters + step sentences)
- runbook is editable (remove/reorder/edit args)
- execution requires explicit "run"
- durable/human-gate step shows parked status
- pack version and template provenance are recorded in the runbook

### Phase 2 — Verb Registry vNext + Pipeline Consolidation (Week 3–4)

**Deliverable:** Expand verb registry format with separated sentence fields. Merge three pipelines into one `IntentService`.

**Verb registry changes:**
- Rename `invocation_phrases` → `search_phrases` (embedding index, unchanged behaviour)
- Add `sentences.step[]` — templates for per-step playback
- Add `sentences.summary[]` — templates for plan-level summary
- Add `sentences.clarify` — per-arg clarification prompts
- Add `required` vs `optional` arg classification
- Add `confirm_policy` per verb
- Add `journey_tags` — which packs can use this verb

**Pipeline consolidation:**
- `rust/src/repl/intent_service.rs` — merges `IntentPipeline`, `HybridIntentMatcher`, and relevant parts of `LexiconPipeline`
- Clean phase separation: scope → pack route → verb search → arg extraction → sentence gen → DSL assembly
- Each phase is a pure function returning an outcome enum
- Orchestrator calls phases explicitly

**Files deprecated:**
- `mcp/intent_pipeline.rs` process methods → `repl/intent_service.rs`
- `repl/intent_matcher.rs` → `repl/intent_service.rs`

**Success condition:**
- Single entry point for all user input (one `process` method)
- Playback consistently high-quality across packs (using `sentences.step` not search phrases)
- Validation errors become user-fixable questions rather than dead ends

### Phase 3 — Proposal Engine (Week 5)

**Deliverable:** Deterministic proposal engine that:
- takes current runbook + last user message + journey pack
- produces a ranked list of **step proposals** with evidence
- never executes; only proposes edits

**Rules:**
- prefer templates first (fast path)
- then verb matching (fallback, within pack's `allowed_verbs`)
- always include "why this verb" explanation
- output must be reproducible given same inputs

**Success condition:** For common prompts, proposes the correct template/steps reliably.

### Phase 4 — Runbook Editing UX + Conversational Clarification (Week 6)

**Deliverable:** Complete co-authoring experience.

**Runbook editing:** User can accept, reject, edit step args, disable/reorder steps, see playback update immediately.

**Conversational clarification:** Replace pickers/modals with natural language questions using `sentences.clarify` templates. Multi-turn clarification loop within `Clarifying` state. Entity resolution via conversational follow-up.

**Success condition:** Runbook feels like "the thing," chat feels like "the assistant to edit the thing."

### Phase 5 — Durable Execution + Human Gates + Wiring (Week 7–8)

**Deliverable:** Step execution modes + full API surface + pack handoff.

**Execution modes:**
- `Sync` (immediate)
- `Durable` (starts, parks, resumes on signal)
- `HumanGate` (review/approval required)

**Pack handoff:** First-class system action for transitioning between packs with context forwarding.

**Wiring:**
- Update `api/repl_routes.rs` to new orchestrator
- Update `mcp/handlers/runbook.rs` to new runbook model
- MCP tools delegate to unified path
- WebSocket event stream for execution progress

**Success condition:** Runbook can park for days and resume without losing narrative continuity.

### Phase 6 — Decommission Old Trapdoors (Week 9)

**Deliverable:** Parity matrix listing old flows → new journeys. Delete paths once covered.

**Files deprecated:**
- `session/dsl_sheet.rs` → `repl/runbook.rs`
- `repl/service.rs` (RunbookService) → unified runbook ops in orchestrator
- Remove `Idle`, `IntentMatching`, `DslReady` states, all six `ClarifyingState` variants
- Remove auto-execute trapdoor, macro-expansion side path, scope-as-conditional

**Success condition:** "The messy REPL" is unused and removable.

---

## 15. What Not to Change

The following components are solid and should be reused, not rewritten:

| Component | Location | Reason |
|---|---|---|
| `HybridVerbSearcher` | `mcp/verb_search.rs` | Candle BGE semantic search works. Wrap it, point it at pack phrases too. |
| `ScopeResolver` | `mcp/scope_resolution.rs` | Stage 0 logic is correct. Promote to state, don't rewrite matching. |
| `RuntimeRegistry` | `dsl-core/config/types.rs` | Verb definitions, arg schemas, validation. Stable. Extend for packs + sentences. |
| `DagAnalyzer` | `repl/dag_analyzer.rs` | Dependency analysis for execution ordering. |
| `phrase_gen` | `dsl-core/config/phrase_gen.rs` | Synonym expansion. Reuse for sentence fallback. |
| `EntityArgResolver` | entity resolution | Tag, fuzzy, semantic matching for entity refs. Wrap the existing resolver. |
| `ob-semantic-matcher` | `rust/crates/ob-semantic-matcher/` | Embedding infrastructure. |

---

## 16. Non-Functional Requirements

### Determinism & Audit
- Runbook edits are event-sourced
- Proposal engine is reproducible (same inputs → same outputs)
- Playback is derived solely from runbook + sentence templates (no LLM in the render path)
- LLM arg extraction is audited per entry (model, prompt hash, extracted args, timestamp)
- Pack version and manifest hash pinned at runbook creation

### Performance
- Orchestrator operations are small (state machine + lookups)
- Sentence generation: <1ms (template substitution only)
- Compile/execute is the only heavy stage
- End-to-end (intent match + arg extract + sentence): <2s
- UI updates are incremental (diff-based)

### Safety
- No implicit side effects
- "Execute" requires explicit confirmation (policy-configurable per step)
- Durable tasks are explicit steps with visible lifecycle
- Agent operates only within pack allow-list

---

## 17. Success Criteria

### User Experience
- Average 3–5 turns from greeting to runbook execution (down from 8–12)
- Users never see raw DSL unless they opt into power-user mode
- Pack-level playback reads like a plan, not a transcript
- Sentence confirmation rate >90% on first playback
- <5 clarifying questions for standard journeys

### Technical
- Single entry point for all user input
- Zero trapdoors: every code path goes through sentence playback
- All state transitions logged with sentence + DSL + outcome
- Agent always operates inside a pack
- `InPack` state always knows pack_id, remaining slots, last proposal

### Compliance
- Every executed command has a sentence trail (human-readable audit)
- Runbooks are persistent, versioned, and recoverable
- No command executes without explicit user intent
- Pack allow-list constrains what the agent can propose
- LLM arg extraction is auditable per entry
- Slot provenance explains user-provided vs defaulted vs inferred

---

## 18. Acceptance Tests (Golden Scenarios)

### Scenario A — Onboarding Request
**User:** "Use Allianz Lux CBU, add 4 products and a common trading matrix ready for onboarding."
**Pass if:**
- Journey chosen: Onboarding Request pack
- Questions asked: only what's missing (e.g., which 4 products if not named)
- Playback: coherent pack-level summary sentence
- Runbook: step list with sentences + args
- Execute: runs (or parks) with visible outcomes per step
- Runbook records pack version, template provenance, slot provenance

### Scenario B — Book Setup
**User:** "Set up a Lux SICAV with a manco, an SPV, and an investment manager."
**Pass if:**
- Template proposed: Lux SICAV canonical
- Clarifications limited to required gaps (names, existing entities)
- Runbook produced even before any execution
- Slot provenance distinguishes user-provided names from defaults

### Scenario C — KYC Case
**User:** "Start a KYC case for Goldman Sachs London branch."
**Pass if:**
- Routes to KYC Case pack
- Proposes new case creation step
- Asks minimal disambiguation (which Goldman entity?)
- Parks if human gate/doc request is needed

### Scenario D — Pack Shorthand (The Adoption Test)
**User:** "Onboard Allianz Lux CBU book."
**Pass if:**
- System routes to Onboarding Request pack (or asks user to pick)
- Proposes a template runbook immediately
- Asks only slot-filling questions — not open-ended exploration
- Produces a one-paragraph pack playback that reads like an onboarding plan

### Scenario E — Power User Direct DSL
**User:** `(cbu.assign-product :cbu-name "Allianz Lux" :product "IRS")`
**Pass if:**
- DSL parsed, sentence generated: "Add IRS product to Allianz Lux"
- Sentence played back for confirmation (not auto-executed)
- Added to runbook as a normal entry
- DSL not shown in default view (only sentence)

### Scenario F — Pack Handoff
**User:** Completes Book Setup, then says "Now onboard this book"
**Pass if:**
- New runbook created for Onboarding Request pack
- Context (client group, CBU name, book ID) forwarded from Book Setup
- Forwarded slots marked as `CopiedFromPrevious` in provenance
- User only asked for new information (products, trading matrix)

---

## 19. Open Questions

1. **Sentence editing:** When a user says "no, just IRS not CDS" — re-run LLM extraction with correction context, or patch args directly? → **Recommendation: re-run with correction context.**

2. **Runbook templates (user-created):** Save a runbook as a reusable template? ("Do the same thing for BlackRock Lux.") → **Phase 2 feature after core loop is solid.**

3. **Multi-verb sentences:** "Add IRS and create a trading matrix" — one sentence, two verbs. Split into two entries or handle compound? → **Recommendation: split. Each entry = one verb = one sentence.**

4. **Macro expansion in sentences:** Macros expand to multiple primitives. Show the macro sentence or the expanded primitives? → **Recommendation: show macro sentence, with expand/collapse in runbook view.**

5. **Error recovery mid-runbook:** If step 3 of 5 fails, stop or continue? → **Recommendation: stop, show status, let user decide (retry/skip/abort).**

6. **Pack evolution:** How do packs version? Can a pack be hot-reloaded without restarting? → **Pack YAML files loaded at startup via `RuntimeRegistry`. Version field enables migration. Manifest hash pinned at runbook creation. Hot-reload is Phase 2.**

7. **Cross-pack operations:** Can a runbook span multiple packs? → **No. One runbook = one pack. Use pack handoff for cross-pack workflows (§7.6).**

---

## 20. File Inventory

### Files to Create (New)

| File | Purpose |
|---|---|
| `rust/src/repl/sentence_generator.rs` | Template-based sentence generation (sentences.step → search_phrases → phrase_gen fallback) |
| `rust/src/repl/intent_service.rs` | Unified intent pipeline (scope + pack route + verb + args + sentence + DSL) |
| `rust/src/repl/runbook.rs` | Unified runbook model with provenance, audit, visibility policy |
| `rust/src/journey/pack.rs` | Pack manifest types, loader, router |
| `rust/src/journey/template.rs` | Template instantiation (skeleton → runbook entries with slot provenance) |
| `rust/src/journey/playback.rs` | Pack-level playback (summary + chapter view) |
| `rust/src/journey/handoff.rs` | Pack handoff types and context forwarding |
| `config/packs/book-setup.yaml` | Starter pack: Book Setup |
| `config/packs/onboarding-request.yaml` | Starter pack: Onboarding Request |
| `config/packs/kyc-case.yaml` | Starter pack: KYC Case |

### Files to Heavily Modify

| File | Change |
|---|---|
| `rust/src/repl/orchestrator.rs` | New state machine, pack-guided dispatch, sentence playback loop |
| `rust/src/repl/types.rs` | New `ReplState` enum (7 states, `InPack` with pack context), `ConfirmPolicy` |
| `rust/src/repl/response.rs` | Sentence-first response payloads, pack playback, visibility policy |
| `rust/src/api/repl_routes.rs` | Wire to new orchestrator |
| `rust/src/mcp/handlers/runbook.rs` | Wire to new runbook model |
| `rust/crates/dsl-core/src/config/types.rs` | Add `sentences` field (step/summary/clarify), rename `invocation_phrases` → `search_phrases`, add `confirm_policy` |

### Files to Deprecate (Phase 6)

| File | Replaced By |
|---|---|
| `rust/src/session/dsl_sheet.rs` | `repl/runbook.rs` |
| `rust/src/repl/service.rs` | Unified runbook operations in orchestrator |
| `rust/src/mcp/intent_pipeline.rs` | `repl/intent_service.rs` (MCP tools delegate to it) |
| `rust/src/repl/intent_matcher.rs` | `repl/intent_service.rs` |

### Files to Keep As-Is

| File | Reason |
|---|---|
| `rust/src/mcp/verb_search.rs` | Solid. Wrapped by IntentService. Extended for pack routing. |
| `rust/src/mcp/scope_resolution.rs` | Solid. Called by IntentService. |
| `rust/src/repl/dag_analyzer.rs` | Solid. Used for execution ordering. |
| `rust/src/repl/staged_runbook.rs` | Types may be kept for DB compatibility during migration. |
| `rust/crates/dsl-core/src/config/phrase_gen.rs` | Reused for sentence generation fallback. |
| `rust/crates/ob-semantic-matcher/` | Embedding infrastructure unchanged. |

---

## 21. Summary: The Commitment

- The **runbook** is the artifact — if it isn't in the runbook, it didn't happen
- **Journey Packs** are the product API — packs define UX, verbs define execution
- **Sentence templates** (separate from search phrases) give trust
- The REPL is a **deterministic loop** (7 states, no trapdoors)
- The **agent always operates inside a pack** — or asks the user to pick one
- **One runbook = one pack** — use handoff for cross-pack workflows
- Execution is always **explicit and visible** (with policy-driven confirmation)
- DSL is **never shown by default** — sentences are the user-facing interface
- Every LLM call is **audited** — model, prompt, extraction, confidence
- Every slot is **provenanced** — user-provided, defaulted, inferred, or forwarded
- **vNext is greenfield discipline**; rollout is feature-flagged incremental migration
- Old code is the **deletion target**

---

*This document merges product vision (Journey Packs, golden loop, adoption philosophy) with engineering blueprint (code forensics, Rust types, pipeline consolidation, file inventory) and incorporates all review redlines (separated sentence templates, conversational semantics, pack versioning, slot provenance, arg extraction audit, visibility policy, confirmation policy, pack handoff, InPack state). The next step is Phase 0: build the sentence generator, runbook model, and pack loader skeleton.*
