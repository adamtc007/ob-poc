# Charter Reconciliation v1

**Status:** Draft for review. Reconciliation only — no code changes proposed here.
**Author:** Claude (with Adam, 2026-07-01)
**Scope:** Reconcile the two authored restructure plans — `capability-crate-restructure-v1.md` (2026-05-13) and `ob-poc-domain-split-v1.md` (2026-05-14) — against the workspace at HEAD. Produce the **intended public surface (charter) per crate** and mark landed / drifted / not-started status. This is the *budget* against which a `cargo public-api` sweep (the *spend*) is diffed. The delta is the tightening + extraction backlog.

The discipline is not re-litigated here; it is `capability-crate-restructure-v1.md` §1. One-line charter per crate; public surface = capability surface; everything else `pub(crate)`.

---

## 0. Headline

Three restructures are in flight simultaneously, not one. The pub audit measured none of their structure:

1. **ob-poc-domain split v1** — structurally landed. The monolithic `ob-poc-domain` was deleted 2026-05-14; 9 DTO/reference crates exist.
2. **Capability restructure v1** — partially landed. `ob-poc-sage`, `ob-poc-journey`, `ob-poc-agent` are **populated, not skeletons** — but `ob-poc`'s `lib.rs` still declares `pub mod sage/journey/agent`. Migration completeness is unverified per module.
3. **Unified DSL v0.1 / v0.2** — separate, partly placeholder. `dsl-resolution`, `dsl-lowering`, the frontends, `dsl-render`, `dsl-migrate*` are tranche placeholders per `Cargo.toml`.

The single most load-bearing charter is `ob-poc`'s own (§4). Measured against the project's own rule, its re-export hub should not exist.

---

## 1. Data / reference-data crates (domain-split v1 — structurally LANDED)

Pure data shapes. Charter surface = the DTO set, nothing else. These should snapshot small and clean under `cargo public-api`; any function, DB accessor, or execution type in the surface is a violation.

| Crate | Charter (one line) | LOC | Status / flag |
|---|---|---|---|
| `ob-poc-types` | Shared DTOs that cross capability boundaries. No logic, no DB, no execution. | — | Landed |
| `ob-poc-diagnostics` | Error types + event infrastructure (`DSLError`, `ParseError`, `DslEvent`, drain task). | — | Landed |
| `ob-poc-bods` | BODS 0.4 / LEI spine reference standard. | 218 | Landed |
| `ob-poc-deal` | Deal taxonomy / fee-billing lifecycle DTOs. | 287 | Landed |
| `ob-poc-trading-profile` | Largest single business capability; mostly self-contained. | 5,632 | Landed |
| `ob-poc-taxonomy` | Vocabulary/layout infrastructure (taxonomy + `view_config_service`, forced pairing). | 6,470 | Landed |
| `ob-poc-ontology` | Lifecycle-stage definitions (distinct from taxonomy combinators). | 1,242 | Landed |
| `ob-poc-semtaxonomy` | Entity-extraction layer. | 514 | Landed |
| `ob-poc-derived-attributes` | Canonical derived-value plane (+ `advisory_lock`, forced pairing). | 829 | Landed |
| `ob-poc-entity-linking` | Mention extraction / resolution. | 1,664 | Landed |

**RESOLVED — booking_principal_types (2026-07-01, git-traced).** The plan's `ob-poc-booking-principal` *did* land standalone as specified (2026-05-14). The entire booking-principal feature was then deliberately deleted (2026-06-15) with its migrations. There is nothing to reconcile; the earlier DRIFT flag was stale. No action.

---

## 2. Capability crates (restructure v1 — PARTIALLY LANDED)

| Crate | Charter (one line) | HEAD state | Status / flag |
|---|---|---|---|
| `ob-poc-boundary` | The execution-side gate: envelope construction, TOCTOU recheck, approval tokens, audit chain, workbook DTOs, LLM-trace hashing, policy gate, ACP discovery projection. | Named re-exports at root | Landed. **LEAK:** verify no draft/runbook state leaks in (that belongs to `dsl-lsp` per §1.5 of the plan). |
| `ob-poc-sage` | Sage drafter: produces runbook drafts from utterances + SemOS knowledge; owns the drafter type vocabulary + deterministic classifier. | `context, disposition, drafter_result, engine, outcome, plane, polarity, pre_classify, session_context, valid_verb_set, verb_resolve_types` | Phase 2 largely landed (incl. the `coder_*` → `drafter_*` rename). **DRIFT:** `ob-poc` still `pub mod sage`. Verify shim, not duplicate. |
| `ob-poc-journey` | Pack-catalogue knowledge: pack manifests, FSM, handoff. Disk-loader of record today; a SemOS-served catalogue provider tomorrow. | `pack, pack_state, handoff` | Initial slice landed. **DRIFT:** `ob-poc` still `pub mod journey`. Verify shim. |
| `ob-poc-agent` | Goal/constellation planning agent: goal frames, frontier, blockers, motivation, approval, planning, MCP/LSP/REPL channels. | 15 modules, substantial | Landed (not in the original §2 table — charter needs ratifying). **DRIFT:** `ob-poc` still `pub mod agent`. Verify shim. |
| `ob-poc-authoring` | Editor / authoring surface: clarify, lexicon, macros, lint, data_dictionary, display_nouns, feedback, language_pack. | Re-exported by `ob-poc` | Landed. |

