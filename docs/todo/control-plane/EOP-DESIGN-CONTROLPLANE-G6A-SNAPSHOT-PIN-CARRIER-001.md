# EOP-DESIGN-CONTROLPLANE-G6A-SNAPSHOT-PIN-CARRIER-001

### Design doc, not implementation. Review before any code lands beyond the narrow, fail-closed slice §6 names as safe to land now.
### Basis: `EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.5.md` §3 tranche G6a. Architect ratified option (b) of R:§C2 (2026-07-14 session instruction): populate the dormant `snapshot_pin` proto field end-to-end, not option (a) (`inputs` binding).
### Status: DRAFT, this session. §7 names one open question that is NOT resolved by this doc and needs architect sign-off before it is closed — the rest is implemented under the fail-closed default this doc establishes.

---

## 0. What this doc resolves

The architect ratified *where* the handle rides (the proto's `snapshot_pin`
field, not `inputs`). It did not resolve *what a bare `Uuid` in that field
means*, because it cannot mean what `execute_verb_admitting_envelope`'s
existing `Option<EnvelopeHandle>` parameter has meant for Path A since T8.1:
`EnvelopeHandle` is `{id: Uuid, content_hash: [u8; 32]}` (`ob-poc-types/src/
envelope_handle.rs:29-33`) — a compound value whose `content_hash` is the
whole point of T8.1 (closes PIR-D-008/PIR-D-010: an id-only handle was
demoted to `#[cfg(test)]`-only, `try_consume_by_id`, specifically because
"no production reason to exist" once the typed handle carries a hash to
check). `InvocationRequest.snapshot_pin` is `message Uuid { bytes value = 1;
}` (`dsl-bus-protocol/proto/dsl_bus.proto:20-22,149`) — 16 bytes, no room
for a 32-byte hash alongside it in the same field. This doc resolves what
the bare UUID that field actually carries, and how (or whether)
content-hash-equivalent verification happens for Path D.

**Resolution reached below (§3-§5): the bare UUID is a correlation id
`ob-poc` uses as `persist_sealed`'s `entry_id` when it mints and consumes
its own envelope synchronously, in one call — bpmn-lite never mints a
handle at all. §7 flags one load-bearing open question this resolution
surfaces that this doc does not itself decide: whether `evaluate()`/
`evaluate_with_report()` should become path-aware for G3/G9. Until that is
answered, the honest, disclosed, fail-closed consequence is that Path D can
never actually reach `ApprovedStp` through this mechanism — safe, not a
security gap, but a real functional limitation worth landing anyway (see
§6) because it is real, tested infrastructure the day that gap closes.**

---

## 1. Why bpmn-lite cannot mint a real `EnvelopeHandle` at all

Re-verified against `~/dev/bpmn-lite` at the commit checked out this
session (`619370d4`+ on `main`, one unrelated commit ahead per this
session's own `git log` check — a dedupe-cache fix, not touched by this
work). `bpmn-lite-engine/src/plan_walker.rs::dispatch_callout` (lines
289-374) is the real production producer of the `InvocationRequest`s
Path D admits (confirmed, matching R:§C2/C3) — it builds `inputs` from
`instance.placeholder_values` + static plan-node args (`build_inputs`,
line 610) and sets `snapshot_pin: None` unconditionally (line 336).

`ExecutionEnvelope::seal` (`ob-poc-control-plane/src/envelope.rs:257`)
requires eight proof-typed values: `AdmittedIntent`, `BoundEntities`,
`ResolvedPack`, `LegalTransition`, `Authorised`, `EvidenceSufficient`,
`WriteSetProof`, `CompiledRunbookRef`, plus `SnapshotPins` — each derived
by re-running the corresponding gate module's own `decide()`/`classify()`/
`build_pins()` against a real `EvaluationContext` field
(`decision.rs:196-321`). None of these exist as concepts inside
`bpmn-lite`: it has no `SemReg` pack registry, no compiled runbook object,
no entity-lifecycle-state reader, no authority/evidence store. `bpmn-lite`
is a separate git repo with zero dependency on `ob-poc-control-plane` (its
`Cargo.toml` does not reference the crate at all — confirmed by its
absence from `~/dev/bpmn-lite`'s own workspace `Cargo.toml` dependency
list). **A bpmn-lite-side seal is not a smaller/deferred version of Path
A's mechanism — it is structurally impossible without importing the whole
control-plane proof-input surface into a repo that has no source for any
of those facts.** This is the load-bearing fact the rest of this doc's
resolution follows from; option (a) of R:§C2 (`EnvelopeHandle` riding in
`inputs`) has exactly the same problem for the same reason — the carrier
choice was never actually the hard part; who can mint a real handle is.

