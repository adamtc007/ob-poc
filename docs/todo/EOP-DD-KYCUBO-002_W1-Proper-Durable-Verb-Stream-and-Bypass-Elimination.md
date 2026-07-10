# KYC/UBO — W1-Proper: Durable Verb Stream & Bypass Elimination
### The persistence + seam design (DB changes start here)

| | |
|---|---|
| **Document** | EOP-DD-KYCUBO-002 |
| **Type** | Design / build hand-off (durable substrate; **first schema**; modifies authoritative write paths) |
| **Version** | 0.3 — Owner decisions ratified; authorises build |
| **Owner** | Adam Cearns |
| **Date** | 2026-06-30 |
| **Binds to** | EOP-DD-KYCUBO-001 v0.2 (the in-memory slice — `KycEventStore` trait, folds, determination, all pure); EOP-VS-KYCUBO-001 v0.6 (V&S); Addendum A (ratified Q4–Q7; H1/H2/H6). |
| **Status** | **AUTHORISES BUILD.** D1–D3 ratified (§12); §3.4 cross-stream contract adopted. No open `DECISION REQUIRED` remains. D2 means W1-proper reaches back into the substrate crate to add content-addressed fold version-dispatch (§3.5) — still pure, still zero sqlx. |

> **What W1-proper proves — and what it does NOT (H2).** It proves the **durable substrate**: the verb stream is the transactional system of record, per-subject ordered under concurrency, with replay/recovery from the durable store and projections strictly derived, on **one structure class (private company), end to end** — and it eliminates the H1 bypass paths for that class. It does **not** prove the multi-class determination *content* (W4, gated on golden fixtures) and does **not** execute screening/risk. This is the slice made durable, not the determination widened.
>
> **First time we modify authoritative writes.** Everything to date was additive (a new pure crate). W1-proper changes existing write paths. H6 (strangler consistency) stops being theoretical here.

> **v0.2 reconciliation note (end-of-design review).** The concurrency spine (§3) and the strangler authority rule (§6) are accepted as-is — they are the correct hard calls. The review found that **durability converts three latent slice-era ambiguities into authoritative-state corruption risks**, and that the single-subject rule (correctly chosen) pushes a hard problem from concurrency into **cross-stream consistency** that v0.1 did not specify. Deltas are marked **[v0.2]** at first occurrence and indexed in §13. Empirical "verified:" claims in v0.1 were checked against the tree; corrections are in §13-L.
>
> **v0.3 ratification note (owner).** The three open decisions are closed: **D1 — transaction-time recovery only** (as-valid-at-T deferred to W6). **D2 — version-dispatch the fold now**, not freeze-and-guard: the owner's directive is *don't kick the issue down the road* — the content-addressed fold-dispatch seam is built in W1-proper while one version exists, so a second lexicon version is purely additive. **D3 — board-control overrides become verbs**, on the principle *all changes are verbs, no side effects* (K-15/K-32/K-35): the human-authored override table is eliminated, the override is an event on the stream. v0.3 edits are marked **[v0.3]**.

---

## 1. Crate placement (per the crate discipline)

The pure crate stays pure. A **new membrane crate** holds all DB access for the domain.

```
ob-poc-types                 (leaf — IntentEvent, Principal, AuthorityRef, taxonomy types)
      ▲
ob-poc-kyc-substrate         (PURE — folds, determination, KycEventStore TRAIT)   ← unchanged, still zero sqlx
      ▲
ob-poc-kyc-store             (NEW MEMBRANE — impl KycEventStore over Postgres; the ONLY sqlx crate)
      ▲
sem_os_postgres / dsl-runtime  (thin verb-op adapters; append at the TransactionScope seam)
```

`ob-poc-kyc-store` depends on `ob-poc-kyc-substrate` (for the trait it implements) + `ob-poc-types` + `sqlx`. The substrate **never** learns about Postgres — the dep-gate (`scripts/check_kyc_substrate_deps.sh`, proven red/green in DD-001 §6a) keeps it honest. **[v0.2]** The same gate must be extended so the substrate also stays free of `ob-poc-kyc-store` (the membrane may depend on the substrate, never the reverse). This is what keeps `recover_determination_at` (in the substrate, pure) testable without a DB.

---

## 2. The durable store — first migration

Two tables. The event stream is authoritative; the stream table is the per-subject sequence allocator.

