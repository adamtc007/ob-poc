# Catalogue Platform Refinement — Vision & Scope (v1.2)

> **Status:** Final. Approved as architecture specification.
> **Date:** 2026-04-23 (v1.2 amendment, post-pilot)
> **Prior versions:** v1.0 (2026-04-18, historical), v1.1 (2026-04-22),
> all retained in `/docs/todo/`.
> **v1.2 changes from v1.1:** 10 amendments codifying what the
> Instrument Matrix pilot (`instrument-matrix-pilot-findings-2026-04-23.md`)
> validated, discovered, or corrected. Does NOT modify any existing
> principle (P1–P15); adds one new principle (P16) plus schema extensions
> and factual updates.
> **Decision level:** final architectural specification.

---

## v1.2 CHANGES FROM v1.1 — at a glance

| # | Amendment | Type | Severity |
|---|---|---|---|
| V1.2-1 | **New principle P16** — DAG is the upstream declarative spec layer (three-layer vertical architecture) | New principle | Architectural |
| V1.2-2 | **`overall_lifecycle:` section** in DAG taxonomy YAML as first-class alongside `slots:` and `cross_slot_constraints:` | Schema extension | Medium |
| V1.2-3 | **`requires_products:` axis** on slot/verb/attribute — enables per-CBU reachability filtering | Schema extension | Medium |
| V1.2-4 | **Prune semantics** as a general architectural pattern (not IM-specific) | Architectural pattern | Medium |
| V1.2-5 | **`PackFqnWithoutDeclaration`** validator error — pack hygiene check ✅ **LANDED** | Validator extension | Small |
| V1.2-6 | **§4.1 reference** to existing products/services/SRDEFs infra (migrations/PRODUCTS_SERVICES_RESOURCES.md) | Factual update | Editorial |
| V1.2-7 | **Borderline-operational-slot pattern** documented (schema-persisted operational state that doesn't really belong in DAG) | Architectural note | Small |
| V1.2-8 | **DSL over-modeling lint check** for verbs clusters that suggest lifecycle where domain is binary | Lint extension | Medium |
| V1.2-9 | **Sem-os scan path** for `dag_taxonomies/` directory formalised | Implementation gap | Medium |
| V1.2-10 | **Estate-scale effort estimate revision** — infrastructure amortizes; governance is the bottleneck | Section rewrite | Editorial |

Landed today (2026-04-23): V1.2-5 (validator check commit `5bc63448`).
L-3 legacy verb deprecation commit `cad0529b` satisfies a pilot loose-end
that didn't need a spec amendment but is captured here for traceability.

---

## 1. NEW PRINCIPLE — P16 (three-layer vertical architecture)

**P16 — The DAG is the upstream declarative spec layer feeding
downstream service-resource provisioning.**

The platform architecture stratifies into three vertical layers:

```
┌──────────────────────────────────────────────────────────┐
│  LAYER 1 — DAG (this catalogue)                          │
│    - static reference data                               │
│    - standing configuration (what BNY systems need)      │
│    - config lifecycle (draft → active → retired)         │
│    - enough to provision downstream                      │
└──────────────────────────────────────────────────────────┘
                            │ feeds
                            ▼
┌──────────────────────────────────────────────────────────┐
│  LAYER 2 — SERVICE RESOURCES (downstream of DAG)         │
│    - accounts opened                                     │
│    - tax flags set                                       │
│    - SWIFT/FIX/BIC routings configured                   │
│    - custodian relationships wired                       │
└──────────────────────────────────────────────────────────┘
                            │ operated by
                            ▼
┌──────────────────────────────────────────────────────────┐
│  LAYER 3 — OPERATIONS (not in DAG)                       │
│    - runtime events (recon runs, margin calls)           │
│    - exception handling (fails, breaks, investigations)  │
│    - execution quality (UAT, SLA, cert)                  │
│    - processing/settlement lifecycle                     │
└──────────────────────────────────────────────────────────┘
```

**Decision rule for any proposed state, slot, or verb:**

> Is this a standing property (WHAT the entity IS or is configured to do),
> or an operational detail (HOW it runs day-to-day)?
> - Standing property → Layer 1 (DAG).
> - Service-resource provisioning → Layer 2.
> - Operational execution → Layer 3 (not in DAG).

This resolves edge-case "should this be a DAG slot?" questions durably
and consistently.

**Sources:** pass-3 addendum, pass-7 (`instrument-matrix-dag-im-sanity-review-pass3-2026-04-23.md`
and `pass7-2026-04-23.md`).

---

## 2. SCHEMA EXTENSIONS (V1.2-2, V1.2-3, V1.2-4)

### 2.1 V1.2-2 — `overall_lifecycle:` section

DAG taxonomy YAML (under `config/sem_os_seeds/dag_taxonomies/`) gains
a first-class `overall_lifecycle:` section alongside the existing
`slots:` and `cross_slot_constraints:` sections.

The overall lifecycle declares an aggregate state machine for the DAG's
target entity (typically the CBU's instance of the workspace's primary
artefact). State derivation rules reference per-slot states + upstream
entity states (cbus.status + service intents + readiness).

**Example** (from Instrument Matrix pilot P.2):
```yaml
overall_lifecycle:
  id: instrument_matrix_overall_lifecycle
  scope: per_cbu
  derived: true
  phases:
    - name: onboarding_requested
      derivation:
        all_of:
          - "cbus.status IN [DISCOVERED, VALIDATION_PENDING]"
          - "EXISTS service_intents WHERE cbu_id = this.cbu_id"
      progression_verbs: [cbu.submit-for-validation]
      next_phase: matrix_scoped
    # ... more phases ...
```

Derived by default — no new persisted column required. The validator
+ runtime project the phase from the underlying slot states.

**Sources:** pass 7.

### 2.2 V1.2-3 — `requires_products:` conditional reachability

Slots, verbs, and attributes gain an optional `requires_products:`
field listing the product IDs that gate the node. Per-CBU effective
DAG is the catalogue intersected with the CBU's active `service_intents`
product set.

**Example:**
```yaml
slots:
  - id: collateral_management
    requires_products: [product.derivatives, product.collateral_mgmt]
    # ...
```

```yaml
verbs:
  pricing-preference:
    set:
      requires_products: [product.fund_accounting]
      # ...
```

- Catalogue-level validator is unchanged (runs over all products).
- Per-CBU config-completeness validator (Tranche-3) masks by
  `CBU.service_intents.products` ∩ node.`requires_products`.
- Runtime verb-surface applies the same mask to filter operator-visible
  actions.

**Sources:** pass 4, pass 5.

### 2.3 V1.2-4 — Prune semantics as a general pattern

Any DAG slot with cascade dependencies MAY declare prune operations.
A prune is subtree deletion at a classification level (asset family /
market / entity class / counterparty / counterparty type) with
cascade through dependent config.

**Pattern elements:**
- **Granularities:** slot-author defines which classification axes
  support prune (pilot IM used: asset_family, market, instrument_class,
  counterparty, counterparty_type).
- **Cascade rules:** declared in `prune_cascade_rules:` section of the
  DAG taxonomy YAML. Each rule specifies targets + transformation
  (remove / deactivate / downgrade).
- **Pre-validation:** required read-only impact preview + post-prune
  universe-coverage re-validation.
- **Abort conditions:** enumerated — prune aborts if it would leave an
  ACTIVE mandate in an invalid state (e.g. no instrument classes, no
  live settlement chain, derivative exposure without ISDA).
- **Template prunes vs CBU-instance prunes:**
  - Template prunes are forward-only (existing clones retained).
  - CBU-instance prunes trigger amendment flow (pass 7 Q-CD).
- **Tier:** prunes baseline to `requires_explicit_authorisation`
  (destructive by definition).

**Sources:** pass-3 §3, post-pilot L-3-style validator alignment.

---

## 3. VALIDATOR EXTENSIONS (V1.2-5, V1.2-8)

### 3.1 V1.2-5 — PackFqnWithoutDeclaration (LANDED 2026-04-23)

**Rule:** Every FQN in a pack's `allowed_verbs` list must resolve to
(a) a declared verb in the catalogue, or (b) a macro FQN. Unresolved
FQNs are well-formedness errors.

**Rationale:** A-2 audit found 11 stale pack FQNs in the pilot pack
(matrix-overlay.apply/.create/.diff/.read/.update/.preview/.list-active,
delivery.create/.list/.read, booking-location.read) that were
pack-authored against expected-but-never-built verb surfaces.

**API:** `dsl_core::config::validate_pack_fqns(declared_verbs, macro_fqns,
pack_entries)`. Pure function; caller loads pack + macro files.

**Status:** LANDED as of commit `5bc63448`. Remaining: wire into the
ob-poc-web startup gate, `cargo x verbs lint`, and `cargo x reconcile
validate`. Tracked as loose-end for next session.

### 3.2 V1.2-8 — DSL over-modeling lint

**Rule:** Detect verb clusters that suggest a richer lifecycle than
the underlying schema or domain supports. Specifically: clusters that
include `suspend` / `reactivate` / `remove` where the schema has only
`is_active` boolean OR the domain reality (from Adam's pilot answers
Q8, Q9a) is binary.

**Rationale:** Pilot flagged `cash-sweep.suspend/.reactivate/.remove`
as 3 verbs collapsing to a single is_active toggle. `booking-principal.retire`
as a rule-entity operation rather than lifecycle state. Clean DSL
should mirror domain state granularity.

**Emits:** policy-sanity warning (not error — over-modeling is a
readability/consolidation opportunity, not a correctness bug). Suggests
consolidation to a single `set-active: boolean` verb where applicable.

**Status:** Deferred. Small scope; add when DSL authoring surface
extends to cover it naturally.

---

## 4. FACTUAL UPDATES (V1.2-6, V1.2-10)

### 4.1 V1.2-6 — existing products/services/SRDEFs infra

**§4.1 update:** The platform has a complete three-layer product-
services-resources architecture pre-dating v1.1, documented in
`migrations/PRODUCTS_SERVICES_RESOURCES.md`:

- `"ob-poc".products` (7 products: CUSTODY, FUND_ACCOUNTING,
  TRANSFER_AGENCY, MARKETS_FX, MIDDLE_OFFICE, COLLATERAL_MGMT, ALTS)
- `"ob-poc".services` (capability layer — SAFEKEEPING, NAV_CALC,
  CASH_MGMT, etc.)
- `"ob-poc".service_resource_types` (SRDEFs with
  `srdef_id = SRDEF::<OWNER>::<Type>::<Code>`)
- `"ob-poc".product_services` (junction — carries `is_mandatory`,
  `is_default`, configuration JSONB = the product manifest)
- `"ob-poc".service_intents` (per-CBU — the lifecycle services profile)
- `"ob-poc".cbu_resource_instances`, `.cbu_service_readiness`,
  `.cbu_unified_attr_requirements`, `.cbu_attr_values`
- **6-stage pipeline:** Intent → Discovery → Rollup → Population →
  Provisioning → Readiness

The Catalogue workspace (Tranche 3) extends this existing infrastructure;
it does not replace it. Pass-4/5 concepts map directly:
- "Product catalogue" = `products` table.
- "Product manifest" = `product_services` junction.
- "Lifecycle services profile" = per-CBU `service_intents` set.
- "Per-CBU effective DAG" = output of Stage 2 Discovery Engine
  (parameterized SRDEF expansion).

**Sources:** pass 6.

### 4.2 V1.2-10 — estate-scale effort estimate revision

**§11 stopping-point estimates revision.** v1.1's per-workspace effort
estimates assumed per-verb declaration cost + governance review at
workshop-heavy pace. Pilot empirical data shows:

- **One-time infrastructure** (P.1 schema / validator / composition /
  startup gate / lint / P.6 smoke test / P.7 reconcile CLI) amortizes
  across all workspaces. Built once, reused 4+ times.
- **Per-verb declaration** is mechanically fast once the pattern is
  established (~minutes per verb using pattern-based defaults + tier
  review).
- **Governance coordination** is the real bottleneck at estate scale:
  cross-workspace tier consistency + real P-G committee governance
  coordination (vs pilot's Adam-alone authority).

**Revised Tranche 2 estimate:**
- Per-workspace authoring: ~1.5 person-days (down from v1.1's ~10 days)
- Cross-workspace consistency pass: ~3 days
- 4 workspaces × 1.5 + 3 = **~9–10 working days** of engineering
- Governance review (P-G committee coordination): likely the 3–4×
  multiplier on top, making Tranche 2 realistic in **4–6 calendar
  weeks** (not months).

**Sources:** `instrument-matrix-pilot-findings-2026-04-23.md` §6.

---

## 5. ARCHITECTURAL NOTES (V1.2-7, V1.2-9)

### 5.1 V1.2-7 — borderline-operational-slot pattern

Some slots have schema-backed states that under the three-layer rule
(P16) are arguably Layer 3 (operational) rather than Layer 1 (DAG).
Example: `delivery` with states PENDING / IN_PROGRESS / DELIVERED /
FAILED / CANCELLED.

**Rule for borderline cases:**
- If the schema persists the states today, retain in DAG until a
  dedicated refactor relocates them to an operations workspace.
- Flag the slot as `borderline_operational: true` in the DAG taxonomy
  YAML for future review.
- Do not migrate schema-persisted state out of DAG in a single step;
  it requires a careful refactor coordinating the validator, verb
  declarations, and any cross-slot constraints.

**Sources:** pass-3 addendum §9.4.

### 5.2 V1.2-9 — sem-os scan for dag_taxonomies/

The DAG taxonomy YAML directory
(`rust/config/sem_os_seeds/dag_taxonomies/`) is a new authoring surface
that needs a scanner function in `sem_os_obpoc_adapter::pipeline_seeds`.

**Obligation:** add `scan_dag_taxonomies()` that reads the directory
and produces:
- `StateMachine` seeds (one per slot with a state machine).
- `StateGraph` seeds (for the overall_lifecycle and each cross-slot
  dependency graph).
- Snapshots in `sem_reg.snapshots` table.

Until this scanner is added, the DAG taxonomy YAML is YAML-only and
doesn't flow into the governed registry. Runtime consumers (validator,
verb surface) continue to work via direct YAML load; the gap is
between YAML authoring and sem-os publication.

**Status:** Deferred. Medium-size implementation; scoped for a dedicated
session alongside Tranche 2 kickoff (when more workspaces need the
same scanner extension).

**Sources:** pilot P.9 findings §4 V1.1-CORRECTION-7.

---

## 6. DoD IMPLICATIONS

No Tranche 1 DoD items (1–13 from v1.1) are modified. V1.2 amendments
are additions; existing items remain in force.

**Two new Tranche 1 DoD items for v1.2:**

14. **Three-layer architecture (P16) documented in platform
    architecture docs** — both developer-facing + operator-facing. Cross-
    referenced from CLAUDE.md or equivalent.

15. **`PackFqnWithoutDeclaration` validator check active in
    catalogue-load gate** — runs alongside the P.1.c three-axis checks.
    Completes V1.2-5 wiring.

Existing DoD item 11 (P-G governance) remains an absolute requirement
for Tranche 2 readiness.

---

## 7. Tranche 2 kickoff checklist

When Tranche 2 begins (estate-scale reconciliation of KYC, Deal, CBU,
and other workspaces), these v1.2 amendments shape the work:

- [ ] V1.2-1 P16 principle applied to each workspace's slot inventory
      — classify every slot as DAG / service-resource / operational.
- [ ] V1.2-2 `overall_lifecycle:` authored for each workspace's primary
      artefact (KYC case, Deal, CBU itself where applicable).
- [ ] V1.2-3 `requires_products:` assigned per slot/verb. Instrument
      Matrix pilot's product-module map in `instrument-matrix-dag-im-sanity-review-pass4-2026-04-23.md`
      §6 is the hypothesis; each workspace confirms.
- [ ] V1.2-4 prune operations authored per workspace where cascades
      matter.
- [ ] V1.2-5 wiring landed in startup gate, lint, reconcile CLI.
- [ ] V1.2-6 § updated in CLAUDE.md or equivalent.
- [ ] V1.2-7 borderline slots flagged per workspace.
- [ ] V1.2-8 lint deferred (low priority).
- [ ] V1.2-9 sem-os scanner for `dag_taxonomies/`.
- [ ] V1.2-10 revised estate-scale estimate signed off by P-G.

---

## 8. What DOES NOT change

For traceability, explicit list of what v1.1 elements survive intact:

**Principles P1–P15** — all stand. Three-axis declaration (P1), DB-free
catalogue (P3), baseline+escalation tier (P11), uniform runbook
composition (P12), semantic-vs-policy distinction (P13), all confirmed
by the pilot and carry forward unchanged.

**Consequence tier taxonomy** (benign / reviewable / requires_confirmation
/ requires_explicit_authorisation) — unchanged, validated at pilot
scale.

**Escalation DSL (R6)** — restricted predicate set unchanged. Pilot
confirmed sufficiency.

**P-G governance** — still required for Tranche 1 DoD; Adam-as-authority
remains pilot-only convention per v1.1 §13.

**Validator warning discipline (P13)** — conservative, narrow. Pilot
confirmed zero false positives on real catalogue.

**Decision gates** (v1.1 §10) — all 30 gates stand; V1.2 amendments
don't modify them.

---

## 9. Closure

v1.2 codifies what the Instrument Matrix pilot discovered + corrected
+ built. The architecture is more complete, the infrastructure is
proven, and the path to Tranche 2 is charted.

Ten amendments. One landed (V1.2-5). Five are docs/schema (V1.2-1,
V1.2-2, V1.2-3, V1.2-4, V1.2-6, V1.2-7, V1.2-10). One is wiring
(V1.2-5 bridge into runtime tools). Two are deferred implementation
(V1.2-8 lint, V1.2-9 scanner). Zero principles modified; backward-
compatible with v1.1.

**v1.2 is final.**

---

**End of Catalogue Platform Refinement v1.2 — Final.**
