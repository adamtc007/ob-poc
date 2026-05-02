# DAG Reachability — SemOS-Canonical Reconciliation — 2026-05-02

> **Companion to:** `docs/governance/dag-reachability-audit-2026-05-02.md` (the original audit) and `docs/todo/P0_dag_reachability_remediation.md` (the remediation plan).
> **Brief:** Architectural review approved the audit's findings and severity calibration but identified that the bottom-up framing leaves three structural concerns under-addressed. The discovery pipeline analysis materially understates the problem. This refinement re-categorises the seven findings into six concern classes and does a **full scan** within each class, not just the cases the original audit happened to surface.
> **Authority:** Adam Cearns, provisional.
> **Status:** Audit refinement only. No code, DSL, or DB changes in this slice. The remediation plan is not updated by this slice; it will be re-sliced once the refinement lands.
> **Decisions context:** D1 stands as drafted. D2 = (a) cascade-via-registry is canonical. D3 reframed via Class D. D4 = (b) audit-only. D5 = (a) add `service-resource.decommission`.

---

## 1. Architectural principle

**SemOS is the canonical state plane for the ob-poc universe.** Every authoritative state mutation — every entity creation, every status transition, every substate flip — must be expressible as a SemOS verb invocation that emits the corresponding `PendingStateAdvance` and writes through a registered `SemOsVerbOp`. State held authoritatively outside SemOS (in a plugin body's direct SQL targeting an entity it doesn't own, in an external pipeline's internal step counter, in a trigger side-effect no verb declares, in a row-count "observability" surface that no verb writes) is a structural defect. This rests on the closure axiom of the utterance pipeline: every reachable state of the system must be reachable AND observable through the verb space, or the entire utterance → DSL → verb chain has a hole. Sage cannot plan through holes. The kernel cannot trace through holes. The AffinityGraph cannot index holes. The original audit's findings are now read as violations of this principle and re-classified by architectural concern, not by flow.

---

## 2. Class A — schema-migration drift (full scan)

The original F-1 was the tip of a much larger drift. Full enumeration of `simple_status_op.rs::STATUS_FLIP_VERBS` against carrier table CHECK constraints in `migrations/master-schema.sql` and `rust/migrations/*.sql` reveals **14 P0 broken verbs beyond F-1**, plus a P0 architectural drift on `deals.deal_status` distinct from F-1.

| # | FQN | File:line | `state_col` written | Carrier table | Drift type | Failure mode |
|---|-----|-----------|---------------------|---------------|------------|--------------|
| 1 | `cbu-ca.submit-for-review` | `simple_status_op.rs:226` | `status` | `cbu_corporate_action_events` | column-name drift | actual column is `ca_status`; UPDATE fails: "column status does not exist" |
| 2 | `cbu-ca.approve` | `simple_status_op.rs:234` | `status` | `cbu_corporate_action_events` | column-name drift | same as #1 |
| 3 | `cbu-ca.reject` | `simple_status_op.rs:243` | `status` | `cbu_corporate_action_events` | column-name drift | same as #1 |
| 4 | `cbu-ca.withdraw` | `simple_status_op.rs:252` | `status` | `cbu_corporate_action_events` | column-name drift | same as #1 |
| 5 | `cbu-ca.mark-implemented` | `simple_status_op.rs:261` | `status` | `cbu_corporate_action_events` | column-name drift | same as #1 |
| 6 | `trading-profile.enter-parallel-run` | `simple_status_op.rs:943` | `status` | `cbu_trading_profiles` | enum drift | target `PARALLEL_RUN` not in `cbu_trading_profiles_status_check` (`DRAFT, VALIDATED, PENDING_REVIEW, ACTIVE, SUPERSEDED, ARCHIVED`); CHECK violation |
| 7 | `trading-profile.suspend` | `simple_status_op.rs:972` | `status` | `cbu_trading_profiles` | enum drift | target `SUSPENDED` not in CHECK |
| 8 | `trading-profile.abort-parallel-run` | `simple_status_op.rs:963` | `status` | `cbu_trading_profiles` | enum drift | target `APPROVED` not in CHECK |
| 9 | `settlement-chain.request-review` | `simple_status_op.rs:869` | `status` | `cbu_settlement_chains` | column-missing | table has no `status` column at all (only `is_active boolean`); UPDATE fails |
| 10 | `settlement-chain.enter-parallel-run` | `simple_status_op.rs:877` | `status` | `cbu_settlement_chains` | column-missing | same as #9 |
| 11 | `settlement-chain.go-live` | `simple_status_op.rs:886` | `status` | `cbu_settlement_chains` | column-missing | same as #9 |
| 12 | `settlement-chain.abort-parallel-run` | `simple_status_op.rs:895` | `status` | `cbu_settlement_chains` | column-missing | same as #9 |
| 13 | `settlement-chain.suspend` | `simple_status_op.rs:904` | `status` | `cbu_settlement_chains` | column-missing | same as #9 |
| 14 | `settlement-chain.reactivate` | `simple_status_op.rs:913` | `status` | `cbu_settlement_chains` | column-missing | same as #9 |

**The deal twist (revising F-1):** the original audit reported `deal.submit-for-bac` and `deal.bac-approve` as writing states (`BAC_APPROVAL`, `KYC_CLEARANCE`) that no longer exist after the D-004 IN_CLEARANCE collapse. **The actual state is more subtle:** `deals_status_check` in `master-schema.sql` *still permits* both old states AND does *not* permit `IN_CLEARANCE`. The DAG declares `IN_CLEARANCE` as the canonical post-D-004 state. So the DB and DAG are out of sync in opposite directions:

- Verbs write `BAC_APPROVAL` / `KYC_CLEARANCE` → DB accepts → DAG state-reducer doesn't recognise → CBU DAG sees no state advance → silent semantic divergence
- Any future verb writing `IN_CLEARANCE` → DB rejects → CHECK violation

Migration `20260429_carrier_08_deals_in_clearance_substates.sql` added the substate columns but did not update `deals_status_check`. This is a P0 schema-DAG-DB triangulation drift, more severe than the original F-1 framing.

**Total Class A drift: 14 P0 broken verbs (CHECK violation or column-missing) + 1 P0 triangulation drift on deals.** The original F-1 captured only 2 of the 14 broken verbs and missed the triangulation entirely.

**Tertiary drift surfaced (P1):** `master-schema.sql` is stale relative to recent migrations — `booking_principal_clearances`, `cbu_service_intent`, `cbu_gateway_connectivity` all live in standalone migrations under `rust/migrations/` and were never folded back. `cargo x schema-export` should be re-run to refresh; until then, any audit using `master-schema.sql` as ground truth will under-report drift.

---

## 3. Class B — closure violations in the precondition graph (full scan, all four DAGs)

Phase 2 extracted **92 concrete `(column, value)` tuples** from `green_when` blocks, transition `precondition` fields, and `derived_state` rules across `deal_dag.yaml`, `cbu_dag.yaml`, `instrument_matrix_dag.yaml`, `lifecycle_resources_dag.yaml`. Intersecting against `STATUS_FLIP_VERBS` plus plugin-op writes:

| Closure status | Count | % |
|----------------|------:|---|
| OK (writer exists in same workspace) | 54 | 58.7 |
| MISSING (no writer found) | 18 | 19.6 |
| IMPLICIT (set by trigger / INSERT default / cascade, no verb) | 12 | 13.0 |
| CROSS_WORKSPACE (writer in another workspace's plane) | 8 | 8.7 |

**Top 10 load-bearing MISSING tuples:**

| # | Transition / state | Required `(column = value)` | Impact | DAG line |
|---|--------------------|----------------------------|--------|----------|
| 1 | deal IN_CLEARANCE → CONTRACTED | `deal_rate_card.status = AGREED` | blocks all commercial closure | `deal_dag.yaml:361,383,394` |
| 2 | deal ONBOARDING → ACTIVE | `deal_onboarding_requests.request_status = COMPLETED` | blocks operational activation | `deal_dag.yaml:444` |
| 3 | cbu_dag investor ELIGIBLE → ACTIVE | `investor_kyc.status = APPROVED` | blocks fund investor onboarding | `cbu_dag.yaml:644` |
| 4 | deal IN_CLEARANCE → CONTRACTED | `deals.kyc_clearance_status = approved` | subgate of deal tollgate | `deal_dag.yaml:394` |
| 5 | billing_profile DRAFT → ACTIVE | `deal_rate_card.status = AGREED` | blocks revenue recognition | `deal_dag.yaml:743` |
| 6 | deal IN_CLEARANCE → REJECTED | `deals.kyc_clearance_status = rejected` | terminal-negative gate missing writer | `deal_dag.yaml:398` |
| 7 | cbu VALIDATED green_when | `mandate.state = active` | cross-workspace closure not owned locally | `cbu_dag.yaml:350` |
| 8 | booking_principal_clearance PENDING → APPROVED | (state machine entry) | gate on deal CONTRACTED | `deal_dag.yaml:357,428–436` |
| 9 | deal_rate_card AGREED uniqueness | only one per `(deal, contract, product)` | enforced via DB trigger, no verb | `deal_dag.yaml:1015` |
| 10 | investor_kyc NOT_STARTED → APPROVED | (investor KYC pipeline) | parallel lifecycle, no writer found | `cbu_dag.yaml:699+` |

**Categorisation of the 18 MISSING:**
- **Verbification debt (7):** schema columns and DAG references exist; no `SemOsVerbOp` registered. Includes the deal substate gap (already captured in F-2 / remediation P0-B) plus 5 more not previously enumerated.
- **Operational out-of-scope (8):** review-workflow completions, counterparty acks, request-completion writes — Layer-3 runtime signals. These require either explicit verbs OR `entry_via: signal` annotations (Class E).
- **Cross-workspace handoff (3):** writers exist in source workspaces; gating is V1.3-1 cross-workspace constraint. Already correctly modelled — these are not defects.

**Closure compliance: 67.4%** when including OK + IMPLICIT + CROSS_WORKSPACE. The 19.6% MISSING is the verbification-debt surface area. Only items in the first category are true closure violations under the SemOS-canonical principle; the second category is solvable via Class E formalisation.

---

## 4. Class C — cascade pattern violations (full plugin-verb survey)

Phase 3 surveyed 84+ plugin verbs across `rust/crates/sem_os_postgres/src/ops/` and `rust/src/domain_ops/`. Off-carrier write classification:

| Category | Count | Verbs |
|----------|------:|-------|
| `cascade_violation` (writes to a SemOS-governed entity outside declared carrier, no registry dispatch) | 7 | cbu.create, cbu.decide, cbu.add-product, cbu.delete-cascade, cbu.assign-ownership, cbu.assign-control, cbu.assign-trust-role, capital.adjust-holding |
| `link_table` (junction/mapping table; flag for review) | 2 | client-group.add-entity, client-group.remove-entity |
| `event_emission` (audit/log/event table; not state-bearing) | several | various — acceptable per SemOS-canonical principle if the event table is not itself a DAG slot |
| `derived_projection` | 0 in surveyed set | — |

**Five worst cascade violators (by count of off-carrier writes or consequence):**

| Verb | File:line | Off-carrier writes | Recommended cascade target |
|------|-----------|--------------------|----------------------------|
| `cbu.delete-cascade` | `cbu.rs:1336–1493` | `client_group_entity` (UPDATE `:1362`), `cbu_group_members` (DELETE `:1375`), `cbu_structure_links` (DELETE `:1387`), `entities` (UPDATE `:1439`), `cbu_entity_roles` (DELETE `:1452`) | Should invoke `client-group.remove-entity`, `cbu.unlink-structure`, `entity.deactivate`, `cbu-role.terminate` via registry |
| `cbu.decide` | `cbu.rs:1198–1326` | `cases` (UPDATE `:1281,1289`), `case_evaluation_snapshots` (INSERT `:1298`) | Should invoke `kyc-case.update-status` via registry; snapshot insert is an event_emission (acceptable) |
| `cbu.add-product` | `cbu.rs:743–945` | `service_delivery_map` (INSERT `:861`), `cbu_resource_instances` (INSERT `:906`) | Should invoke `delivery.start` and a yet-to-exist `service-resource.provision` via registry |
| `cbu.assign-ownership` / `assign-control` / `assign-trust-role` | `cbu_role.rs:42, 140, 223` | `entity_relationships` (INSERT/UPDATE `:94, 181, …`) | Should invoke an entity-graph verb via registry |
| `capital.adjust-holding` | `capital.rs` | `share_classes.issued_shares` (UPDATE) | Should invoke a dedicated share-issuance verb |

**Implications:**

- `cbu.add-product` is doubly implicated: it's both a Class C cascade violator AND the source of the F-3 / F-4 plugin-side-effect entry-state and template-clone problems. Refactoring it to dispatch via registry simultaneously closes Class C, F-3 (`service_intent.active` becomes verb-driven via `service-intent.activate`), and F-4 (template clone becomes `trading-profile.clone-from-template`).
- `cbu.delete-cascade` is the largest single source of off-carrier writes (5 distinct tables). It is also the canonical *correct* test case for Class C: the verb's intent is explicitly cascade, but it does so via direct SQL rather than registry dispatch.
- Event_emission writes (e.g. `case_evaluation_snapshots`, `deal_events`) are not violations under the SemOS-canonical principle because the event table is not itself a state-bearing entity in any DAG. A future refinement may want to formalise this exception via an `audit_emission` verb-spec field so the validator can ignore them automatically.

---

## 5. Class D — discovery pipeline SemOS reconciliation (the deep analysis)

This is the section the original audit understated. F-5 framed the discovery pipeline as an "observability gap." It is not — it is a bidirectional disconnection of an externalised computation from the canonical state plane.

### 5.1 Three architectural tests

**Test 1 — Read closure.** Pipeline verbs read from a mix of SemOS-governed entities (`service_intents`, `attribute_requirements`) and non-governed ones (`srdef_discovery_reasons`, `attribute_rollup_results`, `readiness_results`). The non-governed reads are application-layer derived caches that no DAG taxonomy declares. **Test 1: PARTIAL FAIL.**

**Test 2 — Write closure with `PendingStateAdvance` emission.** Of the 12 write-intent verbs (`service-intent.create`, `service-intent.supersede`, `attributes.set`, `provisioning.run`, `service-resource.sync-definitions`, plus orchestrators), **zero call `emit_pending_state_advance`**. Compare with `cbu.rs`, `entity.rs`, `kyc_case.rs` — all emit after writes. service-pipeline verbs do not. Some writes target SemOS-governed tables (`service_intents` is a DAG slot in `instrument_matrix_dag.yaml`), but the writes do not signal state advance to the DAG reducer. **Test 2: FAIL.**

**Test 3 — Sage navigability.**

- *Q1: "Is discovery in progress / complete / failed for CBU X?"* — **BLOCKED.** No `cbu_discovery_state` slot exists in any DAG. `cbu_overall_lifecycle` has no discovery-progress phase. `instrument_matrix_overall_lifecycle` is trading-enablement scoped, not discovery-scoped. Sage would have to query raw `srdef_discovery_reasons` rows (non-SemOS-governed) and infer progress from absence/presence — loose coupling, not state-machine reasoning.

- *Q2: "Drive an end-to-end discovery cycle for CBU X."* — **UNBRIDGEABLE.** A theoretical sequence exists (`service-intent.create → discovery.run → attributes.rollup → attributes.populate → provisioning.run → readiness.compute`) but: (a) no verb checks if discovery is complete before proceeding; (b) `attributes.rollup`, `populate`, `gaps` are read-only or produce ungovered output tables; (c) `provisioning.run` does not validate discovery completeness; (d) `readiness.compute` is terminal-diagnostic, no back-loop; (e) **3 verbs declared in YAML are not wired in `service_pipeline.rs`** (`service-intent.suspend`, `resume`, `cancel`) — the lifecycle is incomplete at the implementation layer.

### 5.2 Per-verb classification

| Verb FQN | Classification | Reads SemOS? | Writes governed? | Emits advance? | Sage nav |
|----------|---------------|--------------|------------------|----------------|----------|
| `service-intent.create` | Mutator | YES | YES (im.service_intent) | NO | partial |
| `service-intent.list` | Query | YES | N/A | N/A | NO |
| `service-intent.supersede` | Mutator | YES | YES | NO | partial |
| `service-intent.suspend` / `.resume` / `.cancel` | YAML-only (UNWIRED) | — | — | — | — |
| `discovery.run` | Orchestrator | YES | NO (writes to non-governed tables) | NO | YES |
| `discovery.explain` | Query | YES | N/A | N/A | partial |
| `attributes.rollup` | Orchestrator | YES | NO (writes `attribute_rollup_results` not in DAG) | NO | YES |
| `attributes.populate` | Orchestrator | MIXED | NO (`attribute_values` not in DAG) | NO | YES |
| `attributes.gaps` | Query | MIXED | N/A | N/A | YES |
| `attributes.set` | Mutator | YES (registry) | NO (write ungoverned) | NO | partial |
| `provisioning.run` | Orchestrator | MIXED | NO (`cbu_resource_instances` not in DAG slot) | NO | partial |
| `provisioning.status` | Query | NO (external plugin state) | N/A | N/A | NO |
| `readiness.compute` | Orchestrator | MIXED | NO (`readiness_results` derived) | NO | partial |
| `readiness.explain` | Query | MIXED | N/A | N/A | partial |
| `pipeline.full` | Orchestrator | MIXED | NO (cascades writes through sub-verbs) | NO | YES |
| `service-resource.check-attribute-gaps` | Query | MIXED | N/A | N/A | NO |
| `service-resource.sync-definitions` | Mutator | NO (config only) | NO | NO | NO |

### 5.3 The framing answer

The discovery pipeline is in **architectural state (c): externalised system with verb-shaped entry points but no state coupling.**

Justification: the 16 verbs write to 8+ application-layer tables (`service_intents`, `attribute_values`, `srdef_discovery_reasons`, `readiness_results`, ...) of which most are not DAG slots in `cbu_dag.yaml` or `instrument_matrix_dag.yaml`. Discovery progresses invisible to SemOS. Of the 12 write-intent verbs, zero emit `PendingStateAdvance`. Compute happens in externalised Rust code (`ServiceResourcePipelineService`), not in SemOS DAG transitions. Verbs are "shaped" entry points but decoupled from the state machine. SemOS does not know discovery happened.

This is the third broken thing the prompt warns about. It is materially worse than "no observability" — it means Sage cannot reason about discovery state at all, the cross-workspace gating in `cbu_dag.yaml` cannot wait on discovery completion through declarative preconditions, and the AffinityGraph cannot index discovery-derived attribute values into the canonical entity graph.

### 5.4 Required reconciliation (informative, not in remediation slice)

- **State coupling:** add a `cbu_discovery_state` slot to `cbu_dag.yaml` with states (`PENDING → DISCOVERING → ROLLUP → POPULATE → PROVISION → READY` plus `FAILED` / `BLOCKED`).
- **Write closure:** `discovery.run`, `attributes.populate`, `provisioning.run`, `readiness.compute` each emit `PendingStateAdvance` against the new slot.
- **Read closure:** `attributes.rollup` and `attributes.populate` writes target `attribute_values` which becomes a governed slot (it already is in `instrument_matrix_dag` for some attributes — formalise across the board).
- **Wire the unwired:** `service-intent.suspend`, `resume`, `cancel` need `SemOsVerbOp` impls in `service_pipeline.rs`.
- **Sage navigability check:** add an integration test that asserts a Sage agent can answer Q1 and execute Q2 using only registered verbs against a freshly-created CBU.

---

## 6. Class E — `entry_via` taxonomy formalisation

Phase 5 walked all 158 states across the four DAGs. **155 (98.1%) are reachable; 3 are mismatches.** Most "non-verb" entries are *legitimately* non-verb under the formalised taxonomy.

### 6.1 Distribution

| `entry_via` | Count | Notes |
|-------------|------:|-------|
| `verb` | 127 | Direct verb writer registered |
| `trigger` | 8 | DB trigger / time-decay / parent-state mirror; all named in DAG `(backend: ...)` annotations |
| `scheduler` | 2 | `cbu.archived` (archival scheduler), `corporate_action_event.default_applied` (cutoff-time auto-apply) |
| `signal` | 9 | health-check, settlement-pipeline first-trade, research confirmation, SLA threshold |
| `cascade` | (implicit, ~5) | parent-state cascades — formalisation needed (see §6.3) |
| NONE / edge | 12 | All within prune-cascade or deferred-Tranche-3 territory |
| **Mismatch** | **3** | listed below |

### 6.2 Mismatches

| DAG | Slot | State | Issue | Recommended action |
|-----|------|-------|-------|--------------------|
| deal | deal (operational) | `OFFBOARDED` | DAG transitions list `deal.update-status` but no offboard verb is registered in `simple_status_op.rs` and no plugin op writes `OFFBOARDED` | Register `deal.complete-offboard` as `SimpleStatusOp` OR annotate `entry_via: scheduler` if archival |
| deal | deal_rate_card | `CANCELLED` | Implicit cascade ("rate card removed or deal cancelled") — no verb writes the value | Annotate `entry_via: cascade` and reference the parent verb (`deal.cancel`) |
| im | (one trading-profile state) | (depends on cbu_trading_profiles_status_check enum reconciliation per Class A) | Cascade from prune | Annotate `entry_via: cascade` once Class A is fixed |

### 6.3 Validator obligation per `entry_via` value

| `entry_via` | Validator must verify |
|-------------|----------------------|
| `verb` | At least one registered writer for the `(column, value)` tuple in `STATUS_FLIP_VERBS` or a plugin op |
| `cascade` | Parent verb declares `transition_args.cascades` referencing a registered child verb |
| `trigger` | DAG state spec names the trigger; trigger exists in the migration set |
| `scheduler` | DAG state spec names the scheduler; scheduler is referenced from a known background-task entry point |
| `signal` | DAG state spec names the signal source (health-check name, BPMN event, external API) |

Adding these annotations is a separate slice (deferred). With them, the validator can distinguish "legitimate non-verb state" from "actually broken" mechanically. Today the audit needs human judgement to make the distinction, which doesn't scale.

### 6.4 Backend-only inventory (legitimate non-verb states)

19 states reach via trigger / scheduler / signal. All are documented in their respective DAG `(backend: ...)` annotations. None require verb addition under the SemOS-canonical principle — they are by-design externalisations. The validator obligation in §6.3 turns these from "documentary footnotes" into mechanically enforced contracts.

---

## 7. Class F — validator extension spec (4 checks)

Spec only; implementation is a separate slice. Each check fits as a new pass in `rust/xtask/src/reconcile.rs` (the existing `validate` subcommand). Failure exits non-zero and prints structured errors compatible with the existing pre-commit gate.

### Check 1 — SimpleStatusOp drift

**Input:**
- `rust/src/domain_ops/simple_status_op.rs::STATUS_FLIP_VERBS` (parsed via syn or via runtime registry introspection)
- DB CHECK constraints from `migrations/master-schema.sql` and all `rust/migrations/*.sql` (parsed via SQL DDL parser, OR via live DB introspection if `--db` flag set)

**Logic:** for every `SimpleStatusConfig`, verify (a) the table exists, (b) the `state_col` exists on that table, (c) the `target_state` value is in the column's CHECK constraint enumeration.

**Output / error message format:**
```
SimpleStatusOp drift in <fqn> at simple_status_op.rs:<line>
  table:        "<table>"
  state_col:    "<col>"  (actual: "<actual_col>" or NOT FOUND)
  target_state: "<value>"
  CHECK allows: [<v1>, <v2>, ...]
  drift type:   <column-name | column-missing | enum>
  fix:          <suggested>
```

**Sample failing case (current codebase):**
```
SimpleStatusOp drift in cbu-ca.approve at simple_status_op.rs:234
  table:        "cbu_corporate_action_events"
  state_col:    "status"  (actual: "ca_status")
  target_state: "approved"
  CHECK allows: [proposed, under_review, approved, effective, implemented, rejected, withdrawn]
  drift type:   column-name
  fix:          rename SimpleStatusConfig.state_col from "status" to "ca_status"
```

### Check 2 — Precondition closure

**Input:**
- DAG YAMLs in `rust/config/sem_os_seeds/dag_taxonomies/*.yaml`, parsing every `green_when:`, transition `precondition:`, `derived_state:` rule
- Predicate AST from `rust/crates/dsl-core/src/config/predicate/parser.rs` (already parses these)
- Writer set from STATUS_FLIP_VERBS + plugin op SQL writes (the latter requires a per-op annotation `writes:` declared in YAML, since static analysis of arbitrary `sqlx::query` is fragile)

**Logic:** for every `(column, required_value)` tuple referenced in any precondition, verify at least one writer exists. If no writer exists, check whether the column has an `entry_via: trigger | scheduler | signal | cascade` annotation. Fail if neither.

**Output:**
```
Closure violation: <DAG>::<slot>::<state>
  preconditioned on:    <column> = <value>
  declared at:          <dag_file>:<line>
  writers found:        []
  entry_via:            <NONE | mismatch>
  fix:                  add a writer verb OR annotate entry_via in the column's slot spec
```

**Sample failing case:**
```
Closure violation: deal::deal::CONTRACTED
  preconditioned on:    deals.bac_status = approved
  declared at:          deal_dag.yaml:394
  writers found:        []
  entry_via:            NONE (no annotation; verb expected but missing)
  fix:                  register deal.bac-approve as a writer of bac_status='approved'
```

### Check 3 — Cascade pattern

**Input:**
- Plugin op source files under `rust/crates/sem_os_postgres/src/ops/*.rs` and `rust/src/domain_ops/*.rs`
- Static SQL extraction from `sqlx::query`, `sqlx::query_as`, `sqlx::query_scalar` invocations (use `tree-sitter-rust` AST or a regex with sane escapes; the existing `xtask::verbs` lint already does similar parsing)
- Verb YAML declarations of `crud.table` (carrier) and `transition_args.cascades` (declared cascade targets)

**Logic:** for every plugin verb, extract the set of tables it writes (`UPDATE`/`INSERT`/`DELETE`). Allowed targets: (a) the declared carrier, (b) tables declared in the verb's `transition_args.cascades`, (c) tables annotated as `audit_emission` in their own slot spec. Any other write is a violation.

**Output:**
```
Cascade violation in <fqn> at <file>:<line>
  declared carrier:     <table>
  declared cascades:    [<t1>, <t2>, ...]
  off-carrier writes:   [<table_w_action>, ...]
  fix:                  invoke <child_verb> via SemOsVerbOpRegistry, OR declare cascade in YAML, OR annotate target as audit_emission
```

**Sample failing case:**
```
Cascade violation in cbu.delete-cascade at cbu.rs:1336
  declared carrier:     cbus
  declared cascades:    []
  off-carrier writes:   [client_group_entity:UPDATE@1362, cbu_group_members:DELETE@1375, cbu_structure_links:DELETE@1387, entities:UPDATE@1439, cbu_entity_roles:DELETE@1452]
  fix:                  invoke client-group.remove-entity, cbu.unlink-structure, entity.deactivate, cbu-role.terminate via registry; OR declare cascades in deal.yaml
```

### Check 4 — `entry_via` consistency

**Input:**
- DAG YAMLs with `entry_via` annotations (requires DAG schema extension — separate slice to add the field)
- Registry set (verb + cascade + trigger + scheduler + signal sources)

**Logic:** for every state, verify `entry_via` matches the actual mechanism. `verb` requires a registered writer. `cascade` requires the parent verb to declare cascades. `trigger`/`scheduler`/`signal` require the named mechanism to exist in its respective registry.

**Output:**
```
entry_via mismatch: <DAG>::<slot>::<state>
  declared:            entry_via: <X>
  observed:            <actual mechanism or NONE>
  fix:                 <align declaration or implement mechanism>
```

**Sample failing case (post DAG schema extension):**
```
entry_via mismatch: deal::deal_operational_lifecycle::OFFBOARDED
  declared:            entry_via: verb (transitions list deal.update-status)
  observed:            NONE (no registered writer for deal_status='OFFBOARDED')
  fix:                 register a deal.complete-offboard SimpleStatusOp, OR change entry_via to 'cascade'/'scheduler' if appropriate
```

### Wiring

All four checks are pure functions over (parsed YAML + parsed SQL + parsed Rust source). They don't need a live DB at validation time, so they fit the existing `cargo run -p xtask -- reconcile validate` invocation. Each check produces a structured error list; the binary exits non-zero if any list is non-empty. The pre-commit hook gains zero new dependencies.

---

## 8. Updated severity matrix — replace flow-based headline

The original audit's "Headline" table grouped findings by flow. Re-grouped by concern class, severity becomes:

| Concern class | P0 instances | P1 instances | P2 instances | Validator check |
|---------------|-------------:|-------------:|-------------:|-----------------|
| A — schema-migration drift | 14 (broken verbs) + 1 (deal triangulation) | 1 (stale master-schema.sql) | 0 | Check 1 |
| B — closure violations | 7 (verbification debt) | 8 (operational signals needing entry_via) | 0 | Check 2 |
| C — cascade pattern | 7 (cascade violators) | 2 (link-table conditionals) | 0 | Check 3 |
| D — discovery pipeline disconnection | 1 (whole pipeline in state c) | 3 (unwired YAML verbs) | 0 | (composite) |
| E — `entry_via` formalisation | 0 (3 mismatches but P1) | 3 (mismatches) | 19 (annotation backfill) | Check 4 |
| F — validator coverage | (the gap that lets all of the above ship) | — | — | — |

Original audit reported "Deal is the only flow with a production-blocking issue." Reconciled view: **Class A alone has 15 P0 instances across cbu-ca, trading-profile, settlement-chain, and deal — a 7.5× under-count.** The framing change matters because the original audit's flow lens hid drift in the same concern class manifesting in different flows.

The deepest finding remains Class D. The discovery pipeline being in architectural state (c) is not a "P1 observability gap" — it is the only place in the audit where the SemOS-canonical principle is violated *systemically* rather than per-verb. Fixing Class A is mechanical (16 verbs to repair); fixing Class D requires a state-machine architectural decision (the §5.4 reconciliation sketch).

---

## 9. Evidence pointers

- DAG taxonomies: `rust/config/sem_os_seeds/dag_taxonomies/{deal,cbu,instrument_matrix,lifecycle_resources}_dag.yaml`
- SimpleStatusOp registry: `rust/src/domain_ops/simple_status_op.rs`
- Plugin op source: `rust/crates/sem_os_postgres/src/ops/*.rs`, `rust/src/domain_ops/*.rs`
- Schema constraints: `migrations/master-schema.sql` (stale — re-export needed), `rust/migrations/20260429_*.sql` for booking_principal_clearances and substate columns
- Verb YAMLs: `rust/config/verbs/{deal,cbu,trading-profile*,service*,discovery,provisioning*,readiness*,pipeline*,attributes,attribute}.yaml`
- Validator entry point: `rust/xtask/src/reconcile.rs` and `rust/xtask/src/main.rs`
- Predicate parser (already in tree): `rust/crates/dsl-core/src/config/predicate/parser.rs`
- Companion documents: `docs/governance/dag-reachability-audit-2026-05-02.md`, `docs/todo/P0_dag_reachability_remediation.md`

---

## 10. What this refinement does and does not do

**Does:**
- Re-classify the 7 original findings into 6 concern classes
- Surface 14 P0 broken verbs Class A missed, plus a deal triangulation drift
- Quantify Class B beyond F-2 (full 92-tuple scan; 18 MISSING)
- Inventory Class C cascade violators (7 confirmed, 5 worst named)
- Diagnose Class D's framing answer (architectural state c — the third broken thing)
- Walk all 158 states for Class E `entry_via` tagging (155/158 reachable)
- Spec 4 mechanical validator checks that make the closure axiom CI-enforceable

**Does not:**
- Update the remediation plan (`docs/todo/P0_dag_reachability_remediation.md`). That re-slicing is a follow-on once this refinement lands and Adam signs off.
- Implement any of the changes. Validator extension, deal substate verbs, cascade-pattern refactors, discovery state slot — all separate slices.
- Re-litigate D1, D2, D4, D5. D3 reframed via Class D, not re-decided.
- Audit macros, scenarios, or composite expansions (out of scope per Adam directive).
