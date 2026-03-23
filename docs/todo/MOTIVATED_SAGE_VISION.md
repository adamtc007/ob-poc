# Motivated Sage: Goal-Driven Agent Architecture

## Vision & Scope Paper v1.0

**Author:** Adam  
**Date:** March 2026  
**Status:** Agreed — ready for implementation planning  
**Predecessor documents:** NLCI Architecture v1.2, SemOS Constellation Model v1.2, Sage/Coder Pipeline Architecture, Semantic Traceability Kernel  
**Review status:** Two peer review cycles completed. Architecture approach agreed.

---

## 1. Executive Summary

Sage today is a **reactive command resolver**. A user speaks, Sage resolves a verb, executes it, and returns to idle. The constellation model provides the map of what *could* happen, but nothing in the current architecture encodes what *should* happen next. The agent has no planning horizon, no progress awareness, and no drive toward completion.

This paper proposes **Motivated Sage** — an architectural extension that transforms Sage from a passive command resolver into a goal-driven planning agent by exploiting a structural property already present in the system: the constellation DAG, when hydrated with current entity state and annotated with target state, is a computable **feasible frontier** — the set of legal, actionable transitions available at any moment. A planning layer ranks that frontier and proposes the next action. The human approves. The loop continues until the goal is reached.

The core mechanism rests on a straightforward runtime loop: compute actionable work from live state, expose it to the LLM in a continuation-biased prompt frame, gate execution through human approval, re-hydrate after each state change, and repeat. This loop is informed by an empirically proven observation — that autoregressive models exhibit strong completion drive when their context contains visible progress metrics and continuation directives — but the architecture stands on its own as a state-machine-driven planning system regardless of the underlying model's behavioural properties.

This is not speculative. Every structural component required — constellation templates, entity state queries, verb-to-transition mappings, the Sage utterance pipeline — already exists in ob-poc. What is missing is the **runtime loop** that connects them: a goal frame, a gap analyser, a blocker detector, and a prompt template that injects the computed motivation surface into every agent turn.

---

## 2. Problem Statement

### 2.1 The Passive Agent Problem

The current Sage interaction model is synchronous and stateless at the intent level:

```
User utterance → intent resolution → verb execution → response → idle
```

Each utterance is resolved independently. Sage has no memory of what the user is trying to achieve beyond the current turn. If onboarding an Irish ICAV requires 18 macro-expanded verb executions across 7 entities, the user must drive every step. Sage will execute whatever it is told, but it will never say "the next thing you should do is X" unless explicitly asked.

This creates three concrete problems:

**Expertise burden.** The user must know the full constellation topology — which entities need to reach which states, in what order, with what dependencies — and manually drive each step. The constellation model was designed to encode exactly this knowledge, but it is currently consumed only by the compiler and the UI, not by the agent.

**Lost context.** Each utterance starts from zero. If screening results come back and unblock a KYC case transition, Sage doesn't know this was the thing the user was waiting for. The user must re-establish context ("remember we were onboarding that ICAV, screening just cleared, now run the next step").

**No parallel path awareness.** A constellation DAG often has independent branches that can advance in parallel. A passive agent executes the one thing it is told. A planning agent would recognise that while branch A is blocked waiting for external input, branches B and C have unblocked transitions available.

### 2.2 The Empirical Evidence

The E-invariant pattern, developed for Claude Code multi-phase task execution, demonstrates that autoregressive model behaviour is strongly influenced by progress framing in context. When the context contains:

- A visible gap between current state and target state
- A clear path (ordered phases, topological sort)
- An explicit continuation directive ("→ IMMEDIATELY proceed")
- A progress metric ("Progress: 35%")
- A completion invariant ("Progress must reach 100%")

...the model produces continuation tokens with high reliability, even across complex multi-phase plans. Without these signals, the same model routinely stops after the first phase and presents a summary.

This is not motivation in any cognitive sense. It is a structural property of next-token prediction: a context saturated with "you are mid-task, continuation is the default" produces different probability distributions than a context that reads "task complete, awaiting input." But the functional effect — sustained, goal-directed, multi-step execution — is exactly what Sage needs.

### 2.3 The Insight

The constellation DAG, hydrated with current entity state, defines a **feasible frontier** — the set of transitions that are legal and unblocked right now. A planning layer can rank that frontier (by dependency urgency, risk, operator preference, or policy heuristics) and propose the best next action. After execution, the frontier is recomputed from updated state. This is the core loop.

