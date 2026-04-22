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

### 2.2 File: `sem_os_postgres/src/ops/agent.rs` (added 2026-04-22)

| op | lines | external effect | resolution path | status |
|---|---|---|---|---|
| `ActivateTeaching` | ~796 | `tokio::process::Command::new("cargo")` spawns `cargo run --release -p ob-semantic-matcher --bin populate_embeddings` subprocess inside the verb body. Same shape as the Phase-0g-remediated `MaintenanceReindexEmbeddingsOp` but in a different file — likely introduced during the Slice-#80 relocation of CustomOperation impls. Caught 2026-04-22 by the new `lint_external_effects_in_verbs.sh` (L4 script). | **Outbox deferral, same pattern as 2.1.** Reuse `OutboxEffectKind::MaintenanceSpawn` + the existing `MaintenanceSpawnConsumer`. Rewrite `execute` to insert an outbox row instead of spawning directly. Admin synchronous path preserved via the same `cargo run` command. | **OPEN** |

**Completion criteria for 2.2:**

- [x] Lint L4 catches the violation — `scripts/lint_external_effects_in_verbs.sh` grandfathers this file explicitly as of 2026-04-22 (no new violations permitted while the ledger row is open).
- [ ] `ActivateTeaching::execute` rewritten to `INSERT INTO public.outbox ... effect_kind='maintenance_spawn'` with idempotency key `activate_teaching:<trace_id>:<date>`.
- [ ] Lint L4 passes against `sem_os_postgres/src/ops/agent.rs` — remove from `GRANDFATHERED` in the lint script.
- [ ] Row above moves to **CLOSED**.

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
| `BpmnCompileOp` | `client.compile(xml) → bytecode` | write-through | **Outbox deferral with callback.** Verb writes a `bpmn_compilation_requests` row (pending), emits `OutboxDraft::BpmnDispatch { op: Compile, payload: xml, request_id }`. Drainer calls gRPC, writes bytecode back to DB. Caller polls or awaits completion event. | **OPEN** |
| `BpmnStartOp` | `client.start(bytecode, vars) → instance_id` | write-through | Same pattern. Placeholder row + outbox. Caller gets `instance_id` via callback. | **OPEN** |
| `BpmnSignalOp` | ~~`client.signal(instance_id, payload)`~~ — REMOVED from verb body | fire-and-forget | **Outbox deferral complete (2026-04-22).** Verb inserts a `bpmn_signal` row into `public.outbox` inside the ambient scope. Idempotency key: `bpmn_signal:<instance_id>:<message_name>:<blake3-16>`. Returns `Void` synchronously. Atomicity preserved — outer rollback removes the queued signal. **Drainer consumer** (`BpmnSignalConsumer`) pending — outbox rows accumulate with status=`pending` until Phase F.1b lands. Direct synchronous operator path: call the gRPC `signal` RPC against `BPMN_LITE_GRPC_URL` directly. | **PARTIAL (verb refactored; consumer pending)** |
| `BpmnCancelOp` | `client.cancel(instance_id, reason)` | fire-and-forget | Simpler outbox deferral. | **OPEN** |
| `BpmnInspectOp` | `client.inspect(instance_id) → inspection` | read-only | **Pre-fetch at stage 5.** Read-only inspect can happen before txn opens, during intent resolution. Passes inspection result into envelope args. | **OPEN** |

**Completion criteria for bpmn_lite_ops.rs:**

- [ ] `OutboxEffectKind::BpmnDispatch { op_kind, ... }` variant added.
- [ ] 4 write-path ops rewritten to return `OutboxDraft`; callers refactored to handle async completion (`bpmn_lite_grpc_callback` route, existing BPMN integration).
- [ ] 1 read-only op (`BpmnInspectOp`) moved to pre-txn fetch (stage 5 of Sequencer) — or, if that's infeasible, also deferred to outbox.
- [ ] gRPC calls inside execute bodies: **zero** (verified by lint L4).
- [ ] Integration test: BPMN compile → instance_id returned via callback; signal/cancel round-trip; inspect returns fresh data.
- [ ] Row above moves to **CLOSED**.

### 3.2 File: `source_loader_ops.rs` — 16 ops

All 16 instantiate external HTTP loaders (`CompaniesHouseLoader`, `GleifLoader`, `SecEdgarLoader`) and call `.search()` / `.fetch_entity()` / `.fetch_control_holders()` inside `execute_json`.

| sub-group | ops | resolution |
|---|---|---|
| Search ops (return candidate lists) | 4 ops (CH search, SEC search, …) | **Pre-fetch at stage 5.** Fetch happens during intent resolution; results passed into verb envelope args. No txn wraps the HTTP call. |
| Fetch-entity ops (retrieve + persist) | 8 ops | **Split fetch-then-persist.** Each op restructures: (a) HTTP fetch phase (no txn), (b) txn opens, (c) DB persist phase. Cleaner than outbox for these because persist is the domain interest and must be synchronous with the user flow. |
| Fetch-control-holders ops (retrieve + derive graph) | 4 ops | Same split-phase treatment. |

**Completion criteria for source_loader_ops.rs:**

- [ ] Every op either: (a) routes HTTP fetch through pre-txn stage 5 pre-fetch, or (b) splits its body into pre-txn fetch + in-txn persist, with the HTTP call demonstrably outside the Sequencer-opened transaction.
- [ ] HTTP calls inside execute_json bodies: **zero** (lint L4).
- [ ] Integration tests for each op: state before/after identical to current behaviour; timing shows HTTP happens before `BEGIN`.
- [ ] Row above moves to **CLOSED**.

**Status:** 0/16 ops remediated. **OPEN.**

### 3.3 File: `gleif_ops.rs` — 17 ops

Same pattern as `source_loader_ops.rs`. 17 ops instantiate `GleifClient` or `GleifEnrichmentService` and make HTTP calls inside `execute_json`.

Apply the same split-phase / pre-fetch treatment. Most ops are fetch-then-persist (LEI lookup → DB write of entity).

**Completion criteria for gleif_ops.rs:**

- [ ] Each of 17 ops: HTTP fetch moves out of `execute_json` body, into stage 5 pre-fetch or an explicit pre-txn phase.
- [ ] HTTP calls inside execute_json bodies: **zero** (lint L4).
- [ ] Integration tests green.
- [ ] Row above moves to **CLOSED**.

**Status:** 0/17 ops remediated. **OPEN.**

### 3.4 Phase 5f completion criteria

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
