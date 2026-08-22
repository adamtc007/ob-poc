# Vanilla Investment Mandate — Vertical Slice Plan

### EOP-PLAN-MANDATE-VANILLA-001

| | |
|---|---|
| **Document** | EOP-PLAN-MANDATE-VANILLA-001 |
| **Type** | Tranche plan for implementation |
| **Version** | **0.2** — supersedes v0.1 (**v0.1 retained unmodified for the audit trail**) |
| **Implements** | EOP-VS-MANDATE-001 v0.3, **as amended by** EOP-FIND-MANDATE-T0-001 |
| **T0 status** | **DISCHARGED** — see EOP-FIND-MANDATE-T0-001 |
| **Status** | Draft |

## Change log v0.1 → v0.2

1. **T0 is closed.** §1's inferred brick alphabet is replaced with cited
   fact. The ⚠ "CORRECT THIS FIRST" block is gone.
2. **The board and its move grammar already exist and are implemented.**
   T2, T3 and T6 collapse from *build* to *conform*.
3. **Two new tranches, both ahead of everything else** — T-cutover (a live
   break) and T-kill (24 unbacked verbs). Neither existed in v0.1 because
   neither was known.
4. **Design Law 8 is amended**, and its gate is split into three.
5. **All gates reconciled against `EOP-FUZZ-KYCUBO-001`** — §4. The headline
   constraint: ob-poc has **zero** fuzz infrastructure and that document is
   **not ratified**, so no gate here may assume a fuzz harness exists.
6. **Risks rewritten.** The v0.1 #1 risk ("alphabet is inferred") is retired;
   three new ones replace it.

---

## §0 What this slice is, and why it is now smaller

v0.1 assumed a greenfield model to be built. The T0 research found otherwise:

- **The board exists.** `rust/config/sem_os_seeds/dag_taxonomies/instrument_matrix_dag.yaml`
  — 19 verb surfaces, 21 slots, 12 structured state machines, 8 derived
  lifecycle phases, 10 cross-slot constraints, 5 prune-cascade rules. Ratified
  shape per `EOP-DD-DAGBPMN-001` (RATIFIED 2026-08-17): the
  `dag_taxonomies/*.yaml` form already *is* "a DAG entity-state model with DSL
  moves attached to the board."
- **The move grammar exists — in config and in code.** Every transition the
  DAG annotates `# NEW verb — P.3` now exists in verb YAML (findings §A.9),
  and `TradingMatrixOp` (`rust/crates/ob-poc-types/src/trading_matrix.rs:964+`)
  is a 25-variant move enum in production.
- **The entity layer exists.** Universe, booking rules, ISDA, CSA are all at
  or above `cbu_ssi`'s maturity — uuidv7 identity, real FKs (findings §D.2).

What does **not** exist is a single authority over the assembled document,
and that failure is currently live in production. So this slice is not
"build a mandate model." It is:

> **Make the existing board single-authored, reachable, and honest — then
> prove the properties the V&S claims for it.**

**In the slice:** one document authority; removal of capability that has no
backing; conformance of block types, coordinates and agreements to what
already exists; the region-claim properties (the one genuinely novel piece);
assembly; grammar as sole mutation path; a worked end-to-end mandate.

**Not in the slice:** the full 67-class instrument taxonomy; message-type-level
instruction routing (see T-kill); the view layer beyond what T1 is forced to
touch; the Java 25 consumer API.

**Authority split, preserved [VERIFIED]** (`instrument_matrix_dag.yaml:72-78`):
`instrument_template.yaml` + `trading_streetside.yaml` remain authoritative for
REPL/agent navigation. The DAG file is authoritative for **state-machine
semantics only**. No tranche here changes that split.

---

## §1 Brick alphabet — cited, not asserted

> The v0.1 §1 inference is **withdrawn**. The authoritative enumeration is
> **EOP-FIND-MANDATE-T0-001 §A**, from which this section is a summary. Where
> the two differ, the findings note wins.

### 1.1 Sources of record

| What | Where |
|---|---|
| Slots, states, transitions, constraints | `rust/config/sem_os_seeds/dag_taxonomies/instrument_matrix_dag.yaml` |
| Instrument families | `rust/config/verb_schemas/macros/instrument.yaml` |
| Implemented move enum + node taxonomy | `rust/crates/ob-poc-types/src/trading_matrix.rs` |
| Coordinate space | `"ob-poc".cbu_instrument_universe` (`migrations/master-schema.sql:8575-8589`) |
| Region-claim mechanism | `"ob-poc".ssi_booking_rules` (`:18486-18530`) |

