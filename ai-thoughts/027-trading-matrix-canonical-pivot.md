# 027: Trading Matrix Canonical Pivot — Instrument Taxonomy as Generative Core

> **Status:** TODO
> **Priority:** HIGH — Architectural cleanup, stops verb drift
> **Effort:** ~62 hours across 7 phases
> **Depends on:** Existing trading-profile verbs, verb YAML schema

---

## The Core Insight

**Instrument taxonomy is the generative structure for onboarding.**

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│   INSTRUMENT TAXONOMY (what can this CBU trade?)                │
│   ├── Markets (exchanges, OTC venues)                           │
│   ├── Instrument classes (equity, fixed income, derivatives)    │
│   ├── Currency scope                                            │
│   └── Regulatory perimeter (MiFIR, EMIR, SFTR, etc.)           │
│                                                                 │
│                         ║                                       │
│                         ║ GENERATES                             │
│                         ▼                                       │
│                                                                 │
│   SERVICE CONFIGURATIONS (derived from instrument scope)        │
│   ├── Standing instructions (SSIs per instrument/market/ccy)    │
│   ├── Gateway routing (which pipes for which instruments)       │
│   ├── Settlement chains (CSD/subcustodian per market)          │
│   ├── Booking rules (which books for which instrument types)    │
│   ├── Pricing sources (per instrument class)                    │
│   ├── Tax treatment (per jurisdiction/instrument)               │
│   └── Corporate action preferences (per instrument class)       │
│                                                                 │
│                         ║                                       │
│                         ║ MATERIALIZES TO                       │
│                         ▼                                       │
│                                                                 │
│   OPERATIONAL TABLES (projection, not authored directly)        │
│   ├── custody.cbu_instruments                                   │
│   ├── custody.cbu_ssi_assignments                               │
│   ├── custody.cbu_gateway_routing                               │
│   ├── custody.cbu_settlement_chains                             │
│   ├── custody.cbu_booking_rules                                 │
│   └── custody.cbu_ca_preferences                                │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Why this matters:**
- Single source of truth (matrix doc) eliminates "which verb?" confusion
- Derived configs are *consistent* with instrument scope by construction
- Changes propagate correctly (add market → SSIs/gateways/settlement follow)
- Audit trail shows *intent*, not scattered operational writes

---

## Design Contract (Non-Negotiable)

### 1. Canonical Intent Layer
The **Trading Matrix / Trading Profile (CBU doc)** is canonical for:
- Instrument universe (markets, classes, currencies)
- Service configuration policy
- All CBU-specific instrument lifecycle configuration

### 2. Reference Catalog Layer
Global templates/taxonomies remain standalone:
- Instruction templates (message formats, field schemas)
- Gateway definitions (connectivity, protocols)
- Settlement location catalog
- CA event type taxonomy
- Pricing source catalog

### 3. Operational Projection Layer
Per-CBU operational tables are **projections**, written only by:
- `trading-profile.materialize` (or sub-steps)
- Internal projection verbs (not public authoring surface)

**Rule:** Any verb writing per-CBU operational tables MUST be:
- `tier: projection`
- Internal or only callable from materialize pipeline
- Never the primary authoring surface

---

## Verb Tier Definitions

```
┌──────────────┬──────────────────────────────────────────────────┐
│ TIER         │ DESCRIPTION                                      │
├──────────────┼──────────────────────────────────────────────────┤
│ reference    │ Global catalogs, templates, taxonomies           │
│              │ Scope: global                                    │
│              │ Examples: instruction-profile.define-template    │
│              │           corporate-action.define-event-type     │
│              │           settlement.define-location             │
├──────────────┼──────────────────────────────────────────────────┤
│ intent       │ Authoring surface for CBU business policy        │
│              │ Scope: cbu                                       │
│              │ Source of truth: matrix                          │
│              │ Examples: trading-profile.add-market             │
│              │           trading-profile.add-booking-rule       │
│              │           trading-profile.ca.set-election-policy │
├──────────────┼──────────────────────────────────────────────────┤
│ projection   │ Writes operational tables from matrix            │
│              │ Scope: cbu                                       │
│              │ Source of truth: operational                     │
│              │ Internal only (called by materialize)            │
│              │ Examples: _cbu-custody.write-booking-rule        │
│              │           _cbu-custody.write-ssi-assignment      │
├──────────────┼──────────────────────────────────────────────────┤
│ diagnostics  │ Read-only inspection of state                    │
│              │ Examples: cbu-custody.list-booking-rules         │
│              │           trading-profile.diff-vs-operational    │
│              │           trading-profile.export-operational-*   │
├──────────────┼──────────────────────────────────────────────────┤
│ composite    │ Multi-table orchestration verbs                  │
│              │ Must emit plan or structured audit               │
│              │ Examples: trading-profile.materialize            │
│              │           trading-profile.generate-plan          │
└──────────────┴──────────────────────────────────────────────────┘
```

