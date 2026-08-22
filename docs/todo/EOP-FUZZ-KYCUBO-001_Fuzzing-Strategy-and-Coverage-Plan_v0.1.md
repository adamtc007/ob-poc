# EOP-FUZZ-KYCUBO-001 — Fuzzing strategy & coverage plan v0.1 (FOR REVIEW)

Status: **DRAFT, not ratified.** Scoping only — no fuzz code, no xtask changes,
no CI changes have been made. Modeled directly on the sibling bpmn-lite repo's
`EOP-FUZZ-BPMN-ISA-002` (RATIFIED 2026-07-25, `~/dev/bpmn-lite/docs/todo/`),
which is the established, working convention in this codebase family — same
oracle-driven methodology, same per-crate `fuzz/` layout, same `cargo x fuzz`
xtask shape. Not reinventing the approach, applying it to a different target.

## 1. Ground truth (surveyed 2026-08-17)

- **Zero fuzz infrastructure anywhere in ob-poc.** No `fuzz/` directories, no
  `cargo-fuzz` project. `proptest = "1.4"` is declared in
  `rust/Cargo.toml:196` but is not used by any KYC crate (or, on this survey,
  by anything else) — the tests named `sexpr_roundtrip_property_*` in
  `ob-poc-kyc-substrate/tests/render.rs` are ordinary fixed-example `#[test]`
  functions, not `proptest!` generative tests. `cargo-fuzz 0.13.2` is
  installed locally.
- **`ob-poc-kyc-substrate` is the ideal fuzz substrate** — same shape as
  bpmn-lite-kernel: no sqlx, no DB, no async (confirmed via its own
  `Cargo.toml` header comment: "No sqlx. No sem_os_core... Pure semantic
  engine"). Every fold/determination/render/preview/placement function in it
  is synchronous and pure.
- **Compare bpmn-lite's baseline:** at the time its fuzz plan was ratified,
  bpmn-lite had one existing fuzz crate (2 targets, zero CI wiring, zero
  recorded crashes) and grew that to 12 crates / 35 targets, nightly CI, and
  a committed regression-corpus gate over ~1 month, finding one real bug
  (`F2-KERNEL-001`, an un-cancellable-mid-fork instance) on the very first
  live run of the flagship kernel target. ob-poc/KYC-UBO is starting from
  strictly less than bpmn-lite's own baseline (zero vs. one crate).

## 2. Thesis — what fuzzing buys this system

The dsl.kyc write path's own stated invariant (`fold/control.rs` doc
comment) is: **the fold is a pure, deterministic function of the event
stream — no `HashMap` iteration order, no `Uuid::new_v4()`, no
`Utc::now()` inside a fold.** That is exactly the kind of statement
coverage-guided fuzzing is good at falsifying mechanically rather than by
code review. Determinism here isn't a nice-to-have: `ubo.determination.freeze`
is the actual UBO/control finding a KYC case is approved against (K-23), so
"replay the same event history twice and get two different determinations"
is a correctness-of-record bug, not a cosmetic one.

So, mirroring bpmn-lite's framing: the flagship targets are not "throw bytes
at a JSON parser" (useful but shallow) — they are **property-oracle fuzzers
over the fold/determination pipeline**, generating structurally-plausible
event sequences (not raw byte garbage) and asserting the theorems the doc
comments already claim, not just "no crash."

## 3. Oracles

