# Capability-Crate Restructure v1

**Status:** Draft for review. Not yet acted on.
**Author:** Claude (with Adam, 2026-05-13)
**Scope:** Refactor the rust/crates/ layout so each crate states a **capability claim** — what it *does* — rather than a dependency tier. All capability crates are consumed by `ob-poc` (the application).

---

## 1. Discipline (the rule that should have applied from slice 2u)

A crate exists when there is a coherent **capability** worth naming. The bar:

1. **One-line charter.** "This crate does X." If we can't write one sentence, the crate shouldn't exist.
2. **Public surface = capability surface.** The `pub` items are exactly the API a consumer needs to invoke the capability. Anything else is `pub(crate)`.
3. **Minimal inter-crate dependencies.** Capability crates do not import each other unless one capability genuinely depends on another. The application is the integrator, not the crates.
4. **No "boundary-tier" or "depends on nothing" framing.** That's a side-effect of correct capability scoping, not the goal.

Slices 2u → 2dd violated rule 1: I packed `ob-poc-envelope` with anything that compiled cleanly, regardless of whether it belonged to "envelope construction." That's the drift this plan corrects.

---

## 2. Target capability map

### 2.1 Existing crates (charters tightened)

| Crate | Charter |
|-------|---------|
| `ob-poc-types` | **Shared DTOs that cross capability boundaries.** Pure data types (chat, decision, disambiguation, gated_envelope, narration, orientation, graph_scene, session_input, onboarding_state, viewport). No logic, no DB, no execution. |
| `ob-poc-diagnostics` | **Error types + event infrastructure.** `DSLError`, `ParseError`, `DslEvent`, `EventPayload`, `ErrorSnapshot`, the drain task. |
| `ob-poc-macros` | **Proc macros.** `#[derive(IdType)]`. Tooling tier. |
| `dsl-core` | **DSL parser + AST + verb config + ConfigLoader.** No DB, no runtime. |
| `dsl-runtime` | **DSL execution traits + cross-workspace runtime + tx scope.** |
| `dsl-lsp` | **DSL language server + tree-sitter grammar + Zed extension.** |
| `sem_os_*` (6 crates) | **Semantic OS registry, store, server, client, harness, adapter.** Unchanged. |
| `ob-agentic` | **LLM client + lexicon + intent parsing.** Unchanged. |
| `ob-semantic-matcher` | **Candle embeddings + vector search.** Unchanged. |
| `ob-templates` | **Template definitions + expander.** Unchanged. |
| `ob-workflow` | **Workflow task queue + listener.** Unchanged. |
| `entity-gateway` | **Fuzzy entity lookup gRPC service.** Unchanged. |
| `inspector-projection` | **Projection schema generator.** Unchanged. |
| `playbook-core` / `playbook-lower` | **Playbook authoring.** Unchanged. |
| `governed_query_proc` | **Proc macro for governed queries.** Unchanged. |
| `determinism-harness`, `round-trip-harness`, `dsl_types` | **Test/dev harnesses.** Unchanged. |
| `ob-poc-web` | **Axum web server.** Unchanged. |

### 2.2 New crates

| Crate | Charter | Initial modules |
|-------|---------|-----------------|
| `ob-poc-sage` *(new)* | **Sage intent understanding — utterance → structured intent.** No DSL assembly, no execution. | `plane`, `polarity`, `outcome`, `context`, `pre_classify`, `coder_result`, `disposition`, `verb_resolve_types`, `session_context`. Later: `coder`, `verb_resolve`, `verb_index`, `arg_assembly`, `clash_matrix`, `deterministic`, `llm_sage`, `valid_verb_set`, `constrained_match`. |
| `ob-poc-journey` *(new)* | **Pack-guided workflow definitions.** Pack manifests, FSM, handoff. The "what the user is doing" layer. | `pack`, `pack_state`, `handoff`. Later: `pack_manager`, `router`, `playback`, `template`. |
| `ob-poc-domain` *(new)* | **Domain DTOs.** Pure data shapes for the business domains. Reference data, not execution. | `booking_principal_types`, `bods_types`, `deal_types`, `trading_profile`, `taxonomy`, `semtaxonomy`, `ontology`, `derived_attributes`, `view_config_service`, `entity_linking`. |
| `ob-poc-authoring` *(new)* | **Editor / authoring surface.** Tools that serve the human author: clarification, lexicon lookup, lint, macro registry, data dictionary, display nouns, feedback inspector. | `clarify`, `lexicon`, `macros`, `lint`, `data_dictionary`, `display_nouns`, `feedback`, `language_pack`. |

### 2.3 Tightened envelope charter

`ob-poc-envelope` shrinks to its real capability:

