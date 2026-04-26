# Instrument Matrix DAG — Pass 5: Conditional Node Reachability + Lifecycle Services Profiles (2026-04-23)

> **Status:** Adam's extension of pass 4. The DAG has **conditional node
> sets** — per-CBU reachability is gated by the CBU's **lifecycle
> services profile** (the bundle of products the CBU has taken).
> Unreachable nodes (e.g. `pricing_preference` when FA isn't enrolled)
> exist in the catalogue but are invisible / non-executable in that
> CBU's effective DAG.
>
> **Parent:** pass 4 (product-modularity). This pass adds the
> reachability semantics that make product-modularity operational.

---

## 1. Adam's extension (verbatim)

> *"a DAG has conditional node sets — depending on CBU and product
> lifecycle_services profiles — e.g. some nodes may be unreachable —
> e.g. the pricing preferences — when FA as a product has not been
> selected for on-boarding that CBU"*

This adds two concepts on top of pass 4's product-modularity:

1. **Conditional node reachability.** DAG nodes exist statically in
   the catalogue, but their reachability is per-CBU. A node is
   reachable if-and-only-if the gating product is in that CBU's
   profile.

2. **Lifecycle services profile.** A named bundle of products. CBUs
   enrol into a profile; the profile determines which nodes are
   reachable. Examples:
   - *"Custody-Only profile"* = [custody]
   - *"Fund Services Plus profile"* = [custody, FA]
   - *"Full Service Profile"* = [custody, FA, TA, derivatives, cash]
   - *"Hedge Fund PB profile"* = [custody, derivatives, prime-broker,
     securities-lending]

---

## 2. The reachability model

### 2.1 Static catalogue vs. effective per-CBU DAG

The DAG has two views:

**Static catalogue DAG** — the full taxonomy, all slots, all states,
all verbs. Populated by P.2 authoring + P.3 declarations. This is
what the validator runs against at catalogue-load time.

**Effective per-CBU DAG** — the subset of the static DAG reachable for
a specific CBU, given their lifecycle services profile. Derived at
runtime by masking the static DAG with the CBU's profile.

```
static_catalogue_dag ∩ cbu.lifecycle_services_profile = effective_cbu_dag
```

Unreachable nodes are:
- Still present in the catalogue (so the agent knows they EXIST).
- Invisible in the effective DAG (so the agent doesn't offer them to
  operators on this CBU).
- Non-executable (if an agent tries to call a verb on an unreachable
  node, the executor rejects with "requires product X, not enrolled").

### 2.2 What gets gated

Three levels of product-gating granularity:

| Granularity | Example | Gate mechanism |
|---|---|---|
| **Entire slot** | `collateral_management` slot unreachable without derivatives product | `requires_products:` on slot declaration |
| **State machine edges** | `trading_profile.active → parallel_run` edge unreachable for non-complex mandates | `requires_products:` on transition edge |
| **Individual verbs** | `pricing-preference.set` unreachable without FA | `requires_products:` on verb YAML |
| **Attribute values** | `instrument_class.pricing_preference` attribute only writable under FA | `requires_products:` on attribute schema |

Adam's example (FA → pricing_preference attribute) is attribute-level
gating. Other cases cover slot and verb levels. All three should be
supported uniformly.

### 2.3 Relationship to the three-layer model

This extends the pass-3 three-layer model cleanly:

```
Layer 1 (DAG)
  ├── Static catalogue  (all products, all slots, all verbs)
  └── Per-CBU effective DAG  (masked by lifecycle services profile)
         │
         ▼ feeds
Layer 2 (Service Resources — also masked by profile)
         │
         ▼ operated by
Layer 3 (Operations — also masked by profile)
```

The lifecycle-services-profile cascades down all three layers. If FA
isn't in the profile, no FA DAG nodes; no FA service resources
provisioned; no FA runtime operations.

---

## 3. Representation — how this lands in YAML

### 3.1 Slot-level gating (DAG taxonomy YAML)

```yaml
# rust/config/sem_os_seeds/dag_taxonomies/instrument_matrix_dag.yaml
slots:
  - id: collateral_management
    requires_products: [product.derivatives]
    states:
      - configured
      - active
      - suspended
      - terminated
    transitions:
      - from: configured
        to: active
        verb: collateral-management.activate
```

When loading the effective DAG for a CBU whose profile lacks
`product.derivatives`, the loader drops this entire slot.

### 3.2 Verb-level gating (verb YAML)

```yaml
# rust/config/verbs/pricing-preference.yaml
pricing-preference:
  verbs:
    set:
      description: "Set pricing preference for an instrument class"
      behavior: plugin
      requires_products: [product.fund_accounting]
      three_axis:
        state_effect: preserving
        external_effects: []
        consequence:
          baseline: reviewable
```

Verb is visible in the catalogue but filtered out of the agent's
verb surface for CBUs without FA.

### 3.3 Attribute-level gating (entity schema)

Attributes on reference entities (like `instrument_class`) can carry
per-attribute gates. This is less common than slot/verb gating but
Adam's canonical example (`pricing_preference`) is exactly this:

```yaml
# rust/config/sem_os_seeds/entity_types/instrument_class.yaml
instrument_class:
  attributes:
    - name: pricing_preference
      requires_products: [product.fund_accounting]
      type: enum
      values: [official_close, fair_value, broker_consensus]
```

### 3.4 Transition-edge gating (optional)

In case a single state has some edges gated and others not:

```yaml
slots:
  - id: trading_profile_lifecycle
    states: [draft, submitted, approved, active, ...]
    transitions:
      - from: active
        to: active  # cosmetic — capital-only change
        verb: trading-profile.rebalance
        requires_products: [product.middle_office]  # only if MO enrolled
      - from: active
        to: suspended
        verb: trading-profile.suspend
        # no requires_products: always available
```

### 3.5 Profile declaration

Profiles themselves are declared separately:

```yaml
# rust/config/sem_os_seeds/lifecycle_services_profiles/full_fund_services.yaml
id: profile.full_fund_services
display_name: "Full Fund Services"
description: "Custody + Fund Accounting + Transfer Agency"
products:
  - product.custody
  - product.fund_accounting
  - product.transfer_agency
  - product.cash_management
```

And CBUs reference their profile:

```yaml
# cbu attribute (or row in cbus table)
cbu.abc123:
  lifecycle_services_profile: profile.full_fund_services
```

This answers pass-4 Q-AC: profile is the indirection layer. CBU
enrols into a named profile; profile resolves to a product list. Adds
a governance layer (profiles themselves can have their own lifecycle —
which Adam hinted at with "profiles" terminology).

---

## 4. Tooling implications

### 4.1 DAG loader — apply profile filter

The DAG loader gains a per-CBU method:

```rust
pub fn load_effective_dag_for_cbu(
    catalogue: &DagTaxonomy,
    profile: &LifecycleServicesProfile,
) -> EffectiveDag {
    // Drop slots whose requires_products is disjoint from profile.products.
    // Drop verbs whose requires_products is disjoint.
    // Drop attributes whose requires_products is disjoint.
}
```

### 4.2 Agent verb surface — product-aware filter

`SessionVerbSurface` (pass-1 context) already filters verbs by
workspace, ABAC role, lifecycle state. Adds a new filter step:

> **Step N (new): product-enrollment filter.** Drop any verb whose
> `requires_products` is not a subset of the target CBU's profile
> products.

### 4.3 Validator — two-tier

- **Catalogue validator (P.1.c)** — runs at startup. Checks the
  static DAG's structural + well-formedness. Unchanged.
- **Per-CBU config completeness validator (new, Tranche-3 scale)** —
  runs on-demand or on-enrolment-change. Checks that for every product
  in the CBU's profile, all required DAG config is present + valid.

### 4.4 Executor — runtime refusal

If an agent / MCP client bypasses the surface filter and calls a
product-gated verb directly, the executor must refuse with a typed
error:

```
VerbExecutionError::ProductNotEnrolled {
    verb: "pricing-preference.set",
    required_product: "product.fund_accounting",
    cbu_profile: "profile.custody_only",
}
```

This is defence-in-depth — the surface filter should prevent the
issue, but the executor enforces it too.

### 4.5 UX — reachable vs unreachable

Two modes for the catalogue browser / Observatory:

- **Default (per-CBU):** show only reachable nodes + verbs. What the
  operator can actually do on THIS CBU.
- **Toggle "show all products":** show all catalogue nodes with
  unreachable ones grayed-out and marked "requires product X."

---

## 5. Implications for pilot

### 5.1 Pilot scope implications

**Pilot can proceed without product-gating implemented.** For pilot:

- The static catalogue DAG is what P.2 authors and P.3 declares.
- Effective-DAG filtering is a Tranche-3 scale feature (ties to
  Catalogue workspace).
- Pilot treats all 21 slots + all ~210 verbs as always-available.

However, pilot SHOULD:

1. **Add an optional `requires_products:` field** to verb YAML schema
   (P.1.a extension — small). Default empty (always available). This
   makes the catalogue future-ready even if no verb declares a
   requirement in pilot scope.

2. **Document the lifecycle-services-profile concept** in P.9
   findings as a v1.1 candidate amendment. Architectural framing,
   not pilot code.

3. **NOT build** the profile registry, per-CBU enrolment, or
   effective-DAG loader. All Tranche 3.

### 5.2 Impact on P.2 / P.3 / P.9

**P.2 DAG taxonomy YAML:** add `requires_products:` as an optional
field on slot declarations. Don't populate for any slot in pilot
(products aren't modelled yet). Slot schema is ready for later.

**P.3 per-verb declaration:** same — `requires_products:` is an
optional schema field on verb YAML. Don't populate unless Adam points
at a specific verb (e.g. the hypothetical `pricing-preference.set` if
pilot creates that slot).

**P.9 findings:** pass 4 + pass 5 become the architectural section on
"how the IM is product-modular with conditional reachability." Promote
to v1.1 as the definitive statement of what the Instrument Matrix is.

### 5.3 What this changes vs. pass 4

Pass 4 established product-modularity as a static architectural
property.

Pass 5 makes it **runtime-dynamic per CBU** — the DAG visible to an
operator/agent/MCP changes based on which CBU is in session.

Pass 4's Q-AE ("pilot should include a new slot for product-conditional
attrs like pricing_config?") now resolves to:

**Recommendation:** do NOT add a `pricing_config` slot in pilot. The
pilot's job is to validate the declarative pattern (P1 three-axis
verb model). Introducing product-gating infrastructure during pilot
scope-creeps. Defer to Tranche 3 where Catalogue workspace will
manage product-manifest authoring alongside slot authoring.

---

## 6. Remaining unknowns / follow-ups

**U-1: Do profiles have their own lifecycle?**

Adam's term "profiles" suggests they might. E.g. a profile itself
could be `draft → approved → active → retired` — governance around
offering a profile. Or profiles could be static reference data
(BNY's service catalogue). Don't know; flag for later.

**U-2: CBU profile enrolment — which slot owns it?**

From pass 4 Q-AC: attribute on `cbus` or dedicated slot. Pass 5
suggests: **attribute on cbus** for simplicity. Enrolment state
(active / suspended enrolment) handled by the CBU's own lifecycle
(pass-3 added SUSPENDED to cbus.status — aligns).

**U-3: Product authoring + profile authoring — which workspace?**

Probably `SemOsMaintenance` (the existing governance workspace) since
products and profiles are platform-wide catalogue items, not
per-workspace data. Tranche 3 concern.

**U-4: Multi-product slots (intersection vs union gating).**

If a slot has `requires_products: [custody, FA]`, does it mean
"requires BOTH" (intersection) or "requires EITHER" (union)?
Semantically "AND" is the natural reading for "requires." Should
explicit `requires_all_of:` / `requires_any_of:` syntax be introduced?
Probably not for pilot; flag for v1.1.

---

## 7. Summary

**The DAG is product-modular AND product-reachable-conditional.**

- **Static catalogue** — all products, all slots, all verbs. What
  the platform CAN offer.
- **Per-CBU effective DAG** — masked by lifecycle-services-profile.
  What an operator / agent / MCP SEES on this CBU.
- **Lifecycle-services-profile** — named bundle of products. CBUs
  enrol into profiles.
- **Reachability gating at three granularities:** slot, verb,
  attribute. Plus optional per-transition-edge.

**For pilot:**
- Add `requires_products:` as an optional schema field (small P.1
  extension).
- Don't populate any verb/slot with it in pilot.
- Don't build profile registry / effective-DAG loader / per-CBU
  validator.
- Document the architecture in P.9 findings for v1.1 codification.

**For Tranche 3 (Catalogue workspace):**
- Product registry + manifests.
- Lifecycle-services-profile registry with governance.
- Effective-DAG loader + per-CBU config completeness validator.
- Observatory UI with "reachable/unreachable" toggle.
- Agent verb surface's product-aware filter step.

---

## 8. Net pass 4 + pass 5 — complete architectural picture

Three orthogonal architectural dimensions now codified:

| Dimension | Source | Rule |
|---|---|---|
| **Vertical three-layer** | pass 3 addendum | DAG (L1) → Service Resources (L2) → Operations (L3). DAG owns config; downstream layers own execution. |
| **Horizontal product-module** | pass 4 | DAG is the union of modules activated by purchased products. Custody-only = thin; full-service = rich 21-slot. |
| **Runtime reachability** | pass 5 | Per-CBU, DAG is masked by lifecycle-services-profile. Unreachable nodes exist in catalogue but not in effective view. |

Together: the Instrument Matrix is a static catalogue of potential
service specifications that, when enrolled against by a specific CBU
with a specific profile, projects a per-CBU effective DAG that drives
service-resource provisioning (layer 2) and operational execution
(layer 3).

That's the complete architectural statement. Promote to v1.1 as the
Instrument Matrix's core characterization.

---

**End of pass 5.** Asking for Adam's sanity-check on the reachability
model (§2-§3), plus answers to U-1 through U-4 before closing.
