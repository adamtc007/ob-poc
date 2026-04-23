# Instrument Matrix — Slot Inventory Ledger (A-1 v2, 2026-04-23)

> **Version history.**
> - **v1 (superseded, same file earlier today):** classified slots on DSL surface
>   evidence alone — "no lifecycle-sounding verb names → stateless." Adam
>   correctly flagged the methodological flaw: DAG is the entity state
>   **ontology**; DSL is the *currently-exposed* subset of operations over
>   those states. Missing verbs ≠ entity stateless.
> - **v2 (this doc):** schema-calibrated. For each slot, evidence is:
>   (1) the actual `status` / `state` / `is_active` columns in the
>   `"ob-poc".*` schema; (2) the pack verb set's coverage over those
>   states; (3) the gap between schema-implied states and what the DSL
>   can currently reach.
>
> **Still provisional** pending Adam's domain pass on LOW-confidence rows.

> **Purpose:** pre-P.2 audit artefact. Feeds P.2 (DAG taxonomy YAML) with
> domain-grounded state models, not inferred-from-verb-names guesses.
>
> **Parent docs:** pilot plan `instrument-matrix-pilot-plan-2026-04-22.md`,
> break remediation `instrument-matrix-dag-dsl-break-remediation-2026-04-23.md`.

---

## 1. Methodological correction

Per v1.1 P1 + P10, **the DAG is the entity state ontology** — independent of
what verbs our current DSL happens to expose. A slot is genuinely stateless
only when the entity itself has no meaningful states in the business domain.

**Two kinds of evidence differ:**

- **Schema evidence** — `status` / `state` / `is_active` columns with CHECK
  constraints or ENUM types. Authoritative for the states the **persisted
  model** recognises.
- **Domain evidence** — the real-world state model of the entity (legal
  lifecycle, ops lifecycle, regulatory lifecycle). Frequently richer than
  the schema. Requires domain knowledge I don't have at Adam's depth.

**Classification axis changes from v1 to v2:**

- `declare-stateless` now requires **both** (schema has no state column)
  **and** (no plausible domain lifecycle). v1 only required the former.
- `new-state-machine` now distinguishes:
  - **Schema-confirmed** — states verified in the DB schema.
  - **Schema-minimal, domain-rich** — DB has only `is_active` or binary
    status, but the entity has real business states. Adam review required.
- Per-row `confidence: high | medium | low` column added. LOW means Adam
  must confirm the state set before P.2 author can proceed.

---

## 2. Delta summary — v1 → v2