The E-invariant pattern from Claude Code provides empirical evidence that this loop will produce strong completion behaviour in LLM agents. The analogy is instructive:

| E-Invariant (Static Prompt) | Constellation (Dynamic State) |
|---|---|
| "Phase 2 of 6 complete" | "7 of 18 nodes at target state" |
| "→ IMMEDIATELY proceed to Phase 3" | "Next unblocked transition: screening.run on entity.primary" |
| "Progress must reach 100%" | "Goal: all nodes at target state" |
| Phase dependency ordering | DAG topological sort |
| Explicit blockers listed | Precondition failures on transitions |

But the architectural claim does not depend on this analogy. Even without the prompt-layer continuation effect, the system has value: it computes what work is available, proposes it, and maintains continuity across a multi-step process. The prompt framing amplifies the effect; the state machine provides the substance.

The constellation model was described in its own architecture paper as "the layer that transforms Sage from command resolver into planning agent." This paper specifies the mechanism by which that transformation occurs.

---

## 3. Vision

### 3.1 The Motivated Agent Loop

The target interaction model replaces the passive cycle with a goal-driven loop:

```
Goal declaration
  → constellation hydration (load template, query current state)
  → gap analysis (which nodes are not at target?)
  → frontier computation (which transitions are unblocked?)
  → frontier ranking (policy, dependency urgency, operator preference)
  → next-action proposal to user
  → [user approval gate]
  → verb execution
  → state update
  → re-hydrate constellation
  → re-compute frontier
  → next-action proposal
  → ...
  → all nodes at target → goal achieved
```

Key properties:

**Continuation is the default.** After every verb execution, the agent re-assesses and proposes the next action. The user does not re-initiate the planning loop. The default trajectory is forward motion until the goal is reached or an unresolvable blocker is encountered.

**Approval is the gate, not initiation.** The human's role shifts from "tell the agent what to do" to "approve or redirect what the agent proposes." Where an autonomous code agent might execute directly, Sage proposes and waits — appropriate for a compliance domain where actions have regulatory weight. But in both cases, the agent drives forward. The human steers.

**Blocking is explicit and actionable.** When no transition can advance, the agent reports exactly what external event it is waiting for, which entities are affected, and what will happen when the blocker clears. The session enters a **watching state**, not idle — the distinction matters for prompt framing.

### 3.2 Interaction Examples

**Example 1: Goal-driven onboarding**

```
User: "Onboard this Irish ICAV — the ManCo is Dublin-registered,
       two sub-funds, EUR and USD share classes."

Sage: [Loads constellation template M5 (Irish ICAV)]
      [Hydrates: all nodes at INITIAL]
      
      Progress: 0/22 nodes at target. Goal: full ICAV onboarding.
      
      First actions available (parallel):
      1. entity.create for ManCo (Dublin)
      2. entity.create for ICAV umbrella
      3. entity.create for sub-fund EUR
      4. entity.create for sub-fund USD
      
      Recommend: execute all four entity creates. Approve?

User: "Yes, go."

Sage: [Executes 4x entity.create]
      
      Progress: 4/22 (18%). 
      Next available:
      1. cbu.create for ICAV (depends: entity.ICAV = DRAFT ✓)
      2. kyc-case.open for ManCo (depends: entity.ManCo = DRAFT ✓)
      
      Blocked:
      - structure.link (needs CBU to exist)
      - trading-profile.set (needs CBU at VALIDATED)
      
      Recommend: create CBU and open KYC case in parallel. Approve?
```

**Example 2: Blocker handling**

```
Sage: Progress: 14/22 (64%).
      
      BLOCKED — no transitions available.
      
      Waiting for:
      - Screening results on entity.ManCo 
        (submitted via screening.run, pending external response)
      - Document upload: ICAV prospectus 
        (required by tollgate.doc-completeness)
      
      When screening clears → kyc-case can transition to VALIDATED
      When prospectus uploaded → tollgate can transition to PASSED
      
      Check back when either event has occurred and I'll re-assess 
      the frontier.
```

**Example 3: Mid-stream course correction**

```
Sage: Progress: 9/22 (41%).
      Next recommended: screening.run on sub-fund EUR entity.

User: "Hold on the sub-funds. The EUR class might be dropped.
       Focus on getting the ManCo and umbrella to ACTIVE first."

Sage: [Adjusts planning scope — marks sub-fund nodes as deferred]
      
      Revised scope: 9/14 active nodes (64%).
      Next available: kyc-case.validate on ManCo 
      (screening: PASSED, documents: COMPLETE)
      
      Approve?
```