---

## Phase 1: Tagging & Inventory (~8h)

### 1.1 Extend Verb YAML Schema

Add optional metadata fields (non-breaking):

```yaml
# In verb-definition-spec, add to verb schema:
metadata:
  tier: reference | intent | projection | diagnostics | composite
  source_of_truth: matrix | catalog | operational
  scope: global | cbu
  writes_operational: true | false
  noun: string  # e.g., trading_matrix, ssi, gateway, booking_rule, corporate_actions
  internal: true | false  # if true, not exposed to agent/user
```

**Files to modify:**
- `docs/verb-definition-spec.md` — document new fields
- `rust/src/dsl_v2/verb_loader.rs` — parse new fields (ignore if absent)
- `rust/src/dsl_v2/verb.rs` — add VerbMetadata struct

### 1.2 Tag Existing Verbs

Audit and tag all verbs in these domains:
- `trading-profile` → mostly `intent`
- `cbu-custody` → split: some `diagnostics`, some need `projection`
- `instruction-profile` → split: templates are `reference`, assignments are `projection`
- `trade-gateway` → split: definitions are `reference`, routing is `projection`
- `settlement-chain` → split: catalog is `reference`, cbu config is `projection`
- `pricing-config` → likely `projection`
- `tax-config` → likely `projection`
- `corporate-action` → split: event types are `reference`, preferences are `projection`

### 1.3 Generate Verb Inventory

Create CLI/xtask:

```bash
cargo xtask verb-inventory --output docs/verb-inventory.md
```

Output:
- `docs/verb-inventory.md` — human readable, grouped by domain/tier/noun
- `docs/verb-inventory.json` — machine readable for tooling

**Acceptance:**
- All verbs load successfully with new schema
- Inventory generated, reviewed for obvious mis-tiering

---

## Phase 2: Verb Linter (~12h)

### 2.1 Linter Implementation

Create linter that enforces tiering rules:

**File:** `rust/src/dsl_v2/verb_linter.rs`

```rust
pub struct VerbLintResult {
    pub verb: String,
    pub domain: String,
    pub violations: Vec<LintViolation>,
}

pub enum LintViolation {
    OperationalWriteNotProjection { table: String },
    DualPublicIntent { conflicting_verb: String },
    ProjectionNotInternal,
    CompositeWithoutPlan,
    NamingConventionViolation { expected: String, actual: String },
}

pub fn lint_all_verbs(registry: &VerbRegistry) -> Vec<VerbLintResult>;
```

### 2.2 Linter Rules

1. **Operational write restriction**
   - Verbs writing `custody.cbu_*` tables must be `tier: projection`
   - Projection verbs must be `internal: true`

2. **No dual public intent**
   - If `trading-profile.add-X` exists, `cbu-custody.add-X` must be projection/internal
   - Matrix is the only public intent surface for CBU instrument config

3. **Naming conventions**
   - `ensure-*` = idempotent upsert
   - `create-*` = strict create (deprecate in favor of ensure)
   - `list-*` = enumeration
   - `get-*` = by natural key
   - `read-*` = by PK
   - Prefer `end/close/supersede` over `delete`

4. **Composite safety**
   - Verbs crossing >3 tables must be `tier: composite`
   - Composite verbs should support plan generation

### 2.3 Integration Points

- **CI:** Run in warn mode initially, fail mode after cleanup
- **REPL:** `:lint` command for dev feedback
- **MCP:** `lint_verbs` tool for agent inspection

**Acceptance:**
- Linter identifies top offenders
- CI runs lint (warn mode)
- `:lint` works in REPL

---

## Phase 3: Verb Cleanup — Delete & Replace (~12h)

**No deprecation. No aliases. Just delete.**

We're not in production. Rip out the redundant verbs, replace with canonical ones, update all references.

### 3.1 Delete List

These verbs get deleted entirely:

| Delete | Replaced By |
|--------|-------------|
| `cbu-custody.add-universe` | `trading-profile.add-market` |
| `cbu-custody.remove-universe` | `trading-profile.remove-market` |
| `cbu-custody.add-booking-rule` | `trading-profile.add-booking-rule` |
| `cbu-custody.ensure-booking-rule` | `trading-profile.add-booking-rule` |
| `cbu-custody.remove-booking-rule` | `trading-profile.remove-booking-rule` |
| `cbu-custody.update-rule-priority` | `trading-profile.update-booking-priority` |
| `cbu-custody.create-ssi` | `trading-profile.add-standing-instruction` |
| `cbu-custody.ensure-ssi` | `trading-profile.add-standing-instruction` |
| `cbu-custody.activate-ssi` | `trading-profile.activate-standing-instruction` |
| `cbu-custody.suspend-ssi` | `trading-profile.suspend-standing-instruction` |
| `cbu-custody.setup-ssi` | `trading-profile.add-standing-instruction` |
| `instruction-profile.assign-template` | `trading-profile.link-instruction-template` |
| `instruction-profile.remove-assignment` | `trading-profile.unlink-instruction-template` |
| `instruction-profile.add-field-override` | `trading-profile.set-instruction-override` |
| `instruction-profile.remove-field-override` | `trading-profile.clear-instruction-override` |
| `trade-gateway.enable` | `trading-profile.enable-gateway` |
| `trade-gateway.activate` | `trading-profile.activate-gateway` |
| `trade-gateway.suspend` | `trading-profile.suspend-gateway` |
| `trade-gateway.add-routing-rule` | `trading-profile.add-gateway-routing` |
| `trade-gateway.set-fallback` | `trading-profile.set-gateway-fallback` |
| `corporate-action.add-preference` | `trading-profile.ca.set-default-option` |
| `corporate-action.remove-preference` | `trading-profile.ca.clear-default-option` |
| `corporate-action.set-window` | `trading-profile.ca.set-cutoff-rules` |

### 3.2 Keep as Diagnostics (Read-Only)

These stay but are clearly `tier: diagnostics`:

```yaml
# Read operational state (projection tables)
cbu-custody.list-booking-rules      # tier: diagnostics
cbu-custody.list-ssis                # tier: diagnostics  
cbu-custody.get-universe             # tier: diagnostics
instruction-profile.list-assignments # tier: diagnostics
trade-gateway.list-routing           # tier: diagnostics
corporate-action.list-preferences    # tier: diagnostics
```

### 3.3 Update Cascaded References

**Batch scripts:**
```bash
# Find all references to deleted verbs
grep -r "cbu-custody.add-universe" rust/config/
grep -r "cbu-custody.ensure-ssi" rust/config/
grep -r "instruction-profile.assign" rust/config/
# ... etc
```

**Composite verbs (verbs that call other verbs):**
```bash
# Find verb YAMLs with 'calls:' or 'sequence:' sections
grep -l "calls:\|sequence:" rust/config/verbs/*.yaml
```

**Macro definitions:**
```bash
# Find macro files
find rust/config -name "*macro*" -o -name "*batch*"
```

### 3.4 Implementation Steps

1. **Inventory references** (~2h)
   - Script to find all usages of verbs in delete list
   - Output: `docs/verb-cleanup-references.md`

2. **Create missing canonical verbs** (~4h)
   - Add any `trading-profile.*` verbs that don't exist yet
   - Wire handlers to existing ops (may just rename)

3. **Update composite/macro references** (~2h)
   - Edit batch scripts
   - Edit composite verb definitions
   - Edit macro files

4. **Delete old verb YAMLs** (~1h)
   - Remove from `rust/config/verbs/cbu-custody.yaml`
   - Remove from `rust/config/verbs/instruction-profile.yaml`
   - Remove from `rust/config/verbs/trade-gateway.yaml`
   - Remove from `rust/config/verbs/corporate-action.yaml`

5. **Delete orphaned handlers** (~2h)
   - Remove handler code that's no longer referenced
   - Or rename handlers to match new verb names

6. **Run tests, fix breakage** (~1h)
   - `cargo test`
   - Fix any test fixtures using old verbs

**Acceptance:**
- Old verbs gone (compile error if referenced)
- Canonical verbs work
- All tests pass
- No dangling handlers

---

## Phase 4: Materialize Plan/Execute (~12h)

### 4.1 Plan Generation

