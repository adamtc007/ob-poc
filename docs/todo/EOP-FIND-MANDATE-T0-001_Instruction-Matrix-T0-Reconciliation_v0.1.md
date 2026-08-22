# Instruction Matrix — T0 Reconciliation Findings

### EOP-FIND-MANDATE-T0-001

| | |
|---|---|
| **Document** | EOP-FIND-MANDATE-T0-001 |
| **Type** | Research findings — discharges T0 of EOP-PLAN-MANDATE-VANILLA-001 and T0 of EOP-PLAN-MANDATE-001 |
| **Version** | 0.1 |
| **Date** | 2026-08-20 |
| **Status** | Findings. No code, schema, or config was written or modified producing this document. |
| **Supersedes premises in** | EOP-VS-MANDATE-001 v0.3 §8, EOP-PLAN-MANDATE-001 v0.1 §A2, EOP-PLAN-MANDATE-VANILLA-001 v0.1 §1 |

---

## §0 How to read this document

Every factual claim is tagged:

- **[V]** — **VERIFIED.** Read directly in source, schema, or the live
  `data_designer` database, with a `path:line` citation or a reproducible
  `SELECT`.
- **[I]** — **INFERRED.** A judgement drawn from verified facts. Every one
  states what would settle it.

The prior planning session's error was presenting domain inference as
definition. This document does not repeat it: where the code does not
answer a question, §G says so and names what would.

**Method.** File reads; two parallel read-only codebase surveys; and
read-only `SELECT`s against the live `data_designer` database. Schema
ground truth is `migrations/master-schema.sql` (repo root), cross-checked
against the live catalogue — because, as §D2 and §D4 show, the two disagree
in ways that matter.

**Authority split, respected throughout [V]**
(`rust/config/sem_os_seeds/dag_taxonomies/instrument_matrix_dag.yaml:72-78`):
`instrument_template.yaml` + `trading_streetside.yaml` remain authoritative
for REPL/agent navigation; `instrument_matrix_dag.yaml` is authoritative for
**state-machine semantics only**. This document treats the DAG file as
authoritative for the board and grammar, and nothing else.

---

## §0.1 Headline

The drafted documents put the defect in the wrong place. The move grammar
is not missing — it exists, is ratified, and is implemented. What is broken
is narrower, more concrete, and currently **live**:

> **One JSONB column, `cbu_trading_profiles.document`, carries two
> mutually-undeserialisable Rust shapes.** All 417 tree-shaped profiles are
> `DRAFT`; all 16 `ACTIVE` profiles are flat. The read API parses `ACTIVE`
> rows as the tree type and therefore fails on every one of them today. The
> materialiser parses as the flat type and has **never executed** — zero
> recorded runs, ever.

That is the "one authority" law failing in production, not in principle.
Everything else in this document is context for it.

---

# §A The actual brick set and move grammar

Source of record: `rust/config/sem_os_seeds/dag_taxonomies/instrument_matrix_dag.yaml`
— 1548 lines, v1.3, R-4 phase-axis re-anchor dated 2026-04-24. **[V]**

Top-level structure **[V]** (`:84-1548`):

| Key | Line | Content |
|---|---|---|
| `version: "1.0"` | :84 | |
| `workspace: instrument_matrix` | :85 | |
| `dag_id: instrument_matrix_dag` | :86 | |
| `dsl_verb_reconciliation` | :88 | **19** named verb surfaces |
| `overall_lifecycle` | :281 | 8 derived phases |
| `slots` | :439 | **21** slots |
| `cross_slot_constraints` | :1225 | 10 rules |
| `cross_workspace_constraints` | :1326 | 1 rule |
| `product_module_gates` | :1351 | 11 always-on / 9 conditional |
| `out_of_scope` | :1398 | 14 exclusions |
| `prune_cascade_rules` | :1451 | 5 cascade rules |
| `prune_pre_validation` | :1540 | 2 required verbs, 4 abort conditions |

## A.1 The 19 verb surfaces (`dsl_verb_reconciliation`, `:88-270`) **[V]**

Enumerated in file order. **The two the drafted documents missed entirely
are marked ▶** — these are the real instruction-routing layer.

| # | Surface | Line | Verbs |
|---|---|---|---|
| 1 | `instrument_matrix_attach_surface` | :89 | `instrument-matrix.attach` |
| 2 | `instrument_macro_surface` | :91 | `instrument.activate-profile`, `.clone-profile`, `.configure-corporate-actions`, `.full-matrix-setup`, `.review-coverage`, `.setup-equity`, `.setup-fixed-income`, `.setup-funds`, `.setup-fx`, `.setup-listed-derivatives`, `.setup-money-market`, `.setup-otc-derivatives` (12) |
| 3 ▶ | **`settlement_chain_surface`** | :104 | `settlement-chain.create-chain`, `.deactivate-cross-border`, `.list-chains`, `.list-cross-border`, `.list-hops`, `.list-location-preferences`, `.list-locations`, `.remove-hop`, `.set-cross-border`, `.set-location-preference`, `.add-hop`, `.deactivate-chain`, `.define-location` (13) |
| 4 ▶ | **`trade_gateway_surface`** | :118 | `trade-gateway.add-routing-rule`, `.define-gateway`, `.list-cbu-gateways`, `.list-fallbacks`, `.list-gateways`, `.list-routing-rules`, `.read-gateway`, `.remove-routing-rule`, `.set-fallback`, `.activate-gateway`, `.enable-gateway`, `.suspend-gateway` (12) |
| 5 | `trading_profile_surface` | :131 | 32 verbs — document ops (`.add-component`, `.materialize`, `.import`, `.diff`, `.clone-to`, `.link-csa-ssi`, `.set-base-currency`, `.update-im-scope`, `.remove-component`, `.read`, `.get-active`, `.list-versions`, `.activate`), lifecycle (`.create-draft`, `.submit`, `.approve`, `.reject`, `.archive`, `.create-new-version`), CA policy (11 `.ca.*`), validation (`.validate-go-live-ready`, `.validate-universe-coverage`) |
| 6 | `cash_sweep_surface` | :165 | 9 |
| 7 | `cbu_custody_surface` | :175 | 8 |
| 8 | `corporate_action_surface` | :184 | 9 |
| 9 | `entity_settlement_surface` | :194 | 3 |
| 10 | `instruction_profile_surface` | :198 | 7 |
| 11 | `instrument_class_surface` | :206 | 3 |
| 12 | `investment_manager_surface` | :210 | 7 |
| 13 | `isda_surface` | :218 | 6 |
| 14 | `mandate_surface` | :225 | 1 (`mandate.instrument-matrix`) |
| 15 | `matrix_overlay_surface` | :227 | 9 |
| 16 | `movement_surface` | :237 | 14 |
| 17 | `security_type_surface` | :252 | 2 |
| 18 | `subcustodian_surface` | :255 | 3 |
| 19 | `tax_config_surface` | :259 | 11 |

**Correction to the drafted documents [V]:** the V&S §2.3 "instruction-route
brick" family — *"structured message (SWIFT MT / ISO 20022) or blotter/file,
plus manual fallback"* — does not correspond to anything here. The real
routing layer is surfaces 3 and 4: **settlement chains** (locations, hops,
cross-border, location preferences) and **trade gateways** (gateways,
routing rules, fallbacks). See §D4 for where message formats actually live
(short answer: almost nowhere).

## A.2 The overall lifecycle (`:281-429`) **[V]**

`id: instrument_matrix_overall_lifecycle`, `scope: per_cbu`,
**`derived: true`** — "Per-CBU derivation — no stored column. (Adam pass-7
Q-CB: derived, not stored.)" (`:284`, `:288-289`).

The R-4 re-anchor is explicit in the file header (`:5-16`):

> Pre-R-4: the overall_lifecycle was centred on the DATA lifecycle
> (matrix_scoped → im_configured → preferences_set → parallel_run → active)
> — technically correct but **commercially under-weighted**.
> Post-R-4: … centred on CBU-trading-enablement … The IM workspace's purpose
> is not to have correct reference data — its purpose is to **make a CBU
> trading-capable** on specific instruments and markets. The data lifecycle
> becomes a sub-process.

Eight phases, each with a `derivation` predicate block, `progression_verbs`,
and `next_phase`:

| Phase | Line | Derivation (verbatim) | Progression verbs | Next |
|---|---|---|---|---|
| `dormant` | :293 | `all_of: cbus.status = 'VALIDATED'`; `any_of: NOT EXISTS cbu_trading_profiles WHERE cbu_id = this.cbu_id` / `cbu_trading_profiles.status = 'DRAFT'` | `trading-profile.create-draft`, `.submit` | `configuring` |
| `configuring` | :310 | `cbus.status = 'VALIDATED'` AND `cbu_trading_profiles.status IN ['DRAFT','SUBMITTED','APPROVED']` | `.submit`, `.approve`, `.enter-parallel-run` | `trade_permissioned` |
| `trade_permissioned` | :329 | `status IN ['PARALLEL_RUN','ACTIVE']` AND `NOT EXISTS trading_activity … first_trade_at IS NOT NULL` | `.go-live` | `actively_trading` |
| `actively_trading` | :343 | `status = 'ACTIVE'` AND `EXISTS trading_activity … first_trade_at IS NOT NULL` AND `NOT trading_activity.dormant` | `.suspend`, `.create-new-version`, `.restrict`, `.archive` | `(restricted \| suspended \| winding_down \| amending)` |
| `restricted` | :361 | `status = 'ACTIVE'` AND `EXISTS restriction WHERE cbu_id = this.cbu_id AND active = true` | `.lift-restriction`, `.suspend` | `(actively_trading \| suspended)` |
| `amending` | :377 | `status = 'ACTIVE'` AND `EXISTS newer_version WHERE parent_profile_id = this.profile_id AND newer_version.status IN [...]` | *(none — ops on the newer version)* | `actively_trading` |
| `suspended` | :391 | `any_of: status = 'SUSPENDED'` / `cbus.operational_status = 'SUSPENDED'` | `.reactivate`, `.archive` | `(actively_trading \| winding_down)` |
| `winding_down` | :406 | `any_of: status = 'WINDING_DOWN'` / `cbus.operational_status = 'WINDING_DOWN'` | `.archive` | `retired` |
| `retired` | :419 | `any_of: status = 'ARCHIVED'` / `cbus.operational_status IN ['OFFBOARDED','ARCHIVED']` | *(terminal)* | `(none)` |

**Two derivation predicates reference things that do not exist [V]:**

