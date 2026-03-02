Scenario-Based Intent Resolution

Vision & Scope Paper — ob-poc
Status: v0.4 — Phase 0.5 + Phase 1 implemented
Date: 2 March 2026
Author: Adam / Claude collaborative design

⸻

Revision History

Version	Date	Changes
v0.1	2 Mar 2026	Initial vision, research question, capability mapping
v0.2	2 Mar 2026	MacroIndex + ScenarioIndex split; deterministic scoring ledger with hard gates; single-verb blockers for false positive control; compiler-like YAML schema; provenance-only session state; prereq-based sequence validation; explain payload; macro search parity phase; test harness spec
v0.3	2 Mar 2026	Fold “Tier 0 macro exact match” into MacroIndex (removes precedence ambiguity); fix Luxembourg doc bundle example; introduce routes.kind: macro_selector for parameterised journeys; define deterministic MacroIndex scoring + thresholds; bind single-verb blocker to ECIR “strong single-verb” result; add three-valued prereq validation (Pass/Fail/Deferred); require explain payload for MacroIndex + DecisionPackets; add explicit Non-Goals
v0.4	2 Mar 2026	Implementation status: Phase 0.5 (MacroIndex) ✅ complete — macro_index.rs, macro_search_overrides.yaml, wired as Tier -2B in HybridVerbSearcher; Phase 1 (ScenarioIndex + CompoundIntent) ✅ complete — scenario_index.rs, compound_intent.rs, sequence_validator.rs, scenario_index.yaml (10 scenarios), ECIR short-circuit compound signal gate, wired as Tier -2A; all 1565 unit tests pass, zero regressions


⸻

1. The Problem

An analyst says: “Onboard this Luxembourg SICAV with three sub-funds.”

The current pipeline tries to find a single DSL verb. It scores
fund.create-umbrella at 0.80, cbu.create at 0.76, maybe
struct.lux.ucits.sicav at 0.72. It commits to the highest-scoring
single verb and the analyst gets a fraction of what they asked for.

But the analyst didn’t describe a verb. They described an outcome — a
fully onboarded fund structure with roles, mandates, and a KYC case.
That outcome maps to struct.lux.ucits.sicav (a 13-verb macro that
produces a complete runbook), not to any single DSL verb.

The root cause: the intent pipeline resolves to DSL verbs. But
analysts think in outcomes, not verbs. The pipeline needs an intermediate
resolution layer that matches outcome intent before attempting
verb-level dispatch.

The secondary root cause: macros live in Tier 0 of
HybridVerbSearcher — exact label/FQN match only. DSL verbs get the
benefit of Tiers 1-8 (lexicon, learned phrases, semantic embeddings,
phonetic matching). Macros get only exact label match. This asymmetry
is why the pipeline skips past the macro that would actually satisfy
the analyst’s intent and falls through to a single DSL verb at 0.80.

⸻

2. What Already Exists

Before designing anything new, we must understand what the architecture
already provides — because the answer may be closer than it appears.

2.1 Macros Already Do Most of This

The macro system is more powerful than its current role suggests:

Capability	Already Built?	Where
Multi-verb expansion	✓	18 macros expand to 2-24 DSL verbs
Runbook production	✓	Macro expands-to → RunbookEntry sequence
DAG prerequisites	✓	prereqs: [state_exists, verb_completed, any_of]
Unlock chains	✓	30 macros define what they unlock
Argument mapping	✓	${arg.*}, ${scope.*} variable expansion
State tracking	✓	sets-state flags for workflow progress
Mode-gated availability	✓	routing.mode-tags: [onboarding, kyc]
Operator vocabulary	✓	ui.label / ui.description — business language
Entity type awareness	✓	target.operates-on / target.produces

The jurisdiction-specific structure macros are already scenarios in
execution terms:

struct.lux.ucits.sicav → 13 DSL verbs:
  cbu.create
  docs-bundle.apply
  entity.ensure-or-placeholder (×3, for manco/custodian/admin)
  cbu-role.assign (×3)
  entity.ensure-or-placeholder (×2, for TA/auditor)
  cbu-role.assign (×2)
  trading-profile.create

This is indistinguishable from what a “scenario” would produce. The
macro expands-to already generates the ordered verb sequence that
becomes a Runbook.

2.2 The Runbook Already Handles Execution

The Runbook model (repl/runbook.rs) already provides everything a
scenario execution engine would need:
	•	Ordered entries with sentence + DSL + args + provenance
	•	Entry lifecycle: Proposed → Confirmed → Resolved → Executing → Completed
	•	Readiness checking (unresolved refs, confirmation status)
	•	Human gates and durable execution (park/resume)
	•	Audit trail (append-only RunbookEvent log)
	•	Progress tracking and narration
	•	Undo/redo, reorder, disable/enable steps
	•	Pending question derivation for incomplete args
	•	Slot provenance (UserProvided, TemplateDefault, InferredFromContext, CopiedFromPrevious)

2.3 The Unlocks DAG Already Defines Workflow Sequences

The unlocks field on macros forms a directed graph of operator
workflows:

structure.setup ──unlocks──→ structure.assign-role ──unlocks──→ case.open
                             mandate.create                    mandate.create
                             case.open

case.open ──unlocks──→ case.add-party
                       case.solicit-document
                       case.submit ──unlocks──→ case.approve
                                                case.reject

This DAG already encodes “after you set up a structure, you can assign
roles, open a case, or create a mandate.” The workflow sequence is
implicit in the unlock graph.

2.4 What’s Actually Missing

Given all of the above, the gap is surprisingly narrow:

The intent pipeline cannot match utterances to macros effectively.

Macros live in Tier 0 of HybridVerbSearcher — exact label/FQN match
only. If the analyst says “set up a Lux SICAV,” the search must exact-
match against struct.lux.ucits.sicav’s label “Lux UCITS (SICAV)” or
its FQN. There is no semantic matching, no noun-based routing, and no
compound intent recognition for macros.

DSL verbs get the benefit of Tiers 1-8 (lexicon, learned phrases,
semantic embeddings, phonetic matching). Macros get only exact label
match. This asymmetry is why the pipeline skips past the macro that
would actually satisfy the analyst’s intent and falls through to a
single DSL verb at 0.80.

v0.2 insight: The fastest win is to give macros search parity with
DSL verbs — make them eligible for the same deterministic tiers (lexicon,
learned phrases, phonetic, noun-index routing). Many utterances will
then correctly hit the right macro without a separate ScenarioIndex
entry. The ScenarioIndex becomes thin — reserved for multi-macro
journeys and cross-domain composite intents that no single macro
can absorb.

v0.3 clarification: the former “Tier 0 macro exact match” is folded
into the MacroIndex as a fast-path (exact label/FQN) inside Tier -2B.
This removes ambiguity about precedence and prevents “double searching.”

