# EOP-DESIGN-CONTROLPLANE-G2-AUDIT-PROVENANCE-001 — Audit Stream & Gate-Outcome Provenance

### Version: v0.2 (RATIFIED with one amendment)
### Date: 2026-07-13
### Status: RATIFIED (architect, 2026-07-13) with one struck item: the SHA-256 hash chain (and the tamper-evidence framing generally, including the per-event report digest) is descoped — immutable/tamper-evident audit is not needed for the present single-operator deployment. Recorded in DD-2/DD-4 below; revisit if the deployment context ever changes (multi-operator, external users, anything BNY-adjacent). All other decisions stand as drafted. Covers plan v0.5 G2 item 3 (G2b, `control_plane_audit` stream + G11 semantics) and G2 item 4 (per-gate provenance dimension) in one document, per the PIR's named-dependency instruction: whether these are one structure or two is the first question, answered here, not discovered mid-implementation.
### Basis:
- EOP-PLAN-CONTROLPLANE-GRADUATION-001 v0.5, G2 items 3–4, standing rule 3 (window discipline)
- EOP-PIR-CONTROLPLANE-GRADPLAN-001 — GRADPLAN-D-001 (no per-gate source map exists; `gate_outcome_counts` groups strictly by (gate, outcome_kind)); the G2b/item-4 same-table question
- EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001 (RATIFIED) — the consume seam whose outcomes provenance must attribute
- AD-1 RESOLVED (a): G10 grades envelope validity at consume time
- EOP-RESEARCH-CONTROLPLANE-GRADUATION-001 §A1 (G11's named blocker: no audit stream; G14's post-dispatch shape; G13 DecisionSnapshot live with samples), §A3 (shadow-decision query surface)