The three DRIFT rows are the whole "are we done" question. Each needs one check: is `ob-poc/src/<mod>/mod.rs` a thin `pub use ob_poc_<crate>::*` compat shim (migration done, shim retire-able) or does it still contain live code (migration incomplete)? A `cargo public-api` snapshot of each capability crate plus a grep of `ob-poc/src/<mod>` answers it deterministically.

### 2.6 Shim-vs-dup verdicts (resolved 2026-07-01)

The grep is done. None of the three is a clean shim deletion; each is a distinct decision.

| Module | Verdict | Action class |
|---|---|---|
| `ob-poc::sage` | **Hybrid, migration incomplete.** Re-exports the type-vocabulary leaves + `SageEngine` from `ob_poc_sage`; still holds ~9 live local `pub mod`s — the drafter stages (`coder`, `verb_resolve`, `arg_assembly`, `clash_matrix`, `deterministic`, `llm_sage`, `constrained_match`, …), i.e. the "Later:" slice of the 2026-05-13 plan that never landed. | **Phase reopen** — finish moving the drafter stages into `ob-poc-sage`. Not a Phase 2 tightening item. |
| `ob-poc::journey` | **Hybrid, documented as staged.** Re-exports the 3 DTO leaves (`pack`, `pack_state`, `handoff`); `pack_manager`, `playback`, `router` remain live local. `providers::register_pack_providers()` is binary startup wiring — integration glue that legitimately stays application-side. | **Move decision** on `pack_manager`/`playback`/`router` (finish 3C); **keep** `providers`. |
| `ob-poc::agent` | **Name collision, not a migration.** Zero re-exports from `ob_poc_agent`; 12 live local modules (`composite_state`, `learning`, `narration_engine`, `orchestrator`, `telemetry`, `verb_surface`, …) — a *learning/telemetry/orchestration* capability. The `ob-poc-agent` crate is a disjoint *goal/constellation planner* (`goal_frame`, `constellation`, `frontier`, `blockers`, `motivation`, `approval`, `planning`). The shared name is the only overlap. | **Rename/disambiguate** (the §2 DRIFT flag was wrong — corrected here), then decide app-layer vs. own crate for the learning module. |

Consequence for §5: Phase 2.5 (shim retirement) does **not** collapse into the Phase 2 tightening pass. It carries three design decisions — reopen `sage`, finish `journey` 3C, rename `agent` — none of them mechanical. Schedule 2.5 as its own reviewed pass, not an AGY sweep.

---

## 3. Infra / tooling / DSL / sem_os / bpmn (charter = name; audit these last)

Charters here are self-evident from the plan §2.1 ("unchanged") and the crate names. They are lower priority for the diff because their surfaces are already scoped or they are leaf crates.

**Infra / tooling:** `ob-poc-macros` (proc macros), `ob-poc-web` (Axum server), `ob-poc-compiler`, `ob-semantic-matcher` (Candle embeddings + vector search), `ob-templates`, `ob-workflow` (task queue + listener), `entity-gateway` (fuzzy entity lookup gRPC), `inspector-projection` (projection schema generator), `playbook-core`, `ob-agentic` (LLM client + lexicon + intent parsing), `ob-poc-manifest-export`, `ob-poc-bus-handler`.

- **LEAK — `entity-gateway`.** Root re-exports `TantivyIndex` by name (`pub use index::{IndexRegistry, TantivyIndex}`). That leaks the index-engine choice (Tantivy) into the gateway's public API. `unreachable_pub` cannot see this; `cargo public-api` flags it. Candidate for `pub(crate)` unless a consumer genuinely constructs a `TantivyIndex`.

**Unified DSL v0.1 / v0.2 (separate refactor):** `dsl-core`, `dsl-runtime`, `dsl-analysis`, `dsl-atoms`, `dsl-diagnostics`, `dsl-parser`, `dsl-ast`, `dsl-resolution` *(placeholder)*, `dsl-lowering` *(placeholder)*, `dsl-semos-frontend` *(placeholder)*, `dsl-bpmn-frontend` *(placeholder)*, `dsl-sage`, `dsl-migrate` *(placeholder)*, `dsl-migrate-verify` *(placeholder)*, `dsl-render` *(placeholder)*, `dsl-lsp`.