### 1.2 Instrument families — 7, and they are **macros**, not verbs

`instrument.yaml` header: *"The 67 instrument classes collapse to 7 asset
families."* Cross-checked against the DAG's independently-authored
`asset_family` list (`instrument_matrix_dag.yaml:1432`) — exact agreement.

| Macro | Line | `component-type` | Expands to |
|---|---|---|---|
| `instrument.setup-equity` | :19 | `EQUITY` | `trading-profile.add-component`, `settlement-chain.create-chain`, `cbu-custody.derive-required-coverage` |
| `.setup-fixed-income` | :75 | `FIXED_INCOME` | `add-component`, `create-chain` |
| `.setup-otc-derivatives` | :124 | `OTC_DERIVATIVE` | `add-component`, `isda.create`, `trade-gateway.define-gateway` |
| `.setup-listed-derivatives` | :176 | `LISTED_DERIVATIVE` | `add-component`, `define-gateway`, `create-chain` |
| `.setup-fx` | :224 | `FX` | `add-component`, `trading-profile.set-base-currency` |
| `.setup-money-market` | :267 | `MONEY_MARKET` | `add-component`, `create-chain` |
| `.setup-funds` | :311 | `FUND` | `add-component` |

Macros are a **planning and compilation surface, not an execution surface**
(CLAUDE.md, "What 'macro' means in ob-poc"). The compiler expands them to
atomic verbs; the REPL executes only those. Any grammar built here binds to
the atomic layer.

### 1.3 The implemented move alphabet — 25 variants

`TradingMatrixOp` (`trading_matrix.rs:964+`): `AddInstrumentClass`, `AddMarket`,
`AddCounterparty`, `AddUniverseEntry`, `AddSsi`, `ActivateSsi`, `SuspendSsi`,
`AddBookingRule`, `AddSettlementChain`, `AddSettlementHop`, `AddIsda`,
`AddCsa`, `AddProductCoverage`, `AddTaxJurisdiction`, `AddTaxConfig`,
`AddImMandate`, `UpdateImScope`, `AddCsaEligibleCollateral`, `LinkCsaSsi`,
`RemoveIsda`, `RemoveCsa`, `SetBaseCurrency`, `AddAllowedCurrency`,
`RemoveNode`, `SetNodeStatus`.

Node taxonomy `TradingMatrixNodeType`, 8 variants (`:289+`): `Category`,
`InstrumentClass`, `Market`, `Counterparty`, `UniverseEntry`, `Ssi`,
`BookingRule`, `SettlementChain`.

**`SetNodeStatus` needs a ruling in T1** — its only effect is setting
`status_color`, a presentation field. Under Law 2 it is not a move.

### 1.4 The four corrections to v0.1 §1

| v0.1 said | Fact |
|---|---|
| Instrument types: equity / govt bond / corporate bond / ETF / money-market / FX / listed deriv / OTC deriv | Wrong level — 7 *families* over 67 *classes*. Govt/corporate bond and ETF are classes |
| Instruction routes: SWIFT MT / ISO 20022 / blotter-file / manual | The routing layer is **settlement chains** + **trade gateways**. Blotter/file exist nowhere in schema; ISO 20022 only in an unapplied migration |
| Pricing: fee basis (bps on AUM, per-transaction, tiered) | Category error. IM pricing is `cbu_pricing_config` (source / price type / staleness). Fee basis is the `billing`/`deal` domain |
| Instrument families are verbs | They are macros |

### 1.5 Open Rulings — both now closed

- **Ruling 3 (route granularity).** *Closed as a design question, not a
  discovery one.* The intended granularity is **message-type level** —
  `instruction-profile.define-message-type` keys on
  `(lifecycle_event, message_standard, message_type)` with examples `MT540`,
  `sese.023.001.09`. But **nothing implements it**: all four backing tables
  are absent from the live database. The only shipped modelling is
  `cbu_im_assignments.instruction_method varchar(20)` with a 6-value CHECK
  (`SWIFT|CTM|FIX|API|MANUAL|ALERT`), on a table with 0 rows. See T-kill.