Corollary: whichever of R:§C2's two carriers the architect chose, **the
seal has to happen inside `ob-poc`, not `bpmn-lite`**, because only
`ob-poc` holds the facts `ExecutionEnvelope::seal` requires. `snapshot_pin`
riding as a bare `Uuid` is not a limitation this doc works around — it is
the right-shaped field for what bpmn-lite can actually supply: a
correlation id, not a security credential.

---

## 2. What bpmn-lite *can* supply, and what it should carry

`dispatch_callout` already computes a stable, deterministic,
per-callout-attempt correlation id: `callout_id = derive_uuid("callout_id",
instance.instance_id, &node_id, attempt_count)` (line 321) — derived, not
random, so retries of the same node/attempt reproduce the same id (used
today for `PendingInvocation`/`TickOperation::InsertPendingInvocation`
dedup bookkeeping, line 341-349). This is exactly the shape `persist_sealed`
already wants for its `entry_id` parameter (`control_plane_envelope_store.
rs:356`, added by G1 §2.1 specifically to correlate a sealed row to "the
specific step that produced it" — `RunbookEntry.id` for Path A,
`CompiledStep.step_id` at the consume site). **Resolution: `snapshot_pin`
carries `callout_id`, populated by `dispatch_callout` on every submission,
threaded through as `Option<Uuid>` at every hop, and used inside `ob-poc`
purely as `persist_sealed`'s `entry_id` — audit/correlation metadata
linking a sealed-and-immediately-consumed `control_plane_envelopes` row
back to the exact bpmn-lite callout that triggered it, never as an
externally-supplied identity `ob-poc` trusts without independently
deriving its own content hash.**

