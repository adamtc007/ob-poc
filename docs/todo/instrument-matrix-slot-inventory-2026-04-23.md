# Instrument Matrix — Slot Inventory Ledger (A-1 v3, 2026-04-23)

> **Status:** CLOSED. All 9 domain questions from v2 answered by Adam
> 2026-04-23. This is the authoritative slot inventory feeding P.2 DAG
> taxonomy authoring.
>
> **Version history.**
> - **v1** (superseded): classified from DSL surface evidence alone. Method
>   flaw — missing lifecycle verbs ≠ stateless entity.
> - **v2** (superseded): schema-calibrated; 6 classifications flipped; 9
>   LOW/MEDIUM rows queued for Adam domain review.
> - **v3 (this doc):** Adam's 9 answers applied. 5 LOW/MEDIUM flips
>   consolidated, 3 new findings captured (schema migration, DSL
>   over-modeling, hybrid persistence pattern).
>
> **Parent docs:** pilot plan `instrument-matrix-pilot-plan-2026-04-22.md`,
> break remediation `instrument-matrix-dag-dsl-break-remediation-2026-04-23.md`.

---

## 1. Adam's answers (verbatim captures)

| Q | Slot | Adam's answer | v3 effect |
|---|---|---|---|
| Q1 | settlement_pattern | "it has a pre-activation lifecycle" | new-state-machine, real states |
| Q2 | isda_framework | "yes/no" — coverage-only binary | **declare-stateless** |
| Q3 | trade_gateway | "(d) planned but not built — intention was instruction matrix as a document == json; need to consider options"; follow-up: **hybrid for now** (is_active column for query speed, authoritative state inside JSON document) | new-state-machine (planned), hybrid persistence documented |
| Q4 | legal_entity | "binary — KYC is separate and more granular" | **declare-stateless** from IM perspective (KYC workspace owns rich granularity) |
| Q5 | product | "inherit from the main document" (trading-profile) | **declare-stateless** — products inherit profile lifecycle |
| Q6 | trading_profile (template) | "they are templates — either active/available or not" | new-state-machine, 2 states (available / unavailable) |
| Q7 | cbu suspend/archive | "add suspended and archived to the CHECK list states" | **schema migration required**; new-state-machine now 7 states |
| Q8 | booking_principal | "rules looking at client/cbu profiles and products — active/inactive seems not required" | **declare-stateless** — it's a rule-evaluation entity, not a stateful entity |
| Q9a | cash_sweep | "either active or not" | **declare-stateless** — binary flag, not a lifecycle |
| Q9b | service_resource | "refers to lifecycle servicing resources — provisioned / activated / suspended / decommissioned" | new-state-machine, 4 states HIGH confidence |

---

## 2. Final classifications (v3)

### 2.1 Instrument.workspace constellation

| Slot | v3 classification | States | Confidence |
|---|---|---|---|
| `workspace_root` | **declare-stateless** | — | HIGH |

### 2.2 Instrument.template constellation

| Slot | v3 classification | States | Confidence |
|---|---|---|---|
| `group` | **new-state-machine** | `not_started, in_progress, complete, stale, failed` (5) | HIGH (schema-confirmed) |
| `trading_profile` (template) | **new-state-machine** | `available, unavailable` (2) | HIGH (Q6 confirmed) |
| `settlement_pattern` | **new-state-machine** | `draft, configured, reviewed, live, deactivated` proposed (4–5) | HIGH that lifecycle exists (Q1); state names to finalise in P.2 |
| `isda_framework` | **declare-stateless** | — | HIGH (Q2 — coverage-only binary, not a lifecycle entity from IM perspective) |
| `corporate_action_policy` | **declare-stateless** | — | HIGH (no table; compositional projection of `trading-profile.ca.*` attributes) |
| `trade_gateway` | **new-state-machine (planned)** | `defined, enabled, active, suspended` proposed (4) | MEDIUM — states exist in domain; persistence hybrid (see §3.3) |

### 2.3 Trading.streetside constellation

