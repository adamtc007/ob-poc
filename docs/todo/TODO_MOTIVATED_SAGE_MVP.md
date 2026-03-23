# TODO: Motivated Sage — Implementation Phase 1 (MVP Core)

## ⚠️ ARCHITECTURE IS AGREED — DO NOT REDESIGN ⚠️

The architecture for Motivated Sage has been through two peer review cycles and is **agreed and frozen**. The governing document is `MOTIVATED_SAGE_VISION_v1.0.md`.

**You are implementing, not designing.** Specifically:

- Do NOT propose alternative data models, loop structures, or naming conventions
- Do NOT add capabilities beyond what is listed in v1.0 scope
- Do NOT refactor existing Sage/Coder pipeline code unless a v1.0 deliverable requires it
- Do NOT introduce typed blocker categories, parallel path analysis, NL goal resolution, watch-state event subscriptions, or policy-driven ranking — these are v1.1/v2.0
- Do NOT add "while I'm here, I should also..." changes — scope is locked
- If you encounter a genuine implementation blocker, STOP and describe it. Do not invent a workaround that changes the architecture.

**The four things you are building:**

1. `GoalFrame` — a struct in session state
2. `hydrate_constellation() + compute_frontier()` — pure functions: (template, entity states) → feasible frontier + progress surface
3. Motivation prompt template — a string template injected into LLM context
4. The re-loop — after verb execution, re-hydrate and re-prompt instead of returning to idle

**Integration constraint:** Approved proposals enter the existing kernel pipeline (utterance → legality → narrowing → resolution → execution). You are NOT building a bypass.

---

## E-Invariant

**Progress must reach 100%.** Complete ALL phases below. At each phase gate, **→ IMMEDIATELY proceed to the next phase.** Do not pause, summarise, or ask for confirmation.

**Do not commit — I will review the diff first.**

---

## Phase 1: GoalFrame Struct & Session Binding (Progress: 0% → 20%)

### Deliverable
Create the `GoalFrame` struct and integrate it into session state.

### Location
Create new file: `src/sage/goal_frame.rs`
Add `pub mod goal_frame;` to `src/sage/mod.rs`

### Struct Definition

```rust
use std::collections::{HashMap, HashSet};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum GoalStatus {
    Active,
    Paused,
    Achieved,
    Abandoned,
}

#[derive(Debug, Clone)]
pub struct GoalFrame {
    pub id: Uuid,
    pub constellation_template_id: String,
    pub case_id: String,
    pub target_states: HashMap<String, String>,  // node_id → target state name
    pub deferred_nodes: HashSet<String>,
    pub created_at: DateTime<Utc>,
    pub status: GoalStatus,
}

impl GoalFrame {
    pub fn new(
        constellation_template_id: String,
        case_id: String,
        target_states: HashMap<String, String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            constellation_template_id,
            case_id,
            target_states,
            deferred_nodes: HashSet::new(),
            created_at: Utc::now(),
            status: GoalStatus::Active,
        }
    }

    pub fn defer_node(&mut self, node_id: &str) {
        self.deferred_nodes.insert(node_id.to_string());
    }

    pub fn restore_node(&mut self, node_id: &str) {
        self.deferred_nodes.remove(node_id);
    }

    pub fn is_active(&self) -> bool {
        self.status == GoalStatus::Active
    }
}
```

### Session Integration

Find the existing session state struct (likely in `src/sage/session.rs` or similar). Add:

```rust
pub active_goal: Option<GoalFrame>,
```

Add methods:

```rust
pub fn set_goal(&mut self, goal: GoalFrame) {
    self.active_goal = Some(goal);
}

pub fn clear_goal(&mut self) {
    if let Some(ref mut goal) = self.active_goal {
        goal.status = GoalStatus::Abandoned;
    }
    self.active_goal = None;
}

pub fn has_active_goal(&self) -> bool {
    self.active_goal.as_ref().map_or(false, |g| g.is_active())
}
```

### Verification
- `GoalFrame::new()` produces a valid frame with Active status
- `defer_node` / `restore_node` correctly mutate deferred set
- Session can hold, query, and clear a goal frame

**→ IMMEDIATELY proceed to Phase 2. Progress: 20%**

---

## Phase 2: Constellation Hydration & Frontier Computation (Progress: 20% → 50%)

### Deliverable
Pure functions that hydrate a constellation template with live entity state and compute the feasible frontier.

