# Annex: Macro System

> **Last reviewed:** 2026-03-29
> **Total macros:** 93 (across 20 YAML files)
> **Composite macros:** 3 (use `invoke-macro` nesting)
> **Operator domains:** 16 distinct domains
> **Lint rules:** 30+ (MACRO000–MACRO089)

---

## Overview

Macros are **operator-facing compound verbs** — they wrap one or more DSL primitives into
business-friendly vocabulary. An operator says "Set up a Luxembourg UCITS SICAV"; the macro
expands that into 13 primitive verb calls (create CBU, apply doc bundle, assign roles, create
trading profile).

**Design invariants:**
- UI labels never expose implementation terms (`cbu`, `entity_ref`, `trading-profile`)
- Operators work with domain nouns: Structure, Party, Case, Mandate
- Enum keys differ from internal tokens: UI `pe` → internal `private-equity`
- Expansions always use `${arg.X.internal}` for enum arguments
- All YAML keys use **kebab-case** (`expands-to`, `mode-tags`, `operator-domain`)

**Pipeline position:** Macros resolve at Tier -2B (MacroIndex, score 0.96) in the intent
pipeline, after Tier -2A (ScenarioIndex, score 0.97) and before Tier 0+ (verb embeddings).

---

## Key Files

| Purpose | Path |
|---------|------|
| Macro YAML definitions | `rust/config/verb_schemas/macros/*.yaml` (20 files) |
| Search overrides | `rust/config/macro_search_overrides.yaml` |
| Schema types | `rust/src/dsl_v2/macros/schema.rs` |
| Registry loader | `rust/src/dsl_v2/macros/registry.rs` |
| Expander | `rust/src/dsl_v2/macros/expander.rs` |
| MacroIndex (search) | `rust/src/mcp/macro_index.rs` |
| Lint rules | `rust/src/lint/macro_lint.rs` |
| Verb classifier | `rust/src/runbook/verb_classifier.rs` |
| Runbook compiler | `rust/src/runbook/compiler.rs` |
| Sequence validator | `rust/src/mcp/sequence_validator.rs` |
| Compound signals | `rust/src/mcp/compound_intent.rs` |

---

## MacroSchema Type

Defined in `rust/src/dsl_v2/macros/schema.rs` with `#[serde(rename_all = "kebab-case")]`:

```rust
pub struct MacroSchema {
    pub kind: MacroKind,                    // "macro"
    pub tier: Option<MacroTier>,            // Primitive | Composite | Template
    pub aliases: Vec<String>,               // Alternative search names
    pub ui: MacroUi,                        // label, description, target_label
    pub routing: MacroRouting,              // mode_tags, operator_domain
    pub target: MacroTarget,                // operates_on, produces, allowed_structure_types
    pub args: MacroArgs,                    // required + optional arguments
    pub prereqs: Vec<MacroPrereq>,          // DAG state requirements
    pub expands_to: Vec<MacroExpansionStep>,// Verb calls, invoke-macro, when, foreach
    pub sets_state: Vec<SetState>,          // State flags set after execution
    pub unlocks: Vec<String>,              // Verbs/macros unlocked post-execution
}
```

### Expansion Steps

```rust
pub enum MacroExpansionStep {
    VerbCall(VerbCallStep),        // { verb: "cbu.create", args: {...}, as: "@cbu" }
    InvokeMacro(InvokeMacroStep), // { invoke-macro: "struct.ie.aif.icav", args: {...}, import-symbols: ["@cbu"] }
    When(WhenStep),                // Conditional: when → then/else
    ForEach(ForEachStep),          // Loop: foreach var in list do [steps]
}
```

### Prerequisites

```rust
pub enum MacroPrereq {
    StateExists { key: String },             // state flag must be true
    VerbCompleted { verb: String },          // verb must have executed
    AnyOf { conditions: Vec<MacroPrereq> },  // any child satisfied
    FactExists { predicate: String },        // fact predicate
}
```

### Variable Substitution

