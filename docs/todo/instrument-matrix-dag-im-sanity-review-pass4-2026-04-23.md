# Instrument Matrix DAG — Pass 4: Product-Modular Architecture (2026-04-23)

> **Status:** Captures Adam's most significant clarification to date —
> the Instrument Matrix is a **product-driven, modular service
> specification** between client CBU and service provider (BNY). Its
> richness extends based on which products the client has taken.
>
> **Context:** pass 1 (gaps), pass 2 (operational lifecycle), pass 3
> (what-vs-how + new slots), pass 3 addendum (three-layer model).
> This pass adds the **product-modularity** dimension that spans all
> prior passes.
>
> **Supersedes:** none. All pass-1-through-3 content stands; pass 4
> adds an orthogonal structural dimension (product modules) that
> applies on top of existing slots.

---

## 1. Adam's clarification (verbatim)

> *"to fully clarify the usage of the data in the instrument matrix —
> it is all data needed to setup and configure service.resources (eg
> applications) to be good to transact — and instrument matrix with
> enhancements — is the client source of truth of what the client CBU
> is transacting — which BNY systems need to interface with.*
>
> *The richness of the instrument matrix may be extended — depending
> of the products taken — e.g. if the client is sold custody and fund
> accounting then — for FA instrument types / categories will have
> 'pricing preference' attribute set — if its just custody — that is
> not required"*

Three distinct things established:

### 1.1 The IM is the CBU ↔ BNY service specification

The Instrument Matrix is the **client's source of truth of what the
CBU is transacting** — i.e. the specification that defines the service
contract. BNY is the service provider; the CBU is the customer. The
IM captures what the customer has asked the provider to do on their
behalf.

### 1.2 BNY systems consume the IM to provision service resources

Downstream of the IM, BNY's operational systems (custody platform,
fund-accounting system, transfer agency, middle-office, cash
management, etc.) need their configuration inputs. The IM is the
upstream spec that feeds them. Each BNY system interfaces with the IM
to pull its own config.

### 1.3 Richness is product-driven — modular extension

The IM is not a fixed schema. Its richness extends based on the
**products the client has taken**. A client who bought only custody
needs a thinner IM than a client who bought custody + fund accounting
(FA adds pricing preferences per instrument type) than a client who
bought custody + FA + transfer agency (TA adds share-class rules,
subscription terms). Each product activates additional configuration
requirements.

---

## 2. Why this matters — the orthogonal dimension

Pass 3 gave us the **three-layer vertical model**:

```
Layer 1: DAG (config + ref data)
Layer 2: Service resources (provisioned infrastructure)
Layer 3: Operations (runtime)
```

Pass 4 adds the **product-module horizontal dimension** to layer 1:

```
                   ┌─────────────── Instrument Matrix DAG ───────────────┐
                   │                                                      │
Core (always)  ──▶ │  client identity, trading profile, asset classes    │
Custody        ──▶ │  + custody SSIs, settlement chain, subcustodian map │
Fund Acct.     ──▶ │  + pricing preferences (per instrument type), ...  │
Transfer Agy.  ──▶ │  + share classes, subscription rules, ...           │
Middle Off.    ──▶ │  + booking rules, trade routing, ...                │
Derivatives    ──▶ │  + ISDA / CSA / collateral management, ...          │
Cash Mgmt      ──▶ │  + cash sweep config, FX hedge rules, ...           │
                   │                                                      │
                   └────────────────────────────┬─────────────────────────┘
                                                │ (feeds)
                                                ▼
                                   ┌────────────────────────┐
                                   │  BNY Service Resources  │
                                   │  (custody, FA, TA, MO,  │
                                   │   cash, etc.)           │
                                   └────────────────────────┘
```

The DAG for a specific CBU is the **union of the core + the modules
for every product that CBU has taken**. Products are keys; modules are
the config they require.

---

## 3. Illustrative examples

### 3.1 Custody-only mandate

**Products taken:** custody.