| Slot | v1 call | v2 call | Driver of change |
|---|---|---|---|
| `group` | declare-stateless | **new-state-machine (schema-confirmed)** | `client_group.discovery_status` has 5 states: not_started, in_progress, complete, stale, failed |
| `workspace_root` | declare-stateless | confirmed **declare-stateless** | No DB table; genuinely a projection |
| `cbu` | declare-stateless | **new-state-machine (schema-confirmed)** | `cbus.status` has 5 states: DISCOVERED, VALIDATION_PENDING, VALIDATED, UPDATE_PENDING_PROOF, VALIDATION_FAILED |
| `service_intent` | declare-stateless | **new-state-machine (schema-confirmed)** | `service_intents.status` has 3 states: active, suspended, cancelled |
| `legal_entity` | declare-stateless | **new-state-machine (schema-minimal, domain-rich)** | Schema: active/inactive. Domain: incorporated/active/dormant/dissolved/struck-off likely. Adam confirms. |
| `product` | declare-stateless | **new-state-machine (schema-minimal, domain-rich)** | Schema: is_active. Domain: draft/approved/active/withdrawn/retired likely. Adam confirms. |
| `isda_framework` | declare-stateless | **new-state-machine (schema-minimal, domain-rich)** | Schema: is_active only. Domain: negotiated/executed/amended/terminated almost certainly. Adam confirms. |
| `custody` | declare-stateless | confirmed **declare-stateless** | No dedicated table; compositional slot projecting from entity-level SSIs. |
| `booking_location` | declare-stateless | confirmed **declare-stateless** | No state column; pure reference data. |
| `corporate_action_policy` | new-state-machine | **declare-stateless (schema-inferred)** | No CA policy table in schema; slot appears to be a compositional projection of `trading_profile.ca.*` attributes, not a standalone entity. |
| `trade_gateway` | new-state-machine | **declare-stateless (schema-inferred)** | No trade_gateway table in schema; verbs operate on configurations, not on a persistent gateway entity. |
| `delivery` | new-state-machine (3 states) | confirmed, but **5 states, not 3** | `service_delivery_map.delivery_status`: PENDING, IN_PROGRESS, DELIVERED, FAILED, CANCELLED |
| `trading_profile` (streetside) | reconcile-existing | confirmed **reconcile-existing** | `cbu_trading_profiles.status` + 4 timestamp columns encode the existing `trading_profile_lifecycle`. |
| `trading_profile` (template) | new-state-machine (3 states) | **same-entity-as-streetside** | `cbu_trading_profiles` is the only table; template and streetside share it. v2 collapses the two rows — template/streetside is a **row-level distinction** (cbu_id NULL vs non-NULL), not a separate state machine. |
| `settlement_pattern` | new-state-machine (4 states) | **LOW-confidence; probably stateful but schema is only is_active** | Adam: does settlement-chain have a real lifecycle (draft → live → deactivated) or just an on/off switch? |
| `booking_principal` | new-state-machine (3 states) | confirmed schema-minimal (binary active/inactive) + DSL has `retire` | 3-state model (created/active/retired) is consistent with DSL verbs but schema collapses to binary. Confirm. |
| `cash_sweep` | new-state-machine (4 states) | confirmed schema-minimal (is_active) + DSL has suspend/reactivate/remove | State model is DSL-implied, not schema-persisted. Adam: are these real states or just flag permutations? |
| `service_resource` | new-state-machine (4 states) | confirmed schema-minimal (is_active + provisioning_strategy) + DSL has activate/suspend/decommission | Same pattern as cash_sweep. Adam confirms. |

**Net delta: 6 slots flipped stateless → stateful; 2 flipped stateful → stateless; 4 slot definitions refined in state count or cardinality.**

---

## 3. Schema evidence + v2 classification per slot

### 3.1 Instrument.workspace constellation

| Slot | Schema evidence | Domain states (reality) | v2 classification | Confidence |
|---|---|---|---|---|
| `workspace_root` | No DB table. | None — root is a projection/aggregation concept, not an entity. | **declare-stateless** | **HIGH** |

### 3.2 Instrument.template constellation