- **INVERTED EDGE — `dsl-lsp`.** Depends on `ob-poc` (flagged in restructure-v1 §1.5 as an out-of-scope follow-on). The LSP server should be reusable by Sage *and* human editors; it should not depend on the application. Uninvert before treating `dsl-lsp`'s surface as settled.

**sem_os:** `sem_os_postgres`, `sem_os_server`, `sem_os_client`, `sem_os_obpoc_adapter`, `sem_os_harness`, `sem_os_mcp`. (`sem_os_policy` already extracted to `github.com/adamtc007/sem-os` v0.1.1.)

**bpmn:** `bpmn-controller` (pool lifecycle + instance kick-off), `bpmn-runtime`, `bpmn-test-harness`.

---

## 4. `ob-poc` (`.`) — the application, and the crux

Restructure-v1 §1 is explicit: *"All capability crates are consumed by `ob-poc` (the application). The application is the integrator, not the crates."*

That fixes `ob-poc`'s charter and settles the facade question the pub audit left open:

> **`ob-poc` is the application / integrator crate. Its intended library surface is approximately empty.** It exists to wire capability crates into binaries (`dsl_cli`, `dsl_mcp`, `repl`, web). It is not a library that downstream code links for capability access.

Two consequences follow directly, and neither is discretionary under the project's own rule:

**a. The re-export hub is a charter violation, not a curated facade.** `lib.rs` currently re-exports ~19 capability crates (`pub use ob_poc_boundary::acp`, `ob_poc_ontology as ontology`, `ob_poc_taxonomy::taxonomy`, `ob_poc_authoring::{data_dictionary,lint,macros,lexicon,clarify,feedback}`, `ob_poc_trading_profile`, `ob_poc_derived_attributes`, `ob_poc_entity_linking`, `ob_poc_semtaxonomy`, `ob_poc_diagnostics::{error,events}`, …). An application does not re-publish its libraries. These are compat shims. Every one keeps a consumer reachable via `ob_poc::taxonomy` instead of `ob_poc_taxonomy`, which is exactly why the domain-split crates are still namespaces rather than boundaries. Retire them by migrating downstream imports to source crates, then deleting the `pub use`. The `cargo tree -i <crate>` test tells you when a crate has no inbound edge except `ob-poc` — that crate's re-export is the only thing keeping the split cosmetic.

**b. The ~30 local `pub mod`s split into two piles.** Legitimately application-layer (keep, but they can be `pub(crate)` — the application has no external library consumer): `api`, `mcp`, `database`, `services`, `session`, `graph`, `navigation`, `sequencer` / `sequencer_tx` / `sequencer_stages`, `outbox`, `bpmn_integration`, `plan_builder`, `runbook`, `repl`, `service_resources`, `traceability`, `sem_os_runtime`, `sem_reg`, `calibration`, `lookup`, `gleif`, `research`, `journey` (integration glue), `templates`, `domains`. Already-homed elsewhere (verify shim vs un-migrated): `sage`, `agent`, `journey`, `domain_ops`, `dsl_v2` (analyser tier now in `dsl-analysis`/`dsl-runtime`), `semtaxonomy_v2`.

Since `ob-poc`'s intended external library surface is ~empty, the correct workspace-level move is arguably to stop building `ob-poc` as a `lib` target for external linking at all, or to gate its lib behind an explicit `#![deny(unreachable_pub)]` + a `cargo public-api` snapshot pinned to "application-internal, no semver." That converts the whole hub question into a compiler-enforced invariant instead of a review convention.

---

## 5. Diff protocol (how this budget meets the sweep)

1. AGY regenerates the *spend* against HEAD — `cargo public-api` per crate to `audits/surface/<crate>.txt`; `cargo tree -i <crate>` per capability crate; layering-leak scan. No cached logs; regenerate (E7).
2. For each crate: `spend − charter = unearned surface`. That list is the tightening backlog per crate.
3. For each capability crate with a DRIFT flag (§2): grep `ob-poc/src/<mod>` for live code. Shim → schedule shim deletion. Live code → migration is incomplete; that's a Phase reopen, not a tightening.
4. `cargo tree -i` with only an `ob-poc` inbound edge → that re-export in `ob-poc/lib.rs` (§4a) is the sole thing preventing the split from being real. Migrate + delete.
5. Commit `audits/surface/*.txt` as the baseline; the CI gate is a snapshot diff, not a warning count.