```sql
-- The authoritative system of record (K-16). Append-only. Per-subject ordering (Q6).
CREATE TABLE "ob-poc".kyc_intent_events (
    subject_root     uuid        NOT NULL,
    seq              bigint      NOT NULL,           -- dense, per subject_root (Q6)
    event_id         uuid        NOT NULL,
    verb_fqn         text        NOT NULL,
    lexicon_hash     text        NOT NULL,           -- exact verb definition (Q7); see H-d / §3.5
    actor            jsonb       NOT NULL,           -- Principal
    authority        text        NOT NULL,           -- object-capability (K-17, K-35)
    target           jsonb       NOT NULL,
    payload          jsonb       NOT NULL,
    payload_hash     text        NOT NULL,
    idempotency_key  text        NOT NULL,
    causation_id     uuid        NULL,               -- [v0.2] load-bearing for cross-stream (§3.4)
    correlation_id   uuid        NOT NULL,
    as_of            timestamptz NOT NULL,           -- FROZEN clock, an input (Q6); VALID-time (B1)
    committed_at     timestamptz NOT NULL DEFAULT now(),  -- TRANSACTION-time; recovery axis (B1)
    captured_effects jsonb       NOT NULL DEFAULT '[]',  -- external-lookup results (Q6/H5)
    PRIMARY KEY (subject_root, seq),
    UNIQUE (subject_root, idempotency_key)           -- idempotent re-apply (F)
);
CREATE INDEX kyc_intent_events_event_id_idx ON "ob-poc".kyc_intent_events (event_id);
CREATE INDEX kyc_intent_events_corr_idx     ON "ob-poc".kyc_intent_events (correlation_id);
-- [v0.2 / B1] recovery is by transaction-time; index it.
CREATE INDEX kyc_intent_events_committed_idx ON "ob-poc".kyc_intent_events (subject_root, committed_at);

-- Per-subject sequence allocator + optional fold checkpoint.
CREATE TABLE "ob-poc".kyc_subject_streams (
    subject_root      uuid PRIMARY KEY,
    next_seq          bigint      NOT NULL DEFAULT 0,
    checkpoint_seq    bigint      NULL,              -- last folded seq (perf; pure-derivable)
    checkpoint_state  jsonb       NULL,              -- folded snapshot at checkpoint_seq
    checkpoint_lexicon_manifest text NULL,           -- [v0.2/H-d] manifest the checkpoint was folded under
    updated_at        timestamptz NOT NULL DEFAULT now()
);
```

**[v0.2] `seq` allocation invariant (do not regress).** `seq` is allocated from `kyc_subject_streams.next_seq` (a transactionally-updated column), **never** from a Postgres `SEQUENCE`/`nextval()`. This is load-bearing: gap-free dense `seq` under rollback (exit criterion 2) holds *only* because allocation, increment, and event-insert share one transaction. A `SEQUENCE` is non-transactional and reintroduces gaps under rollback. Comment this in the migration.

**`committed_at` vs `as_of` (B1).** **[v0.2]** These are two distinct time axes and must not be conflated:
- `as_of` = **valid-time** (when the fact is true in the world; can be backdated — evidence dated last year, recorded today). An input, frozen, never `now()`.
- `committed_at` = **transaction-time** (when we recorded the belief). Wall-clock, monotonic with `seq` within a subject by construction (set at append). **This is the recovery axis** (see §5 / B1). Never read by a fold's *logic*; only by the recovery *filter*.

`checkpoint_state` is a performance cache: bit-identical to replaying events `0..=checkpoint_seq` **under the `FoldRegistry` identified by `checkpoint_lexicon_manifest`** (§3.5 — version-dispatch makes the manifest pin load-bearing, not advisory); it must be invalidatable (drop it and re-fold) and is subject to the §3.6 audit.

---

## 3. The append protocol (the concurrency-hard core)

One verb invocation appends to **exactly one subject's stream** (rule, §3.3). The append is transactional and serializes per subject:

```
BEGIN;  -- the verb transaction (the existing TransactionScope)
  -- 1. Lock this subject's stream row. Serializes appends PER subject;
  --    different subjects proceed in parallel (no global lock — Q6).
  SELECT next_seq, checkpoint_seq, checkpoint_state, checkpoint_lexicon_manifest
    FROM kyc_subject_streams WHERE subject_root = $S FOR UPDATE;
    -- (INSERT the row on first event for a subject)

  -- 2. Fold the current state for precondition checks (replay from checkpoint).
  let state = fold(checkpoint_state, events_where seq > checkpoint_seq);   -- pure (substrate)

  -- 3. Validate preconditions against folded state (proof ratchet, reconcile, etc.).
  check_preconditions(verb, state)?;   -- e.g. verify needs a prior attach-evidence event (K-11)

  -- 4. Append the intent event AT next_seq. This IS the state change (K-35: no state w/o cause).
  INSERT INTO kyc_intent_events (subject_root, seq, committed_at, ...) VALUES ($S, next_seq, now(), ...);
  UPDATE kyc_subject_streams SET next_seq = next_seq + 1, updated_at = now() WHERE subject_root = $S;

  -- 5. Enqueue projection + external effects on the outbox (dispatched once, post-commit).
  INSERT INTO outbox (...) VALUES (...);   -- existing outbox, UNIQUE(idempotency_key, effect_kind)
COMMIT;
```

Properties this protocol gives:

- **Atomic event = state (K-16, K-35).** The event and any same-txn projection write commit together; there is no path to state without an originating event, and no event that fails to land its state. A rollback drops both.
- **Per-subject total order (Q6), parallel across subjects.** The `FOR UPDATE` row lock is the ordering domain; held only for the append. Hot-subject contention is the cost; cross-subject throughput is unaffected.
- **Deadlock-free by construction.** A verb touches one subject (§3.3), so a transaction holds at most one stream lock — no lock-ordering cycles. Cross-subject effects are modelled as **separate** events linked by `causation_id` (§3.4), never as a multi-subject atomic write.
- **Idempotent re-apply (F).** `UNIQUE(subject_root, idempotency_key)` rejects a duplicate append; a retried verb is a no-op, not a double-event.
- **Effects dispatch once (H5).** External effects (GLEIF/screening/`document.solicit`) go on the outbox at first apply and run post-commit; results are written back as `captured_effects` on the event. **Replay folds state only; it never re-enqueues.**

### 3.3 The single-subject rule

A verb transaction appends to exactly one `subject_root` stream. This is the invariant that makes the lock model deadlock-free and the ordering domain coherent. Determination of a fund that pulls in a GP is modelled as events on the fund's stream plus separate, causation-linked events on the GP's stream — not one cross-subject transaction. (This is also what keeps replay per-subject, per Q6.)

**[v0.2] Review confirmation.** Verified against the tree: control edges are per-determination-root, and board-controller derivation (`board_control_rules.rs:413`, `WHERE to_entity_id = $1`) reads direct edges to one entity, not a recursive cross-subject graph. The per-subject fold is coherent and **fund→GP is expressible** under this rule. The rule is accepted. **But** it converts a concurrency problem into a cross-stream-consistency problem, and v0.1 did not specify that contract. §3.4 is the missing piece and is **required before the protocol is frozen** (it is W4-blocking, and the protocol is being locked now).

### 3.4 Cross-stream emission contract [v0.2 — NEW; closes B2, B3, and the failure gap]

When a verb on subject `A`'s stream must affect subject `B` (the canonical case: `ubo.determination.freeze` on a fund's stream emits person-obligations onto persons' streams), it does **not** write `B`. It commits on `A`, enqueues an outbox effect, and the drainer appends a causation-linked event to `B` in a **separate** transaction following the §3 protocol for `B`. Three rules make this safe:

1. **Deterministic idempotency key (B3).** The emitted event's `idempotency_key` is **derived from causation**, not minted per delivery:
   `idempotency_key = hash(causing_event_id ‖ target_subject ‖ effect_kind)`.
   The outbox `UNIQUE(idempotency_key, effect_kind)` dedupes the *dispatch*; this rule dedupes the *resulting stream append* under the outbox's at-least-once delivery. Without it, a redelivered freeze-effect creates duplicate obligations on `B`.

2. **Retraction on re-determination (B2).** Re-determination is not additive. A re-`freeze` on `A` computes the set-difference of resolved subjects:
   - `emitted_now − emitted_before` → obligation-**create** effects (as today),
   - `emitted_before − emitted_now` → obligation-**supersede** effects (new),
   each causation-linked to the re-freeze event. The "emitted_before" set is itself a fold of `A`'s stream (the prior freeze's emissions are events), so the diff is pure and replayable. Without this, persons accumulate stale obligations from dead determinations — the determination⊥obligation back-channel the V&S §4 forbids, inverted.