| Slot | v3 classification | States | Confidence |
|---|---|---|---|
| `cbu` | **new-state-machine** | `DISCOVERED, VALIDATION_PENDING, VALIDATED, UPDATE_PENDING_PROOF, VALIDATION_FAILED, SUSPENDED, ARCHIVED` (7) | HIGH (5 schema-confirmed + 2 added per Q7; migration required — see §3.1) |
| `trading_profile` (streetside) | **reconcile-existing** | `draft, submitted, approved, active, suspended, archived, rejected` (7 — existing `trading_profile_lifecycle`) | HIGH |
| `custody` | **declare-stateless** | — | HIGH (no dedicated table; slot is a view over `entity_settlement_identity` scoped to CBU counterparties) |
| `booking_principal` | **declare-stateless** | — | HIGH (Q8 — rule-evaluation entity, not stateful) |
| `cash_sweep` | **declare-stateless** | — | HIGH (Q9a — binary flag, DSL over-modeling — see §3.2) |
| `service_resource` | **new-state-machine** | `provisioned, activated, suspended, decommissioned` (4) | HIGH (Q9b confirmed) |
| `service_intent` | **new-state-machine** | `active, suspended, cancelled` (3) | HIGH (schema-confirmed) |
| `booking_location` | **declare-stateless** | — | HIGH (no state column; reference data) |
| `legal_entity` | **declare-stateless** | — | HIGH (Q4 — binary is enough for IM; KYC workspace owns granular lifecycle) |
| `product` | **declare-stateless** | — | HIGH (Q5 — inherits from trading-profile lifecycle) |
| `delivery` | **new-state-machine** | `PENDING, IN_PROGRESS, DELIVERED, FAILED, CANCELLED` (5) | HIGH (schema-confirmed) |

---

## 3. Findings captured during v2 → v3 calibration

### 3.1 Schema migration required — `cbus.status`

Adam's answer to Q7 requires extending `cbus.status` CHECK constraint:

```sql
-- Current:
CHECK (status IN ('DISCOVERED', 'VALIDATION_PENDING', 'VALIDATED',
                  'UPDATE_PENDING_PROOF', 'VALIDATION_FAILED'))
-- Required:
CHECK (status IN ('DISCOVERED', 'VALIDATION_PENDING', 'VALIDATED',
                  'UPDATE_PENDING_PROOF', 'VALIDATION_FAILED',
                  'SUSPENDED', 'ARCHIVED'))
```

**Deliverable:** a forward-only migration
`rust/migrations/<date>_extend_cbu_status_check.sql` adding the two
values. Pre-pilot migration — lands before P.3 declares any CBU
transition verb that targets `suspended` / `archived`.

**Sequencing:** migration must land before P.3's CBU declaration work; can
land any time after A-1 v3 closes. Suggest bundling with P.2 DAG taxonomy
commit since P.2 YAML will list the 7 states explicitly.

### 3.2 DSL over-modeling findings (→ v1.1 candidate amendments)

Four slots show DSL verbs that imply richer lifecycle than domain reality:

| Slot | DSL verbs suggesting lifecycle | Domain truth | Finding |
|---|---|---|---|
| `cash_sweep` | `suspend, reactivate, remove` (3 verbs → 3-state intent) | binary (active/not) | 3 verbs all toggle `is_active` — consider consolidating to `set-active` / `unset-active` |
| `booking_principal` | `retire` | rule entity, no lifecycle | `retire` is really "remove-rule"; rename for clarity |
| `product` | per-product CRUD | inherits from trading-profile | products shouldn't have independent lifecycle verbs |
| `legal_entity` | `create, list, read` only | binary per schema | matches — no over-modeling here |

These become entries in P.9 findings as v1.1 candidate amendments: "DSL
operational phrasings should be audited against domain state models to
avoid verb proliferation around binary flags."

### 3.3 Architectural decision — hybrid persistence for document-shaped slots

Adam (Q3 follow-up): hybrid model for `trade_gateway` and by extension any
"instruction matrix as JSON document" slot.

**The pattern:**
- Row-level column: `is_active` boolean — for query speed / indexing.
- Authoritative state: inside the JSON document body (`document.state =
  'active'`).
- State machine owned by the **document type definition**, not by the
  SQL CHECK constraint.

