# Three-Plane Refactor — Drift Control Framework (v0.1)

> **Status:** Proposal for peer review. Becomes a Phase 0 deliverable (**Phase 0h**) and a prerequisite for Phase 1 if adopted.
> **Created:** 2026-04-18
> **Scope:** all drift dimensions across the 21-week, 7-phase three-plane refactor. Cross-cuts every phase.
> **Companion artefacts:**
> - `docs/todo/three-plane-architecture-v0.3.md` (destination)
> - `docs/todo/three-plane-architecture-implementation-plan-v0.1.md` (phases, decisions)
> - `docs/todo/phase-0a-ownership-matrix.md` (file-level classification)
> - `docs/todo/pattern-b-a1-remediation-ledger.md` (39-op remediation tracker)
>
> This framework supplements, not supersedes, the above. If any conflict arises, this framework governs *process*; the others govern *substance*.

---

## Contents

1. Purpose
2. Drift taxonomy — the 10 dimensions
3. Layered defence model
4. Baseline snapshot contract
5. Gate artefact (YAML) contract
6. PR-level drift rubric
7. Contract golden files
8. Lint specifications (L1–L4 + new L5–L7)
9. Cumulative regression monitoring
10. Weekly refactor-health review
11. Escalation protocol
12. Automated infrastructure layout
13. Correction workflows per drift type
14. Meta-drift control — preventing the framework itself from drifting
15. Integration with existing plan artefacts
16. Appendix A — Baseline snapshot field list
17. Appendix B — Gate YAML templates per phase
18. Appendix C — Custom clippy lint bodies (L2–L7)
19. Appendix D — PR template body
20. Appendix E — Weekly review minutes template
21. Appendix F — Drift-indicator metric cheat sheet

---

## 1. Purpose

A 21-week, multi-crate refactor touching 625+ operations, 20 crates, 4 subsystems and a frontend is a large enough change to drift silently. Per-slice drift — the kind that passes individual phase gates but accumulates into architectural erosion across phases — is the specific risk this framework addresses.

**Drift is any divergence between the refactor's as-designed state (captured in v0.3 + decisions + ledger) and the refactor's as-implemented state, that is not deliberate and accounted for.**

This framework exists so that:

1. Drift is **detected** early, at the PR level when possible, at phase-gate level at the latest.
2. Drift is **prevented** where automation allows — lints, golden files, harnesses.
3. Drift is **corrected** through a deterministic protocol, not ad-hoc discussion.
4. The framework itself is bounded — drift-control can itself drift if the framework is not versioned and reviewed.

---

## 2. Drift taxonomy — the 10 dimensions

Every drift encountered in this refactor will fall into one of these 10 categories. Each category has a dedicated detection mechanism (§§4–9) and a correction workflow (§13).

### D1. Decision drift

A slice implements something that silently contradicts a closed decision (D1–D11 or any future D*).

*Example:* a PR inside Phase 5 wraps an external HTTP call inside a 2PC saga — contradicting D8 and the D11 Pattern B resolution.

*Detection:* PR drift rubric (§6) + human review.

### D2. Scope drift

A slice adds work not in the phase's declared scope.

*Example:* a Phase 2 PR (CustomOperation move) also refactors `RunbookPlanStep` shape — that's Phase 5 / Q4 work, not Phase 2.

*Detection:* phase scope statements + PR-to-phase scope check in rubric.

### D3. Contract drift

Trait or struct shapes listed as locked change without deliberate spec update.

*Example:* a Phase 5b PR adds a new field to `VerbExecutionPort::execute_json`'s signature.

*Detection:* contract golden files (§7) + CI diff detection.

### D4. Regression drift

A measurable metric (intent hit rate, test pass rate, latency p95, scenario verb rate, narration round-trip time) degrades without acknowledgment.

*Example:* intent hit rate drops from 78.2% to 76% over 4 PRs, each PR individually passing its ±1% gate.

*Detection:* baseline snapshots (§4) + cumulative regression monitoring (§9).

### D5. Gate drift

A phase gate is declared passed without actually meeting its criteria.

*Example:* Phase 5b closes on "tests green" without the determinism-harness byte-identity check having been rerun.

*Detection:* gate YAML artefacts with explicit pass/fail signatures (§5).

### D6. Baseline drift

The refactor's starting assumptions (v0.3 §4.1 or implementation plan §1.2) become stale and the refactor acts on wrong premises.

*Example:* Motivated Sage goes from "5%" to "30%" over 3 months mid-refactor and stage 2a/2b design no longer matches reality.

*Detection:* baseline snapshot re-capture at each phase gate (§4).

### D7. Invariant drift

A cross-phase invariant (closed-loop rehydrate, determinism, single dispatch site, three-plane dependency direction) silently breaks.

*Example:* Phase 5c introduces a case where `writes_since_push` is advanced without a matching rehydrate call.

*Detection:* dedicated invariant tests (§8) + determinism harness + L1–L7 lints.

### D8. Cumulative drift

Individual slices each pass their local gate but the aggregate drifts.

*Example:* 10 PRs each acceptable individually; combined, they shift NLP responsibility partly back into `SessionVerbSurface`, violating Q3 resolution.

*Detection:* rolling-window metrics (§9) + weekly review (§10).

### D9. Ledger drift

Pattern B remediation ledger rows quietly stay OPEN forever; or worse, silently move to CLOSED without the underlying work being done.

*Example:* Phase 5f is "done" but 6 `source_loader_ops` still have `reqwest::*` calls inside `execute_json`; lint L4 escape hatch was quietly used without updating the ledger.

*Detection:* lint L4 (zero escape-hatch tolerance) + ledger row review + DoD item 19.

### D10. Meta-drift (framework drift)

The drift framework itself becomes outdated — unmaintained, ignored, or inconsistent with reality.

*Example:* Gate YAMLs stop being updated from PR #180 onward; nobody notices until Phase 6.

*Detection:* §14.

---

## 3. Layered defence model

Drift is caught at the **latest** possible layer. The earlier, the cheaper. The defence layers from earliest to latest:

```
Layer 1 — Pre-commit (local author workstation, runs in seconds)
   ├── L1 cargo-deny (dependency direction)
   ├── L4 partial (regex scan for external effects in execute bodies)
   └── contract-golden-diff (warn on trait shape change)

Layer 2 — PR CI (runs on push, minutes)
   ├── cargo build --workspace
   ├── cargo clippy --workspace (includes L2, L3, L4, L5, L6, L7)
   ├── cargo test --workspace
   ├── determinism harness (byte-compare stage outputs)
   ├── round-trip harness (effect-equivalence on touched CRUD ops)
   ├── contract golden file check (insta/similar)
   ├── baseline snapshot delta (warn > 0.5%, fail > 1%)
   ├── PR drift rubric (GitHub Action checks PR description format)
   └── lint L1 (cargo-deny) full run

Layer 3 — Weekly health review (30 min, Mondays)
   ├── Ledger status sweep (Pattern A/B rows)
   ├── Baseline-drift trend review
   ├── Decision-drift review (PRs merged last week)
   ├── Scope-drift review (current phase scope adherence)
   └── Meta-drift review (framework self-check quarterly)

Layer 4 — Phase gate review (at each phase boundary)
   ├── Gate YAML signed off
   ├── Baseline snapshot re-captured
   ├── Golden files re-verified
   ├── Ledger rows updated (Pattern A at Phase 0g close, Pattern B at 5f close)
   └── Entry criteria for next phase confirmed

Layer 5 — Refactor-wide review (Phase 6 gate, Phase 7 gate, DoD sign-off)
   ├── All 10 drift dimensions checked
   ├── Framework itself reviewed (has it drifted?)
   ├── Metrics snapshot vs Phase 0 baseline
   └── DoD items 1–19 all checked
```