- `trade_permissioned` and `actively_trading` both depend on
  `trading_activity` (`:338`, `:352-353`). The `trading_activity` **slot was
  deleted** in the state-graph remediation Phase 7b (`:865-870`): *"source_entity
  cbu_trading_activity has zero writers and zero live rows (COMMENT ON TABLE
  marks it DEPRECATED). Every transition in this slot was a fictional
  `(backend: ...)` row."* Live DB: `SELECT count(*) FROM "ob-poc".cbu_trading_activity`
  = **0**. So the two phases separating "permitted to trade" from "actually
  trading" — the whole commercial point of the R-4 re-anchor — **cannot be
  derived**.
- `restricted` depends on `EXISTS restriction WHERE … active = true` (`:371`).
  There is no `restriction` slot, no `restriction` table, and no verb that
  creates one.

## A.3 The 21 slots (`:439-1216`) **[V]**

Header comment says "21 slots" (`:432`) — **accurate** (`grep -c "^  - id: "`
over the slots block = 21, after the `trading_activity` deletion). Composition:
**12 with a structured state machine**, **1 reference-only**, **8 stateless
with rationale**.

### A.3.1 Slots with structured state machines (12)

| Slot | Line | `source_entity` | `state_column` | States | Terminal |
|---|---|---|---|---|---|
| `group` | :456 | `"ob-poc".client_group` | `discovery_status` | not_started(entry), in_progress, complete, stale, failed | *(none)* |
| `trading_profile_template` | :501 | `"ob-poc".cbu_trading_profiles` (`cbu_id IS NULL`) | `status` | available(entry), unavailable | unavailable |
| `settlement_pattern_template` | :528 | `"ob-poc".cbu_settlement_chains` | `(derived — see below)` | draft(entry), configured, reviewed, parallel_run, live, suspended, deactivated | deactivated |
| `trade_gateway` | :649 | `(hybrid — JSON document + is_active column)` | `(document body — not SQL CHECK)` | defined(entry), enabled, active, suspended, retired | retired |
| `trading_profile` | :711 | `"ob-poc".cbu_trading_profiles` (`cbu_id IS NOT NULL`) | `status` | DRAFT(entry), SUBMITTED, APPROVED, PARALLEL_RUN, ACTIVE, SUSPENDED, REJECTED, SUPERSEDED, ARCHIVED | ARCHIVED, SUPERSEDED |
| `investment_manager_assignment` | :825 | `"ob-poc".cbu_im_assignments` | `status` | ACTIVE(entry), SUSPENDED, TERMINATED | TERMINATED |
| `service_resource` | :913 | `"ob-poc".service_resource_types` | `(derived from is_active + provisioning_strategy)` | provisioned(entry), activated, suspended, decommissioned | decommissioned |
| `service_intent` | :953 | `"ob-poc".service_intents` | `status` | active(entry), suspended, cancelled | cancelled |
| `delivery` | :1025 | `"ob-poc".service_delivery_map` | `delivery_status` | PENDING(entry), IN_PROGRESS, DELIVERED, FAILED, CANCELLED | DELIVERED, FAILED, CANCELLED |
| `reconciliation` | :1093 | **`(new — P.2 schema work…)`** | `(new)` | draft(entry), active, suspended, retired | retired |
| `corporate_action_event` | :1140 | **`(new — per-mandate CA event tracking)`** | `(new)` | election_pending(entry), elected, default_applied | elected, default_applied |
| `collateral_management` | :1174 | **`(new — per-mandate CSA/collateral config)`** | `(new)` | configured(entry), active, suspended, terminated | terminated |

**Full transition sets** (the move grammar proper), verbatim from the file:

- **`group`** (`:480-498`) — 6 transitions, **all `via:` free-text backend
  triggers**, no verb FQNs: `"research pipeline kick-off (upstream trigger)"`,
  `"discovery confirmed signal (backend)"`, `"research fault signal (backend)"`,
  `"time-decay trigger"`, `"refresh trigger"`, `"retry trigger"`. **[V]** This
  slot is not verb-driven at all — the same structural finding
  EOP-DD-DAGBPMN-001 §5 recorded for `kyc_dag.yaml`.
- **`trading_profile_template`** (`:522-525`) — 1: `available → unavailable via trading-profile.retire-template`.
- **`settlement_pattern_template`** (`:565-595`) — 10: `draft→configured` via `settlement-chain.add-hop` **and** via `.define-location`; `configured→reviewed` via `.request-review`; `reviewed→parallel_run` via `.enter-parallel-run`; `parallel_run→live` via `.go-live`; `parallel_run→reviewed` via `.abort-parallel-run`; `live→suspended` via `.suspend`; `suspended→live` via `.reactivate`; `live→deactivated` and `suspended→deactivated` via `.deactivate-chain`. Plus `graph_attributes: superseded_by` (`:597-601`).
- **`trade_gateway`** (`:677-695`) — 6: `defined→enabled` (`.enable-gateway`), `enabled→active` (`.activate-gateway`), `active→suspended` (`.suspend-gateway`), `suspended→active` (`.reactivate-gateway`), `active→retired` and `suspended→retired` (`.retire-gateway`).
- **`trading_profile`** (`:786-822`) — 12: `DRAFT→SUBMITTED` (`.submit`), `SUBMITTED→APPROVED` (`.approve`), `SUBMITTED→REJECTED` (`.reject`), `REJECTED→DRAFT` (`.create-draft`), `APPROVED→PARALLEL_RUN` (`.enter-parallel-run`), `PARALLEL_RUN→ACTIVE` (`.go-live`), `PARALLEL_RUN→APPROVED` (`.abort-parallel-run`), `ACTIVE→SUSPENDED` (`.suspend`), `SUSPENDED→ACTIVE` (`.reactivate`), `ACTIVE→SUPERSEDED` (`.supersede`), `ACTIVE→ARCHIVED` and `SUSPENDED→ARCHIVED` (`.archive`).
- **`investment_manager_assignment`** (`:848-854`) — 2, one with a real YAML list `from: [ACTIVE, SUSPENDED]`.
- **`service_resource`** (`:935-950`) — 5. **`service_intent`** (`:975-987`) — 4, plus `additional_operations: service-intent.supersede` (`:989-990`).
- **`delivery`** (`:1071-1086`) — 5.
- **`reconciliation`** (`:1116-1131`) — 5. **`corporate_action_event`** (`:1163-1169`) — 2, one of which is `via: "(automatic trigger at cutoff)"` (free text, not a verb). **`collateral_management`** (`:1198-1213`) — 5.

Three slots additionally carry `predicate_bindings` — a richer gating
mechanism than plain state: `trading_profile` (`:736-755`, binding
`compliance_review`, `ops_review`, `committee_signoff` with a
`required_universe`, and `objection`) and `delivery` (`:1037-1052`). Both
also carry `green_when` clauses (`:764-771`, `:1061-1066`) expressing
completeness in prose-like predicate form. **[I]** These are the closest
thing in the file to the V&S's "Completeness" and "Prerequisite" rule kinds;
nothing was found that evaluates them.

### A.3.2 Reference-only slot (1) **[V]**

`cbu` (`:702-709`) — `state_machine: "(reconcile-existing — see cbu_dag.yaml
§2.1 cbu slot…)"`. The comment records that "the pre-R-3 IM-local
re-declaration has been removed", i.e. IM references CBU state read-only and
does not own it.

### A.3.3 Stateless slots with rationale (8) **[V]**

| Slot | Line | Rationale (condensed) |
|---|---|---|
| `workspace_root` | :445 | Projection/aggregation root; no entity lifecycle |
| `isda_framework` | :603 | ISDA lifecycle lives in external legal-ops; IM treats coverage as read-only yes/no (Adam Q2) |
| `corporate_action_policy` | :622 | Standing config on the trading profile; "CA policy is a what, not a how" |
| `custody` | :872 | Compositional projection over `entity_settlement_identity`; no slot table |
| `cash_sweep` | :893 | Binary active/not (Adam Q9a) |
| `booking_location` | :992 | Reference data; no lifecycle |
| `legal_entity` | :998 | Binary active/inactive; KYC owns granular lifecycle |
| `product` | :1011 | Lifecycle inherited from parent trading profile (Adam Q5) |

## A.4 The three axes **[V]**

Declared in the file header (`:37-45`):

1. **Vertical (what vs how)** — "config states + transitions only; runtime
   events out of scope." Enforced by the `out_of_scope` list (`:1398-1418`),
   which names 7 layer-3 runtime concerns (recon RUN events, CA processing
   and settlement, margin calls, exception/break handling, partial settlement,
   FIX UAT certification, SLA tracking) and 5 other-workspace concerns.
2. **Horizontal (product-modular)** — "slot/verb reachability gated by the
   CBU's `service_intents` set". Expressed two ways: per-slot `product_gates`
   on 3 slots (`reconciliation` :1133-1138, `corporate_action_event`
   :1171-1172, `collateral_management` :1215-1216) and the file-level
   `product_module_gates` block (`:1351-1389`) — 11 `always_on`,
   9 `conditionally_on`. **Critically, `:1346-1348` states: "During pilot
   rollout, these are INFORMATIONAL ONLY — the catalogue-load gate does not
   yet filter by product."** See §D4 for the `requires_products` correction.
3. **Temporal (CBU trading enablement)** — the 8 phases of §A.2, "with the
   DATA lifecycle (DRAFT → SUBMITTED → APPROVED → PARALLEL_RUN → ACTIVE) as a
   sub-process within `configuring → trade_permissioned`."

## A.5 Cross-slot and cross-workspace constraints **[V]**

`cross_slot_constraints` (`:1225-1317`), 10 rules — 7 `severity: error`,
3 `severity: warning`:

| # | id | Line | Severity |
|---|---|---|---|
| 1 | `mandate_requires_validated_cbu` | :1227 | error |
| 2 | `mandate_active_requires_live_settlement` | :1235 | error |
| 3 | `archived_mandate_cascades_dependents` | :1244 | warning |
| 4 | `cbu_suspended_implies_mandate_suspended` | :1257 | warning |
| 5 | `cbu_archived_requires_mandate_archived` | :1266 | error |
| 6 | `retired_gateway_prunes_routing_rules` | :1274 | error |
| 7 | `deactivated_chain_requires_universe_recheck` | :1283 | warning |
| 8 | `decommissioned_resource_cascades_intent` | :1292 | error |
| 9 | `isda_coverage_required_for_derivative_trading` | :1301 | error |
| 10 | `collateral_management_active_requires_isda` | :1311 | error |

`cross_workspace_constraints` (`:1326-1337`), 1 rule — V1.3-1, Mode A
blocking: `cbu.cbu = VALIDATED` gates `instrument_matrix.trading_profile`
transition `"DRAFT -> SUBMITTED"`.

**Rule 9 is the V&S's `otc_requires_isda` prerequisite, already declared. [V]**
It is declared as a constraint, not as a transition guard; nothing was found
that evaluates it.

## A.6 Prune semantics (`:1421-1548`) **[V]**