> **The typed contract between Sage and execution.** Envelope construction, TOCTOU recheck, approval tokens, audit chain, workbook DTOs, LLM trace hashing, DSL coder output binding, mutation pre-flight, gate policy, session-input draft mode, ACP discovery projection.

Modules to KEEP in envelope:
- `envelope_builder`, `toctou_recheck`, `approval_token`, `audit_chain`, `mutation_preflight`
- `workbook`, `workbook_diagnostics`, `workbook_revision`
- `llm_trace`, `dsl_coder`, `kyc_dry_run`
- `policy` (PolicyGate)
- `session` (AgentMode, WorkspaceKind, SubjectKind, WorkspaceRegistryEntry, SessionInputDraftMode)
- `session_trace`
- `acp`, `acp_facade`, `acp_protocol`, `acp_runtime_context`, `acp_dag_semantic`, `acp_pack_context_envelope_v2`, `acp_registry_projection`, `acp_session_input_draft_mode`, `acp_state_anchor`
- `traceability::types` (only the typed DTO subset already extracted in slice 2h)
- `advisory_lock` *(may relocate to `ob-poc-infra` or stay if it's only used by envelope-tier persistence)*

Modules to MOVE OUT of envelope:
- → `ob-poc-sage`: `sage/*`
- → `ob-poc-journey`: `journey/*`
- → `ob-poc-domain`: `booking_principal_types`, `bods_types`, `deal_types`, `trading_profile`, `taxonomy`, `semtaxonomy`, `ontology`, `derived_attributes`, `view_config_service`, `entity_linking`
- → `ob-poc-authoring`: `clarify`, `lexicon`, `macros`, `lint`, `data_dictionary`, `display_nouns`, `feedback`, `language_pack`

---

## 3. Dependency graph (target)

```
ob-poc-web         (axum server binary)
        │
        ▼
  ob-poc (application — orchestrator, REPL, sequencer, runbook, MCP, REST,
          BPMN integration, calibration, research, domain_ops, services,
          sem_reg, agent, lookup, mcp, plan_builder)
        │
        ├── ob-poc-sage          ──┐
        ├── ob-poc-journey       ──┼── ob-poc-types ── ob-poc-diagnostics
        ├── ob-poc-domain        ──┤
        ├── ob-poc-authoring     ──┘
        ├── ob-poc-envelope      ── ob-poc-sage*, ob-poc-journey*, ob-poc-types
        ├── dsl-runtime          ── dsl-core ── ob-poc-types
        └── sem_os_postgres      ── sem_os_core ── ob-poc-types
```

\* envelope's ACP projection currently needs pack manifest types (slice 2d.2 reason) and some Sage types (or it just hands FQNs back and lets consumers pull sage themselves — TBD, see §6).

**Acyclic check** — only one direction: `ob-poc` → capability crates → primitives (`ob-poc-types`, `ob-poc-diagnostics`). Capability crates do NOT reach across to each other except envelope → journey (load-bearing, one edge) and possibly envelope → sage (TBD).

---

## 4. Drift census (current envelope contents)

Modules currently in `ob-poc-envelope/src/` and their target home:

| Module | Current | Target | Reason |
|--------|---------|--------|--------|
| `envelope_builder` | envelope | **envelope** | core charter |
| `toctou_recheck` | envelope | **envelope** | core charter |
| `approval_token` | envelope | **envelope** | core charter |
| `audit_chain` | envelope | **envelope** | core charter |
| `mutation_preflight` | envelope | **envelope** | core charter |
| `workbook`/`workbook_diagnostics`/`workbook_revision` | envelope | **envelope** | core charter |
| `llm_trace` | envelope | **envelope** | core charter |
| `dsl_coder` | envelope | **envelope** | core charter |
| `kyc_dry_run` | envelope | **envelope** | core charter |
| `policy` | envelope | **envelope** | core charter |
| `session` | envelope | **envelope** | core charter |
| `session_trace` | envelope | **envelope** | core charter |
| `acp*` (9 modules) | envelope | **envelope** | core charter |
| `traceability::types` | envelope | **envelope** | core charter |
| `advisory_lock` | envelope | **envelope** (or `ob-poc-infra`) | DB plumbing — TBD |
| `sage/*` (9 modules: plane, polarity, outcome, context, pre_classify, coder_result, disposition, verb_resolve_types, session_context) | envelope | **ob-poc-sage** | wrong capability |
| `journey/*` (3 modules: pack, pack_state, handoff) | envelope | **ob-poc-journey** | wrong capability |
| `booking_principal_types` | envelope | **ob-poc-domain** | wrong capability |
| `bods_types` | envelope | **ob-poc-domain** | wrong capability |
| `deal_types` | envelope | **ob-poc-domain** | wrong capability |
| `trading_profile` | envelope | **ob-poc-domain** | wrong capability |
| `taxonomy` | envelope | **ob-poc-domain** | wrong capability |
| `semtaxonomy` | envelope | **ob-poc-domain** | wrong capability |
| `ontology` | envelope | **ob-poc-domain** | wrong capability |
| `derived_attributes` | envelope | **ob-poc-domain** | wrong capability |
| `view_config_service` | envelope | **ob-poc-domain** | wrong capability |
| `entity_linking` | envelope | **ob-poc-domain** | wrong capability |
| `clarify` | envelope | **ob-poc-authoring** | wrong capability |
| `lexicon` | envelope | **ob-poc-authoring** | wrong capability |
| `macros` | envelope | **ob-poc-authoring** | wrong capability |
| `lint` | envelope | **ob-poc-authoring** | wrong capability |
| `data_dictionary` | envelope | **ob-poc-authoring** | wrong capability |
| `display_nouns` | envelope | **ob-poc-authoring** | wrong capability |
| `feedback` | envelope | **ob-poc-authoring** | wrong capability |
| `language_pack` | envelope | **ob-poc-authoring** | wrong capability |

**Net effect:** envelope sheds ~28 modules and retains ~22. Three new crates absorb the drift.

---

## 5. Migration plan

### Phase R: Review (now)
Adam reviews this plan; we agree on charters and dependency edges before any code moves.

### Phase 1: Create new capability crate skeletons
For each of `ob-poc-sage`, `ob-poc-journey`, `ob-poc-domain`, `ob-poc-authoring`:
1. `rust/crates/<crate>/Cargo.toml` with minimum deps (likely just `ob-poc-types`, `serde`, `chrono`, `uuid`).
2. Empty `src/lib.rs` with the charter as the module doc.
3. Add to workspace members in `rust/Cargo.toml`.
4. Add path dep to `ob-poc` (the application).
5. Verify each crate builds empty.

One commit per crate creation.

### Phase 2: Move sage/* out of envelope into ob-poc-sage
9 modules. Inter-module dep direction: `outcome` depends on `plane` + `polarity`; `disposition` depends on `coder_result` + `outcome`; `verb_resolve_types` depends on `coder_result`; everything else is independent or already depends on the listed siblings.

Sub-slices (one commit each):
- 2.1 — `plane`, `polarity` (no inter-sibling deps)
- 2.2 — `outcome`, `context`
- 2.3 — `coder_result`, `verb_resolve_types`, `disposition`
- 2.4 — `pre_classify`, `session_context`

After phase 2: `ob-poc-envelope::sage::*` no longer exists. The compat re-export in `src/sage/mod.rs` points to `ob_poc_sage::*` instead. All call sites unchanged.

### Phase 3: Move journey/* out of envelope into ob-poc-journey
- 3.1 — `pack`, `handoff`, `pack_state`

Envelope's ACP projection imports `journey::pack` types; rewrite to import from `ob_poc_journey::pack` directly (envelope → journey dependency edge introduced here, one edge only).

### Phase 4: Move domain DTOs out of envelope into ob-poc-domain
10 modules. Sub-slices by dependency cluster (each commit):
- 4.1 — `booking_principal_types`, `bods_types`, `deal_types` (independent)
- 4.2 — `ontology`, `taxonomy`, `semtaxonomy` (taxonomy depends on view_config_service, semtaxonomy is independent)
- 4.3 — `view_config_service`, `derived_attributes` (DB-coupled)
- 4.4 — `trading_profile`, `entity_linking`

### Phase 5: Move authoring modules out of envelope into ob-poc-authoring
8 modules. Sub-slices:
- 5.1 — `clarify`, `data_dictionary`, `display_nouns`
- 5.2 — `lexicon`, `macros`, `lint`
- 5.3 — `language_pack`, `feedback`

### Phase 6: Tighten public surfaces
For each capability crate, audit `pub`:
- Anything not used outside the crate → `pub(crate)`.
- Anything only used by tests → `#[cfg(test)]`.
- Anything intended as the public API → keep `pub` and document it in the crate header.

Add `#![deny(unreachable_pub)]` to each crate's `lib.rs`.

### Phase 7: Audit cyclic deps + dead deps
Run `cargo tree` per crate; verify no capability crate imports another except the documented edges (envelope → journey, possibly envelope → sage). Remove any leftover dev/dead deps.

---

## 6. Decisions (locked 2026-05-13)

1. **envelope → sage edge: OPAQUE.** ACP exposes pack/verb FQNs, policy reasons, state-hash digests — never `OutcomeIntent` / `ObservationPlane` / Sage taxonomy. Locks in Sage refactor budget; the wire protocol does not move when Sage's internal vocabulary changes.

2. **envelope → journey edge: BREAK.** Envelope owns its own `PackProjection { fqn, allowed_verbs, mode_tags, … }`. The projection function (`fn from(pack: &PackManifest) -> PackProjection`) lives in `ob-poc` (the application). Envelope no longer reaches journey for types.

3. **No `ob-poc-infra` crate.** Helpers like `advisory_lock` go with their primary consumer (`derived_attributes` → `ob-poc-domain`). `view_config_service` goes with `taxonomy`. If a helper proves cross-cutting later, extract then.

4. **`ob-poc` stays as the single application crate.** A crate is for reusable capability; the application is the consumer, not a capability. After the 28 misplaced modules leave for capability crates, `ob-poc` is the integrator (orchestrator + sequencer + REPL + REST + MCP + BPMN). Internal `src/` module discipline gives the boundaries.

5. **`ob-poc-types` absorbs cross-capability shared DTOs, by rule.** Audit per-DTO: referenced by ≥2 capability crates → hoist to `ob-poc-types`. Referenced by only one capability crate → stays there. Avoids `ob-poc-types` becoming the new dumping ground.

6. **Sage = types-crate + app-side engines.**
   - **`ob-poc-sage` (capability crate):** pure Sage vocabulary + the deterministic classifier — the 9 already-relocated modules + the small pure engines (`clash_matrix`, `arg_assembly`, `coder`, `verb_resolve`, `verb_index`) once we extract their `mcp::intent_pipeline` dep.
   - **Stays in `ob-poc`:** `valid_verb_set` (pulls `sem_os_runtime` + `database` + `agent::learning` + `mcp::verb_search`), `llm_sage` (LLM-client wiring), `deterministic` (trait impl wrapping the lot). These are the *Sage application* — wiring the capability into the execution tier.
   - Reason: capability crate keeps minimal deps (`dsl-core` / `dsl-runtime` / `ob-poc-types`); tangled adapters stay where they can reach everything.

7. **Rename `ob-poc-envelope` → `ob-poc-boundary`.** Names what the crate is *for*, not what it *contains*. The boundary between intent (Sage) and execution (sequencer); the surface where ACP discovery, the policy gate, and the audit chain meet. "Envelope" is one artifact at the boundary, not the capability itself.

---

## 7. What this plan does NOT do (yet)

- Touch `ob-poc` internal modules outside the drift list (no relocation of `agent/`, `repl/`, `runbook/`, `sequencer/`, `mcp/`, `domain_ops/`, etc.).
- Renumber slices 2u–2dd. They are committed and the relocations they performed will be picked up unchanged when the destination crate moves out from under boundary.
- Address per-domain crate split (item 4 from the original draft) — deferred to v2.

---

## 8. Decision record

**Approved by:** Adam (conversation 2026-05-13)
**Approved on:** 2026-05-13

---

## 9. Post-bed-in review checkpoint (v2)

This v1 plan draws capability boundaries based on the best information we have *today*. Some boundaries will only feel right (or wrong) once the application has lived inside the new shape for a few weeks. A v2 review will happen after the new structure beds in.

**When to trigger:** ~4 weeks after Phase 7 completes, OR when any of the warning signs below appear earlier.

**What to look for:**
1. **Too-small crates** — any crate whose lib.rs is <300 LOC of real content. Candidate for fold-back into a sibling capability or `ob-poc`.
2. **Too-large crates** — any crate whose surface is incoherent or whose changes routinely touch the whole crate. Candidate for split (esp. `ob-poc-domain` per §6 item 4).
3. **Friction at the boundary → journey break** (§6 item 2). If projecting `PackManifest → PackProjection` in the app turns into a maintenance burden vs. the leak it prevents.
4. **Friction at the boundary → sage opaqueness** (§6 item 1). If ACP consumers are constantly asking for Sage-shape data and the app has to keep adding projection surfaces.
5. **Engines that didn't move to `ob-poc-sage`** (§6 item 6). If `valid_verb_set` / `llm_sage` / `deterministic` end up being reused outside ob-poc — promote.
6. **Helpers that did fold into domain** (§6 item 3). If `advisory_lock` or `view_config_service` get a second non-domain consumer — extract then.
7. **Dependency edges that crept in** that aren't documented in the §3 graph. Run `cargo tree` per capability crate; the only allowed targets are `ob-poc-types`, `ob-poc-diagnostics`, `dsl-core`, `dsl-runtime`, and (where documented) one other capability crate.

**What v2 is NOT:** a chance to second-guess the v1 charters wholesale. The aim is to converge, not to re-design. Hard cap of 4 weeks of v2 effort.

**Tracking:** When v2 starts, append a §10 with v2 findings + decisions; preserve §6 untouched so the original reasoning stays auditable.
