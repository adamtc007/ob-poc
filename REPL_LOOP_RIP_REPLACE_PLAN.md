# REPL Loop Re‑engineering (Rip & Replace) — Implementation Re‑Plan

**Goal:** Replace the current “chat + trapdoors + stub paths” with a **single, coherent REPL loop** whose *only durable output* is a **Runbook/Sheet**.  
The agent’s job is to **build, explain, and iterate** that runbook via Q/A; the runtime’s job is to **compile/execute** it deterministically; the UI’s job is to **render and edit** it.

---

## 1) The Golden Loop (Non‑Negotiable)

### Loop (single mental model)
1. **Select Client Group Context**
2. **Select Journey Mode** (Book setup / Onboarding request / KYC case / New case request / etc.)
3. **Q/A to Resolve Intent** (minimal questions; stop as soon as we can act safely)
4. **Propose Runbook Step(s)** (candidate steps with evidence + “why”)
5. **Playback Understanding** (verb sentence templates assembled into plain English)
6. **User Edits / Confirms**
7. **Compile → Execute** (sync or durable)
8. **Observe Outcomes** (results + next questions)
9. **Append/Refine Runbook**
10. **Repeat until Done / Hand‑off**

### Prime directive
> **If it isn’t in the runbook, it didn’t happen.**  
No side effects, no “helpful hidden actions,” no alternate execution paths.

---

## 2) Root-and-Branch Vision (What we’re building)

### 2.1 Runbook/Sheet is the Product
The runbook is a **first‑class, user‑readable artifact**:
- versioned (diffable)
- auditable (who/what/why)
- executable (compilable)
- explainable (playback sentences)
- editable (user can reorder/disable/param‑edit steps)

### 2.2 Journey Packs (first‑class capability — the “secret sauce”)
Journey Packs are the **user-facing unit of work** that sits *above* atomic DSL verbs and *above* single-entity (CBU) operations.

They are what makes an agent-led approach feel like **shorthand for a complete intention**:

> “I want to onboard Allianz Lux CBU book…”  
…is not 40 verbs. It’s **one pack + one template**, with a small, guided Q/A to fill gaps, producing a runbook the user can read and approve.

#### Why packs make or break adoption
Users will not “trust” a free-form agent if it feels like:
- random questions
- unpredictable next actions
- verb soup
- unclear progress/completion

Packs solve this by providing:
- **Predictability**: “this is the onboarding journey; these are the steps”
- **Speed**: template-first planning (fast path)
- **Safety**: explicit scope limits + required fields
- **Trust**: coherent playback (“what we think you’re doing”) at pack and step level
- **Completeness**: definition-of-done and progress tracking

#### What a Journey Pack contains (Pack Manifest)
A pack is a versioned manifest (YAML/JSON) plus optional helper code. Minimum fields:

- **Identity**
  - `pack_id`, `name`, `version`, `description`
- **Entry prompts / routing**
  - `invocation_phrases[]` (for pack selection)
  - `required_context` (e.g., needs `client_group`, optionally `book_id`/`case_id`)
- **Scope & safety**
  - `allowed_verbs[]` (hard allow-list)
  - `forbidden_verbs[]` (optional hard deny-list)
  - `risk_policy` (when execution requires explicit confirm)
- **Goal schema**
  - `definition_of_done` (conditions over runbook + outcomes)
  - `progress_signals[]` (what to show as “done/remaining”)
- **Question policy**
  - `required_questions[]` (minimal set)
  - `optional_questions[]` (only ask if needed)
  - `stop_rules` (when to stop asking and propose a plan)
- **Templates (canonical runbook skeletons)**
  - `templates[]` each with:
    - `template_id`, `when_to_use`
    - `step_skeleton[]` (verb IDs + required args slots)
    - `defaults` and `arg_inference_rules`
- **Playback narrative**
  - `pack_summary_template` (one-paragraph explanation)
  - `section_layout_rules` (group steps into human-readable chapters)

> Important: packs must work even if intent-to-verb matching is imperfect, because the **template fast-path** carries most real usage.

#### Pack selection (deterministic)
The orchestrator must choose a pack deterministically via:
1. explicit user selection (preferred)
2. pack routing rules (invocation phrases + context)
3. fallbacks (ask: “Are you doing Book Setup, Onboarding Request, or KYC Case?”)

#### Example: Onboarding Request Pack (high-level)
**Intent shorthand:** “Onboard Allianz Lux CBU book to products + trading matrix.”