**Required DAG richness** (slots + attributes from A-1 v3 + pass 3
that are activated by custody product):
- `trading_profile` (core — always needed)
- `custody` (activated — SSIs, subcustodian)
- `settlement_pattern` (activated — how things settle)
- `booking_location` (ref data — where trades book)
- `entity-settlement.*` (custody-activated)
- `subcustodian.*` (custody-activated)

**Not needed** for custody-only: FA pricing preferences, TA share
classes, derivatives/collateral, cash sweep (unless separately taken).

### 3.2 Custody + Fund Accounting mandate

**Products taken:** custody + fund accounting.

**Adds to 3.1:**
- Per-`instrument_class` + `security_type` **pricing preference**
  attribute (whose price source, stale-price policy, fallback
  hierarchy).
- Valuation-source configuration (official close, fair-value
  fallback).
- NAV-calculation inputs (fee accruals, FX sources).
- Potentially a `pricing_config` slot (new).

### 3.3 Custody + FA + Transfer Agency mandate

**Adds to 3.2:**
- Share-class definitions (per-class fee, distribution policy,
  currency).
- Subscription / redemption terms per class.
- Dealing day rules, cut-off times.
- Transfer-agency-specific slots.

### 3.4 Custody + FA + Derivatives (long-short hedge fund)

**Adds to 3.2 + derivatives:**
- `isda_framework` (activated — ISDA counterparties).
- `collateral_management` (activated — per pass-3).
- Derivatives pricing sources (different from cash instrument
  pricing).
- Potentially `prime_broker` slot (new — PB relationships for
  leveraged mandates).

---

## 4. Architectural implications

### 4.1 Products need a registry

The platform needs a **product catalogue** — the list of services BNY
offers, each with an identifier. Examples:
- `product.custody`
- `product.fund_accounting`
- `product.transfer_agency`
- `product.middle_office`
- `product.cash_management`
- `product.derivatives`
- `product.prime_brokerage`
- `product.securities_lending`

### 4.2 Each product needs a config-manifest

Each product declares **what DAG slots + attributes it requires** to
configure its associated service resources. A manifest looks roughly:

```yaml
# rust/config/sem_os_seeds/product_manifests/fund_accounting.yaml
product: product.fund_accounting
requires_slots:
  - trading_profile (always-required)
  - pricing_config (new — FA-specific)
requires_attributes:
  - on: instrument_class
    attribute: pricing_preference
    required: true
  - on: instrument_class
    attribute: fallback_price_source
    required: false
  - on: security_type
    attribute: valuation_frequency
    required: true
activates_service_resources:
  - bny_eagle_star
  - bny_pricing_feed
```

### 4.3 CBU product enrolment needs a slot (or attribute)

For each CBU, the platform must know *which products they have
taken*. This is a CBU-level concern. Two options:

**(A) CBU attribute.** `cbus.enrolled_products` — array or set of
product identifiers. Simple.

**(B) Dedicated slot.** `cbu_product_enrolment` with per-enrolment
lifecycle (enrolled → active → suspended → terminated). Richer.

From Adam's past answers:
- CBU-level investment guidelines are CBU attributes (G-2 resolved).
- By analogy, CBU-level product enrolment might also be an attribute.

Unless product enrolment itself has governance lifecycle (approval
chain for adding a new product to an existing CBU), attribute suffices.

**Ask for Adam:** is product enrolment a CBU attribute or its own
slot?

### 4.4 Validation becomes product-aware

The catalogue validator (P.1.c) currently runs a single pass per verb
checking structural + well-formedness issues. For the pilot, that's
sufficient.

For product-modularity to work, a second validation pass is needed
**per CBU**:

> *For this CBU, with enrolled products [P1, P2, ...], is every
> required DAG slot / attribute per product-manifest present and
> correctly declared?*

This is different from the per-verb validator — it's a **per-instance
validation** run against a specific CBU's DAG state. Calls the
validator tool something like `validate_cbu_config(cbu_id)`.