`prune_cascade_rules`, 5 rules: `asset_family_prune_cascades` (:1453, 7
cascade targets), `market_prune_cascades` (:1481), `instrument_class_prune_cascades`
(:1495), `counterparty_prune_cascades` (:1507), `counterparty_type_prune_cascades`
(:1521). `prune_pre_validation` (`:1540-1548`) requires
`trading-profile.prune-impact` and `.validate-universe-coverage`, with 4
abort conditions.

The prune header (`:1432`) independently enumerates the asset families:

> `asset_family (7 families: equity / fixed_income / otc_derivatives /
> listed_derivatives / fx / money_market / funds)`

## A.7 The corrected brick alphabet — instrument families **[V]**

**Precision correction to the task brief.** The 7 instrument families are
**macros**, not atomic verbs — `kind: macro`, Tier -2B in the intent
pipeline, expanded by the compiler into atomic DSL verbs before any REPL
execution. Source: `rust/config/verb_schemas/macros/instrument.yaml`.

That file's own header (`:7-8`): *"The 67 instrument classes collapse to 7
asset families. Most CBUs use `instrument.full-matrix-setup` which chains
all families."*

| Macro | Line | `component-type` emitted | `expands-to` |
|---|---|---|---|
| `instrument.setup-equity` | :19 | `EQUITY` | `trading-profile.add-component`, `settlement-chain.create-chain`, `cbu-custody.derive-required-coverage` |
| `instrument.setup-fixed-income` | :75 | `FIXED_INCOME` | `add-component`, `create-chain` |
| `instrument.setup-otc-derivatives` | :124 | `OTC_DERIVATIVE` | `add-component`, `isda.create`, `trade-gateway.define-gateway` |
| `instrument.setup-listed-derivatives` | :176 | `LISTED_DERIVATIVE` | `add-component`, `define-gateway`, `create-chain` |
| `instrument.setup-fx` | :224 | `FX` | `add-component`, `trading-profile.set-base-currency` |
| `instrument.setup-money-market` | :267 | `MONEY_MARKET` | `add-component`, `create-chain` |
| `instrument.setup-funds` | :311 | `FUND` | `add-component` (with `fund-type`, default `UCITS`) |

Composite and operational macros in the same file: `instrument.full-matrix-setup`
(:357 — chains all 7 `component-type`s plus `create-draft`, `create-chain`,
`isda.create`, `validate-go-live-ready`), `.review-coverage` (:419),
`.activate-profile` (:458), `.configure-corporate-actions` (:499),
`.clone-profile` (:543).

Every family macro declares `prereqs: [{type: state_exists, key: trading_profile.exists}]`
and `routing.mode-tags: [trading, structure]`. The file header records three
invariants (`:10-13`): all macros require a trading profile; settlement chains
and SSIs are **per-market, not per-instrument**; OTC derivatives require ISDA,
listed derivatives require an exchange gateway.

**Cross-check [V]:** the macro file's 7 `component-type` values and the DAG's
independently-authored `asset_family` list (`:1432`) agree exactly. Two
separately-maintained artefacts naming the same 7 is strong evidence this is
the real alphabet, not a coincidence of one author.

