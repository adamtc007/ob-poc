# Catalogue Platform Refinement — Vision & Scope (v1.3)

> **Status:** DRAFT — peer review pending (Adam, 2026-04-24).
> **Date:** 2026-04-24 (v1.3 draft, post-Tranche-2).
> **Prior versions:** v1.0 (2026-04-18), v1.1 (2026-04-22), v1.2 (2026-04-23),
> all retained in `/docs/todo/`.
>
> **v1.3 changes from v1.2:** 7 amendments codifying patterns that
> recurred across the 4 Tranche 2 workspace DAGs (IM pilot + KYC +
> Deal + CBU). 4 have 3-workspace evidence (P0); 3 have 2-workspace
> evidence (P1). Plus one new principle (P17) that reframes how
> cross-workspace state composition works.
>
> **Decision level:** peer-review draft. Scope adjudicated by Adam
> 2026-04-24 (D-1..D-5, see
> `tranche-2-cross-workspace-reconciliation-2026-04-24.md` §8).
>
> **Input findings:**
> - `instrument-matrix-pilot-findings-2026-04-23.md` (incl. §10 phase-axis addendum)
> - `tranche-2-kyc-findings-2026-04-23.md`
> - `tranche-2-deal-findings-2026-04-23.md` (incl. §7 — 13 business-reality gaps)
> - `tranche-2-cbu-findings-2026-04-23.md` (incl. §7.0 foundational + §7.1-7.3 — 13 gaps)
> - `tranche-2-cross-workspace-reconciliation-2026-04-24.md`

---

## v1.3 CHANGES FROM v1.2 — at a glance

| # | Amendment | Type | Evidence | Severity |
|---|---|---|---|---|
| V1.3-1 | **`cross_workspace_constraints:` block** — cross-DAG blocking gates (CAND-2/5) | Schema extension | 3 workspaces | P0 |
| V1.3-2 | **`derived_cross_workspace_state:` block** — cross-DAG aggregate / projection state (CAND-13, NEW) | Schema extension | 1 workspace (emerging) | P0 |
| V1.3-3 | **`parent_slot:` + `state_dependency:` on slots** — hierarchy + cascade (CAND-11) | Schema extension | 3 workspaces | P0 |
| V1.3-4 | **SUSPENDED universal convention + validator lint** (CAND-10) | Convention + lint | 3 workspaces | P0 |
| V1.3-5 | **`dual_lifecycle:` on slots** — linked lifecycles with junction state (CAND-7/9) | Schema extension | 2 workspaces | P1 |
| V1.3-6 | **`periodic_review_cadence:` + `validity_window:`** — regulatory refresh cadence (CAND-3/12) | Schema extension | 2 workspaces | P1 |
| V1.3-7 | **Commercial-commitment tier-apply convention** (CAND-8) | Convention | 1 workspace | P1 |

Plus one new principle:

| # | Principle | Scope |
|---|---|---|
| P17 | **Cross-workspace state composition** — state in one workspace may be blocked-by, derived-from, or cascaded-from state in another workspace. Three distinct composition modes. | Architectural |

Plus one foundational reframe documented (not a new amendment,
dominates v1.3 interpretation):

- **CBU as money-making apparatus.** CBU is the operational trading
  unit a client has established on the market. Its purpose-in-life
  is to be operationally active. This reframing drives the CBU DAG
  re-centring (R-3) and IM phase-axis re-anchor (R-4); spec-level,
  it shapes how P16 layers + P17 composition interact.

---

## 1. NEW PRINCIPLE — P17 (cross-workspace state composition)

**P17 — State in one workspace may compose with state in another
workspace via three distinct modes, each with different semantics,
schema, and runtime behaviour.**

v1.2's `cross_slot_constraints` pattern was **intra-DAG** — it
governed dependencies between slots within a single workspace. The
Tranche 2 authoring repeatedly surfaced the need for **inter-DAG**
state relationships. Three distinct modes emerge:

### 1.1 Mode A — Blocking (constraint)

> Slot X in workspace A cannot transition to state S unless slot Y
> in workspace B is in state T.

This is a **gate** — A's progress is blocked by B's state. No
projection; validator + runtime check at transition time.

**Examples:**
- `deal.CONTRACTED` requires `kyc_case.APPROVED` (Deal workspace
  blocked by KYC workspace)
- `cbu.VALIDATED` requires `kyc_case.APPROVED` (CBU workspace
  blocked by KYC workspace)

**v1.3 schema:** §2.1 V1.3-1 `cross_workspace_constraints:`.

### 1.2 Mode B — Aggregation / projection (derivation)

> Slot X in workspace A has a state that is DERIVED FROM the
> conjunction of slots in workspaces B, C, D.

This is a **projection** — A's state is computed from the others'
states. No blocking; A passively reflects the aggregate. Runtime
computes on-the-fly (D-3 decision).