### 3.3 What This Is Not

**Not autonomous execution.** Sage proposes; the human approves. Every verb execution in the compliance domain passes through a human gate. The "motivation" is in the planning and proposal loop, not in unsupervised action.

**Not a workflow engine.** BPMN handles durable long-running orchestration. Motivated Sage is the conversational layer *above* the workflow engine — it understands the constellation, interprets state changes (including those driven by background BPMN processes), and maintains conversational continuity about progress. If a BPMN process completes a screening step, Sage detects the state change and proposes the next action.

**Not a search algorithm.** The frontier computation is deterministic graph traversal with precondition filtering. However, **ranking** the frontier — choosing which of several legal transitions to propose first — may involve domain heuristics (dependency urgency, risk ordering, time-to-value, operator preference). The constellation computes what is possible; the ranking layer determines what is strategically preferred. **The key distinction: frontier membership is deterministic and verifiable; frontier ordering is heuristic and therefore reviewable.** In v1.0, the LLM itself performs ranking based on the prompt surface — this is a provisional ranking mechanism, not a truth source. The human approval gate exists precisely because ranking may be wrong even when the frontier is correct. Future versions may introduce explicit policy-driven ranking functions that encode learned operator preferences.

---

## 4. Capabilities Required

### 4.1 Goal Frame

**C-01: Goal declaration and session binding**

A session must be able to hold an active goal frame — a reference to a constellation template instantiated for a specific case. The goal frame persists across turns within the session and survives context window truncation (it must be re-injectable from session state, not dependent on conversation history).

```
GoalFrame {
    constellation_template_id: String,  // e.g., "m5_irish_icav"
    case_id: String,                    // specific onboarding case
    target_states: Map<NodeId, State>,  // where each entity should end up
    deferred_nodes: Set<NodeId>,        // user-excluded from current scope
    created_at: Timestamp,
    status: GoalStatus,                 // ACTIVE | PAUSED | ACHIEVED | ABANDONED
}
```

**C-02: Goal inference from natural language**

When a user declares intent at the goal level ("onboard this Irish ICAV"), Sage must resolve this to the correct constellation template. This is a higher-level intent resolution than verb resolution — it maps a business objective to a constellation, not a command to a verb. The existing NLCI pipeline's domain-hint extraction is the foundation, but the resolution target is different (constellation template vs DSL verb).

**Ambiguity resolution policy** (strict ordering):

1. **Explicit template reference.** The user names or references a specific constellation template or active case. Resolution is deterministic. ("Continue the Meridian ICAV onboarding" → lookup case by name.)
2. **Strong structural cues.** The utterance contains enough jurisdiction, vehicle type, and structure detail to uniquely identify a template. ("Irish ICAV, two sub-funds, Dublin ManCo" → M5.) Resolution proceeds.
3. **Guided clarification.** Multiple templates fit the utterance. Sage presents the candidates with distinguishing characteristics and asks the user to select. ("That could be an Irish ICAV [M5] or an Irish QIAIF [M6] — which structure?")
4. **Never silently over-commit.** If ambiguity remains after structural cues, Sage must not guess. A wrong template selection front-loads errors into every subsequent proposal. Clarification is always cheaper than correction.

This policy prevents goal-template resolution from becoming its own under-specified intent system. The resolution must be unambiguous before the planning loop begins.

**C-03: Goal modification mid-stream**

The user must be able to adjust scope without abandoning the goal. Deferring nodes, re-prioritising branches, adding entities not in the original template — all without losing progress state. The goal frame is mutable; the constellation hydration reflects mutations.

### 4.2 Gap Analysis Engine

**C-04: Constellation hydration**

Given a goal frame, load the constellation template DAG and populate each node with current entity state from the database. This is a join between the template's node definitions and live entity records.

```
HydratedNode {
    node_id: NodeId,
    entity_type: String,
    entity_id: Option<Uuid>,       // None if entity not yet created
    current_state: Option<State>,   // None if entity not yet created
    target_state: State,
    is_deferred: bool,
    gap: GapStatus,                 // AT_TARGET | REACHABLE | BLOCKED | DEFERRED
}
```

**C-05: Gap computation**