| Oracle | Statement checked | Source of truth |
|---|---|---|
| O1 No-panic | any event/payload/verb_fqn sequence fed to `fold_control`/`fold_obligations`/`check_preconditions`/`DeterminationStrategy::resolve`/render/parse never panics — always `Result` or a total function | fold/control.rs, fold/obligation.rs doc invariants |
| O2 Fold determinism | folding the same `Vec<IntentEvent>` twice (or replaying via `FoldRegistry`) produces bit-identical state | fold/control.rs:13-18 doc invariant (no HashMap/RNG/clock in a fold) |
| O3 Decode/encode roundtrip | `parse(render(event))` reproduces `event.target` + `event.payload` | render.rs + `crates/dsl-parser`; today only asserted over 5 hand-picked fixtures in `tests/render.rs` |
| O4 Cycle/resource safety | `DeterminationStrategy::resolve` over an adversarially-cyclic `ControlState` (e.g. `A controls B controls A`) terminates without panicking or overflowing the stack | determination.rs:131, :212 — DFS cycle guards exist; fuzzing proves they hold rather than inspecting them |
| O5 Normalizer fail-open/fail-closed consistency | `EdgeKind`'s two independent classifiers (`fold/control.rs::edge_kind_from_payload`, fail-open historical-replay backstop, vs. `kyc_stream_ops.rs::normalize_assert_control_payload`, fail-closed pre-append gate) never let an unrecognized `kind` string reach the stream un-rejected through the op path | fold/control.rs:267-298, kyc_stream_ops.rs:664 |
| O6 Cross-module invariant never fires | the `smo_person_id`/`smo_event_id` pairing is checked identically in three independent places (`fold/control.rs:489-492`, `determination.rs:798-801` — a live `panic!`, `kyc_stream_ops.rs:852` — a live `Err`); fuzzing arbitrary `apply-smo-fallback` payloads should never trip any of the three | see §5 finding below |
| O7 Registry totality | `FoldRegistry::get` never silently falls back for a near-colliding-but-wrong `lexicon_hash` | fold/registry.rs:97 |

## 4. Target inventory, ranked

Priorities mirror bpmn-lite's P0 (flagship) → P3 (explicit non-goal).
Everything below cites real file:line locations verified during scoping
(one correction to the initial survey noted at the bottom of this section).

### P0 — fold/determination flagship (`ob-poc-kyc-substrate/fuzz/`)

| Target | Input shape | Oracles |
|---|---|---|
| `fold_replay` | byte tape → structured generator picks from the 21 known `verb_fqn`s + emits mostly-plausible-but-sometimes-garbage `payload` JSON per verb (tuned admit rate, same "F-A" lesson as bpmn-lite — naive `Arbitrary` JSON rarely reaches interesting fold branches) → `fold_control` + `fold_obligations` in sequence → `check_preconditions` against the next candidate | O1 O2 O6 |
| `determination_resolve` | `fold_replay`'s generator, seeded to include adversarial cycles/self-loops in the resulting `ControlState` → all 8 `DeterminationStrategy` impls (`OwnershipProngStrategy`, `ControlProngStrategy`, `TrustRoleStrategy`, `FundControlStrategy`, `FoundationCouncilStrategy`, `StateOwnedStrategy`, `CooperativeMemberStrategy`, `NomineePierceStrategy`) | O1 O4 |
| `sexpr_roundtrip` | arbitrary `IntentEvent` (reuse the `fold_replay` generator) → `render_intent_event_to_sexpr` → `dsl_parser::parse` → compare | O1 O3 |

### P0 — decoders (`ob-poc-kyc-substrate/fuzz/`)

| Target | Input shape | Oracles |
|---|---|---|
| `edge_kind_decode` | arbitrary string → both `edge_kind_from_payload` (fold-side) and the `EDGE_KIND_WIRE_VALUES` membership check (op-side logic, reachable without pulling in the full `ob-poc` crate — see layout note below) | O1 O5 |
| `structure_class_decode` | arbitrary string → `structure_class_from_payload` | O1 O5 (also surfaces the finding below: there is currently no op-side fail-closed gate for `structure-class` at all, unlike `EdgeKind`) |
| `hash_from_hex` | arbitrary bytes → `Hash::from_hex` | O1 |
| `map_principal` | arbitrary string → `Principal` via `Uuid::new_v5` fallback | O1 (documented determinism: same string always maps to same UUID) |

**Layout note:** `kyc_stream_ops.rs`'s `normalize_assert_control_payload`/
`normalize_pierce_nominee_payload` live in the root `ob-poc` crate (heavy
dependency graph — pulling that in as a fuzz-crate dependency is expensive
to build and slows iteration). Since both op-side functions ultimately just
match against the same `EDGE_KIND_WIRE_VALUES` table `ob-poc-kyc-substrate`
already exports, `edge_kind_decode` gets full coverage of the real
classification logic without that dependency; only revisit if op-layer
fuzzing (P2 below) happens anyway.

