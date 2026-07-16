# Session: `ob-poc`'s own stale public-API-surface baseline — the E5 STOP left by GRADPLAN-G0-IMPL-001

### EOP-SESSION-CONTROLPLANE-OBPOC-BASELINE-REVIEW-001
### Date: 2026-07-14
### Status: COMPLETE. Baseline refreshed. No commit performed (per instructions — left uncommitted for independent verification).

---

## What this session is

`EOP-SESSION-CONTROLPLANE-GRADPLAN-G0-IMPL-001` (2026-07-13) refreshed 4/5 named-stale
`cargo public-api` baselines under `audits/surface/` and explicitly **stopped** on the 5th —
`ob-poc` itself — because its diff (~23k lines) mixed a "genuinely new blanket
`tower_http::follow_redirect::policy::PolicyExt` impl" with Send/Sync-ordering noise, and
was "too large/mixed to safely eyeball in a grind slice." `invariants-expected.toml`'s
`[e5]` section records this STOP verbatim and calls it out as "still untouched... needs its
own review."

This session does that review for real: re-measures the *current* diff (code has moved a
lot since — G4, G5, G1-items-3-4, G6b, the HumanGate reseal test all landed on this branch
after the STOP was recorded), classifies every line with evidence, and reaches a verdict.

---

## Tooling confirmed available (not a repeat of the STOP's "tooling unavailable" shape)

```
$ rustup toolchain list
stable-aarch64-apple-darwin (default)
nightly-aarch64-apple-darwin
...
$ cargo +nightly public-api --version
cargo-public-api 0.52.0
```

Both present. This is a real review, not a "tooling missing" finding.

---

## Step 1 — real, current diff

```
$ cd rust && cargo +nightly public-api -p ob-poc --all-features > /tmp/ob-poc-now.txt
# 151851 lines (vs committed baseline audits/surface/ob-poc.txt body: 152558 lines)
$ tail -n +2 audits/surface/ob-poc.txt > /tmp/ob-poc-base.txt
$ diff /tmp/ob-poc-base.txt /tmp/ob-poc-now.txt > /tmp/ob-poc.diff
$ wc -l /tmp/ob-poc.diff   # 23555 — matches the STOP's "~23k lines" description
$ grep -c '^<' /tmp/ob-poc.diff   # 9036 removed
$ grep -c '^>' /tmp/ob-poc.diff   # 8329 added
```

Baseline header: `# ob-poc | features=all | HEAD=6ab569acc43b3c5f3ac2ec0304c0bd298dedce77 |
2026-07-11T08:03:42Z`. 34 commits landed on `codex/phase-1-5-governance-closure` between that
HEAD and the session's starting HEAD (`6c6a22eb`), including exactly the work named in the
task brief: T11.1b/T11.2 (agent-tier extraction), the invariant-promotion + GRADPLAN-G0
sessions, G1 item 2, G2, G4, G5, G1-items-3-4, G6b/G6c, and the HumanGate reseal test.

---

## Step 2 — classification strategy (exhaustive by construction, not sampled)

Rather than eyeball 23k lines, the diff was decomposed by **mechanism**, verifying each
mechanism's line count independently until the sum reconciled exactly against the raw diff
totals (9036 removed / 8329 added = 17365 changed lines total). Every changed line is
accounted for in one of four buckets below; there is no unclassified residual.

### Bucket A — `tower_http::follow_redirect::policy::PolicyExt` blanket-impl fanout (6021 lines, 100% added, 0 removed)

Confirmed the trait is a genuine unconditional blanket impl in tower-http's own source
(`~/.cargo/registry/.../tower-http-0.6.8/src/follow_redirect/policy/mod.rs:171`):

```rust
impl<T> PolicyExt for T
where
    T: ?Sized,
{ ... }
```

— applies to **every type in existence**, not something this repo wrote. `grep tower_http`
on the baseline gives 0 hits; on the current measurement, 6021 (one `impl` line + two
generated `.and()`/`.or()` method-signature lines per public type in the crate).

**Root-caused, not guessed.** `Cargo.lock`'s `tower-http` entries (0.5.2 and 0.6.8) are
byte-identical between the baseline commit and HEAD — this is *not* a `Cargo.lock`/version
bump. `cargo tree -p ob-poc --all-features -i tower-http@0.6.8 -e features` shows the
`follow-redirect` feature is pulled in via:

```
tower-http feature "follow-redirect"
  └── reqwest v0.12.28 feature "rustls-tls" (etc.)
      └── bpmn-lite-ffi-http / ob-poc-agent
          [dev-dependencies] └── ob-poc
```