**Invariant:** no slice enters a later layer without clearing the earlier ones. No phase enters without its predecessor's gate YAML signed.

---

## 4. Baseline snapshot contract

### 4.1 What is captured

A baseline snapshot is a machine-readable YAML file capturing the state of metrics and artefacts that matter for drift detection. It is captured:

- At **Phase 0 start** — the reference baseline.
- At **each phase gate closure** — one per phase (0, 0g, 1, 2, 3, 4, 5a–5f, 6, 7).
- **On demand** by any reviewer (via a single `cargo x baseline-snapshot` command).

### 4.2 File format

Stored at `docs/refactor-health/baselines/<phase>-<date>.yaml`. Example:

```yaml
snapshot_version: 1
captured_at: 2026-04-18T14:30:00Z
captured_by: phase-0-start
git_sha: 6bbf134e
workspace:
  crate_count: 20
  crate_list:
    - dsl-core
    - dsl-lsp
    - entity-gateway
    - governed_query_proc
    - inspector-projection
    - ob-agentic
    - ob-poc-macros
    - ob-poc-types
    - ob-poc-web
    - ob-semantic-matcher
    - ob-templates
    - ob-workflow
    - playbook-core
    - playbook-lower
    - sem_os_client
    - sem_os_core
    - sem_os_harness
    - sem_os_obpoc_adapter
    - sem_os_postgres
    - sem_os_server
tests:
  passing: 1194       # from `cargo test --workspace --no-fail-fast`
  failing: 0
  ignored: 12
  duration_seconds: 483
metrics:
  intent_hit_rate_first_attempt_pct: 78.2
  intent_hit_rate_two_attempt_pct: 99.4
  scenario_verb_rate_pct: 91.7
  narration_round_trip_ms_p95: 187
  outbox_drain_lag_ms_p95: null       # not yet wired
  determinism_harness_coverage_pct: null  # pre-harness
code:
  domain_ops_file_count: 89
  custom_operation_impl_count: 650    # approximate
  trait_signature_hashes:
    VerbExecutionPort: sha256:abc123...
    CustomOperation: sha256:def456...
    GatedVerbEnvelope: null   # not yet defined
    PendingStateAdvance: null
    TransactionScope: null
schema:
  migrations_head: 128
  entity_tables_with_row_version: 0
  entity_tables_total: 147
  outbox_tables:
    - sem_reg.outbox_events
pattern_b_ledger:
  pattern_a_open: 1     # sem_os_maintenance_ops.rs
  pattern_b_open: 38    # 5 bpmn_lite + 16 source_loader + 17 gleif
  total_closed: 0
decisions_honoured:
  # booleans that are cheap to verify automatically
  D1_row_version_bigint_everywhere: null  # not yet deployed
  D7_alternate_step_executors_removed: false
  D8_new_a1_violations: false
  D11_pattern_b_ledger_exists: true
```

### 4.3 Capture script

`rust/xtask/src/baseline_snapshot.rs`:

```
cargo x baseline-snapshot --label "<phase>-<date>" --out docs/refactor-health/baselines/
```

Collects from:
- `cargo test --workspace --no-fail-fast -- --list` → test counts
- Test output → pass/fail/ignored
- `ls rust/crates/ | wc -l` → crate count
- `cargo metadata --format-version 1` → crate list
- Dedicated intent-hit-rate harness run
- Dedicated scenario-verb-rate harness run
- Perf harness (narration, outbox)
- Postgres schema introspection → row_version coverage
- Git SHA
- Trait signature hashes (computed via `rustfmt`-normalised text of trait + SHA-256)
- Pattern B ledger parse for open/closed counts

### 4.4 Delta comparison rules

Every CI run produces a "transient" snapshot at HEAD and diffs it against the **most recent phase-gate baseline**.

| Metric | Warn threshold | Fail threshold |
|---|---|---|
| tests.passing | -0.5% | -1% or any new failure |
| intent_hit_rate_first_attempt_pct | -0.5 pts | -1.0 pt |
| intent_hit_rate_two_attempt_pct | -0.25 pts | -0.5 pt |
| scenario_verb_rate_pct | -1.0 pt | -2.0 pts |
| narration_round_trip_ms_p95 | +20% | +50% |
| outbox_drain_lag_ms_p95 (post-5e) | +20% | +50% |
| determinism_harness_coverage_pct | any drop | any drop |
| trait_signature_hashes | any change | any change without corresponding golden-file update |
| pattern_b_ledger.total_closed | any regression (open → closed → open) | any regression |

Warn = PR posts a comment and requires acknowledgment.
Fail = PR merge blocked.

### 4.5 Retention

All baseline snapshots are retained in-repo. Phase-0 baseline is permanent. Gate baselines retained for the life of the refactor + 12 months.

---

## 5. Gate artefact (YAML) contract

### 5.1 Purpose

A phase gate is not closed until its YAML artefact is committed with pass/fail signatures. Prose gate criteria are binding when referenced from the YAML, but only the YAML is the authoritative record.

### 5.2 File location

`docs/refactor-health/gates/phase-<id>.yaml`. One per phase and sub-phase.

### 5.3 Format