**This replaces EOP-PLAN-MANDATE-VANILLA-001 v0.1 §1's inferred list**
("equity, government bond, corporate bond, ETF, money-market, FX, listed
derivative, OTC derivative"). Government bond / corporate bond / ETF are
*instrument classes*, one level below family; the real cut is 7 families
over 67 classes.

## A.8 The implemented move alphabet in Rust **[V]**

`TradingMatrixOp`, `rust/crates/ob-poc-types/src/trading_matrix.rs:964+` —
**25 variants**. This is a legal-move enum in production code today:

`AddInstrumentClass`, `AddMarket`, `AddCounterparty`, `AddUniverseEntry`,
`AddSsi`, `ActivateSsi`, `SuspendSsi`, `AddBookingRule`, `AddSettlementChain`,
`AddSettlementHop`, `AddIsda`, `AddCsa`, `AddProductCoverage`,
`AddTaxJurisdiction`, `AddTaxConfig`, `AddImMandate`, `UpdateImScope`,
`AddCsaEligibleCollateral`, `LinkCsaSsi`, `RemoveIsda`, `RemoveCsa`,
`SetBaseCurrency`, `AddAllowedCurrency`, `RemoveNode`, `SetNodeStatus`.

Node taxonomy: `TradingMatrixNodeType`, 8 variants (`trading_matrix.rs:289+`)
— `Category`, `InstrumentClass`, `Market`, `Counterparty`, `UniverseEntry`,
`Ssi`, `BookingRule`, `SettlementChain`.

**`AddCsa` and `RemoveCsa` exist and are implemented**
(`ob-poc-trading-profile/src/ast_builder.rs:888`), which contradicts
EOP-PLAN-MANDATE-001 §A1.5's *"CSA exists only as a comment"*. That claim is
true of the plpgsql migration only (`20260106_trading_profile_ast_migration.sql:269`
— `'children', '[]'::jsonb,  -- CSAs would be children here`), not of the
live path.

**`SetNodeStatus` [I]:** a move whose only effect is to set a presentation
field (`status_color`). Under V&S Law 2 this is not a legal move at all —
it is a rendering instruction admitted to the grammar. Worth an explicit
ruling in v0.2 rather than a silent deletion.

## A.9 Verb existence check — the grammar is implemented, not aspirational **[V]**

Every transition in the DAG annotated `# NEW verb — P.3` now **exists** in
verb YAML. Checked by grepping `rust/config/verbs/` at verb-key indentation:

| Verb | Declared at |
|---|---|
| `settlement-chain.request-review` | `custody/settlement-chain.yaml:915` |
| `settlement-chain.enter-parallel-run` | `custody/settlement-chain.yaml:955` |
| `settlement-chain.go-live` | `custody/settlement-chain.yaml:992` |
| `settlement-chain.abort-parallel-run` | `custody/settlement-chain.yaml:1029` |
| `settlement-chain.suspend` | `custody/settlement-chain.yaml:1069` |
| `settlement-chain.reactivate` | `custody/settlement-chain.yaml:1112` |
| `trade-gateway.reactivate-gateway` | `custody/trade-gateway.yaml:805` |
| `trade-gateway.retire-gateway` | `custody/trade-gateway.yaml:842` |
| `trading-profile.enter-parallel-run` | `trading-profile.yaml:1896` |
| `trading-profile.go-live` | `trading-profile.yaml:1933` |
| `trading-profile.abort-parallel-run` | `trading-profile.yaml:1973` |
| `trading-profile.suspend` | `trading-profile.yaml:2012` |
| `trading-profile.reactivate` | `trading-profile.yaml:2054` |
| `trading-profile.supersede` | `trading-profile.yaml:2093` |
| `trading-profile.restrict` / `.lift-restriction` | `trading-profile.yaml:2648` / `:2688` |
| `trading-profile.prune-impact` | `trading-profile.yaml:2575` |
| `trading-profile.retire-template` | `trading-profile.yaml:2140` |
| `reconciliation.activate/.suspend/.reactivate` | `reconciliation.yaml:75/112/154` |
| `collateral-management.activate/.suspend/.reactivate` | `collateral-management.yaml:84/121/163` |
| `corporate-action-event.elect` | `corporate-action-event.yaml:85` |
| `delivery.start` / `.cancel` | `delivery.yaml:247` / `:283` |
| `service-resource.reactivate` / `.decommission` | `service-resource.yaml:1977` / `:1084` |

**The `# NEW verb — P.3` comments are stale; the verbs landed.** This is the
strongest single piece of evidence that the board and its move grammar are
implemented rather than planned, and it is the fact that most changes the
shape of the refactor.

Verbs also carry the DAG binding back: **45 occurrences** of
`transition_args.target_workspace: instrument_matrix` across 10 verb files
(`trading-profile.yaml`, `custody/settlement-chain.yaml`,
`custody/trade-gateway.yaml`, `reconciliation.yaml`,
`collateral-management.yaml`, `corporate-action-event.yaml`, `delivery.yaml`,
`service-pipeline.yaml`, `service-resource.yaml`, `instrument-matrix.yaml`),
and `lifecycle.requires_states` declarations derived from the DAG (e.g.
`trading-profile.yaml:1124` `requires_states: [DRAFT]` on `.submit`,
`:1195` `[SUBMITTED]` on `.approve`, `:1309` `[ACTIVE, SUSPENDED]`). **[V]**

## A.10 Gaps found while enumerating **[V]**

1. **`trading-profile.restrict` / `.lift-restriction` claim a transition the
   slot does not declare.** Both declare `three_axis.state_effect: transition`
   and `transition_args.target_slot: trading_profile`
   (`trading-profile.yaml:2648-2683`, `:2688-2721`). The `trading_profile`
   slot state machine (`:756-823`) has **no `RESTRICTED` state and no
   transition via either verb**. They appear only in
   `overall_lifecycle.phases[restricted].progression_verbs` (`:373-374`),
   whose derivation depends on a nonexistent `restriction` entity (`:371`).
   This is the same defect class as the `transition_args.target_workspace`
   drift already fixed twice in this codebase.
2. **Three slots govern entities that do not exist**: `reconciliation`
   (`:1099` `source_entity: "(new — P.2 schema work or reuse of existing recon
   table if present)"`), `corporate_action_event` (`:1146`),
   `collateral_management` (`:1180`). Their verbs exist (§A.9); their tables
   do not.
3. **`trade_gateway` names no table** — `source_entity: "(hybrid — JSON
   document + is_active column for query speed)"` (`:654`),
   `state_column: (document body — not SQL CHECK)` (`:655`). See §D4: the
   `trade_gateways` table does not exist in the live database.
4. **`settlement_pattern_template`'s "no state column" comment is stale — and
   the fix landed.** The file says (`:544-548`) *"draft / configured /
   reviewed / parallel_run are currently conceptual; schema has no state
   column yet. Will require a migration to persist — track in P.9 findings."*
   **Live DB contradicts this**: `cbu_settlement_chains.lifecycle_status text`
   exists with a CHECK enumerating exactly the DAG's 7 states
   (`draft|configured|reviewed|parallel_run|live|suspended|deactivated`).
   Good news, and the comment should be retired.
5. **Two of the eight lifecycle phases are underivable** — see §A.2.

## A.11 Ownership gap — the board is wider than the pack **[V]**

`rust/config/sem_os_seeds/domain_packs/ob_poc_instrument_matrix.yaml`:

```yaml
pack_id: ob-poc.instrument-matrix
compatibility_tier: dry_run_only
owned_dags: [instrument_matrix_dag]
owned_packs: [instrument-matrix]
owned_verb_prefixes:
  - instrument.
  - instrument-matrix.
  - trading-profile.
  - settlement-chain.
  - trade-gateway.
owned_entity_kinds: []
```

Both `allowed_transitions` entries carry `dry_run_enabled: true` and
**`mutation_enabled: false`**.

Checked across every file in `domain_packs/`, the prefixes the DAG
references break down as:

| Ownership | Prefixes |
|---|---|
| Owned by the IM pack (5) | `instrument.`, `instrument-matrix.`, `trading-profile.`, `settlement-chain.`, `trade-gateway.` |
| Owned by another pack (3) | `cbu-custody.` → `ob_poc_cbu.yaml`; `mandate.` → `ob_poc_book_setup.yaml`; `service-resource.` → `ob_poc_onboarding_request.yaml`, `ob_poc_product_service_taxonomy.yaml` |
| **Owned by no pack (18)** | `isda.`, `instruction-profile.`, `corporate-action.`, `entity-settlement.`, `tax-config.`, `investment-manager.`, `cash-sweep.`, `matrix-overlay.`, `movement.`, `security-type.`, `subcustodian.`, `instrument-class.`, `pricing-config.`, `reconciliation.`, `collateral-management.`, `delivery.`, `corporate-action-event.`, `service-intent.` |

Of the 19 `dsl_verb_reconciliation` surfaces, **12 are pack-unowned**; the
remaining 6 unowned prefixes appear only in slot transitions. **[I]** Even a
perfectly-modelled board is not reachable through the governed pack surface
for most of its own declared verbs, and the pack that does own the DAG
cannot mutate anything (`mutation_enabled: false`).

---

# §B V&S §5 design laws — reconciliation

| # | Law | Verdict |
|---|---|---|
| 1 | No arrangement in storage | **Violated** (and the drafted docs mis-specify how) |
| 2 | No presentation in the board | **Violated, and load-bearing** |
| 3 | One mutation path | **Violated** — two authorities, same column |
| 4 | Assembly total and deterministic | **Violated** (order-dependent hash; no `assemble()` exists) |
| 5 | Identity content-derived | **Violated in the generator; satisfied in the entity layer** |
| 6 | Rules versioned and pinned | **Not applicable today** |
| 7 | One authority | **Violated** — the central finding (§C) |
| 8 | Resolution single-valued | **Contradicted by the implemented design — the V&S should give way** |

## Law 1 — No arrangement in storage: **VIOLATED [V]**

`TradingMatrixNode` (`rust/crates/ob-poc-types/src/trading_matrix.rs:654-683`):

```rust
pub struct TradingMatrixNode {
    pub id: TradingMatrixNodeId,
    pub node_type: TradingMatrixNodeType,
    pub label: String,
    pub sublabel: Option<String>,
    pub children: Vec<TradingMatrixNode>,   // <-- arrangement IS the storage
    pub status_color: Option<StatusColor>,
    pub is_loaded: bool,
    pub leaf_count: usize,
}
```

**Correction to the drafted documents.** EOP-PLAN-MANDATE-001 §A2.3 and the
task brief both describe *"the JSONB AST's parent/path/ordinal/leaf_count"*.
Verified across the plpgsql generator, the Rust type, and the TypeScript
mirror: **`parent`, `path`, and `ordinal` do not exist anywhere.**

What actually stores arrangement is three things:
- `children: Vec<TradingMatrixNode>` — nesting is the storage (`:670`).
- `TradingMatrixNodeId(pub Vec<String>)` — a **path array** (`:65`), with
  `parent()` (`:81`), `is_child_of()` (`:92`), and `depth()` (`:102`) as
  *derived methods*, not serialised fields.
- `leaf_count: usize` (`:682`) and root `total_leaf_count` (`:832`).

**[I]** The path-array `id` is the subtler violation: node identity is
*derived from position*, so moving a node changes its id. That is the
inverse of Law 5, and it means Laws 1 and 5 cannot both be satisfied without
re-basing identity on the entity's own `uuidv7`.

## Law 2 — No presentation in the board: **VIOLATED, and load-bearing [V]**

`label`, `sublabel`, `status_color`, `is_loaded`, `leaf_count` sit on every
node alongside the facts. Minted in **both** generators:

- plpgsql: `rust/migrations/20260106_trading_profile_ast_migration.sql:81-86`
  — `'status_color', 'green', 'is_loaded', true, 'leaf_count', 1`, applied
  unconditionally to every leaf.
- live Rust: `ob-poc-trading-profile/src/ast_builder.rs` — `.with_sublabel(...)`
  at `:387`, `:431`, `:476`, `:550`, `:613`, `:716`, `:765`, `:821`, `:878`,
  `:936`, `:984`, `:1102`, `:1169`, `:1228`, `:1359`; `node.status_color = Some(...)`
  at `:649`.

**Why this is load-bearing, not cosmetic:** the React UI reads these fields
**straight out of the persisted JSONB** and does not recompute them —
`ob-poc-ui-react/src/features/viewport/components/TradingMatrixTree.tsx:114-116`
(`getStatusColorClass(node.status_color)`), `:179-188` (renders `sublabel`,
`leaf_count`), `:446` (`{node.children.length} children • {node.leaf_count}
total leaves`), `:535` (`total_leaf_count`); type mirror at
`ob-poc-ui-react/src/api/tradingMatrix.ts:192-201`.

**[I]** Stripping presentation from the board is therefore a **breaking UI
change**, not a type-level tidy-up. Whichever tranche asserts
`no_presentation_in_block` owns that migration or must explicitly defer it.

## Law 3 — One mutation path: **VIOLATED [V]**

Two Rust modules write `cbu_trading_profiles.document`, with **different
document types**:

| Authority | Type | Write sites |
|---|---|---|
| `ob-poc-trading-profile/src/ast_db.rs` | `TradingMatrixDocument` (tree) | `:163`, `:230`, `:293`, `:332`, `:391` |
| `ob-poc-trading-profile/src/document_ops.rs` | `TradingProfileDocument` (flat) | `:104`, `:196`, `:1703`, `:1795`, `:1853`, `:1925`, `:1944`, `:2006`, `:2198`, `:2271` |

Their own module doc comments contradict each other:

> `ast_db.rs:8-9` — *"The document is the single source of truth. **No
> materialization to operational tables is needed** — the document structure
> directly serves the UI."*
>
> `document_ops.rs:3-4` — *"These handlers modify the JSONB document
> directly, not operational tables. The document is the source of truth;
> **operational tables are materialized from it**."*

Both are reachable from live verbs: `ast_db` from ~23 call sites in
`rust/src/domain_ops/trading_profile.rs` (`:1433`, `:1609`, `:1653`, …,
`:3072`); `document_ops` from 8 (`:2691`, `:2721`, `:2745`, `:2779`, `:2810`,
`:2898`, `:2927`, `:3019`).

## Law 4 — Assembly total and deterministic: **VIOLATED [V]**

- **Order dependence.** `add_child` appends (`trading_matrix.rs:724`), so
  child order is insertion order. `compute_document_hash` is SHA-256 over
  `serde_json::to_string(doc)` (`ast_db.rs:147-152`). Shuffling the order in
  which moves are applied therefore changes the board hash. The V&S's
  `order_independence` gate would fail as written today.
- **No `assemble()` exists.** The tree *is* the storage, so there is no
  blocks→board function to be total over. Law 4 is not so much violated as
  **unimplementable in the current architecture** — which is precisely what
  makes it a useful gate once §C is fixed.

## Law 5 — Identity content-derived: **VIOLATED in the generator, SATISFIED in the entity layer [V]**

- Entity layer is correct: `cbu_ssi.ssi_id uuid DEFAULT uuidv7() NOT NULL`
  (`master-schema.sql:9245`); same pattern on `cbu_instrument_universe`
  (`:8576`), `ssi_booking_rules` (`:18487`), `isda_agreements` (`:14247`),
  `csa_agreements` (`:10717`). *(`uuidv7()` is a PostgreSQL 18 builtin —
  confirmed present in `pg_catalog` on the live database, so the fact that
  `master-schema.sql` never defines it is benign.)*
- Generator is not: `migrate_trading_profile_to_ast` mints fresh
  `gen_random_uuid()` for `universe_id` (`20260106_…sql:75`), `rule_id`
  (`:175`), `ssi_id` (`:213`), `isda_id` (`:260`). Re-running produces
  different ids for the same facts.
- **Two hash algorithms on one column [V]:** the migration writes
  `document_hash = md5(v_new_doc::text)` (`:397`); the live Rust writes
  SHA-256 (`ast_db.rs:150`); the schema comment says SHA-256
  (`master-schema.sql:9511`).

## Law 6 — Rules versioned and pinned: **NOT APPLICABLE today [V]**

No rule engine exists to version. `cbu_trading_profiles.version` versions the
*document*, not the rules that derived it. **[I]** Law 6 only acquires
meaning once assembly exists; it is correctly sequenced last in both drafted
plans.

## Law 7 — One authority: **VIOLATED — see §C**

## Law 8 — Resolution single-valued: **CONTRADICTED — recommend the V&S gives way**

`ssi_booking_rules.specificity_score` is a **stored generated column**
(`master-schema.sql:18500-18524`), a weighted bitmask over which criteria are
non-NULL:

| Dimension | Weight |
|---|---|
| `counterparty_entity_id` | 32 |
| `instrument_class_id` | 16 |
| `security_type_id` | 8 |
| `market_id` | 4 |
| `currency` | 2 |
| `settlement_type` | 1 |

Range 0–63. NULL means wildcard — table comment (`:18537`): *"Layer 3:
ALERT-style booking rules. Priority-based matching with wildcards (NULL =
any)."*

Every resolution path is a ranking, never a rejection:
`ORDER BY priority ASC, specificity_score DESC ... LIMIT 1`
(`rust/tests/custody_integration.rs:452`, `:594`, `:695`). **Nothing anywhere
rejects an overlap as ambiguous** — not `ssi_booking_rules`, not
`cbu_pricing_config`, not gateway routing, not
`cbu_settlement_location_preferences`. The only modelled failure is *zero*
matches (`sem_os_postgres/src/ops/custody.rs:206-211`).

> **Recommendation: the V&S gives way.** Overlap resolution by specificity is
> the implemented, in-production, negotiated practice, and the generated
> bitmask encodes a deliberate dimension precedence that someone chose. Law 8
> should be restated as **"resolution is total and deterministic"**.

This closes **V&S §8 Open Ruling 4** in favour of the second horn.

**But two real holes remain, and they deserve gates [V]:**

1. **Ties are silently non-deterministic.** `ssi_booking_rules` has UNIQUE on
   `(cbu_id, priority, rule_name)` (`master-schema.sql:27819`) and
   `(cbu_id, rule_name)` (`:27827`) — **none on the criteria tuple**. Two
   rules with identical criteria (hence identical `specificity_score`) and
   identical `priority` but different `rule_name` and different `ssi_id` both
   match; `LIMIT 1` then picks arbitrarily, with no stable ordering.
2. **`cbu_pricing_config` has no specificity mechanism at all**
   (`master-schema.sql:8823-8845`) — only `priority integer DEFAULT 1`, and
   no UNIQUE on `(cbu_id, instrument_class_id, market_id, currency)`. Pricing
   overlap has neither a ranking tiebreak nor a rejection.

**Three incompatible specificity scales coexist [V]:** the DB bitmask 0–63
(`master-schema.sql:18500`); an independent plain count of non-None criteria,
range 0–6, in `ast_builder.rs:690-699`; and a hand-seeded `100` in
`scripts/seed_allianz_trading_matrix.sql:154`. The UI displays whichever one
its source produced (`tradingMatrix.ts:69`,
`TradingMatrixTree.tsx:303`).

So `resolution_unambiguous` should become three gates:
**`resolution_is_total_and_deterministic`**, **`no_specificity_ties`**, and
**`one_specificity_scale`**.

---

# §C Where the defect actually sits

EOP-PLAN-MANDATE-001 §A3 located the defect in **generation** (the plpgsql
function), **arrangement storage**, **presentation in fact nodes**, and
**direction of flow**. Verdicts:

| §A3 claim | Verdict |
|---|---|
| Presentation baked into fact nodes | **CONFIRMED**, and worse — it is in the live Rust type and read directly by the UI, not merely in a migration artefact |
| Arrangement stored | **PARTIALLY CONFIRMED, mis-specified** — `children` + path-`id` + `leaf_count`, but `parent`/`path`/`ordinal` do not exist |
| Generation lives in plpgsql | **CORRECTED** — that function is a one-shot, already-run migration; the live generator is Rust |
| Direction of flow (document → entities) is the largest change | **CORRECTED, and much weaker** — that path has never executed |

## C.1 The plpgsql function is not the live path **[V]**

`migrate_trading_profile_to_ast` — definition
`rust/migrations/20260106_trading_profile_ast_migration.sql:20-316`, driver
`DO` block `:359-405`, installed copy `migrations/master-schema.sql:3359-3658`.
It is a **one-shot migration**, gated by `needs_ast_migration(document)`
(`:325-342`). Both functions are present in the live database
(`pg_proc`, schema `ob-poc`), but nothing calls them at runtime.

The **live** generator is Rust:

- `ob-poc-trading-profile/src/ast_builder.rs` — 1708 lines, ~30 builder
  functions, entry point `apply_op` at `:79`. Module header (`:1-13`):
  *"The document IS the AST… No SQL tables are touched — all state lives in
  the document."*
- Funnelled through `ast_db::apply_and_save` (`:427-448`):
  `ensure_draft` → `load_document` → `apply_op` → `compute_leaf_counts` →
  `save_document`.
- Reached from ~23 call sites in `rust/src/domain_ops/trading_profile.rs`.

**Consequence:** retiring the plpgsql function — EOP-PLAN-MANDATE-001's T3
deliverable — changes nothing about the live path. That tranche as written
retires an artefact that is already inert.

Worth recording, since the plpgsql shape informed the drafted docs' model of
the AST: it is **lossy** — market↔class association is a cartesian product
(`:63-120`, because the flat source has no linkage), `country_code` comes
from a hard-coded MIC-substring `CASE` defaulting to `'XX'` (`:97-111`),
`status_color` is unconditionally `'green'`, `is_loaded` unconditionally
`true`, and `cfi_prefix`/`pset_bic` are hard-coded NULL (`:129`, `:221`).

## C.2 The materialisation arrow has never fired **[V]**

`migrations/020_trading_profile_materialization.sql` is 45 lines and creates
**only an audit table**, `"ob-poc".trading_profile_materializations` (`:9-33`)
— no transform logic, and it never writes `document`. Its comment (`:44-45`):
*"Audit trail for trading-profile:materialize operations — tracks when
documents are projected to operational tables."*

The real materialiser is Rust:
`rust/src/domain_ops/trading_profile.rs:~380-540`
(`materialize_ssi` `:604`, `materialize_universe` `:689`,
`materialize_booking_rules` `:764`, `materialize_isda_agreements` `:835`,
`materialize_investment_managers` `:1041`, `materialize_corporate_actions`
`:1207`), logging to the audit table at `:523-536`.

**Live database:**

```sql
SELECT count(*) FROM "ob-poc".trading_profile_materializations;
-- 0
```

EOP-PLAN-MANDATE-001 §A1.6 called reversing this arrow *"the single largest
directional change in the refactor"*. **It is an arrow that has never been
traversed.** The refactor is correspondingly smaller.

## C.3 The actual defect: two authorities, one column **[V]**

`cbu_trading_profiles.document jsonb NOT NULL`
(`migrations/master-schema.sql:9457-9487`) is deserialised into **two
mutually-undeserialisable Rust types**:

| Type | Location | Required fields (no serde default) |
|---|---|---|
| `TradingMatrixDocument` (tree) | `ob-poc-types/src/trading_matrix.rs:811-849` | `version: i32`, `children: Vec<TradingMatrixNode>` |
| `TradingProfileDocument` (flat) | `ob-poc-trading-profile/src/types.rs:42-85` | `universe: Universe` |

Neither shape can be parsed as the other: the tree has no `universe`, the
flat has no `version`/`children`.

**Live distribution** (`data_designer`, 2026-08-20):

```sql
SELECT status, count(*) AS n,
       count(*) FILTER (WHERE document ? 'version')  AS has_version,
       count(*) FILTER (WHERE document ? 'children') AS has_children,
       count(*) FILTER (WHERE document ? 'universe') AS has_universe
FROM "ob-poc".cbu_trading_profiles GROUP BY status;
```

| status | n | `version` | `children` | `universe` |
|---|---|---|---|---|
| DRAFT | 427 | 417 | 417 | 7 |
| ACTIVE | 16 | 0 | 0 | 16 |

The split is clean and pathological: **every tree document is DRAFT; every
ACTIVE document is flat.**

### Consequence 1 — the read API is broken for every ACTIVE profile **[V]**

`ast_db::load_active_document` (`:63-87`) selects
`WHERE status = 'ACTIVE'` and deserialises as `TradingMatrixDocument`. All 16
ACTIVE rows are flat, so every call fails with `missing field \`version\``.

That is the sole path behind `GET /api/cbu/:cbu_id/trading-matrix`
(`rust/src/api/trading_matrix_routes.rs:59`, route `:85`, mounted at
`ob-poc-web/src/main.rs:2047`) and therefore behind the React Viewport page
(`ViewportPage.tsx:28`, `:323-324`, `:384`).

### Consequence 2 — materialisation is broken for every tree profile **[V]**

`TradingProfileMaterialize::execute`
(`rust/src/domain_ops/trading_profile.rs:319`) deserialises the same column
as `TradingProfileDocument`. All 417 tree DRAFTs fail with
`missing field \`universe\``. Consistent with the 0 recorded materialisation
events in §C.2.

### Contributing rot **[V]**

- The column comment (`master-schema.sql:9503`) still documents the **flat**
  shape — *"JSONB document containing: universe, investment_managers,
  isda_agreements, settlement_config, booking_rules, standing_instructions,
  pricing_matrix, valuation_config, constraints"* — never updated for the
  2026-01-06 AST migration.
- The `status` CHECK in the dump (`:9486`) permits
  `DRAFT|SUBMITTED|APPROVED|PARALLEL_RUN|ACTIVE|SUSPENDED|REJECTED|SUPERSEDED|ARCHIVED`
  but **not** `VALIDATED` or `PENDING_REVIEW` — which `ast_db.rs:98` queries
  for and `ast_db.rs:335` writes. (`rust/migrations/20260502_trading_profile_status_align_dag.sql:13-17`
  replaces that constraint; the dump appears to predate it.)
- The table comment (`:9494`) still asserts documents *are* materialised to
  operational tables "via the trading-profile.materialize verb".

**[I]** The most likely history: the 2026-01-06 AST migration converted the
DRAFT corpus to the tree shape and left ACTIVE rows alone, and no one
exercised the ACTIVE read path afterwards. What would settle it: the
`trading_profile_migration_backup` table (`20260106_…sql:348-353`) and the
`created_at` spread (DRAFT 2026-01-05 → 2026-03-30; ACTIVE 2026-01-12 →
2026-05-01, i.e. ACTIVE rows were created *after* the migration ran).

---

# §D Closure of the open premises

## D.1 Is there a live AST generation path, or only the `migrate_*` one-off? **[V] — YES, and it is Rust**

Answered in §C.1. `ob-poc-trading-profile::ast_builder` +
`ast_db::apply_and_save`, ~23 live call sites. The plpgsql function is inert.
There are **no triggers** on `cbu_trading_profiles`, no generated columns
producing the AST, and the only view over it (`v_active_trading_profiles`,
`master-schema.sql:19444-19456`) is read-only pass-through.

EOP-PLAN-MANDATE-001 §A2.1's framing — "decides whether T2 replaces a live
path or only a migration artifact" — is answered: **a live path**, and a
different one than assumed.

## D.2 Do universe / booking_rule / isda / csa have the same maturity as `cbu_ssi`? **[V] — YES, all four**

Reference point, `cbu_ssi` (`master-schema.sql:9244-9269`): `ssi_id uuid
DEFAULT uuidv7() NOT NULL` (`:9245`), FK `market_id → markets(market_id)`
(`:37294-37295`), UNIQUE `(cbu_id, ssi_name)` (`:24843`), FK `cbu_id → cbus`
ON DELETE CASCADE (`:37286`). Comment (`:9276`): *"Layer 2: Pure SSI account
data. No routing logic — just the accounts themselves."* Caveat: `market_id`
is nullable and appended last, so the FK is proper but the dimension is
optional.

| Family | Table(s) | uuidv7 | FKs | Verdict | Impurity |
|---|---|---|---|---|---|
| universe | `cbu_instrument_universe` `:8575-8589` | yes `:8576` | 4 (`:36847`, `:36855`, `:36863`, `:36871`) | **≥ `cbu_ssi`** | `currencies`/`settlement_types` are text arrays, not FK rows (`:8580-8581`); `counterparty_key` is a sentinel-UUID hack for NULL-tolerant natural keys (`:8588`) |
| booking_rule | `ssi_booking_rules` `:18486-18530` | yes `:18487` | 5 | **≥ `cbu_ssi`** — most constrained of the four | `isda_asset_class`/`isda_base_product` are loose text (`:18498-18499`) shadowing `isda_product_taxonomy`, with no FK |
| isda | `isda_agreements` `:14246`, `isda_product_coverage` `:14264`, `isda_product_taxonomy` `:14278` | yes | 6 total | **= `cbu_ssi`** | — |
| csa | `csa_agreements` `:10716-10729` | yes `:10717` | 2 (`:37823`, `:37831`) | **≈ `cbu_ssi`**, thinnest | no table COMMENT; `csa_type` free text with **no CHECK** |

**None is missing; none is a thin denormalised text table.** EOP-PLAN-MANDATE-001
§A1.1's conclusion — *"the entity layer is largely correct today"* — is
**confirmed and now generalised from SSI to all four families.**

Row counts, live (2026-08-20), which qualify "correct" as "correct but
mostly empty":

| Table | rows |
|---|---|
| `cbu_instrument_universe` | 5537 |
| `ssi_booking_rules` | 36 |
| `cbu_ssi` | 15 |
| `settlement_locations` | 6 |
| `csa_agreements` | 3 |
| `isda_agreements` | 2 |
| `cbu_settlement_chains` | 1 |
| `settlement_chain_hops` | **0** |
| `cbu_ssi_agent_override` | **0** |
| `cbu_pricing_config` | **0** |
| `cbu_im_assignments` | **0** |
| `cbu_gateway_connectivity` | **0** |
| `cbu_trading_activity` | **0** |

## D.3 Is `cbu_ssi_agent_override.sequence_order` a settlement fact or a display ordinal? **[I] — a fact by intent, but wholly inert**

DDL (`master-schema.sql:9283-9294`): `sequence_order integer NOT NULL`
(`:9290`). Sole constraint on it: UNIQUE `(ssi_id, agent_role,
sequence_order)` (`:24835-24836`). **No column comment. No table comment. No
CHECK. No default.**

**Evidence it is a settlement fact:**

1. The source-document values are settlement-shaped —
   `rust/tests/scenarios/valid/ssi_onboarding_document.json:117-125`:
   `agent_role: "INT1"` (SWIFT intermediary-1 party role),
   `sequence_order: 1`, `reason: "Local market requires Japanese
   sub-custodian"`. A market-structure justification, not a rendering choice.
2. It is a **required, non-`Option`, non-defaulted input field** of the SSI
   onboarding document struct
   (`sem_os_postgres/src/ops/custody.rs:410-417`) — asserted by the source,
   not assigned by the application.
3. The uniqueness is scoped **per agent role**, not per parent. A display
   ordinal would be unique per `ssi_id` alone; role-scoping is what you do
   when the ordinal indexes position within a role's chain (INT1, INT2, …).
4. Contrast a genuine display ordinal in this repo:
   `migrations/067_deal_record_fee_billing.sql:112` —
   `sequence_order INT NOT NULL DEFAULT 1, -- Ordering within the deal`,
   read as `ORDER BY sequence_order NULLS LAST, fee_type, fee_subtype`
   (`rust/src/database/deal_repository.rs:286`). Nullable, defaulted,
   commented "ordering", sorted with fallbacks. `cbu_ssi_agent_override`'s has
   none of those markers.

**Evidence it is inert:**

1. **Exactly one write site** (`custody.rs:565-580`, inside
   `cbu-custody.setup-ssi`) and **zero read sites** — no `SELECT`, no
   `ORDER BY`, no UI binding anywhere in Rust, SQL, or TypeScript.
2. **Live DB: `SELECT count(*) FROM "ob-poc".cbu_ssi_agent_override` = 0.**
   Never written, either.
3. A properly-modelled twin already exists: `settlement_chain_hops.hop_sequence`
   (`master-schema.sql:17916`) with UNIQUE `(chain_id, hop_sequence)`
   (`:27643`), a role CHECK enumerating `CUSTODIAN|SUBCUSTODIAN|AGENT|CSD|ICSD`
   (`:17927`), and a table comment (`:17935`): *"Individual hops/intermediaries
   in a settlement chain."*

### Ruling

> **The design laws DO need an explicit ORDERED FACT carve-out** — settlement
> chains are genuinely ordered by market convention, and that order is
> constitutive, not presentational. EOP-PLAN-MANDATE-001 §B2 Risk 3 is
> correct to want it named rather than quietly violated.
>
> **But `sequence_order` is the wrong exemplar to hang it on.** It has zero
> rows, zero readers, no documented semantics, and duplicates a concern
> `settlement_chain_hops.hop_sequence` already models properly. Recommend the
> carve-out cite `hop_sequence`, and that `cbu_ssi_agent_override` be
> triaged separately as a candidate for the same treatment the recent
> share-register kill list applied to unbacked capability.

**[I]** flagged: intent is inferred from fixture values and column shape,
not from any assertion the schema or code makes. See §G.

## D.4 Where do instruction formats and pricing preferences live today?

### Instruction formats: **[V] — essentially nowhere**

The **only** shipped modelling of instruction delivery is a single column:

```sql
-- migrations/master-schema.sql:8547 (table cbu_im_assignments)
instruction_method character varying(20) NOT NULL,
-- :8559
CONSTRAINT valid_instruction_method CHECK (instruction_method IN
  ('SWIFT','CTM','FIX','API','MANUAL','ALERT'))
```

Indexed at `:29910`. Live rows in `cbu_im_assignments`: **0**.

**Absent from ground truth entirely:** any MT number, `ISO 20022`, `sese.`,
`camt.`, `pacs.`, `message_type`, `message_format`, `instruction_format`,
`blotter`, or `FILE` as a delivery channel.

The rich model **exists only as unapplied DDL** —
`rust/migrations/202501_instruction_gateway.sql`, targeting schema `custody`
(not `"ob-poc"`), which effectively does not exist (2 mentions of `custody.`
in 41k lines of `master-schema.sql`). It defines
`custody.instruction_message_types` (`:11-23`, comment `:25` —
*"Reference data: instruction message types (MT540, sese.023, etc.)"*,
`message_standard` ∈ `MT|MX|FIX|FPML|PROPRIETARY` per `:27`),
`custody.instruction_templates` (`:30-41`),
`custody.cbu_instruction_assignments` (`:46-60`),
`custody.cbu_instruction_field_overrides` (`:65-75`),
`custody.trade_gateways` (`:89-102`, `protocol` comment `:105` —
*"MT, MX, FIX_4_2, FIX_4_4, FIX_5_0, FPML, REST, SOAP, FILE, MANUAL"* — the
only place `FILE` and `MX` appear anywhere),
`custody.cbu_gateway_connectivity` (`:108-124`),
`custody.cbu_gateway_routing` (`:128-142`, comment `:144` —
*"Priority-based gateway routing rules (similar to SSI booking rules)"*, but
with `priority` only and **no `specificity_score`**), and
`custody.cbu_gateway_fallbacks` (`:146-158`).

**Live-DB existence check** (`to_regclass('"ob-poc".<t>')`):

| Table | exists |
|---|---|
| `instruction_message_types` | **no** |
| `instruction_templates` | **no** |
| `cbu_instruction_assignments` | **no** |
| `cbu_instruction_field_overrides` | **no** |
| `trade_gateways` | **no** |
| `cbu_gateway_routing` | **no** |
| `cbu_gateway_fallbacks` | **no** |
| `cbu_gateway_connectivity` | yes (re-created in `"ob-poc"` at `:8457`, without its `trade_gateways` FK parent) |

**This settles V&S §8 Open Ruling 3 ("route granularity — channel level or
message-type level?") as a design question, not a discovery one.** The
*intended* granularity is unambiguous: `instruction-profile.define-message-type`
declares `conflict_keys: [lifecycle_event, message_standard, message_type]`
with `message-type` described as *"e.g., MT540, MT542, sese.023.001.09, FIX
NewOrderSingle"* (`rust/config/verbs/custody/instruction-profile.yaml:10-70`).
That is **message-type level**. But nothing implements it — the verbs exist
and the table does not. See §D.6.

### Pricing preferences: **[V] — `cbu_pricing_config`, real but empty**

`"ob-poc".cbu_pricing_config` (`master-schema.sql:8823-8845`) — `config_id
uuid DEFAULT uuidv7()`, 5 FKs (`:36935`, `:36943`, `:36951`, `:36959`,
`:36967`), 3 CHECKs (`source` ∈ 9 vendors, `price_type` ∈ 6, `stale_action`
∈ `WARN|BLOCK|USE_FALLBACK|ESCALATE`). Comment (`:8851-8852`): *"Pricing
source configuration by instrument class. Materialized from trading
profile."* **Live rows: 0.** Structural flaw already noted under Law 8: no
UNIQUE on the criteria tuple, no specificity mechanism.

Adjacent preference tables, both real: `cbu_ca_preferences` (`:8110`),
`cbu_settlement_location_preferences` (`:9178-9189`, carrying `priority
integer DEFAULT 50 NOT NULL` and `reason text` — the same
priority-without-specificity pattern).

### Correction to the task brief: `requires_products:` is not implemented **[V]**

The brief states *"pricing_preference is product-gated via `requires_products:`
(fund_accounting)"*. Repo-wide grep for `requires_products` returns **exactly
one hit, and it is a comment**:

```
rust/config/sem_os_seeds/dag_taxonomies/instrument_matrix_dag.yaml:42
#      product.fund_accounting enrolled) — V1.2-3 requires_products:.
```

There is no `requires_products` key in any verb YAML, pack, or Rust type.
Product gating is expressed instead as `product_module_gates` /
per-slot `product_gates` (§A.4), which the file itself marks
**INFORMATIONAL ONLY** (`:1346-1348`). `pricing_preferences (FA product)`
appears only as a *cascade target* inside `prune_cascade_rules`
(`:1478`, `:1504`). And `pricing_preference` as a term exists nowhere in the
repo outside that DAG file.

**[I]** So there is no "pricing brick family" in the V&S's sense. Pricing is
a region-claiming table (`cbu_pricing_config`) plus a prune-cascade target,
and it is unpopulated. What the V&S §2.4 calls the pricing family — *"fee
basis over a region… bps on AUM, per-transaction, tiered, minimum charge"* —
is the **`billing`/`deal` domain**, not the instrument matrix. That is a
category error in the drafted V&S, not a gap in the implementation.

## D.5 Which consumers read the AST JSONB? (compatibility surface) **[V]**

### Rust

| Site | Shape |
|---|---|
| `ob-poc-trading-profile/src/ast_db.rs:50-59` `load_document` | tree |
| `…/ast_db.rs:67-87` `load_active_document` | tree |
| `…/ast_db.rs:94-114` `load_working_document` | tree |
| `…/ast_db.rs:262`, `:325`, `:383`, `:436` (internal re-loads) | tree |
| `rust/src/api/trading_matrix_routes.rs:59` — **HTTP read path**, route `:85` | tree |
| `rust/src/services/trading_profile_document_impl.rs:32` (trait impl) | tree |
| `rust/crates/sem_os_postgres/src/ops/trading_profile_ca.rs:33` (CA verbs) | tree |
| `ob-poc-trading-profile/src/document_ops.rs:52`, `:2084` | **flat** |
| `rust/src/domain_ops/trading_profile.rs:319` (materialize) | **flat** |

Service trait: `dsl-runtime/src/service_traits/trading_profile_document.rs:40,45`,
registered at `ob-poc-web/src/main.rs:1131-1137`.

**Metadata-only readers** (touch the row, not `document` — safe under any
reshape): `sem_os_postgres/src/constellation_hydration.rs:270-283`;
`rust/src/database/context_discovery_service.rs:161-174`;
`rust/src/database/visualization_repository.rs:1272-1279`, `:1301-1309`;
`rust/src/database/semantic_state_service.rs:151`;
`dsl-runtime/src/cross_workspace/slot_state.rs:144`, `:171`.

### TypeScript

`ob-poc-ui-react/src/api/tradingMatrix.ts` — node mirror `:192-201`, document
mirror `:222-232`, fetch `:252`, `getNodeTypeIcon` `:258`, `getNodeTypeLabel`
`:324`. `…/features/viewport/components/TradingMatrixTree.tsx` — `:114-116`,
`:179-188`, `:303`, `:446`, `:535`. `…/features/viewport/ViewportPage.tsx` —
`:28`, `:323-324`, `:384`.

### SQL

`v_active_trading_profiles` (`master-schema.sql:19444-19456`) — exposes
`tp.document` verbatim.

### Unwired

`rust/crates/inspector-projection/src/generator/matrix.rs` `MatrixGenerator`
(reads `leaf_count` `:87`, `total_leaf_count` `:78`/`:104`,
`specificity_score` `:487`) — **no callers anywhere**; repo-wide grep outside
the crate returns only re-exports (`inspector-projection/src/lib.rs:72`,
`src/generator/mod.rs:22`).

**[I]** The compatibility surface is small and almost entirely internal. The
one genuinely external binding is the React Viewport tree, and it is bound to
exactly the presentation fields Law 2 wants removed.

## D.6 Is overlap between region-claiming rules illegal or specificity-resolved? **[V] — specificity-resolved**

Answered under Law 8. **Closes V&S §8 Open Ruling 4.** Consequences for
Design Law 8 and the `resolution_unambiguous` gate are stated there.

## D.7 Is a Rust-side taxonomy already present? **[V] — YES**

(v0.1 T0 deliverable 4 asked this; EOP-PLAN-MANDATE-001 §A2.6 listed it as
premise 6.) `TradingMatrixOp` (25 variants) and `TradingMatrixNodeType`
(8 variants) — §A.8. Assembly must be **consistent with these, not a second
author of them**.

## D.8 Additional finding: 24 verbs and one SQL function have no backing **[V]**

Not one of the six premises, but surfaced while answering D.4 and it changes
the plan's scope. Extracted every `crud.table` from the IM-referenced verb
files and checked each against the live catalogue with `to_regclass`:

| Surface | dead verbs | dead tables |
|---|---|---|
| `trade-gateway.*` | **8 of 14** — `define-gateway`, `read-gateway`, `list-gateways`, `add-routing-rule`, `list-routing-rules`, `remove-routing-rule`, `set-fallback`, `list-fallbacks` | `trade_gateways`, `cbu_gateway_routing`, `cbu_gateway_fallbacks` |
| `instruction-profile.*` | **7 of 7** — the entire surface | `instruction_message_types`, `instruction_templates`, `cbu_instruction_assignments`, `cbu_instruction_field_overrides` |
| `pricing-config.*` | **8 of 14** — `set-valuation-schedule`, `list-valuation-schedules`, `set-fallback-chain`, `list-fallback-chains`, `set-stale-policy`, `list-stale-policies`, `set-nav-threshold`, `list-nav-thresholds` | `cbu_valuation_schedule`, `cbu_pricing_fallback_chains`, `cbu_stale_price_policies`, `cbu_nav_impact_thresholds` |
| `instrument-matrix.*` | **1 of 2** — `instrument-matrix.instrument-matrix` | `v_permitted_instruments` |

Clean surfaces, for contrast: `settlement-chain.*` (19/19 backed),
`isda.*` (6/6), `corporate-action.*` (9/9), `tax-config.*` (11/11),
`entity-settlement.*` (3/3), `investment-manager.*` (7/7),
`cash-sweep.*` (9/9), `matrix-overlay.*` (9/9), `delivery.*` (5/5).

**Plus a missing SQL function.** `cbu-custody.lookup-ssi` executes:

```rust
// rust/crates/sem_os_postgres/src/ops/custody.rs:178-179
SELECT ssi_id, ssi_name, rule_id, rule_name, rule_priority, specificity_score
FROM "ob-poc".find_ssi_for_trade($1, $2, $3, $4, $5, $6, NULL)
```

`find_ssi_for_trade` is **absent from `pg_proc` in the live database** and
from `master-schema.sql`. The production SSI-resolution path — the thing that
answers "which SSI does this trade settle to" — is dead. The test at
`rust/tests/custody_integration.rs:581` even hedges
(*"Use the find_ssi_for_trade function if it exists, otherwise manual query"*)
and then does not call it. Separately, `custody.rs:174` types
`specificity_score` as `Option<rust_decimal::Decimal>` against an `integer`
column.

**[I]** This is the same defect class the recent share-register work
addressed: verbs declared against schema that never shipped. Recommended
disposition is deletion, not table-building — see the v0.2 plan's T-kill.

---

# §E Claims in the drafted documents that are now contradicted

| # | Document | Claim | Verdict | Evidence |
|---|---|---|---|---|
| 1 | VANILLA v0.1 §1 | Instrument types are equity / govt bond / corporate bond / ETF / money-market / FX / listed deriv / OTC deriv | **Wrong level.** 7 macro-driven *families* over 67 classes; govt/corporate bond and ETF are classes, not families | §A.7 |
| 2 | VANILLA v0.1 §1 | Instruction routes = SWIFT MT / ISO 20022 / blotter-file / manual | **Wrong twice.** The routing layer is `settlement_chain_surface` + `trade_gateway_surface`; blotter/file exist nowhere in schema; ISO 20022 exists only in an unapplied migration | §A.1, §D.4 |
| 3 | VANILLA v0.1 §1 | Pricing family = fee basis (bps on AUM, per-transaction, tiered, minimum charge) | **Category error.** IM pricing is `cbu_pricing_config` (source / price type / staleness). Fee basis is the `billing`/`deal` domain | §D.4 |
| 4 | VANILLA v0.1 §1 / task brief | Instrument families are verbs | **Precision correction.** They are macros (Tier -2B) that expand to atomic verbs | §A.7 |
| 5 | Task brief | `pricing_preference` product-gated via `requires_products:` | **Not implemented.** One occurrence repo-wide, in a comment | §D.4 |
| 6 | VS v0.3 §5 Law 8; §6 `resolution_unambiguous`; §8 Ruling 4 | Ambiguous overlap is illegal | **Contradicted by implementation.** Specificity ranking, never rejection. **V&S should give way** | Law 8 |
| 7 | VS v0.3 §2.3–2.5 | Routes and pricing are *the* region-claiming families | **Mis-located.** The implemented region-claim mechanism is `ssi_booking_rules` (SSI selection); pricing and settlement-location preferences follow the same pattern; "routes" as described do not exist | Law 8, §D.4 |
| 8 | VS v0.3 §8 Ruling 1 | Booking rules: stored fact vs generated taxonomy — "evidence favours the former" | **Confirmed and closed.** Stored fact: negotiated `priority` + generated `specificity_score` | Law 8 |
| 9 | VS v0.3 §3.2 | "Rules are a versioned input to board identity" | **Not applicable today.** No rule engine exists | Law 6 |
| 10 | PLAN-001 §A1.5 | "CSA exists only as a comment" | **False for the live path.** True only of the plpgsql (`20260106_…sql:269`). `TradingMatrixOp::AddCsa`/`RemoveCsa` and `ast_builder::add_csa:888` are live; `csa_agreements` is a real, populated table | §A.8, §D.2 |
| 11 | PLAN-001 §A1.6 | Reversing document→entities is "the single largest directional change in the refactor" | **Overstated.** 0 recorded materialisation events, ever | §C.2 |
| 12 | PLAN-001 §A3 and T3 | The defect is the plpgsql generator; T3 retires it | **Mis-located.** One-shot, already run, inert. The live generator is Rust | §C.1 |
| 13 | PLAN-001 §A1.2 | Unstable identity is "confined to the AST generator" | **Half right.** True of the migration. The live Rust path derives node ids from *path segments* — stable under reload, but arrangement-derived, so it violates Law 1 rather than Law 5 | Law 1, Law 5 |
| 14 | PLAN-001 §A2.3 / VANILLA §1 / task brief | The AST stores `parent` / `path` / `ordinal` / `leaf_count` | **Three of four are false.** No `parent`, `path`, or `ordinal` anywhere. Real arrangement carriers: `children`, path-array `id`, `leaf_count` | Law 1 |
| 15 | PLAN-001 §A2.2 | Table maturity beyond SSI is "assumed" | **Now verified — all four are at parity.** Premise closed favourably | §D.2 |
| 16 | PLAN-001 §A1.3 | `cbu_ssi_agent_override.sequence_order` "may be legitimate… but it is the exact shape the design laws forbid" | **Refined.** Ordered-fact carve-out is justified, but this column is inert (0 rows, 0 readers, no semantics asserted); cite `settlement_chain_hops.hop_sequence` instead | §D.3 |
| 17 | VANILLA v0.1 §2 T2/T3/T6 | Build the coordinate space, agreement prerequisites, and legal-move grammar | **Already exist.** `cbu_instrument_universe`; ISDA/CSA tables + `AddIsda`/`AddCsa` + an FK-enforced `csa_requires_isda`; 25 `TradingMatrixOp` variants | §A.8, §D.2 |
| 18 | VANILLA v0.1 §2 T0 | The brick alphabet is blocking and unknown | **Discharged by this document** | §A |
| 19 | Both plans | Gates are described as "fuzzable" | **ob-poc has zero fuzz infrastructure**, and `EOP-FUZZ-KYCUBO-001` is DRAFT, not ratified. Gates must be authored as ordinary tests first | See v0.2 §4 |

---

# §F Field-level classification

Categories: **CF** constitutive fact · **ARR** arrangement · **PRE**
presentation · **NEG** negotiated term · **⚠** disputed, flagged not
resolved. Identity/audit columns (`*_id` PKs, `created_at`, `updated_at`,
`created_by`, `is_active`, `effective_date`, `expiry_date`) are CF
throughout and elided per table.

### `cbu_ssi` (`master-schema.sql:9244-9269`)

| Column | Cat |
|---|---|
| `cbu_id`, `market_id` | CF (constitutive references) |
| `ssi_name`, `ssi_type` | CF |
| `safekeeping_account`, `safekeeping_bic`, `safekeeping_account_name` | CF |
| `cash_account`, `cash_account_bic`, `cash_currency` | CF |
| `collateral_account`, `collateral_account_bic` | CF |
| `pset_bic`, `receiving_agent_bic`, `delivering_agent_bic` | CF |
| `status` | CF (lifecycle) |
| `source`, `source_reference` | CF (provenance) |

### `cbu_ssi_agent_override` (`:9283-9294`)

| Column | Cat |
|---|---|
| `ssi_id`, `agent_role`, `agent_bic`, `agent_account`, `agent_name` | CF |
| **`sequence_order`** | **⚠** — ordered fact by intent, inert in practice. §D.3. Ruling needed |
| `reason` | CF (provenance of the override) |

### `cbu_instrument_universe` (`:8575-8589`)

| Column | Cat |
|---|---|
| `cbu_id`, `instrument_class_id`, `market_id`, `counterparty_entity_id` | CF — **this is the coordinate** |
| `currencies[]`, `settlement_types[]` | CF, but denormalised as text arrays |
| `is_held`, `is_traded` | CF |
| `counterparty_key` | **⚠** — sentinel-UUID artefact of NULL-tolerant natural keys. Not a fact; a storage workaround |

### `ssi_booking_rules` (`:18486-18530`)

| Column | Cat |
|---|---|
| `cbu_id`, `ssi_id` | CF |
| `rule_name` | CF (identity), PRE (used as a display label) |
| **`priority`** | **NEG** — negotiated precedence, per V&S §8 Ruling 1 |
| `instrument_class_id`, `security_type_id`, `market_id`, `currency`, `settlement_type`, `counterparty_entity_id` | CF — **the region claim**; NULL = wildcard |
| `isda_asset_class`, `isda_base_product` | CF, denormalised (shadows `isda_product_taxonomy`, no FK) |
| **`specificity_score`** | **⚠** — derived fact, generated from the criteria. Not arrangement, not presentation. But three incompatible scales exist (Law 8) |

### `isda_agreements` (`:14246-14257`) / `csa_agreements` (`:10716-10729`)

| Column | Cat |
|---|---|
| `cbu_id`, `counterparty_entity_id`, `agreement_date`, `governing_law` | CF |
| `isda_id` (on CSA) | CF — the prerequisite reference, FK-enforced |
| `csa_type` | CF (free text, no CHECK — see §D.2) |
| `threshold_amount`, `threshold_currency`, `minimum_transfer_amount`, `rounding_amount` | **NEG** |
| `collateral_ssi_id` | CF |

### `cbu_pricing_config` (`:8823-8845`)

| Column | Cat |
|---|---|
| `cbu_id`, `profile_id`, `pricing_resource_id` | CF |
| `instrument_class_id`, `market_id`, `currency` | CF — **the region claim** |
| `priority` | **NEG** |
| `source`, `price_type`, `fallback_source` | CF |
| `max_age_hours`, `tolerance_pct`, `stale_action` | **NEG** |

### `cbu_im_assignments` (`:8532-8560`)

| Column | Cat |
|---|---|
| `cbu_id`, `profile_id`, `manager_entity_id`, `manager_lei`, `manager_bic`, `manager_name`, `manager_role` | CF |
| `priority` | **NEG** |
| `scope_all`, `scope_markets[]`, `scope_instrument_classes[]`, `scope_currencies[]`, `scope_isda_asset_classes[]` | CF — **region claim**, denormalised as text arrays |
| `instruction_method`, `instruction_resource_id` | CF |
| `can_trade`, `can_settle`, `can_affirm` | **NEG** (mandate permissions) |
| `status`, `termination_date` | CF (lifecycle) |

### `cbu_settlement_chains` / `settlement_chain_hops`

| Column | Cat |
|---|---|
| `cbu_id`, `chain_name`, `market_id`, `instrument_class_id`, `currency`, `settlement_type` | CF |
| `is_default` | **NEG** |
| `lifecycle_status` | CF (lifecycle; DAG-aligned — §A.10) |
| `chain_id`, `role`, `intermediary_entity_id`, `intermediary_bic`, `intermediary_name`, `account_number`, `ssi_id`, `instructions` | CF |
| **`hop_sequence`** | **CF — ordered fact.** Recommended exemplar for the carve-out (§D.3) |

### `cbu_trading_profiles` (`:9457-9487`)

| Column | Cat |
|---|---|
| `cbu_id`, `group_id`, `template_id`, `source_document_id`, `sla_profile_id` | CF |
| `version`, `status` | CF (lifecycle) |
| `is_template` | CF |
| **`document`** | **⚠ — mixed.** Contains CF, ARR, and PRE simultaneously. This *is* the defect (§C) |
| `document_hash` | Derived identity — but see Law 5 (two algorithms) |
| `materialization_status`, `materialized_at`, `materialization_hash` | CF (provenance of a path that never runs — §C.2) |
| `validated_*`, `submitted_*`, `rejected_*`, `activated_*`, `superseded_*`, `notes` | CF (audit) |

### `TradingMatrixNode` (`ob-poc-types/src/trading_matrix.rs:654-683`)

| Field | Cat |
|---|---|
| `node_type` (+ its typed payload) | **CF** |
| `id` (path array) | **ARR** |
| `children` | **ARR** |
| `leaf_count`, root `total_leaf_count` | **ARR** (derived from arrangement) |
| `label`, `sublabel` | **PRE** |
| `status_color` | **PRE** |
| `is_loaded` | **PRE** (lazy-loading hint — pure client concern) |

**Gate check (v0.1 T0 deliverable 5):** every column above carries exactly
one category, and the four disputed items — `sequence_order`,
`counterparty_key`, `specificity_score`, `cbu_trading_profiles.document` —
are **flagged for ruling, not silently resolved.**

---

# §G What cannot be answered from the code

Each with what would settle it.

1. **Is `sequence_order` *intended* as settlement chain order?**
   Nothing in the schema asserts it (no comment, no CHECK), nothing reads it,
   and the table has zero rows. §D.3's verdict is **[I]**, inferred from
   fixture values and constraint shape.
   *Settled by:* a domain ruling from Adam, or the origin of
   `rust/tests/scenarios/valid/ssi_onboarding_document.json`.

2. **Are the 16 ACTIVE flat profiles live production data or stale fixtures?**
   This determines whether §C's break is a live incident or a test-data
   artefact, and therefore how the T-cutover tranche is sequenced.
   *Settled by:* data ownership / provenance; and
   `"ob-poc".trading_profile_migration_backup`, which the 2026-01-06
   migration populated (`20260106_…sql:348-353`, `:382-383`).

3. **Was `rust/migrations/202501_instruction_gateway.sql` abandoned or merely
   deferred?** It defines a coherent, well-shaped instruction-routing model
   against a `custody` schema that was never created, and 24 verbs were
   authored against it. Deleting the verbs (recommended) versus applying the
   migration are opposite calls.
   *Settled by:* whoever authored it; or a product decision on whether
   message-type-level instruction routing is in scope at all.

4. **Is coverage intended to be total, or coverage-or-explicit-fallback?**
   (VANILLA v0.1 §4 Risk 4, still open.) No fallback mechanism exists in the
   schema — the only modelled failure is zero matches
   (`custody.rs:206-211`), and `cbu_gateway_fallbacks` does not exist.
   *Settled by:* a domain ruling on whether an unrouted tradeable coordinate
   is an error or an accepted manual-handling case.

5. **Do the `green_when` and `predicate_bindings` blocks (§A.3.1) have an
   intended evaluator?** They are richer than anything else in the file and
   nothing consumes them.
   *Settled by:* the Catalogue Platform v1.3 authors, or the
   `tranche-2-cross-workspace-reconciliation` document the DAG header cites
   (`:82`).

6. **Does the KYC/UBO placement machinery actually fit as the mandate
   grammar?** Carried unresolved from EOP-PLAN-MANDATE-001 T5 ⚠ and VANILLA
   v0.1 T6 ⚠. **Not investigated in this pass** — out of the scope set by
   the task brief.
   *Settled by:* a type-level fit check of `ob-poc-kyc-substrate`'s placement
   and preview types against `TradingMatrixOp`.

7. **Should `SetNodeStatus` be a legal move?** (§A.8.) It mutates only a
   presentation field. Under Law 2 it is not a move at all.
   *Settled by:* a ruling in v0.2 T1 — with the caveat that the UI reads
   `status_color` from storage (Law 2).

---

## §H Reproducing the live-database claims

All four queries are read-only. Run via `psql -d data_designer` or the
`ob-poc-db` MCP tool.

```sql
-- 1. Document-shape distribution by status (§C.3)
SELECT status, count(*) AS n,
       count(*) FILTER (WHERE document ? 'version')  AS has_version,
       count(*) FILTER (WHERE document ? 'children') AS has_children,
       count(*) FILTER (WHERE document ? 'universe') AS has_universe
FROM "ob-poc".cbu_trading_profiles GROUP BY status;

-- 2. Materialisation has never run (§C.2)
SELECT count(*) FROM "ob-poc".trading_profile_materializations;

-- 3. Which verb-declared tables actually exist (§D.8)
SELECT t.tbl, (to_regclass('"ob-poc".'||t.tbl) IS NOT NULL) AS exists_live
FROM (VALUES ('trade_gateways'),('cbu_gateway_routing'),('cbu_gateway_fallbacks'),
             ('instruction_message_types'),('instruction_templates'),
             ('cbu_instruction_assignments'),('cbu_instruction_field_overrides'),
             ('cbu_valuation_schedule'),('cbu_pricing_fallback_chains'),
             ('cbu_stale_price_policies'),('cbu_nav_impact_thresholds'),
             ('v_permitted_instruments'),('cbu_ssi'),('ssi_booking_rules'),
             ('cbu_instrument_universe'),('isda_agreements'),('csa_agreements'),
             ('cbu_settlement_chains'),('settlement_chain_hops'),
             ('cbu_pricing_config'),('cbu_im_assignments'))
     AS t(tbl) ORDER BY 2, 1;

-- 4. find_ssi_for_trade does not exist (§D.8)
SELECT p.proname, n.nspname FROM pg_proc p
JOIN pg_namespace n ON n.oid = p.pronamespace
WHERE p.proname IN ('find_ssi_for_trade','migrate_trading_profile_to_ast',
                    'needs_ast_migration','uuidv7');
```

Row counts in §D.2 are plain `SELECT count(*)` per table.