Template outputs a runbook with chapters:
1. **Context** — select client group, choose CBU/book
2. **Products** — add custody/fund accounting/TA/etc
3. **Trading Setup** — apply instrument universe + counterparties + margin/legal profile
4. **Readiness** — validate completeness, generate onboarding request artifact
5. **Handoffs** — create tasks/requests for downstream teams (durable/human gates)

Clarification questions are constrained to missing slots (e.g., “Which 4 products?” “Which instrument universe?”), not open-ended.

#### Operational rule
> The agent never “invents a journey.” It must always operate *inside a pack* — or ask the user to pick one.

That single rule eliminates most trapdoors and makes the system explainable.

### 2.3 Verb Sentence Templates (Adoption Lever)
Every verb must provide structured sentences that the agent can assemble into:
- **Step playback**: “Add product *Custody* to *Allianz Lux SICAV*.”
- **Plan summary**: “You’re using Allianz Lux CBU, adding 4 products, and applying a common trading matrix for onboarding to BNY production.”
- **Clarification prompts**: “To add the trading matrix, I need the instrument universe and counterparties. Which applies?”

This converts the “agent’s internal representation” into **human confidence**.

---

## 3) The New System Boundary (Rip & Replace)

### 3.1 Hard split into three concerns
1. **REPL Orchestrator (new)**  
   Owns: state machine, Q/A policy, proposal generation, playback, runbook editing.
2. **Runbook Store (new or refactored)**  
   Owns: persistence, versioning, diffing, audit, event stream.
3. **Compiler/Executor (existing-ish, but treated as a black box)**  
   Owns: compile, validate, execute, produce outcomes/events.

**Key discipline:** the orchestrator never “does work directly.” It only **writes runbook steps** and requests compile/execute.

### 3.2 No compatibility constraints
This is explicitly **not** “work with the current messy code.”  
We build a clean vNext loop and then:
- route traffic gradually
- deprecate old pathways
- delete trapdoors once coverage is met

---

## 4) Core Data Artifacts (Minimal & Clean)

### 4.1 Client Context
A stable object that answers: “what client group am I working on?”
- `client_group_id`
- optional: default CBU/book/case preferences
- permissions / tenancy

### 4.2 Runbook (the only durable truth)
- `runbook_id`
- `journey_type`
- `context` (client group + selected book/case references)
- `steps[]` (ordered list)
- `status` (Draft / Active / Parked / Completed)
- `audit[]` (events: created, edited, executed, resumed)

### 4.3 Step (single atomic intent)
- `verb_id`
- `args` (typed)
- `labels` (for UI and summary)
- `sentence_slots` (filled values for playback)
- `execution_mode` (sync / durable / human-gated)
- `state` (Proposed / Confirmed / Executed / Failed / Parked)

### 4.4 Outcome Events
Compiler/executor emits events the orchestrator consumes:
- `validation_error`
- `execution_result`
- `durable_task_started`
- `external_signal_received`
- `human_review_required`
- `step_completed`

---

## 5) REPL Orchestrator: Deterministic State Machine

### 5.1 States (no hidden branches)
1. **Start**
2. **ClientContextSelected**
3. **JourneySelected**
4. **IntentGathering**
5. **PlanProposed** (runbook changes prepared, not yet applied)
6. **PlaybackPresented**
7. **UserEditOrConfirm**
8. **CompileValidate**
9. **Execute**
10. **Observe**
11. **Loop**

### 5.2 Clarification Policy (minimal questions)
The orchestrator asks questions only when:
- the next step would be **unsafe** or **meaningfully ambiguous**
- the journey pack declares a field as **required**
- the executor returns a validation error that can be fixed by user input

Stop criteria:
- we can propose a safe step with defaults OR
- we can propose multiple options with clear tradeoffs

---

## 6) Implementation Re‑Plan (Rip & Replace Roadmap)

### Phase 0 — Lay the tracks (2–4 days of focused build)
**Deliverable:** a working REPL skeleton that can:
- select client group
- select journey
- create an empty runbook
- append steps manually
- compile/validate via a stub executor
- render playback via stub sentences

Success condition:
- one clean “golden loop” exists end‑to‑end, even if verbs are fake.

### Phase 1 — Journey Packs System vNext (first-class) + Starter Packs + Sentence Library
**Deliverable:** the Journey Pack framework plus **3 starter packs** that users can actually run:
1. **Book setup** (Lux/UK patterns)
2. **Onboarding request** (CBU + products + trading matrix)
3. **KYC case** (open case / request docs / review gates)