```yaml
gate_version: 1
phase: "5b"
phase_name: "Sequencer extraction"
opened_at: 2026-07-15T09:00:00Z
opened_by: adamtc007
depends_on:
  - "5a"           # prerequisite phases
  - "0g"           # if applicable
  - "row-version-migration"
scope_statement: |
  Rename-and-refactor rust/src/repl/orchestrator_v2.rs into rust/src/sequencer/
  (module). Surface nine stages as typed functions. Introduce stage 6 Gate
  boundary producing GatedVerbEnvelope. Extract NLP from SessionVerbSurface
  per Q3 resolution.
criteria:
  - id: determinism-harness-byte-identical
    description: "Stage outputs 4–8 byte-identical vs phase-5a baseline"
    check_command: "cargo x determinism-check --baseline phase-5a"
    status: pending
    signed_by: null
    evidence: null

  - id: all-tests-pass
    description: "cargo test --workspace returns 0 failures"
    check_command: "cargo test --workspace --no-fail-fast"
    status: pending
    signed_by: null
    evidence: null

  - id: single-dispatch-site-enforced
    description: "Lint L2 passes: VerbExecutionPort::execute_json called from exactly 1 non-test location"
    check_command: "cargo clippy --workspace -- -D custom::single-dispatch-site"
    status: pending
    signed_by: null
    evidence: null

  - id: stage-boundaries-typed
    description: "Every stage function has typed input and output, no serde_json::Value pass-through"
    check_command: "cargo x stage-boundary-audit rust/src/sequencer/"
    status: pending
    signed_by: null
    evidence: null

  - id: nlp-out-of-semos
    description: "Lint L3 passes: no candle_*/embedding crate imports in sem_os_* crates"
    check_command: "cargo deny check bans"
    status: pending
    signed_by: null
    evidence: null

  - id: ledger-rows-updated
    description: "Pattern B ledger rows for this phase are at current status"
    check_command: "cargo x ledger-check --phase 5b"
    status: pending
    signed_by: null
    evidence: null

  - id: baseline-snapshot-captured
    description: "docs/refactor-health/baselines/phase-5b-*.yaml exists"
    check_command: "ls docs/refactor-health/baselines/phase-5b-*.yaml"
    status: pending
    signed_by: null
    evidence: null

  - id: contract-golden-files-green
    description: "Golden files for locked traits match current trait shapes"
    check_command: "cargo insta review --require-snapshots"
    status: pending
    signed_by: null
    evidence: null

exit_conditions:
  phase_6_unblocked: false   # only becomes true if this is 5f
  phase_7_unblocked: false

closed_at: null
closed_by: null
sign_off_commit_sha: null
```

### 5.4 Closure rules

- A gate closes only when **all criteria** have `status: passed` and non-null `signed_by` + `evidence` (commit SHA or artefact path).
- Closure is a commit that sets `closed_at`, `closed_by`, `sign_off_commit_sha`.
- Closure cannot be reverted in a subsequent commit without a companion PR labeled `gate-re-open` that includes an explanation.
- Phase N+1 cannot open unless Phase N's gate YAML is closed. CI checks this at Phase N+1 kickoff PR.

### 5.5 Gate templates

Per-phase gate templates live in Appendix B. Phase 0h (this framework) has its own gate.

---

## 6. PR-level drift rubric

### 6.1 PR description requirements

Every PR in this refactor branch (whatever it is named) must include a machine-parseable header:

```
<!-- DRIFT-HEADER-v1 -->
phase: 2               # one of 0|0g|0h|1|2|3|4|5a|5b|5c|5d|5e|5f|6|7
scope_statement: |
  Move CustomOperation trait + CustomOperationRegistry + VerbRegistrar to
  dsl-runtime. Additive: PendingStateAdvance and outbox_drafts fields
  appended to VerbExecutionOutcome.
decisions_touched:
  - D3  # macro moved with this PR? yes/no in body
  - D7  # alternate step executors affected? no, deferred to 5b
contract_shapes_changed:
  - VerbExecutionOutcome  # if yes, must accompany golden-file update
new_dependencies_added: []
a1_lint_exceptions_added: []   # #[allow(external_effects_in_verb)] introduced
ledger_rows_touched: []         # entries in pattern-b-a1-remediation-ledger.md
regression_notes: |
  Expected: determinism harness byte-identical with additive-field equality
  (empty vectors for all 625 ops). Intent hit rate unchanged.
<!-- /DRIFT-HEADER -->
```

### 6.2 CI-level rubric checks (Layer 2)

A GitHub Action (or equivalent) parses the drift header and enforces:

| Check | Fail condition |
|---|---|
| Header present | Header missing |
| Phase valid | Phase not in enumerated list |
| Scope statement non-empty | ≤20 chars |
| Phase-scope alignment | Files changed outside the phase's expected scope with no justification |
| Decisions_touched sanity | Files matching `file patterns associated with Dn` but `Dn` not in list |
| Contract shapes | Golden-file diff detected but field empty |
| A1 exceptions | `#[allow(external_effects_in_verb)]` introduced but field empty, OR field populated but no ledger reference in code |
| Ledger rows | Ledger file modified but field empty |

### 6.3 Human reviewer obligations

For every PR that touches ≥1 item in contract_shapes_changed OR decisions_touched OR a1_lint_exceptions_added: reviewer must explicitly acknowledge in a comment.

Reviewer checklist (expected pinned as PR template sidebar):

```
- [ ] Drift header present and accurate
- [ ] Scope aligns with declared phase
- [ ] No decision contradicted
- [ ] Contract shape changes intentional + golden files updated
- [ ] A1 exceptions justified + ledger updated
- [ ] Baseline delta warnings acknowledged (if any)
- [ ] Determinism harness green
- [ ] No new entries in "see also" or "removed" patterns without reason
```

### 6.4 Exit (merging)

PR merges only if: header valid + all CI gates green + reviewer checklist complete. GitHub branch protection enforces.

---

## 7. Contract golden files

### 7.1 Purpose

Certain types are load-bearing contracts between planes. Their shapes must not change without deliberate spec update. Golden files lock the shape.

### 7.2 Locked types

| Type | Lives in | Golden file |
|---|---|---|
| `VerbExecutionPort` trait | `dsl-runtime` (post Phase 1) | `tests/contracts/verb_execution_port.snap` |
| `CustomOperation` trait | `dsl-runtime` (post Phase 2) | `tests/contracts/custom_operation.snap` |
| `VerbRegistrar` trait | `dsl-runtime` (post Phase 2) | `tests/contracts/verb_registrar.snap` |
| `GatedVerbEnvelope` | `ob-poc-types` (post Phase 0b) | `tests/contracts/gated_verb_envelope.snap` |
| `AuthorisationProof` | `ob-poc-types` | `tests/contracts/authorisation_proof.snap` |
| `PendingStateAdvance` | `ob-poc-types` | `tests/contracts/pending_state_advance.snap` |
| `TransactionScope` trait | `ob-poc-types` | `tests/contracts/transaction_scope.snap` |
| `OutboxDraft` | `ob-poc-types` | `tests/contracts/outbox_draft.snap` |
| `OutboxEffectKind` enum | `ob-poc-types` | `tests/contracts/outbox_effect_kind.snap` |
| `VerbExecutionOutcome` | `dsl-runtime` | `tests/contracts/verb_execution_outcome.snap` |
| `VerbExecutionContext` | `dsl-runtime` | `tests/contracts/verb_execution_context.snap` |
| `StateGateHash` | `ob-poc-types` | `tests/contracts/state_gate_hash.snap` |

### 7.3 Generation mechanism

Use the `insta` crate (already common in Rust). For each locked type, a dedicated test:

```rust
// tests/contracts/verb_execution_port.rs
#[test]
fn contract_verb_execution_port() {
    let shape = shape_of::<dyn VerbExecutionPort>();
    insta::assert_snapshot!("verb_execution_port", shape);
}
```

`shape_of` is a helper in a new `rust/crates/contract-shape/` crate that emits a canonical, deterministic text representation:

```
trait VerbExecutionPort: Send + Sync
fn execute_json(
    &self,
    args: &Value,
    ctx: &mut VerbExecutionContext,
    pool: &PgPool,
) -> impl Future<Output = Result<VerbExecutionOutcome>> + Send
```