```yaml
trading-profile.generate-materialization-plan:
  description: Generate diff between matrix intent and operational state
  args:
    - name: cbu-id
      type: uuid
      required: false  # if omitted, use session scope
    - name: nouns
      type: string_list
      required: false  # if omitted, all nouns
      description: Selective materialization (ssi, gateway, booking, ca, etc.)
  returns:
    type: record
    fields:
      - plan_id: uuid
      - cbu_count: integer
      - operations: array of MaterializationOp
      - warnings: array of string
```

**MaterializationOp structure:**
```rust
pub struct MaterializationOp {
    pub table: String,
    pub operation: OpType,  // Insert, Update, Delete
    pub key: serde_json::Value,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
    pub reason: String,  // "New market added", "SSI updated", etc.
}
```

### 4.2 Plan Execution

```yaml
trading-profile.apply-materialization-plan:
  description: Execute a generated plan
  args:
    - name: plan-id
      type: uuid
      required: true
  returns:
    type: record
    fields:
      - applied_count: integer
      - failed_count: integer
      - audit_id: uuid
```

### 4.3 Convenience Verb

```yaml
trading-profile.materialize:
  description: Generate and apply plan in one step
  args:
    - name: cbu-id
      type: uuid
      required: false
    - name: nouns
      type: string_list
      required: false
    - name: dry-run
      type: boolean
      default: false
      description: If true, generate plan but don't apply
  returns:
    type: record
```

### 4.4 Diff Inspection

```yaml
trading-profile.diff-vs-operational:
  description: Show what would change without generating formal plan
  tier: diagnostics
```

**Acceptance:**
- Plan generation works for all nouns
- Plan shows clear before/after
- Apply is transactional
- Idempotent (re-run produces no changes)

---

## Phase 5: Corporate Actions Integration (~8h)

### 5.1 Matrix CA Section Schema

Add to trading matrix document structure:

```rust
pub struct TradingMatrixCorporateActions {
    pub enabled_event_types: Vec<CaEventType>,
    pub notification_policy: NotificationPolicy,
    pub election_policy: ElectionPolicy,
    pub default_options: HashMap<CaEventType, DefaultOption>,
    pub cutoff_rules: Vec<CutoffRule>,
    pub proceeds_ssi_mapping: HashMap<ProceedsType, SsiReference>,
}
```

### 5.2 Matrix CA Verbs

```yaml
trading-profile.ca.enable-event-types:
  tier: intent
  args:
    - name: event-types
      type: string_list
      description: CA event types to enable (dividend, rights, merger, etc.)

trading-profile.ca.set-notification-policy:
  tier: intent
  args:
    - name: channels
      type: string_list
    - name: sla-hours
      type: integer

trading-profile.ca.set-election-policy:
  tier: intent
  args:
    - name: elector
      type: string  # im | admin | client
    - name: evidence-required
      type: boolean

trading-profile.ca.set-default-option:
  tier: intent
  args:
    - name: event-type
      type: string
    - name: default-option
      type: string

trading-profile.ca.link-proceeds-ssi:
  tier: intent
  args:
    - name: proceeds-type
      type: string  # cash | stock
    - name: ssi-id
      type: uuid
```

### 5.3 CA Reference Catalog

Keep separate (global, not CBU-specific):

```yaml
corporate-action.define-event-type:
  tier: reference
  scope: global

corporate-action.list-event-types:
  tier: reference
  scope: global
```

### 5.4 CA Materialization

During `trading-profile.materialize`:
- Write `custody.cbu_ca_preferences` from matrix CA section
- Validate referenced SSIs exist
- Emit warnings for missing references