**Example:**
- `cbu.operationally_active` = `kyc_case.APPROVED` AND
  `deal.status ∈ {CONTRACTED, ONBOARDING, ACTIVE}` AND
  `im.trading_enablement ∈ {trade_permissioned, actively_trading}`
  AND `cbu_evidence.all_verified`

The CBU doesn't block anything — it AGGREGATES and exposes the
compound state as a first-class projected state on itself.

**v1.3 schema:** §2.2 V1.3-2 `derived_cross_workspace_state:`.

### 1.3 Mode C — Hierarchy cascade (state dependency)

> Child slot's state depends on parent slot's state. State changes
> in parent cascade to child. Parent and child may be in different
> workspaces.

This is a **hierarchical dependency** — parent-child link is
state-aware; child cannot be in certain states if parent is in
certain states.

**Examples:**
- Feeder fund CBU's operational state depends on master fund CBU's
  operational state (CBU G-12)
- Deal schedule's legal force depends on master deal's state
  (Deal G-9)
- KYC entity_workstream's state depends on its parent case's state
  (KYC, already modelled)

**v1.3 schema:** §2.3 V1.3-3 `parent_slot:` + `state_dependency:`.

### 1.4 Decision rule — which mode to use

> When modelling cross-workspace state interaction, classify:
>
> - Is A's progress prevented by B's state, with no projection?
>   → **Mode A blocking** (V1.3-1 cross_workspace_constraints)
> - Does A expose a compound state derived from B + C + D?
>   → **Mode B aggregation** (V1.3-2 derived_cross_workspace_state)
> - Is there a parent/child relationship where state flows from
>   parent to child?
>   → **Mode C cascade** (V1.3-3 parent_slot + state_dependency)
>
> Same workspaces may use multiple modes — Deal.CONTRACTED is
> blocked by KYC (Mode A) AND cbu.operationally_active aggregates
> from KYC/Deal/IM (Mode B). They are orthogonal concerns.

**Relationship to P16:** P16 established layer stratification
(DAG → service resources → operations). P17 operates at Layer 1
(DAG) and governs how Layer 1 slots compose across workspaces.

**Sources:** Deal findings §7; CBU findings §7.0; reconciliation
pass §3.

---

## 2. SCHEMA EXTENSIONS (V1.3-1 through V1.3-6)

### 2.1 V1.3-1 — `cross_workspace_constraints:` block

DAG taxonomy YAML (`config/sem_os_seeds/dag_taxonomies/*.yaml`)
gains a new optional top-level section parallel to
`cross_slot_constraints:`.

**Schema:**

```yaml
cross_workspace_constraints:
  - id: <unique_id>
    description: "<human-readable>"
    source_workspace: <workspace_name>     # e.g. kyc
    source_slot: <slot_id>                  # e.g. kyc_case
    source_state: <state_id | [state_id]>   # blocker must be IN this state
    source_predicate: "<optional SQL-like or DSL predicate>"  # alternative to source_state for complex conditions
    target_workspace: <workspace_name>     # e.g. deal
    target_slot: <slot_id>                  # e.g. deal
    target_transition: "<from_state> -> <to_state>"  # or "* -> <to_state>" for any source
    severity: error | warning | informational
```

**Semantics:**
- `source_workspace` is the producer of the gating state.
- `target_workspace` is the consumer; its transition is gated.
- Either `source_state` OR `source_predicate` — not both.
- `target_transition` uses `*` to mean any source state.
- Validator loads the source workspace's DAG + runtime state and
  checks the predicate at target transition time.
- `severity: error` blocks the transition; `warning` permits it
  with diagnostic; `informational` logs only.

**Worked example — Deal blocks on KYC:**

```yaml
# In deal_dag.yaml
cross_workspace_constraints:
  - id: deal_contracted_requires_kyc_approved
    description: "Deal cannot reach CONTRACTED until scoping KYC case is APPROVED"
    source_workspace: kyc
    source_slot: kyc_case
    source_state: APPROVED
    source_predicate: "cases.client_group_id = this_deal.primary_client_group_id"
    target_workspace: deal
    target_slot: deal
    target_transition: "KYC_CLEARANCE -> CONTRACTED"
    severity: error
```

**Migration from v1.2:** existing Deal + CBU + KYC DAGs have such
constraints expressed INSIDE `cross_slot_constraints:` with
`v1_3_candidate: true` markers. Migration script moves them to the
new section. No breaking changes — v1.2 `cross_slot_constraints:`
with intra-DAG scope is unchanged.

### 2.2 V1.3-2 — `derived_cross_workspace_state:` block

DAG taxonomy YAML gains a new optional top-level section for
projection/aggregation.

**Schema:**