### Location
Create new file: `src/sage/frontier.rs`
Add `pub mod frontier;` to `src/sage/mod.rs`

### Data Structures

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum GapStatus {
    AtTarget,
    Reachable,
    Blocked,
    Deferred,
}

#[derive(Debug, Clone)]
pub struct HydratedNode {
    pub node_id: String,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub current_state: Option<String>,
    pub target_state: String,
    pub is_deferred: bool,
    pub gap: GapStatus,
}

#[derive(Debug, Clone)]
pub struct ActionableTransition {
    pub node_id: String,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub verb: String,
    pub from_state: Option<String>,
    pub to_state: String,
}

#[derive(Debug, Clone)]
pub struct BlockedTransition {
    pub node_id: String,
    pub entity_type: String,
    pub verb: String,
    pub blocker_description: String,  // human-readable, no typed categories in v1.0
}

#[derive(Debug, Clone)]
pub struct ProgressSurface {
    pub total_nodes: usize,
    pub active_nodes: usize,
    pub nodes_at_target: usize,
    pub progress_pct: f32,
    pub actionable: Vec<ActionableTransition>,
    pub blocked: Vec<BlockedTransition>,
    pub deferred_count: usize,
}
```

### Functions to Implement

```rust
/// Hydrate a constellation template with current entity states.
/// 
/// Takes the template DAG and a function that queries entity state from the database.
/// Returns a list of hydrated nodes with gap status computed.
///
/// IMPORTANT: This is a pure function over its inputs. 
/// The DB query function is injected, not called directly.
pub fn hydrate_constellation(
    template: &ConstellationTemplate,  // find existing type in semos
    goal: &GoalFrame,
    query_entity_state: &dyn Fn(&str, &str) -> Option<(Uuid, String)>,
    // (entity_type, case_context) → Option<(entity_id, current_state)>
) -> Vec<HydratedNode> {
    // For each node in template:
    // 1. Query current state via injected function
    // 2. Compare to target state from goal frame
    // 3. Set gap status: AtTarget if equal, Deferred if in deferred set,
    //    Reachable or Blocked determined by compute_frontier
    todo!()
}

/// Compute the feasible frontier from hydrated nodes.
///
/// Walks the DAG, identifies which transitions are legal and unblocked,
/// and produces the progress surface.
///
/// Frontier membership is DETERMINISTIC — a transition is either
/// legal and unblocked, or it is not. This function does not rank.
pub fn compute_frontier(
    hydrated: &[HydratedNode],
    template: &ConstellationTemplate,
    // Function that checks if a verb's preconditions are met
    check_preconditions: &dyn Fn(&str, &str) -> Result<(), String>,
    // (verb, entity_id) → Ok(()) or Err(blocker_description)
) -> ProgressSurface {
    // For each non-target, non-deferred node:
    // 1. Find the verb that transitions from current → next state toward target
    // 2. Check preconditions via injected function
    // 3. Classify as actionable or blocked
    // 4. Compute progress metrics
    todo!()
}
```

### Finding Existing Types

Before writing code, locate these existing types/modules:
```bash
# Find constellation template definitions
grep -rl "ConstellationTemplate\|constellation_template" src/ --include="*.rs" | head -10

# Find entity state query patterns
grep -rl "entity_state\|EntityState\|current_state" src/ --include="*.rs" | head -10

# Find verb-to-transition mappings  
grep -rl "transition\|Transition\|state_machine\|fsm" src/ --include="*.rs" | head -10