Each pack ships with:
- pack manifest (scope, definition-of-done, question policy)
- **at least 2 templates** (canonical runbook skeletons)
- minimal sentence templates for the verbs used by those templates (so playback works immediately)

Success condition:
- user can choose a pack and get a coherent runbook proposal with **<5 clarifying questions**
- pack-level playback reads like a plan, not a transcript

### Phase 2 — Verb Registry vNext: full sentences + required fields + clarification hooks
**Deliverable:** expand the verb registry format (or layer) to cover *all* verbs you intend to expose, including:
- `args` schema and validation
- **step sentences** + **summary sentences** + **clarification prompts**
- required vs optional fields
- journey tags (which packs can use it)

Success condition:
- playback is consistently high-quality across packs
- validation errors become user-fixable questions rather than dead ends

### Phase 3 — Proposal Engine (intent → candidate steps)
**Deliverable:** deterministic proposal engine that:
- takes current runbook + last user message + journey pack
- produces a ranked list of **step proposals** with evidence
- never executes; only proposes edits

Rules:
- prefer templates first (fast path)
- then verb matching (fallback)
- always include “why this verb” explanation
- output must be reproducible given same inputs

Success condition:
- for common prompts, it proposes the correct template/steps reliably.

### Phase 4 — Runbook Editing UX (agent + user co-authoring)
**Deliverable:** user can:
- accept proposal
- reject proposal
- edit step args
- disable/reorder steps
- see playback update immediately

Success condition:
- runbook feels like the “thing,” chat feels like the “assistant to edit the thing.”

### Phase 5 — Durable execution + human gates (KYC reality)
**Deliverable:** step execution modes:
- `sync` (immediate)
- `durable` (starts, parks, resumes on signal)
- `human_gate` (review/approval required)

Success condition:
- the runbook can park for days and resume without losing narrative continuity.

### Phase 6 — Decommission old trapdoors (feature flag + parity matrix)
**Deliverable:** parity matrix listing old flows → new journeys.
Delete paths once covered.

Success condition:
- “the messy REPL” is unused and removable.

---

## 7) Non‑Functional Requirements (baked in)

### Determinism & Audit
- runbook edits are event‑sourced
- proposal engine is reproducible
- playback is derived solely from runbook + sentences

### Performance
- orchestrator operations are small (state machine + lookups)
- compile/execute is the only heavy stage
- UI updates are incremental (diff-based)

### Safety
- no implicit side effects
- “execute” requires explicit confirmation (journey-configurable)
- durable tasks are explicit steps with visible lifecycle

---

## 8) What to Build First (Pragmatic sequencing)

If you want the shortest path to user acceptance:
1. **Journey selection + templates** (packs make it feel like “shorthand for the whole job”)
2. **Playback engine** (sentences) + runbook viewer/editor (trust and readability)
3. **Only then** expand broad “intent → verb discovery” beyond pack routing

Reason: users adopt the system when it feels **structured (packs)** and **self-explaining (playback)** — not when it feels “smart.”

---

## 9) Concrete Acceptance Tests (Golden Scenarios)

### Scenario A — Onboarding request (your example)
User: “Use Allianz Lux CBU, add 4 products and a common trading matrix ready for onboarding.”
Pass if:
- journey chosen: Onboarding Request
- questions asked: only what’s missing (e.g., which 4 products if not named)
- playback: coherent summary sentence
- runbook: step list + args
- execute: runs (or parks) with visible outcomes

### Scenario B — Book setup
User: “Set up a Lux SICAV with a manco, an SPV, and an investment manager.”
Pass if:
- template proposed: Lux SICAV canonical
- clarifications limited to required gaps
- runbook produced even before any execution

### Scenario C — KYC case request
User: “Start a KYC case for Goldman Sachs London branch.”
Pass if:
- proposes new case creation step
- asks minimal disambiguation
- parks if human gate/doc request is needed

---


### Scenario D — Journey pack shorthand (the adoption test)
User: “Onboard Allianz Lux CBU book.”
Pass if:
- system routes to **Onboarding Request Pack** (or asks the user to pick a journey)
- proposes a template runbook immediately
- asks only slot-filling questions (products, matrix, counterparties) — not open-ended exploration
- produces a one-paragraph pack playback that reads like an onboarding plan

## 10) Summary: the rip & replace commitment

- The **runbook is the artifact**
- The REPL is a **deterministic loop**
- Journeys give **structure**
- Sentence templates give **trust**
- Execution is always **explicit and visible**
- Old code is not the constraint; it’s the deletion target