```yaml
derived_cross_workspace_state:
  - id: <unique_id>
    description: "<human-readable>"
    host_workspace: <workspace_name>       # workspace that exposes the state
    host_slot: <slot_id>                    # slot on which the derived state lives
    host_state: <state_id>                  # name of the derived state
    derivation:
      all_of:                               # all listed conditions must hold
        - { workspace: <ws>, slot: <slot>, state: <state> }
        - { workspace: <ws>, slot: <slot>, state: [<st1>, <st2>] }  # state-in-set
        - { workspace: <ws>, slot: <slot>, predicate: "<expr>" }     # predicate
      any_of:                               # at least one must hold (optional)
        - { workspace: <ws>, slot: <slot>, state: <state> }
    exposure:
      visible_as: first_class_state | annotation
      cacheable: true | false               # runtime may cache; default true
```

**Semantics:**
- `host_slot.host_state` is a DERIVED state — it never has an
  underlying schema column. Runtime computes it on-the-fly (per
  D-3) by evaluating the `derivation` block.
- `all_of` + `any_of` are both optional; at least one required.
- Each condition references a slot in any workspace; runtime
  cross-loads state at query time.
- `exposure.visible_as: first_class_state` means the state shows
  up in the host slot's state machine as a virtual state (for UI,
  verb-surface filtering). `annotation` means it's a side-label
  only, not gating anything.
- `cacheable: true` allows the runtime to memoise within a
  request scope; `false` forces recomputation.

**Runtime implementation (per D-3 — on-the-fly):**
- No materialised column, no trigger.
- Computed inside the constellation projection pipeline when a
  `host_slot` is hydrated.
- Cache scope = single request/session turn (cacheable true).
- Invalidation: automatic per-request; no cross-request
  invalidation complexity.

**Worked example — CBU operationally_active:**

```yaml
# In cbu_dag.yaml (post-R-3 re-centring)
derived_cross_workspace_state:
  - id: cbu_operationally_active
    description: |
      Aggregate operational-readiness state for the CBU. Reflects
      whether the CBU is cleared, contracted, trade-enabled, and
      evidenced. Projected from KYC + Deal + IM + CBU evidence.
    host_workspace: cbu
    host_slot: cbu
    host_state: operationally_active
    derivation:
      all_of:
        - { workspace: kyc, slot: kyc_case, state: APPROVED }
        - { workspace: deal, slot: deal, state: [CONTRACTED, ONBOARDING, ACTIVE] }
        - { workspace: im, slot: trading_profile, state: [trade_permissioned, actively_trading] }
        - { workspace: cbu, slot: cbu_evidence, predicate: "all cbu_evidence WHERE cbu_id = this.cbu_id .verification_status = 'VERIFIED'" }
    exposure:
      visible_as: first_class_state
      cacheable: true
```

This is the highest-leverage P0 candidate — it enables the CBU
workspace to OWN the aggregate operational state without
duplicating state across DAGs.

**Interaction with `overall_lifecycle:` (V1.2-2):** overall
lifecycle can reference derived cross-workspace states in phase
derivation clauses. Example:

```yaml
overall_lifecycle:
  phases:
    - name: actively_trading
      derivation:
        all_of:
          - "cbu.operationally_active"  # resolves to the derived state
```

### 2.3 V1.3-3 — `parent_slot:` + `state_dependency:` on slots

Slots gain optional `parent_slot:` and `state_dependency:` fields
to express hierarchy with state cascade. Parent may be in the same
workspace OR a different workspace.

**Schema:**

```yaml
slots:
  - id: <child_slot_id>
    stateless: false
    state_machine: { ... }
    parent_slot:
      workspace: <workspace_name>       # optional; defaults to same workspace
      slot: <parent_slot_id>
      join:                              # how child rows are linked to parent
        via: <join_table>
        parent_fk: <column>
        child_fk: <column>
    state_dependency:
      # Map of parent-state -> child-allowed-states
      # If parent is in a state NOT listed here, child is unconstrained
      cascade_rules:
        - parent_state: SUSPENDED
          child_allowed_states: [SUSPENDED, CLOSED]
          cascade_on_parent_transition: true   # auto-transition child when parent transitions
          default_child_state_on_cascade: SUSPENDED
        - parent_state: WINDING_DOWN
          child_allowed_states: [WINDING_DOWN, CLOSED, SUSPENDED]
      severity: error | warning
```

**Semantics:**
- If parent is in `SUSPENDED` state, validator enforces that child
  MUST be in one of the listed `child_allowed_states`.
- `cascade_on_parent_transition: true` means when parent
  transitions into this state, runtime auto-transitions child to
  `default_child_state_on_cascade` (within the child's own
  allowed transitions — if blocked by child-specific rules, throws
  a cascade conflict error).
- Cross-workspace parent/child: `parent_slot.workspace` names the
  other workspace; runtime cross-loads parent state.

**Worked example — master-feeder fund cascade (CBU):**