For structs, the snapshot is the field-by-field layout in declaration order (with types). For enums, the variant list with payload shapes. For traits, method signatures in source order. Macros and attributes excluded.

### 7.4 Update workflow

A contract shape change is a deliberate act:

1. PR author changes the type.
2. PR author runs `cargo insta review` locally; approves the new snapshot.
3. PR description contract_shapes_changed field lists the changed type.
4. Reviewer verifies the change matches the intended spec update.
5. If the change is not spec-authorised, PR is rejected or the spec update is filed in the same PR.

### 7.5 Off-limits

Contract changes that are **never** acceptable mid-refactor (require re-opening decision + version bump):

- Changes to `VerbExecutionPort` signature (locked at Phase 2 per R2).
- Removal of any field from `GatedVerbEnvelope` (only additions with `#[serde(default)]`).
- Removal of any `OutboxEffectKind` variant.

CI lint L6 (§8) enforces these specific bans by comparing the AST of the golden file to a frozen baseline from its first-committed version.

---

## 8. Lint specifications (L1–L7)

### L1 — One-way dependencies

**Tool:** `cargo-deny` (`deny.toml`).

```toml
[graph]
# these packages MUST NOT depend on the packages listed in 'deny'
[[bans.deny]]
name = "sem_os_core"
wrappers = []
deny = ["dsl-runtime", "ob-poc", "ob-poc-web"]

[[bans.deny]]
name = "sem_os_postgres"
wrappers = []
deny = ["dsl-runtime", "ob-poc", "ob-poc-web"]

[[bans.deny]]
name = "dsl-runtime"
wrappers = []
deny = ["sem_os_core", "sem_os_postgres", "sem_os_server", "ob-poc", "ob-poc-web"]

[[bans.deny]]
name = "dsl-core"
wrappers = []
deny = ["sqlx", "tokio-postgres", "diesel"]

[[bans.allow-only]]
name = "ob-poc-types"
allow = ["std", "uuid", "chrono", "serde", "serde_json", "blake3"]
```

**Fails on:** any forbidden crate appearing in dependency graph.

### L2 — Single dispatch site

**Tool:** custom clippy lint.

**Scope:** counts call sites of `VerbExecutionPort::execute_json` across the workspace.

**Fails on:** more than 1 non-test call site. Expected site: `ob-poc::sequencer::stage_8::dispatch_envelope` (post Phase 5b). Test mocks allowed via `#[cfg(test)]`.

**Escape hatch:** none.

### L3 — No NLP in control plane

**Tool:** custom clippy lint + `cargo-deny`.

**Scope:** any file under `rust/crates/sem_os_*/src/` or `rust/src/sem_reg/`.

**Fails on:** import of `candle_core`, `candle_nn`, `candle_transformers`, `tokenizers`, `tiktoken-rs`, `ob-semantic-matcher`, or any crate whose metadata contains tokenizer/embedding keywords.

**Escape hatch:** none.

### L4 — No external effects inside CustomOperation execute bodies

**Tool:** custom clippy lint.

**Scope:** function bodies of `async fn execute` and `async fn execute_json` for any type with `impl CustomOperation`.

**Fails on:** any of the following symbols appearing in the body:

- `reqwest::`, `reqwest::Client`, `.get(`, `.post(`, `.put(`, `.delete(`, `.send(` (on HTTP client types)
- `http::`, `hyper::`, `surf::`, `isahc::`
- `tokio::process::`, `std::process::Command`, `Command::new`
- `tonic::` (gRPC client; allowed only if accessed via trait object passed in)
- Any struct name ending in `Client`, `Connection`, `HttpLoader` that is **not** a trait object parameter

**Escape hatch:** `#[allow(external_effects_in_verb)]` on the function. Additional requirements:

1. Must be accompanied by a `// TODO: ledger row <file>.rs — see pattern-b-a1-remediation-ledger.md`.
2. The referenced ledger row must **not** read CLOSED.
3. A companion check in the framework's `ledger-check` tool flags any mismatch.

### L5 — No serde_json::Value pass-through at stage boundaries

**Tool:** custom clippy lint.

**Scope:** function signatures in `rust/src/sequencer/` (post Phase 5b).

**Fails on:** any stage function taking or returning `serde_json::Value` as a typed parameter (not wrapped in a dedicated newtype).

**Rationale:** v0.3 §8.3 requires typed inputs/outputs per stage.

### L6 — Contract shape freeze

**Tool:** custom clippy lint + golden-file diff.

**Scope:** traits and structs listed in §7.2.

**Fails on:** any of the specifically banned changes in §7.5.

**Mechanism:** a `frozen-contracts.yaml` file lists (type, first-committed-snapshot-sha). If the current snapshot diffs against the frozen snapshot in banned ways (field removals, signature-breaking changes), CI fails.

### L7 — No raw SQL writes outside allowlisted paths

**Tool:** Reuses existing `rust/scripts/lint_write_paths.sh` (per CLAUDE.md SemOS-first enforcement).

**Scope:** writes to `attribute_registry`, entity tables, and other SemOS-governed stores.

**Fails on:** raw SQL `INSERT`/`UPDATE`/`DELETE` outside allowlisted service modules.

**Rationale:** pre-existing lint; incorporated into framework to ensure it's kept green through the refactor.

### Integration

All 7 lints run in CI (Layer 2). Local pre-commit (Layer 1) runs L1 + L4-partial only (fast).

---

## 9. Cumulative regression monitoring

### 9.1 The problem

Single-PR baseline comparisons catch step changes. They miss slow drift: 10 PRs each under the 0.5% warn threshold but accumulating to -5%.

### 9.2 Monitored metrics

Each metric has a fast comparison (last PR) and a slow comparison (rolling window).

| Metric | Slow window | Warn on | Fail on |
|---|---|---|---|
| `intent_hit_rate_first_attempt_pct` | last 10 PRs | downward slope ≥ 0.2 pts/PR over window, or window mean < baseline by 1 pt | window mean < baseline by 2 pts |
| `intent_hit_rate_two_attempt_pct` | last 10 PRs | downward slope ≥ 0.1 pts/PR | window mean < baseline by 0.5 pt |
| `scenario_verb_rate_pct` | last 10 PRs | downward slope ≥ 0.5 pts/PR | window mean < baseline by 2 pts |
| `narration_round_trip_ms_p95` | last 10 PRs | upward slope ≥ 5%/PR | window mean > baseline × 1.5 |
| `tests.passing` | last 10 PRs | downward slope (any) | any failing test |
| `determinism_harness_coverage_pct` | last 10 PRs | any drop | any drop |
| `outbox_drain_lag_ms_p95` (post-5e) | last 10 PRs | upward slope ≥ 5%/PR | window mean > 1s |
| `pattern_b_ledger.total_closed` | last 10 PRs (post-5e) | no forward progress for 10 PRs during Phase 5f | any closed→open regression |

### 9.3 Mechanism

A `cargo x refactor-health` command:

1. Fetches the last 10 baseline snapshots for the current phase.
2. Computes per-metric linear regression slope + window mean.
3. Emits a report: `docs/refactor-health/weekly-YYYY-MM-DD.md`.
4. Warns / fails per thresholds.