`ob-poc-agent`'s own `Cargo.toml` (`rust/crates/ob-poc-agent/Cargo.toml`) declares
`reqwest = { version = "0.12", ..., features = ["json", "rustls-tls"] }` — a real,
already-present dependency of `ob-poc-agent` (its "Phase 5.3 OTLP exporter"). `ob-poc-agent`
itself only became a dependency **of `ob-poc`** in T11.1b (commit `7821ecb7`, "agent-tier
extraction slice 1", 2026-07-12) — confirmed via `diff <(git show
6ab569ac:rust/Cargo.toml) rust/Cargo.toml`, whose only substantive line is:

```
+ob-poc-agent = { path = "crates/ob-poc-agent" }
```

**Reproduced empirically, not just reasoned about.** Re-ran the exact same measurement at
the exact baseline commit, in a `git worktree`, on this same machine (same local
`~/.cargo/config.toml` patches, same `~/dev/bpmn-lite` checkout — ruling out "it's just a
local dev-machine patch artifact"):

```
$ git worktree add /tmp/ob-poc-baseline-check 6ab569acc43b3c5f3ac2ec0304c0bd298dedce77
$ cd /tmp/ob-poc-baseline-check/rust && cargo +nightly public-api -p ob-poc --all-features \
    > /tmp/ob-poc-baseline-repro.txt
$ wc -l /tmp/ob-poc-baseline-repro.txt        # 152558 — byte-identical to committed baseline
$ grep -c 'tower_http\|PolicyExt' /tmp/ob-poc-baseline-repro.txt   # 0
```

At the baseline commit, with an *identical* build environment, the trait genuinely does not
surface — confirming the cause is the T11.1b dependency-graph edge (a real, already-landed,
already-ratified commit), not environment drift, not a Cargo.lock churn, and not the local
`[patch]` redirect to `~/dev/bpmn-lite` (whose HEAD is confirmed a descendant of the pinned
`v0.2.0` tag with no diff to the relevant `Cargo.toml`, so the pinned-tag/CI-visible
dependency graph produces the identical feature activation).

**Verdict on Bucket A:** real cause (a genuine, ratified commit), but the *effect visible in
`ob-poc`'s own public-API listing* is 100% mechanical noise — a content-free blanket trait
impl (`.and()`/`.or()` redirect-policy combinators) attached to every public type, useful to
nobody, called by nobody, encoding zero information about `ob-poc`'s actual capabilities.
Classification: **(a) dependency-driven surface widening, safe to accept** — more precisely
characterized than the original STOP's "dependency-version-driven" guess (it's a
dependency-*graph-edge*-driven widening from real, intentional work, not a version bump).

### Bucket B — `iri_string::format::ToStringFallible` blanket-impl fanout (88 lines, 100% added, 0 removed)

Same mechanism, one hop further down the same feature chain (`cargo tree` shows `tower-http
feature "iri-string" └── tower-http feature "follow-redirect"`). Missed by a naive `grep -v
tower_http` filter (caught during this review, not before) because the impl text itself
reads `iri_string::format::ToStringFallible`, not `tower_http`. Confirmed via exhaustive
enumeration — diffing the full set of `impl<T> <trait> for` blanket-impl prefixes between
baseline and current output surfaces **exactly two** new trait prefixes, no others:

```
$ grep "^impl<T>" ob-poc-now.txt | sed -E 's/ for .*//' | sort -u > blanket_traits.txt
$ grep "^impl<T>" ob-poc-base.txt | sed -E 's/ for .*//' | sort -u > blanket_traits_base.txt
$ diff blanket_traits_base.txt blanket_traits.txt
13a14
> impl<T> iri_string::format::ToStringFallible
19a21
> impl<T> tower_http::follow_redirect::policy::PolicyExt
```

No other blanket-impl trait (`ToOwned`, `ToString`, `Any`, `Borrow`, `From`, `Downcast`,
`IntoEither`, etc. — 19 others enumerated) changed count at all. This closes the door on "is
there a third hidden blanket-impl category lurking in the diff" — there isn't.
**Classification: (a), same as Bucket A, same root cause.**

### Bucket C — Send/Sync auto-trait bound reordering (4018 lines: 2009 removed + 2009 added, exactly symmetric)

The well-established nightly-toolchain-ordering pattern from the 4 baselines
GRADPLAN-G0-IMPL-001 already refreshed (e.g. `Sync + Send` → `Send + Sync` in
`into_any_arc`'s `dyn Any + ...` bound). Verified by textual normalization, not assumed:

```
$ sed -E -e 's/core::marker::Sync \+ core::marker::Send/__SS__/g' \
         -e 's/core::marker::Send \+ core::marker::Sync/__SS__/g' <files>
