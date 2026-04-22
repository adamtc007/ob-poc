# Pattern A / Pattern B — A1 Remediation Ledger

> **Status:** OPEN. Must close before the three-plane refactor is Done.
> **Created:** 2026-04-18
> **Decision reference:** D11 resolution = Option III (Hybrid). Pattern A gates Phase 0; Pattern B gates Phase 6 (not Phase 5 start).
> **Do not delete this file until every row in §2 and §3 reads `CLOSED`.**

---

## 0. Purpose

Under **Decision D8**, a SemOS state advance requiring external effects inside the inner transaction blocks Phase 5. The Phase 0a ownership matrix surfaced **4 pre-existing A1 violations** in the current codebase:

1. `sem_os_maintenance_ops.rs` — subprocess spawn
2. `bpmn_lite_ops.rs` — external gRPC
3. `source_loader_ops.rs` — external HTTP
4. `gleif_ops.rs` — external HTTP

Per **Decision D11 (2026-04-18, Option III)**, these are grandfathered into the refactor as **Phase 0g** (Pattern A, one op) and **Phase 5f** (Pattern B, ~38 ops). They are NOT cleared. They are scheduled.

**This ledger exists so the compromise cannot be quietly abandoned.** Each op is named with its remediation plan. Each file moves to CLOSED only when its A1 violation is demonstrably removed — verified by CI lint.

---

## 1. Guardrails — how this compromise is enforced

**G1. This file is an artefact of the refactor.** It is referenced from §13 of `three-plane-architecture-implementation-plan-v0.1.md` and from §9 of `phase-0a-ownership-matrix.md`. Closing those artefacts without closing this ledger is a process error.

**G2. Phase 6 depends on Phase 5f.** The implementation plan makes CRUD dissolution conditional on every row in §3 of this ledger reading CLOSED. Phase 6 is the phase where the biggest deletions (and therefore the biggest perceived wins) happen — anchoring it to 5f makes 5f impossible to skip politically.