| Slot | Schema evidence | Domain states (reality) | v2 classification | Confidence |
|---|---|---|---|---|
| `group` | `client_group.discovery_status`, 5 states via CHECK: `not_started, in_progress, complete, stale, failed`. | Discovery lifecycle (research → confirmed → stale as facts age). Well-modelled by schema. | **new-state-machine** — states: `not_started, in_progress, complete, stale, failed` | **HIGH** (schema-confirmed) |
| `trading_profile` (template) | Shares `cbu_trading_profiles` table with streetside; distinguished by `cbu_id IS NULL`. | Template lifecycle likely: draft → validated → published → superseded. Simpler than streetside (no submit/approve per-CBU). | **same entity, separate row class** — lifecycle may be distinct from streetside | **MEDIUM** (Adam to confirm whether templates have their own approval lifecycle or inherit the streetside full set) |
| `settlement_pattern` | `cbu_settlement_chains.is_active` boolean. `settlement_chain_hops.role` and `settlement_locations.location_type` are categorical (role kind, location kind) — not lifecycle. No history table. | Settlement chains likely have: proposed → configured → live → deactivated → archived. Regulatory / ops-review likely. | **new-state-machine (schema-minimal, domain-rich)** — tentative states: `draft, configured, active, deactivated` | **LOW** (Adam to confirm real settlement-chain lifecycle — is there an ops-approval step before a chain goes live?) |
| `isda_framework` | `isda_agreements.is_active` boolean only. No CSA lifecycle column. | ISDA agreements in practice: negotiated → signed → executed → amended → terminated. CSAs nest within with their own active/terminated state. ISDA absolutely has states in the domain. | **new-state-machine (schema-minimal, domain-rich)** — tentative states: `draft, negotiated, executed, amended, terminated` | **LOW** (Adam: is ISDA lifecycle in scope for our model or is it sourced from counterparty docs and we treat it as read-only once created?) |
| `corporate_action_policy` | No dedicated table. CA behaviour stored as attributes on `cbu_trading_profiles` (the `trading-profile.ca.*` verbs) and events/preferences as rows on child tables. | Policy itself is a compositional view over attributes, not a lifecycle entity. | **declare-stateless** | **MEDIUM** (revisit if CA policy has its own approval lifecycle we haven't discovered) |
| `trade_gateway` | No dedicated table. Gateway verbs (`trade-gateway.*`) operate on configurations held elsewhere. | Gateways have real lifecycle (defined → enabled → active → suspended → retired) but it may be stored via booking_principal status or similar. | **declare-stateless OR new-state-machine (depending on where gateway state actually lives)** | **LOW** (Adam: where is `trade_gateway.status` actually persisted? Inline on booking_principal? A table I missed?) |

### 3.3 Trading.streetside constellation

| Slot | Schema evidence | Domain states (reality) | v2 classification | Confidence |
|---|---|---|---|---|
| `cbu` | `cbus.status`, 5 states via CHECK: `DISCOVERED, VALIDATION_PENDING, VALIDATED, UPDATE_PENDING_PROOF, VALIDATION_FAILED`. | CBU discovery + validation lifecycle. Schema-complete for this purpose; CBU also has suspend/archive but those may be on a separate flag not captured in status. | **new-state-machine** — states: `DISCOVERED, VALIDATION_PENDING, VALIDATED, UPDATE_PENDING_PROOF, VALIDATION_FAILED` (+ possibly suspended/archived — to confirm) | **HIGH** for the 5 schema states; **MEDIUM** for whether suspend/archive belong |
| `trading_profile` (streetside) | `cbu_trading_profiles.status` + timestamp columns: `validated_at, submitted_at, rejected_at, superseded_at`. Existing state machine `trading_profile_lifecycle` in `trading_streetside.yaml` has 7 states. | Matches inline definition. | **reconcile-existing** — lift 7-state `trading_profile_lifecycle` to formal DAG taxonomy YAML | **HIGH** |
| `custody` | No dedicated table. Custody SSIs live on `entity_settlement_identity` keyed to entities, not CBUs. | Custody is a **compositional projection** — the slot surfaces "entity settlement identity" data scoped to the current CBU's counterparties, not its own entity. Statelessness is genuine for the slot; the underlying SSIs themselves do have states on `entity_settlement_identity`. | **declare-stateless** (slot) | **MEDIUM** (confirm slot is purely a read view, not something that can itself be activated/suspended) |
| `booking_principal` | `booking_principal.status`, 2 states via CHECK: `active, inactive`. | Domain: booking principals are bank-side legal entities with lifecycle (onboarded → active → retired). Schema collapses to binary. DSL has `retire` verb → 3-state intent. | **new-state-machine (schema-minimal, domain-rich)** — tentative: `active, retired` (with "onboarding" maybe pre-active) | **MEDIUM** (Adam: confirm whether "retired" == "inactive" or whether there's a meaningful distinction between pre-onboarding vs post-retirement inactive) |
| `cash_sweep` | `cbu_cash_sweep_config.is_active` boolean + categorical config columns (sweep_frequency, vehicle_type, interest_allocation — these are attribute choices, not states). | Cash sweep lifecycle: configured → active → suspended (short-term pause) → retired (permanent). | **new-state-machine (schema-minimal, domain-rich)** — tentative: `configured, active, suspended, retired` | **MEDIUM** (DSL has suspend/reactivate/remove; confirm domain distinction between suspend and remove) |
| `service_resource` | `service_resource_types.is_active` + `provisioning_strategy` (create/request/discover — strategy choice, not state). | Service resource lifecycle: provisioned → active → suspended → decommissioned. | **new-state-machine (schema-minimal, domain-rich)** — tentative: `provisioned, active, suspended, decommissioned` | **MEDIUM** (DSL verbs suggest the 4-state model; confirm states are real) |
| `service_intent` | `service_intents.status`, 3 states via CHECK: `active, suspended, cancelled`. | Matches schema. | **new-state-machine** — states: `active, suspended, cancelled` | **HIGH** (schema-confirmed) |
| `booking_location` | No state column; only timestamps. | Booking locations are static reference (cities/jurisdictions); no lifecycle. | **declare-stateless** | **HIGH** |
| `legal_entity` | `legal_entity.status`, 2 states via CHECK: `active, inactive`. | Legal entities in reality have: incorporated → active → dormant → dissolved / struck-off. Plus regulatory states (compliant, under investigation). Schema collapses to binary. | **new-state-machine (schema-minimal, domain-rich)** — tentative: `active, inactive` per schema; domain may need richer set | **LOW** (Adam: is "active/inactive" the right granularity for our use, or do we need incorporated/dissolved distinction for KYC downstream?) |
| `product` | `products.is_active` boolean only. | Products have lifecycle: draft → approved → active → deprecated → retired. | **new-state-machine (schema-minimal, domain-rich)** — tentative: `active, inactive` per schema; domain richer | **LOW** (Adam: do products in the Instrument Matrix context have their own lifecycle or inherit from trading-profile approval flow?) |
| `delivery` | `service_delivery_map.delivery_status`, 5 states via CHECK: `PENDING, IN_PROGRESS, DELIVERED, FAILED, CANCELLED`. Plus timestamps. | Schema-complete. | **new-state-machine** — states: `PENDING, IN_PROGRESS, DELIVERED, FAILED, CANCELLED` | **HIGH** (schema-confirmed; more states than v1's 3-state guess) |

---

## 4. Summary by classification

| Classification | Count | Slots |
|---|---|---|
| **declare-stateless (HIGH)** | 2 | `workspace_root`, `booking_location` |
| **declare-stateless (MEDIUM)** | 2 | `custody`, `corporate_action_policy` |
| **declare-stateless (LOW)** | 1 | `trade_gateway` (pending Adam input on where state lives) |
| **new-state-machine (HIGH — schema-confirmed)** | 4 | `group`, `cbu`, `service_intent`, `delivery` |
| **new-state-machine (MEDIUM)** | 5 | `trading_profile` (template), `booking_principal`, `cash_sweep`, `service_resource`, `cbu` (suspend/archive question) |
| **new-state-machine (LOW — schema-minimal, domain-rich)** | 4 | `settlement_pattern`, `isda_framework`, `legal_entity`, `product` |
| **reconcile-existing** | 1 | `trading_profile` (streetside) |

**18 slot-rows total** (same as v1). **9 LOW/MEDIUM rows require Adam review** before P.2 authoring can proceed with confidence.

---

## 5. Questions for Adam (P.2 gating)

### LOW-confidence rows requiring domain input

**Q-A1.1 (settlement_pattern).** Does a settlement chain have a real
pre-activation lifecycle (draft → reviewed → live), or does creating a
chain make it immediately live with just an is_active toggle?

**Q-A1.2 (isda_framework).** Is ISDA lifecycle in scope for us to model,
or do we treat ISDA agreements as opaque counterparty contracts where
the only state we track is "do we have coverage, yes or no"?

**Q-A1.3 (trade_gateway).** Where is gateway state actually persisted?
If not in a dedicated table, is it a boolean on booking_principal, or
something else?

**Q-A1.4 (legal_entity).** Is the binary active/inactive enough, or do
downstream KYC / compliance flows need incorporated / dissolved /
struck-off distinctions?

**Q-A1.5 (product).** Products in Instrument Matrix — own lifecycle
(draft/approved/active/retired) or inherited from trading-profile
approval flow?

### MEDIUM-confidence rows — sanity checks

**Q-A1.6 (trading_profile template).** Template lifecycle same as streetside
or simpler (no submit/approve/reject-per-CBU)?

**Q-A1.7 (cbu suspend/archive).** Are CBU suspend and archive states
missing from the `cbus.status` CHECK list, or are they held on another
column?

**Q-A1.8 (booking_principal retirement).** Schema is binary active/inactive;
DSL has `retire` verb. Is retirement a terminal state distinct from
generic "inactive", or are they synonyms?

**Q-A1.9 (cash_sweep / service_resource suspend vs remove).** For both,
is there a real domain distinction between `suspended` (reversible pause)
and `retired/decommissioned` (terminal), or is it "inactive with different
labels"?

---

## 6. Impact on P.2 effort estimate

v1 predicted 29 new states + 1 lifted machine (7 states) = 36 states total.
v2 recalibrates:

| Bucket | Slot count | States (v2 estimate) |
|---|---|---|
| Schema-confirmed HIGH | 4 | 5 (group) + 5 (cbu) + 3 (service_intent) + 5 (delivery) = **18** |
| Domain-rich LOW (conservative estimate 3–5 states each) | 4 | ~16 |
| MEDIUM (likely 3–4 states each) | 5 | ~17 |
| Reconcile-existing | 1 | 7 (trading_profile_lifecycle) |
| Stateless | 4 | 0 |
| **Total** | **18 slots** | **~58 states (± 10 depending on Adam's LOW-confidence answers)** |

P.2 effort was estimated at 3 days (2–5 range). Schema-calibrated state
count is ~60% higher than v1's 36-state estimate. Re-estimate:

- 58 states × ~10 min = ~10 hours authoring.
- Reconciliation of trading_profile_lifecycle: 2 hours.
- Adam review of 9 LOW/MEDIUM rows: ~3 hours (can parallelise with
  authoring).
- Consistency check + YAML alignment: ~4 hours.
- **Total: ~3 days** — still inside the original 2–5 range, but closer to
  the upper end.

---

## 7. Exit criterion for A-1 v2

A-1 v2 is **CLOSED** when:

- [x] Every slot has a schema-evidence row.
- [x] Every slot has a v2 classification + confidence.
- [ ] All 9 LOW/MEDIUM questions in §5 have Adam answers.
- [ ] State-count recalibration from §6 is confirmed / adjusted.

Until the last two are done, the ledger is **OPEN (pending domain input)**.
P.2 authoring can START against the 5 HIGH-confidence rows but cannot
complete without Adam's responses to §5.

---

## 8. Methodological note for future audits

This v1→v2 correction proves a general rule for any future DAG↔DSL audit
in this codebase: **classify from the persisted schema + known domain
ontology, not from the current verb surface.** Verb surfaces are
operationally incomplete; schemas are closer to ground truth; domain
reality is richer still than either. Rank-order of evidence:

1. **Domain ontology** (authoritative — Adam's knowledge).
2. **DB schema with CHECK / ENUM constraints** (authoritative for what's
   persistable).
3. **Verb surface coverage** (operational — shows what we've built, not
   what exists).

A-1 v1 used (3) alone. A-1 v2 uses (2) + (3) and flags (1) gaps for
Adam. Future audits should target (1) + (2) + (3) in that order.