**Aside, out of scope, disclosed not fixed:** `dispatch_callout` also sets
`authority: None` unconditionally (line 333). `dsl-bus-server::services.rs`
`InvocationServiceImpl::submit` (lines 116-125) rejects any request with
`authority: None` as `RejectedAuthority` ("Missing authority context")
before it ever reaches the dispatcher — this looks like it would make
every real bpmn-lite→ob-poc callout reject today, independent of this
doc's scope. Not investigated further or touched here: it is a distinct,
pre-existing gap (R4's tenant-derivation check, unrelated commit history)
that this session's own `git log -- dsl-bus-server/src/services.rs`
shows is not part of the parallel dedupe-cache fix either. Flagged for the
operator to triage separately; `snapshot_pin` threading does not depend on
it (both fields are independent, optional/required proto fields on the
same message) and this doc's own tests below construct `InvocationContext`
directly, bypassing `submit`'s authority gate, so they are unaffected by
it either way.

---

## 3. Why this is not a reopening of the T8.1 id-only-consume gap

`try_consume_by_id` (id-only, no content-hash check) was deliberately
demoted to `#[cfg(all(test, feature = "database"))]` by T8.1 specifically
because a production caller trusting a bare id without verifying a hash
against it is a real weakening (`control_plane_envelope_store.rs:620-636`).
This design does **not** resurrect that path: `mint_envelope_for_bus` (§4)
never calls `try_consume`/`try_consume_by_id`/`try_consume_in_scope`
against a caller-supplied id at all. It mints a **brand new**
`ExecutionEnvelope` via `ExecutionEnvelope::seal` (computing its own
content hash from a `SnapshotPins`/proof set `ob-poc` itself derived,
zero of it sourced from the wire) and persists it. The actual consume —
the step that checks a presented handle's content hash against the
persisted row — is **not** run by this method at all; it happens later,
inside `execute_verb_admitting_envelope`'s own `admit_in_scope` call
(§4's revised design, corrected during this session's own live-DB test
run — see below), using `envelope.handle()`'s own freshly-computed hash,
never anything bpmn-lite sent. `bus_pin`/`callout_id` never touches the
hash-comparison code path (`consume_core`'s `expected_content_hash`
parameter) at all — it only ever reaches `persist_sealed`'s `entry_id`
column. There is no new "trust an id without a hash" surface here; the
hash check T8.1 built stays exactly as strict as it already is for every
existing caller.

---

## 4. The mechanism (`mint_envelope_for_bus`)

New `pub async fn` on `ObPocVerbExecutor`
(`rust/src/sem_os_runtime/verb_executor_adapter.rs`, alongside the existing
`execute_verb_admitting_envelope`/`admit_in_scope`) — lives inside the
`ob-poc` crate specifically because `EnforcedVerbs`, `persist_sealed`, and
friends are `pub(crate)` to `ob-poc` (`agent::control_plane_envelope_
store.rs`), not reachable from `ob-poc-web` (where `bus_runtime.rs`
lives) — confirmed by grep: every existing cross-boundary call from
`bus_runtime.rs` into `ob_poc_control_plane::*` goes through fully-`pub`
crate items only (`evaluate_shadow`, `applicability::apply_matrix`, the
`context`/`intent_admission`/`stp_classifier`/`versioning`/`snapshot`
module types), never `ob-poc`'s own `pub(crate)` `agent::` modules.

**Correction made during this session's own live-DB test run (RED found
against the first draft):** the first draft of this method both minted
*and* consumed the envelope in one call, mirroring the prose in §0/§1's
early framing. A live-DB test written against that draft
(`enforced_verb_with_full_context_mints_and_consumes`, §8) exposed why
that is actually the wrong shape: consuming inside `mint_envelope_for_bus`
would run the consume in its **own** transaction, separate from
`execute_verb_admitting_envelope`'s dispatch scope — breaking T9.2's
rollback-together property (a dispatch failure after admission must roll
the consume back too, so the envelope is reconsumable, not permanently
burned on a transient failure; Path A already has this, proven live-DB by
`execute_verb_admitting_envelope_rolls_back_the_consume_when_dispatch_
fails`). **Revised: `mint_envelope_for_bus` only mints and persists
(`status = 'sealed'`) — it does not consume.** The handle it returns is
threaded into the existing `execute_verb_admitting_envelope` call exactly
as any other envelope handle would be; that call's own `admit_in_scope` →
`check_admission_in_scope` → `try_consume_in_scope_with_pins` chain
performs the actual consume, inside the same scope as the dispatch it
gates — giving Path D the identical atomicity property Path A already
has, not a weaker one. This is a strict improvement over the original
"mint and consume in the same call" framing, found by testing the claim
rather than trusting it.

```rust
/// G6a: Path D's own admission-time envelope minting. Bus-federated
/// callers cannot mint a real `ExecutionEnvelope` (§1) — this method
/// mints and persists (`status = 'sealed'`) but does **not** consume;
/// the caller threads its returned handle into the existing
/// `execute_verb_admitting_envelope` call, whose own admission chain
/// performs the actual consume inside the same scope as the dispatch it
/// gates — preserving T9.2's rollback-together atomicity (see the
/// correction note above this listing). Short-circuits to `None`
/// whenever `verb_fqn` isn't enforce-gated for `ExecutionPath::
/// BusFederated` — zero added cost on the production-default (nothing
/// enforced) path, matching `check_admission`'s own early-return shape.
pub async fn mint_envelope_for_bus(
    &self,
    verb_fqn: &str,
    cp_ctx: &ob_poc_control_plane::context::EvaluationContext,
    bus_pin: Option<Uuid>,
) -> Option<ob_poc_types::EnvelopeHandle> {
    let enforced = crate::agent::control_plane_envelope_store::EnforcedVerbs::from_env().ok()?;
    if !enforced.is_enforced(verb_fqn, ob_poc_types::ExecutionPath::BusFederated) {
        return None;
    }
    let now = chrono::Utc::now();
    let validity = ob_poc_control_plane::envelope::ValidityWindow::new(
        now, now + chrono::Duration::minutes(5),
    );
    let decision = ob_poc_control_plane::decision::evaluate(cp_ctx, validity);
    let ob_poc_control_plane::decision::ControlPlaneDecision::ApprovedStp(envelope) = decision
    else {
        return None; // Rejected / RequiresHumanGate -> no envelope for Path D today (§7).
    };
    let entry_id = bus_pin.unwrap_or_else(Uuid::new_v4);
    let pool = self.executor.pool();
    if !crate::agent::control_plane_envelope_store::persist_sealed(
        pool, Uuid::nil(), entry_id, verb_fqn, &envelope,
    ).await {
        return None; // best-effort persist failed -> don't hand out an
                      // id that can't be found at consume time (would
                      // just surface as NotFound anyway; short-circuit
                      // here is honest, not a behaviour change).
    }
    Some(envelope.handle())
}
```

`bus_runtime.rs::ObPocVerbAdapter::execute` calls this **before** the
existing `execute_verb_admitting_envelope` call, threading its result into
the fourth argument in place of the hardcoded `None`:

```rust
let bus_pin = /* from the newly-widened trait param, §5 */;
let handle = self.executor
    .mint_envelope_for_bus(local_verb_id, &cp_ctx, bus_pin)
    .await;