- **Ruling 4 (overlap).** *Closed: specificity-resolved, not illegal.*
  `ssi_booking_rules.specificity_score` is a stored generated bitmask (0–63);
  every resolution path is `ORDER BY priority ASC, specificity_score DESC
  LIMIT 1`; nothing anywhere rejects an overlap. **Design Law 8 is amended
  accordingly** — see §1.6.

### 1.6 Amendment to V&S §5 Law 8

> **Law 8 (as amended).** *Resolution is total and deterministic.* Every
> eligible coordinate resolves to exactly one route and one price, by a
> declared precedence rule, with no arbitrary tie-break.

The V&S gives way: overlap resolution by specificity is implemented,
in production, and encodes a deliberate dimension precedence. But two genuine
holes remain, and they get their own gates (§2 T4):

- `ssi_booking_rules` has **no UNIQUE on the criteria tuple** — two rules with
  identical criteria and priority tie, and `LIMIT 1` picks arbitrarily.
- `cbu_pricing_config` has **no specificity mechanism at all** — only
  `priority integer DEFAULT 1`, no UNIQUE on the region.
- **Three incompatible specificity scales coexist**: DB bitmask 0–63, an
  independent 0–6 count in `ast_builder.rs:690-699`, and a hand-seeded `100`
  in `scripts/seed_allianz_trading_matrix.sql:154`.

### 1.7 An ordered-fact carve-out is justified

Settlement chains are ordered by market convention, and that order is
constitutive. The design laws need an explicit **ORDERED FACT** category,
distinct from display ordinals.

**Cite `settlement_chain_hops.hop_sequence`** (`master-schema.sql:17916`,
UNIQUE `(chain_id, hop_sequence)` `:27643`, role CHECK `:17927`, table comment
`:17935`) as the exemplar — **not** `cbu_ssi_agent_override.sequence_order`,
which has no comment, no CHECK, zero readers, and zero rows.

---

## §2 Tranches

### T0 — Alphabet and premise closure — **CLOSED**

Discharged by **EOP-FIND-MANDATE-T0-001**. All six premises answered, both
Open Rulings closed, field-level classification complete with four disputed
items explicitly flagged rather than silently resolved. **No longer blocking.**

Four questions remain genuinely unanswerable from code and are listed in that
document §G. Two of them gate tranches here and are called out below.

---

### T-cutover — One document authority ⟵ **NEW, first**

**Why first.** This is Law 7 failing in production, not in principle, and no
later tranche is meaningful until it is fixed: with two shapes in one column
there is nothing single to assemble, hash, or round-trip.

**The break** (findings §C.3): `cbu_trading_profiles.document jsonb NOT NULL`
carries two mutually-undeserialisable Rust types —
`TradingMatrixDocument` (tree; requires `version`, `children`, neither
defaulted) and `TradingProfileDocument` (flat; requires `universe`, not
defaulted). Live: **all 417 tree documents are DRAFT; all 16 ACTIVE documents
are flat.** Consequences:

1. `ast_db::load_active_document` filters `status='ACTIVE'` and parses as the
   tree type → fails `missing field \`version\`` on **every ACTIVE profile**.
   That is the sole path behind `GET /api/cbu/:cbu_id/trading-matrix`
   (`rust/src/api/trading_matrix_routes.rs:59`) and the React Viewport page.
2. `trading-profile.materialize` parses as the flat type → fails
   `missing field \`universe\`` on **all 417 tree DRAFTs**. Consistent with
   `SELECT count(*) FROM "ob-poc".trading_profile_materializations` = **0**.

**Scope.** A decision plus its cutover:

- Which shape wins. *(Recommendation: the tree — it is the live authoring
  path, the API contract, and the UI binding. The flat type's only unique
  consumer is a materialiser that has never run.)*
- Disposition of the 16 ACTIVE flat rows and the 417 tree DRAFTs. **Blocked
  on findings §G2** — whether those 16 rows are production data or stale
  fixtures. Resolve before choosing between convert / re-derive / quarantine.
- Whether `trading-profile.materialize` is repaired or retired. Given 0 runs
  ever, and given `cbu_ssi`/`ssi_booking_rules`/`cbu_instrument_universe` are
  populated by other paths, retirement is the honest default. If it is kept,
  it must run green on a real profile.
- Retire the two contradicting module doc comments (`ast_db.rs:8-9` vs
  `document_ops.rs:3-4`) and the stale column/table comments
  (`master-schema.sql:9494`, `:9503`, `:9511`).