Walk the hydrated DAG. For each node not at target:
- Is there a verb that transitions from current state toward target state?
- Are that verb's preconditions met? (Depends on other nodes' states, external conditions, document availability, etc.)
- If preconditions are not met, what specific preconditions fail?

Output: an ordered list of **actionable transitions** (unblocked) and **blocked transitions** (with specific blocker descriptions).

**C-06: Progress metric computation**

Compute a progress percentage and a structured progress summary:

```
ProgressSurface {
    total_nodes: usize,
    active_nodes: usize,          // total minus deferred
    nodes_at_target: usize,
    progress_pct: f32,            // nodes_at_target / active_nodes
    actionable_count: usize,      // transitions available right now
    blocked_count: usize,         // transitions waiting on preconditions
    blockers: Vec<BlockerDetail>, // what each blocked node is waiting for
}
```

### 4.3 Blocker Detection & Watching

**C-07: Precondition analysis**

For each blocked transition, decompose the blocker into a typed category:

| Blocker Type | Description | Agent Behaviour |
|---|---|---|
| `ENTITY_STATE` | Depends on another entity reaching a state | Propose advancing that entity first |
| `EXTERNAL_EVENT` | Waiting for external input (screening results, document upload) | Report and enter watching state |
| `HUMAN_DECISION` | Requires explicit human judgment (tollgate review) | Prompt user to make the decision |
| `TEMPORAL` | Time-based constraint (cooling-off period, review schedule) | Report expected availability time |
| `DATA_MISSING` | Required data not yet provided (address, tax ID) | Prompt user to provide the data |

**C-08: Parallel path identification**

When the primary path is blocked, identify independent branches in the DAG that can advance. This is a standard reachability analysis on the DAG with blocked edges removed. The agent should propose parallel work rather than just reporting the block.

**C-09: Watch state and re-activation**

When all paths are blocked on external events, the session enters a watch state. The watch state is a prompt-framing distinction, not a durable subscription.

**v1.0 scope (session-scoped only):** The watch state persists only within the active session. If the user returns to the session and issues any utterance, Sage re-hydrates the constellation and recomputes the frontier — detecting any state changes that occurred while the session was inactive (e.g., screening results that arrived, documents uploaded). This is a pull model: re-assessment is triggered by user re-engagement, not by event subscription.

**v2.0+ scope (durable re-activation):** Event-driven re-activation — where entity state changes push notifications to watching goal frames — requires durable session/goal persistence, event subscription infrastructure, user notification semantics, and resumability guarantees. This is substantially more complex than session-scoped watching and is explicitly deferred. The v1.0 pull model is sufficient for proving the core planning loop.

### 4.4 Prompt Surface (Continuation-Biased Context Frame)

**C-10: Motivation prompt template**

This is the architectural keystone. Every Sage turn, when a goal frame is active, the LLM context must include a computed prompt block:

```markdown
## Active Goal: {constellation_name} for {case_id}
Progress: {nodes_at_target}/{active_nodes} ({progress_pct}%)
Goal state: All active nodes at target.

### Available Actions (unblocked)
1. {verb} on {entity} — advances {node} from {current} → {next_state}
2. {verb} on {entity} — advances {node} from {current} → {next_state}

### Blocked
- {node}: waiting for {blocker_type}: {blocker_detail}
- {node}: waiting for {blocker_type}: {blocker_detail}

### Deferred (user-excluded)
- {node}: deferred by user at {timestamp}

→ Propose the next action(s) to advance toward goal state.
  If multiple actions are available, recommend parallel execution where safe.
  If all paths are blocked, report blockers and enter watching state.
```

This prompt block replaces the "awaiting user input" framing with "you are mid-task, here is the frontier, propose the next step." The continuation-biased framing amplifies the model's tendency to propose forward actions rather than summarise or idle.

**C-11: Progress visibility in Constellation UI**

The Constellation UI panel (already integrated into the Sage chat session window) must reflect the goal frame state. The tree-first slot view and SVG ownership canvas should visually distinguish:
- Nodes at target (complete)
- Nodes with available transitions (actionable)
- Nodes blocked (waiting)
- Nodes deferred (user-excluded)

This gives the user the same progress visibility the LLM gets in its prompt — both human and agent are looking at the same map.

### 4.5 Approval & Control

**C-12: Approval modes**

Three modes, selectable per session:

| Mode | Behaviour | Use Case |
|---|---|---|
| **Step** | Propose one action, wait for approval | High-risk or unfamiliar constellations |
| **Batch** | Propose all available parallel actions, wait for batch approval | Normal operation |
| **Auto** | Execute unblocked actions automatically, report results | Low-risk, well-tested constellations (future) |

Step is the default. Auto requires explicit opt-in and is gated by constellation template metadata (only templates marked `auto_eligible` can run in auto mode). This is the compliance safety valve.

**C-13: Rollback and correction**

If a verb execution produces an unexpected result, the user must be able to:
- Undo the last action (if the verb supports rollback via the ChangeSet architecture)
- Redirect the plan ("skip this branch, focus on that one")
- Abandon the goal without losing entity state created so far

The goal frame is an overlay on the constellation; abandoning the goal doesn't revert executed verbs.

---

## 5. Architectural Fit

### 5.1 What Already Exists

| Component | Current State | Role in Motivated Sage |
|---|---|---|
| Constellation templates (SemOS) | Defined, compiled, consumed by UI | **The plan.** Template DAG is the goal structure. |
| Entity state (PostgreSQL) | Full lifecycle FSM per entity | **Current position.** Queried for hydration. |
| DSL verb surface | ~1,123 verbs across 134 domains | **The edges.** Verb = transition between states. |
| Sage utterance pipeline | Operational, 3-layer NLCI | **The executor.** Resolves and runs proposed actions. |
| Constellation UI | React panel with tree view, inspector, SVG canvas | **The dashboard.** Visual progress surface for human. |
| Macro compiler | Compiles macros to expanded verb sequences | **Sequencing knowledge.** Macro expansion order informs topological sort. |
| ChangeSet architecture | Immutable, auditable, rollback-capable | **Safety net.** Supports undo on verb execution. |
| BPMN orchestration | Durable long-running verb execution | **Background driver.** External events arrive via BPMN process completion. |

### 5.2 What Must Be Built

| Component | Complexity | Dependencies |
|---|---|---|
| `GoalFrame` session concept | Low | Session state management |
| Goal-level intent resolution | Medium | Extends NLCI pipeline with constellation-target resolution |
| `hydrate_constellation()` | Low | Constellation template + entity state queries (both exist) |
| `compute_gap()` | Low | DAG walk with state comparison — pure function |
| `detect_blockers()` | Medium | Needs typed precondition model per verb |
| `topological_sort_remaining()` | Low | Standard DAG toposort with blocked-edge pruning |
| Motivation prompt template | Low | Template string with computed values |
| Event-driven re-activation | High | Durable goal persistence, event subscriptions, notification — **deferred to v2.0+** |
| Constellation UI goal overlay | Medium | Extends existing React panel with goal state colouring |
| Approval mode switching | Low | Session-level flag |

### 5.3 The Critical Path

The minimum viable Motivated Sage requires only four new components:

1. **GoalFrame** — a struct in session state
2. **hydrate + frontier computation** — a pure function: (template, entity states) → feasible frontier + progress surface
3. **Motivation prompt template** — a string template injected into LLM context with frontier state
4. **The loop** — after verb execution, re-hydrate and re-prompt instead of returning to idle

Plus one integration constraint: **approved proposals enter the existing kernel pipeline**, not a bypass path. The Coder receives verb invocations from Motivated Sage exactly as it would from direct user utterances.

Everything else (blocker typing, parallel path analysis, watch state, approval modes, UI overlay, policy-driven ranking) is enhancement. The core mechanism is: **compute the frontier, show it to the model, let the model propose the next step, gate through approval, execute through the kernel.**

### 5.4 Integration with Sage/Coder Split

Motivated Sage is purely a Sage-side capability. It does not affect Coder mode. The Sage/Coder boundary remains:

- **Sage** (observation plane, motivated): understands the constellation, plans the path, proposes actions, maintains goal frame
- **Coder** (execution plane, reactive): resolves specific verbs to DSL, compiles, executes

When Sage proposes an action and the user approves, the proposal is handed to the Coder pipeline as a resolved verb invocation — exactly as if the user had typed the utterance directly. The Coder doesn't know or care that the utterance was agent-generated rather than human-generated. This separation is clean and preserves the existing pipeline.

### 5.5 Integration with BPMN Orchestration

Motivated Sage and BPMN operate at different layers and complement each other:

