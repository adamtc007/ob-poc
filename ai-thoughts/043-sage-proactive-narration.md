# ADR 043: Sage Proactive Narration — Goal-Directed Workflow Guidance

> **Status:** PROPOSED (2026-03-29)
> **Scope:** Transition Sage from reactive Q&A to proactive goal-directed narration
> **Dependencies:** Constellation hydration, OnboardingStateView, SessionVerbSurface, StepNarration
> **Risk:** Operator annoyance if narration is too aggressive — needs a volume dial

---

## Vision

Today Sage is reactive. The operator says "assign the depositary", Sage executes it, returns the result. The operator must know what to do next. If they don't ask, Sage is silent.

The vision: **Sage sees the finish line and narrates the path to it.** After every action, Sage looks at the constellation — which slots are filled, which are empty, which have dependencies — and tells the operator where they are, what's next, and what's blocking. The operator doesn't ask "what's missing"; Sage volunteers it.

This is the difference between a tool that executes commands and an agent that drives a workflow.

### What it looks like

**Today (reactive):**
```
Operator: assign BNP Paribas as depositary
Sage:     Done. Depositary assigned to Lux UCITS SICAV Alpha.
Operator: what's next?
Sage:     You could assign a management company, open a KYC case, or...
```

**Target (proactive):**
```
Operator: assign BNP Paribas as depositary
Sage:     Depositary assigned to Lux UCITS SICAV Alpha.
          ── Progress: 3 of 7 roles filled ──
          Required:  Management Company (empty — needed for UCITS authorisation)
          Optional:  Administrator, Auditor, Investment Manager, Domiciliation Agent
          Next step: assign a Management Company, or open a KYC case to begin due diligence
```

The operator didn't ask for the progress report. Sage computed it from the constellation delta and narrated it because there's an unfilled required slot.

---

## Design Principles

### 1. The constellation IS the goal

Every workspace has a constellation map. Every constellation has slots. Every slot has a state (empty → placeholder → filled). The completion state of the constellation is the implicit goal.

Sage doesn't need a separate "goal engine" — the constellation already encodes what "done" looks like. The narration just reads the delta between current state and complete state.

### 2. Required vs optional drives urgency

Slots have cardinality: some are required for the structure type (depositary for UCITS, GP for PE), others are optional (auditor, legal counsel). Sage narrates required gaps with urgency, optional gaps as suggestions.

The macro YAML already encodes this via `prereqs` and `unlocks`:
- A required slot blocks downstream macros (case.open requires structure.exists)
- An optional slot doesn't block anything

### 3. Narration has a volume dial

Not every response needs a progress report. The narration verbosity should adapt:

| Context | Narration Level |
|---------|----------------|
| First action in a new workspace | Full: show the complete constellation with all slots |
| After filling a required slot | Medium: show progress fraction + next required slot |
| After filling an optional slot | Light: acknowledge + mention remaining optional count |
| After a read/query verb (no state change) | Silent: just return the data |
| Operator explicitly asks "what's next" | Full: detailed gap analysis |
| Operator is exploring (rapid queries) | Silent: don't interrupt the flow |

The signal for "exploring" vs "building" is `writes_since_push` — if the operator has been reading without writing, they're exploring. If they've been writing, they're building and narration helps.

### 4. Narration is not prescription

Sage suggests, it doesn't mandate. "Next step: assign a Management Company" is a suggestion based on the dependency graph. The operator can ignore it and do something else. Sage adapts to what they actually do, not what it suggested.

The constellation has no enforced ordering beyond prereqs. The operator can fill slots in any order. Sage just highlights the critical path.

---

## Architecture

### Data Flow

```
Operator utterance
    │
    ▼
ReplOrchestratorV2.process()
    │
    ├── Verb resolution (MacroIndex / embeddings)
    ├── DSL generation
    ├── Execution
    │
    ▼
Constellation re-hydration (writes_since_push > 0)
    │
    ▼
┌─────────────────────────────────────┐
│ NEW: NarrationEngine.compute()      │
│                                     │
│ Inputs:                             │
│   - pre_state: slot states before   │
│   - post_state: slot states after   │
│   - constellation_map: slot defs    │
│   - workspace: current workspace    │
│   - writes_since_push: int          │
│   - last_verb: what just executed   │
│                                     │
│ Outputs:                            │
│   - NarrationPayload                │
│     ├── progress: "3 of 7 roles"    │
│     ├── delta: what just changed    │
│     ├── required_gaps: Vec<Gap>     │
│     ├── optional_gaps: Vec<Gap>     │
│     ├── suggested_next: Vec<Verb>   │
│     ├── blockers: Vec<Blocker>      │
│     └── verbosity: Full|Medium|     │
│                     Light|Silent    │
└─────────────────────────────────────┘
    │
    ▼
ChatResponse (existing)
    ├── result: execution result
    ├── onboarding_state: OnboardingStateView
    └── narration: NarrationPayload      ◄── NEW FIELD
```