```yaml
# In cbu_dag.yaml (post-R-3)
slots:
  - id: cbu
    stateless: false
    state_machine: { ... }
    parent_slot:
      workspace: cbu
      slot: cbu
      join:
        via: cbu_entity_relationships
        parent_fk: parent_cbu_id
        child_fk: child_cbu_id
    state_dependency:
      cascade_rules:
        - parent_state: SUSPENDED
          child_allowed_states: [SUSPENDED]
          cascade_on_parent_transition: true
          default_child_state_on_cascade: SUSPENDED
        - parent_state: OFFBOARDED
          child_allowed_states: [OFFBOARDED, ARCHIVED]
          cascade_on_parent_transition: true
          default_child_state_on_cascade: OFFBOARDED
      severity: error
```

**Worked example — deal schedule cascade to master deal:**

```yaml
# In deal_dag.yaml (post-R-5)
slots:
  - id: deal
    stateless: false
    state_machine: { ... }
    parent_slot:
      workspace: deal
      slot: deal
      join:
        via: deals
        parent_fk: parent_deal_id
        child_fk: deal_id
    state_dependency:
      cascade_rules:
        - parent_state: CANCELLED
          child_allowed_states: [CANCELLED]
          cascade_on_parent_transition: true
          default_child_state_on_cascade: CANCELLED
      severity: error
```

**Migration:** existing hierarchy relationships (cbu_entity_
relationships, deal_documents.role PRIMARY/ADDENDUM/SCHEDULE, KYC
case/workstream) are stateless in v1.2. Adding `parent_slot:` +
`state_dependency:` is purely additive.

### 2.4 V1.3-4 — SUSPENDED universal convention

Not a schema field but a **validator lint** and **convention**:

**Rule:** Any slot with `cardinality: root | mandatory` and
`expected_lifetime: long_lived` SHOULD include a `SUSPENDED` state
with bidirectional transitions to the preceding operational state.
Validator emits **warning** if absent.

**Schema addition (small):**

```yaml
slots:
  - id: <slot_id>
    stateless: false
    state_machine:
      id: <id>
      expected_lifetime: short_lived | long_lived | ephemeral
      # ... rest unchanged
```

**Lint behaviour:**
- `expected_lifetime: long_lived` + no SUSPENDED state in the
  state machine → warning `LongLivedSlotMissingSuspended`.