**Implication for P.3:** when declaring three-axis attributes on
verbs, we need to know which product gates each slot's config. This
might surface as:
- Per-verb `requires_product:` field (verb-YAML) — "this verb is only
  meaningful if product X is taken."
- Per-attribute `requires_product:` (YAML) — similarly.

Not pilot-gating; can be added as a v1.1 candidate amendment.

### 4.5 Instrument Matrix richness is conditional on product bundle

The pilot plan assumes 21 slots for the pack-declared surface. That's
the **UNION** of all products' requirements. In practice, no single
CBU would have all 21 slots populated — their slot set matches their
product bundle.

This means:
- Catalogue validation (layer-1 spec consistency) covers all slots.
- Per-CBU validation (layer-2 instance consistency) covers only the
  CBU's active modules.

---

## 5. Implications for pilot scope

### 5.1 Pilot product-bundle scope

The pilot doesn't need to address all 7+ products. But it must be
clear which bundle the pilot exercises. Options:

**(A) Narrow** — pilot focuses on custody + trading only. Thinnest
IM. Tests the core + custody module.

**(B) Medium** — custody + FA (the common institutional bundle).
Tests the pricing-preference pattern Adam described as the concrete
example.

**(C) Full** — custody + FA + TA + derivatives. Stress-tests every
module. Matches the 21-slot pack surface.

**My recommendation:** **(C) full bundle at pilot scope**, because
the pack already declares all the verbs for all modules. Pilot
validates the declarative pattern; narrow/medium would leave module
combinations untested.

**Ask for Adam:** which product bundle is pilot-scope?

### 5.2 Pilot does NOT need to build the product catalogue

The pilot can proceed with **implicit product bundle = "everything in
the current Instrument Matrix pack"**. Building a formal product
catalogue + manifests + per-CBU enrolment is a Tranche-3-scale effort.

For pilot, the artefact is:
- Current 21-slot DAG covers the full union.
- Per-verb declarations happen for all verbs in the pack.
- Product-manifest formalisation defers to Tranche 3.

### 5.3 P.9 findings additions

Pass 4 adds these findings for P.9 / v1.1 candidates:
- **F-P1:** The IM is the CBU↔BNY service specification, not just a
  mandate model. Document the architectural role.
- **F-P2:** DAG richness is product-modular. Codify the pattern for
  estate-scale.
- **F-P3:** Product catalogue + manifests needed for estate-scale
  validation (per-CBU config completeness check). Out of pilot scope.
- **F-P4:** CBU product enrolment storage (attribute vs slot)
  undecided — ask Adam.

---

## 6. Known instrument-matrix richness map — what products activate what

First-pass mapping from instrument-matrix slots/attributes to
products. This is my domain-knowledge hypothesis; validates against
Adam's examples.