⸻

3. Research Question

Are scenarios composite DSL verbs that expand to macros?
Or something else?

3.1 Answer: Scenarios Are Neither

Scenarios are not a new construct type. They don’t need a new YAML
schema, a new runtime model, or a new execution engine.

Scenarios are an intent-matching enhancement that routes utterances to
existing macros — particularly the multi-verb macros that already
produce runbooks.

The architecture already has:
	•	DSL verbs (653) — atomic database operations
	•	Macros (47) — operator vocabulary; 29 single-verb, 18 multi-verb
	•	Runbook — ordered execution plan with audit trail

What it needs is a way for the intent pipeline to recognise composite
intent and route it to the right macro, instead of short-circuiting at
the first DSL verb that scores above threshold.

3.2 The Hierarchy, Clarified

┌─────────────────────────────────────────────────┐
│  INTENT LAYER (new — what this paper proposes)  │
│                                                 │
│  "Onboard a Luxembourg SICAV"                   │
│       ↓                                         │
│  Scenario Intent Match / MacroIndex Match       │
│  • noun: fund, sicav, luxembourg                │
│  • action: onboard / set up / create            │
│  • compound signal: jurisdiction + structure     │
│       ↓                                         │
│  Routes to: struct.lux.ucits.sicav (macro)      │
└────────────────────┬────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────┐
│  MACRO LAYER (existing — pre-processor)         │
│                                                 │
│  struct.lux.ucits.sicav                         │
│       ↓ expands-to                              │
│  [cbu.create, docs-bundle.apply,                │
│   entity.ensure-or-placeholder × 5,             │
│   cbu-role.assign × 5,                          │
│   trading-profile.create]                       │
│       ↓ produces                                │
│  Runbook with 13 entries                        │
└────────────────────┬────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────┐
│  RUNBOOK LAYER (existing — execution plan)      │
│                                                 │
│  1. Create Allianz Lux SICAV          [Proposed]│
│  2. Apply Luxembourg UCITS doc bundle [Proposed]│
│  3. Add Management Company            [Proposed]│
│  ...                                            │
│  13. Create trading mandate           [Proposed]│
│       ↓ execute (step by step, with human gates)│
└────────────────────┬────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────┐
│  DSL VERB LAYER (existing — atomic execution)   │
│                                                 │
│  Each RunbookEntry executes one DSL verb:       │
│  (cbu.create :name "Allianz Lux"                │
│              :kind sicav                         │
│              :jurisdiction LU)                   │
└─────────────────────────────────────────────────┘

Scenarios are the intent layer. Macros are the pre-processor. Runbook
is the execution plan. DSL verbs are the atomic operations.

There is no new runtime construct. A “scenario” is a classification
applied at intent time, not a thing that exists at execution time. By
the time the Runbook is built, the scenario has dissolved — it selected
a macro, the macro expanded, and from that point forward everything is
standard Runbook execution. Scenario identity may be stored only as
provenance metadata on RunbookEntries for narration and audit; it does
not affect execution.

⸻

4. Capabilities

4.1 Scenario Intent Recognition

Detect that an utterance describes a composite outcome rather than a
single operation.

Positive signals (favour scenario match):

Signal	Example	Interpretation
Outcome verbs	“onboard”, “set up”, “establish”, “spin up”	Not a DSL verb — composite
Jurisdiction + structure	“Lux SICAV”, “Irish ICAV”, “Delaware LP”	Jurisdiction-specific macro
Multiple nouns	“fund with share classes and roles”	Multi-step workflow
Phase language	“do the KYC”, “run screening”, “complete onboarding”	Workflow phase, not single verb
Quantifiers	“three sub-funds”, “all the roles”	Iteration over a template

Negative signals (single-verb blockers — favour ECIR fallthrough):

Signal	Example	Effect
Explicit atomic verb	“create umbrella fund”, “assign custodian”	Skip Tier -2, go ECIR
Single noun, no compound action	“open KYC case”, “add role”	Skip Tier -2, go ECIR
Direct DSL verb name	“cbu.create”, “fund.create-umbrella”	Skip Tier -2, go ECIR
Interrogative without outcome	“what documents are missing?”	Skip Tier -2, go verb search

v0.3 key control (deterministic): the single-verb blocker is bound
to ECIR. If ECIR produces exactly one verb candidate with confidence ≥ X,
and the utterance lacks (a) quantifiers, (b) jurisdiction+structure pair,
and (c) multi-noun workflow signals, then skip Tier -2 entirely and go
straight to ECIR.

This is the primary control for keeping scenario false positives below 5%.

These are distinct from single-verb signals. “Create an umbrella fund”
is a single verb (fund.create-umbrella). “Set up a Luxembourg SICAV”
is a scenario (struct.lux.ucits.sicav).

4.2 Deterministic Scoring Ledger (ScenarioIndex)

Scenario matching uses a fixed, auditable scoring table — no embeddings,
no ML. Every signal contributes a deterministic score, and hard gates
ensure precision.

Signal scoring table:

Signal Bucket	Examples	Score
Compound outcome verb	onboard / set up / establish / “spin up”	+4
Jurisdiction found	LU / Luxembourg / “Lux” (unambiguous)	+4
Structure noun	sicav / icav / LP / “umbrella fund”	+3
Phase noun	KYC / screening / mandate / custody	+2
Quantifier	“three sub-funds”, “all roles”	+2
Macro metadata match	macro declares jurisdiction=LU or structure=sicav	+3
Negative: single-verb cue	“create umbrella fund” / “add role” / “set attribute”	−6

Hard gates (all must pass):

Gate	Rule	Rationale
Gate 1: Compound signal	Must have either (a) compound outcome verb OR (b) jurisdiction + structure as a pair	Prevents “simple create” from matching
Gate 2: Mode compatibility	Scenario’s declared modes must include session’s current mode	Governance
Gate 3: Minimum score	Total score ≥ 8 after all signals evaluated	Prevents weak matches

Example scoring trace:

Utterance: "Onboard this Luxembourg SICAV with three sub-funds"

Signal: "onboard"              → compound outcome verb    +4
Signal: "Luxembourg"           → jurisdiction LU          +4
Signal: "SICAV"                → structure noun           +3
Signal: "three sub-funds"      → quantifier               +2
Signal: macro struct.lux.ucits.sicav matches LU + SICAV  +3
                                                    Total: 16

Gate 1: compound outcome verb present               ✓ PASS
Gate 2: mode = onboarding, scenario allows onboarding ✓ PASS
Gate 3: 16 ≥ 8                                       ✓ PASS

Result: ScenarioMatched { scenario_id: "lux-sicav-setup", score: 16 }

Utterance: "Create an umbrella fund"