- `expected_lifetime: short_lived` or `ephemeral` → no check.
- Override: slot author can set
  `suspended_state_exempt: true` with a rationale comment
  (e.g. for lifecycles where SUSPENDED semantically doesn't apply).

**Rationale:** SUSPENDED is a universal commercial concern
(regulatory holds, disputes, client distress). Flagged missing in
Deal G-5, CBU G-2, implicit in KYC's BLOCKING red_flag. Having it
at the slot level as a convention prevents the omission pattern
from recurring in Tranche 3 authoring.

**Existing exemptions:**
- `kyc_case` is long-lived but arguably correctly without SUSPENDED
  (BLOCKED state + escalate verbs cover the same ground). May
  require exempt marker.
- `cbu_evidence`, `holding` sub-slot lifetime varies — not subject
  to this lint.

### 2.5 V1.3-5 — `dual_lifecycle:` on slots

Slots may declare linked-but-distinct lifecycles that share a
junction state. Captures the commercial-vs-operational and
discovery-vs-operational dualities.

**Schema:**

```yaml
slots:
  - id: <slot_id>
    stateless: false
    state_machine: { ... }               # primary lifecycle
    dual_lifecycle:
      - id: <secondary_lifecycle_id>
        description: "<human-readable>"
        owner: <owner_role>               # e.g. "sales+BAC" or "ops"
        junction_state_from_primary: <state_id>   # where primary hands off
        states: [...]
        transitions: [...]
        terminal_states: [...]
```

**Semantics:**
- The primary `state_machine:` block is unchanged — it's the
  slot's canonical state.
- `dual_lifecycle:` declares ONE OR MORE additional lifecycle
  chains that begin AT the primary's `junction_state_from_primary`
  state and run in parallel (conceptually — often different owners
  manage each).
- Validator treats the dual lifecycle as a sub-state-machine;
  runtime exposes both state-machine views.
- Ownership label drives reporting + permissions (not yet enforced
  in v1.3; documentation-level only).

**Worked example — Deal commercial-vs-operational:**

```yaml
# In deal_dag.yaml (post-R-5)
slots:
  - id: deal
    state_machine:
      id: deal_commercial_lifecycle
      owner: "sales+BAC"
      states: [PROSPECT, QUALIFYING, NEGOTIATING, BAC_APPROVAL, KYC_CLEARANCE, CONTRACTED]
      # ... transitions through CONTRACTED
      terminal_states: [CONTRACTED, LOST, REJECTED, WITHDRAWN, CANCELLED]
    dual_lifecycle:
      - id: deal_operational_lifecycle
        description: "Operational servicing of contracted deal — ops-owned"
        owner: "ops"
        junction_state_from_primary: CONTRACTED
        states: [ONBOARDING, ACTIVE, SUSPENDED, WINDING_DOWN, OFFBOARDED]
        transitions:
          - from: ONBOARDING, to: ACTIVE, via: deal.update-status
          - from: ACTIVE, to: SUSPENDED, via: deal.suspend
          - from: SUSPENDED, to: ACTIVE, via: deal.reinstate
          - from: (ACTIVE, SUSPENDED), to: WINDING_DOWN, via: deal.begin-winding-down
          - from: WINDING_DOWN, to: OFFBOARDED, via: deal.update-status
        terminal_states: [OFFBOARDED]
```

**Worked example — CBU discovery-vs-operational:**

```yaml
# In cbu_dag.yaml (post-R-3)
slots:
  - id: cbu
    state_machine:
      id: cbu_discovery_lifecycle
      owner: "compliance"
      states: [DISCOVERED, VALIDATION_PENDING, VALIDATED, UPDATE_PENDING_PROOF, VALIDATION_FAILED]
      terminal_states: [VALIDATED, VALIDATION_FAILED]
    dual_lifecycle:
      - id: cbu_operational_lifecycle
        description: "Operational trading / servicing of the CBU"
        owner: "trading+ops"
        junction_state_from_primary: VALIDATED
        states: [dormant, trade_permissioned, actively_trading, restricted, suspended, winding_down, offboarded, archived]
        transitions:
          - from: dormant, to: trade_permissioned, via: trading-profile.activate
          - from: trade_permissioned, to: actively_trading, via: "(backend: first trade executed)"
          # ... etc
        terminal_states: [offboarded, archived]
```

**Migration:** existing single state machines are unchanged.
Adding `dual_lifecycle:` is purely additive. Workspaces that don't
need it ignore the field.

### 2.6 V1.3-6 — `periodic_review_cadence:` + `validity_window:`

Slots and evidence-type reference data gain fields for regulatory
refresh cadence.

**Schema — on slots:**

```yaml
slots:
  - id: <slot_id>
    stateless: false
    state_machine: { ... }
    periodic_review_cadence:
      base_window: "<duration>"          # e.g. "P1Y" (ISO 8601)
      risk_tiered_overrides:
        - risk_tier: HIGH
          window: "P6M"
        - risk_tier: LOW
          window: "P2Y"
      review_scope: full | partial
      scheduler_hook: <layer_3_scheduler_id>  # optional — identifies the Layer-3 scheduler
```

**Schema — on evidence types (reference data):**

```yaml
evidence_types:
  - id: corporate_formation_docs
    validity_window: once                 # special value: no refresh
  - id: ubo_declaration
    validity_window: "P1Y"
  - id: sanctions_screening
    validity_window: "P14D"
  - id: financial_statements
    validity_window: "P1Y"
```

**Semantics:**
- `periodic_review_cadence:` on a slot means the slot's state
  machine has an automatic re-review trigger; scheduler (Layer 3)
  fires `review_scope` transitions when the `base_window` +
  `risk_tiered_overrides` compute.
- `validity_window:` on evidence types means derived
  `cbu_evidence.EXPIRED` transitions fire when
  `now() - verified_at > validity_window`.
- DAG validator checks consistency (cadence declared → slot has a
  re-review transition; validity_window declared → evidence-type
  state machine includes EXPIRED).
- Runtime: `periodic_review_cadence:` integrates with the existing
  Layer-3 scheduler; `validity_window:` drives EXPIRED transition
  computation.

**Worked example — KYC case periodic review:**

```yaml
# In kyc_dag.yaml (v1.3 update)
slots:
  - id: kyc_case
    state_machine: { ... }
    periodic_review_cadence:
      base_window: "P2Y"
      risk_tiered_overrides:
        - risk_tier: HIGH
          window: "P1Y"
        - risk_tier: LOW
          window: "P3Y"
      review_scope: full
```

**Worked example — CBU evidence types:**

```yaml
# In cbu_dag.yaml or separate evidence_types config
evidence_types:
  - id: corporate_formation_docs
    validity_window: once
  - id: bo_declaration
    validity_window: "P1Y"
  - id: financial_statements
    validity_window: "P1Y"
  - id: sanctions_screening
    validity_window: "P14D"
  - id: tax_residency_cert
    validity_window: "P1Y"
  - id: source_of_wealth_attestation
    validity_window: "P3Y"
```

**Migration:** no breaking change. Workspaces without cadence
requirements omit the field. Layer-3 scheduler wiring is a separate
engineering task outside v1.3 spec scope.

---

## 3. VALIDATOR EXTENSIONS

### 3.1 New validator checks

| Check | Triggers on | Severity | Module |
|---|---|---|---|
| `CrossWorkspaceConstraintUnresolved` | source_workspace/slot referenced in v1.3-1 block doesn't exist | error | dsl-core |
| `CrossWorkspaceConstraintSelfReference` | source_workspace == target_workspace (should use intra-DAG cross_slot_constraints) | error | dsl-core |
| `DerivedCrossWorkspaceStateUnresolved` | derivation clause references unknown workspace/slot/state | error | dsl-core |
| `DerivedCrossWorkspaceStateCycle` | derivation forms a cycle (A derives from B which derives from A) | error | dsl-core |
| `ParentSlotUnresolved` | parent_slot.workspace/slot doesn't exist | error | dsl-core |
| `StateDependencyInconsistent` | cascade_rules reference states not in parent's state machine | error | dsl-core |
| `LongLivedSlotMissingSuspended` | expected_lifetime: long_lived + no SUSPENDED state | warning | dsl-core |
| `DualLifecycleJunctionMissing` | dual_lifecycle.junction_state_from_primary doesn't exist in primary SM | error | dsl-core |
| `PeriodicReviewCadenceInconsistent` | cadence declared but no re-review transition in state machine | warning | dsl-core |
| `ValidityWindowWithoutExpiredState` | evidence type has validity_window but state machine has no EXPIRED | warning | dsl-core |

### 3.2 API additions

```rust
// dsl-core/src/config/cross_workspace.rs (new)
pub struct CrossWorkspaceConstraint { ... }
pub struct DerivedCrossWorkspaceState { ... }
pub fn validate_cross_workspace_constraints(
    all_dags: &HashMap<String, Dag>,
) -> Vec<ValidationIssue>;

// dsl-core/src/config/hierarchy.rs (new)
pub struct ParentSlotRef { ... }
pub struct StateDependency { ... }
pub fn validate_hierarchy(
    all_dags: &HashMap<String, Dag>,
) -> Vec<ValidationIssue>;

// dsl-core/src/config/validator.rs (extended)
pub fn validate_long_lived_suspended_convention(
    dag: &Dag,
) -> Vec<ValidationIssue>;
```

### 3.3 Runtime impact

**Constellation projection pipeline:**
- New step: after hydrating host slot, evaluate
  `derived_cross_workspace_state` blocks by cross-loading
  dependent workspaces' slot states (once per request; cached).
- Parent-child cascade: state transitions propagate via
  `state_dependency.cascade_rules` before the host transition
  commits.
- Cross-workspace constraint check: on any verb that triggers a
  transition matching a target_transition, pre-transition check
  cross-loads source state and validates.

**Verb surface:**
- Derived states appear in verb-surface filtering (if
  `exposure.visible_as: first_class_state`).
- Gated transitions (blocked by cross-workspace constraint)
  surface with `PruneReason::CrossWorkspaceGateNotMet`.

**Performance budget:**
- Per-request cross-loading budget: up to 5 adjacent workspaces;
  cached within request scope.
- Derivation evaluation: O(n) across derivation conditions;
  typically 4-8 per derived state.
- Expected overhead: < 5ms per request turn for typical CBU
  contexts.

---

## 4. CONVENTIONS (V1.3-7)

### 4.1 V1.3-7 — Commercial-commitment tier convention

**Rule:** When authoring three-axis declarations, verbs that
**emit external commercial commitment to a counterparty** default
to tier ≥ `requires_confirmation`.

**Definition of "external commercial commitment":**
- Sends a binding offer, rate, term, or obligation to a
  counterparty (client, fund admin, manco, etc.).
- Creates or materially modifies a commercial commitment owed by
  or to BNY.
- NOT satisfied by internal-only notifications or deliberation.

**Examples (from Tranche 2):**
- `deal.agree-rate-card` — bilateral commitment → req_ex_auth
- `billing.generate-invoice` — creates receivable → req_conf
- `billing.activate-profile` — starts revenue recognition → req_conf
- `deal.propose-rate-card` — sends proposal (not yet binding) → stays reviewable
- `deal.counter-rate-card` — sends counter (not binding) → stays reviewable

**Application:** tier-apply classifier (used in retrofit scripts
like `t2d3_retrofit.py`, `t2c3_retrofit.py`) gains a
`commercial_commitment` cluster. Pack and verb authors reference
the convention in tier-review documentation.

**Scope:** documentation + classifier update; no schema change.

---

## 5. FOUNDATIONAL REFRAMES (not new amendments, spec-level context)

Two findings from Tranche 2 re-shape how v1.3 interpretations
apply to specific workspaces. Not v1.3 amendments per se, but
critical context for implementers.

### 5.1 CBU-as-money-making-apparatus reframe

**CBU is Adam's coined construct for the operational trading unit
a commercial client has established on the market to make money.**
Not a generic client-entity.

- Parent (Allianz, BlackRock) = commercial client.
- CBU = what the client BUILT to deploy capital and earn.
- `cbu_category` enum represents DIFFERENT SHAPES of money-making
  apparatus (FUND_MANDATE / CORPORATE_GROUP / etc.) with
  different revenue mechanics and service-consumption patterns.

**Consequence for v1.3:** CBU DAG re-centring (R-3) places
operational lifecycle ON the CBU workspace (not deferred to
IM/Deal/KYC). V1.3-2 (derived_cross_workspace_state) is the
mechanism — CBU aggregates from adjacent workspaces without
duplicating state.

Memory: `/memory/user_cbu_construct.md`.

### 5.2 IM phase-axis re-anchor

**IM's `overall_lifecycle` centres on CBU trading-enablement, not
data lifecycle.** Data lifecycle becomes a sub-process.

New IM phases:
`dormant → configuring → trade_permissioned → actively_trading → restricted → suspended → winding_down → retired`

**Consequence for v1.3:** R-4 re-authors IM overall_lifecycle
with CBU-trading-enablement as the phase axis. The
`trading_activity` slot (new — tracks first_trade_at / last_trade_at /
dormancy) projects into CBU's `operationally_active` derived
state (V1.3-2).

Documented in `instrument-matrix-pilot-findings-2026-04-23.md` §10.

---

## 6. DoD IMPLICATIONS

### 6.1 New workspace authoring checklist (Tranche 3)

New workspaces authored after v1.3 lands MUST:

- [ ] Classify every slot's `expected_lifetime:` (short/long/ephemeral).
- [ ] Long-lived slots include SUSPENDED state (or exempt with rationale).
- [ ] Hierarchy relationships express `parent_slot:` + `state_dependency:`.
- [ ] Cross-workspace dependencies classified as Mode A/B/C per P17.
- [ ] Mode A gates → `cross_workspace_constraints:` block.
- [ ] Mode B aggregates → `derived_cross_workspace_state:` block.
- [ ] Mode C hierarchies → `parent_slot:` on child slot.
- [ ] Periodic review cadence declared for slots with regulatory
      refresh; validity_window declared for evidence types.
- [ ] Dual lifecycles declared where commercial vs operational
      or discovery vs operational phase ownership diverges.
- [ ] Verb tier-apply follows V1.3-7 commercial-commitment
      convention.

### 6.2 Primary workspace remediation (v1.2 → v1.3)

Existing Tranche 2 primary workspaces will be updated to v1.3 in
the following sequence (interleaved with Tranche 3 authoring per
D-5):

- **R-3 CBU DAG re-centring** — add operational-lifecycle states,
  wire `derived_cross_workspace_state` for `operationally_active`,
  add `parent_slot:` for master-feeder, apply dual_lifecycle for
  discovery-vs-operational. Schema migration deferred (D-2) —
  `chk_cbu_status` CHECK stays at 5 states; DAG leads.
- **R-4 IM phase-axis re-anchor** — re-author overall_lifecycle on
  CBU-trading-enablement; add `trading_activity` slot; project
  into CBU aggregate.
- **R-5 Deal targeted fixes** — add BAC_APPROVAL state,
  pricing-approval rate-card states, terminal-granularity split,
  promote `deal_sla` stateful, apply `dual_lifecycle:` for
  commercial-vs-operational, add `parent_slot:` for master-deal/
  schedule hierarchy. Schema migrations deferred to Tranche 3.
- **R-6 CBU small gaps** — manco state machine, investor
  REDEEMING, share-class lifecycle, holding RESTRICTED/PLEDGED/
  FROZEN, CBU-level CA events.

---

## 7. MIGRATION GUIDE (v1.2 → v1.3)

### 7.1 Breaking changes

**None.** v1.3 is strictly additive:
- New top-level DAG YAML sections: `cross_workspace_constraints:`,
  `derived_cross_workspace_state:`.
- New optional fields on slots: `parent_slot:`, `state_dependency:`,
  `dual_lifecycle:`, `periodic_review_cadence:`,
  `expected_lifetime:`, `suspended_state_exempt:`.
- New reference data: `evidence_types:` with `validity_window:`.

### 7.2 Migration script (optional, for clean-up)

```bash
# Move v1.3-CAND-2 cross-workspace constraints from
# cross_slot_constraints: to cross_workspace_constraints: block
cargo run -p xtask --release -- migrate-v1_3 \
    --path rust/config/sem_os_seeds/dag_taxonomies/ \
    --move-v1_3-candidates
```

Finds entries in v1.2 DAGs marked with `v1_3_candidate: true` and
offers to move them into the new v1.3 section.

### 7.3 Validator compatibility

v1.2 DAGs validate cleanly under the v1.3 validator
(no new errors; lint warnings emerge for long-lived slots missing
SUSPENDED state). Authors address lint warnings during normal
maintenance or as part of R-3/R-5.

### 7.4 Runtime backward compatibility

The constellation projection pipeline extends — pre-v1.3 DAGs
return the same projection they did before (no derived states
unless declared). v1.3-declaring DAGs gain the new derived-state
exposure and cross-workspace gate enforcement.

---

## 8. EXECUTION PLAN (post-approval)

Per reconciliation pass §5 (reference):

| Phase | Scope | Effort | Dependency |
|---|---|---|---|
| **R-1** | Draft this spec | 2-3 hr | — |
| R-2 | Validator + schema support for all 7 v1.3 candidates | 1-2 days eng | R-1 approved |
| R-3 | CBU DAG re-centring (operational-purpose-first) | 3 hr | R-2 |
| R-4 | IM phase-axis re-anchor | 1 hr | R-3 |
| R-5 | Deal targeted fixes | 3 hr | R-2 |
| R-6 | CBU targeted small gaps | 2 hr | R-3 |
| R-7 | Fixture hygiene cleanup | 30 min | — (interleave) |

Total serial critical path R-1 → R-2 → R-3 → R-4 ≈ 12-18 hours.
Interleave path (D-5): Tranche 3 authoring runs in parallel with
R-3/R-5/R-6 post-R-2. Tranche 3 new workspaces adopt v1.3
conventions from day one.

---

## 9. WHAT DOES NOT CHANGE

v1.3 leaves the following unchanged from v1.2:

- **P1-P16** — all existing principles, including P16 three-layer
  architecture.
- **Core DAG taxonomy schema** — `slots:`, `overall_lifecycle:`,
  `cross_slot_constraints:`, `product_module_gates:`, `prune_*:`
  sections retain their v1.2 semantics.
- **Three-axis declarations** — `state_effect` × `external_effects`
  × `consequence` model unchanged.
- **Validator public API** — existing `validate_*` functions
  unchanged; new functions added for v1.3 constructs.
- **Runtime verb surface** — existing CCIR / SessionVerbSurface
  pipeline unchanged; new hooks for cross-workspace derivation
  added at integration points.
- **KYC workspace** — no DAG changes required.

---

## 10. OPEN QUESTIONS FOR PEER REVIEW

Items where Adam's direction would disambiguate before R-2
implementation begins.

**OQ-1 — P17 mode orthogonality.** The three P17 modes (blocking /
aggregation / cascade) are presented as orthogonal. Is there a
scenario where a single relationship genuinely needs two modes
simultaneously? If yes, should v1.3 support composition of modes
on the same pair of slots, or should authors pick one?

**OQ-2 — Derived-state freshness semantics.** D-3 says on-the-fly
derivation with per-request caching. Is per-request cache scope
correct, or should derived state be recomputed on a session-level
cadence (e.g. every 30 seconds)? This matters for long-lived
chat sessions.

**OQ-3 — Dual-lifecycle ownership enforcement.** V1.3-5
introduces `owner:` labels on state machines. Should these be
documentation-only (v1.3) or enforceable via verb-authorization
rules (v1.4)? Deferring to v1.4 is simpler; enforcing in v1.3
tightens governance.

**OQ-4 — SUSPENDED universality strictness.** V1.3-4 flags
long-lived slots missing SUSPENDED as a warning. Should this be
an error for new slots authored post-v1.3? KYC case and evidence
are legitimate exemptions — should the exempt marker require a
governance reason code?

**OQ-5 — cbu_category as lifecycle gate.** The CBU reframe (§5.1)
calls for `cbu_category` to gate operational capabilities. Does
v1.3 formalise this via a new `category_gated:` field on slots
(analog to `requires_products:` from V1.2-3), or is the existing
V1.2-3 `requires_products:` field sufficient (treating category
as a "meta-product")?

**OQ-6 — Market-facing identity ownership.** CBU reframe §5 calls
for LEI/BIC/depo-account references to be first-class on CBU. In
current schema these are scattered across entity tables,
trading_profiles, SSIs. Does v1.3 introduce a dedicated
`market_identity:` slot type, or is this handled via a projection
pattern in R-3?

---

## 11. CLOSURE

**v1.3 DRAFT COMPLETE — awaiting peer review.**

Seven amendments codified:
- 4 P0 with 3-workspace evidence (CAND-2/5, CAND-10, CAND-11, CAND-13)
- 3 P1 with 2-workspace evidence (CAND-7/9, CAND-3/12, CAND-8)

One new principle (P17 cross-workspace state composition).

Two foundational reframes documented (CBU = money-making
apparatus; IM phase-axis = CBU-trading-enablement).

6 open questions flagged for peer review.

Migration path additive; no breaking changes from v1.2.

Execution plan interleaves primary-workspace remediation
(R-3/R-4/R-5/R-6) with Tranche 3 authoring post-R-2 validator
support landing.

**Next: Adam peer review; disposition of OQ-1..OQ-6 during
review; spec approval triggers R-2 engineering kickoff.**

**R-1 end.**