### Key Types

```rust
/// Proactive narration payload, computed after every state-changing action.
pub struct NarrationPayload {
    /// Human-readable progress summary
    /// e.g., "3 of 7 roles filled for Lux UCITS SICAV Alpha"
    pub progress: Option<String>,

    /// What changed in this turn
    /// e.g., "Depositary: empty → filled (BNP Paribas)"
    pub delta: Vec<SlotDelta>,

    /// Required slots still empty — these block downstream workflows
    pub required_gaps: Vec<NarrationGap>,

    /// Optional slots still empty — suggestions, not blockers
    pub optional_gaps: Vec<NarrationGap>,

    /// Suggested next actions, ordered by dependency priority
    pub suggested_next: Vec<SuggestedAction>,

    /// Active blockers (prereqs not met for available macros)
    pub blockers: Vec<NarrationBlocker>,

    /// Narration verbosity for this turn
    pub verbosity: NarrationVerbosity,
}

pub struct SlotDelta {
    pub slot_name: String,
    pub slot_label: String,
    pub from_state: SlotState,
    pub to_state: SlotState,
    pub entity_name: Option<String>,
}

pub struct NarrationGap {
    pub slot_name: String,
    pub slot_label: String,
    pub why_required: Option<String>,  // "needed for UCITS authorisation"
    pub suggested_verb: String,        // verb FQN to fill this slot
    pub suggested_macro: Option<String>, // macro FQN if one wraps this verb
    pub suggested_utterance: String,   // natural language suggestion
}

pub struct SuggestedAction {
    pub verb_fqn: String,
    pub macro_fqn: Option<String>,
    pub utterance: String,
    pub priority: ActionPriority,
    pub reason: String,
}

pub enum ActionPriority {
    /// Required slot unfilled — blocks downstream
    Critical,
    /// Next in dependency chain — natural progression
    Recommended,
    /// Optional but contextually relevant
    Optional,
}

pub enum NarrationVerbosity {
    /// Full constellation overview with all gaps
    Full,
    /// Progress fraction + next required action
    Medium,
    /// Acknowledge + remaining count
    Light,
    /// No narration (read-only action or exploring)
    Silent,
}
```

### Verbosity Decision Logic

```rust
fn compute_verbosity(
    last_verb: &str,
    writes_since_push: usize,
    required_gaps: &[NarrationGap],
    is_first_action_in_workspace: bool,
) -> NarrationVerbosity {
    // First action in workspace: show the full picture
    if is_first_action_in_workspace {
        return NarrationVerbosity::Full;
    }

    // Read-only verb (no state change): stay silent
    if !verb_is_write(last_verb) {
        return NarrationVerbosity::Silent;
    }

    // Just filled the last required slot: celebrate
    if required_gaps.is_empty() {
        return NarrationVerbosity::Full; // "All required roles filled!"
    }

    // Filled a required slot: show progress
    if slot_was_required(last_verb) {
        return NarrationVerbosity::Medium;
    }

    // Filled an optional slot: light touch
    NarrationVerbosity::Light
}
```

### Completion Model

The constellation map already defines slot cardinality and dependencies. But it doesn't
explicitly mark slots as "required" vs "optional" for a given structure type. This information
lives in the macro YAML (via `required_roles` and `optional_roles` on struct macros).

**Completion source hierarchy:**
1. **Macro `required_roles`** — if the active macro specifies required roles, use those
2. **Constellation slot `min_cardinality`** — if the slot definition has a minimum, it's required
3. **State machine transitions** — if a tollgate gate checks for slot completion, those slots are required
4. **Default** — slots with no cardinality constraint are optional

### Integration Points

| Component | Change | Risk |
|-----------|--------|------|
| `ChatResponse` | Add `narration: Option<NarrationPayload>` field | Low — additive |
| `response_adapter.rs` | Compute narration after constellation hydration | Low — new code path |
| `OnboardingStateView` | Already has forward_verbs — narration enriches, doesn't replace | Low |
| React `ChatMessage` | Render narration payload below execution result | Low — UI only |
| `ReplOrchestratorV2` | Capture pre-execution slot states for delta computation | Medium — touches hot path |

### What Narration Does NOT Do