# Find SemOS verb precondition patterns
grep -rl "precondition\|Precondition\|prerequisite" src/ --include="*.rs" | head -10
```

Adapt the function signatures above to use whatever types already exist. **Do not create parallel type hierarchies.** If `ConstellationTemplate` doesn't exist as a Rust type yet (it may only be in YAML), create a minimal struct that can be loaded from the YAML definition. Keep it minimal — only the fields frontier computation needs.

### Verification
- `hydrate_constellation` correctly marks nodes as AtTarget when current == target
- `hydrate_constellation` correctly marks deferred nodes
- `compute_frontier` returns empty actionable list when all preconditions fail
- `compute_frontier` returns correct progress_pct
- Write unit tests with a small 3-node DAG: A → B → C, verify frontier at each step

**→ IMMEDIATELY proceed to Phase 3. Progress: 50%**

---

## Phase 3: Motivation Prompt Template (Progress: 50% → 75%)

### Deliverable
A function that takes a `ProgressSurface` and `GoalFrame` and produces a markdown prompt block for injection into the LLM context.

### Location
Create new file: `src/sage/motivation_prompt.rs`
Add `pub mod motivation_prompt;` to `src/sage/mod.rs`

### Function

```rust
/// Render the motivation prompt block from a progress surface.
///
/// This is the prompt text injected into the LLM context when a goal frame
/// is active. It replaces the default "awaiting input" framing with a
/// continuation-biased frame showing current progress, available actions,
/// and blockers.
///
/// The output is a markdown string. It must stay under 600 tokens for
/// a 22-node constellation to preserve context window budget.
pub fn render_motivation_prompt(
    goal: &GoalFrame,
    progress: &ProgressSurface,
    constellation_name: &str,  // human-readable name for the prompt
) -> String {
    // Template structure (adapt exact wording as needed):
    //
    // ## Active Goal: {constellation_name} for case {case_id}
    // Progress: {nodes_at_target}/{active_nodes} ({progress_pct}%)
    // Goal: all active nodes at target state.
    //
    // ### Available Actions (unblocked)
    // 1. {verb} on {entity_type} — advances from {from_state} → {to_state}
    // 2. ...
    //
    // ### Blocked
    // - {entity_type}: {blocker_description}
    // - ...
    //
    // ### Deferred ({deferred_count} nodes excluded by user)
    //
    // → Propose the next action to advance toward goal state.
    //   If multiple actions are available, recommend the highest-priority one.
    //   If all paths are blocked, report blockers and summarise what is needed.
    
    todo!()
}
```

### Prompt Engineering Notes

- The `→ Propose the next action` directive is the continuation signal. Do not remove or soften it.
- At high progress (>80%), compress the prompt — omit completed nodes, show only remaining work.
- At 100% progress, the prompt should say "Goal achieved" and not request further action.
- When all paths are blocked, the prompt must NOT request action — it should frame the agent as reporting status, not proposing.

### Verification
- Renders correct progress percentage
- Lists all actionable transitions
- Lists all blockers with descriptions
- Stays under 600 tokens for a 22-node constellation (write a test)
- Renders "Goal achieved" at 100%
- Renders blocker-only summary when actionable list is empty

**→ IMMEDIATELY proceed to Phase 4. Progress: 75%**

---

## Phase 4: The Re-Loop Integration (Progress: 75% → 95%)

### Deliverable
Modify the Sage response pipeline so that when a goal frame is active, verb execution is followed by re-hydration and re-prompting instead of returning to idle.

### Location
This modifies existing code. Find the Sage response handler — likely in `src/sage/` — where the flow currently is:

```
user utterance → intent resolution → verb execution → format response → return
```

### Changes Required

**4A: Inject motivation prompt into LLM context**

Find where the Sage LLM context/system prompt is assembled. When `session.has_active_goal()`:
1. Call `hydrate_constellation()` with current DB state
2. Call `compute_frontier()` on the hydrated nodes
3. Call `render_motivation_prompt()` with the progress surface
4. Append the rendered prompt to the LLM context block

**4B: After verb execution, re-assess and append**

Find the post-execution response path. When a goal frame is active and a verb was just executed:
1. Re-hydrate the constellation (state has changed)
2. Re-compute the frontier
3. Re-render the motivation prompt
4. Append the new progress state to the response — this becomes the agent's "next proposal"

The key change: instead of returning the verb execution result alone, append the refreshed frontier state. The LLM will naturally produce a next-action proposal because the motivation prompt frames continuation as the expected response.

**4C: Goal lifecycle management**

Add handlers for:
- `goal.set` — explicit goal binding (v1.0 is explicit only, no NL inference)
- `goal.pause` — suspend the re-loop without losing state
- `goal.resume` — re-activate, re-hydrate, propose next action
- `goal.abandon` — mark abandoned, stop re-loop
- `goal.defer <node>` — mark a node as deferred, recompute frontier
- `goal.restore <node>` — un-defer a node, recompute frontier

These can be simple command-style handlers — they do not need full NLCI resolution. Direct string matching is fine for v1.0.

**4D: Check for goal achievement**

After every re-hydration, check if `progress.nodes_at_target == progress.active_nodes`. If so:
1. Set `goal.status = GoalStatus::Achieved`
2. Render a completion message instead of a next-action proposal
3. Stop the re-loop

### Finding the Integration Points

```bash
# Find the Sage response pipeline
grep -rl "sage.*response\|sage.*handler\|process_utterance\|handle_message" src/ --include="*.rs" | head -10