let result = self.executor
    .execute_verb_admitting_envelope(local_verb_id, args, &mut ctx, handle, ExecutionPath::BusFederated)
    .await
    .map_err(map_executor_error)?;
```

`cp_ctx` is the same `EvaluationContext` the adapter already builds for
its existing shadow-only `evaluate_shadow` call (currently built inline
inside the `tokio::spawn` block) — reused, not duplicated: built once,
cloned/referenced for both the unchanged shadow-audit spawn and the new
real-evaluation call.

This keeps `execute_verb_admitting_envelope`'s own signature, and its
already-proven T9.2/T10.2 atomicity (single-use consume, rollback-together,
pin-verification-in-scope) completely untouched — Path D gets the exact
same admission mechanism Path A already has proven live-DB, differing only
in *where* the `Some(handle)` it receives came from.

---

## 5. Wire threading (bpmn-lite: `plan_walker.rs` → `services.rs`; ob-poc: trait → adapter)

1. **`bpmn-lite-engine/src/plan_walker.rs::dispatch_callout`**: `snapshot_
   pin: Some(uuid_to_proto(callout_id))` in place of `None` (line 336) —
   `callout_id` is already in scope at that point (line 321-323).
2. **`dsl-bus-server/src/services.rs`**: `InvocationContext` (lines 41-49)
   gains `pub snapshot_pin: Option<Uuid>`; `submit()`'s `ctx` construction
   (lines 175-183) gains `snapshot_pin: from_proto_opt(&req.snapshot_pin)
   .unwrap_or(None)` — malformed-but-present is treated as absent (`None`),
   not a hard reject, since this field's whole design contract (§3) is
   "correlation metadata, never trusted for identity/security" — a
   malformed pin degrades to "no correlation available," the same outcome
   as it being genuinely absent, not a reason to reject a dispatch that
   would otherwise be perfectly legitimate. (Contrast `idempotency_key`,
   which *is* trusted for identity and correctly hard-rejects on
   malformed input.)
3. **`ob-poc-bus-handler/src/lib.rs`**: `VerbExecutor::execute` gains a
   fifth parameter `snapshot_pin: Option<uuid::Uuid>`; `ObPocBusHandler::
   dispatch` passes `ctx.snapshot_pin` through.
4. **`ob-poc-web/src/bus_runtime.rs`**: `ObPocVerbAdapter::execute`'s
   signature widens to match; the incoming `snapshot_pin` becomes `bus_pin`
   in §4's call.

---

## 6. What lands now vs. what is flagged (fail-closed default)

**Lands now, unconditionally safe:**
- The full wire threading (§5) — real, tested, working regardless of §7.
- `mint_envelope_for_bus` (§4) — real, tested against
  synthetic full `EvaluationContext` fixtures (proving the *mechanism*:
  admit when `ApprovedStp`, reject/no-op otherwise, entry_id correlation
  correct, zero cost when unenforced).
- Behaviour for every verb NOT listed in `OB_POC_CONTROL_PLANE_ENFORCE_
  VERBS:D` (the production default, currently every verb): **byte-for-byte
  identical to today** — `mint_envelope_for_bus` short-circuits
  to `None` before touching `evaluate()`/the DB at all, and
  `execute_verb_admitting_envelope(..., None, BusFederated)` was already
  the exact call shape in production before this change.

**Honest, disclosed, NOT a security gap — a functional limitation left
open, flagged in §7 for architect review:** for any verb an operator *does*
list under `OB_POC_CONTROL_PLANE_ENFORCE_VERBS:D` (not the default; an
explicit opt-in per G3's own design), `mint_envelope_for_bus`
will **always** return `None` in production today — not because of a bug
in this doc's mechanism, but because `evaluate()`'s own `PROOF_BEARING_
GATES` check (`decision.rs:76-83`) unconditionally requires `GateId::
PackResolution` and `GateId::RunbookProof` to succeed, and both are
concepts `applicability()` (`applicability.rs:103-109,131-138`) already
independently confirms bus dispatch structurally cannot supply ("bus
dispatch has no runbook object at all" — `G9_JUSTIFICATION_D`). This means
today, opting a bus verb into enforce mode makes that verb's Path D
dispatch behave exactly like it would if this whole tranche had never
landed: `RejectedNoEnvelope`-shaped rejection, 100% of the time, on
every attempt — the same "graduating before this gap closes is an outage
on that verb" shape G1's own doc names for Path A's pre-landing state,
not a new failure mode this doc introduces. It is fail-closed, not a
weaker check than before (there is no "before" for Path D enforce-mode
in production — nothing has ever been listed under `:D` in the shadow-
only production default), so nothing regresses. It is landed anyway
because the wire-level plumbing (§5) and the mint/consume mechanism (§4)
are real, independently tested infrastructure that becomes immediately
useful the moment §7 is resolved — not scaffolding that has to be
re-threaded later.

---

## 7. Open question flagged for architect sign-off (NOT decided by this doc)

`decision.rs`'s own existing comment (lines 103-130, pre-dating this
session) already names this exact question and explicitly defers it:
*"Whether a future path-aware caller of `evaluate`/`evaluate_with_report`
should treat `NotApplicable` among `PROOF_BEARING_GATES` as vacuously
satisfied (like `Success`) or as a hard block is a real design question
this sweep does not answer... Resolved here as fail-closed... until a
real path-aware caller of this function exists and that design question
gets its own review."*

**This design doc's `mint_envelope_for_bus` is that first real
path-aware caller.** Whether `evaluate()`/`evaluate_with_report()` should
be widened to take an `ExecutionPath` and treat `GateId::PackResolution`/
`GateId::RunbookProof` as vacuously satisfied for `BusFederated` (and,
symmetrically, `DslDirect`/`WorkflowDispatched`, which `applicability()`
already marks `NotApplicable` for the same two gates) is a real,
security-relevant decision this doc does **not** make: it changes what
gets admitted vs rejected for every future path-aware caller of a shared,
already-load-bearing function Path A depends on today (any change to
`evaluate`/`evaluate_with_report` risks Path A regression if done
carelessly — it is not an isolated addition, it is a modification to code
`sequencer.rs`'s `phase5_runtime_recheck` already calls in production).

**Recommendation (not self-authorised):** widen `evaluate_with_report` to
accept `path: ExecutionPath` (defaulting Path A call sites to
`RunbookSequencer`, unchanged behaviour there since `applicability()`
marks nothing `NotApplicable` for `RunbookSequencer`), and inside
`rejection_from_report`'s `PROOF_BEARING_GATES` walk, treat a gate whose
`report.get(id)` is `NotEvaluated`/absent-by-design **and** whose
`applicability(id, path)` is `NotApplicable` as vacuously satisfied rather
than a `GateFailure` — mirroring what `apply_matrix` already does for the
*shadow*-decision-row presentation layer, but now inside the real
admission-affecting decision function. This is the smallest change that
resolves the comment's own named question consistently with the ratified
applicability matrix (G5, already architect-approved) rather than
inventing new semantics.

**Until that lands (a separate, reviewed change — not part of this
session):** enforce-mode is real infrastructure for Path D but not yet
*usable* infrastructure — an operator who lists a bus verb under
`OB_POC_CONTROL_PLANE_ENFORCE_VERBS:D` gets a permanent, honest rejection,
not a silent bypass. This is the correct, disclosed, fail-closed default
to ship while §7 awaits its own review.

---

## 8. Test plan

- **bpmn-lite** (`dsl-bus-server`, live crate-internal tests): `submit()`
  with a real `snapshot_pin` set on the wire request → asserts the
  `InvocationContext` the dispatcher receives carries the matching `Some
  (uuid)`; a second test with `snapshot_pin: None` → asserts `None` flows
  through unchanged (regression guard against the widening breaking the
  no-pin case). `bpmn-lite-engine`: `dispatch_callout` unit/integration
  test asserting the constructed `InvocationRequest.snapshot_pin` matches
  the same `derive_uuid("callout_id", ...)` value `PendingInvocation`
  bookkeeping already uses.
- **ob-poc** (`ob-poc-bus-handler` unit tests): widen the existing
  `MockExecutor`/`ctx()` fixture (`src/tests.rs`) to carry `snapshot_pin`,
  add a case asserting `ObPocBusHandler::dispatch` forwards it unchanged
  to `VerbExecutor::execute`'s new parameter.
- **ob-poc** (`verb_executor_adapter.rs`, live-DB, `#[ignore]`-gated
  matching house convention): `mint_envelope_for_bus`
  end-to-end —
  1. Not-enforced verb → `None` regardless of `cp_ctx`/`bus_pin` content,
     zero DB writes (RED before any implementation: the function doesn't
     exist yet; GREEN after).
  2. Enforced verb + a full `sealable_context()`-shaped `EvaluationContext`
     (mirroring `decision.rs`'s own fixture, including synthetic
     `pack_resolution`/`runbook_proof` inputs — proving the *mechanism*
     works when all eight proof-bearing facts are genuinely present,
     which is the honest state this infra is ready for the moment §7
     resolves) → `Some(handle)`, and a row appears in `control_plane_
     envelopes` with `entry_id = bus_pin` and `status = 'sealed'` — NOT
     `'consumed'` (§4's correction: minting persists but does not
     consume; consumption is deferred to the caller's
     `execute_verb_admitting_envelope` call, below).
  3. Enforced verb + today's actual Path D-realistic `EvaluationContext`
     (only `intent_admission`/`stp_classifier`/`version_pinning`
     populated, matching `bus_runtime.rs`'s real construction) → `None`,
     proving §6's disclosed limitation is real and reproducible, not
     asserted from prose alone.
  4. End-to-end: case 2's handle threaded into `execute_verb_admitting_
     envelope(..., Some(handle), BusFederated)` → dispatch succeeds and
     the envelope shows `status = 'consumed'`, single-use (a second
     dispatch attempt with the same handle is rejected
     `AlreadyConsumed`) — proving the full chain, not just the minting
     half.
  RED→GREEN proof: run case 2 and case 4 against `git stash`'d code (the
  method doesn't exist, compile fails / or against a version that still
  passes `None` unconditionally) to confirm they fail before the change
  and pass after — matching this program's established discipline.

---

## 9. Files touched (for the operator's independent review — not committed by this session)

**bpmn-lite** (`~/dev/bpmn-lite`):
- `bpmn-lite-engine/src/plan_walker.rs` — `dispatch_callout` populates `snapshot_pin`.
- `dsl-bus-server/src/services.rs` — `InvocationContext` widened + copied from `req.snapshot_pin`.
- `dsl-bus-server/src/tests.rs` — new coverage (§8).
- `bpmn-lite-engine` tests — new coverage (§8).

**ob-poc** (`~/Developer/ob-poc`):
- `rust/crates/ob-poc-bus-handler/src/lib.rs` — `VerbExecutor::execute` trait signature widened.
- `rust/crates/ob-poc-bus-handler/src/tests.rs` — fixture + coverage updated.
- `rust/crates/ob-poc-web/src/bus_runtime.rs` — `ObPocVerbAdapter::execute` threads the pin, calls the new mint method, replaces the hardcoded `None`.
- `rust/src/sem_os_runtime/verb_executor_adapter.rs` — new `mint_envelope_for_bus` + live-DB tests.
- `docs/todo/control-plane/EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md` — §4/§8 correction per R:§C3 (bpmn-lite-side work is real, not "ob-poc-only").