ECIR (strong single-verb) → 1 candidate: fund.create-umbrella (conf ≥ X)
No quantifier, no jurisdiction+structure pair, no multi-noun workflow signals

Single-Verb Blocker fires → skip Tier -2 → resolve at ECIR

This gives determinism (same input always same output), tunability
(adjust scores in YAML without code changes), and explainability
(the scoring trace is the audit trail).

4.3 MacroIndex vs ScenarioIndex (v0.2 Split, v0.3 Tightening)

v0.1 proposed a single ScenarioIndex that routes to macros. Review
feedback identified that this risks duplicating data already present
on macro definitions (ui.label, routing.mode-tags, target.*,
jurisdiction implied by FQN, etc.).

v0.2 architecture: two-layer index.
v0.3 tightening: MacroIndex owns macro exact match + deterministic parity tiers.

Layer 1: MacroIndex (derived + curated, routes to one macro)

The MacroIndex gives macros search parity with DSL verbs. It is
derived from existing macro metadata at startup, with optional curated
overrides:
	•	ui.label → lexicon synonym match and primary phrase match
	•	ui.description → secondary phrase candidate
	•	target.operates-on / target.produces → noun-index routing
	•	FQN structure (e.g., struct.lux.ucits.sicav) → jurisdiction + structure extraction
	•	routing.mode-tags → mode-gated availability
	•	Curated: additional invocation phrases, aliases, natural language triggers

MacroIndex also includes the former Tier 0 exact match (label/FQN) as
an O(1) fast-path before other deterministic tiers.

This means many utterances (“set up Lux SICAV”) will correctly hit
struct.lux.ucits.sicav through the MacroIndex without a separate
ScenarioIndex entry — because the macro becomes “searchable like a verb.”

Layer 2: ScenarioIndex (thin, hand-curated, routes to macro sequence / selector)

The ScenarioIndex is reserved for cases that MacroIndex alone cannot
handle:
	•	Journey sequences — ordered multi-macro workflows
	•	Cross-domain composite intents — “full onboarding” spans structure,
mandate, and KYC domains
	•	Highly idiomatic phrasing — synonyms that don’t belong on a single macro
	•	Selection cases — “full onboarding journey” must choose a jurisdiction-specific
structure macro deterministically (see macro_selector below)

Resolution hierarchy:

Tier −2A: Journey ScenarioIndex (explicit YAML: macro_sequence / macro_selector / graph-walk)
Tier −2B: MacroIndex (derived + curated metadata, single macro match; includes exact-match fast-path)
Tier −1:  ECIR noun→verb
Tier  1+: Verb pipeline (lexicon/learned/phonetic/embedding etc for verbs)

4.4 Deterministic MacroIndex Scoring (v0.3)

MacroIndex matching is deterministic and auditable (no embeddings).
It uses a separate scoring model from scenarios (simpler, macro-centric).

MacroIndex scoring table (example defaults):

Signal Bucket	Examples	Score
Exact FQN match	struct.lux.ucits.sicav	+10
Exact label match	“Lux UCITS (SICAV)”	+8
Alias / phrase match	curated phrase hit	+6
Jurisdiction match	utterance → LU, macro → LU	+3
Mode match	session_mode ∈ macro.mode_tags	+2
Noun overlap	nouns intersect macro nouns	+2
Target kind match	entity kind / operates-on alignment	+2
Penalty: mismatch	wrong jurisdiction / wrong mode	−999 (hard exclude)

MacroIndex hard gates:
	•	Gate M1: mode compatibility (hard exclude if not compatible)
	•	Gate M2: minimum score threshold (default ≥ 6)
	•	Gate M3: disambiguation band (if top 2 within Δ ≤ 2 → DecisionPacket)

This keeps macro parity deterministic and explainable, without pulling
scenario-level logic into macro matching.

4.5 Scenario-to-Macro Routing

Given a recognised journey scenario intent, select the correct macro or
macro sequence. The ScenarioIndex YAML uses a compiler-like schema with
declarative match logic.

scenario_index_version: 1

scenarios:
  - id: lux-sicav-setup
    title: "Luxembourg UCITS SICAV Setup"
    modes: [onboarding, kyc]
    requires:
      any_of:
        - compound_action
        - all_of: [jurisdiction, structure]
    signals:
      actions: [onboard, setup, establish, create]
      jurisdictions: [LU, Luxembourg]
      nouns_any: [sicav, ucits, umbrella, sub-fund]
      phrases_any:
        - "set up a Luxembourg SICAV"
        - "onboard Lux UCITS"
        - "create a SICAV in Luxembourg"
        - "new Lux fund"
    routes:
      kind: macro
      macro_fqn: struct.lux.ucits.sicav
    explain:
      display_macro_steps: true

  - id: ie-icav-setup
    title: "Irish UCITS ICAV Setup"
    modes: [onboarding, kyc]
    requires:
      any_of:
        - compound_action
        - all_of: [jurisdiction, structure]
    signals:
      actions: [onboard, setup, establish, create]
      jurisdictions: [IE, Ireland]
      nouns_any: [icav, ucits, umbrella]
      phrases_any:
        - "set up an Irish ICAV"
        - "onboard Ireland UCITS"
        - "new ICAV"
    routes:
      kind: macro
      macro_fqn: struct.ie.ucits.icav
    explain:
      display_macro_steps: true

  - id: full-screening
    title: "Full KYC Screening"
    modes: [kyc]
    requires: { compound_action: true }
    signals:
      actions: [run, do, perform, complete]
      nouns_any: [kyc, screening, sanctions, pep, adverse-media]
      phrases_any:
        - "run the checks"
        - "do the KYC screening"
        - "run full screening"
    routes:
      kind: macro_sequence
      macros: [case.open, screening.full]
    explain:
      display_macro_steps: true

  - id: full-onboarding-journey
    title: "Complete Onboarding Journey"
    modes: [onboarding]
    requires:
      all_of: [compound_action]
    signals:
      actions: [onboard, "set up everything", "do the whole thing"]
      nouns_any: [onboarding, client, everything]
      phrases_any:
        - "do the full onboarding"
        - "onboard everything"
        - "complete the onboarding journey"
    routes:
      kind: macro_selector
      select:
        by_jurisdiction:
          LU: struct.lux.ucits.sicav
          IE: struct.ie.ucits.icav
          # Add more jurisdictions explicitly as needed
      then:
        macros: [mandate.create, mandate.add-product, case.open, case.add-party, case.solicit-document]
    explain:
      display_macro_steps: true
      show_unlock_graph: true

Key schema design points:
	•	requires makes match logic declarative and deterministic. The
resolver evaluates any_of / all_of gates before scoring signals.
	•	routes.kind is an explicit discriminated union: macro, macro_sequence,
or macro_selector (v0.3).
	•	macro_selector removes ambiguity: journeys can choose a jurisdiction-specific