| Pattern | Source |
|---------|--------|
| `${arg.X}` | Argument value |
| `${arg.X.internal}` | Enum internal token |
| `${scope.client_id}` | Session scope |
| `${session.current_case}` | Session state |
| `@symbol` | Symbol binding from prior step (`as: "@cbu"`) |

---

## Macro Inventory by Domain

### Structure Macros (24 macros)

**Generic** (`structure.yaml`): `structure.setup`, `.assign-role`, `.list`, `.select`, `.roles`

**Luxembourg** (`struct-lux.yaml`):
- `struct.lux.ucits.sicav` — 13 steps (CBU + doc bundle + ManCo + depositary + optional roles + trading profile)
- `struct.lux.aif.raif` — 13 steps (similar pattern, AIF-specific roles)
- `struct.lux.pe.scsp` — 13 steps (GP-led, PE-specific)

**Ireland** (`struct-ie.yaml`):
- `struct.ie.ucits.icav` — 13 steps
- `struct.ie.aif.icav` — 12 steps (QIAIF/RIAIF subtypes)
- `struct.ie.hedge.icav` — **composite**: invokes `struct.ie.aif.icav` + adds hedge roles

**UK** (`struct-uk.yaml`):
- `struct.uk.authorised.oeic` — 16 steps (ACD + depositary + optional roles)
- `struct.uk.authorised.aut` — 14 steps (manager + trustee pattern)
- `struct.uk.authorised.acs` — 14 steps (operator + depositary)
- `struct.uk.authorised.ltaf` — **composite**: invokes `struct.uk.authorised.oeic` + LTAF specifics
- `struct.uk.manager.llp` — 11 steps (designated members + compliance officers)
- `struct.uk.private-equity.lp` — 14 steps (GP-led partnership)

**US** (`struct-us.yaml`):
- `struct.us.40act.open-end` — 15 steps (investment adviser + custodian + optional roles)
- `struct.us.40act.closed-end` — 15 steps (with listing exchange)
- `struct.us.etf.40act` — 17 steps (authorized participant + market maker)
- `struct.us.private-fund.delaware-lp` — 18 steps (GP + IM + exemption types)

**Cross-Border** (`struct-cross-border.yaml`):
- `struct.hedge.cross-border` — 19 steps (master fund + US feeder + IE feeder + `cbu.link-structure`)
- `struct.pe.cross-border` — **composite**: invokes `struct.lux.pe.scsp` + US parallel + aggregator

**Product Suites** (in `governance.yaml`):
- `structure.product-suite-custody-fa-ta` — 3 steps (Custody + FA + TA)
- `structure.product-suite-full` — 5 steps (Custody + FA + TA + Collateral + Middle Office)
- `structure.remove-all-products` — 5 steps (clean slate)

### KYC Case Macros (9 macros, `case.yaml`)

`case.open` → `.add-party` → `.solicit-document` → `.submit` → `.approve` / `.reject` / `.request-info`
Plus: `case.list`, `case.select`

### KYC Workstream Macros (3 macros, `kyc-workstream.yaml`)

`kyc-workstream.add` → `.update-status` → `.close`

### KYC Workflow Macros (3 macros, `kyc-workflow.yaml`)

- `kyc.full-review` — 4 steps (PEP + sanctions + adverse media + document solicitation)
- `kyc.collect-documents` — 1 step (document.solicit-set)
- `kyc.check-readiness` — 2 steps (missing docs + unsatisfied requirements)

### Screening Macros — Two Families

**Party-level** (`screening.yaml`, 4 macros): Ad-hoc screening outside workstream context
- `screening.full` — 3 steps (PEP + sanctions + adverse media)
- `screening.pep-check`, `screening.sanctions-check`, `screening.media-check` — 1 step each

**Workstream-level** (`screening-ops.yaml`, 3 macros): KYC case context
- `screening-ops.run` — 1 step (unified `screening.run` verb with type selector)
- `screening-ops.review-hit` — 1 step (adjudicate a screening hit)
- `screening-ops.bulk-refresh` — 1 step (refresh all case screenings)