### P1

| Target | Input shape | Oracles |
|---|---|---|
| `preview_apply` | two arbitrary `&[IntentEvent]` slices (committed + speculative candidates) → `preview()` | O1 |
| `fold_registry` | arbitrary 32-byte `lexicon_hash` values near a registered hash → `FoldRegistry::get` | O1 O7 |

### P2 — deferred until P0/P1 prove out

- **Op-layer fuzzing proper** (`kyc_stream_ops.rs`'s 21 `SemOsVerbOp::execute`
  impls): real value (see the `unwrap_or_else(nil)` finding below, found by
  reading, not running) but needs either extracting the pure
  arg-validation logic out of each op into standalone functions callable
  without a `VerbExecutionContext`/`TransactionScope`, or a heavier
  async/DB-backed harness. Same shape as bpmn-lite's P2 engine-command-tier
  deferral (async ⇒ ~2 orders of magnitude fewer execs/s).
- **DB-row decode components**: `Hash::from_hex` is already P0 above;
  `row_to_event`'s three `serde_json::from_value::<Principal/TargetBinding/
  Vec<CapturedEffect>>` calls are the real decoder surface and are pure and
  fuzzable directly against arbitrary `serde_json::Value` without needing a
  real `PgRow` — worth adding once P0 lands, cheap.

### P3 — out of scope, with reasons

- `PgKycEventStore::append`'s §3 protocol (idempotency key, `FOR UPDATE`
  seq allocation) — transaction/DB-bound, not libFuzzer-shaped. If this
  needs adversarial-input testing, a property/model-based integration test
  (proptest driving concurrent/adversarial `idempotency_key` collisions
  against a real Postgres, not cargo-fuzz) is the right instrument — same
  role bpmn-lite's `nightly-chaos.yml` plays for `bpmn-lite-store-postgres`.
- `cross_stream.rs`'s outbox read/write paths — DB-bound, same reasoning.

## 5. Findings from scoping (not from running anything — worth acting on regardless of whether fuzzing gets built)

Two real issues surfaced by reading the code while scoping targets. Neither
is fixed here — flagging for your call, consistent with not silently
expanding this turn's scope:

1. **Five (not three) obligation-track ops silently fold a malformed
   `subject-id` onto the nil UUID instead of rejecting it.**
   `kyc_stream_ops.rs:1141,1176,1211,1246,1281` —
   `KycObligationUpdateIdentity`, `UpdateScreening`, `UpdateRisk`,
   `Satisfy`, and `Waive` all do
   `json_extract_uuid(args, ctx, "subject-id").unwrap_or_else(|_| Uuid::nil())`
   rather than propagating the extraction error. A malformed `subject-id`
   from a live session gets silently appended to the nil-UUID subject's
   stream instead of being rejected at the op boundary — every other op in
   this file (and every other UUID field) uses `?`/`Err(anyhow!(...))`
   instead. This is a real, fixable inconsistency, independent of fuzzing —
   found by inspection, verified directly (not taking the survey's "three"
   count on faith turned up two more).
2. **`structure-class` has no fail-closed gate at the op layer, unlike
   `kind`.** `EdgeKind` is validated against `EDGE_KIND_WIRE_VALUES` before
   append (`kyc_stream_ops.rs:664`); `structure_class_from_payload`
   (`fold/control.rs:302`) is fail-open — an unrecognized string silently
   folds to `structure_class: None`, caught only later by the weaker
   `Precondition::StructureClassSupported` gate. Either this asymmetry is
   intentional (worth a one-line doc comment saying so, given
   `valid_values` is supposed to be "mandatory, always the wire string" per
   this repo's own authoring rule) or it's a gap matching the same defect
   class T0.3/K-G7 already fixed twice elsewhere in this stack.

## 6. Harness: `cargo x fuzz`

Directly replicate bpmn-lite's `xtask/src/fuzz.rs` (744 lines, already
proven in production over there — no need to redesign):

- `cargo x fuzz list` — enumerate targets across per-crate `fuzz/` dirs.
- `cargo x fuzz run [--target T] [--time SECS]` — build via
  `cargo +nightly fuzz` (ob-poc pins stable `1.96` at the repo root
  `rust-toolchain.toml`, same shape as bpmn-lite's stable `1.95` pin —
  fuzzing always needs an explicit nightly override); evolved corpus
  persists under the crate's git-ignored `fuzz/corpus/`.
- `cargo x fuzz smoke` — CI/pre-push mode: build all targets, run each
  briefly, run the regression corpus, exit non-zero on any crash.
- `cargo x fuzz regress` — run every target over its **committed**
  `fuzz/regressions/` inputs only (seconds, deterministic) — every crash
  found gets minimized (`cargo fuzz tmin`) and committed alongside its fix,
  so the gate runs forever on a stable time budget.
- `cargo x fuzz seed` / `clean` — corpus/disk hygiene.
- Fail-closed toolchain guard: missing nightly or `cargo-fuzz` binary is a
  hard error with install instructions, never a silent skip.
- Results capture: `fuzz-results/<UTC-stamp>/summary.md` + per-target JSONL
  (execs, execs/s, coverage edges, corpus count, crash paths).
  `fuzz-results/` gitignored.

Layout: per-crate `fuzz/` sub-crate (`ob-poc-kyc-substrate/fuzz/` first,
mirroring `bpmn-lite-kernel/fuzz/`), isolated `[workspace]`, xtask as the
unifier — matches this repo's own existing `crates/<name>/fuzz/`-style
convention (none exist yet, but it's bpmn-lite's established pattern and
there's no reason to diverge).

## 7. CI

ob-poc doesn't have a direct analog to bpmn-lite's `production-gates.yml`,
but `invariants.yml` and `control-plane-proofs.yml` (`.github/workflows/`)
play the same "always-on blocking gate" role. Proposed:

- New `nightly-fuzz.yml` — nightly job, time-boxed per P0 target, corpus
  cached across nights, crash artifacts uploaded, red on any finding.
- `cargo x fuzz regress` wired into `invariants.yml` (deterministic,
  seconds) so every previously-found crash becomes a permanent blocking
  gate, same role as bpmn-lite's `fuzz-regressions` job.

## 8. Open decisions before implementation (your call)

- **F-A Input generation:** structured byte-tape generators per target
  (recommended — bpmn-lite tried naive `Arbitrary`-on-payload first and it
  yielded near-0% interesting-branch coverage) vs. `arbitrary` derive
  directly on `IntentEvent`/`ControlState` (less code, likely shallow
  coverage for the same reason).
- **F-B Op-layer scope (P2):** defer op-layer fuzzing to a second phase
  (recommended, matches bpmn-lite's own P2 deferral for its async engine
  tier) vs. pulling it into phase 1 now that the `unwrap_or_else(nil)`
  finding shows real value there.
- **F-C The two findings in §5:** fix now (small, independent of fuzzing
  infra existing at all) vs. bundle into whichever phase touches that file.
- **F-D CI placement:** separate `nightly-fuzz.yml` (recommended, matches
  bpmn-lite) vs. folding into an existing workflow.

## 9. Phasing

1. **F1 Plumbing:** `cargo x fuzz` subcommands ported from bpmn-lite's
   `xtask/src/fuzz.rs`, nightly-toolchain guard, results capture. Seed with
   the three cheapest pure decoders (`edge_kind_decode`,
   `structure_class_decode`, `hash_from_hex`) as the harness's own
   red→green receipt (bpmn-lite's convention: a fuzzer that's never seen
   red proves nothing — plant a known-fixed bug class temporarily, confirm
   the target finds it within the smoke budget).
2. **F2 Fold flagship:** `fold_replay` generator + O1/O2/O6.
3. **F3 Determination + roundtrip:** `determination_resolve` (O4),
   `sexpr_roundtrip` (O3).
4. **F4 CI:** `nightly-fuzz.yml` + regress gate in `invariants.yml`.
5. **F5 (deferred, separate sign-off):** P2 op-layer targets.