**G3. CI lint L4 enforces closure.** After Phase 5f, workspace lint **L4** (specified in the implementation plan's Appendix B) asserts **zero occurrences** of:
- `reqwest::`, `.get(`, `.post(`, `.send(`, `http::`, `hyper::`
- `tokio::process::`, `Command::new`
- `tonic::` (or any gRPC client directly)

…inside `execute_json` or `execute` bodies in any file containing `impl CustomOperation`. An `#[allow(external_effects_in_verb)]` attribute escape hatch requires a TODO comment referencing this ledger.

**G4. Definition of Done contains explicit clause:** the implementation plan's §13 DoD item 19 reads: *"Pattern-A remediation ledger (Phase 0g) and Pattern-B remediation ledger (Phase 5f) are both closed; lint L4 is green."*

**G5. Pre-commit hook (optional, recommended).** A pre-commit hook in `rust/scripts/` checks L4 and fails the commit on regression. Even before Phase 5f is complete, this hook fires on any NEW violation introduced outside the 4 grandfathered files.

---

## 2. Phase 0g — Pattern A (subprocess)

**Blocks:** Phase 1 (treated as a Phase 0 deliverable sibling, does not block Phase 0b–0e in parallel).

### 2.1 File: `sem_os_maintenance_ops.rs`

| op | lines | external effect | resolution path | status |
|---|---|---|---|---|
| `MaintenanceReindexEmbeddingsOp` | 391–560 (Phase 0g rewrite) | ~~`tokio::process::Command::new("cargo")` spawns `cargo run --release -- populate_embeddings` subprocess~~ — REMOVED | **Outbox deferral complete.** `execute` and `execute_json` now insert a `maintenance_spawn` row into `public.outbox` (migration 131) and return `{"status": "queued", ...}`. No subprocess in verb body. Phase 5e drainer will consume the row and spawn the subprocess post-commit. | **CLOSED 2026-04-18** |

### 2.2 File: `sem_os_postgres/src/ops/agent.rs` (added + CLOSED 2026-04-22)

| op | lines | external effect | resolution path | status |
|---|---|---|---|---|
| `ActivateTeaching` | ~757-870 | ~~`tokio::process::Command::new("cargo")` spawns `populate_embeddings` subprocess directly inside verb body~~ — REMOVED | **Outbox deferral complete (2026-04-22).** Verb now inserts a `maintenance_spawn` row into `public.outbox` (reusing the Phase 0g-built `OutboxEffectKind::MaintenanceSpawn` + `MaintenanceSpawnConsumer` infrastructure wholesale). Idempotency key `activate_teaching:<trace_id>:<YYYY-MM-DD>` collapses concurrent same-day activations to one subprocess run. Admin synchronous path preserved via direct `cargo run -p ob-semantic-matcher --bin populate_embeddings`. Behaviour change noted: response now `{"status": "queued", ...}` instead of `{"success": true, "embedded_count": N}` — callers that need post-embedding counts must poll the outbox or the embeddings table directly. | **CLOSED 2026-04-22** |

**Completion criteria for 2.2 (all met):**

- [x] Lint L4 caught the violation — `scripts/lint_external_effects_in_verbs.sh` grandfathered the file on 2026-04-22.
- [x] `ActivateTeaching::execute` rewritten to `INSERT INTO public.outbox ... effect_kind='maintenance_spawn'` with idempotency key `activate_teaching:<trace_id>:<date>`.
- [x] Lint L4 passes against `sem_os_postgres/src/ops/agent.rs` — removed from `GRANDFATHERED` in the lint script.
- [x] Lint now reports `Grandfathered hits: 0 (scheduled for Phase F.1-F.3)`.
- [x] Row above moves to **CLOSED**.

**Completion criteria for 0g (all met):**

- [x] `OutboxEffectKind::MaintenanceSpawn` variant added to `ob-poc-types::gated_envelope::OutboxEffectKind` (Phase 0b).
- [x] `public.outbox` table via migration `131_public_outbox.sql` (Phase 0d, per D10 — separate from `sem_reg.outbox_events`).
- [x] `MaintenanceReindexEmbeddingsOp::execute_json` rewritten to `INSERT INTO public.outbox ... effect_kind='maintenance_spawn'` with idempotency key `maintenance_spawn:<trace_id>:<date>[-force]`. Same treatment on `execute` for legacy callers.
- [ ] Drainer handler for `MaintenanceSpawn` (spawns subprocess with idempotency guard) — **DEFERRED TO PHASE 5e** per framework plan. Until Phase 5e, queued rows accumulate in `public.outbox` with status=`pending`. Admins retain a direct synchronous path: `cargo run --release --package ob-semantic-matcher --bin populate_embeddings`.
- [x] Lint L4 passes against `sem_os_maintenance_ops.rs` — verified 2026-04-18: `grep -E 'tokio::process::|Command::new|reqwest|http::|hyper::' returns no matches`.
- [x] Row above moves to **CLOSED**.

**Alternative considered:** path-exclusion (mark verb as admin-only, reject in Sequencer stage 6 gating for non-admin sessions). Rejected — outbox deferral gives a uniform pattern for all long-running admin tasks and re-uses infrastructure being built anyway.

**Behaviour change noted:** invocations of `maintenance.reindex-embeddings` now return `status=queued` instead of `status=success`. Until Phase 5e drainer is live, the actual subprocess must be run via the direct path above. This is a documented transitional regression; when Phase 5e lands, the queued→processed cycle becomes fully automated.

---

## 3. Phase 5f — Pattern B (external service integration)

**Blocks:** Phase 6 (CRUD dissolution).
**Starts:** after Phase 5e (outbox + drainer + WebSocket fully wired).
**Rationale for sequencing:** Pattern B resolution re-uses the outbox infrastructure built in 5e. Attempting 5f before 5e is redundant plumbing.

### 3.1 File: `bpmn_lite_ops.rs` — 5 ops

| op | external call | shape | resolution | status |
|---|---|---|---|---|
| `BpmnCompileOp` | ~~`client.compile(xml) → bytecode`~~ — MOVED to `pre_fetch` hook | idempotent (bytecode = hash of XML) | **Pre-fetch migration complete (2026-04-22, F.1b).** Compile is deterministic: re-compiling the same XML returns the same bytecode. Side-effect on bpmn-server side is safely idempotent, so pre_fetch + retry-on-rollback is sound (unlike `start` which creates fresh instance IDs). Returns bytecode via `_bpmn_compile_result` args key; execute reads back and formats. | **CLOSED 2026-04-22** |
| `BpmnStartOp` | ~~`client.start(bytecode, vars) → instance_id`~~ — MOVED to `pre_fetch` hook | write-through (non-idempotent) | **Pre-fetch migration complete (2026-04-22, F.1d).** Saga risk acknowledged: bpmn-server generates a fresh instance_id per call, so a rollback in the outer txn leaves an ORPHANED bpmn instance with no DB trail on our side. The pre-F.1d implementation had the same orphan window (gRPC fired inside the verb body; the verb's own per-verb txn had no effect on the bpmn service). Net effect: A1 invariant satisfied; saga risk UNCHANGED. Future slice: add a periodic reaper that finds bpmn instances with no corresponding DB correlation row and cancels them. Tracked as §3.1 follow-up. | **CLOSED 2026-04-22** (A1); saga reaper **follow-up** |
| `BpmnSignalOp` | ~~`client.signal(instance_id, payload)`~~ — REMOVED from verb body | fire-and-forget | **Outbox deferral complete (2026-04-22).** Verb writes a `bpmn_signal` row; drainer consumer `BpmnSignalConsumer` (registered in `ob-poc-web::main` alongside `MaintenanceSpawnConsumer`) performs the actual `client.signal(...)` post-commit. Retryable on gRPC failure with bounded attempts via drainer. | **CLOSED 2026-04-22** |
| `BpmnCancelOp` | ~~`client.cancel(instance_id, reason)`~~ — REMOVED from verb body | fire-and-forget | **Outbox deferral complete (2026-04-22).** Drainer consumer `BpmnCancelConsumer` registered; same retry semantics as `BpmnSignalConsumer`. | **CLOSED 2026-04-22** |
| `BpmnInspectOp` | ~~`client.inspect(instance_id) → inspection`~~ — MOVED to `pre_fetch` hook | read-only | **Pre-fetch hook complete (2026-04-22, Phase F.1).** `SemOsVerbOp` trait extended with optional `pre_fetch(&args, &mut ctx) -> Option<Value>`. Dispatcher calls it BEFORE opening the scope, merges the returned JSON into args, then calls `execute` normally. `BpmnInspect::pre_fetch` does the gRPC call and returns `{"_inspection": ...}`; `BpmnInspect::execute` reads `_inspection` from args and formats the typed result — zero I/O inside the inner txn. A1 invariant satisfied. | **CLOSED 2026-04-22** |

**Completion criteria for bpmn_lite_ops.rs:**

- [ ] `OutboxEffectKind::BpmnDispatch { op_kind, ... }` variant added.
- [ ] 4 write-path ops rewritten to return `OutboxDraft`; callers refactored to handle async completion (`bpmn_lite_grpc_callback` route, existing BPMN integration).
- [ ] 1 read-only op (`BpmnInspectOp`) moved to pre-txn fetch (stage 5 of Sequencer) — or, if that's infeasible, also deferred to outbox.
- [ ] gRPC calls inside execute bodies: **zero** (verified by lint L4).
- [ ] Integration test: BPMN compile → instance_id returned via callback; signal/cancel round-trip; inspect returns fresh data.
- [ ] Row above moves to **CLOSED**.

### 3.2 File: `source_loader_ops.rs` — 15 ops (3 pure-config + 12 HTTP)

Originally listed as 16 ops; the actual count is 15 — 3 pure-config ops
(`SourcesList`, `SourcesInfo`, `SourcesFindForJurisdiction`) have no I/O
and need no remediation.

All 12 I/O ops remediated via pre_fetch (2026-04-22, Phase F.2):

| op | HTTP call | pre_fetch result key | status |
|---|---|---|---|
| `SourcesSearch` | `source.search(...)` | `_sources_search_results` | **CLOSED** |
| `SourcesFetch` | `source.fetch_entity(...)` | `_sources_fetched_entity` | **CLOSED** |
| `CompaniesHouseSearch` | `loader.search(...)` | `_search_results` | **CLOSED** |
| `CompaniesHouseFetchCompany` | `loader.fetch_entity(...)` | `_fetched_entity` | **CLOSED** |
| `CompaniesHouseFetchPsc` | `loader.fetch_control_holders(...)` | `_psc_holders` | **CLOSED** |
| `CompaniesHouseFetchOfficers` | `loader.fetch_officers(...)` | `_officers` | **CLOSED** |
| `CompaniesHouseImportCompany` | entity + PSC + officers | `_ch_import_entity/_psc_count/_officer_count` | **CLOSED** |
| `SecEdgarSearch` | `loader.search(...)` | `_sec_search_results` | **CLOSED** |
| `SecEdgarFetchCompany` | `loader.fetch_entity(...)` | `_sec_fetched_entity` | **CLOSED** |
| `SecEdgarFetchBeneficialOwners` | `loader.fetch_control_holders(...)` | `_sec_beneficial_owners` | **CLOSED** |
| `SecEdgarFetchFilings` | `loader.fetch_entity(... raw)` | `_sec_filings` | **CLOSED** |
| `SecEdgarImportCompany` | entity + BO | `_sec_import_entity/_bo_count` | **CLOSED** |

**Pattern used:** the `SemOsVerbOp::pre_fetch` hook added in Phase F.1.
`pre_fetch` performs the HTTP call and stores the formatted result under
a file-unique key in the returned JSON object; the dispatcher merges it
into `args` before opening the transaction scope. `execute` reads the
pre-fetched data from args (zero I/O), does any DB work (create entity,
log research action), and returns.

For Import ops the pattern is richer: HTTP fetch + optional dependent
HTTP fetches (PSC, officers, beneficial owners) all run in pre_fetch
under a single outer HTTP phase. DB writes (`create_entity_from_normalized`
and `log_research_action`) stay in execute, sharing the inner txn scope.

**Status:** 12/12 I/O ops remediated (3 pure-config ops trivially CLOSED).
Row moves to **CLOSED 2026-04-22**.

### 3.3 File: `gleif_ops.rs` — 17 ops (10 pure-lookup + 1 dispatcher + 6 DB-write-interleaved)

**Phase F.3 status (2026-04-22): partial — 11/17 closed (§3.3a); 6 remain OPEN (§3.3b).**

#### §3.3a — 10 pure-HTTP + HTTP-with-DB-lookup ops + 1 dispatcher (CLOSED 2026-04-22)

| op | pre_fetch result key | uses pool for DB lookup? | status |
|---|---|---|---|
| `GleifSearch` | `_gleif_search_candidates` | — | **CLOSED** |
| `GleifGetRecord` | `_gleif_record` | — | **CLOSED** |
| `GleifGetParent` | `_gleif_parent` | — | **CLOSED** |
| `GleifGetChildren` | `_gleif_children` | — | **CLOSED** |
| `GleifGetUmbrella` | `_gleif_umbrella` | yes (entity-id → LEI) | **CLOSED** |
| `GleifGetManager` | `_gleif_manager` | yes (entity-id → LEI) | **CLOSED** |
| `GleifGetMasterFund` | `_gleif_master_fund` | yes (entity-id → LEI) | **CLOSED** |
| `GleifGetManagedFunds` | `_gleif_managed_funds` | — | **CLOSED** |
| `GleifTraceOwnership` | `_gleif_trace_ownership` | yes (entity-id → LEI) | **CLOSED** |
| `GleifLookupByIsin` | `_gleif_isin_lookup` | — | **CLOSED** |
| `GleifLookup` (dispatcher) | delegates to sub-op | delegates | **CLOSED** |

**Trait extension in F.3:** `SemOsVerbOp::pre_fetch` signature gained a
`&sqlx::PgPool` parameter (Phase F.1/F.2 previously had just args+ctx).
Ops that look up `entity_id → LEI` before calling GLEIF can now do that
DB read in pre_fetch outside the txn. Pool access is for READ-ONLY
auto-commit queries only; writes still happen in `execute` under the
caller's scope.

**Dispatcher delegation:** `GleifLookup::pre_fetch` matches on
`target-type` and delegates to the selected sub-op's `pre_fetch`;
sub-op result JSON flows through the outer dispatcher into its
`execute`, which delegates to the same sub-op's `execute`. Both legs
of the delegation preserve the pre_fetch contract.

#### §3.3b — 6 DB-write-interleaved ops (PARTIAL; 1 CLOSED 2026-04-22)

These interleave HTTP with DB WRITES, not just reads. The path
forward (started 2026-04-22 late-session) is to split the service
into fetch-only + persist-only method pairs, letting each op route
HTTP through pre_fetch and DB writes through execute.

| op | status | notes |
|---|---|---|
| `GleifEnrich` | **CLOSED 2026-04-22** | `GleifEnrichmentService::fetch_all_for_enrich` (HTTP-only, bundles 9 GLEIF calls into `EnrichmentFetch`) + `persist_enrichment` (DB-only). GleifEnrich op wires pre_fetch → execute. Legacy `enrich_entity` kept as wrapper for non-op callers. |
| `GleifImportTree` | **CLOSED 2026-04-22** | New `fetch_corporate_tree_only` + `persist_corporate_tree` methods on GleifEnrichmentService. The HTTP is a single `client.fetch_corporate_tree(root, depth)` BFS that returns a `CorporateTreeResult` (records + discovered relationships); all DB writes happen in persist. Added `Serialize+Deserialize` derives on `CorporateTreeResult` and `DiscoveredRelationship` for the args round-trip. |
| `GleifRefresh` | **CLOSED 2026-04-22** | Reuses `fetch_all_for_enrich` + `persist_enrichment` from GleifEnrich split. pre_fetch resolves refresh targets (single entity or bulk-stale-discovery DB query), then fetches an `EnrichmentFetch` bundle per target. Execute loops through bundles and calls `persist_enrichment`; per-target HTTP errors counted separately from persist errors (both surface in the `errors` response field). |
| `GleifImportManagedFunds` | **OPEN (needs redesign)** | Uses `get_or_create_entity_by_lei` helper which does conditional HTTP based on DB state (fetches LEI record only if entity doesn't already exist). Moving this to pre_fetch requires either (a) pre-fetching all POSSIBLE umbrella/ManCo records unconditionally then using whichever the DB didn't already have, or (b) two-pass with a pre-resolution DB check. Clean fit is unclear; likely needs a focused design slice. |
| `GleifResolveSuccessor` | **CLOSED 2026-04-22** | Misclassified in original ledger — on inspection is pure HTTP (chase successor chain via `get_lei_record` loop, no DB writes). Straight pre_fetch migration; result under `_gleif_successor`. |
| `GleifImportToClientGroup` | **OPEN (needs redesign)** | Same helper-conditional-HTTP pattern as `GleifImportManagedFunds`. Deferred to the same design slice. |

**EnrichmentFetch struct** (rust/src/gleif/enrichment.rs): serde-
round-trippable bundle of `LeiRecord`, `BicMapping`s, direct-parent
`(lei, name)`, ultimate-parent `(lei, name)`, fund-manager, umbrella,
master — all optional where the corresponding GLEIF relationship
may be absent. Pre_fetch produces it, execute deserializes from
args and hands it to persist_enrichment.

**Remaining work path:** apply the same split pattern to the other 5
service methods (`import_corporate_tree`, `refresh_entity`,
`import_managed_funds`, etc.). Each method follows the same
mechanical refactor — ~200-400 lines per method.

**Status:** 15/17 ops CLOSED (§3.3a 11 + §3.3b 4); 2/17 OPEN (§3.3b remaining 2). **PARTIAL.**

### 3.4 File: `request_ops.rs` — helper-indirect gRPC (CLOSED 2026-04-22, F.1c)

This file was added to the original grandfathered list at the lint
level (taint propagation) because ~8 request lifecycle verbs
(`request.cancel`, `request.remind`, `request.escalate`,
`request.extend`, etc.) called a shared helper `try_send_bpmn_signal`
that did a direct gRPC `client.signal(...)` call.

| shape | resolution | status |
|---|---|---|
| Fire-and-forget gRPC signal via helper | **Outbox deferral.** `try_send_bpmn_signal` helper rewritten to (a) look up the active BPMN correlation for the case, (b) insert a `bpmn_signal` row into `public.outbox` (same effect_kind + idempotency key shape as `BpmnSignal::execute`), (c) append the audit entry to `communication_log`. All steps share the caller's ambient txn; the gRPC call is performed post-commit by the existing `BpmnSignalConsumer` drainer. Lazy `OnceLock<BpmnLiteConnection>` static and the `bpmn_client()` helper deleted entirely. | **CLOSED 2026-04-22** |

Side effect: the audit trail type changed from `BPMN_SIGNAL` to
`BPMN_SIGNAL_QUEUED` and now records the outbox_id + idempotency_key
alongside the correlation metadata. Downstream readers of
`communication_log` that filtered on `type = 'BPMN_SIGNAL'` need to
update their filter to include `BPMN_SIGNAL_QUEUED`; confirmed no
such reader exists in-repo.

Lint script `rust/scripts/lint_external_effects_in_verbs.sh` —
`src/domain_ops/request_ops.rs` removed from the `GRANDFATHERED`
array. L4 Layer 1 still passes with 0 grandfathered hits.

### 3.5 Phase 5f completion criteria

Phase 5f is CLOSED when:

- [ ] All three files in §3.1–§3.3 have their rows at **CLOSED**.
- [ ] Lint L4 green across the entire `rust/src/domain_ops/` (no `#[allow(external_effects_in_verb)]` escape-hatch usage without accompanying TODO+ledger reference).
- [ ] This ledger §3 reads **CLOSED** at the section level.
- [ ] DoD item 19 in `three-plane-architecture-implementation-plan-v0.1.md` can be checked.

Only then may Phase 6 begin.

---

## 4. Escalation rules

- **If Phase 5e slips by more than 2 weeks:** re-evaluate whether 5f must still gate 6, or whether intermediate Phase 6 work on non-Pattern-B ops can proceed under a narrower lint scope (L4 applied only to the 4 grandfathered files).
- **If any op in §3 turns out to require architectural change beyond "outbox + fetch-split":** escalate to a dedicated design document for that op. The ledger row stays OPEN; the design doc is linked from the row.
- **If D11 is revisited (moved to Option I or Option II):** update this ledger's status header and cross-reference the new decision. Do not silently re-classify rows.

---

## 5. Review schedule

- **Weekly** during Phases 1–5: 10-minute ledger check. Status per row. Blockers surfaced.
- **Phase 5 gate review:** this ledger reviewed alongside Phase 5e gate. If 5f rows are open, schedule 5f sub-phase.
- **Phase 6 gate review:** this ledger MUST read CLOSED.
- **Refactor Done review:** this ledger file remains in the repo as historical record. It does not get deleted. It proves the compromise was honoured.

---

## 6. Row-count summary

| phase | file | ops | status |
|---|---|---|---|
| 0g | sem_os_maintenance_ops.rs | 1 | **CLOSED 2026-04-18** |
| 5f | bpmn_lite_ops.rs | 5 | OPEN |
| 5f | source_loader_ops.rs | 16 | OPEN |
| 5f | gleif_ops.rs | 17 | OPEN |
| **open remaining** | | **38 ops across 3 files (all Phase 5f)** | |
| **closed so far** | | **1 op across 1 file** | |

When every remaining row reads CLOSED and lint L4 is green workspace-wide, the compromise of D11 has been honoured in full. Phase 0g has closed the Pattern A row; Pattern B (Phase 5f, 38 ops) blocks Phase 6 per D11.