### UBO Macros (8 macros, `ubo.yaml`)

`ubo.discover` → `.allege` → `.verify` → `.promote` → `.approve` / `.reject` / `.expire`
Plus: `ubo.trace-chains`

### Evidence Macros (5 macros, `evidence.yaml`)

`evidence.require` → `.link` → `.verify` / `.reject` / `.waive`

### Document Request Macros (5 macros, `doc-request.yaml`)

`doc-request.create` → `.send` → `.respond` → `.verify` / `.reject`

### Red Flag Macros (3 macros, `red-flag.yaml`)

`red-flag.raise` → `.resolve` / `.escalate`

### Tollgate Macros (2 macros, `tollgate.yaml`)

`tollgate.evaluate` → `.override`

### Party Macros (9 macros, `party.yaml`)

`party.add` / `.add-person` / `.add-company` + `.search` / `.details` / `.update` / `.list` / `.assign-identifier` / `.set-address`

### Mandate Macros (7 macros, `mandate.yaml`)

`mandate.create` → `.add-product` / `.set-instruments` / `.set-markets` + `.list` / `.select` / `.details`

### Governance Macros (4 macros, `governance.yaml`)

- `governance.bootstrap-attribute-registry` — 3 steps (bridge + sync + check gaps)
- `governance.define-service-dictionary` — 4 steps (check gaps + sync + rollup + gaps)
- `governance.full-publish-pipeline` — 5 steps (precheck + validate + dry-run + plan + publish)
- `governance.reconcile-registry` — 3 steps (bridge + sync + recompute stale)

### Attribute Macros (2 macros, `attribute.yaml`)

`attribute.seed-domain`, `attribute.seed-derived` — governance workspace only

---

## Pack → Macro Mapping

| Pack | Workspaces | Macro Families |
|------|-----------|----------------|
| **book-setup** | cbu, instrument_matrix, on_boarding | struct.*, structure.*, mandate.*, party.*, product-suite-* |
| **cbu-maintenance** | cbu | struct.*, product-suite-* |
| **kyc-case** | kyc, on_boarding | case.*, screening.*, screening-ops.*, kyc.*, kyc-workstream.*, ubo.*, evidence.*, doc-request.*, red-flag.*, tollgate.*, party.*, structure.* |
| **semos-maintenance** | sem_os_maintenance | attribute.seed-* |
| **deal-lifecycle** | deal, on_boarding | (no macros) |
| **product-service-taxonomy** | product_maintenance | (no macros) |
| **onboarding-request** | on_boarding | (no macros) |
| **session-bootstrap** | (internal) | (no macros) |

**Workspace isolation rule:** A macro appears in a pack only if its mode-tags are compatible
with that pack's workspace. KYC macros are not in `book-setup` or `cbu-maintenance`. Governance
macros are only in `semos-maintenance`.

---

## MacroIndex — Search & Scoring

### Scoring Table (deterministic, no LLM)

| Signal | Score | Notes |
|--------|-------|-------|
| Exact FQN match | +10 | Query exactly matches `screening.full` |
| Exact label match | +8 | Query matches UI label |
| Alias match | +6 | Query matches curated alias |
| Label substring | +5 | Query contains full label text |
| Label word coverage ≥75% | +4 | Most label words appear in query |
| Jurisdiction hint | +3 | Explicit jurisdiction match |
| Jurisdiction in query | +3 | Query contains "luxembourg", "ireland", etc. |
| Mode match | +2 | Active mode in macro's mode_tags |
| Noun overlap | +2 | Query tokens overlap noun_tokens |
| Structure type match | +2 | Query mentions macro's structure type |
| Target kind match | +2 | Query mentions what macro produces |

### Hard Gates

- **M1**: Mode compatibility — if active mode specified and macro has mode_tags, must overlap. Penalty: -999.
- **M2**: Minimum score ≥ 6
- **M3**: Disambiguation band ≤ 2 → return up to 5 ambiguous candidates

### Search Overrides