Runs weekly (scheduled GitHub Action) and on demand.

### 9.4 Alert response

Warn: posted in weekly review; discussed; acknowledged or escalated.

Fail: next PR in the phase blocked until the metric is restored OR the baseline is deliberately re-set (requires sign-off from the role who closed the last phase gate).

---

## 10. Weekly refactor-health review

### 10.1 Cadence and duration

- **When:** Monday 15:00 local, 30 minutes.
- **Who:** the refactor owner + at least one reviewer + whoever owns the ledger.
- **Where:** notes committed to `docs/refactor-health/weekly/YYYY-MM-DD.md`.

### 10.2 Agenda (template)

```markdown
# Weekly Refactor Health Review — <date>

## Attendees

- …

## 1. Ledger status (5 min)

- Pattern A (§2): [open / closed]
- Pattern B (§3): X/38 closed, Δ vs last week

## 2. Baseline delta report (5 min)

- Auto-generated: `docs/refactor-health/weekly-<date>.md`
- Metrics with warn status: …
- Metrics with fail status: …

## 3. PR decision/scope/contract drift review (10 min)

- PRs merged since last review: N
- Drift-header exceptions flagged: …
- Scope mis-alignments caught in review: …
- Contract golden-file updates: …

## 4. Current phase status (5 min)

- Phase in flight: …
- Gate criteria status: …
- Blockers: …

## 5. Next phase prep (3 min)

- …

## 6. Escalations (2 min)

- Any Tier 2 or Tier 3 triggers (§11)?

## Decisions made this review

- …

## Action items

- …
```

### 10.3 Retention

All weekly reviews are retained in-repo under `docs/refactor-health/weekly/`. They are never deleted. They form the paper trail for every decision and every drift episode.

### 10.4 Meta-review quarterly

Once per quarter, one weekly review slot is replaced with a **framework self-review** (§14).

---

## 11. Escalation protocol

### 11.1 Tiers

**Tier 0 — PR-level.** A CI check fails. PR blocked. Author fixes. No escalation.

**Tier 1 — Slice-level.** Weekly review detects drift requiring a decision (scope change, a1 exception justification, contract update). Assigned owner. Resolved within 1 week.

**Tier 2 — Phase-level.** Gate review detects drift that invalidates the phase's assumptions. Phase is halted. A remediation plan is drafted and reviewed before phase resumes.

**Tier 3 — Refactor-level.** Any of:
- Three Tier-2 incidents within 4 weeks
- A decision (D1–D11) needs re-opening
- Baseline drift (D6) invalidates v0.3 assumptions
- Framework itself needs major revision

Response: refactor halted. Cross-cutting review. Decisions re-opened with proper process. v0.4 considered.

### 11.2 Concrete triggers

| Observation | Tier |
|---|---|
| PR fails L1 | Tier 0 |
| PR fails baseline delta | Tier 0 |
| PR introduces new A1 exception without ledger row | Tier 0 |
| Weekly rolling-window warn on any metric | Tier 1 |
| Weekly rolling-window fail | Tier 1 → Tier 2 if unresolved after 2 weeks |
| Gate YAML cannot be signed after 2 attempts | Tier 2 |
| Contract shape change banned by L6 | Tier 2 |
| Motivated Sage completion rate diverges from assumption by >15% | Tier 2 |
| Phase 5f cannot close within 6 weeks | Tier 2 → Tier 3 if still unresolved after 8 weeks |
| 3 Tier-2 incidents in 4 weeks | Tier 3 |
| Pattern B ledger shows closed→open regression | Tier 3 (immediate) |

### 11.3 Tier response timelines

| Tier | Response time |
|---|---|
| Tier 0 | Immediate (blocked PR) |
| Tier 1 | 1 week |
| Tier 2 | 2 weeks to remediation plan; phase halted until plan accepted |
| Tier 3 | Refactor halted; no new PRs in refactor branch; cross-cutting review within 1 week |

### 11.4 Documentation requirement

Every Tier-1 or higher event is logged in `docs/refactor-health/incidents/<date>-<tier>-<slug>.md`. Format:

```markdown
# Incident: <title>
- Tier: 2
- Detected: <date>
- Detected by: <mechanism>
- Phase: 5c
- Drift dimension(s): D3 (contract), D7 (invariant)
- Description: …
- Remediation plan: …
- Closed: <date>
- Lessons learned: …
```

These are kept in-repo permanently.

---

## 12. Automated infrastructure layout

### 12.1 Directory structure

```
docs/refactor-health/
├── baselines/
│   ├── phase-0-2026-04-21.yaml
│   ├── phase-0g-2026-05-05.yaml
│   └── ...
├── gates/
│   ├── phase-0.yaml
│   ├── phase-0g.yaml
│   ├── phase-1.yaml
│   └── ...
├── weekly/
│   ├── 2026-04-21.md
│   ├── 2026-04-28.md
│   └── ...
├── incidents/
│   └── <date>-<tier>-<slug>.md
└── FRAMEWORK-VERSION.md      # tracks framework version + change log

rust/crates/contract-shape/   # helper crate for §7
rust/tests/contracts/          # golden file snapshots
rust/xtask/src/
├── baseline_snapshot.rs       # cargo x baseline-snapshot
├── determinism_check.rs       # cargo x determinism-check
├── refactor_health.rs         # cargo x refactor-health
├── ledger_check.rs            # cargo x ledger-check
└── stage_boundary_audit.rs    # cargo x stage-boundary-audit

rust/clippy-lints/             # custom clippy lints
├── external_effects_in_verb.rs    # L4
├── single_dispatch_site.rs        # L2
├── no_nlp_in_control_plane.rs     # L3
├── no_json_value_at_stage_boundary.rs  # L5
└── contract_freeze.rs             # L6

.github/workflows/
├── ci.yml                      # existing
├── drift-rubric.yml            # parses PR drift header
├── baseline-delta.yml          # runs baseline snapshot on PR, diffs
├── weekly-health.yml           # scheduled weekly run
└── gate-close-guard.yml        # blocks Phase N+1 kickoff if Phase N gate not closed

deny.toml                        # L1 configuration
frozen-contracts.yaml            # L6 configuration
rust/scripts/lint_write_paths.sh # L7 (pre-existing)
```

### 12.2 Build-up timeline (Phase 0h)

Phase 0h is **this framework** becoming executable. Deliverables:

1. **Day 1–2:** directory structure, FRAMEWORK-VERSION.md, baseline snapshot script, first Phase 0 baseline captured.
2. **Day 3–4:** gate YAML templates for Phases 0, 0g, 0h, 1; Phase 0 gate YAML populated.
3. **Day 5–7:** custom clippy lints L2, L4 skeleton (L3, L5 spec, deferred to their phase).
4. **Day 8–10:** contract-shape crate + insta snapshots for traits that exist today (`VerbExecutionPort`, `CustomOperation`).
5. **Day 11–12:** `cargo x refactor-health` script; first weekly review.
6. **Day 13–14:** PR drift rubric GitHub Action; merge-blocked branch protection configured.

**Total:** ~2 weeks calendar, though most of this parallelises with Phase 0b–0e.