**Acceptance:**
- CA policy authored in matrix
- CA references SSIs (doesn't duplicate them)
- Materialize writes CA operational tables

---

## Phase 6: Session Integration (~4h)

### 6.1 Session-Aware Materialization

When session scope is set (Galaxy/Book/CBU set):

```
trading-profile.materialize scope=session
```

Operates on all CBUs in current session scope.

### 6.2 Batch Plan Generation

```
trading-profile.generate-materialization-plan scope=session
```

Returns aggregated plan across all scoped CBUs.

### 6.3 Progress Reporting

For large scope (100+ CBUs), emit progress:
- Via REPL progress bar
- Via MCP streaming updates

**Acceptance:**
- Materialize respects session scope
- Batch operations work correctly
- Progress visible for large batches

---

## Phase 7: Cleanup & Documentation (~6h)

### 7.1 Archive Superseded Docs

- Archive `docs/TODO_TRADING_MATRIX.md` (3,574 lines) → `docs/archive/`
- This TODO (027) becomes the implementation reference

### 7.2 Update CLAUDE.md

Add to mandatory reading table:
- `ai-thoughts/027-trading-matrix-canonical-pivot.md`

Add trigger phrases:
- "trading matrix", "instrument taxonomy", "materialize", "booking rule", "SSI"

### 7.3 Create Annex: Trading Matrix Architecture

Create `docs/trading-matrix-architecture.md`:
- Canonical layer diagram
- Verb tier definitions
- Materialization flow
- Quick reference for which verbs to use

### 7.4 Update Agent Lexicon

Ensure agent:
- Suggests canonical `trading-profile.*` verbs
- Warns if user mentions deprecated verbs
- Understands materialize workflow

**Acceptance:**
- CLAUDE.md updated
- Annex doc created
- Agent uses correct verbs

---

## Testing Requirements

### Golden Scenario Tests

```rust
#[test]
fn test_matrix_to_operational_roundtrip() {
    // 1. Create matrix with full config
    let matrix = create_test_matrix()
        .with_markets(["NYSE", "LSE", "XETRA"])
        .with_instrument_classes(["equity", "fixed_income"])
        .with_booking_rules([...])
        .with_ssis([...])
        .with_ca_policy([...]);
    
    // 2. Generate plan
    let plan = trading_profile_generate_plan(&matrix);
    assert!(!plan.operations.is_empty());
    
    // 3. Apply plan
    let result = trading_profile_apply_plan(&plan);
    assert!(result.failed_count == 0);
    
    // 4. Verify operational tables
    assert_operational_matches_matrix(&matrix);
    
    // 5. Re-run materialize (idempotent)
    let plan2 = trading_profile_generate_plan(&matrix);
    assert!(plan2.operations.is_empty()); // No changes
}
```

### Deprecation Tests

```rust
#[test]
fn test_deprecated_verb_warns_and_redirects() {
    let result = execute_dsl("cbu-custody.add-booking-rule ...");
    assert!(result.warnings.contains("deprecated"));
    assert!(result.success); // Still works
}
```

### Linter Tests

```rust
#[test]
fn test_linter_catches_projection_not_internal() {
    let violations = lint_verb("cbu-custody.write-booking-rule");
    assert!(violations.contains(ProjectionNotInternal));
}
```

---

## Acceptance Criteria Summary

| # | Criterion |
|---|-----------|
| 1 | Matrix is the only public authoring surface for CBU instrument lifecycle intent |
| 2 | Operational tables written only through materialize pipeline |
| 3 | Verb linter prevents reintroducing dual sources of truth |
| 4 | Deprecated verbs warn/redirect correctly |
| 5 | Plan/Execute provides deterministic, auditable materialization |
| 6 | Corporate actions authored in matrix, reference SSIs |
| 7 | Session-aware batch operations work |
| 8 | Regression tests prove idempotency |
| 9 | CLAUDE.md and annex updated |

---

## Implementation Order

```
Phase 1  (Tagging)        ████████░░░░░░░░░░░░  ~8h   ← START HERE
Phase 2  (Linter)         ░░░░░░░░████████████  ~12h
Phase 3  (Delete/Replace) ░░░░░░░░░░░░████████  ~12h  ← Rip out old, wire new
Phase 4  (Plan/Exec)      ░░░░░░░░░░░░░░░░████  ~12h
Phase 5  (CA)             ░░░░░░░░░░░░░░░░░░██  ~8h   ← Can parallel with 4
Phase 6  (Session)        ░░░░░░░░░░░░░░░░░░░█  ~4h
Phase 7  (Docs)           ░░░░░░░░░░░░░░░░░░░░  ~6h
                                            ─────
                                            ~62h
```

**Simplified:** No deprecation/alias machinery. Just delete old verbs, replace with canonical, fix references. Compiler errors guide cleanup.

---

## Related Documents

- `docs/TODO_TRADING_MATRIX.md` — superseded by this (archive)
- `docs/strategy-patterns.md` — data model philosophy
- `docs/verb-definition-spec.md` — verb YAML structure
- `rust/config/verbs/trading-profile.yaml` — current trading profile verbs
- `ai-thoughts/016-capital-structure-ownership-model.md` — related ownership model