`rust/config/macro_search_overrides.yaml` provides curated aliases merged at startup:
```yaml
struct.lux.ucits.sicav:
  aliases:
    - "lux ucits"
    - "luxembourg sicav"
    - "set up a lux ucits"
```

---

## Expansion Engine

### Fixpoint Expansion

Recursive expansion for nested `invoke-macro` steps:

```
expand_macro_fixpoint(fqn, args, session, registry, limits) → FixpointExpansionOutput
```

**Limits** (default): `max_depth: 8`, `max_steps: 500`

**Invariants:**
- **INV-4**: Per-path cycle detection (same macro can appear in separate non-cyclic branches)
- **INV-12**: `ExpansionLimits` snapshot in every audit for replay verification

**Output:**
```rust
pub struct FixpointExpansionOutput {
    pub statements: Vec<String>,           // Fully expanded DSL (no invoke-macro left)
    pub audits: Vec<MacroExpansionAudit>,   // Audit trail per expansion
    pub limits: ExpansionLimits,            // Limits in effect
    pub total_steps: usize,                // Total steps across all recursions
}
```

### Runbook Integration

Pipeline: `VerbClassifier` → `expand_macro_fixpoint()` → runbook step compilation

```rust
pub enum VerbClassification<'a> {
    Primitive { fqn: String },
    Macro { fqn: String, schema: &'a MacroSchema },
    Unknown { name: String },
}
// Lookup order: Macro registry first (shadows primitives), then verb registry
```

### Sequence Validation

`SequenceValidator` validates macro sequences from ScenarioIndex before execution:

```rust
pub fn validate_macro_sequence(
    macros: &[String],
    registry: &MacroRegistry,
    current_state_flags: &HashSet<String>,
    completed_verbs: &HashSet<String>,
) -> SequenceValidationResult
// Simulates execution: checks prereqs at each step, applies sets_state
```

---

## Lint Rules

Two-pass validation: schema-only (Pass 1) + cross-registry (Pass 2).

| Range | Category | Key Rules |
|-------|----------|-----------|
| MACRO010-019 | Structure | `kind` must be "macro"; UI section required; forbidden tokens in labels |
| MACRO020-029 | Routing | `mode-tags` required and non-empty |
| MACRO030-039 | Target | `operates-on` required; `produces` validated |
| MACRO040-049 | Args | `type` + `ui-label` required; no `entity_ref` type; `kinds` only in `internal` |
| MACRO050-059 | Prereqs | prereqs required (can be empty list) |
| MACRO060-069 | Expansion | `expands-to` required + non-empty; variable references validated; no raw s-expressions |
| MACRO070-079 | Cross-registry | `unlocks` references must exist; `invoke-macro` targets must exist |
| MACRO080-089 | UX warnings | Missing `autofill-from` or `picker` on ref-typed args |
| PACK001 | Workspace bleed | Macro mode-tags must be compatible with every workspace in the pack |

Run: `cargo x verbs lint-macros`

### PACK001: Workspace-Macro Bleed Detection

For each macro in a pack's `allowed_verbs`, PACK001 checks that the macro's `mode-tags` are
compatible with **every** workspace the pack serves. If a workspace accepts none of the macro's
mode-tags, the macro is exposed to operators who have no context for it.

**Workspace-to-mode-tag compatibility table:**

| Workspace | Accepted mode-tags |
|-----------|--------------------|
| `cbu` | structure, trading, onboarding |
| `kyc` | kyc, onboarding |
| `deal` | deal, onboarding |
| `on_boarding` | (all) — umbrella workspace |
| `product_maintenance` | product, trading |
| `instrument_matrix` | trading, structure |
| `sem_os_maintenance` | stewardship, governance |

**Example violation:**
```
PACK001 error: macro 'screening.full' [mode-tags: kyc, onboarding] in pack
  'book-setup' is exposed to workspace 'instrument_matrix' which accepts
  none of those tags
```

**Fix:** Either remove the macro from the pack, or add the appropriate mode-tag to the macro
if it legitimately belongs in that workspace context.