---

## 13. Correction workflows per drift type

### D1 (Decision drift)

1. PR flagged in review.
2. Decision is either (a) honoured → PR amended, or (b) re-opened → new decision record appended to implementation plan §10.3.
3. If (b), all downstream plan artefacts (ledger, gate YAMLs, golden files) are re-checked for consistency.
4. Incident log entry filed.

### D2 (Scope drift)

1. PR author justifies in PR description.
2. Reviewer decides: (a) acceptable, (b) split into separate PR in correct phase, (c) reject.
3. If (a), the phase scope_statement in gate YAML is amended with the justification.

### D3 (Contract drift)

1. Golden file diff detected.
2. If banned by L6: PR blocked.
3. If allowed: author must update `contract_shapes_changed` drift header and reviewer must acknowledge.
4. Consumers of the changed type audited.

### D4 (Regression drift — single-PR)

1. Fail threshold: PR blocked. Author investigates, fixes or explains.
2. Warn threshold: acknowledgment required; not blocking.
3. Repeated warns across consecutive PRs → escalate to D8 (cumulative).

### D5 (Gate drift)

1. CI check `gate-close-guard.yml` blocks next phase PRs.
2. Gate YAML re-reviewed; missing criteria identified.
3. Either closed properly, or phase re-opened with amendment commit.

### D6 (Baseline drift)

1. Weekly review flags baseline mismatch with plan assumptions.
2. If mismatch is large: Tier 2 incident; plan section updated; affected decisions re-checked.
3. If small: noted in baseline snapshot comments.

### D7 (Invariant drift)

1. Dedicated invariant test fails (determinism harness, closed-loop test, single-dispatch lint).
2. PR blocked.
3. Invariant considered for promotion to a lint if currently only a test (or vice versa).

### D8 (Cumulative drift)

1. Rolling-window check fails.
2. Blocker applied to next PRs in phase.
3. Phase halted if not resolved in 1 week; Tier 2 incident.

### D9 (Ledger drift)

1. Weekly review: any row reads OPEN for >2 weeks with no commit-level progress → flagged.
2. If closed→open regression → Tier 3 incident immediately.
3. Lint L4 ensures escape-hatch usage is tied to an open ledger row.

### D10 (Meta-drift)

1. Quarterly self-review detects framework staleness.
2. Framework amended; FRAMEWORK-VERSION.md incremented.
3. Integration with plan artefacts re-verified.

---

## 14. Meta-drift control

### 14.1 The framework versioned

`docs/refactor-health/FRAMEWORK-VERSION.md` tracks:

```markdown
# Framework Version

Current version: 0.1
First adopted: 2026-04-18
Last amended: 2026-04-18

## Version log

- 0.1 (2026-04-18): initial framework
```

### 14.2 Amendment workflow

Any change to this framework (`three-plane-drift-control-framework.md`) requires:

1. A companion PR with a drift-header like any other PR.
2. `phase: 0h-amendment` in the header.
3. A version bump in FRAMEWORK-VERSION.md.
4. Reviewer-approved diff.

Amendments during a running phase should be minimal. Major amendments should wait for a phase boundary.

### 14.3 Quarterly self-review

Every 3 months, the weekly review is replaced with a framework self-review. Agenda:

```markdown
# Framework Self-Review — <date>

## 1. Is every drift dimension still relevant?

- D1–D10: still relevant? Any new patterns observed?

## 2. Are the detection mechanisms still working?

- Lint hit rates (L2/L4/L6 fired in last quarter)
- Contract golden file reviews triggered
- Baseline snapshot trend: noisy, stable, ignored?

## 3. Are the correction workflows being used?

- Incidents logged: count + tier distribution
- Tier 2/3 resolution time

## 4. Has the framework itself been maintained?

- Gate YAMLs fresh?
- Weekly reviews actually held?
- Minutes retained?

## 5. What should change in v0.N+1?

- …
```

### 14.4 Exit criterion

This framework retires when the refactor's DoD item 19 is checked AND a 3-month "clean" post-DoD period has passed with no drift incidents. At that point, the framework artefacts are archived but retained in-repo.

---

## 15. Integration with existing plan artefacts

### 15.1 Plan updates

This framework adds:

- **Phase 0h** to the implementation plan. 2-week calendar, parallelisable with 0b–0e.
- **Gate YAMLs** for every phase (templates in Appendix B).
- **Baselines** at every phase gate closure.
- **PR drift header** requirement on all refactor PRs.
- **Weekly review** cadence.
- **DoD item 20:** framework self-review at Phase 7 close confirms no unmitigated drift.

### 15.2 Non-conflict

This framework does not override v0.3. It adds a process layer. Where v0.3 locks trait shapes, this framework provides the mechanism (§7). Where v0.3 defines decisions, this framework provides the enforcement (§6).

### 15.3 Cross-references

- v0.3 §9 Determinism ↔ framework §8 Lint L4, §9 Cumulative regression
- v0.3 §16 Decision gates ↔ framework §5 Gate YAMLs, §6 PR rubric
- Implementation plan §10.3 Decisions D1–D11 ↔ framework §6 Drift header, §13 D1 correction
- Implementation plan §10.6 DoD item 19 ↔ framework §10 Weekly review, §9 Ledger metric
- Phase 0a matrix §9 A1 clarification ↔ framework §8 Lint L4 enforcement

---

## Appendix A — Baseline snapshot field list (canonical)

```yaml
# All fields are required unless noted `optional`.
# Values of `null` are valid where the phase hasn't yet produced the artefact.

snapshot_version: int  # 1
captured_at: timestamp  # ISO-8601 UTC
captured_by: string     # e.g. "phase-5b-close"
git_sha: string         # commit hash
framework_version: string  # from FRAMEWORK-VERSION.md

workspace:
  crate_count: int
  crate_list: list[string]

tests:
  passing: int
  failing: int
  ignored: int
  duration_seconds: int

metrics:
  intent_hit_rate_first_attempt_pct: float
  intent_hit_rate_two_attempt_pct: float
  scenario_verb_rate_pct: float
  narration_round_trip_ms_p95: int | null
  outbox_drain_lag_ms_p95: int | null
  determinism_harness_coverage_pct: float | null
  round_trip_harness_coverage_ops_tested: int | null

code:
  domain_ops_file_count: int
  custom_operation_impl_count: int
  trait_signature_hashes:
    VerbExecutionPort: string | null       # sha256 of canonical text
    CustomOperation: string | null
    VerbRegistrar: string | null
    GatedVerbEnvelope: string | null
    PendingStateAdvance: string | null
    TransactionScope: string | null
    OutboxDraft: string | null
    OutboxEffectKind: string | null
    VerbExecutionOutcome: string | null
    VerbExecutionContext: string | null
    StateGateHash: string | null

schema:
  migrations_head: int
  entity_tables_with_row_version: int
  entity_tables_total: int
  outbox_tables: list[string]

pattern_b_ledger:
  pattern_a_open: int
  pattern_b_open: int
  total_closed: int
  file_status:
    sem_os_maintenance_ops.rs: "OPEN" | "CLOSED"
    bpmn_lite_ops.rs: "OPEN" | "IN_PROGRESS" | "CLOSED"
    source_loader_ops.rs: "OPEN" | "IN_PROGRESS" | "CLOSED"
    gleif_ops.rs: "OPEN" | "IN_PROGRESS" | "CLOSED"

decisions_honoured:
  D1_row_version_bigint_everywhere: bool | null
  D2_row_version_per_entity_group_rollout: bool | null
  D3_macro_in_dsl_runtime_macros: bool | null
  D4_websocket_per_session: bool | null
  D5_chat_response_narration_populated: bool | null
  D6_no_pg_crud_executor_subcrate: bool | null
  D7_alternate_step_executors_removed: bool | null
  D8_no_new_a1_violations: bool | null
  D9_determinism_harness_every_pr: bool | null
  D10_separate_outbox_table: bool | null
  D11_pattern_b_ledger_exists: bool | null

invariants_honoured:
  closed_loop_rehydrate_after_writes: bool | null
  single_dispatch_site: bool | null
  three_plane_dependency_direction: bool | null
  no_nlp_in_control_plane: bool | null
  no_external_effects_in_verb_bodies_except_ledgered: bool | null
```