structure macro deterministically without “magic parameterisation.”
	•	explain tells the UI what to display, making audit and disambiguation consistent.
	•	modes enforces governance — scenario only matches if session mode is compatible.

4.6 Scenario-Guided Runbook Building

When a scenario matches, the system doesn’t just dump all entries into
the runbook at once. It enters scenario mode — a guided,
conversational workflow:

Analyst: "Onboard a Luxembourg SICAV for Allianz"

Agent:   I'll walk you through setting up a Lux UCITS SICAV.
         [Runbook: struct.lux.ucits.sicav — 13 steps]
         
         Step 1: Create the umbrella fund.
         What's the fund name and SICAV registration number?

Analyst: "Allianz Europa Fund, registered SICAV number R1234"

Agent:   Got it. Step 1 ready.
         Step 2: Apply the Luxembourg UCITS document bundle.
         [auto-filled from jurisdiction — no question needed]
         
         Steps 3-7: Role assignments.
         I need to know the Management Company, Custodian,
         Administrator, Transfer Agent, and Auditor.
         Who's the ManCo?

Analyst: "ManCo is FundRock, custodian is BNY"

Agent:   [fills FundRock as ManCo, BNY as custodian]
         Who's the administrator?

...continues through all 13 steps...

Agent:   Runbook complete — 13 steps, all confirmed.
         Ready to execute? [run / show plan / edit step N]

This is conversational arg collection within the runbook framework.
Each macro expansion step becomes a RunbookEntry. Steps with unfilled
args become PendingQuestions. The Runbook’s existing derive_pending_ questions() method drives the conversation.

4.7 Scenario Disambiguation

When the utterance is ambiguous between scenarios:

Analyst: "Set up a fund in Ireland"

Agent:   Ireland supports several fund structures:
         1. UCITS ICAV  [struct.ie.ucits.icav — 15 steps]
         2. AIF ICAV    [struct.ie.aif.icav — 15 steps]  
         3. Hedge ICAV  [struct.ie.hedge.icav — 6 steps]
         
         Which structure type?

This is a DecisionPacket at the scenario level — the same pattern as
verb disambiguation, but selecting among macros rather than verbs.

4.8 Scenario Composition from Unlock Chains

The unlocks DAG on macros already defines valid workflow sequences.
A scenario can be defined as a path through the unlock graph:

Full Onboarding Journey (scenario):
  struct.lux.ucits.sicav          (produces structure + roles)
  → unlocks: structure.assign-role, mandate.create, case.open
  
  mandate.create                  (create trading mandate)
  → unlocks: mandate.add-product, mandate.set-instruments
  
  mandate.add-product             (add custody products)
  mandate.set-instruments         (set instrument universe)
  mandate.set-markets             (set market access)
  
  case.open                       (open KYC case)
  → unlocks: case.add-party, case.solicit-document, case.submit
  
  case.add-party                  (add UBOs to case)
  case.solicit-document           (request KYC documents)
  case.submit                     (submit for review)

This is 10 macros in sequence, following the unlock graph. The
struct.lux.ucits.sicav macro itself expands to 13 verbs, the subsequent
macros each expand to 1 verb. Total: ~22 runbook entries covering the
entire onboarding journey.

Key insight: The unlock DAG means scenarios don’t need to hard-code
the full sequence. A scenario can specify a starting macro and an end
condition. The agent then walks the unlock graph, offering unlocked
macros at each step. The scenario provides the intent frame (we’re
doing “full onboarding”), the unlock graph provides the available
next steps, and the analyst confirms or skips at each point.

4.9 Macro Sequence Validation via Existing Prerequisites (v0.2) + Deferred (v0.3)

When a scenario uses routes.kind: macro_sequence, the resolver must
validate that the sequence is executable before committing to it. This
uses the existing prerequisite and state-tracking machinery — no new
validation rules needed.

Each macro already declares:
	•	prereqs — what must be true before it can run
	•	sets-state — what becomes true after it runs

Validation is compile-time style checking of the planned runbook:

1. Start with current world state (session flags, DAG state)
2. For macro₁ in sequence:
   a. Check macro₁.prereqs against current state → pass/fail/deferred
   b. Apply macro₁.sets-state to state (simulated)
3. For macro₂ in sequence:
   a. Check macro₂.prereqs against (current state ∪ macro₁ effects) → pass/fail/deferred
   b. Apply macro₂.sets-state
4. Continue until sequence complete or prereq fails

v0.3 three-valued prereq result:
	•	Pass — prereq satisfied by simulated state
	•	Fail — prereq definitely not satisfiable by earlier steps
	•	Deferred — prereq depends on unresolved args (e.g., an ID not yet collected)

Deferred prereqs do not invalidate the sequence. They mark the
sequence as conditionally valid, and later macro expansions are delayed
until required args exist (collected via PendingQuestions).

If prereqs don’t line up, return a deterministic error:

ScenarioSequenceInvalid {
    failing_macro: String,       // e.g., "case.add-party"
    missing_prereq: String,      // e.g., "state_exists: case.exists"
    satisfied_by: Vec<String>,   // e.g., ["case.open", "case.select"]
}

If prereqs are deferred on unresolved args:

ScenarioSequenceConditionallyValid {
    deferred_macro: String,      // e.g., "case.add-party"
    requires_args: Vec<String>,  // e.g., ["case_id"]
}

This preserves determinism while avoiding false “invalid sequence” errors.

4.10 Explain Payload (ScenarioIndex + MacroIndex)

Every Tier -2 resolution produces an explain payload for audit logging
and UI disambiguation. This is the deterministic answer to “why did the
system choose this?”

Scenario explain payload (Tier -2A):

pub struct ScenarioResolution {
    pub scenario_id: String,
    pub title: String,
    pub route: ScenarioRoute,
    pub score_total: i32,
    pub matched_signals: Vec<MatchedSignal>,
    pub gates_passed: Vec<GateResult>,
    pub resolution_tier: ResolutionTier,  // Tier2A_Journey | Tier2B_MacroIndex
}

pub enum ScenarioRoute {
    Macro { fqn: String },
    MacroSequence { fqns: Vec<String> },
    MacroSelector { selected: String, then_fqns: Vec<String> },
}

pub struct MatchedSignal {
    pub bucket: String,       // "compound_outcome_verb", "jurisdiction", etc.
    pub matched_text: String, // "onboard", "Luxembourg", etc.
    pub score: i32,           // +4, +3, −6, etc.
}

pub struct GateResult {
    pub gate: String,         // "compound_signal", "mode_compatibility", "min_score"
    pub passed: bool,
    pub detail: String,       // "compound_action: onboard", "mode: onboarding ∈ [onboarding, kyc]"
}