| Concern | Motivated Sage | BPMN |
|---|---|---|
| Planning horizon | Full constellation (all entities, all states) | Single long-running verb execution |
| Persistence | Session-scoped (goal frame in session) | Durable (survives restarts) |
| Execution model | Conversational (propose → approve → execute) | Autonomous (process engine drives) |
| Human interaction | Every step (or batched) | At designated human tasks |
| State changes from | Sage-driven verb execution | Background process completion |

When BPMN processes complete (e.g., screening results return), they update entity state in PostgreSQL. In v1.0, Motivated Sage detects this on the user's next interaction — re-hydrating the constellation and recomputing the frontier. The user experiences: "I see screening cleared on the ManCo since we last spoke — you can now proceed with KYC validation. Approve?" In v2.0+, event-driven re-activation (C-09 durable scope) would push this notification proactively.

### 5.6 Integration with the Semantic Traceability Kernel

Motivated Sage generates proposals. The Traceability Kernel governs execution. The boundary between them must be explicit.

**Principle: Motivated Sage does not bypass the kernel phase law.**

When Sage proposes an action and the user approves, the approved action enters the standard kernel pipeline: utterance → legality check → narrowing → resolution → execution. Phase 2 of the kernel remains authoritative. Motivated Sage is an upstream source of candidate actions, not an alternative execution path. This means:

- A proposal that passes frontier computation may still fail legality in the kernel (e.g., governance policy rejects the transition for the current user role)
- Narrowing and resolution still apply — the kernel may refine or disambiguate the verb invocation even when Sage has already identified it
- The execution trace records the full path, including that the action originated from a goal-driven proposal rather than a direct user utterance

**GoalProposalTrace: a new trace artefact.**

The kernel's existing trace model records what was executed and why at the verb level. Motivated Sage introduces a higher-level artefact: the **GoalProposalTrace**, which records the context in which a proposal was generated. Conceptually, it captures:

```
GoalProposalTrace {
    goal_frame_id: String,              // which goal this proposal serves
    frontier_snapshot: FrontierSummary,  // compact representation of actionable/blocked state
    proposed_action: VerbInvocation,     // what was proposed
    frontier_alternatives: Vec<VerbInvocation>, // what else was available
    ranking_rationale: String,          // why this action was proposed first
    user_decision: ApprovalDecision,    // APPROVED | REDIRECTED | DEFERRED
    resulting_trace_id: Option<TraceId>, // links to kernel execution trace if approved
    timestamp: Timestamp,
}
```

**Note:** The exact persistence shape — particularly what constitutes `FrontierSummary` — is deliberately left soft at vision-paper stage. A full hydrated DAG snapshot is conceptually complete but may be expensive to store; a compact frontier summary plus constellation version pin may be sufficient for audit purposes. The implementation plan should resolve this based on actual payload sizes and audit query patterns.

This creates a linked sequence: goal declaration → proposal (with frontier context) → user approval → kernel execution trace → state change → next proposal. The full chain is auditable: for any executed verb, a compliance reviewer can trace back to "why was this action proposed at this point in the onboarding?" and see the hydrated constellation state that produced the recommendation.

**Relationship to Loop 3 (operational learning).**

The kernel's Loop 3 path — learning from completed traces to build predictive prompting and pathway models — is a future consumer of GoalProposalTrace data, not something Motivated Sage should reinvent. Specifically:

- Motivated Sage v1.0 uses LLM-driven frontier ranking (the model picks from available actions based on the prompt surface)
- Loop 3, once operational, could analyse historical GoalProposalTraces to extract ranking heuristics: "for Irish ICAV onboardings, operators consistently prefer to complete all entity creates before opening KYC cases"
- Those extracted heuristics could feed back into the frontier ranking layer as policy-driven ranking functions, replacing or augmenting LLM judgment

This is a clean separation: Motivated Sage generates the proposals and records the traces; Loop 3 learns from them. Neither needs to exist for the other to function, but together they form a closed improvement loop.

---

## 6. The Mechanism: Why This Works

### 6.1 The Architecture Stands on Live-State Computation

The core mechanism does not depend on any particular model behaviour. It is:

1. **Compute the feasible frontier** from hydrated constellation state — which transitions are legal and unblocked right now
2. **Rank the frontier** — by dependency urgency, policy, or operator preference — and propose the top candidate(s)
3. **Gate execution** through human approval
4. **Re-hydrate** after state change and return to step 1

This is a deterministic planning loop driven by a state machine. It adds value even with a model that has no particular completion drive — the user still gets "here are your available next steps, ranked" instead of silence.

### 6.2 Why Prompt Framing Amplifies the Effect