**Gates**

- **`single_document_shape`** — every row in `cbu_trading_profiles`
  deserialises into exactly one declared type. Asserted over the **live
  corpus**, not a fixture — a fixture-only version of this gate is gameable
  and would have passed throughout the period the break existed.
- **`active_profile_loads`** — `load_active_document` succeeds for every
  ACTIVE profile. **RED today**; that is the point. Provable RED before the
  fix, per §4's red-first rule.
- **`materialize_or_retire`** — either `trading-profile.materialize` succeeds
  end-to-end on a real profile, or the verb, its op, its audit table and its
  `materialization_*` columns are removed. No third state; "present but never
  exercised" is what produced this defect.
- **`one_document_hash_algorithm`** — a single algorithm writes
  `document_hash`, and the column comment matches it. (Today: `md5` in the
  migration, SHA-256 in Rust, comment says SHA-256.)

---

### T-kill — Remove unbacked capability ⟵ **NEW, gates the rest**

**Decision: delete, do not build.** Same defect class as the recent
share-register kill list — verbs declared against schema that never shipped.
Building the tables to satisfy the verbs would import a routing model nobody
has decided to own.

**Inventory** (findings §D.8; verified against the live catalogue with
`to_regclass`):

| Surface | Dead verbs | Dead tables |
|---|---|---|
| `trade-gateway.*` | **8 of 14** — `define-gateway`, `read-gateway`, `list-gateways`, `add-routing-rule`, `list-routing-rules`, `remove-routing-rule`, `set-fallback`, `list-fallbacks` | `trade_gateways`, `cbu_gateway_routing`, `cbu_gateway_fallbacks` |
| `instruction-profile.*` | **7 of 7 — the entire surface** | `instruction_message_types`, `instruction_templates`, `cbu_instruction_assignments`, `cbu_instruction_field_overrides` |
| `pricing-config.*` | **8 of 14** — `set-valuation-schedule`, `list-valuation-schedules`, `set-fallback-chain`, `list-fallback-chains`, `set-stale-policy`, `list-stale-policies`, `set-nav-threshold`, `list-nav-thresholds` | `cbu_valuation_schedule`, `cbu_pricing_fallback_chains`, `cbu_stale_price_policies`, `cbu_nav_impact_thresholds` |
| `instrument-matrix.*` | **1 of 2** — `instrument-matrix.instrument-matrix` | `v_permitted_instruments` |

**Plus a missing SQL function.** `cbu-custody.lookup-ssi` executes
`SELECT ... FROM "ob-poc".find_ssi_for_trade($1,...,NULL)`
(`rust/crates/sem_os_postgres/src/ops/custody.rs:178-179`).
`find_ssi_for_trade` is **absent from `pg_proc`** on the live database. The
production "which SSI does this trade settle to" path is dead. This one is
**not** a deletion candidate — SSI resolution is the region-claim mechanism
T4 is built on, so it must be repaired, and repairing it is a T4 prerequisite.

**Blocked on findings §G3** — whether
`rust/migrations/202501_instruction_gateway.sql` (which defines the whole
instruction-routing model against a `custody` schema that was never created)
was abandoned or deferred. Deletion and application are opposite calls;
this needs an owner's answer, not an inference.

**Also in scope, same class:** `trading-profile.restrict` /
`.lift-restriction` declare `state_effect: transition` and
`target_slot: trading_profile`, but the slot has no `RESTRICTED` state and no
transition via either verb (findings §A.10). Either add the state or drop the
transition claim — not both, and not neither.

**Clean surfaces, untouched:** `settlement-chain.*` (19/19),
`isda.*` (6/6), `corporate-action.*` (9/9), `tax-config.*` (11/11),
`entity-settlement.*` (3/3), `investment-manager.*` (7/7),
`cash-sweep.*` (9/9), `matrix-overlay.*` (9/9), `delivery.*` (5/5).

**Gates**

- **`no_unbacked_crud_verbs`** — for every verb declaring `behavior: crud`,
  its `crud.table` resolves via `to_regclass`. Repo-wide, cheap,
  non-gameable, and **should outlive this slice** as a standing invariant
  alongside `cargo x registry-graph`.
- **`no_missing_sql_functions`** — every SQL function referenced from a
  plugin op exists in `pg_proc`. Narrower than the above but the same shape.