MacroIndex explain payload (Tier -2B): MacroIndex returns the same
shape (score + matched_signals + gates), even if the buckets differ
(e.g., exact label match, alias phrase match, jurisdiction match, etc.).
This prevents macro parity from becoming a black box.

DecisionPackets: when 2–3 candidates are returned (scenario or macro),
the DecisionPacket must contain each candidate’s explain payload so
the UI can present deterministic “why these options” evidence.

This makes:
	•	UI disambiguation trivial — show matched signals and scores
	•	Audit logging complete — every resolution is reproducible
	•	Tuning data-driven — export resolution traces, adjust weights

4.11 Fallback: Single Verb Still Works

If the utterance doesn’t match any scenario or macro, the pipeline
falls through to ECIR noun→verb resolution and then to the existing
verb search pipeline. Scenarios are additive. They don’t replace verb-
level dispatch — they intercept composite intents before they get
misdirected to a single verb.

"Set up a Lux SICAV"         → MacroIndex match → struct.lux.ucits.sicav
"Do the full onboarding"     → Scenario match → macro_selector/sequence
"Create an umbrella fund"    → Single-Verb Blocker → ECIR → fund.create-umbrella
"What documents are missing" → Verb pipeline → document.list-missing


⸻

5. Design Constraints

5.1 Scenarios Are Not a New Runtime Construct

A scenario must dissolve into existing constructs by the time execution
begins. No new entry type in the Runbook. No new execution model. No
new database tables. The scenario selects a macro (or macro sequence),
the macro(s) expand to RunbookEntries, and from that point forward the
existing Runbook machinery handles everything.

Scenario identity may be stored only as provenance metadata on
RunbookEntries for narration and audit; it does not affect execution.

5.2 Provenance-Only Session State (v0.2 Resolution)

v0.1 proposed two options for scenario mode session state. v0.2
resolves this: Runbook is the state, with provenance metadata for
narration.

When a scenario produces a Runbook, each entry gets an origin tag:

// On RunbookEntry (in existing labels HashMap, no new field needed):
entry.labels.insert("origin_kind".into(), "macro".into());
entry.labels.insert("origin_macro_fqn".into(), macro_fqn.into());
entry.labels.insert("origin_scenario_id".into(), scenario_id.into());

No new struct. No new runtime model. Just provenance metadata using the
existing labels: HashMap<String, String> field that RunbookEntry
already has.

Narration then becomes deterministic:
	•	Count entries with same origin_scenario_id → show completed/total
	•	Look up scenario title from ScenarioIndex → “Step 4 of 13: Lux UCITS
SICAV Setup”

The agent’s orchestrator checks for pending questions before attempting
new utterance→verb resolution. The Runbook’s existing
derive_pending_questions() method drives the conversation. No new
session state tracking needed.

5.3 Scenarios Must Respect Governance

Macro availability is already gated by routing.mode-tags. Scenarios
must respect this: if the analyst is in trading mode and the scenario
references macros only available in onboarding mode, the scenario
should not match. The scenario YAML declares compatible modes
(modes: [onboarding, kyc]), and the resolver checks against the
current session’s mode as Gate 2.

5.4 Macros Remain the Pre-Processor

Macros are the canonical mapping from operator vocabulary to DSL verbs.
Scenarios route to macros. They never bypass macros to produce DSL verbs
directly. This preserves the macro system as the single point of
operator→DSL translation.

ALLOWED:   scenario → macro → DSL verb
ALLOWED:   scenario → [macro₁, macro₂, ...] → DSL verbs
FORBIDDEN: scenario → DSL verb  (bypasses macro layer)

If a scenario needs a verb sequence that no existing macro provides,
the correct response is to create the macro, not to let scenarios
expand directly to verbs.

5.5 Scenarios Are Configuration, Not Code

Like NounIndex, like verb YAMLs, like macro definitions — scenarios
are YAML data. No new Rust types for scenario-specific execution.
The scenario index is loaded at startup, matched at intent time, and
then the existing macro expansion and Runbook machinery takes over.

5.6 Scenario Matching Must Be Deterministic

Scenario matching should not depend on embedding similarity. It uses
the same approach as ECIR: noun extraction, action classification,
plus additional signals (jurisdiction, structure type, phase language).
The matching is rule-based and auditable. If we need to explain “why
did the system choose the full onboarding journey,” the answer should
be “because the utterance matched scenario full-onboarding-journey with
score N and selected jurisdiction LU,” not “because the embedding cosine
was 0.83.”

The scoring ledger (§4.2) and explain payload (§4.10) make this
concrete. Every resolution produces a full trace that can be logged,
compared, and replayed.

5.7 Non-Goals (v0.3)

To prevent scope creep and preserve determinism, v0.3 explicitly does
not attempt:
	•	Probabilistic DAG-walking composition in v1 (dynamic unlock-walk may be a later feature flag).
	•	Embedding-based scenario selection (embeddings are not used for Tier -2 decisions).
	•	New database tables or new Runbook entry types.
	•	Scenario-to-verb expansion that bypasses macros.

⸻

6. Capability Matrix

Capability	Construct	Exists?	Notes
Atomic DB operation	DSL Verb	✓ 653 verbs	No change
Operator vocabulary	Macro (single-verb)	✓ 29 macros	Becomes searchable via MacroIndex
Composite structure setup	Macro (multi-verb)	✓ 18 macros	Up to 24 verbs
Execution plan	Runbook	✓	Full lifecycle
Workflow sequencing	Unlock DAG	✓	30 macros with unlocks
Arg collection	PendingQuestion	✓	Derived from Runbook
Audit trail	RunbookEvent	✓	Append-only log
Macro search parity	MacroIndex	✗ NEW	Includes exact match + deterministic parity tiers
Composite intent recognition	ScenarioIndex	✗ NEW	Thin YAML, journeys + selectors
Deterministic scenario scoring	Scoring Ledger	✗ NEW	Fixed signal table + hard gates
Deterministic macro scoring	MacroIndex Scoring	✗ NEW	Thresholds + disambiguation band
Scenario→macro routing	Scenario Resolver	✗ NEW	Deterministic match with explain
Guided runbook building	Scenario Mode	✗ NEW	Conversational flow via PendingQuestion
Macro sequence orchestration	Sequence Validator	✗ NEW	prereqs/sets-state validation (Pass/Fail/Deferred)
Resolution explainability	Explain Payload	✗ NEW	Required for Tier -2A and Tier -2B

The new capabilities are all in the intent layer. Nothing changes
in the macro, runbook, or DSL verb layers.

⸻

7. Where Scenarios Sit in the Search Pipeline (v0.3)

utterance
  ↓
FastCommand check (undo, run, show plan)           ← existing
  ↓
Tier -1 probe: ECIR strong-single-verb check       ← NEW (v0.3 binding)
  ├─ 1 verb candidate (conf ≥ X) AND no compound signals → SKIP Tier -2
  └─ else continue
  ↓