- **Does not change verb resolution.** The intent pipeline is unchanged. Narration is post-execution.
- **Does not enforce ordering.** The operator can ignore suggestions. No gates, no blocks.
- **Does not call the LLM.** Narration is deterministic — computed from constellation delta, not generated text. Templates with slot substitution, not free-form prose.
- **Does not replace OnboardingStateView.** The existing state view is a structured data payload for the UI. Narration is a human-readable complement.

---

## Contextual Query Resolution

The proactive narration also solves the "what's missing" / "what's left" problem identified
in the intent pipeline analysis. Today these contextual queries fail because the verb searcher
has no constellation context.

With narration, two approaches:

### Approach A: NarrationEngine as query handler (PREFERRED)

When the utterance matches a contextual pattern ("what's left", "what's missing", "what's next",
"where are we", "show progress"), the orchestrator routes to NarrationEngine directly instead of
verb search. NarrationEngine returns a Full-verbosity narration payload without executing any verb.

This is clean: no embedding search needed, no verb collision, deterministic response from
constellation state. The patterns are a small fixed set (~20 phrases) detected before verb search.

```rust
const CONTEXTUAL_PATTERNS: &[&str] = &[
    "what's left", "what's missing", "what's remaining",
    "what's next", "what do I need to do",
    "where are we", "show progress", "how far along",
    "what's outstanding", "what's still needed",
    "are we done", "is everything complete",
    "what's blocking", "any blockers",
    "show gaps", "what gaps",
    "status", "progress report",
    "summary", "overview",
];
```

### Approach B: Verb surface bias for contextual queries

Thread the active slot's state verbs into `HybridVerbSearcher` as a boost set. When the
utterance is ambiguous ("what's missing"), the searcher prefers verbs that are valid in the
current constellation context.

This is riskier: it changes the search pipeline, adds a new bias dimension, and could cause
regressions on non-contextual queries. The contextual pattern set is small enough that
Approach A is cleaner.

**Recommendation:** Approach A. Detect contextual patterns early, route to NarrationEngine,
return constellation-derived response. No pipeline changes.

---

## Phases

### Phase 0: NarrationEngine core (deterministic, no LLM)

- `NarrationPayload` type
- `compute_narration()` from pre/post constellation states
- `compute_verbosity()` decision logic
- `NarrationGap` with suggested_utterance templates
- Unit tests with mock constellation states
- Wire into `response_adapter.rs` — populate `ChatResponse.narration`

### Phase 1: React rendering

- `NarrationPanel` component below execution result
- Progress bar (filled/total slots)
- Gap list with clickable suggested utterances
- Verbosity-aware rendering (collapse for Light, expand for Full)
- Silent = no panel rendered

### Phase 2: Contextual query routing

- Detect contextual patterns in `ReplOrchestratorV2` before verb search
- Route to `NarrationEngine.query()` for Full narration without execution
- Add the ~20 contextual patterns to intent test fixtures
- Verify no regression on existing hit rates

### Phase 3: Completion celebration + workspace transition

- Detect when all required slots are filled
- Narrate completion: "All required roles assigned. Ready to open KYC case."
- Suggest workspace transition when current workspace goal is met
- "Structure setup complete. Switch to KYC workspace to begin due diligence?"

---

## Guardrails

1. **No LLM in narration.** Templates only. Deterministic, fast, testable.
2. **No blocking.** Narration suggests, never prevents. The operator always has full control.
3. **No verbosity without state change.** Read-only actions = silent. Don't narrate queries.
4. **Regression gate.** Intent hit rate must not drop below 92% two-attempt after Phase 2.
5. **Operator override.** If the operator says "stop suggesting" or similar, suppress narration for the session. Respect the signal.

---

## Success Metrics

| Metric | Target |
|--------|--------|
| Narration computed per write action | 100% |
| Contextual queries resolved without verb search | >90% |
| Operator follows suggested_next verb | Track, no target (observe adoption) |
| Narration latency overhead | <5ms (constellation already hydrated) |
| "What's missing" / "what's next" hit rate | >95% (Approach A, pattern matching) |

---

## Non-Goals

- **Sage does not make decisions.** It narrates state and suggests actions. The operator decides.
- **Sage does not learn operator preferences.** Narration volume is rule-based (verbosity logic), not learned. Learning preferences is a separate concern.
- **Sage does not orchestrate multi-step workflows.** That's the runbook compiler's job. Narration shows where you are in the constellation, not what sequence to follow.
- **Sage does not replace the UI state panel.** OnboardingStateView and the constellation panel are structured data. Narration is the human-readable complement — "what to do next" vs "what the current state is."