- **`no_orphan_transition_claims`** — every verb declaring
  `three_axis.state_effect: transition` with `transition_args.target_slot: S`
  appears as a `via:` in slot `S`'s transitions. This is the defect class this
  codebase has now hit three times.
- Standard removal hygiene, per the share-register precedent: YAML block +
  Rust op + registry registration + stale `verb_pattern_embeddings` /
  `verb_centroids` / `phrase_bank` rows, then `cargo x verbs compile`,
  embeddings repopulate, `cargo x registry-graph` clean.

---

### T1 — Block types and content-derived identity

**Scope changes from v0.1: conform, don't author.** Reuse
`TradingMatrixNodeType` (8 variants) as the sealed block discriminant rather
than defining a parallel taxonomy. Findings §D.7 closed the "is a Rust-side
taxonomy already present" premise: it is, and assembly must be consistent with
it rather than a second author of it.

**The hard part is not the type — it is the UI.** `label`, `sublabel`,
`status_color`, `is_loaded`, `leaf_count` are read **straight out of the
persisted JSONB** by `TradingMatrixTree.tsx:114-116`, `:179-188`, `:446`,
`:535`. Stripping presentation is a **breaking UI change**, not a type tidy-up.
This tranche either owns that migration (recompute client-side) or explicitly
defers it and says so — silently keeping the fields while claiming Law 2 is
the failure mode to avoid.

**Also decide:** whether `SetNodeStatus` remains a legal move (§1.3).

**Gates** — unchanged from v0.1, all **type-level**, all runnable today as
ordinary tests:

- `no_arrangement_in_block` — no `parent`, `path`, `ordinal`, `depth`,
  `is_root`, or child-collection field on the block type. *(Note: v0.1 and
  EOP-PLAN-MANDATE-001 both claimed `parent`/`path`/`ordinal` are stored
  today. They are not — the real carriers are `children`, the path-array `id`,
  and `leaf_count`. The gate is still right; its premise was wrong.)*
- `no_presentation_in_block` — same, for `status_color` / `is_loaded` /
  `leaf_count` / `label` / `sublabel`.
- `identity_is_stable` — the same entity loaded twice yields the same block
  id; identity never calls a random or clock source. **Note the tension:**
  today's node id is *derived from path*, so it is stable under reload but
  changes when a node moves. Satisfying this gate and `no_arrangement_in_block`
  together requires re-basing identity on the entity's own `uuidv7` — which
  the entity layer already provides for all four families (findings §D.2).

---

### T2 — Coordinate space ⟶ **collapse to "conform"**

The coordinate space is not to be designed; it exists as
`"ob-poc".cbu_instrument_universe` (`master-schema.sql:8575-8589`, 5537 live
rows): `instrument_class_id`, `market_id`, `currencies[]`,
`settlement_types[]`, `counterparty_entity_id`, with 4 real FKs and a natural
key `(cbu_id, instrument_class_id, market_id, counterparty_key)`.

Remaining work is **asserting legality against it**, plus two honest
acknowledgements: `currencies`/`settlement_types` are text arrays rather than
FK rows, and `counterparty_key` is a sentinel-UUID workaround for
NULL-tolerant natural keys (findings §F).