---

## Appendix B — Gate YAML templates per phase

All templates are at `docs/refactor-health/gates/templates/`.

### B.1 Phase 0 template (skeleton)

```yaml
gate_version: 1
phase: "0"
phase_name: "Matrix + envelope + harnesses + outbox schema + row-version audit"
depends_on: []
scope_statement: |
  Per implementation plan §13 Phase 0. Deliverables 0a–0f + 0g + 0h.
criteria:
  - id: matrix-reviewed
    description: "Phase 0a ownership matrix reviewed and approved"
    check_command: "test -f docs/todo/phase-0a-ownership-matrix.md"
    status: pending
  - id: envelope-types-compile
    description: "GatedVerbEnvelope + PendingStateAdvance + TransactionScope + OutboxDraft + StateGateHash compile-only in ob-poc-types"
    check_command: "cargo build -p ob-poc-types"
    status: pending
  - id: state-gate-hash-test-vectors
    description: "BLAKE3 canonical encoding test vectors in place"
    check_command: "cargo test -p ob-poc-types --test state_gate_hash_vectors"
    status: pending
  - id: outbox-migration-drafted
    description: "public.outbox table migration compiled (not yet wired to production)"
    check_command: "test -f rust/migrations/129_outbox.sql"
    status: pending
  - id: determinism-harness-compiles
    description: "rust/crates/determinism-harness/ builds green"
    check_command: "cargo build -p determinism-harness"
    status: pending
  - id: round-trip-harness-compiles
    description: "rust/crates/round-trip-harness/ builds green"
    check_command: "cargo build -p round-trip-harness"
    status: pending
  - id: row-version-audit-complete
    description: "docs/todo/row-versioning-audit.md lists every affected entity table"
    check_command: "test -f docs/todo/row-versioning-audit.md"
    status: pending
  - id: framework-active
    description: "Phase 0h framework live — lints, baselines, gate YAMLs in place"
    check_command: "cargo x refactor-health --dry-run"
    status: pending
  - id: phase-0-baseline-captured
    description: "docs/refactor-health/baselines/phase-0-*.yaml exists"
    check_command: "ls docs/refactor-health/baselines/phase-0-*.yaml"
    status: pending
exit_conditions:
  phase_1_unblocked: false   # true only when all above criteria pass + 0g closed
```

### B.2 Phase 0g template (Pattern A)

```yaml
gate_version: 1
phase: "0g"
phase_name: "Pattern A A1 remediation — subprocess → outbox"
depends_on: ["0"]
scope_statement: |
  MaintenanceReindexEmbeddingsOp subprocess spawn moves from execute body
  to OutboxDraft::MaintenanceSpawn. Drainer handler added.
criteria:
  - id: outbox-effect-kind-variant-added
    description: "OutboxEffectKind::MaintenanceSpawn variant exists in ob-poc-types"
    check_command: "cargo test -p ob-poc-types --test outbox_effect_kind_variants"
    status: pending
  - id: subprocess-moved-out-of-execute-body
    description: "MaintenanceReindexEmbeddingsOp contains no Command::new / tokio::process:: in execute_json"
    check_command: "cargo clippy --workspace -- -D custom::external-effects-in-verb"
    status: pending
  - id: drainer-handler-implemented
    description: "outbox_drainer handles MaintenanceSpawn effect kind"
    check_command: "cargo test --test outbox_drainer_maintenance_spawn"
    status: pending
  - id: integration-test-green
    description: "reindex-embeddings verb triggers post-commit subprocess, not in-txn"
    check_command: "cargo test --test reindex_post_commit"
    status: pending
  - id: ledger-row-closed
    description: "pattern-b-a1-remediation-ledger.md §2 row reads CLOSED"
    check_command: "cargo x ledger-check --section 2 --expect CLOSED"
    status: pending
  - id: l4-partial-green-on-file
    description: "Lint L4 passes on sem_os_maintenance_ops.rs"
    check_command: "cargo clippy -p ob-poc --lib -- -D custom::external-effects-in-verb"
    status: pending
```

### B.3 Template structure for Phases 1, 2, 3, 4, 5a–5f, 6, 7

Each phase gate YAML follows the Phase 0 / 0g pattern. Each has:

- `depends_on:` prior phases
- `scope_statement:` one-paragraph phase summary
- Standard criteria: tests pass, clippy clean, determinism harness byte-identical against previous baseline, baseline snapshot captured
- Phase-specific criteria: see implementation plan §3 for each phase's "Regression gate"
- Lint-specific criteria (e.g., Phase 5b requires L2/L5 green; Phase 5f requires L4 green workspace-wide)

Detailed templates authored during Phase 0h Day 3–4; stubs committed then fleshed out per phase.

### B.4 Phase 7 template (final gate)

```yaml
gate_version: 1
phase: "7"
phase_name: "Remove shim, finalise, enforce lints"
depends_on: ["6"]
scope_statement: |
  Delete execute_json_via_legacy shim. Enforce L1-L7. Update CLAUDE.md. 
  Close out three-plane refactor.
criteria:
  - id: shim-deleted
    description: "execute_json_via_legacy() does not exist anywhere in rust/src"
    check_command: "! grep -r 'execute_json_via_legacy' rust/src"
    status: pending
  - id: deprecation-reexport-removed
    description: "sem_os_core::execution does not re-export VerbExecutionPort"
    check_command: "! grep -q 'pub use.*VerbExecutionPort' rust/crates/sem_os_core/src/execution.rs"
    status: pending
  - id: all-lints-green
    description: "L1, L2, L3, L4, L5, L6, L7 all pass"
    check_command: "cargo clippy --workspace -- -D custom::all && cargo deny check"
    status: pending
  - id: claude-md-updated
    description: "CLAUDE.md crate list, trigger-phrase table, principle statement updated"
    check_command: "cargo x claude-md-sync --check"
    status: pending
  - id: dod-item-19-checked
    description: "Pattern B ledger CLOSED + L4 green (v0.3 DoD item 19)"
    check_command: "cargo x ledger-check --expect-closed && cargo clippy -- -D custom::external-effects-in-verb"
    status: pending
  - id: framework-self-review-complete
    description: "Framework self-review confirms no unmitigated drift (DoD item 20)"
    check_command: "test -f docs/refactor-health/framework-self-review-phase-7.md"
    status: pending
  - id: final-baseline-vs-phase-0
    description: "Final baseline diff against Phase 0 is documented and explained"
    check_command: "cargo x refactor-health --final-report"
    status: pending
exit_conditions:
  refactor_done: true
```