### Verification points (HEAD checks the implementing session performs before coding; a failed check is a stop-and-review, not a workaround):
- V1: exact storage shape of per-gate outcomes today (columns vs JSON report on `control_plane_shadow_decisions`) and what `gate_outcome_counts`/`report_to_json` actually read
- V2: G13 `DecisionSnapshot` content — sufficient to re-derive the decision outcome from recorded gate outcomes alone? (bears on DD-4's replay depth)
- V3: the sequencer commit call site G2 item 2 wires (`commit_attested`) — the same site emits this doc's `DispatchCommitted` event; coordinate the two diffs
- V4: the consume seam per the ratified seal→consume design — where `EnvelopeConsumed` emits
- V5: current `session_id` semantics on shadow rows (GM's marker predicate depends on it; DD-5 keeps this doc off that mechanism)

---

## 1. The first question: one structure or two?

**Decision DD-1: two related structures, not one — an append-only audit
stream, plus a provenance dimension that is *derived at query level*,
with the audit stream as the storage substrate for two of its three
values.**

The intuition that these are "one table wearing two hats" fails on
timing. The three provenances attach to gate outcomes produced at
three different moments in a decision's lifecycle:

| Provenance | Producer | When |
|---|---|---|
| `shadow_eval` | Path A gate stack at `phase5_runtime_recheck` | decision time |
| `consume_seam` | G10 under AD-1(a), at the admitting entry | admission time (later) |
| `post_dispatch` | G14 attestation at `commit_attested` | commit time (later still) |

A single per-decision row carrying all three requires updating that
row as later events arrive. An audit stream must be append-only —
that is what makes G11 (AuditReplay) mean anything. So one mutable
table is disqualified by the audit requirement, and one append-only
table holding "the decision's gate report" is disqualified by the
timing spread. Two structures, joined by `decision_id`.

Options considered and rejected:
- **(B) Widen `control_plane_shadow_decisions` with nullable
  consume/attest columns updated post-hoc.** Rejected: mutates window
  rows after the fact (GW's evidence base becomes retroactively
  editable — the campaign-honesty risk in plan §5, mechanized);
  breaks append-once discipline; couples GM's window predicate to
  update timing.
- **(C) Move ALL gate outcomes, including shadow evaluation, into the
  audit stream and deprecate the decision row's report.** Coherent
  long-term, but it rewrites the existing query surface
  (`gate_outcome_counts`, `report_to_json`, `shadow_divergence_stats`),
  the E3 probe, and the divergence-triage tooling inside G2's window
  constraint — maximal blast radius at the exact point standing
  rule 3 demands minimal Path-A perturbation. Rejected for G2;
  recorded as a legitimate post-GM consolidation candidate.
- **(A, adopted): decision row untouched; new append-only
  `control_plane_audit` stream carries lifecycle events including the
  later-arriving gate outcomes; provenance is materialized in the
  counting query as a UNION.** Shadow semantics untouched by
  construction; both new provenances are new writes at new call
  sites.

## 2. The audit stream

**DD-2: `control_plane_audit` — append-only, one row per lifecycle
event, typed event enum, per-stream monotonic sequence. (Hash chain
struck at ratification — see status header.)**

Schema (Postgres, in the `"ob-poc"` schema alongside the shadow table):

```sql
CREATE TABLE "ob-poc".control_plane_audit (
    seq         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    decision_id UUID        NOT NULL,
    event_type  TEXT        NOT NULL,   -- serialized AuditEvent discriminant
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    session_id  TEXT        NOT NULL,   -- same convention as shadow rows (V5)
    payload     JSONB       NOT NULL    -- typed per event_type, schema below
);
CREATE INDEX ON "ob-poc".control_plane_audit (decision_id);
CREATE INDEX ON "ob-poc".control_plane_audit (event_type);
```

Rust event type (in `ob-poc-control-plane`; persistence behind the
`database` feature — the sqlx cfg-gating defect being fixed in the
parallel E5 session is the cautionary tale, and this doc inherits its
rule: **no unconditional sqlx in any crate this touches**):

```rust
/// Exhaustively matched everywhere. Adding a variant must break every
/// consumer until handled — same doctrine as GateId / GateResult.
pub enum AuditEvent {
    DecisionEvaluated {
        outcome: DecisionOutcome,      // ApprovedStp / HumanGate / Rejected
        snapshot_ref: SnapshotId,      // G13 linkage (V2)
    },
    EnvelopeSealed   { envelope_id: EnvelopeId, expires_at: Timestamp },
    EnvelopeConsumed { envelope_id: EnvelopeId,
                       gate_outcome: GateOutcomeRecord },  // G10, provenance consume_seam
    DispatchCommitted { attested: bool,
                        gate_outcome: GateOutcomeRecord }, // G14, provenance post_dispatch
    DispatchRolledBack { reason: RollbackReason },
    DivergenceTriaged  { classification: TriageClass, runbook_ref: String },
}
```

Design points:
- **No hash chain, no digests** (struck at ratification). The stream's
  guarantees are append-only-by-convention plus same-transaction
  emission — sufficient for a single-operator deployment where the
  threat model contains no adversary. If tamper-evidence is ever
  needed, chaining retrofits with a genesis discontinuity; that cost
  is accepted knowingly rather than paid speculatively.
- **Emission is same-transaction with the state it describes** where a
  transaction exists (consume, commit — per the seal→consume design's
  atomicity guarantees); `DecisionEvaluated` emits where the shadow
  row is written today, as an additional insert in the same scope.
  **Window-discipline obligation W1 (see §6): this additional insert
  must not alter what the shadow row itself records.**
- **No `DeployMarker` or `ExerciseRun` events** — see DD-5.

## 3. The provenance dimension

**DD-3: provenance is a closed three-value enum, stored implicitly by
event locus, materialized explicitly only in the counting view.**

```rust
pub enum GateOutcomeProvenance { ShadowEval, ConsumeSeam, PostDispatch }
```

No provenance column is added to `control_plane_shadow_decisions`
(its gate report is `ShadowEval` by definition), and none is needed
on the audit rows (`EnvelopeConsumed` ⇒ `ConsumeSeam`,
`DispatchCommitted` ⇒ `PostDispatch` by construction). The dimension
becomes explicit in one place — the rebuilt counting query:

```
gate_outcome_counts :=
    SELECT gate, outcome_kind, 'shadow_eval'  AS provenance, count(*) FROM <decision-row report source>   -- V1
  UNION ALL
    SELECT gate, outcome_kind, 'consume_seam' AS provenance, count(*) FROM audit WHERE event_type='EnvelopeConsumed'
  UNION ALL
    SELECT gate, outcome_kind, 'post_dispatch' AS provenance, count(*) FROM audit WHERE event_type='DispatchCommitted'
```

The G2 item 1 fix (the `"missing"` sentinel misclassification for
historical rows) lands in the same query rebuild — one rewrite, both
fixes, coordinated in one diff.

**Per-gate expected-provenance map** (normative for the E3 probe;
G15+ additions must extend this map or fail the exhaustiveness test):
G1–G9, G12, G13 → `ShadowEval`; G10 → `ConsumeSeam` (AD-1(a));
G14 → `PostDispatch`; G11 → `ShadowEval` over the audit stream itself
(§4). The probe's assertion becomes: substantive samples exist for
each gate **at its expected provenance** — a gate reporting samples
only at the wrong provenance FAILS (that is the sentinel-detection
value the dimension exists to buy).

## 4. G11 — AuditReplay semantics (previously undefined)

**DD-4: G11 evaluates, per decision under review: (i) completeness,
(ii) outcome re-derivation. Full input-level re-execution is
explicitly out of scope for G2; integrity-as-tamper-evidence struck
at ratification (see status header).**

- **(i) Completeness:** the event sequence for the decision matches
  the legal lifecycle grammar — `DecisionEvaluated` first, ordered
  consistently by `seq`/`occurred_at`; `Sealed` iff outcome was
  ApprovedStp; `Consumed` at most once per envelope; `Committed` xor
  `RolledBack` after any `Consumed`. (Note: identity columns can skip
  values on rolled-back inserts, so seq-gaplessness is deliberately
  NOT asserted — ordering is, gaplessness isn't.)
- **(ii) Re-derivation:** recompute the decision outcome from the
  recorded gate outcomes + the G13 snapshot's decision inputs and
  compare with the recorded outcome (depth contingent on V2; if the
  snapshot is insufficient, (iii) degrades to gate-outcomes-only
  re-derivation and the gap is recorded in this doc at ratification,
  not silently).

G11 thus becomes evaluable the moment the stream has real rows —
which under AD-3(a) means GW's campaign feeds it; its E3 samples are
`ShadowEval`-provenance evaluations *about* the stream.

## 5. What this doc deliberately does NOT own

**DD-5: GW's exercise-of-record and GM's deploy marker stay on the
session-id + marker mechanism, outside the audit stream.** The stream
records what the system did; the campaign ledger records why the
operator drove it. Folding campaign bookkeeping into the audit chain
would make the evidence artifact self-referential (the stream
attesting to the honesty of the process that fills it) and couple
GM's window predicate to this doc's schema. The window predicate
remains exactly as GM defines it. If a future need arises to
correlate campaign runs to audit spans, it joins on `session_id` —
already present on both tables.

Also out of scope: `GateResult::NotApplicable` (G5 item 1 — the enum
change rides the matrix ratification, not this doc); option (C)'s
consolidation (post-GM candidate); any change to divergence
classification (standing rule 3).

## 6. Window-discipline proof obligations (standing rule 3)

All of the following are tests carried in the implementing diffs, not
assertions in prose:

- **W1** — for a fixed scenario, the `control_plane_shadow_decisions`
  row written with the audit emission in place is field-identical to
  the row written without it (golden-row test). The stream is
  additive; the shadow row is untouched.
- **W2** — divergence classification outputs are byte-identical on
  the fixture corpus pre/post (the triage path reads nothing new).
- **W3** — the rebuilt `gate_outcome_counts` returns, for the
  `shadow_eval` provenance slice, exactly the counts the old query
  returned on the same data (modulo the item-1 sentinel fix, whose
  delta is enumerated in its own test).
- **W4** — no Path-A gate emits at `ConsumeSeam` or `PostDispatch`
  provenance except G10/G14 respectively (the expected-provenance map
  enforced as a test, not a comment).

All work lands within G2, pre-merge (GM depends on G2's exit) — the
counted window therefore opens with this machinery already
underneath it, and no post-GM shadow-semantics change arises from
this design by construction.

## 7. Impact on the plan (v0.5 → v0.6 deltas on ratification)

- G2 item 3 (G2b): "build the stream" → "implement §2 + §4 of this
  doc"; its design-doc obligation is discharged by this document.
- G2 item 4: "provenance dimension" → "implement §3 of this doc";
  same discharge.
- G1 item 2's named dependency: satisfied when §3's counting view and
  probe map are merged (G10's consume-seam samples then have a home).
- G2 exit gate: unchanged in form; its "provenance dimension merged"
  clause now points at W1–W4 green + the probe's per-(gate,
  expected-provenance) assertion live.
- E3 probe: assertion change per §3; infrastructure-vs-invariant
  marker split unchanged.

## 8. Decision register

| ID | Decision | Alternatives rejected |
|---|---|---|
| DD-1 | Two structures; provenance derived at query level; audit stream stores the two late provenances | one mutable table (B); full consolidation now (C) |
| DD-2 | Append-only `control_plane_audit`, typed exhaustive `AuditEvent`; hash chain STRUCK at ratification (tamper-evidence descoped for single-operator deployment; retrofit-with-discontinuity accepted if ever needed) | event-per-gate rows (volume without value) |
| DD-3 | Closed 3-value provenance enum, implicit in storage, explicit in the counting view; per-gate expected-provenance map normative for E3 | provenance column on shadow rows (mutation risk, redundant) |
| DD-4 | G11 = lifecycle completeness + outcome re-derivation; input-level replay out of scope; tamper-evidence integrity struck at ratification | full re-execution (unscoped, snapshot-dependent) |
| DD-5 | Campaign/deploy bookkeeping stays out of the stream; join on session_id if ever needed | audit-stream self-reference |