The loop becomes substantially more effective when the LLM context is framed for continuation rather than idle.

An autoregressive language model predicts P(next_token | context). Its training corpus contains millions of task-execution examples where mid-task contexts (progress reports, checklists with items completed, phase-gated plans) are followed by continuation tokens. End-of-task contexts ("how can I help?") are followed by idle tokens.

When the motivation prompt injects "Progress: 41%, next action available, → propose next step," the model is placed in a context distribution where continuation is the statistically dominant response. This is the same effect exploited by the E-invariant pattern in Claude Code, but Motivated Sage's version is more robust because the prompt surface is computed from live state rather than statically authored.

The amplification is real and useful, but the paper does not claim it as the architectural foundation. The foundation is the state machine. The prompt framing is the accelerant.

### 6.3 Why the Constellation Makes the Frontier Trustworthy

The feasible frontier is dynamically accurate because it is derived from live state:

- Progress percentage reflects actual database state, not a counter
- Available actions are computed from real precondition checks, not a static list
- Blockers reflect actual external dependencies, not assumed ones
- The frontier is recomputed after every state change

This means the agent cannot be driven to propose an action whose preconditions aren't met. The prompt surface only presents genuinely available transitions. Unlike a static plan (which can drift from reality), the constellation-computed frontier is always current.

### 6.4 The Approval Gate as Alignment Mechanism

The approval gate serves a dual purpose:

1. **Safety**: the human verifies each proposed action before execution in a compliance-sensitive domain
2. **Strategic correction**: the constellation computes what is *legal*; the human asserts what is *preferred*. When the agent proposes an action that is technically valid but strategically wrong ("yes, you *could* run screening now, but I want to finalise the structure first"), the human redirects and the agent re-ranks the frontier

The approval gate is not a limitation of the architecture — it is the mechanism by which domain strategy (which the constellation does not fully encode) enters the planning loop.

---

## 7. Scope Boundaries

### 7.1 In Scope (v1.0)

- Goal frame concept and session binding (session-scoped, non-durable)
- **Explicit goal binding only** — v1.0 requires the user to specify a constellation template and case directly (by name, ID, or unambiguous reference), not via open natural-language inference. This is a deliberate product decision: the planning loop is the innovation; goal binding can start explicit and graduate to NL in v1.1.
- Constellation hydration from live entity state
- Gap analysis with frontier computation and progress metrics
- Frontier ranking (LLM-driven from prompt surface; provisional heuristic, not a truth source)
- Motivation prompt template (continuation-biased context framing)
- Step-mode approval (propose one, wait, execute, re-assess)
- Basic blocker reporting (blocked/unblocked, no typed categories)
- Session-scoped watch state (pull-model: re-assess on user re-engagement)
- Integration with existing Sage utterance pipeline
- **Approved proposals flow through the standard kernel phase law** (utterance → legality → narrowing → resolution → execution)

### 7.2 In Scope (v1.1)

- **Goal-level intent resolution from natural language (C-02)** — graduates from explicit binding to NL inference with the ambiguity resolution policy defined in §4.1
- Batch-mode approval (propose parallel actions)
- Typed blocker categories (C-07)
- Parallel path identification (C-08)
- Constellation UI goal overlay (C-11)

### 7.3 Future (v2.0+)

- Auto-mode execution for low-risk constellations
- Durable watch state with event-driven re-activation (C-09 full scope)
- Durable goal frame persistence across session boundaries
- Policy-driven frontier ranking functions (replacing LLM-only ranking)
- Loop 3 integration: learning ranking heuristics from GoalProposalTrace history
- Cross-constellation coordination (multiple active goals)
- Goal templates derived from successful execution traces

### 7.4 Out of Scope

- Changes to the Coder pipeline
- Changes to verb execution mechanics
- Changes to BPMN orchestration
- New constellation templates (this paper addresses the runtime loop, not the content)
- Multi-user coordination (multiple agents on the same goal)

---

## 8. Open Questions

**Q1: Goal frame persistence.** Should the goal frame survive session boundaries? If the user closes the chat and returns tomorrow, should Sage resume the goal? The ChangeSet architecture provides durability for executed actions, but the goal frame itself is currently session-scoped. Promoting it to a durable entity adds complexity but enables multi-session onboarding workflows.