Tier -2A: Journey ScenarioIndex                    ← NEW
  ├─ matched macro → expand → runbook
  ├─ matched macro_sequence/macro_selector → validate → expand → runbook
  ├─ ambiguous (2-3 scenarios) → DecisionPacket (with explain payloads)
  └─ no match → fall through
  ↓
Tier -2B: MacroIndex (macro search parity)         ← NEW
  ├─ includes exact match fast-path (former Tier 0)
  ├─ includes deterministic parity tiers (lexicon/phrases/phonetic/noun routing)
  ├─ matched macro → expand → runbook
  ├─ ambiguous (2-3 macros) → DecisionPacket (with explain payloads)
  └─ no match → fall through
  ↓
Tier -1: ECIR Noun→Verb                             ← NEW (from ECIR paper)
  ├─ 1 candidate → deterministic
  ├─ 2-5 candidates → narrow embedding (allowed at Tier -1 only)
  └─ no match → fall through
  ↓
Verb pipeline (Tiers 1–8 for verbs only)           ← existing
  ↓
Execute / Runbook / UI loop                        ← existing

Scenarios at Tier -2A intercept composite intent before the pipeline
tries to resolve to individual verbs. MacroIndex at Tier -2B catches
single-macro matches that the exact match-only path would miss. If neither
matches, the utterance continues through ECIR and the existing verb search
pipeline unchanged.

The ECIR strong-single-verb probe is the false-positive governor: if the
utterance is clearly a single operation, Tier -2 is skipped entirely and the
utterance goes straight to ECIR.

⸻

8. Interaction with ECIR

ECIR (Entity-Centric Intent Resolution), MacroIndex, and ScenarioIndex
are complementary but distinct:

Dimension	ECIR	MacroIndex	ScenarioIndex
Resolves to	DSL verb	Single macro	Macro, macro sequence, or macro selector
Signal	Noun + action	Label/alias + noun + jurisdiction + mode	Multi-signal + hard gates + scoring ledger
Cardinality	1 verb	1 macro (1-24 verbs)	1+ macros (1-22+ entries)
Confidence	High	High	Very high
Fallthrough	To verb pipeline	To ECIR	To MacroIndex
Data source	NounIndex YAML	Derived from macro metadata (+ overrides)	ScenarioIndex YAML
Explain payload	✓	✓ (v0.3)	✓

They share the NounIndex infrastructure. Scenario matching uses noun
extraction and action classification (from ECIR) as building blocks,
then adds jurisdiction, structure type, compound signal detection, and
hard gates.

// Resolution flow (v0.3)
fn resolve_intent(
    utterance: &str,
    noun_index: &NounIndex,
    macro_index: &MacroIndex,
    scenario_index: &ScenarioIndex,
    session_mode: &str,
) -> IntentResolution {
    // Extract features ONCE (shared across all tiers)
    let nouns = noun_index.extract(utterance);
    let action = NounIndex::classify_action(utterance);
    let jurisdiction = extract_jurisdiction(utterance);
    let compound_signals = detect_compound_signals(utterance);

    // v0.3: Single-Verb Blocker bound to ECIR (strong single-verb probe)
    if let Some(ecir_probe) = noun_index.ecir_probe_strong_single_verb(&nouns, action, utterance) {
        if ecir_probe.confidence >= X
           && !compound_signals.has_quantifier
           && !compound_signals.has_jurisdiction_structure_pair
           && !compound_signals.has_multi_noun_workflow {
            return IntentResolution::Ecir(ecir_probe.into_resolution());
        }
    }

    // Tier -2A: Journey ScenarioIndex
    if let Some(scenario) = scenario_index.resolve(
        &nouns, action, jurisdiction, &compound_signals, session_mode
    ) {
        return IntentResolution::Scenario(scenario);
    }

    // Tier -2B: MacroIndex
    if let Some(macro_match) = macro_index.resolve(
        &nouns, action, jurisdiction, session_mode
    ) {
        return IntentResolution::Macro(macro_match);
    }

    // Tier -1: ECIR noun→verb
    resolve_ecir(utterance, &nouns, action, noun_index)
}

The key gate: Scenario matching only activates when compound signals
are present AND the ECIR strong-single-verb probe does not fire. Without
“onboard / set up / run all / do the screening” type language, the utterance
typically resolves at ECIR or MacroIndex, preserving precision.

⸻

9. Open Questions

9.1 Macro Gaps

The scenario index will expose macros that should exist but don’t.
For example, there’s no screening.full macro that wraps
screening.sanctions + screening.pep + screening.adverse-media
into a single multi-verb expansion. Currently these are bare DSL verbs.

Resolution approach: Before building the scenario index, audit the
verb surface for common multi-verb patterns that lack a wrapping macro.
Create the missing macros first, then build scenarios that route to
them. This keeps the constraint that scenarios always route to macros,
never directly to verbs.

9.2 Scenario vs Template

The existing template system (template.invoke, template.batch)
already supports parameterised multi-verb sequences stored in the
database. How do scenarios relate?
	•	Templates are stored, parameterised, named execution plans. They
exist at runtime and can be versioned. They’re the “save and replay”
mechanism.
	•	Scenarios are intent-matching patterns that route to macros. They
exist at intent-resolution time and dissolve before execution.

A scenario can produce a Runbook that is identical to what a template
would produce. The difference is the entry point: templates are selected
by name from a menu, scenarios are selected by intent from natural
language. They converge at the Runbook level.

Future convergence: A commonly-used scenario could be “promoted”
to a template for direct access. A template could be registered as a
scenario for natural language access. The two concepts are complementary
views of the same workflow.

9.3 Dynamic Composition vs Static Definition

Should scenarios be statically defined in YAML (like macros), or should
the agent compose them dynamically by walking the unlock DAG?

Recommendation: Start static. The jurisdiction-specific structure
macros already encode the right verb sequences. The scenario index
routes to them. Dynamic composition (walking the unlock DAG at runtime
to build a macro sequence) is architecturally sound but introduces
non-determinism — the agent would need to decide which unlocked macros
to include, which is exactly the kind of probabilistic decision the
architecture is designed to avoid.

Static scenarios, curated by domain experts, are auditable and
predictable. Dynamic DAG-walking can be a future enhancement behind a
feature flag, once the static scenarios prove the conversational model
works.

9.4 Iteration Within Scenarios

Many real workflows involve iteration: “add three sub-funds,” “assign
all roles,” “request documents for each UBO.” The current macro model
doesn’t have native iteration — struct.lux.ucits.sicav expands to a
fixed set of 13 entries with fixed role slots.

Options:
	•	Repeat-macros in scenario definition. The scenario YAML can mark
