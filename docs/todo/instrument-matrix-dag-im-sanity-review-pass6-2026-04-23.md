# Instrument Matrix DAG — Pass 6: Alignment with existing Products/Services/SRDEFs (2026-04-23)

> **Status:** Correction pass. Passes 4 + 5 proposed a product-catalogue + profile-registry
> architecture. Adam pointed out: *"We do have a product, lifecycle_service and
> servicing_resource set of tables."*
>
> Reading `migrations/PRODUCTS_SERVICES_RESOURCES.md` confirms — the infrastructure
> exists and is richer than my proposals. This pass aligns pass-4/5 concepts to the
> actual existing entities so P.2/P.3/P.9 don't invent parallel structure.
>
> **What changes:** terminology and references. The architectural insight stands; the
> implementation doesn't need new tables / registries.

---

## 1. Existing architecture — what's already there

`rust/migrations/master-schema.sql` + `migrations/PRODUCTS_SERVICES_RESOURCES.md` define
a complete product/service/resource pipeline:

### Three-layer taxonomy

| Layer | Table | Purpose |
|---|---|---|
| 1. **Products** | `"ob-poc".products` | Commercial bundles a client buys: CUSTODY, FUND_ACCOUNTING, TRANSFER_AGENCY, MARKETS_FX, MIDDLE_OFFICE, COLLATERAL_MGMT, ALTS. Each has `product_family`, `regulatory_framework`, `kyc_context`. |
| 2. **Services** | `"ob-poc".services` | Capabilities that compose products: SAFEKEEPING, CASH_MGMT, SETTLEMENT, CORP_ACTIONS, REPORTING, REG_REPORTING, NAV_CALC, INVESTOR_ACCT. Has `lifecycle_tags` array, `sla_definition` JSON. |
| 3. **SRDEFs** | `"ob-poc".service_resource_types` | Typed infrastructure definitions with `srdef_id` generated as `SRDEF::<OWNER>::<ResourceType>::<Code>`. E.g. `SRDEF::CUSTODY::Account::custody_securities`, `SRDEF::SWIFT::Connection::swift_messaging`. |

### Composition junction

| Table | Purpose |
|---|---|
| `"ob-poc".product_services` | **This is the product-manifest I proposed to invent in pass 4.** `is_mandatory`, `is_default`, per-link `configuration` JSONB. A product's services are the capabilities it MUST deliver. |

### Per-CBU state

| Table | Purpose |
|---|---|
| `"ob-poc".service_intents` | **This is the "lifecycle services profile" I proposed in pass 5.** `cbu_id, product_id, service_id, options jsonb, status (active/suspended/cancelled)`. One row per (CBU, product, service). Options JSONB carries markets / currencies / instrument_classes / etc. UNIQUE on (cbu_id, product_id, service_id). |
| `"ob-poc".cbu_resource_instances` | Per-CBU provisioned instances of SRDEFs. |
| `"ob-poc".cbu_service_readiness` | Computed "good-to-transact" status per CBU+service. |
| `"ob-poc".cbu_unified_attr_requirements` | Rolled-up attribute requirements per CBU. |
| `"ob-poc".cbu_attr_values` | Sourced attribute values per CBU. |

### SRDEF parameterization

SRDEFs declare `per_market boolean`, `per_currency boolean`, `per_counterparty boolean`
flags. When a CBU's service_intent.options has `markets: ["XNAS", "XNYS", "XLON"]`
and the SRDEF has `per_market = true`, the Discovery Engine expands to 3 SRDEF
instances — one per market. M × N × K when multiple flags set.

### Six-stage pipeline

```
1. Intent         → service_intents (what CBU wants)
2. Discovery      → srdef_discovery_reasons (SRDEF expansion by parameterization)
3. Rollup         → cbu_unified_attr_requirements (merge attr requirements)
4. Population     → cbu_attr_values (source values with evidence)
5. Provisioning   → cbu_resource_instances (create/request/discover)
6. Readiness      → cbu_service_readiness (good-to-transact status)
```

### Attribute Registry

`"ob-poc".attribute_registry` — closed-world dictionary. Every attribute has
`applicability` JSON (CSG rules — conditional on context) + `validation_rules` +
reconciliation rules. `category` ∈ {identity, financial, compliance, document,
risk, contact, address, tax, employment, product, ...}.

---

## 2. Mapping pass-4/5 concepts to existing entities

Pass 4 and pass 5 proposed concepts that MAP directly to existing infrastructure:

| Pass-4/5 concept | Actual existing entity | Notes |
|---|---|---|
| "Product catalogue" | `products` table | 7 products defined + metadata |
| "Product manifest" (what config each product requires) | `product_services` junction | `configuration` JSONB per link is the manifest content |
| "Lifecycle services profile" | CBU's set of `service_intents` rows | A CBU's profile IS the set of (product_id, service_id, options) tuples it's subscribed to |
| "Per-CBU effective DAG" | Output of Discovery Engine (Stage 2) | Parameter expansion of service_intents.options against SRDEFs with per_* flags |
| "`requires_products:` on verb/slot" | Already implicit in `product_services` + `service_intents` | A verb operates on an intent whose product/service is in the CBU's intent set |
| "CBU product enrolment" (pass-4 Q-AC) | `service_intents` rows per CBU | Enrolment IS the service_intent — no separate slot needed |
| "Product catalogue to be built" (pass-4 Q-AD) | Already built | Exists at `products` + `services` + `service_resource_types` |
| "Attribute-level gating" (pass-5 §2.2) | `attribute_registry.applicability` + CSG rules | The existing rule model for conditional attribute requirements |

**Net:** everything pass-4/5 proposed as *new* already exists. The insight was right;
the naming was parallel rather than aligned.

---

## 3. Corrected terminology (for P.2 / P.3 / P.9)

Going forward, use existing names:

- ~~"product catalogue"~~ → **`products` table**
- ~~"lifecycle services profile"~~ → **CBU's `service_intents` set**
- ~~"product manifest"~~ → **`product_services` junction with `configuration` JSONB**
- ~~"profile registry"~~ → (not needed; service_intents is the per-CBU record)
- ~~"`requires_products:`"~~ → (not needed as new field; verbs that operate on `service_intents` already scope to products via FK)

Pass-5 questions U-1 through U-4 get resolved:

- **U-1 (do profiles have lifecycle):** `service_intents.status` has 3 states
  (active, suspended, cancelled) — CBU-level enrolment lifecycle is already there.
- **U-2 (CBU profile enrolment storage):** already `service_intents` per-CBU rows.
- **U-3 (product/profile authoring):** `products` + `services` + `service_resource_types`
  are CRUD tables with verbs already declared in the pack (`service-resource.*`,
  `product.*`, etc.). Authoring lives in SemOsMaintenance workspace.
- **U-4 (`requires_all_of` vs `requires_any_of`):** resolved by the existing
  `product_services.is_mandatory` flag. Mandatory services MUST be active for
  product to be "ready"; optional ones don't gate.

---

## 4. What the Instrument Matrix DAG actually contributes

Given the existing architecture, the IM DAG's contribution is narrower than
passes 4/5 suggested. Not "product catalogue + profile registry" (already there).
The DAG contributes:

### 4.1 Lifecycle semantics for existing entities