3. **Emission failure is dead-lettered, never dropped (new).** A cross-stream append can fail its own precondition on `B` (e.g. `B`'s stream is in a state that rejects the obligation-create). The outbox already retries (at-least-once); a **permanent** failure (max-attempts) routes to a dead-letter with an alert, and the determination on `A` is flagged `emission_incomplete` — it does **not** silently succeed. A frozen determination whose emitted obligations did not all land is a known-bad state, surfaced, not hidden.

> **Eventual-consistency window (accepted).** Between the freeze commit on `A` and the obligation-create commit on `B`, the system holds "person is a UBO" without the person's obligation yet existing. For KYC this is acceptable — the obligation is *derived* and converges — provided rules 1–3 hold. The window is bounded by drainer latency, not unbounded.

### 3.5 Lexicon-version dispatch [v0.3 — H-d; D2 RATIFIED: version-dispatch now]

The substrate fold today matches on `verb_fqn.as_str()` (`fold/control.rs:277`), **not** on `lexicon_hash`. The store persists `lexicon_hash` per event (correct) but the fold ignores it. K-18/K-31 replay-faithfulness therefore holds **only while exactly one lexicon version exists** — and the migration makes old streams durable, so the day a second version ships, replaying an old stream would run the *new* fold semantics against it. **D2 closes this now rather than deferring** (the seam is cheap with one version, expensive to retrofit onto durable streams).

**Mandated design — content-addressed fold dispatch (in the substrate, pure):**

1. **Fold logic is content-addressed alongside the lexicon entry.** A verb's fold behaviour is registered under its `lexicon_hash`. The substrate gains a registry `FoldRegistry: lexicon_hash → FoldImpl` (a `BTreeMap`, per the determinism invariant — no `HashMap`).
2. **The fold dispatches per event on the event's stored `lexicon_hash`**, not on FQN:
   ```rust
   // substrate (pure). Each event folds under the EXACT impl it was written against.
   fn fold_control(events: &[&IntentEvent], reg: &FoldRegistry) -> ControlState {
       events.iter().fold(ControlState::default(), |st, e|
           reg.impl_for(&e.lexicon_hash)        // hard error if unregistered — never silently skip
              .apply_control(st, e))
   }
   ```
   An event whose `lexicon_hash` is not in the registry is a **hard error**, never a silent no-op — an unknown verb semantics cannot be guessed (K-35).
3. **Semantic change = new hash = new registered impl.** Changing a verb's fold behaviour registers a *new* `FoldImpl` under a *new* hash; the old impl stays registered forever so historical streams replay faithfully. The match-arms are never edited in place — that is the whole point.
4. **W1-proper ships one version, but the seam is live and tested.** A RED test registers two trivially-different fold impls under two hashes, appends one event under each, and asserts each event folds under its own impl (not the latest). This proves the seam works before a real second version exists.

`checkpoint_lexicon_manifest` (§2) records the manifest a checkpoint was folded under; a manifest change invalidates the checkpoint (drop and re-fold under the new registry). The substrate's existing `LexiconManifest` (DD-001 §3) already content-addresses entries — the `FoldRegistry` is the behavioural sibling keyed by the same hashes.

> **Crate impact [v0.3].** D2 is a change to `ob-poc-kyc-substrate` (the DD-001 crate): `fold_control`/`fold_obligations` gain the registry parameter and per-event dispatch. Still pure, still zero sqlx — the dep-gate is unaffected. The store (`ob-poc-kyc-store`) passes the registry through; it does not own fold logic.

### 3.6 The lock discipline and checkpoint are conventions — guard them [v0.2 — H-a, M-a]

- **Single append chokepoint (H-a).** §3 correctness depends on *every* writer to `kyc_intent_events[S]` first taking `FOR UPDATE kyc_subject_streams WHERE subject_root=S`. The schema cannot enforce this. **All** appends route through one method — `KycStore::append_in_scope(scope, event)` in `ob-poc-kyc-store` — and a source-scanning guard (dep-gate style) **fails the build on any `INSERT INTO kyc_intent_events` outside that method.** Shadow-writes (§6), backfills, and migrations are bound by the same rule.
- **Checkpoint write protocol (M-a).** `checkpoint_seq` / `checkpoint_state` / `checkpoint_lexicon_manifest` are written **under the same stream `FOR UPDATE` lock** (or atomically with their `seq`, proven monotonic), so the append-path read in step 2 can never see a `checkpoint_seq` inconsistent with `checkpoint_state`. Plus a **periodic audit** (background job + RED test): re-fold a subject from `seq 0` and assert equality to `fold(checkpoint_state, events > checkpoint_seq)`. A wrong checkpoint silently diverges every determination; "drop and re-fold" only helps if you *detect* it.

---

## 4. The seam wiring

Intercept at the existing choke point (verified: `dsl-runtime/src/execution.rs:48–64` already carries `principal`/`correlation_id`/`execution_id`; `sequencer_tx.rs`; `verb_executor_adapter.rs:151`; `SemOsVerbOp::execute` at `ops/mod.rs:995`). The change is to make the verb op, for cut KYC/UBO verbs:

1. **Surface `as_of` once** from `VerbExecutionContext` (frozen at verb entry — never `now()` inside the op).
2. **Resolve `actor`/`authority`** from `principal` (already present; persist them — today they are dropped).
3. **Call the substrate** to validate preconditions against the folded state.
4. **Call the store** (`ob-poc-kyc-store::append_in_scope`) to run the §3 append inside the same `TransactionScope`.
5. **Enqueue** projection + effect rows on the outbox (cross-stream effects carry the §3.4 derived key).

The op holds **no determination logic** — it orchestrates substrate (pure) + store (DB). This is the compiler-binds / runtime-executes split at the verb boundary.

---

## 5. Projections & recovery

- **Existing tables become projections.** `ubo_edges`, `cases`, `entity_workstreams`, `ubo_determination_runs`, and the **board-control stores** (post-cutover, §7) are written **only** by the outbox projection drainer, by folding the stream — never by direct verb writes (K-34). They are disposable and rebuildable.
- **Drainer reuses the outbox** (verified: `20260421_public_outbox.sql` **[v0.2 — corrected filename; v0.1 said `131_public_outbox.sql`]**, `UNIQUE(idempotency_key, effect_kind)`, `FOR UPDATE SKIP LOCKED`). Projection folds must be **idempotent and convergent** under at-least-once delivery and possible reordering (a fold of the full stream is both, by construction).
- **[v0.2 — H-c] Projection inputs include non-stream reference data.** The board-controller projection joins KYC-stream edges with **CBU-structural** data (`cbu_control_anchors`, read by `compute_for_cbu` to resolve the issuer entity). So "bit-identical rebuild" (exit criterion 5) is conditioned on those structural inputs being stable/versioned. State each projection's full input set in its drainer; a rebuild is reproducible *given the same reference inputs*, not in isolation.
- **Recovery / point-in-time (K-33) — recover by transaction-time [v0.2 — B1].** v0.1 defined recovery as `where as_of <= T`. Because `as_of` is **valid-time** and KYC backdating is legitimate, `as_of` is non-monotonic with `seq`, so `as_of <= T` returns a **non-prefix** of the stream → a holey fold → a garbage determination. K-33 ("as it stood at a past point") is **transaction-time** semantics. Recovery is therefore:
  ```
  recover_determination_at(subject, T) =
      fold( events where committed_at <= T, ordered by seq )   -- a true prefix
  ```
  `as_of` remains the valid-time annotation on each event (used for evidence validity windows, displayed in audit), **not** the recovery filter. `recover_determination_at`'s second parameter is renamed/retyped to a transaction-time bound to make this unambiguous at the call site.
  > **[v0.3 — D1 RATIFIED]** W1-proper ships **transaction-time recovery only.** The as-valid-at-T bitemporal query ("what was true in the world at T, as we believe now") is a **separate** query over valid-time windows, **deferred to the W6 read-model** — it does not overload `recover_determination_at` and is out of W1-proper scope.
- `rebuild_projection(subject)` folds the whole stream under the current manifest.

---

## 6. H6 — strangler cutover (per structure class)

The migration is **not** atomic across the domain; it cuts over **one structure class at a time**, and the rule for "which side is authoritative when stream and current-state disagree" is explicit:

1. **Shadow.** For the class, the verb appends the intent event **and** performs the existing direct write. The **old table is authoritative**; the stream shadows. No reads change. (Shadow-writes obey §3.6 — they route through the chokepoint.)
2. **Differential.** A projection folded from the stream is compared to the live table for the class until they are equal over a soak window (the same differential discipline as the slice's ec1, but against live data).
3. **Flip (atomic, per class).** Stop the direct write; the projection drainer becomes the **only** writer of those rows; the **stream is authoritative** from the flip instant. The class's flag flips in the reference plane.
4. **Rollback path.** If a post-flip divergence is detected, re-point reads to a rebuilt projection (a pure re-fold) — the stream is intact, so recovery is a re-fold, never data loss.

**Authority rule:** a table is authoritative **until** its class is flipped; the stream is authoritative the instant the class flips. Never both. The per-class flag is the single source of "who is authoritative right now."

---

## 7. H1 — bypass-path elimination (staged by risk)

Three authoritative-write paths bypass the verb stream and must route through it or be eliminated. Staged lowest-risk first; each gated on differential-equality before flip. **[v0.2] all three claims below were verified against the tree (§13-L).**

| Stage | Bypass path | Nature | Fix | Risk |
|---|---|---|---|---|
| **H1-a** | `set_bods_interest_type` **trigger** | **Verified pure derivation** (`CASE NEW.edge_type WHEN …`, master-schema.sql) | Move the derivation into the fold; the value becomes a **projection field**, not a trigger-written column. Delete the trigger. **[v0.2] Note: the function exists in BOTH `ob-poc` and `public` schemas — "delete the trigger" is two triggers.** | Low — pure function of the edge. |
| **H1-b** | `fn_compute_economic_exposure` **recursive CTE** (`migrations/031`) | Determination logic in SQL (K-32) | The economic computation belongs to `OwnershipProngStrategy` in the substrate. **[v0.2] Verified live callers exist** (`api/capital_routes.rs`, `ops/economic_exposure.rs`) — so **demote to a read-only projection helper, do NOT delete** (the API route reads it). It must never be an authoritative write. | Medium — read-only, but callers exist; demote not delete. |
| **H1-c** | board-control **DELETE-and-recompute** | **Destructive mutation of authoritative data** (K-34 ⚠, K-13 ⚠) | Board-control becomes a **fold output** (derived from control edges in the stream); the table becomes a **projection** rebuilt via the drainer, never DELETE-recomputed. Recompute logic moves into the substrate. | **High** — destructive on authoritative data; readers expect it populated. Do last, full shadow→differential→flip. |

**[v0.3 — H-b; D3 RATIFIED: overrides become verbs] H1-c surface is larger than v0.1 stated. Inventory, then convert.** The tree has **three** board-control stores, not one:
- `cbu_board_controller` (`board_control_rules.rs:511`) — the derived table v0.1 named;
- `board_controller_cache` (`ops/control/board.rs:413`) — a second derived store;
- `board_controller_overrides` (`ops/control/board.rs:494`) — **human-authored, not derived.**

A derived projection cannot silently absorb human overrides — so per **D3 (*all changes are verbs, no side effects*, K-15/K-32/K-35)** the human-authored override table is **eliminated**: a board-control override becomes a verb, `ubo.board-controller.override` (declaring `authority` + a recorded `basis`, supersede-never-delete like every other edge), appended to the stream and **folded** into the board-control projection deterministically (latest non-superseded override wins; the fold composes derived-controller + override-event in one pass). The two derived stores (`cbu_board_controller`, `board_controller_cache`) become projection rows written only by the drainer. After conversion there is **no** non-stream authoritative input to board control — the override is an event, not a side-effecting table write. The H1-c inventory step confirms all three stores are accounted for (two → projections, one → folded verb events) before the flip.

**[v0.2 — M-d] H1-c differential needs a quiescence condition.** A DELETE-recompute table is not append-maintained, so comparing a stream-fold to it under timing skew yields false diffs → either never-converges (false negative) or flips on a false convergence. The §6 differential for H1-c compares **only at quiescence**: no pending recompute trigger fired and unsettled, no pending fold in the drainer. This is the one place the slice's ec1-style differential does not transfer cleanly.

H1-c is the dangerous one and is sequenced **last**, after the shadow/differential machinery (§6) is proven on the safer paths. Until all three route through the stream, **K-15 is unmet** — so "W1-proper done" requires all three flipped for the cut class.

**[v0.2 — M-b] G2 resolved: `ubo.delete-relationship` IS a hard DELETE.** Verified (`ubo.yaml:493`: "Hard delete any entity relationship by ID", `behavior: crud`). v0.1's conditional resolves to **YES** → it is the same K-13/K-34 family and is converted to a **supersede** event in the H1-c stage. Caveat: it is a *generic* relationship delete (not UBO-scoped), so confirm no non-KYC caller depends on hard-delete semantics before converting.

---

## 8. Exit criteria (W1-proper is done when…)

For the private-company class, durably, under concurrency:

1. **Atomic append.** A verb either appends its intent event and its projection effect together or neither; a forced rollback mid-verb leaves no orphan state (K-16, K-35).
2. **Per-subject order under concurrency.** N concurrent appends to one subject produce a dense gap-free `0..N-1` sequence; appends to distinct subjects proceed in parallel (Q6). *(Concurrency test, under load.)*
3. **Idempotent re-apply.** Re-issuing a verb with the same `idempotency_key` is a no-op, not a second event (F).
4. **Durable point-in-time recovery [v0.2/B1].** `recover_determination_at(subject, T)` returns the determination bit-identically as it stood **at transaction-time T**, from the durable store, pinned to lexicon-manifest + graph-hash (K-33, K-18) — folding a true `committed_at <= T` prefix.
5. **Projections are derived.** Every row in the cut-class projection tables is produced **only** by folding the stream (given the §5 reference inputs); a full rebuild reproduces them bit-identically (K-34).
6. **Effects dispatch once.** Replaying a subject's stream re-folds state and re-dispatches **no** external effect (H5).
7. **H1 closed for the class.** None of the three bypass paths writes authoritative state for the cut class; K-15 holds (grep + integration proof).
8. **Cutover safety.** The differential held over the soak window before flip; a forced post-flip divergence recovers by re-fold with no data loss (H6).
9. **[v0.2/B2/B3] Cross-stream emission is exactly-once and retracting.** A re-determination that drops a previously-resolved person emits an obligation-supersede on that person's stream; redelivery of any cross-stream effect produces no duplicate (derived idempotency key); a permanently-failing emission dead-letters and flags the determination `emission_incomplete`, never silently succeeds.
10. **[v0.2/H-a/M-a] Discipline guards hold.** No `INSERT INTO kyc_intent_events` exists outside the single append chokepoint (source-scan guard); the checkpoint audit (re-fold from 0) matches the incremental fold.
11. **[v0.3/H-d/D2] Fold version-dispatch is live.** The fold dispatches per event on `lexicon_hash` via the `FoldRegistry`; an unregistered hash is a hard error; two impls registered under two hashes each fold their own events (seam proven before a real second version exists).

---

## 9. RED tests to add

External, public-API, RED-first (fail against current `main`). In addition to the durable analogues of the slice's seven:

- **Concurrency:** spawn N tasks appending to one subject → assert dense contiguous seq, no duplicates, no lost updates.
- **Atomicity:** inject a failure between event-insert and outbox-enqueue → assert full rollback, no orphan.
- **Idempotency:** double-submit same key → one event.
- **Rebuild equality:** truncate a projection, rebuild from the stream → bit-identical to pre-truncate.
- **Replay-no-redispatch:** count outbox effect enqueues across a fold vs a replay → first >0, replay == 0.
- **Bypass guard:** integration test asserting no authoritative write to cut-class tables occurs outside the projection drainer (K-15/K-34).
- **Cutover differential:** shadow-write a fixture, fold a projection, assert equal to the live direct-write before authorising flip. **[v0.2]** for H1-c, assert equality **at quiescence** (M-d).
- **[v0.2/B1] Backdated recovery:** append events with non-monotonic `as_of` (a backdated evidence event after a later-valid one); assert `recover_determination_at(subject, T)` folds a `committed_at`-prefix and is unaffected by `as_of` ordering. *(Fails today if recovery filters on `as_of`.)*
- **[v0.2/B2] Retraction:** freeze resolving {P1,P2}; supersede P2's chain; re-freeze → assert an obligation-supersede event lands on P2's stream and P2's obligation is terminal-superseded, while P1's persists.
- **[v0.2/B3] Cross-stream dedupe:** redeliver a freeze-emitted effect twice → assert one obligation-create event on the target stream.
- **[v0.2/H-a] Chokepoint guard:** source-scan asserts zero `INSERT INTO kyc_intent_events` outside `KycStore::append_in_scope`.
- **[v0.2/M-a] Checkpoint audit:** corrupt a `checkpoint_state`, run the audit → assert divergence detected.
- **[v0.3/H-d/D2] Fold version-dispatch:** register two trivially-different fold impls under two hashes; append one event under each; assert each folds under its own impl (not the latest), and that an event with an unregistered `lexicon_hash` is a hard error, not a skip.

---

## 10. What W1-proper does NOT do (deferred)

The other ten structure classes (W4, gated on golden fixtures — Addendum A §3.2); screening/risk **execution** (obligations + dispositions recorded, engine external — V&S §9.2); the full obligation graph beyond what the private-company slice emits; UI read-model generalisation (W6 beyond the one drainer); investor-onboarding routing execution; **[v0.3 — D1]** the as-valid-at-T bitemporal query (W6 read-model). *(Version-dispatch is NOT deferred — D2 builds the fold-registry seam in W1-proper, §3.5.)*

---

## 11. Hand-off note for the build agent

- **Model:** Opus. The per-subject append protocol (§3), the single-subject deadlock-freedom rule (§3.3), the **cross-stream emission contract (§3.4)**, the **fold version-dispatch seam (§3.5/D2)**, the **transaction-time recovery (§5/B1)**, and the H1-c destructive cutover (§7) are the irreversible decisions — a wrong move corrupts authoritative state. This is concurrency + transactional + live-data surgery.
- **Crates touched:** `ob-poc-kyc-store` (NEW membrane, all sqlx); **`ob-poc-kyc-substrate` (the DD-001 crate — D2 adds the `FoldRegistry` + per-event dispatch to `fold_control`/`fold_obligations`; stays pure)**; `sem_os_postgres`/`dsl-runtime` adapters (seam). The dep-gate must stay green on the substrate throughout.
- **Order:** **fold-registry version-dispatch in the substrate (§3.5/D2) + its seam test** → store crate + migration (§2) → append protocol + concurrency tests (§3) → **chokepoint guard + checkpoint audit (§3.6)** → seam wiring (§4) → projections + transaction-time recovery (§5) → **cross-stream emission contract + its RED tests (§3.4)** → shadow/differential machinery (§6) → H1-a → H1-b → **board-control inventory + `ubo.board-controller.override` verb conversion (§7 H-b/D3) →** H1-c. Do **not** start H1-c until §6 is proven on H1-a/b **and** the override→verb conversion has landed (no human-authored board-control table remains).
- **Invariants that must stay structural (carried from the slice):** no "set status" verb (status is a fold output); no `now()` inside a verb (`as_of` is an input); nothing nondeterministic in a fold (no random, no `HashMap` iteration into a hash). The store must not leak these — the durable impl reads `as_of` from the event, never the clock, and recovery reads `committed_at`, never `now()`.
- **The single most dangerous temptation:** writing authoritative state directly "just for now" during cutover instead of through the shadow→flip mechanic. That re-introduces exactly the K-34 violation being removed. Every cut-class authoritative write goes through the stream or the drainer — never direct. The §3.6 chokepoint guard exists to make this temptation fail the build.

---

## 12. Owner decisions (ratified — gate cleared)

| # | Decision | Ruling [v0.3] | Lands in |
|---|---|---|---|
| **D1 (B1)** | Recovery axis: transaction-time only, or also bitemporal as-valid-at-T? | **Transaction-time only.** As-valid-at-T deferred to W6 read-model. | §2, §5, exit 4. |
| **D2 (H-d/§3.5)** | Lexicon: freeze-and-guard, or version-dispatch now? | **Version-dispatch now** — *don't kick the issue down the road.* Content-addressed `FoldRegistry`, per-event dispatch, seam tested with two impls. Touches the substrate crate. | §3.5, §2 (checkpoint manifest), exit 11, RED. |
| **D3 (H-b/§7)** | Board-control overrides: verb events, or a separate authoritative input? | **Verb events** — *all changes are verbs, no side effects* (K-15/K-32/K-35). The human-authored `board_controller_overrides` table is eliminated; override = `ubo.board-controller.override`, folded. | §7 H1-c. |
| **§3.4** | Cross-stream emission contract (retraction, derived idempotency, dead-letter). | **Adopted as written.** | §3.4, exit 9, RED. |

All four are closed. This document **authorises the W1-proper build** per the §11 order.

---

## 13. Change log

| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-06-30 | W1-proper design. Membrane crate `ob-poc-kyc-store`; first migration (`kyc_intent_events` + `kyc_subject_streams`); transactional per-subject append protocol (FOR UPDATE ordering, single-subject deadlock-freedom, idempotency, captured-effects/replay-no-redispatch); seam wiring; projections-as-folds + point-in-time recovery; H6 per-class shadow→differential→flip with the explicit authority rule; staged H1-a/b/c bypass elimination. First doc to authorise schema and modify authoritative writes. |
| 0.2 | 2026-06-30 | **End-of-design review folded in.** Spine (§3) and authority rule (§6) accepted unchanged. **Blockers:** B1 — recovery is transaction-time (`committed_at`-prefix), not `as_of` (valid-time), which backdating makes a holey non-prefix (§2, §5, exit 4, RED). B2 — `freeze` must emit obligation-**supersede** for dropped persons on re-determination (§3.4, exit 9, RED). B3 — cross-stream emitted events use causation-derived idempotency keys (§3.4, exit 9, RED). **New §3.4 cross-stream emission contract** (idempotency + retraction + dead-letter failure semantics) — the piece the single-subject rule (§3.3) owed. **High:** H-a — single append chokepoint + source-scan guard (§3.6, exit 10); H-b — three board-control stores incl. human overrides, inventory + decision before H1-c (§7, D3); H-c — projections take non-stream reference inputs (§5); H-d — fold doesn't dispatch on `lexicon_hash`, freeze-and-guard or version-dispatch (§3.5, D2). **Medium:** M-a — checkpoint written under stream lock + re-fold audit (§3.6); M-b — G2 confirmed `delete-relationship` is a hard DELETE → supersede (§7); M-c — economic CTE has live callers, demote not delete (§7); M-d — H1-c differential needs a quiescence condition (§7). **`seq`-from-row (not SEQUENCE) invariant** made explicit (§2). **§12 owner-decision table** (D1–D3) gates build. **§13-L verifications** below. |
| 0.3 | 2026-06-30 | **Owner decisions ratified; authorises build.** **D1** — transaction-time recovery only; as-valid-at-T deferred to W6 (§5, §10). **D2** — version-dispatch the fold now (not freeze-and-guard): §3.5 rewritten to mandate a content-addressed `FoldRegistry` with per-event `lexicon_hash` dispatch, hard-error on unregistered hash, two-impl seam test; checkpoint manifest pin made load-bearing (§2); **reaches into the substrate crate** (`fold_control`/`fold_obligations` gain the registry, stay pure) — reflected in §11 crate-touch list and build order (version-dispatch goes first). **D3** — board-control overrides become the verb `ubo.board-controller.override` (folded, supersede-never-delete); the human-authored `board_controller_overrides` table is eliminated, no non-stream authoritative input to board control remains (§7). §12 converted from open-decisions to ratified record. exit 11 + the lexicon RED test rewritten from "guard" to "dispatch". No change to the spine, the cross-stream contract, or the §13-L verifications. |

### 13-L. Empirical verification of v0.1 "verified:" claims

| Claim | Result |
|---|---|
| Outbox `UNIQUE(idempotency_key, effect_kind)`, `effect_kind`, `FOR UPDATE SKIP LOCKED` | ✅ confirmed (`20260421_public_outbox.sql:24,35`). **Filename corrected** — v0.1 cited `131_public_outbox.sql`. |
| `set_bods_interest_type` is a pure derivation | ✅ confirmed (`CASE NEW.edge_type …`, master-schema.sql). **In two schemas** (`ob-poc` + `public`). |
| `fn_compute_economic_exposure` callers to verify | ✅ callers exist (`api/capital_routes.rs`, `ops/economic_exposure.rs`) → demote, not delete. |
| `ubo.delete-relationship` hard DELETE (G2) | ✅ confirmed (`ubo.yaml:493`, "Hard delete …", `behavior: crud`) → supersede in H1-c. |
| board-controller derivation scope | ✅ single-entity (`WHERE to_entity_id = $1`), single-subject-foldable — but **three stores** exist incl. human `board_controller_overrides`, and it reads CBU-structural `cbu_control_anchors`. |
| substrate fold dispatches on lexicon_hash | ❌ matches `verb_fqn` only (`fold/control.rs:277`) → H-d. |
| seam carries `principal`/`correlation_id`/`execution_id` | ✅ confirmed (`dsl-runtime/src/execution.rs:48–64`). |