a step as repeat: true, meaning the agent should ask “add another?”
after each execution and re-expand the macro with new args.
	•	Macro-level iteration. Add a repeat field to MacroExpansion
allowing a single macro to expand to N entries based on a count arg.
	•	Post-expansion editing. Expand the macro to the default entries,
then let the analyst add/remove/reorder in the Runbook. The Runbook
already supports this (add_entry, remove_entry, reorder).

The third option is simplest and already works. The analyst says “I need
a third sub-fund” and the agent adds another fund.create-subfund
entry to the Runbook. No new machinery needed.

9.5 MacroIndex Maintenance

The MacroIndex is primarily derived from existing macro metadata at
startup. The question is how much hand-curation is needed on top.

Derivable automatically from each macro:
	•	ui.label → primary search phrase
	•	ui.description → secondary search phrase
	•	target.operates-on → entity type filter
	•	routing.mode-tags → mode compatibility
	•	FQN segment extraction → jurisdiction (if struct.lux.* → LU),
structure type (if *.sicav → sicav), domain

Needs curation:
	•	Natural language aliases (“spin up a Lux fund” → struct.lux.ucits.sicav)
	•	Disambiguation hints (if multiple macros match same signals)
	•	Invocation phrases beyond what ui.label and description provide

Recommend: derive automatically at startup, allow optional
macro_search_overrides.yaml for curated additions. Keep the override
file small — if it grows past ~20 entries, that’s a signal that macro
ui.label and description fields should be improved instead.

⸻

10. Scope and Phasing

Phase 0.5: Macro Search Parity ✅ COMPLETE

The highest-ROI change. Do this first.

Make macros eligible for the same deterministic search tiers that DSL
verbs already use:
	•	Lexicon synonym match (deterministic) — macro labels and descriptions
become lexicon entries
	•	Learned phrases (deterministic) — agent.teach can target macros,
not just verbs
	•	Phonetic matching (deterministic) — macro FQNs and labels get
dmetaphone entries
	•	NounIndex routing (deterministic) — macros with target.operates-on
participate in noun→macro lookup

This requires changes to HybridVerbSearcher::search() to include
macros as candidates alongside verbs in deterministic tiers, with
MacroIndex scoring (§4.4) and explain payloads (§4.10).

Build the MacroIndex at this phase: load macro metadata, derive
search terms, build HashMap for O(1) exact match and fast phrase lookup.
Thread into the orchestrator pipeline as Tier -2B.

If this alone captures the majority of “set up Lux SICAV” style
utterances, the ScenarioIndex remains thin.

GATE: Macros appear as search results for utterances that reference
their labels, descriptions, or derived metadata. cargo test --lib
passes. No regression in verb-level hit rate.

Implementation: rust/src/mcp/macro_index.rs (MacroIndex struct with
deterministic scoring, O(1) FQN/label/alias lookup, jurisdiction/noun/mode
indexes, scoring ledger per §4.4). rust/config/macro_search_overrides.yaml
(curated aliases). Wired into HybridVerbSearcher as Tier -2B via
VerbSearcherFactory. Score 0.96. All 1565 tests pass.

Phase 1: ScenarioIndex + Scoring Ledger ✅ COMPLETE
	•	Create rust/config/scenario_index.yaml with ~5-10 journey scenarios
	•	Build ScenarioIndex with load, validate, resolve methods
	•	Implement deterministic scenario scoring ledger (signal table + hard gates)
	•	Implement ECIR-bound single-verb blocker (strong single-verb probe)
	•	Integrate as Tier -2A before MacroIndex
	•	Scenario disambiguation (DecisionPacket for ambiguous matches)
	•	Implement explain payload on all Tier -2 resolutions (scenario + macro)

Dependency: Phase 0.5 (MacroIndex) + ECIR Phase 1 (NounIndex)

Implementation: rust/src/mcp/scenario_index.rs (ScenarioIndex with
deterministic scoring ledger, hard gates G1-G3, route resolution for
macro/macro_sequence/macro_selector routes, explain payloads).
rust/src/mcp/compound_intent.rs (CompoundSignals extraction — compound
actions, jurisdictions, structure/phase nouns, quantifiers). ECIR short-
circuit modified to check compound signals before resolving at Tier -1.
rust/src/mcp/sequence_validator.rs (three-valued prereq validation:
Pass/Fail/Deferred). rust/config/scenario_index.yaml (10 journey scenarios
covering LU/IE/UK/US jurisdictions + cross-border + screening).
Wired as Tier -2A in HybridVerbSearcher. Score 0.97. 20 scenario_index
tests + 23 compound_intent tests + all 1565 existing tests pass.

Phase 2: Guided Runbook Building (2-3 days)
	•	When scenario matches a multi-verb macro, expand into Runbook
	•	Add provenance metadata to RunbookEntries (origin_kind,
origin_macro_fqn, origin_scenario_id in labels)
	•	Orchestrator checks pending questions before attempting new verb
resolution
	•	Conversational arg collection using existing PendingQuestion mechanism
	•	Progress narration using existing narrate_progress() enhanced with
scenario title from provenance

Dependency: Existing Runbook + PendingQuestion infrastructure.

Phase 3: Macro Sequence Orchestration (2-3 days)
	•	Scenario routes.kind: macro_sequence and macro_selector
	•	Each macro in sequence expands to Runbook entries in order
	•	Sequence validation using existing prereqs/sets-state
(§4.9) — compile-time style checking (Pass/Fail/Deferred)
	•	Deterministic error on prereq failure with suggested alternatives
	•	Conditional validity for deferred prereqs (requires args)
	•	Gap detection — identify missing macros needed by scenarios

Dependency: Phase 1 + Phase 2.

Phase 4: Macro Gap Filling (2-3 days)
	•	Create missing macros identified in Phase 3 (e.g., screening.full,
kyc-review.complete)
	•	Add scenario entries for newly created macros
	•	Tune scenario trigger phrases based on hit rate testing

Phase 5: Measurement + Tuning (1-2 days)
	•	Extend intent hit rate harness with scenario and macro test cases
	•	Measure: scenario match rate, correct selection, false positive rate,
runbook completion rate
	•	Tune signal scores and thresholds based on failures
	•	Export explain payloads for resolution traces

Total: 12-16 days, after ECIR.

10.1 Test Harness (Non-Negotiable)

Add a YAML/TOML test corpus for Tier -2 and ECIR interactions, integrated with the
existing harness:

# Scenario test cases — expect Tier -2A resolution

[[test]]
utterance = "Onboard this Luxembourg SICAV with three sub-funds"
expected_verb = ""  # not a verb-level resolution
category = "scenario"
difficulty = "medium"
expected_tier = "scenario"
expected_scenario_id = "lux-sicav-setup"
expected_route_kind = "macro"
expected_route_target = "struct.lux.ucits.sicav"