**Gates** (v0.1's, re-scoped to the existing table)

- `invalid_coordinate_rejected` — fuzz coordinate tuples; every invalid
  combination is rejected with a typed error.
- `duplicate_placement_rejected` — whole-board uniqueness, not cursor-local.
  Partially enforced already by the natural key; the gate asserts the grammar
  agrees with the constraint.
- `placement_is_total` — success or typed rejection, never a panic, never a
  silently-wrong board.

---

### T3 — Agreements and prerequisites ⟶ **collapse to "conform"**

Already present: `isda_agreements` / `isda_product_coverage` /
`isda_product_taxonomy` / `csa_agreements`, all uuidv7 with real FKs
(findings §D.2); `TradingMatrixOp::AddIsda` / `AddCsa` / `RemoveIsda` /
`RemoveCsa` implemented (`ast_builder.rs:843`, `:888`).

- **`csa_requires_isda` is already enforced** — FK
  `csa_agreements.isda_id → isda_agreements(isda_id)` ON DELETE CASCADE
  (`master-schema.sql:37831`). An orphan CSA is already unconstructible at the
  storage layer. The gate becomes a *confirmation*, and should assert the
  grammar cannot construct one either.
- **`otc_requires_isda` is declared but unenforced** — the DAG declares it as
  `isda_coverage_required_for_derivative_trading`, `severity: error`
  (`instrument_matrix_dag.yaml:1301-1309`); nothing evaluates it. **This is
  T3's real work.**

**Gates**: `otc_requires_isda` (new enforcement), `csa_requires_isda`
(confirmation), `prerequisite_order_independent` (unchanged from v0.1 — still
forces the ruling on whether prerequisites are strictly ordered).

---

### T4 — Region claims ⟵ **still the risk tranche, now better specified**

The genuinely novel piece, and now with a concrete referent:
`ssi_booking_rules` is the implemented region-claim mechanism — 6 nullable
criteria dimensions (NULL = wildcard), a negotiated `priority`, and a
generated `specificity_score` bitmask. `cbu_pricing_config` and
`cbu_settlement_location_preferences` follow the same pattern with weaker
machinery.

**Prerequisite:** `find_ssi_for_trade` must be repaired (T-kill) — it is the
resolution function this tranche's gates measure.

**Gates**

- **`region_claim_matches_expected_coordinates`** — a claim covers exactly the
  coordinates its criteria describe; fuzz criteria, assert the covered set.
  (Unchanged from v0.1.)
- **`resolution_is_total_and_deterministic`** ⟵ *replaces
  `resolution_unambiguous`.* Every eligible coordinate resolves to exactly one
  route and one price, by the declared precedence, deterministically. Follows
  Law 8 as amended (§1.6).
- **`no_specificity_ties`** ⟵ *new.* No two active rules for one CBU share
  both `priority` and `specificity_score` with an identical criteria tuple.
  Today this is unconstrained and `LIMIT 1` picks arbitrarily.
- **`one_specificity_scale`** ⟵ *new.* Exactly one specificity computation
  exists. Today: three (DB bitmask 0–63, `ast_builder.rs:690-699` count 0–6,
  seeded `100`).
- **`coverage_complete_or_explicit_fallback`** ⟵ *amended from
  `coverage_complete`.* v0.1 §4 Risk 4 anticipated this; the research confirms
  it is unresolved. **Blocked on findings §G4** — no fallback mechanism exists
  in the schema (`cbu_gateway_fallbacks` does not exist; the only modelled
  failure is zero matches, `custody.rs:206-211`). Author the gate as
  coverage-or-explicit-fallback **and flag it**, rather than asserting blanket
  coverage that the domain may not want.

---

### T5 — Assembly

Unchanged from v0.1 in intent. One sequencing note: assembly is only
meaningful **after T-cutover** — with two shapes in one column there is
nothing single to assemble, and `order_independence` cannot be stated, let
alone tested.

Note also that today's `compute_document_hash` is SHA-256 over
`serde_json::to_string(doc)` with insertion-ordered `children`
(`ast_db.rs:147-152`, `trading_matrix.rs:724`) — so `order_independence`
would fail as currently implemented. That is a correct RED starting point.

**Gates**: `order_independence` (keystone), `assembly_is_total`,
`dangling_ref_is_loud`, `reverse_index_derived` — all unchanged.

---

### T6 — Legal-move grammar ⟶ **collapse to "conform"**, keep the keystone

The grammar exists: 25 `TradingMatrixOp` variants, applied through
`ast_builder::apply_op` (`:79`) and funnelled by `ast_db::apply_and_save`
(`:427-448`).

**The real work is Law 3: make it the sole mutation path.** Today
`document_ops.rs` writes the same column at 10 sites with a different document
type (findings, Law 3). Retiring those write paths *is* this tranche.

Enabled-move **enumeration** (not merely checking) remains the requirement, for
cheap fuzzing.

⚠ **KYC-substrate reuse is still an unverified premise** — carried from v0.1
T6, deliberately not investigated in the T0 pass (findings §G6). Verify the
type-level fit against `ob-poc-kyc-substrate`'s placement/preview types before
planning this tranche in detail.

**Gates**: `round_trip` (**the plan's keystone**, unchanged),
`grammar_preserves_invariants`, `illegal_moves_rejected`,
`cursor_never_persisted`, `known_mandates_reachable` — all unchanged, plus:

- **`single_mutation_path`** ⟵ *new.* A source-level assertion that
  `cbu_trading_profiles.document` has exactly one writing module.

---

### T7 — Vanilla mandate end-to-end

Unchanged from v0.1. Gates: `vanilla_mandate_builds`,
`vanilla_mandate_round_trips`, `traversal_neighbourhood`.

One addition, because the research showed the board is wider than its pack
(findings §A.11 — the IM pack owns 5 verb prefixes, 18 are pack-unowned, and
both its declared transitions carry `mutation_enabled: false`):

- **`slice_is_reachable_through_the_pack`** — the worked mandate is
  constructible through the governed pack surface, not only through direct op
  calls. Otherwise T7 proves the model works while the product still cannot
  reach it.

---

## §3 Sequencing

```
T-cutover ──► T-kill ──► T1 ──► T2 ──► T3
 (one          (delete    (blocks) (coords) (agreements)
  authority)    unbacked)                       │
                                                ▼
                                            T4 (regions — risk)
                                                │
                                T5 ──► T6 ──► T7
                            (assembly) (grammar) (slice)
```

- **T-cutover first, always.** Everything downstream assumes one document.
- **T-kill second.** Cheap, and its `no_unbacked_crud_verbs` gate should
  become a standing invariant before more surface is added.
- **T2 before T4** — regions claim over the coordinate space.
- **T5 before T6** — assembly is testable without the grammar; `round_trip`
  depends on assembly.
- **T4 remains the risk tranche.** Schedule review attention there.

**Two tranches are blocked on rulings that code cannot supply** — T-cutover on
findings §G2 (are the 16 ACTIVE rows production data?) and T-kill on §G3 (was
the instruction-gateway migration abandoned or deferred?). Both are one-question
answers; neither should be inferred.

---

## §4 Gate reconciliation against EOP-FUZZ-KYCUBO-001

**Existing conventions win.** The gates in v0.1 and in V&S §6 were written
from first principles; `EOP-FUZZ-KYCUBO-001` is the established convention in
this codebase family (itself modelled on bpmn-lite's ratified
`EOP-FUZZ-BPMN-ISA-002`). Where they differ, this section defers.

### 4.1 The binding constraint: there is no harness

**`EOP-FUZZ-KYCUBO-001` is DRAFT, not ratified** (`:3`), and **ob-poc has zero
fuzz infrastructure** (`:12`) — no `fuzz/` directories, no cargo-fuzz project;
`proptest` is declared at `rust/Cargo.toml:196` and used by nothing.

> **Therefore: no gate in this plan may assume a fuzz harness exists.** Every
> gate is authored to run as an ordinary `#[test]` first, with fuzzing as a
> later upgrade. A plan that lands red because its harness was never built has
> proved nothing.

Where v0.1 says "fuzzed", read: *"property stated as a test over enumerated or
hand-built cases now; upgraded to a fuzz target if and when F1 plumbing
lands."*

### 4.2 Vocabulary: oracles, not gate names

The convention is **targets** (an input shape) × **oracles** (a property
asserted), `EOP-FUZZ-KYCUBO-001 §3`. The V&S §6 "gates" are oracles in this
vocabulary. Restated rather than duplicated:

| Existing oracle | Statement | This plan's gates that instantiate it |
|---|---|---|
| **O1** No-panic | total function or `Result`, never a panic | `placement_is_total`, `assembly_is_total`, `resolution_is_total_and_deterministic` |
| **O2** Determinism | same input twice → bit-identical output | `order_independence`, `board_is_reproducible`, `resolution_is_total_and_deterministic` |
| **O3** Roundtrip | `decode(encode(x)) == x` | `round_trip` (**the keystone**), `vanilla_mandate_round_trips` |
| **O4** Cycle/resource safety | adversarial cyclic input terminates | assembly over a self-referential block set |
| **O7** Registry totality | no silent fallback on a near-miss key | `dangling_ref_is_loud` |

**Genuinely new, with no existing oracle**, and therefore the only gates this
plan adds to the shared vocabulary:

| Gate | Why it is new |
|---|---|
| `no_unbacked_crud_verbs` | Config↔schema conformance. Not a property of a pure function — a repo-wide static+catalogue check |
| `no_missing_sql_functions` | Same class |
| `no_orphan_transition_claims` | Config↔config conformance (verb YAML ↔ DAG YAML) |
| `single_document_shape` | Data-corpus invariant over the live table |
| `no_specificity_ties` | Schema-constraint invariant |
| `one_specificity_scale` | Source-level uniqueness assertion |
| `single_mutation_path` | Source-level uniqueness assertion |

Everything else maps onto O1–O7 and should be *named* in those terms.

### 4.3 Harness shape, if and when it lands

Do **not** propose an alternative. Adopt `EOP-FUZZ-KYCUBO-001 §6` verbatim:
per-crate `fuzz/` sub-crate with an isolated `[workspace]`;
`cargo x fuzz list|run|smoke|regress|seed|clean` ported from bpmn-lite's
`xtask/src/fuzz.rs`; fail-closed nightly-toolchain guard; results under
`fuzz-results/<UTC-stamp>/`; committed minimized regressions under
`fuzz/regressions/`. CI per `§7`: a new `nightly-fuzz.yml` plus
`cargo x fuzz regress` wired into `invariants.yml`.

Two methodology points adopted verbatim:

- **Structured byte-tape generators, not naive `Arbitrary`** (`§8 F-A`) —
  bpmn-lite measured near-0% interesting-branch coverage from the naive
  approach.
- **"A fuzzer that's never seen red proves nothing"** (`§9.1`). Every gate
  here needs a red-first receipt. `active_profile_loads` and
  `order_independence` are **already red** and should be committed red-first.
  For the rest, use this repo's existing `git stash` red-proof discipline.

### 4.4 The substrate requirement is already met — a verified enabling fact

`EOP-FUZZ-KYCUBO-001 §1` identifies purity as what makes
`ob-poc-kyc-substrate` the ideal fuzz target: no sqlx, no async, no DB.

The mandate board's pure core **already has that shape**
(`rust/crates/ob-poc-trading-profile/src/lib.rs`): `ast_builder` (:23),
`types` (:35) and `validate` (:36) are **unconditional**, while `ast_db`
(:25), `document_ops` (:32) and `resolve` (:34) are
`#[cfg(feature = "database")]`. So the grammar and assembly logic already
build at `--no-default-features` without sqlx — no extraction work is needed
before fuzzing becomes possible.

### 4.5 Two gates drop as duplicates

- v0.1's `no_presentation_in_block` and `no_arrangement_in_block` are the same
  assertion twice over different field lists — keep both names, but implement
  as one type-level check with two field sets.
- V&S §6's `identity_is_stable` and PLAN-001 T6's `board_is_reproducible` are
  both O2 at different granularities. Keep the finer one (`identity_is_stable`,
  T1) and state the coarser one (`board_is_reproducible`) as its corollary
  rather than a separate target.

---

## §5 Risks

1. **UI coupling to persisted presentation.** ⟵ *new #1.*
   `status_color`, `sublabel`, `leaf_count` are read directly out of the JSONB
   by `TradingMatrixTree.tsx`. Enforcing Law 2 is a breaking UI change. T1
   owns the migration or explicitly defers it — the failure mode is keeping
   the fields while claiming the law.
2. **Dual document authority is a live break, not a design smell.** ⟵ *new.*
   16 ACTIVE profiles unreadable through the API; 417 unmaterialisable; zero
   materialisation runs ever. T-cutover exists solely to retire this, and it
   is blocked on one ownership question (§G2).
3. **The board is unreachable through the governed surface.** ⟵ *new.*
   18 verb prefixes are pack-unowned; the IM pack is `dry_run_only` with
   `mutation_enabled: false`. T7's `slice_is_reachable_through_the_pack`
   exists to catch "the model works but nobody can use it."
4. **T4 gate semantics now depend on an *amended* law, not an open ruling.**
   Ruling 4 is closed (specificity-resolved), so the risk shifts from
   "undecided" to "the amendment must be accepted." If the V&S is not amended,
   T4's gates contradict the implementation.
5. **KYC grammar reuse is still a premise** (carried from v0.1 T6), and was
   deliberately not investigated in the T0 pass.
6. **Coverage may be intentionally partial** (carried from v0.1 §4 Risk 4),
   still unresolved — and now known to have **no fallback mechanism in the
   schema** to express the partiality with.
7. ~~**Alphabet is inferred.**~~ **Retired** — discharged by
   EOP-FIND-MANDATE-T0-001 §A.

---

## §6 What this plan does not specify

Unchanged from v0.1: gates pin the **contract**, not the mechanism. Internal
data structures, module layout and algorithms are the implementer's choice —
except "adopt a published RPST rather than hand-rolling", and except the fuzz
harness shape, which is fixed by `EOP-FUZZ-KYCUBO-001 §6` and must not be
reinvented.