Existing tables have states (service_intents.status, cbus.status, products.is_active,
trading_profiles.status) but **no formal state machines declared**. The DAG
taxonomy YAML (P.2 authors) formalises:
- Valid transitions between states.
- Which verbs trigger which transitions.
- Cross-entity state constraints (e.g. mandate can't activate unless CBU validated).

This is the real content of the DAG YAML.

### 4.2 Three-axis declarations on verbs

P.3's core contribution. Every verb operating on the existing tables gets
state_effect + external_effects + consequence declared. Existing verbs don't
carry these axes today.

### 4.3 Runbook composition rules (Components A/B/C)

P12 rules over the verb set. The three-axis declarations feed this.

### 4.4 Validator + catalogue-load gate

P.1 delivers the mechanism. The existing tables supply the data; the DAG
supplies the declarative structure.

---

## 5. Revised pilot scope — what stays, what drops

### What stays (unchanged)

- 21 slots from pass 3. They map to real entities:
  - `trading_profile` (template + streetside) → `cbu_trading_profiles` table
  - `settlement_pattern` → `cbu_settlement_chains` + child tables
  - `cbu` → `cbus` table
  - `service_resource` → `service_resource_types` (SRDEFs)
  - `service_intent` → `service_intents` table (already exists, already has state)
  - `delivery` → `service_delivery_map` table
  - `custody` → `cbu_custody_ssis` / `entity_settlement_identity`
  - `booking_principal` → `booking_principal` table
  - `cash_sweep` → `cbu_cash_sweep_config`
  - `booking_location` → `booking_location` table
  - `legal_entity` → `legal_entity` table
  - `product` → `products` table (core catalogue entity)
  - `isda_framework` → `isda_agreements`
  - (etc.)
- ~56 states to author in P.2.
- ~210 verbs to declare in P.3.
- P.1 infrastructure (schema + validator + composition + startup gate + lint).

### What drops from pass-4/5 proposals

- **No new product catalogue** (`products` exists).
- **No product manifest YAML** (`product_services.configuration` exists).
- **No `lifecycle_services_profile_registry`** (CBU's `service_intents` set IS
  the profile).
- **No `requires_products:` field** on verb YAML (existing `service_intents`
  linkage to `product_id` carries the info).

### What new-for-pilot after pass-6 realignment

**One thing.** Ensure the `instrument_matrix_dag.yaml` authored in P.2 references
the existing entities correctly:
- State machines for existing `*.status` columns (service_intents, cbu_trading_profiles,
  cbus, etc.) declared with their real column/table names.
- Verb-to-entity references using the real `product_id` / `service_id` linkage.
- Cross-slot constraints reference real join paths, not invented ones.

This is P.2-author careful-work, not new code.

---

## 6. What "conditional node reachability" (pass 5) maps to in the existing model

Pass 5's reachability model maps to the existing Discovery Engine (Stage 2):

> *"a DAG has conditional node sets — depending on CBU and product
> lifecycle_services profiles"*

In the existing architecture:
- A CBU's `service_intents` rows declare which products + services the CBU has.
- The **Discovery Engine** (Stage 2) expands those intents against SRDEFs +
  dependencies to produce the CBU's actual infrastructure set.
- An SRDEF not discovered for that CBU is **unreachable** — no intent links to
  it, nothing gets provisioned.
- An attribute in the registry whose `applicability` rules don't match the CBU's
  intent options is **not required** for that CBU.

So "conditional node reachability" in DAG terms = "not discovered by the
Discovery Engine given this CBU's intents." Existing mechanism.

The DAG's role: declare what the discovery engine SHOULD produce, i.e. the
lifecycle states + transitions that correspond to each discovery outcome.

---

## 7. Key architectural statement (revised)

**The Instrument Matrix DAG is the declarative lifecycle + semantic layer over the
existing products/services/SRDEFs/service_intents pipeline.**

The pipeline supplies *what* (data, provisioning, attributes). The DAG supplies
*how this data transitions* (state machines, valid transitions, three-axis verb
semantics, runbook composition rules).

Neither replaces the other. The DAG is governance/semantics; the pipeline is
execution.

---

## 8. Pilot actions — revised

Given this alignment:

**P.2 author:**
- Reference existing tables by name in the DAG YAML (e.g.
  `source_entity: "ob-poc".cbu_trading_profiles`).
- Declare state machines over the REAL status columns, not idealised ones.
- Cross-slot constraints reference real join paths (e.g.
  "trading_profile.active requires cbus.status = VALIDATED" ←
  `cbu_trading_profiles.cbu_id → cbus.cbu_id → cbus.status`).

**P.3 author:**
- For each verb, the `three_axis` declaration operates on real tables. Verify
  verb's DB mutation matches the declared `state_effect`.
- No `requires_products:` field needed — product gating is implicit via
  `service_intents`.

**P.9 findings:**
- Document the alignment: DAG is declarative lifecycle over existing pipeline.
- Correct passes 4 + 5 to use existing terminology.
- Remove suggestions to build product catalogue / profile registry.
- Keep architectural insights: product-modularity, conditional reachability,
  three-layer (DAG / Service Resources / Operations) — all valid, map to
  existing infra.

---

## 9. Questions for Adam (pass-6 gate)

**Q-BA.** Is this alignment accurate? Specifically: the pilot DAG YAML declares
lifecycle state machines over the existing `*.status` columns (cbus,
service_intents, cbu_trading_profiles, etc.) — right entity?

**Q-BB.** Is there an existing named-bundle concept (beyond the raw
products/services tables) that I should map "lifecycle services profile" to, or
is CBU's `service_intents` set the canonical answer?

**Q-BC.** For the pilot, is the in-scope product bundle *all 7 products* (full
pipeline covered) or a narrow subset (e.g. custody + FA only)?

---

**End of pass 6.** This closes the architectural alignment. Pass 4+5 insights
stand but map to existing entities; no new registries/catalogues needed for
pilot or estate scale.