**Fail-closed on unknown workspaces:** If a new `WorkspaceKind` is added to the Rust enum and
used in a pack but NOT added to the compatibility table, PACK001 rejects **every** macro in
that pack for the unknown workspace. This forces the table to be updated before any macros
can ship in the new workspace. See [Adding a Workspace](#adding-a-workspace) below.

---

## Adding a Macro

1. **Create YAML** in `rust/config/verb_schemas/macros/<domain>.yaml`
2. Follow the schema: `kind: macro`, `ui`, `routing`, `target`, `args`, `prereqs`, `expands-to`, `sets-state`, `unlocks`
3. All YAML keys must be **kebab-case** (`expands-to`, not `expands_to`)
4. **Add to pack** — add the macro FQN to the appropriate pack's `allowed_verbs`
5. **Add search overrides** — if the macro needs curated aliases, add to `macro_search_overrides.yaml`
6. **Add scenario** — if the macro represents a compound journey, add to `scenario_index.yaml`
7. **Lint**: `cargo x verbs lint`
8. **Compile + embed**: `cargo x verbs compile && DATABASE_URL="postgresql:///data_designer" cargo run --release -p ob-semantic-matcher --bin populate_embeddings`

### YAML Template

```yaml
my-domain.my-macro:
  kind: macro

  ui:
    label: "My Action"
    description: "Does something useful"
    target-label: "Result"

  routing:
    mode-tags: [onboarding, kyc]
    operator-domain: my-domain

  target:
    operates-on: structure-ref
    produces: null

  args:
    style: keyworded
    required:
      name:
        type: str
        ui-label: "Name"
    optional: {}

  prereqs:
    - type: state_exists
      key: structure.exists

  expands-to:
    - verb: cbu.create
      args:
        name: "${arg.name}"
      as: "@cbu"

  sets-state:
    - key: my-domain.created
      value: true

  unlocks:
    - my-domain.next-step
```

---

## Adding a Workspace

When a new `WorkspaceKind` is added (e.g., `Compliance`), the macro system requires updates
at multiple layers. PACK001 enforces this — if any step is skipped, the lint fails.

### Checklist

1. **Add the enum variant** to `WorkspaceKind` in `rust/src/repl/types_v2.rs`
   - Add `label()`, `description()`, registry metadata (constellation family, subject kind)
   - Serde serialization uses `snake_case` (e.g., `Compliance` → `"compliance"`)

2. **Update PACK001 compatibility table** in `rust/xtask/src/main.rs`
   - Add a row to `workspace_accepts_any_mode_tag()`:
     ```rust
     "compliance" => &["compliance", "onboarding"],
     ```
   - Choose mode-tags that describe what operations make sense in this workspace
   - If you skip this step, PACK001 **fails closed** — every macro in every pack that
     serves this workspace will be flagged as an error

3. **Create or update a pack** in `rust/config/packs/`
   - Add the new workspace to the pack's `workspaces` list
   - Add macros to `allowed_verbs` — only macros whose mode-tags include a tag
     accepted by the new workspace
   - Run `cargo x verbs lint-macros` to verify zero PACK001 errors

4. **Create constellation map** in `rust/config/sem_os_seeds/constellation_maps/`
   - Define slots with primitive verbs that macros in this workspace expand to
   - Assign to a constellation family

5. **Create or assign mode-tag** for macros that will serve this workspace
   - New macros: set `mode-tags` to include the new workspace's accepted tags
   - Existing macros: add the tag if the macro legitimately operates in the new context
   - Do NOT add mode-tags to macros that don't belong — PACK001 exists to prevent this

6. **Wire into orchestrator** in `rust/src/repl/orchestrator_v2.rs`
   - Add workspace selection routing (ScopeGate fork or WorkspaceSelection handler)

7. **Verify the full chain:**
   ```bash
   cargo x verbs lint-macros          # PACK001 — zero errors
   cargo x pre-commit                 # Format + clippy + unit tests
   cargo x verbs compile              # Sync verb registry
   ```

### What PACK001 Fail-Closed Looks Like

If you add `WorkspaceKind::Compliance` and wire it into the `kyc-case` pack but forget
step 2 (updating the compatibility table):

```
PACK001 error: macro 'case.open' [mode-tags: kyc, onboarding] in pack 'kyc-case'
  is exposed to workspace 'compliance' which accepts none of those tags
PACK001 error: macro 'screening.full' [mode-tags: kyc, onboarding] in pack 'kyc-case'
  is exposed to workspace 'compliance' which accepts none of those tags
... (every macro in the pack fails)
```

The fix: add `"compliance" => &["compliance", "kyc", "onboarding"]` to the table in
`workspace_accepts_any_mode_tag()`. Then re-run lint — only macros with incompatible
mode-tags will remain flagged (real bleed, not false positives).

### Architecture: Three-Layer Isolation

```
┌─────────────────────────────────────────────────────────────┐
│ Layer 1: Constellation Slots                                │
│   Primitive verbs gated by entity state (when: empty/filled)│
│   Defines WHAT operations are valid on WHICH entities       │
├─────────────────────────────────────────────────────────────┤
│ Layer 2: Pack allowed_verbs                                 │
│   Workspace-scoped whitelist of macros + verbs              │
│   Defines WHICH macros operators can see in WHICH workspace │
├─────────────────────────────────────────────────────────────┤
│ Layer 3: PACK001 Lint                                       │
│   Mode-tag ↔ workspace compatibility check                  │
│   Ensures pack wiring is INTENTIONAL, not accidental        │
│   Fail-closed: unknown workspace → reject all macros        │
└─────────────────────────────────────────────────────────────┘
```

Each layer catches a different class of error:
- **Constellation** prevents executing a verb when the entity isn't in the right state
- **Pack** prevents offering a verb/macro in the wrong workspace
- **PACK001** prevents accidentally wiring a macro into a workspace where it doesn't belong

---

## Composite Macros (invoke-macro)

Three macros use `invoke-macro` to compose from simpler macros:

| Macro | Invokes | Symbol Imports |
|-------|---------|---------------|
| `struct.ie.hedge.icav` | `struct.ie.aif.icav` | `@cbu`, `@trading-profile` |
| `struct.uk.authorised.ltaf` | `struct.uk.authorised.oeic` | `@cbu`, `@trading-profile` |
| `struct.pe.cross-border` | `struct.lux.pe.scsp` | `@cbu`, `@trading-profile` |

Pattern:
```yaml
expands-to:
  - invoke-macro: struct.ie.aif.icav
    args:
      name: "${arg.name}"
      aifm: "${arg.aifm}"
    import-symbols:
      - "@cbu"
      - "@trading-profile"
  # ... additional steps using @cbu from the invoked macro
```

---

## State DAG

Macros form a prerequisite DAG via `prereqs` and `sets-state`:

```
structure.setup/select → sets: structure.exists
  └→ case.open (requires: structure.exists) → sets: case.exists
       ├→ kyc-workstream.add (requires: case.exists) → sets: workstream.exists
       │    ├→ screening-ops.run (requires: workstream.exists) → sets: screening.completed
       │    │    └→ screening-ops.review-hit (requires: screening.completed)
       │    ├→ doc-request.create (requires: workstream.exists) → sets: docs.requested
       │    └→ kyc-workstream.close (requires: workstream.exists) → sets: workstream.closed
       ├→ ubo.discover (requires: case.exists + structure.exists) → sets: ubo.chain-built
       │    └→ ubo.allege → sets: ubo.allegation-exists
       │         └→ evidence.require (requires: ubo.allegation-exists) → sets: evidence.exists
       ├→ tollgate.evaluate (requires: case.exists) → sets: tollgate.evaluated
       │    └→ tollgate.override (requires: tollgate.evaluated)
       └→ case.submit (requires: case.exists) → sets: case.submitted
            └→ case.approve/reject (requires: case.submitted)
```