[[test]]
utterance = "Set up an Irish ICAV"
expected_verb = ""
category = "scenario"
difficulty = "easy"
expected_tier = "scenario"
expected_scenario_id = "ie-icav-setup"
expected_route_kind = "macro"
expected_route_target = "struct.ie.ucits.icav"

# Single-verb blocker (ECIR strong single-verb) — expect Tier -1 resolution (NOT Tier -2)

[[test]]
utterance = "Create an umbrella fund"
expected_verb = "fund.create-umbrella"
category = "direct"
difficulty = "easy"
expected_tier = "ecir"

[[test]]
utterance = "Assign custodian"
expected_verb = "cbu-role.assign"
category = "direct"
difficulty = "easy"
expected_tier = "ecir"

# MacroIndex test cases — expect Tier -2B resolution

[[test]]
utterance = "Set up structure for Lux SICAV"
expected_verb = ""
category = "macro_match"
difficulty = "medium"
expected_tier = "macro_index"
expected_route_target = "struct.lux.ucits.sicav"

The harness reports:
	•	Scenario match rate (compound utterances correctly routed)
	•	Macro match rate (single-macro utterances correctly routed)
	•	False positive rate (single-verb utterances incorrectly intercepted)
	•	Resolution tier distribution (how many utterances resolve at each tier)
	•	Explain payload validation (matched signals match expectations)
	•	Deferred validation coverage (macro sequences marked conditionally valid when args missing)

⸻

11. Success Criteria

Metric	Target
Compound intent correctly routed to scenario	≥80%
Scenario selects correct macro/sequence	≥90%
MacroIndex matches correct macro (non-scenario)	≥75%
Single-verb intent NOT intercepted by Tier -2 (false positive)	≥95% (<5% FP rate)
Runbook completion rate (all entries executed)	≥70%
Analyst turns to complete a scenario	≤2× the number of required args
No regression in single-verb hit rate	≥ pre-scenario baseline
Explain payload present on all Tier -2 resolutions and DecisionPackets	100%


⸻

12. Relationship to ECIR

ECIR and scenarios should be built in sequence:
	1.	ECIR first. It establishes the NounIndex, action classifier, and
Tier -1 integration pattern. Scenarios reuse all of this.
	2.	Macro search parity second (Phase 0.5). Give macros deterministic
search parity and explainability via MacroIndex (Tier -2B). This is
the highest-ROI change and reduces ScenarioIndex size.
	3.	Scenarios third. Add Tier -2A (journeys) above MacroIndex, plus
scoring ledger and hard gates. Use macro_selector for deterministic
jurisdiction-specific journey selection.

The three systems together provide four-level intent resolution:

Tier -2A: Journey Scenarios (composite intent → macro selector/sequence → runbook)
Tier -2B: MacroIndex (macro parity → single macro → runbook)
Tier -1:  ECIR (noun + action → single verb, deterministic)
Tier  1+: Verb pipeline (lexicon/learned/phonetic/embedding for verbs)

This gives the pipeline a graduated response: complex journey outcomes
at Tier -2A, single-macro outcomes at Tier -2B, specific operations at
Tier -1, and fuzzy verb matching at Tier 1+. Each tier handles the
utterances it’s best suited for, and the fallthrough guarantees nothing
is lost.

⸻

13. Implementation Hooks

Concrete integration points for Claude Code implementation:

13.1 New Types (Small)

// rust/src/mcp/macro_index.rs (NEW)
pub struct MacroIndex { ... }            // Derived from macro metadata at startup
pub struct MacroMatch { ... }            // Single macro resolution result
pub struct MacroExplain { ... }          // Explain payload for MacroIndex resolution

// rust/src/mcp/scenario_index.rs (NEW)
pub struct ScenarioIndex { ... }         // Loaded from scenario_index.yaml
pub struct ScenarioResolver { ... }      // Scoring ledger + gate evaluation
pub struct ScenarioResolution { ... }    // Result with explain payload
pub struct MatchedSignal { ... }         // Individual signal match
pub struct GateResult { ... }            // Gate pass/fail with detail

// rust/src/mcp/compound_intent.rs (NEW)
pub fn extract_jurisdiction(utterance: &str) -> Option<String>
pub fn detect_compound_signals(utterance: &str) -> CompoundSignals

// rust/src/mcp/sequence_validator.rs (NEW)
pub enum PrereqCheck { Pass, Fail { missing: String }, Deferred { requires_args: Vec<String> } }
pub fn validate_macro_sequence(...) -> SequenceValidationResult

13.2 Orchestrator Integration (Surgical)

In the same place that FastCommand parsing runs (orchestrator entry
point):
	1.	Extract features once (nouns, action, jurisdiction, compound_flags)
	2.	Run ECIR strong-single-verb probe — if it fires, resolve at ECIR (skip Tier -2)
	3.	Call ScenarioResolver (Tier -2A)
	4.	If no scenario, call MacroIndex (Tier -2B)
	5.	If matched, build runbook via macro expansion (existing machinery)
	6.	Else fall through to ECIR / verb pipeline

13.3 RunbookEntry Provenance (Minimal)

When macro expansion creates RunbookEntries, tag them:

entry.labels.insert("origin_kind".into(), "macro".into());
entry.labels.insert("origin_macro_fqn".into(), macro_fqn.into());
// Only if triggered by a scenario:
entry.labels.insert("origin_scenario_id".into(), scenario_id.into());

No new fields, no new structs. Uses existing labels HashMap.

⸻

14. Conclusion

The research question — “are scenarios composite DSL verbs?” — resolves
to no. Scenarios are not a new construct at any layer below intent.
They are an intent-matching pattern that routes to existing macros.
Multi-verb macros already produce runbooks. Single-verb macros already
wrap DSL verbs. The Runbook already handles execution, audit, and
human interaction.

The missing piece is two-fold:
	1.	Macro search parity (Tier -2B) — make macros searchable through
deterministic tiers, including exact match fast-path, and require
explain payloads for audit and UI disambiguation. This alone captures
the majority of single-macro intent misses. (Phase 0.5, highest ROI.)
	2.	A thin ScenarioIndex (Tier -2A) — for multi-macro journeys and
cross-domain composite intents that no single macro can absorb.
Deterministic scoring ledger with hard gates, ECIR-bound single-verb
blocker for false-positive control, sequence validation using existing
prereqs/sets-state with Pass/Fail/Deferred, and explain payloads for audit.

Together these give the pipeline a four-tier graduated response: journey
scenarios → macro matches → noun-based verb selection → verb search.
Each tier handles the utterances it’s best suited for, and the
fallthrough guarantees nothing is lost.

The architectural principle holds: the LLM translates intent, the DSL
executes deterministically, SemOS governs. Scenarios enhance the
translation, not the execution.