The output of steps 2–4 is the actual extraction/tightening plan the pub audit was reaching for and missed.

---

## 6. Sweep reconciliation — spend vs budget (2026-07-01, HEAD `86031a08`)

Phase 1 ran green. 56 surfaces written; 54 measured under `--all-features`; `ob-poc-web` and `xtask` have no `[lib]` target (binary-only) — expected, not gaps. Deltas against the budget:

### 6.1 CONFIRMED — the split is graph-cosmetic (as predicted §4a)

9 of 13 named crates are **HUB-ONLY** — their only inbound workspace edge is `ob-poc`: `ob-poc-authoring`, `-bods`, `-deal`, `-trading-profile`, `-taxonomy`, `-ontology`, `-semtaxonomy`, `-derived-attributes`, `-entity-linking`. The entire domain-split + authoring tier is a namespace, not a boundary, at the dependency-graph level. Real decoupling is entirely deferred to the Phase 3 hub dismantle; the split as it stands bought directory hygiene and nothing at the graph.

`ob-poc-agent`'s only direct dependent is `xtask` — **not** `ob-poc`. This confirms §2.6: `ob-poc::agent` (learning/telemetry) and the `ob-poc-agent` crate (goal/constellation planning) are disjoint code sharing a name. Name-collision call holds.

### 6.2 RESOLVED — the "leak" is an off-by-default membrane, not a default-surface violation

Checked at HEAD: all seven flagged crates (`ob-poc-derived-attributes`, `-diagnostics`, `-entity-linking`, `-taxonomy`, `-trading-profile`, `-authoring`, `-sage`) are `default = []`, `database = ["dep:sqlx"]`, with `sqlx` marked `optional = true`. The `PgPool` surface exists **only under `--all-features`**; in a default build these crates are pure data and pull no sqlx. The sweep ran `--all-features`, so it saw the maximal (membrane-on) surface. `ob-poc-diagnostics` documents this in-file: the feature gates the DB-backed event sinks (`PgEventStore`, `session_log`) and the DB `DSLError` variants, aligned with `ob-poc`'s own `database` feature.

So the §1 "pure data" charter is **true for the default surface** and false only for the opt-in membrane. This is a legitimate data-crate-with-optional-store pattern, not a boundary violation.

**Decision (2026-07-01): amend + gate + defer. No extraction now.**
1. **Amend §1** to read: *pure data in the default surface; an optional, off-by-default `database` feature adds `PgPool`-taking loaders/sinks.* Zero code churn.
2. **Gate the default surface** in CI: `cargo public-api -p <crate>` at **default features** must contain no `sqlx`/`PgPool`/`PgConnection` symbol. This is the cheap invariant that protects the true charter (pure-by-default) and catches any future *un-gated* leak — which would be the real violation.
3. **Defer the data/store split.** The cleaner shape (DTO crate pure; a `-store` crate owns the `database` path) matches the existing `ob-poc-kyc-store` precedent, and the domain crates diverge from it. But they are HUB-ONLY (§6.1) with no direct DB consumer today, so extracting stores now is speculative. Trigger: extract a per-domain `-store` when a *second, direct* consumer of the crate's `database` path appears (i.e. at or after hub-dismantle). Recorded as an accepted, documented divergence, not a bug.

### 6.3 SEQUENCING CONSTRAINT — conditional on the `database` feature

The compounding risk from §6.1 × the membrane is now precise: a downstream binary that imports `ob_poc_taxonomy` (etc.) directly *without* `database` inherits **no** sqlx coupling. The dormant→live coupling only fires if the consumer enables `database`. So the Phase 3 rule is: when retiring a crate's hub re-export, migrate its **type** consumers to the crate at default features (pure), and route any **loader** consumer to the `database`-feature path (today the app's `database` module; tomorrow a `-store` per the §6.2 trigger). Do not propagate the `database` feature to consumers that only need the types. The membrane, not the crate, is what must not leak across the hub dismantle.

### 6.4 SEPARATE — dsl-analysis transport leak (DSL-tier, not this pass)

`dsl-analysis` exposes `tonic::transport::channel::Channel` in its surface. Unlike the `PgPool` case, an analysis crate has no obvious reason to carry a gRPC transport type, and this sits in the same DSL tier as the known inverted edge (`dsl-lsp → ob-poc`). Treat as part of the Unified-DSL boundary work, not the domain-crate reconciliation. Check whether it is feature-gated; if not, it is a genuine leak.

### 6.4 Phase 2 unaffected

`unreachable_pub` narrowing operates on the *unreachable* surface; every finding in §6.2 is *reachable* pub and untouched by it. Phase 2 remains mechanical and ready; it does not gate on, and is not gated by, the `PgPool` decision.