| Slot / Attribute | Activated by product(s) |
|---|---|
| `trading_profile` (core) | All — every mandate has a trading profile |
| `trading_profile.im_scope` | All — mandate scope |
| `trading_profile.allowed_currency` | All |
| `trading_profile.im_mandate` | All |
| `custody` slot | `product.custody` |
| `cbu-custody.*` verbs | `product.custody` |
| `settlement_pattern` slot | `product.custody` (core settlement) |
| `settlement-chain.*` verbs | `product.custody` |
| `booking_location` | `product.custody` or `product.middle_office` |
| `subcustodian.*` | `product.custody` |
| `entity-settlement.*` | `product.custody` |
| `tax-config.*` verbs | `product.custody` (tax on positions) |
| **`pricing_preference` attribute** (per instrument_class) | **`product.fund_accounting`** (Adam's explicit example) |
| **Pricing-source config** (new; not yet modeled) | **`product.fund_accounting`** |
| Valuation-policy config | `product.fund_accounting` |
| Share-class definitions | `product.transfer_agency` |
| Subscription / redemption terms | `product.transfer_agency` |
| Dealing-day / cut-off rules | `product.transfer_agency` |
| `booking_principal` | `product.middle_office` |
| `instruction-profile.*` | `product.middle_office` |
| `trade_gateway` | `product.middle_office` + `product.trade_execution` |
| `matrix-overlay.*` | `product.middle_office` (overlay logic) |
| `isda_framework` + `collateral_management` | `product.derivatives` |
| CSA-specific verbs | `product.derivatives` |
| `cash_sweep` slot | `product.cash_management` |
| `movement.*` (capital / subscription events) | `product.transfer_agency` or `product.custody` |
| `reconciliation` slot (pass 3) | Any product (per-stream: position=custody; cash=cash_management; NAV=FA) |
| `corporate_action_event` slot (pass 3) | `product.custody` (CA processing happens in custody chain) |
| `delivery` slot | Any — cross-product delivery tracking |
| `service_resource`, `service_intent` | Meta — always present (describes what BNY resources serve this CBU) |

---

## 7. Questions for Adam (pass-4 gate)

Before closing pass 4:

**Q-AA.** Is my product-modular understanding correct? I.e. the DAG
expands horizontally based on product bundles, and a validator would
eventually check per-CBU that the required config is present per
purchased product.

**Q-AB.** Pilot scope — does the pilot exercise the full product
bundle (recommended) or a narrow subset?

**Q-AC.** CBU product enrolment — attribute on `cbus` table (simple),
or dedicated slot with governance lifecycle (rich)?

**Q-AD.** Product catalogue — does one exist today in ob-poc, or does
it need to be built? (If exists, where is it?)

**Q-AE.** Should pilot-scope include **one new slot for
product-conditional attributes** (e.g. `pricing_config` as Adam's
example implies), or defer all product-conditional structure to
Tranche 3?

**Q-AF.** The product-module map in §6 — is it roughly right, or does
my guesswork miss something significant?

---

## 8. Feed-forward

### 8.1 Changes to pilot scope after pass 4

**None forced.** Pass 4's core insight (product-modularity) is
architectural framing, not a slot/state change. The 21-slot inventory
from pass 3 stands. The ~56-state count stands. P.2 and P.3 scopes
are unchanged.

**What pass 4 adds:**
- Documentation in P.9 findings of the product-modular architecture.
- v1.1 candidate amendment codifying product-manifest pattern.
- An "are the ~186 pack verbs + ~22 new verbs product-uniform or
  should they carry `requires_product:` tags?" question for Adam.

### 8.2 Estate-scale implications

Pass 4 is a **significant input for Tranche 3** (governed authorship
mechanism). The Catalogue workspace may need:
- Product catalogue authoring verbs.
- Product-manifest authoring.
- Per-CBU product-enrolment management.
- Product-aware validation.

Flag in findings.

### 8.3 Relationship to G-2 resolution

Pass 1 G-2 resolved "investment guidelines are CBU-level attributes."
Pass 4 provides the mechanism: CBU-level attributes are populated based
on the CBU's product enrolment. Investment guidelines tied to the
portfolio-management service (if offered). Same pattern as pricing
preferences tied to FA.

---

## 9. Summary

**Instrument Matrix = client-source-of-truth service specification,
product-modular, feeding BNY service-resource provisioning.**

The DAG is not a fixed schema. It's the union of:
- Core mandate spec (always).
- Product-specific modules (activated per CBU's product bundle).

The richness scales with what the client has bought. A custody-only
mandate needs a thin IM; a full-service (custody + FA + TA +
derivatives + cash) mandate needs the full 21-slot richness.

Validation is two-tiered:
- Layer 1 (catalogue validator — P.1.c): all slots + verbs are
  structurally valid. Always required.
- Layer 2 (per-CBU product-completeness check — future): every CBU
  has the config required by its enrolled products. Tranche-3 scale.

This framing is the architectural statement of what the Instrument
Matrix is FOR. Worth promoting to v1.1 as a core section.

**End of pass 4.** Pending Adam's answers to Q-AA through Q-AF.