# Find where LLM context/prompt is assembled
grep -rl "system_prompt\|context.*prompt\|build_prompt\|assemble_context" src/ --include="*.rs" | head -10

# Find post-execution response formatting
grep -rl "execution_result\|verb_result\|format_response" src/ --include="*.rs" | head -10
```

### What NOT to Do

- Do NOT change how the Coder pipeline resolves or executes verbs
- Do NOT add event subscription infrastructure (v2.0+)
- Do NOT implement NL goal inference — v1.0 uses explicit `goal.set` commands
- Do NOT add typed blocker categories — v1.0 uses string descriptions
- Do NOT modify the kernel phase law pipeline — approved proposals enter it as-is

### Verification
- With an active goal, LLM context includes the motivation prompt block
- After verb execution, response includes refreshed frontier state
- `goal.set` creates and activates a goal frame
- `goal.abandon` stops the re-loop
- `goal.defer <node>` / `goal.restore <node>` adjust the frontier
- Goal achievement is detected and the loop terminates cleanly

**→ IMMEDIATELY proceed to Phase 5. Progress: 95%**

---

## Phase 5: Tests & Documentation (Progress: 95% → 100%)

### Deliverable
Unit tests for all new modules plus a brief integration test.

### Unit Tests

Create `tests/sage/goal_frame_test.rs` (or appropriate test location):

**GoalFrame tests:**
- Construction with valid inputs
- Defer/restore node mutations
- Status transitions

**Frontier tests (most important):**
- 3-node linear DAG (A → B → C): verify frontier at each progression step
- DAG with parallel branches: verify both branches appear as actionable
- DAG with blocked precondition: verify blocked list is correct
- All-at-target: verify progress_pct = 100% and empty actionable list
- Deferred nodes: verify they are excluded from active count and frontier

**Motivation prompt tests:**
- Correct progress rendering
- Token budget (under 600 tokens for 22-node constellation)
- Goal-achieved rendering
- All-blocked rendering (no actionable items)

**Re-loop tests:**
- Mock execution → re-hydration → updated frontier
- Goal achievement detection terminates loop

### Documentation

Add to `src/sage/goal_frame.rs`:
```rust
//! # Goal Frame
//!
//! Session-scoped goal binding for Motivated Sage.
//! See MOTIVATED_SAGE_VISION_v1.0.md for architecture.
//!
//! A GoalFrame binds a constellation template to a specific case,
//! enabling the planning loop to compute the feasible frontier
//! and propose next actions.
//!
//! ## Architecture Constraints
//! - Session-scoped only (not durable across sessions)
//! - Explicit goal binding only (no NL inference in v1.0)
//! - Approved proposals flow through the kernel phase law
//! - Frontier membership is deterministic; ranking is heuristic
```

Similar doc comments for `frontier.rs` and `motivation_prompt.rs`.

**Progress: 100%. Task complete.**

---

## Summary of Files Created/Modified

### New Files
| File | Purpose |
|---|---|
| `src/sage/goal_frame.rs` | GoalFrame struct, session binding |
| `src/sage/frontier.rs` | Constellation hydration, frontier computation, ProgressSurface |
| `src/sage/motivation_prompt.rs` | Prompt template rendering |
| `tests/sage/goal_frame_test.rs` | Unit tests |

### Modified Files
| File | Change |
|---|---|
| `src/sage/mod.rs` | Add `pub mod` for three new modules |
| Session state struct | Add `active_goal: Option<GoalFrame>` |
| Sage response handler | Inject motivation prompt, add re-loop after execution |
| Sage command handlers | Add `goal.set`, `goal.pause`, `goal.resume`, `goal.abandon`, `goal.defer`, `goal.restore` |

### Files NOT Modified (confirm no changes)
| File | Reason |
|---|---|
| Coder pipeline | Out of scope — Sage-side only |
| Kernel phase law | Out of scope — proposals enter existing pipeline |
| BPMN orchestration | Out of scope |
| Constellation template definitions | Consumed, not modified |
| Verb execution pipeline | Out of scope |