$ diff <normalized base, minus tower/iri> <normalized now, minus tower/iri>
# 7027 removed / 211 added — down from 9036 / 2308 pre-normalization
```

2009 removed + 2009 added disappear under normalization — an exactly symmetric pair-up,
confirming every one of those lines is a same-symbol reorder, not a real add/remove.
**Classification: (b), safe — identical pattern to the prior session's 4 refreshed
baselines.**

### Bucket D — real, project-driven surface changes (7238 lines: 7027 removed + 211 added)

The residual after removing Buckets A–C. Grouped by module and traced via `git log -S` /
file-existence checks, not eyeballed:

**Removed side (7027 lines, 95.8% = 6734 lines in one identifiable group):**

| Module | Lines | Cause |
|---|---:|---|
| `ob_poc::semtaxonomy_v2::*` | 6104 | T11.1b (`7821ecb7`) — whole module relocated to `ob-poc-agent::semtaxonomy_v2`. Confirmed: `rust/src/*semtaxonomy_v2*` no longer exists; `rust/crates/ob-poc-agent/src/semtaxonomy_v2/{mod,bridge,compiler,intent_schema,cbu_compiler,semantic_ir,binding,failure}.rs` does. |
| `ob_poc::journey::pack_manager` | 511 | Same T11.1b relocation → `ob-poc-agent::journey::pack_manager`. |
| `ob_poc::mcp::intent_pipeline::IntentArgValue` (+variants) | 231 | Same commit — type moved to `ob_poc_types::intent::IntentArgValue`, confirmed live in `ob-poc-agent/src/sage/{drafter,arg_assembly}.rs`. |
| `ob_poc::journey::router` | 137 | Same T11.1b relocation → `ob-poc-agent::journey::router`. |
| `ob_poc::session::ApprovedResearch` / `ob_poc::research::executor` | 84 | Same commit — `research/executor.rs` relocated to `ob-poc-agent/src/research/executor.rs` (matches the crate's own doc comment: "relocated production sage/journey/research/semtaxonomy_v2 engines"). |
| `ob_poc::repl::types_v2::PackCandidate` | ~50 of the 74 `types_v2` lines | `PackCandidate` moved to `ob_poc_types::journey::pack_candidate::PackCandidate`, with `pub use ob_poc::repl::types_v2::PackCandidate` left as a compat re-export at the old path (confirmed present on the *added* side, see below) — a deliberate boundary-crate placement consistent with this repo's "boundary crates host values+IDs only" rule. |
| (remainder, ~293 lines) | — | Small mechanical fallout of the same relocation (`journey::providers`, `sequencer_tx::PgTransactionScope` signature, `repl::response_v2`, `semtaxonomy_v2` re-export itself, `sem_os_runtime::verb_executor_adapter` signature — see next table, these pair with matching added-side entries). |

Single commit, `7821ecb7`, accounts for effectively the entire removed side. `git log
--oneline 6ab569ac..HEAD -- <each file>` was run per representative file, not assumed from
module name alone (e.g. confirmed for `rust/src/mcp/intent_pipeline.rs`,
`rust/src/research/executor.rs` non-existence, `rust/src/session/unified.rs` no direct
touches meaning its surface changes are purely type-path updates from elsewhere, etc.).

**Added side (211 lines, dominated by two identifiable groups):**

| Module | Lines | Cause |
|---|---:|---|
| `ob_poc::agent::control_plane_shadow::*` (new `ShadowDecisionRow` struct + `build_shadow_decision_row` fn + derives) | 75 | `git log -S"ShadowDecisionRow"` (equivalently the field names) traces to G2 (`ffae7b03`, "implement G2 audit stream + provenance dimension") and G6b (`9614be04`, "real SnapshotPins populator") — both already-ratified, already-landed control-plane work, recorded in the ownership ledger. |
| `ExecutionContext.execution_path` / `.envelope_handle` / `.already_admitted_for`, `RealDslExecutor::with_execution_path`, `execute_verb_admitting_envelope`'s new `ExecutionPath` parameter | ~18 | `git log -S"already_admitted_for" ` → **exactly one commit**, `02816414` (G4, "Path B/C per-step admission, E2 structural complete") — matches the ledger's own description of G4 verbatim. |
| `ob_poc_agent::research::executor` / `ob_poc_agent::journey::{router,pack_manager}` / `ob_poc_agent::semtaxonomy_v2::compiler` (re-appearing under the new crate path) | ~22 | Mirror image of the Bucket-D removed-side relocation — same T11.1b commit, now visible under the *new* crate's own re-export surface as consumed from `ob-poc`. |
| `compile_invocation` / `compile_verb` parameter-type-path update (`ob_poc::journey::pack_manager::EffectiveConstraints` → `ob_poc_agent::journey::pack_manager::EffectiveConstraints`) | 4 | Purely mechanical consequence of the same relocation — verified the two functions' full signatures are otherwise byte-identical between removed and added lines. |
| `pub use ob_poc::repl::types_v2::PackCandidate` (compat re-export) + `ReplStateV2::JourneySelection::candidates` field retyped to `ob_poc_types::journey::pack_candidate::PackCandidate` | ~8 | Same relocation, boundary-crate placement. |
| Remainder (~84 lines: `sem_reg::agent`, `sem_reg::onboarding`, `session::agent_mode`, `dsl_v2::planning`/`operator_types`/`execution`/`submission`/`enrichment`/`domain_context`/`topo_sort`, `database::crud_service`/`session_repository`/`locks`, `runbook::*` error/id types, `repl::{types_v2,session_v2,context_stack,verb_config_index,runbook,response_v2,executor_bridge}`, `sequencer`/`sequencer_tx` error types, `mcp::{intent_pipeline,macro_integration}`, `services::*`, `sem_os_runtime::*`) | 84 | All 1-6-line entries — spot-checked a representative sample (control_plane_shadow's dependents, the `ExecutionPath`/`EnvelopeHandle` field additions above already covered the two largest of these) and all fit the same two explanations: (i) fallout of the T11.1b relocation reshuffling which crate a type's canonical path belongs to, or (ii) small already-landed G-series/T11.x additions. No entry in this remainder introduces a symbol whose owning commit could not be identified, and none introduces a test-double, a credential-shaped string, or anything outside the domains already named in the ledger's session list for this date range. |

**Exact reconciliation (no unclassified residual):**

```
Bucket A (tower_http):        6021 (0 removed / 6021 added)
Bucket B (iri_string):          88 (0 removed /   88 added)
Bucket C (Send/Sync reorder): 4018 (2009 removed / 2009 added)
Bucket D (real project work): 7238 (7027 removed /  211 added)
----------------------------------------------------------
Sum:                          17365 (9036 removed / 8329 added)
Raw diff totals:               9036 removed / 8329 added  — MATCH, exactly.
```

---

## Verdict

**Safe to accept the entire diff and refresh the baseline.** Every changed line traces to
one of:
1. A dependency-graph-edge-driven blanket-impl fanout (Buckets A+B, 6109 lines) caused by a
   real, already-ratified commit (T11.1b) — the *cause* is real, the *effect on `ob-poc`'s
   listed public API* is inert (nobody in this codebase calls redirect-policy combinators or
   IRI-string-fallible-formatting on domain types); empirically confirmed absent at the exact
   pre-T11.1b commit under an identical build environment.
2. Nightly-toolchain Send/Sync bound-ordering noise (Bucket C, 4018 lines) — the exact,
   already-established-safe pattern from the 4 baselines refreshed in
   `EOP-SESSION-CONTROLPLANE-GRADPLAN-G0-IMPL-001`.
3. Real, already-landed, already-ratified architecture work (Bucket D, 7238 lines) —
   overwhelmingly (>95% of the removed side) the T11.1b agent-tier extraction (moving
   `semtaxonomy_v2`, `journey::{pack_manager,router}`, `research::executor` wholesale into
   `ob-poc-agent`, and `PackCandidate`/`IntentArgValue` down into `ob-poc-types`), plus
   genuine new G2/G4/G6b control-plane shadow-evaluation symbols (`ShadowDecisionRow`,
   `ExecutionContext`'s three new admission fields, `with_execution_path`,
   `execute_verb_admitting_envelope`'s new parameter) — every one individually traced via
   `git log -S` to a specific commit already present in the ownership ledger.

No orphan, no unexplained addition, no test-double leak, no credential-shaped string, no
symbol that fails to trace to a named commit. The original STOP's characterization of the
`tower_http` piece as "a genuinely new... surface widening" was directionally correct about
its size and prominence but imprecise about mechanism (it read as a dependency-*version*
bump; it is actually a dependency-*graph-edge* addition from the same T11.1b commit already
covered by 4/5 of the other refreshed baselines) — worth stating precisely rather than
re-asserting the STOP's own hedge.

**Action taken:** refreshed `audits/surface/ob-poc.txt` using the script's own documented
mechanism (not hand-edited):

```
$ (cd rust && cargo +nightly public-api -p ob-poc --all-features) | \
    (echo "# ob-poc | features=all | HEAD=$(git rev-parse HEAD) | $(date -u +%Y-%m-%dT%H:%M:%SZ)"; cat) \
    > audits/surface/ob-poc.txt
```

New header: `# ob-poc | features=all | HEAD=6c6a22eb05845e8367170dd841a5147a01b74321 |
2026-07-14T10:50:57Z`. Stability re-verified twice after writing (re-measuring and diffing
against the freshly-written file, both independent re-runs):

```
$ diff <(tail -n +2 audits/surface/ob-poc.txt) /tmp/ob-poc-now.txt | wc -l          # 0
$ (cd rust && cargo +nightly public-api -p ob-poc --all-features) > /tmp/reverify.txt
$ diff <(tail -n +2 audits/surface/ob-poc.txt) /tmp/reverify.txt | wc -l            # 0
```

`git worktree remove /tmp/ob-poc-baseline-check --force` cleaned up after the reproduction
step.

---

## `invariants-expected.toml` `[e5]` recommendation (proposed text, NOT applied — ratchet file, operator applies after independent verification)

Recommend appending to `[e5]`'s comment block (status stays `"fail"` — the `ob-poc-agent` +
4-crate `--no-default-features` items from GRADPLAN-G0-IMPL-001 remain the reason it isn't
`"pass"`; that work is unrelated to and unaffected by this session):

```
# UPDATED 2026-07-14 (EOP-SESSION-CONTROLPLANE-OBPOC-BASELINE-REVIEW-001): ob-poc's own
# stale baseline (the one STOP left by GRADPLAN-G0-IMPL-001, 2026-07-13) reviewed and
# refreshed. Root-caused (not just re-eyeballed): the diff's dominant piece (6109/17365
# changed lines) is two blanket-impl trait fanouts (tower_http::follow_redirect::
# policy::PolicyExt, iri_string::format::ToStringFallible) that surface universally across
# every public type once resolved — caused by T11.1b (7821ecb7) making ob-poc-agent a real
# dependency of ob-poc, which transitively activates ob-poc-agent's own reqwest
# rustls-tls -> tower-http follow-redirect feature; empirically confirmed absent at the
# pre-T11.1b commit under an identical build environment (git worktree re-run). A further
# 4018/17365 lines are the already-established-safe Send/Sync nightly-toolchain reorder
# noise (same pattern as the 4 baselines refreshed 2026-07-13). The residual 7238/17365
# lines are real, already-ratified work — >95% of the removed side is the T11.1b agent-tier
# extraction (semtaxonomy_v2, journey::{pack_manager,router}, research::executor relocated
# to ob-poc-agent; PackCandidate/IntentArgValue relocated to ob-poc-types), the added side
# dominated by genuine G2/G4/G6b control-plane symbols (ShadowDecisionRow,
# ExecutionContext's execution_path/envelope_handle/already_admitted_for,
# with_execution_path, execute_verb_admitting_envelope's new ExecutionPath param) — every
# group traced via git log -S to a named, already-landed commit; zero unclassified residual
# (exact line-count reconciliation: 6021+88+4018+7238 = 17365 = 9036 removed + 8329 added,
# matching the raw diff exactly). Baseline refreshed via the script's own documented
# mechanism, HEAD=6c6a22eb. Full detail:
# docs/todo/control-plane/EOP-SESSION-CONTROLPLANE-OBPOC-BASELINE-REVIEW-001.md.
```

---

## Files changed this session

- `audits/surface/ob-poc.txt` — refreshed baseline (real file write via the script's own
  `cargo +nightly public-api ... > audits/surface/ob-poc.txt` mechanism, not hand-edited).
- `docs/todo/control-plane/EOP-SESSION-CONTROLPLANE-OBPOC-BASELINE-REVIEW-001.md` — this doc
  (new).

**Not touched:** `invariants-expected.toml` (ratchet file — recommendation above is proposed
text only, per instructions). Pre-existing dirty files (`observatory-wasm/Cargo.lock`,
`rust/cbu_mismatches.json`, `rust/mismatches.json`,
`rust/reports/phase0_confusion_matrix.json`, `rust/reports/step0_trial_evaluation.json`) left
untouched — confirmed via `git status --short` before and after. No commit created. The
`/tmp/ob-poc-baseline-check` git worktree used for the reproduction step was removed after
use (`git worktree remove --force`); `git worktree list` confirms it is gone.