**Applies to (candidates):**
- `trade_gateway` (confirmed — Q3 answer).
- `instruction_profile` (possibly — same "instruction matrix as JSON"
  family; not yet schema-persisted anyway so hybrid is the clean default).
- `corporate_action_policy` (possibly — currently compositional; if
  formalised, would fit this pattern).

**Does NOT apply to (schema evidence):**
- `cbus`, `client_group`, `cbu_trading_profiles`, `service_intents`,
  `service_resource_types`, `service_delivery_map`, `legal_entity`,
  `booking_principal` — all have real `status` columns with CHECK / ENUM.
- `isda_agreements`, `booking_location`, `products` — declare-stateless
  in v3.

**Implication for P.2:** DAG taxonomy YAML covers state machines for
both patterns (column-persisted and JSON-persisted). Validator
`transitions.dag` reference works identically regardless of where the
state physically lives.

**Captured as v1.1 candidate amendment:** the three-axis declaration
and runbook composition model are agnostic to persistence shape. Add a
sentence to v1.1 §6.2 clarifying that `state_effect: transition` with
`transitions.dag: X` is valid for both SQL-column-backed and
JSON-document-backed state machines.

---

## 4. v3 summary

| Classification | Count | Slots |
|---|---|---|
| **declare-stateless (HIGH)** | 9 | workspace_root, isda_framework, corporate_action_policy, custody, booking_principal, cash_sweep, booking_location, legal_entity, product |
| **new-state-machine (HIGH)** | 6 | group (5), trading_profile-template (2), cbu (7), service_resource (4), service_intent (3), delivery (5) |
| **new-state-machine (HIGH states, name finalise in P.2)** | 1 | settlement_pattern (4–5) |
| **new-state-machine (planned, hybrid persistence)** | 1 | trade_gateway (4) |
| **reconcile-existing (HIGH)** | 1 | trading_profile-streetside (7) |
| **Total** | **18** | — |

**Total states across all slots:**
- HIGH schema-confirmed: 5 + 2 + 7 + 4 + 3 + 5 + 7 (reconcile) = **33 states**
- settlement_pattern to finalise: ~5 states
- trade_gateway planned: 4 states
- **Total: ~42 states**

Down from v2's 58 (several MEDIUM/LOW slots collapsed to stateless after
Adam's answers).

---

## 5. Impact on pilot phases

**P.2 DAG taxonomy effort (originally 2–5 day range):**
- 33 HIGH states + ~9 to-finalise = ~42 states × 10 min = 7 hours authoring.
- `trading_profile_lifecycle` reconciliation: 2 hours.
- CBU migration SQL + migration-commit: 1 hour.
- Doc-vs-column pattern documentation: 1 hour.
- Consistency review: 2 hours.
- **Total: ~1.5 days** — lower end of the range now that LOW/MEDIUM
  rows collapsed.

**P.3 three-axis declaration effort:** reduced scope — 9 stateless slots
mean their verbs need `state_effect: preserving` with empty transitions.
Faster to declare than stateful slots. No P.3 estimate change — stays
10–25 days.

**New pre-P.3 migration:** `cbus.status` CHECK extension. ~1 hour of
focused work; gate-sequential with P.2 taxonomy commit.

---

## 6. Deliverables from A-1 v3

1. **This ledger** — closed, authoritative input for P.2.
2. **Migration file** (to be written alongside P.2 commit):
   `rust/migrations/20260423_extend_cbu_status_check.sql` adding
   `SUSPENDED` and `ARCHIVED` to `cbus.status` CHECK.
3. **Three v1.1 candidate findings** for P.9 (Section 3 above):
   - DSL over-modeling audit methodology.
   - Document-as-JSON hybrid persistence pattern in three-axis model.
   - Schema-migration-required-for-pilot pattern (Q7 CBU states).

---

## 7. Exit criterion

A-1 v3 is **CLOSED** as of 2026-04-23. All 9 domain questions answered;
3 findings captured; schema-migration deliverable scoped. P.2 can begin
immediately against the full 18-slot classification.

Next artefact: A-2 (pack-delta disposition ledger) — closes break B-2
before P.3.