---

## Appendix C — Custom clippy lint bodies

All lints implemented in `rust/clippy-lints/src/lib.rs` (new crate; compiled as a dylib loaded by `.cargo/config.toml`).

### C.1 L2 — single_dispatch_site

```rust
// Counts call sites of VerbExecutionPort::execute_json across the
// workspace (excluding #[cfg(test)] blocks). Fails if count > 1.
//
// Implementation: LateLintPass that walks all expressions, identifies
// method calls resolving to VerbExecutionPort::execute_json, and
// increments a workspace-global counter.
//
// Expected sole call site (post Phase 5b):
//   ob-poc::sequencer::stage_8::dispatch_envelope
//
// Escape: none.
```

### C.2 L3 — no_nlp_in_control_plane

```rust
// Denies importing any crate in the NLP/embedding set from any module
// inside rust/crates/sem_os_*/src/ or rust/src/sem_reg/.
//
// Denied crate set (configurable in deny.toml):
//   candle_core, candle_nn, candle_transformers, tokenizers,
//   tiktoken-rs, ob-semantic-matcher
//
// Escape: none.
```

### C.3 L4 — external_effects_in_verb

```rust
// For every type implementing CustomOperation, walks async fn execute
// and async fn execute_json bodies. Fails on any of:
//
//   - Symbols from crates: reqwest, http, hyper, surf, isahc, tonic
//   - std::process::Command, tokio::process::Command
//   - Calls to methods named post, get, put, delete, send, send_request
//     when receiver type is an HTTP client (configurable)
//   - Names ending in Client, Connection, HttpLoader unless they're trait
//     object parameters (e.g., &dyn BpmnLiteConnection is fine;
//     BpmnLiteConnection::new() is not)
//
// Escape: #[allow(external_effects_in_verb)] on the fn requires a
// TODO comment referencing pattern-b-a1-remediation-ledger.md and
// the referenced ledger row must not read CLOSED. The ledger-check
// xtask verifies consistency.
```

### C.4 L5 — no_json_value_at_stage_boundary

```rust
// For functions in rust/src/sequencer/, fails if any parameter or return
// type is serde_json::Value (unwrapped). Allowed: Value wrapped inside
// a dedicated typed newtype.
//
// Escape: #[allow(json_at_stage_boundary)] with justification comment.
```

### C.5 L6 — contract_freeze

```rust
// Reads frozen-contracts.yaml at workspace root. For each (type, 
// frozen_snapshot_sha), computes current snapshot and verifies against
// the banned-change rules for that type.
//
// Banned rules supported:
//   - no-field-removal: struct field removal fails
//   - no-signature-breaking: trait method signature change fails
//   - no-variant-removal: enum variant removal fails
//   - no-public-visibility-downgrade: pub -> pub(crate) fails
//
// Escape: deliberate contract change requires updating
// frozen-contracts.yaml (tracked via PR) AND a companion spec doc update.
```

---

## Appendix D — PR template body

`.github/PULL_REQUEST_TEMPLATE.md`:

```markdown
<!-- DRIFT-HEADER-v1 -->
phase: <0|0g|0h|1|2|3|4|5a|5b|5c|5d|5e|5f|6|7>
scope_statement: |
  <one paragraph describing what this PR does and why it's in this phase>
decisions_touched: []           # e.g., [D3, D7]
contract_shapes_changed: []      # e.g., [VerbExecutionOutcome]
new_dependencies_added: []       # e.g., [blake3]
a1_lint_exceptions_added: []     # entries added in any file under domain_ops
ledger_rows_touched: []          # ledger file sections modified
regression_notes: |
  <expected vs observed baseline/harness/test deltas>
<!-- /DRIFT-HEADER -->

## Summary

<what and why>

## Drift considerations

<anything flagged above that needs reviewer attention>

## Test plan

<how this was verified>

## Checklist

- [ ] Drift header accurate
- [ ] Determinism harness green
- [ ] Lints green (L1-L7 per phase applicability)
- [ ] Contract golden files reviewed if touched
- [ ] Baseline delta warnings acknowledged
- [ ] Ledger updated if A1-relevant
```

---

## Appendix E — Weekly review minutes template

`docs/refactor-health/weekly/<date>.md`:

```markdown
# Weekly Refactor Health Review — <date>

**Attendees:** …
**Framework version:** 0.1
**Current phase:** <phase id>

## 1. Ledger status

| Section | Open | Closed | Δ since last week |
|---|---|---|---|
| Pattern A (§2) | … | … | … |
| Pattern B (§3) — bpmn_lite_ops.rs | … | … | … |
| Pattern B (§3) — source_loader_ops.rs | … | … | … |
| Pattern B (§3) — gleif_ops.rs | … | … | … |

## 2. Baseline delta report

- Report: docs/refactor-health/weekly-<date>.md
- Warn-level items: …
- Fail-level items: …
- Acknowledgments: …

## 3. PRs merged since last review

<list with drift-dimension-flags per PR>

## 4. Drift dimension sweep

- D1 (decision): …
- D2 (scope): …
- D3 (contract): …
- D4 (regression): …
- D5 (gate): …
- D6 (baseline): …
- D7 (invariant): …
- D8 (cumulative): …
- D9 (ledger): …
- D10 (meta): …

## 5. Current phase status

…

## 6. Next phase prep

…

## 7. Escalations

…

## Decisions made this review

- …

## Action items

- [ ] …
```

---

## Appendix F — Drift-indicator metric cheat sheet

Quick-reference for the weekly review:

| Metric | Green | Amber | Red |
|---|---|---|---|
| intent_hit_rate_first_attempt_pct | ≥ baseline - 0.5 | baseline -1.0 to -0.5 | < baseline -1.0 |
| tests.passing | = baseline | baseline - 1 to -5 | < baseline - 5 or any new failure |
| determinism_harness_coverage_pct | = baseline | baseline - 0.1 | < baseline - 0.1 |
| narration_round_trip_ms_p95 | < baseline × 1.1 | baseline × 1.1 – 1.5 | > baseline × 1.5 |
| pattern_b.open (Phase 5f) | monotonically decreasing | flat for 1 week | flat for 2 weeks or increasing |
| gate YAMLs closed in order | yes | a skip noted and justified | a skip unexplained |
| framework amendments | 0 per phase | ≤ 1 per phase | > 1 per phase |
| weekly reviews held | 100% | 90-100% | < 90% |

---

**End of framework v0.1. Awaiting peer review.**