**Q2: Constellation template granularity.** Current macro definitions (M1–M18) map to specific fund structures. Should goal-level intent resolution target macros, or should there be higher-level goal templates that compose multiple macros? E.g., "onboard a cross-border feeder-master structure" might involve M15 + M16 + M17.

**Q3: Approval fatigue.** In step mode, a 22-node constellation requires up to 22 approval cycles. Batch mode helps, but for well-understood constellations, the user may want to approve the entire plan upfront and let Sage execute. How do we design the escalation from step → batch → auto in a way that's auditable and regulatorily defensible?

**Q4: Conflict with user intent.** If the user asks Sage to do something that contradicts the active goal frame (e.g., "delete that entity" when the goal frame expects it to reach ACTIVE), should Sage warn, comply silently, or require explicit goal modification? The answer affects how strongly the goal frame constrains behaviour.

**Q5: Prompt budget.** The motivation prompt template consumes context window tokens. For a 22-node constellation with 8 blockers, the prompt block might be 400–600 tokens. In a long conversation, this is non-trivial. Should the prompt surface be compressed at high progress levels? (At 90% complete with 2 nodes remaining, the full DAG state adds noise.)

**Q6: Observability for compliance.** Regulatory audit trails need to record not just *what* was executed but *why*. The GoalProposalTrace artefact (§5.6) addresses this at the proposal level, but the full chain — goal declaration → frontier state → proposal → approval → kernel execution → state change — may require additional linking infrastructure beyond what the current ChangeSet model provides. How tightly should GoalProposalTrace be coupled to the kernel's existing trace model vs being a parallel artefact with cross-references?

---

## 9. Relationship to Prior Architecture Papers

| Paper | Relationship |
|---|---|
| **NLCI Architecture v1.2** | Motivated Sage extends the utterance pipeline with goal-level intent resolution. The three-layer decomposition (observation → Semantic IR → verb resolution) remains; a new resolution target (constellation template) is added alongside verb resolution. |
| **SemOS Constellation Model v1.2** | This paper is the **runtime activation** of the constellation model. The constellation paper defined the static structure (templates, DAG format, one-DAG principle). This paper defines how that structure drives agent behaviour at runtime. |
| **Sage/Coder Pipeline Architecture** | Motivated Sage operates entirely within the Sage plane. The Sage/Coder boundary is preserved. Coder receives resolved verb invocations from Motivated Sage exactly as it would from direct user utterances. |
| **CBU Entity Documentation** | The CBU lifecycle FSM (DISCOVERED → ... → TERMINATED) is a concrete instance of the entity-level state machine that Motivated Sage hydrates and navigates. Every entity doc produced using the CBU template provides the state/transition definitions that gap analysis consumes. |
| **Stewardship Agent / ChangeSet Architecture** | The ChangeSet model provides the auditability and rollback infrastructure that makes the approval-gated execution loop safe. Motivated Sage creates ChangeSets for each approved action, maintaining the immutable audit trail. |
| **Document Polymorphism & StateGraph** | StateGraph (entity scope → graph walk → LLM select) is conceptually parallel to Motivated Sage's gap analysis. Both walk a graph to determine available actions. StateGraph operates at the document/proof level; Motivated Sage operates at the constellation/entity level. They may share infrastructure. |
| **Semantic Traceability Kernel** | Motivated Sage sits **above** the kernel phase law. Proposals are generated from the hydrated constellation frontier, but once approved they enter the standard kernel pipeline (legality → narrowing → resolution → execution). GoalProposalTrace is a new trace artefact linking goal-level reasoning to kernel execution traces. Loop 3 operational learning is a future consumer of proposal trace data, not something Motivated Sage reinvents. |

---

## 10. Summary

Motivated Sage is the architectural answer to a single question: **how do you make an LLM agent behave as though it wants to finish the job?**

The answer: you compute the feasible frontier from live state, rank it, show it to the model in a continuation-biased frame, gate execution through human approval, and re-compute after every state change. The architecture stands on the state machine — the constellation provides the map, entity state provides the current position, gap analysis provides the frontier, and the approval gate provides the safety. Prompt framing amplifies the effect but is not the foundation.

The mechanism is simple. The capability it unlocks is not: a compliance-domain AI agent that actively drives onboarding workflows to completion, maintains awareness across multi-entity constellations, handles parallel paths and blockers intelligently, and re-plans dynamically as state changes — all while keeping the human in the approval loop, the kernel phase law intact, and the audit trail traceable from goal declaration through every proposal to every executed verb.

---

*End of document.*
