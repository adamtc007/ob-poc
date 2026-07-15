# CLAUDE.md

> **Last reviewed:** 2026-07-01
> **Frontend:** React/TypeScript (`ob-poc-ui-react/`) — Chat UI with scope panel, Inspector, Semantic OS Tab
> **Backend:** Rust/Axum (`rust/crates/ob-poc-web/`) — Serves React + REST API
> **Crates:** 54 workspace crates (incl. `ob-poc` application root) — 21 ob-poc-* library (web, types, diagnostics, boundary, sage, journey, authoring, agent, macros · split-v1: bods, deal, booking-principal, semtaxonomy, ontology, entity-linking, trading-profile, derived-attributes, taxonomy · kyc-stack: **ob-poc-kyc-substrate, ob-poc-kyc-store, ob-poc-kyc-seam**) · 11 sem_os_* (core, types, ontology, policy, taxonomy, postgres, server, client, obpoc_adapter, harness, mcp) · 4 dsl-* (dsl-core, dsl-lsp, dsl-runtime, dsl-analysis) · 4 ob-* (ob-agentic, ob-templates, ob-workflow, ob-semantic-matcher) · 4 misc (entity-gateway, xtask, playbook-core, inspector-projection) · 9 unified-dsl-v0.1 (dsl-atoms, dsl-diagnostics, dsl-parser, dsl-ast, dsl-bpmn-frontend, dsl-lowering, dsl-resolution, bpmn-runtime, bpmn-test-harness)
> **Verbs:** 1,282 canonical verbs across 134 domains; 23 are dsl.kyc stream-backed (W1–W5: `ubo.edge.*` (6), `ubo.determination.*` (4), `ubo.board-controller.override`, `kyc.subject.*` (2), `kyc.role.*` (2), `kyc.obligation.*` (6), `kyc.person.*` (2)); 58 legacy determination verbs deleted (ubo/control/ownership/board write verbs replaced)
> **Macros:** 103 operator macros (22 YAML files, 18 domains, 3 composite), Tier -2B in intent pipeline
> **MCP Tools:** ~102 tools (DSL, verbs, learning, session, batch, research, taxonomy, sem_reg, stewardship, db_introspect, session_verb_surface)
> **DAG Taxonomies:** 12 (CBU + KYC + Deal + Catalogue + InstrumentMatrix + BookingPrincipal + LifecycleResources + ProductServiceTaxonomy + SemOsMaintenance + SessionBootstrap + OnboardingRequest + BookSetup) — see `rust/config/sem_os_seeds/dag_taxonomies/`
> **Latest schema additions:** `rust/migrations/20260630_kyc_intent_events.sql` (authoritative append-only verb stream + per-subject seq allocator — EOP-DD-KYCUBO-002 §2), `20260630_kyc_committed_at_clock_timestamp.sql` (B1 monotonicity fix: `DEFAULT clock_timestamp()` not `now()`), `20260630_w2_lexicon_manifest.sql` (`dsl_verbs.lexicon_hash` + `kyc_lexicon_manifest` table — Q7 whole-lexicon version), `20260630_kyc_control_edge_projection.sql` + `20260630_kyc_obligation_projection.sql` (disposable stream projections — K-34)
> **Workspaces:** 12 (8 domain: CBU, KYC, Deal, Catalogue, InstrumentMatrix, BookingPrincipal, LifecycleResources, ProductServiceTaxonomy) + (4 infrastructure: SemOsMaintenance, SessionBootstrap, OnboardingRequest, BookSetup)
> **Catalogue spec:** `docs/todo/catalogue-platform-refinement-v1_2.md` (consolidated authoritative spec, 2026-04-26 — supersedes v1.0/v1.1/v1.3). Tranche 1 implementation complete: validator (transition_args + EXISTS predicate), Sage/REPL policies, P-G provisional designation, GatePipeline default-on, CI gate. Tranche 2 (estate reconciliation: 487 verbs to declare, 153 preserving-with-transition_args migration warnings to fix) follows.
> **KYC/UBO dsl.kyc V&S implementation:** `docs/todo/EOP-DD-KYCUBO-001_*.md` + `EOP-DD-KYCUBO-002_*.md` + `EOP-DD-KYCUBO-003_*.md`. W1–W7 stream/lexicon/projection infrastructure complete on branch `codex/phase-1-5-governance-closure`; **EOP-DD-KYCUBO-003 (2026-07-01)** wired the actual determination logic into the write path — `ubo.determination.freeze` had been bypassing `OwnershipProngStrategy` entirely (a naive all-active-edges proxy, no threshold/axis split) and `kyc.person.approve` had no K-23 gate; both fixed, RED→GREEN proven. **M4 (2026-07-15): `ControlProngStrategy` landed** — `DeterminationStrategy::resolve()` generalized to take `&ControlState` (was a pre-filtered economic-edge slice); the new strategy resolves natural persons via a chain of control-kind edges (voting rights, board appointment, GP statutory, LLP designated member, dominant influence — `EdgeKind`), `Prong::ControlByOtherMeans`, no percentage quantum. Selectable via `ubo.determination.select-strategy`'s `strategy: control_prong_strategy`; `freeze` dispatches to it alongside `ownership_prong_strategy`. v1 scope, documented on the type: control-kind edges only, does not cross into the economic axis for an intermediate controlling entity's own UBOs (v2). Found and fixed while wiring the live path: `ubo.edge.assert-control`'s YAML arg was named `edge_kind` but the fold reads payload key `kind` — every real DSL call would have silently misclassified every control edge as `DominantInfluence` (same defect class as the R3 structure-class bug from EOP-DD-KYCUBO-003, caught before this verb ever went live). Still open: `pierce-nominee` (K-8, M2), lexicon-manifest coverage for the 11 obligation verbs (M2), a wire mapping from `assert-control`'s `kind` string to `EdgeKind::TrustRole`'s sub-kind (trust/foundation structure classes — real gap, not silently missed, see `dsl-kyc.yaml`'s `structure-class` description), and true institutional/role-based strategies beyond ownership+control (M2). Crate stack: `ob-poc-kyc-substrate` (pure engine, no sqlx) ← `ob-poc-kyc-store` (Postgres membrane, §3 append protocol, projectors, drainers, W2 manifest publish) ← `ob-poc-kyc-seam` (the only KYC crate touching dsl-runtime; `append_in_scope` §3.6 chokepoint). `as_of` is now a field on `VerbExecutionContext` (frozen at dispatch). Dep-gate: `rust/scripts/check_kyc_substrate_deps.sh`.
> **Schema Overview:** `migrations/OB_POC_SCHEMA_ENTITY_OVERVIEW.md`
> **Embeddings:** Candle local (384-dim, BGE-small-en-v1.5) — 24,587 patterns vectorized

This is the root project guide. **Detailed implementation docs live in annex files** — see [Domain Annexes](#domain-annexes) at the bottom.

---

## Quick Start

```bash
cd rust/

# Agentic Scenario Harness
cargo x harness list                               # List all suites + scenario counts
cargo x harness run --all                           # Run all 44 scenarios (needs DATABASE_URL)
cargo x harness run --suite scenarios/suites/governance_strict.yaml
cargo x harness run --scenario direct_dsl_denied_viewer
cargo x harness dump --scenario direct_dsl_denied_viewer

# Pre-commit (fast)
cargo x pre-commit          # Format + clippy + unit tests

# Full check
cargo x check --db          # Include database integration tests

# Deploy (Full stack: React frontend + Rust backend)
cargo x deploy              # Build React + server + start

# Chrome DevTools MCP — Automated UI Testing
# Requires: Chrome with remote debugging (chrome://inspect/#remote-debugging)
# Add to .mcp.json: { "chrome": { "command": "npx", "args": ["-y", "chrome-devtools-mcp", "--autoConnect"] } }
# Available tools: navigate_page, take_screenshot, take_snapshot, click, type_text,
#   press_key, list_console_messages, list_network_requests, get_network_request
# Test fixture: tests/fixtures/ui_smoke_test.toml (tollgate flow + demo sequences)
# Usage in Claude Code session:
#   @chrome navigate to http://localhost:3000 and run the scope gate flow
#   @chrome take a screenshot after each tollgate transition
#   @chrome check for console errors after the full flow
cargo x deploy --skip-frontend  # Skip React rebuild (backend only)

# Run server directly (serves React from ob-poc-ui-react/dist/)
DATABASE_URL="postgresql:///data_designer" cargo run -p ob-poc-web

# React development (hot reload)
cd ob-poc-ui-react && npm run dev  # Runs on port 5173, proxies API to :3000

# BPMN-Lite service (standalone workspace at bpmn-lite/)
cargo x bpmn-lite build            # Build
cargo x bpmn-lite test             # Run all tests
cargo x bpmn-lite start            # Build release + start (port 50051)
cargo x bpmn-lite start --database-url postgresql:///data_designer  # With PostgresProcessStore
cd bpmn-lite && cargo run -p xtask -- smoke --spawn-server
cd bpmn-lite && cargo run -p xtask -- stress --spawn-server --instances 300 --workers 16

# Schema overview (living doc with mermaid ER diagrams)
npx md-to-pdf migrations/OB_POC_SCHEMA_ENTITY_OVERVIEW.md

# Refresh schema exports from the live source DB
cargo x schema-export

# Populate embeddings (REQUIRED after verb YAML changes)
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings
# Use --force to re-embed all patterns (e.g., after model change)

# Semantic OS standalone server
SEM_OS_DATABASE_URL="postgresql:///data_designer" SEM_OS_JWT_SECRET=dev-secret cargo run -p sem_os_server

# Sem OS domain-pack reload check (build-engine style index; no snapshot publication)
cargo run --manifest-path xtask/Cargo.toml -- sem-reg domain-pack-check
cargo run --manifest-path xtask/Cargo.toml -- sem-reg domain-pack-check --pack-id ob-poc.cbu --force-check --update-index
```

**Schema ground truth (repo-root paths, not `rust/`-relative):** `migrations/master-schema.sql` (canonical) + `schema_export.sql` (convenience copy) — both written by `cargo x schema-export` (`rust/xtask/src/main.rs::schema_export`, which runs from the repo root). These are the ONLY two schema-dump files in the tree; `rust/migrations/master-schema.sql` / `rust/schema_export.sql` (unmaintained stale duplicates) and `docs/generated/schema.sql` (unmaintained, predated the `kyc`→`"ob-poc"` schema rename) were deleted 2026-07-02 during the state-graph remediation (RW-6) — if you see either path referenced in an older doc, treat it as historical and re-point to the canonical pair above.

### BPMN-Lite Platform Status (bpmn-lite + dmn-lite consolidated, B0 — 2026-05-16)

The compilation-and-execution kernel described in V&S v1.1 ships as a single platform repo at **github.com/adamtc007/bpmn-lite**. The dmn-lite vocabulary was consolidated into the bpmn-lite workspace during B0 (pre-flight to B1 deployment work); the dmn-lite repo is archived.

The unified workspace (edition 2024, resolver "3", rust-version "1.95") ships both vocabularies as peer crates:

- **bpmn-lite** — process vocabulary (compiler, fiber VM, FFI catalogue infrastructure, gRPC server). A3–A11 complete: `Instr::ExecFfi`, in-process FFI dispatch, BPMN data-object + FFI annotation parser, compile-time schema verifier, json_path evaluator.
- **dmn-lite** — decision vocabulary (s-expression DSL, compiler, stack VM, static analysis, dmn-lite-bridge FFI owner). Phase 1 complete (Profile v0.1).

**Test count (post-B7):** 579 passing, 5 ignored across the consolidated workspace (excluding Postgres integration tests). The single workspace eliminates the previous `[patch]` mechanism — `ffi-types` resolves as a workspace path dep for all consumers.

**B-phase complete (2026-05-16, tag `v0.1.0-heterogeneous-ffi`):** B1 (Dockerfile, cargo-chef), B5 (`docker-ffi-smoke` dmn-lite proof), B6 (HTTP FFI contract), B7 (`docker-http-smoke` HTTP proof), B8 (inline with B7), B9 (`docker-heterogeneous-smoke` — both HTTP and dmn-lite in one BPMN process, shared Flag bridge). V&S Claims 1 and 2 substantiated against the deployed containerised stack.

The local `bpmn-lite/` directory remains inside ob-poc for monorepo development convenience (has its own `.git`; ob-poc does not track it as a submodule). bpmn-lite consumes `ob-poc-types` as a rev-pinned git dep (currently `397470cb`); ob-poc consumes bpmn-lite over gRPC (`bpmn_integration/` — 12 files). See `docs/annex-bpmn-lite.md` for the integration pattern.

For internals of both vocabularies, see `bpmn-lite/CLAUDE.md`.

### Unified DSL v0.1 Status (2026-05-21)

Nine new crates ship the v0.1 unified DSL implementation (see `docs/design/v0.1/` for full spec):

- **dsl-atoms** — `StructuralKind` (20 variants), `DeclarativeKind` (4), `AtomKindClass`, `ParamType`. Atom kind taxonomy for the unified s-expression DSL.
- **dsl-diagnostics** — `DiagnosticBag`, `Span`, 11 well-known diagnostic code constants. Source-attributed error reporting.
- **dsl-parser** — Logos lexer + hand-written recursive-descent parser. Produces `SourceFile { atoms: Vec<RawAtom> }`. Handles `,name`/`,@name` template substitution, `$pre-node` insertion markers, `->` flow sugar.
- **dsl-ast** — `AtomBag` with structural/declarative classification, name index, kind-filtered iteration.
- **dsl-bpmn-frontend** — `assemble(bag, diag) -> RailwayGraph`. Full structural validation (reachability, termination, gateway fan-out, boundary attachment). 12 worked example compilation tests.
- **dsl-lowering** — `lower(graph, name) -> JourneySpec`. Deterministic serialisation of RailwayGraph to runtime executable form.
- **dsl-resolution** — `resolve(bag, registry, diag)`: validates decision-pack templates, provenance atoms, governance-status refs. `validate_bpmn(source, name, registry) -> ValidateResponse`. `PackRegistry` with load_packs_from_dir(). 12 seed packs loaded at `dsl-source/packs/`.
- **bpmn-runtime** — `RuntimeEngine` (journey-persisted hydrate/dehydrate model). `InMemoryJourneyStore`. Full event loop with parallel fork/join, merge protocol (detect-and-fail on undeclared conflicts), inclusive gateway dynamic fan-in, token-death short-circuit, verb invocation interface, switch adaptor protocol. Schema at `migrations/20260521_dsl_journey_runtime.sql`.
- **bpmn-test-harness** — `Scenario`/`RunResult` test builder. `compile_dsl()`. `instantiate_pack()` for all 12 packs (Sage stub). 89 tests total across all 9 crates.

**Test count (2026-05-21):** 89 passing, 0 failures, 3 ignored (perf).

**Tranche 1 (SemOS regression baseline) and Tranche 3 (SemOS reshape) are pending.** See `docs/todo/master-implementation-plan-v0_1.md`.

**SemOS is the hub for all things.** All paths lead to SemOS — nowhere else. The PostgreSQL schema is a supplementary store, a materialized projection, switchable if needed.

SemOS-first attribute lifecycle (2026-03-28, extended 2026-04-02):
- `AttributeDefBody` carries ALL metadata (category, validation_rules, applicability, is_derived, derivation_spec_fqn, visibility, etc.)
- Two-tier model: `AttributeVisibility::External` (governed, full changeset ceremony) vs `Internal` (operational, auto-approved, evidence_grade=prohibited)
- `attribute.define` — governed external attributes (full ceremony)
- `attribute.define-internal` — internal/system attributes (operational tier, auto-approved, no changeset)
- `attribute.update-internal` — lightweight metadata update for internal attributes only (guards against external mutation)
- `attribute.define-derived` — derived attributes with paired DerivationSpec (unchanged)
- `attribute.define` publishes SemOS snapshot FIRST, then materializes to `attribute_registry` via `materialize_to_store()`
- Materialization trigger on `sem_reg.snapshots` auto-projects active AttributeDef snapshots to `attribute_registry`
- Identity resolution prioritizes SemOS FQNs (precedence 0) over store UUIDs (precedence 1)
- SRDEF loader resolves attributes via SemOS first, with store fallback
- Catalogue store write functions are restricted to `pub(crate)` — governed verb handlers and accepted SemOS projectors are the sanctioned callers.
- `service-resource.sync-definitions` remains the bulk SRDEF authoring path, but its loader must be idempotent and report entity-grain transitions for SRDEF, owner-principal, and resource-attribute mutations.
- CI lint: `rust/scripts/lint_write_paths.sh` enforces the table-scoped P1 catalogue/snapshot direct-SQL allowlist and rejects bare-public mutator methods in the catalogue store modules. This is source scanning, not call-graph analysis; it does not prove arbitrary indirect callers are verb-mediated.

Derived attribute persistence (2026-03-27):
- runtime derived values persist in `"ob-poc".derived_attribute_values`
- dependency lineage persists in `"ob-poc".derived_attribute_dependencies`
- CBU consumers read canonical derived rows through `"ob-poc".v_cbu_derived_values`
- legacy `"ob-poc".cbu_attr_values` remains the direct/manual/non-derived observation plane
- `set_cbu_attr_value()` rejects `source = 'derived'` — derived values go canonical only

SemOS Maintenance workspace (2026-03-28):
- `WorkspaceKind::SemOsMaintenance` — first-class agentic workspace
- ScopeGate fork: "infrastructure" bypasses client group selection, routes directly to SemOS workspace
- Constellation family: `registry_governance`, map: `registry.stewardship` (7 slots)
- 4 state machines: `changeset_lifecycle`, `attribute_def_lifecycle`, `derivation_spec_lifecycle`, `service_resource_def_lifecycle`
- Pack: `semos-maintenance` with 40+ allowed verbs (changeset, governance, registry, attribute, typed-attribute, derivation, service-resource)
- 4 governance macros (Tier -2B): `governance.bootstrap-attribute-registry`, `governance.define-service-dictionary`, `governance.full-publish-pipeline`, `governance.reconcile-registry`
- 4 governance scenarios (Tier -2A): compound intent resolution for SemOS maintenance utterances
- New verbs: `service-resource.check-attribute-gaps`, `service-resource.sync-definitions`, `typed-attribute.record/get/list-for-entity`, `derivation.recompute-stale`, `attribute.bridge-to-semos`
- Verb search: 6 phrasing detection improvements (domain_filter bypass for semantic/macro/scenario/learned tiers, short query threshold scaling, multi-domain pack dominant_domain suppression, noun index for new domains)
- Utterance test harness: 353 test cases across all 7 workspaces, per-workspace hit rate reporting
- Hit rates: 78.2% first-attempt, 99.4% two-attempt, 2 wrong verbs (all workspaces above 30%)
- Contextual query detection: 16 patterns intercepted before verb search, routed to NarrationEngine
- Governed phrase authoring (v1.2): `phrase_bank` table (13,570 entries), `phrase_mapping` SemOS object type, `phrase_authoring_lifecycle` state machine (8 states), 9 phrase.* verbs, AI proposal pipeline with 5-signal confidence scoring + risk-tiered approval routing
- Onboarding product macros: `structure.product-suite-custody-fa-ta`, `structure.product-suite-full`, `structure.remove-all-products` — compound intent → multi-step runbook → per-entity expansion → DAG-ordered → confirm all → execute atomically
- Macro priority: ScenarioIndex (1.05) > MacroIndex (1.04) > exact phrase (1.0) — macros always win over single verbs when both match (safer, atomic, complete)
- Per-entity macro expansion: runbook compiler replicates macro steps per CBU UUID in scope
- Macro audit (2026-03-29): fixed `expands_to` → `expands-to` YAML key in `attribute.seed-*` macros (serde kebab-case deserialization bug); removed 8 KYC-domain macros from `book-setup` pack (screening, case, kyc-workflow macros leaked into CBU/InstrumentMatrix workspaces); added search overrides for `screening-ops.*` workstream-level macros; two screening families coexist: `screening.*` (party-level ad-hoc) and `screening-ops.*` (workstream-level KYC)
- PACK001 lint rule: workspace-macro bleed detection — checks every macro in a pack's `allowed_verbs` has mode-tags compatible with all pack workspaces; prevents KYC/screening macros from leaking into CBU/InstrumentMatrix contexts. The workspace↔mode-tag compatibility table now lives in the runtime as the single source of truth (`ob_poc::agent::workspace_mode_tags::workspace_accepts_any_mode_tag`) — both PACK001 (xtask) and the runtime allowed-set composition consume it; documented in `docs/annex-macros.md`
- `cargo clippy` clean across entire codebase

Infrastructure Workspace Scope Bypass (2026-06-04):
- Direct selection of infrastructure/maintenance workspaces (BPMN, SemOS Maintenance, etc.) in the initial Universe list bypasses the client ScopeGate.
- Sets client scope context to nil UUID, sets session name to "SemOS Infrastructure", and transitions directly into the workspace.


SemOS domain-pack taxonomy reload (2026-05-14):
- Sem OS domain packs are the configuration-native ownership boundary for domain-specific YAML shape. Business crates are clients/implementation homes; they do not own Sem OS taxonomy shape.
- Domain Pack manifests live in `rust/config/sem_os_seeds/domain_packs/*.yaml` and declare owned DAGs, DSL packs, state machines, constellation maps/families, universes, verb prefixes, entity kinds, and informational business-crate links.
- `DomainPack` is a SemReg object type. Domain packs are scanned into `SeedBundle.domain_packs` by `sem_os_obpoc_adapter` and published through the existing Sem OS seed bootstrap path.
- Runtime DAG discovery and Sem OS seed assembly are manifest-owned: DAG/state-machine/constellation/universe seeds are visible only when declared by a Domain Pack. Macro definitions are visible only when an owned DSL pack exposes the macro FQN in `allowed_verbs`; the macro is then unpacked through `expands-to[]` into atomic DSL verbs for reconciliation. Direct directory walkers are parser/tooling utilities, not the production ownership source.
- Reload uses a build-engine index in `sem_reg.domain_pack_reload_index`: path + mtime + size are the cheap "maybe dirty" check; canonical surface hash is the correctness check.
- Reload checking does not publish snapshots directly. It reports `clean`, `index_only`, or `publish_required`; actual Sem OS mutation remains behind `bootstrap_seed_bundle()`, where identical payloads skip and changed payloads publish non-breaking successor snapshots.
- Manual trigger: `cargo run --manifest-path xtask/Cargo.toml -- sem-reg domain-pack-check [--pack-id ob-poc.cbu] [--force-check] [--update-index] [--json]`.
- Architecture note: `docs/architecture/sem-os-domain-pack-taxonomy-reload.md`.

---

## What "macro" means in ob-poc

> **Macros are governed recipes: pack-scoped, hashable, versioned multi-step
> domain patterns that Sage may discover and bind, but which the compiler
> must expand into ordinary DSL atomics before any REPL execution occurs.**

This definition is load-bearing. The word "macro" carries different meanings
in other ecosystems (C preprocessor text substitution, Rust `macro_rules!`
hygienic AST transformation, Lisp symbolic code rewriting) — none of those
apply here.

In ob-poc, a macro is:

- **A first-class SemOS registry entity.** Hashed, versioned, lifecycle
  FSM (`draft → active → deprecated → retired`). Authored as YAML,
  catalogued like a verb.
- **Pack-scoped and governed.** Pack manifests gate which macros are
  allowed. The pack governs; the macro proposes affinity via `mode_tags`.
- **A multi-step domain recipe.** Each macro encodes an expert outcome
  pattern — slot contract, preconditions, ordered expansion, expected
  state transitions, refusal/pending-question conditions.
- **A planning + compilation surface, not an execution surface.** ACP
  exposes macros to Sage so the agent picks the right recipe. The
  compiler then expands the macro into an ordered DSL atomic sequence,
  and the REPL executes only those atomics. Macros have **no mutation
  authority after expansion** — execution is verb-only.

The reason for the macro tier to exist: without it, Sage is operating at
the verb floor (one mutation at a time) and has to invent multi-step arcs
itself. Macros encode the multi-step pattern as a registered, hashed,
versioned, governed object, which Sage chooses as an atomic option. This
reduces hallucination surface and encodes expert domain knowledge into
the dispatch surface itself.

See `docs/annex-macros.md` and
`docs/architecture/sem-os-domain-pack-taxonomy-reload.md` for the durable
macro and domain-pack architecture notes that protect this discipline under
LLM pressure.

## Repository Hygiene

- Root `todo/` is transient planning scratch space and is ignored by git.
  Do not use it as an architecture source of truth.
- Durable project guidance belongs in `CLAUDE.md`, `AGENTS.md`, and
  persistent docs under `docs/`, especially `docs/architecture/`.
- Generated screenshots and local UI smoke artifacts are local evidence only
  unless a task explicitly promotes them into a reviewed artifact.

## Non-Negotiable Implementation Rules

### 1. Type Safety First

**Never use untyped JSON (`serde_json::json!`) for structured data.** Always define typed structs.

```rust
// WRONG - Untyped
Ok(ExecutionResult::Record(serde_json::json!({ "groups_created": row.0 })))

// CORRECT - Typed struct with Serialize/Deserialize
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeriveGroupsResult { pub groups_created: i32 }
let result = DeriveGroupsResult { groups_created: row.0 };
Ok(ExecutionResult::Record(serde_json::to_value(result)?))
```

**Where to define types:** Domain results → `ob-poc-types`, DSL config → `dsl-core/src/config/types.rs`, API → near handler or shared `types.rs`.

### 2. Consistent Return Types

| YAML `returns.type` | Rust Pattern |
|---------------------|--------------|
| `uuid` | `ExecutionResult::Uuid(uuid)` |
| `record` | `ExecutionResult::Record(serde_json::to_value(typed_struct)?)` |
| `record_set` | `ExecutionResult::RecordSet(...)` |
| `affected` | `ExecutionResult::Affected(count)` |
| `void` | `ExecutionResult::Void` |

### 3. Option<T> for Nullable/Optional Values
Use `Option<T>` consistently. No sentinel values or silent nulls.

### 4. Error Types over Panics
Use `Result<T, E>` and `?` operator. Never `.unwrap()` in production code paths.
```rust
let value = map.get("key").ok_or_else(|| anyhow!("Missing key"))?;
```

### 5. Re-export Types at Module Boundary
When a module uses types from another crate, re-export them for consumers. **Never use `pub use types::*`** — classify each type and export an explicit allowlist. Internal types stay `pub(crate)` or `#[cfg(test)]`-gated. See `dsl-core/src/config/mod.rs` and `dsl-core/src/lib.rs` for the canonical pattern (43-type config allowlist, 17-type root set).

### 6. Single DAG Identity (Observatory)

**All projections of constellation state read from `tos.hydrated_state`.** No consumer hydrates independently. The canonical flow is:

```
verb execution → rehydrate_tos() → tos.hydrated_state (HydratedSlot tree)
    ↓ project (read-only)
    ├── RunbookPlanStep (compiler)
    ├── NarrationPayload (narration engine)
    ├── OnboardingStateView (chat UI)
    ├── GraphSceneModel (Observatory canvas)
    ├── OrientationContract (Observatory viewport)
    └── SessionFeedback (REPL response)
```

**Two kinds of state on the session:** Resource state (the DAG, rehydrated after writes) and viewport state (view level, lens, focus — session-local, no rehydration). Navigation verbs mutate viewport state only.

**All resource-state and constellation-state changes flow through `orchestrator_v2.process()`.** No REST endpoint may directly mutate session resource state, navigation, or constellation state.

**Documented metadata exemptions** (live alongside the constellation, not inside it — do not pass through `process()`):
- `POST /api/session/:id/bindings` — symbol→entity aliases (`set_session_binding` in `agent_routes.rs`). Shell-style aliases scoped to a session; no constellation impact.
- `POST /api/session/:id/focus` — `stage_focus` UI filter (`set_session_focus`). Viewport hint, not resource state.
- `POST /api/session/:id/subsession` + `complete_subsession` — research scratch buffer that writes resolved bindings back into the parent. Sub-sessions are isolated from the parent's runbook by construction.

These three are exempt because they're session metadata, not DAG state. Promote to a `process()`-mediated input variant only if a real consistency bug appears (e.g. a binding survives across a runbook checkpoint when it shouldn't).

Observatory endpoints (`observatory_routes.rs`) MUST read from the session's `WorkspaceFrame.hydrated_state`, never call `try_hydrate_cbu()` independently or build `FocusState` from scratch. The `ShowLoop` is a transitional exception (renders SemReg object detail, not constellation slot state).

---

## Core Architecture: CBU-Centric Model

**CBU (Client Business Unit) is the atomic unit.** Everything resolves to sets of CBUs.

```
Session = Set<CBU>
  Universe (all CBUs)
    └── Book (commercial client's CBUs: Allianz, BlackRock)
         └── CBU (single trading unit)
              ├── TRADING view (default) — instruments, counterparties
              └── UBO view (KYC mode) — ownership/control taxonomy
  Group structure cross-links CBUs via ownership/control edges
  Clusters/galaxies are DERIVED from these edges, not stored
```

### Three Investment Registers & Share Nomenclature

A single share structure underpins three distinct analytical projections (Registers) depending on the context:

1. **`control_interest`** $\rightarrow$ UBO / Controller KYC
   * *Question:* Who controls the Asset Owner (AO)?
   * *Basis:* Projected with `basis = 'VOTES'`.
   * *Key components:* `kyc_control_edge_projection` (dsl.kyc stream fold — K-34), `kyc_subject_rollup_projection`, `kyc_obligation_projection`. Legacy `control_edges`/`entity_ubos`/`cbu_board_controller` tables are empty and their write verbs have been deleted (W4 rip, 2026-06-30).
2. **`economic_participant`** $\rightarrow$ Investor AML / Source of Funds / Eligibility
   * *Question:* Who participates economically in the AO?
   * *Basis:* Projected with `basis = 'ECONOMIC'`.
   * *Key components:* `kyc.investors` (lifecycle), `kyc.holdings` (`usage_type = 'TA'`), `kyc.movements` (transaction ledger).
3. **`portfolio_holding`** $\rightarrow$ Asset Eligibility / Sanctions / Market / Custody / Look-Through Policy
   * *Question:* What does the AO hold?
   * *Key components:* `kyc.fund_vehicles`, `kyc.fund_compartments`, `kyc.investor_role_profiles` (look-through policy).
   * *Computation:* Bounded recursive CTE `kyc.fn_compute_economic_exposure()`.

- **ViewMode:** Unit struct — always TRADING. Use `view.cbu :mode ubo` for KYC/UBO.
- **GraphScope:** `Empty | SingleCbu | Book | Jurisdiction | EntityNeighborhood | Custom`
- **Session = Run Sheet = Viewport Scope:** Session is single source of truth. `entity_scope.cbu_ids` drives the viewport. Run sheet tracks per-statement `DslStatus`: Draft → Ready → Executing → Executed → Failed → Cancelled.

### Unified Session Pipeline

All user input routes through `POST /api/session/:id/input` → `ReplOrchestratorV2.process()`.

**Mandatory tollgate sequence:**
1. **ScopeGate** → Client group selection (non-negotiable)
2. **WorkspaceSelection** → KYC | OnBoard | CBU | Deal | Product Maint | Instrument Matrix
3. **JourneySelection** → Pack selection within workspace
4. **InPack** → Verb matching + sentence generation
5. **SentencePlayback** → Confirm/reject proposed DSL
6. **RunbookEditing** → Review runsheet
7. **Executing** → Step-by-step execution with narration

`ReplSessionV2` is the canonical session. `UnifiedSession` retained for execution context only. `CbuSession` removed. Response adapter converts `ReplResponseV2` → `ChatResponse` for frontend compatibility.

**Closed-loop invariant:** After verb execution (`writes_since_push > 0`), the TOS constellation is re-hydrated from the database before building the response. This ensures the UI always renders post-execution entity state (updated slot states, available verbs, progress). Constellation refresh is triggered by entity state changes, not every turn. After re-hydration, `compute_narration()` produces a `NarrationPayload` (progress, delta, gaps, suggested next actions) attached to the response for the NarrationPanel. Contextual queries ("what's next", "what's missing") bypass verb search entirely and return `query_narration()` directly.

> **Key files:** `rust/src/repl/orchestrator_v2.rs` (orchestrator), `rust/src/api/response_adapter.rs` (adapter), `rust/src/api/agent_enrichment.rs` (onboarding state enrichment)

---

## DSL Pipeline (Single Path)

ALL DSL generation goes through: **User → verb_search → dsl_generate (LLM extracts args as JSON) → deterministic DSL assembly → dsl_execute**

```
Search Priority (9-tier):
-2A. ScenarioIndex (journey-level compound intent, score 0.97)
-2B. MacroIndex (macro search parity, score 0.96)
-0.5. ConstellationVerbIndex (state-gated noun+action lookup)
  0. Operator macros (1.0 exact / 0.95 fuzzy)
  1-2. Learned exact (1.0)
  3. User semantic (pgvector, BGE asymmetric)
  5. Blocklist filter
  6. Global semantic fallback
  7. Phonetic fallback (0.80)
```

**Route types:** Scenarios support `macro_selector` (jurisdiction x vehicle_type → macro FQN) and `verb_selector` (entity type determination → verb FQN, e.g., analyse-ubo vs trace-ownership). Both are resolved by CompoundSignals before verb search.

**PolicyGate:** Server-side single-pipeline enforcement. `SemOsContextEnvelope` replaces `SemRegVerbPolicy`: carries allowed verbs, pruned verbs with structured `PruneReason` (4 variants: AbacDenied, EntityKindMismatch, AgentModeBlocked, PolicyDenied), `AllowedVerbSetFingerprint` (SHA-256), TOCTOU recheck. Pre-constrained verb search threads allowed verbs into `HybridVerbSearcher`.

**ACP boundary (2026-05-06):** `ob_poc_acp` is the launchable Agent Client Protocol server over newline-delimited JSON-RPC stdio. ACP is the rich agent-editor projection surface for SemOS discovery, not the policy or mutation authority. ACP exposes two personas, `sage:planning` and `sage:execution`; discovery/planning/explanation/attestation are Sage workflow phases, not ACP modes. `obpoc/policy`, `obpoc/projections/list`, and `obpoc/projection/get` expose Domain Pack policy, projection catalogue entries, typed hashed projection envelopes, pack-declared `semos://...` resource URI schemes, discovery probe allow/refuse reasons, context classification/redaction rules, transition dry-run/mutation capability, and the mutation boundary (`workbook_approval_and_compiled_runbook_gate`). Projection is demand-driven: trace/audit events record `acp_mechanism_summary`, `acp_fallback_summary`, `projection_count`, `projection_bytes`, and `projection_latency_ms` to catch over-eager ACP projection during MVP-DryRun review. Visibility and authority are independent: Sage/editor may observe any classification-permitted projection the Domain Pack exposes; direct ACP mutation is refused. Enforcement remains behind ACP in SemOS Domain Pack validation, workbook integrity, approval tokens, and the compiled runbook execution gate.

**SessionVerbSurface:** 7-step compute pipeline: Registry → AgentMode → Scope+Workflow (merged) → SemReg CCIR → Lifecycle → FailPolicy → Rank+CompositeStateBias. FailClosed default = ~30 safe-harbor verbs. Dual fingerprints: `vs1:<hex>` (surface) vs `v1:<hex>` (SemReg).

**LLM removed from semantic loop:** Verb discovery is pure Rust (5-15ms via Candle). LLM used only for arg extraction (200-500ms).

**Key files:** `rust/src/agent/orchestrator.rs`, `rust/src/mcp/verb_search.rs`, `rust/src/mcp/intent_pipeline.rs`, `rust/src/agent/sem_os_context_envelope.rs`, `rust/src/agent/verb_surface.rs`, `rust/src/mcp/scenario_index.rs`, `rust/src/mcp/macro_index.rs`

> **Full details:** `docs/annex-dsl-and-intent.md`

---

## React Frontend

**Cockpit layout:** The ChatPage is the primary UI — egui WASM constellation canvas (center, always visible) + chat messages & panels (right column). The Observatory page remains as a full-screen option.

```
ob-poc-ui-react/src/
├── api/              # API client (chat.ts, scope.ts, semOs.ts, observatory.ts)
├── features/chat/    # Cockpit UI: egui canvas center + chat right + panels
├── features/observatory/  # Full-screen Observatory (standalone option)
├── features/inspector/    # Projection inspector (tree + detail)
├── stores/           # Zustand state management
└── types/            # TypeScript types
```

**ChatPage layout (cockpit):**
```
[Sessions w-64] | [egui Canvas flex-1     ] | [Chat + Panels w-[28rem]]
                  [FlightDeck status bar   ]   [Messages (scrollable)  ]
                  [Canvas (60fps WASM)     ]   [ChatInput              ]
                                               [Scope, Constellation   ]
                                               [Narration, Verbs       ]
```

The egui canvas renders `GraphSceneModel` from the Observatory API (polled every 5s). Navigation is direct: hover/click/double-click nodes on the canvas fires verbs through the standard REPL input pipeline.

**Key Endpoints:**

| Endpoint | Purpose |
|----------|---------|
| `POST /api/session` | Create session (optional `workflow_focus`) |
| `POST /api/session/:id/input` | **Unified ingress** (`kind`: utterance / decision_reply / repl_v2) |
| `GET /api/session/:id/scope-graph` | Loaded CBUs (scope) |
| `GET /api/cbu/:id/constellation` | Hydrated constellation tree |
| `GET /api/cbu/:id/cases` | KYC cases for constellation binding |
| `GET /api/projections/:id` | Inspector projection |
| `GET /api/sem-os/context` | Registry stats + recent changesets |
| `GET /api/constellation/by-name` | Resolve CBU by name + hydrate constellation |
| `POST /api/session/:id/runbook/compile` | Compile multi-workspace runbook plan |
| `GET /api/session/:id/runbook/plan` | Get current runbook plan |
| `POST /api/session/:id/runbook/approve` | Approve compiled plan for execution |
| `POST /api/session/:id/runbook/execute` | Execute next plan step (INV-3 gate) |
| `POST /api/session/:id/runbook/cancel` | Cancel plan mid-execution |
| `GET /api/session/:id/runbook/status` | Current plan status + cursor |
| `GET /api/session/:id/acp/policy` | ACP-visible SemOS policy/capability decisions |
| `GET /api/session/:id/acp/projections` | ACP-visible SemOS projection catalogue |
| `GET /api/session/:id/acp/projections/:kind` | Typed ACP projection envelope with hash/classification metadata |
| `POST /api/session/:id/acp/open` | Open ACP adapter session (no direct mutation capability) |
| `POST /api/session/:id/acp/close` | Close ACP adapter session |
| `POST /api/session/:id/acp/context` | Assemble redacted Sage context via Domain Pack discovery policy |
| `GET /api/session/:id/trace` | Session trace (append-only mutation log) |
| `GET /api/session/:id/trace/:seq` | Single trace entry by sequence |
| `POST /api/session/:id/trace/replay` | Replay trace (strict/relaxed/dry_run) |
| `GET /api/observatory/session/:id/orientation` | OrientationContract for session |
| `GET /api/observatory/session/:id/show-packet` | ShowPacket with orientation |
| `GET /api/observatory/session/:id/graph-scene` | GraphSceneModel (constellation projection) |
| `GET /api/observatory/session/:id/navigation-history` | OrientationContract sequence |
| `GET /api/observatory/session/:id/diagrams/:type` | Mermaid diagram (erd, verb_flow, etc.) |
| `GET /api/observatory/health` | SemReg health metrics |

**Legacy (410 Gone):** `POST /chat`, `POST /decision/reply`, `POST /repl-input`, `POST /select-verb`

```bash
cd ob-poc-ui-react && npm install && npm run dev  # Hot reload on :5173
npm run build && npm run typecheck && npm run lint
```

> **Full details:** `docs/annex-frontend-and-tools.md`

---

## Feature Status

**Complete (✅):** React Migration (077), V2 REPL (7-state, 320 tests), Runbook Compilation, Candle Semantic Pipeline, Agent Pipeline + PolicyGate, Solar Navigation (038), Promotion Pipeline (043), Teaching (044), Client Group Resolver (048), Workflow Task Queue (049), Transactional Execution (050), CustomOp Auto-Registration (051), Client Group Research (055), REPL Viewport Feedback (056), Verb Disambiguation UI (057), Unified Architecture (058), Playbook System (059), LSP (060/063), CBU Structure Macros (064), Unified Lookup (074), Lexicon (072), Entity Linking (073), Clarification UX (075), Inspector-First (076), Deal Record & Fee Billing (067), BPMN-Lite (all phases incl. Phase 4 PostgresProcessStore + Phase 5A Inclusive Gateway), BPMN-Lite Integration (Phase B), BPMN-Lite Authoring (Phases B-D), KYC/UBO Skeleton (S1-S2), Semantic OS (Phases 0-9 + Standalone v1.1 + Stewardship Phase 0-1), Governed Registry Authoring (v0.4, migrations 099-102), CCIR + SessionVerbSurface, Loopback Calibration (v0.3), Onboarding State View, Verb Disambiguation UX, Constellation Orphan Remediation, SemOS Grounded Action Surface, Pipeline Leak Remediation, Sage Intent Skeleton (Phase 1), Entity-First Utterance Parsing, Coder Rewrite (Phase 2), Sage-Primary Chat Narration, SemTaxonomy Three-Step, NLCI CBU Cutover, CBU Role Surface Reconciliation, Phase 0 Vocabulary Rationalization (Batches 1-3), Schema Consolidation (115-121), Domain Metadata Coverage (306/306 tables), Scenario-Based Intent Resolution (Phases 0.5-5), AffinityGraph & Diagram Generation, Discovery Pipeline (Phase 2), Utterance API Coverage Harness, Unified Session Input Cutover, Workspace-Scoped REPL Navigation, SemOS Attribute DSL + Schema Cleanup, SemOS Footprint Hydration S6, SemOS Document Governance Bootstrap (122-123), StateGraph Pipeline (Phase 0-3 substrate), Session Stack Machine Runbook Architecture (R1-R9, migrations 125-128), Unified Session Pipeline (ADR 040 — tollgates enforced, 149/149 tests, response adapter, dead code removal -4,480 lines), Derived Attribute Persistence (D0-D12 — canonical two-table model, staleness propagation, CBU projection view), SemOS-First Hub Implementation (Phases 1-7 — AttributeDefBody complete, SemOS-first write path, materialization trigger, identity resolution inverted, 7 new verbs, SemOS Maintenance workspace), Sage Proactive Narration (ADR 043 — NarrationEngine, contextual query intercept, post-execution narration, NarrationPanel React component, narration boost signal, end-to-end wiring), Verb/Noun Separation (S-expression aligned — assemble-cbu macro_selector, analyse-ubo verb_selector, action stem extractor), Instrument Matrix Two-Stage (group template + CBU instance), Session Recovery (resume with fresh scope), Two-Tier Attribute Model (AttributeVisibility External/Internal, attribute.define-internal + attribute.update-internal, migration 130, operational-tier auto-approved), BPMN-Lite Durability Fixes (transaction atomicity via atomic_start/atomic_complete, job claim timeout + reclaim, tick_all orchestrator, dedupe cache TTL pruning, 3 background housekeeping tasks), Cross-Workspace State Consistency (P1-P10 — shared atom registry with lifecycle FSM, shared fact versioning, workspace fact refs with staleness propagation, constellation replay types, remediation events with FSM, external call idempotency envelope, provider capabilities, compensation records, YAML seeds for 5 initial atoms, platform DAG derivation; 12 shared-atom verbs + 4 remediation verbs, 8 migrations, 10 cross_workspace modules, 10 macros (8 shared-atom + 2 remediation incl. batch foreach), 6 scenario routes, 24 constellation slots), Pub API Surface Cleanup (Tier A — 36 items tightened across 4 crates: `sem_os_postgres` sqlx_types→pub(crate), `sem_os_core` 4 internal fns→pub(crate), `dsl-core` 2 wildcard re-exports→explicit allowlists, `dsl_v2` expansion/macros/entity_deps re-exports trimmed, test-only fns cfg(test)-gated; test boundary enforcement: runbook e2e+pipeline tests rewritten as external harnesses against public API with YAML macro fixtures, source-scanning invariant tests moved internal), SemOS Execution Port (Phases 0-3 — VerbExecutionPort trait in sem_os_core, PgCrudExecutor in sem_os_postgres with 12/14 CRUD operations, ObPocVerbExecutor adapter + VerbExecutionPortStepExecutor bridge, orchestrator wired, production startup activated, execute_json compatibility shim complete across 625/625 `rust/src/domain_ops` CustomOperation impls, ob-execution-types dead crate removed; 20 active crates), **Phase 5c-migrate COMPLETE** (80 slices — 567 ops relocated to `sem_os_postgres::ops::*` via YAML-first re-implementation + 119 Pattern B ops in `rust/src/domain_ops/*` converted to `SemOsVerbOp`, all registered in single canonical `SemOsVerbOpRegistry` via `sem_os_postgres::ops::build_registry() + ob_poc::domain_ops::extend_registry()`; slice #80 deleted the `CustomOperation` trait, `CustomOpFactory`, `CustomOperationRegistry`, `inventory::collect!` registry, `#[register_custom_op]` proc-macro, `dsl-runtime-macros` crate, `dispatch_plugin_via_execute_json` fallback, `verify_plugin_verb_coverage*` helpers — net −1,030 LOC; `SemOsVerbOp` is the sole plugin-verb execution contract, dispatched through the Sequencer-owned transaction scope), **Catalogue Platform v1.3 CODE COMPLETE** (2026-04-25 — 9 DAG taxonomies authored across Tranche 2+3 (4 primary + 5 Tranche 3), 758/1245 verbs three-axis declared (60.9%), full v1.3 runtime stack landed across `dsl-core::config::dag_registry` + `dsl-runtime::cross_workspace::*` modules: DagRegistry with 5 indices (constraints/aggregates/parents/children/verb→transition), SlotStateProvider (24-row Postgres dispatch table) + SqlPredicateResolver, GateChecker (Mode A blocking), DerivedStateEvaluator + DerivedStateProjector (Mode B aggregation/tollgate), CascadePlanner + PostgresChildEntityResolver (Mode C hierarchy), TransitionArgs metadata field on VerbConfig + 87 verbs declaring it, GatePipeline bundle + `VerbExecutionPortStepExecutor::with_gate_pipeline` builder + `ReplOrchestratorV2::with_gate_pipeline` builder; pre-dispatch gate check + post-dispatch single-level cascade execution wired in `step_executor_bridge.rs`; GatePipeline now wired by `ob-poc-web::main` (slot_state_provider + predicate_resolver + gate_checker + derived_state_projector active; `cascade_planner: None` is the only remaining gap); spec at `docs/todo/catalogue-platform-refinement-v1_3.md`), **Intent Trace Evidence Capture** (Option C, 2026-06-18 — extended `IntentTrace` + `"ob-poc".intent_events` (migration 20260618) with `surface_full_count`/`surface_pack_scoped_count` (from `FilterSummary`), `soft_stage_flow`, `state_observer`, `entity_confidence`; non-mutating `verb_surface::observe_state_reachability` (Step-5 lifecycle, read-only) + `verb_search::soft_stage_flow` (derived from final results — no `search()` surgery, ranking unchanged by construction); board-fixture eval harness `tests/intent_trace_eval.rs` over the 524-case CIC corpus; verdict split — operationally **Option B** (no architecture change), v0.5 state-reducer thesis **UNTESTED** because only 22/1306 verbs ≈ 1.68% declare `requires_states`; reports under `rust/reports/`), **CBU Metadata Cleanup I1–I5** (corpus-blind, frozen `88eb3699` — `cbu.assign-role :role-type` dispatching plugin (`cbu_role::AssignRole`) folds the 6 `cbu.assign-*` specialists into one discoverable verb while **retaining every write-path** (ownership %, control, trust interest, fund LP %, SP, signatory) as unregistered dispatch targets; board-cbu-operational re-grounded on manifest `owned_verb_prefixes:[cbu.]` 473→102 verbs; dangling `cbu-custody.` prefix dropped; `cbu_overall_lifecycle` is owned via `owned_dags:[cbu_dag]` (it is a DAG-taxonomy FSM; the cleanup briefly mis-listed it under `owned_state_machines`, which resolves against `state_machines/*.yaml` seeds it has none of — corrected 2026-06-18, the seed-loader panic it caused fixed) + UPPERCASE `requires_states`/`transitions_to` authored on 7 transition verbs (gated 4→11); bidirectional verb↔op coverage tests hold; 47 phrases re-homed + 72 stale embeddings pruned)), **Workspace Macro Admission by Membership** (commit `aadec63e`, 2026-06-18 — fixed a production macro-suppression bug: `SessionVerbSurface::allowed_fqns()` emitted atomic `RuntimeVerb`s only, so any workspace-constrained allowed set matched-then-**dropped** every macro/scenario at the tier filters (`verb_search.rs` 806/860/901/958) — the outcome-level layer went dark whenever a workspace was active; macros survived only on fail-open `allowed_verbs=None`. The workspace↔mode-tag table was **lifted out of the `xtask` PACK001 lint into the runtime** `ob_poc::agent::workspace_mode_tags` as the single source of truth (xtask re-consumes it); `SessionVerbSurface` gained `owned_macros`, populated in `compute_session_verb_surface` from `stage_focus → workspace → membership-owned macro FQNs` and unioned into `allowed_fqns()`. Admission is by `routing.mode-tags` membership (140/140 macros declare them), **never** the FQN leading-domain token (`struct.lux.ucits.sicav` has mode-tags `[onboarding, structure]` → owned by the `cbu` workspace); the macro/scenario tier filters stay, the **set widens by membership**. The clean 102-verb cbu atomic pack is unchanged. The `tests/intent_trace_eval.rs` CBU board now drives the real `compute_session_verb_surface` (`stage_focus=semos-onboarding`, 13-domain workspace + owned macros) instead of a bespoke 4-domain pack reconstruction; corpus scoped by workspace membership (110 in-scope / 109 out-of-scope tagged by true workspace); composed re-run over the 347-verb board: within-2@5 **93.97%**, recall@5 **96.55%**, confident-wrong **3.4%**, the membership-owned macro layer worth **+21.5pts** of within-2@5 vs an atomic-only board)), **CBU Add-Product Transaction Atomicity** (2026-06-21, branch `cbu-add-product-atomicity` — `cbu.add-product`'s child writes (`service-intent.create` + the discovery/rollup/populate pipeline) previously dispatched via `scope.pool()` (a fresh out-of-txn connection) and committed independently, so a rolled-back add-product left committed-orphan `service_intents`/`srdef_discovery_reasons` (the §10.2 idempotency-gate COVERAGE_GAP). Threaded the parent transaction's `scope.executor()` (`&mut PgConnection`) down the whole `service_resources` pipeline: each `ServiceResourcePipelineService` DB method gained a connection-based `_in(conn)` variant; the 3 discovery engines take `&mut PgConnection` at their entry methods; the derivation engine's `self.pool.begin()` became `conn.begin()` — a **SAVEPOINT** when the connection is already inside a txn, preserving the per-spec multi-statement isolation while joining the parent (the `derived_attributes` `*_tx` repo is untouched); `ServicePipelineService::dispatch_service_pipeline_verb` now takes `&mut dyn TransactionScope`, routing `service-intent.create` + `discovery.run` through `scope.executor()` and all other arms through `scope.pool()` (unchanged). Teeth: `test_cbu_add_product_rolls_back_atomically` runs add-product on a confirmed CBU via the public `execute_plan_atomic_in_scope`, drops the scope, asserts zero orphan `service_intents`/`srdef_discovery_reasons` — GREEN, teeth-proven RED by re-introducing the pool-write at the dispatch. db_integration 31/0, lib lane 1923/0, derivation persistence 2/2)), **CBU⊥KYC Domain Decoupling — Unit A** (2026-06-22, branch `cbu-add-product-atomicity`, commit `623b5c91` — **principle: CBU is a purely structural container that knows nothing about KYC; inter-domain coupling is one-directional read-only (KYC reads CBU via the mandatory ManCo entity, never the reverse).** A read-only 4-probe audit (`docs/todo/cbu-kyc-decoupling-blast-radius.md`) mapped the coupling to 3 code clusters (macros/scenarios CLEAN). Unit A removed the worst: **deleted `cbu.decide`** — a `cbu.*` verb that wrote the structural `cbus.status` from a KYC/AML decision AND reached into KYC (`case_evaluation_snapshots` INSERT + `kyc-case.*` dispatches); it was redundant because the KYC domain already owns the full decision set (`kyc-case.{approve,reject,approve-with-conditions,escalate,close,update-status}`). Also **removed the cross-workspace gate `cbu_validated_requires_kyc_case_approved`** — structural `VALIDATION_PENDING→VALIDATED` (via `cbu.confirm`) no longer depends on KYC approval; the KYC "good-to-transact" read stays only on the operational/transact gate (`cbu_operationally_active`). Landed as ONE DAG+DSL lockstep change (no dangling verb↔DAG ref): DAG `via:` edges → `cbu.confirm`/`cbu.reject`, phrases re-homed ("record KYC/AML decision"→`kyc-case.approve`, "approve CBU/structure"→`cbu.confirm`), refs cleaned from `domain_metadata`/`taxonomy`/`verb_index`/`lexicon`/constellation, C1/C2/C3/C5 rewritten to `cbu.confirm` (identical VALIDATION_PENDING profile). Verified: `verbs compile` Removed:1 (→1302), `domain-pack-check` no seed-loader panic, 39 embeddings + 1 centroid pruned, db_integration 31/0 + plugin-coverage + C1/C2/C3/C5 green; net −296 LOC. **Cluster 5 DONE** (commit `83bfd614`): `cbu.inspect` is now structural-only — stripped its KYC reads (screenings via cases/entity_workstreams + the kyc_cases list) and projection fields; the inspect record is cbu core + entities/roles + documents + services, matching its declared metadata reads. **Cluster 3 DONE** (migration `rust/migrations/20260622_rename_cbu_to_ubo_relationship_verification.sql`): renamed `cbu_relationship_verification` → `ubo_relationship_verification` — it is KYC/UBO verification data (written only by `ubo.*`, read by tollgate + visualization; **no `cbu.*` verb touches it**), cbu_id-scoped but NOT CBU-structural; the `cbu_` prefix made KYC data masquerade as CBU state. Full rename: table + PK/UNIQUE/FK/CHECK + 3 indexes + 4 PG15 NOT-NULL constraints + the stale view COMMENT (rename doesn't auto-update comments/function bodies); dependent views (`ubo_convergence_status` etc.) auto-cascaded. Updated all code/config SQL refs (ubo_graph, tollgate, visualization_repository, query_engine, xtask harnesses, domain_metadata) + regenerated `master-schema.sql`/`schema_export.sql` + `cargo sqlx prepare --workspace -- --all-targets` + ER diagrams. Historical migrations (004a/004b/010/202412) keep the old name by design. Verified: 1315 rows + 472 view rows preserved, db_integration 31/0, clippy clean. (The `ubo-test` xtask harness had PRE-EXISTING multi-layer schema drift (wrong `kyc.` schema on 6 tables, a clean cascade ~65 FK-children stale, a missing NOT-NULL `confidence` column) — **now fully repaired**: dynamic FK-catalogue cascade under `session_replication_role=replica` + `confidence` added; `cargo x ubo-test all` cleans + passes all 3 UBO scenarios (3/0). Also **removed the `category_gated` fund-only slot-activation rule** (5 blocks across cbu_dag/kyc_dag — investor/holding/share_class no longer gate on `cbu_category`; the rule was unrecognised and code-unconsumed).) **Cluster 4 DONE** (commit `<this>`): relocated 7 foreign-domain FSM slots OUT of `cbu_dag.yaml` (the CBU pack was co-owning KYC/UBO/investor/manco state machines via `owned_dags:[cbu_dag]`) — 6 → `kyc_dag.yaml` (`investor`, `investor_kyc`, `holding`, `manco`, `entity_proper_person`, `entity_limited_company_ubo`; the kyc pack now owns them), 1 → `onboarding_request_dag.yaml` (`client_group_entity_review`); each replaced by a read-only reference stub in cbu_dag. KEPT in cbu_dag (CBU-adjacent, NOT KYC, no separate owning pack): `cbu_corporate_action`, `cbu_evidence`, `cbu_disposition`, `share_class` (fund structure of the CBU). The FSMs are declarative seeds with **zero code/seed consumers** (verified), so the move is bounded. Also fixed 2 residual mines in cbu_dag: the stale `out_of_scope` comment still advertising the removed KYC→CBU-VALIDATED gate, and `cbu_discovery_lifecycle owner: compliance` → `owner: cbu` (CBU structural validation is CBU-owned, not compliance; `owner:` is descriptive-only, not parsed). Verified: `domain-pack-check` all 3 affected packs `publish_required` with NO seed-loader panic / no dangling ref, build clean, C2/C3 green. **CBU⊥KYC decoupling COMPLETE** — every live CBU↔KYC coupling removed: cbu.decide (deleted), structural-KYC gate (removed), reverse-write table (renamed to ubo_*), cbu.inspect (structural-only), foreign FSM ownership (relocated). The only remaining CBU↔KYC links are the intended read-only ones (the operational/transact gate reading KYC clearance, and the ManCo entity pivot))

**KYC/UBO dsl.kyc V&S Program (EOP-VS-KYCUBO-001 v0.6 "From Percentage to Determination") — W1–W7 infrastructure COMPLETE (2026-06-30), determination logic corrected 2026-07-01 (EOP-DD-KYCUBO-003):** Three-crate pure stack (`ob-poc-kyc-substrate` ← `ob-poc-kyc-store` ← `ob-poc-kyc-seam`). W1 durable verb stream: `kyc_intent_events` (K-16 system of record, per-subject ordering Q6, `committed_at DEFAULT clock_timestamp()` B1), §3 append protocol (FOR UPDATE per-subject lock, seq from `next_seq` not a SEQUENCE — gap-free under rollback), `FoldRegistry` (D2 — total per-event dispatch on `lexicon_hash`, hard-error on unregistered hash). W2 reference-plane lexicon: `dsl_verbs.lexicon_hash` + `kyc_lexicon_manifest` (whole-manifest hash Q7, `publish_manifest()`, K-30 lint — covers only the 12 W1/W4 determination verbs; the 11 W3/W5 obligation verbs are not yet in the content-addressed manifest, M2 open). W4 determination: 12 dsl.kyc verbs (`ubo.edge.{assert-control,assert-economic-interest,attach-evidence,verify,supersede,reconcile-conflict}` + `ubo.determination.{select-strategy,compute-fold,apply-smo-fallback,freeze}` + `kyc.subject.{register,classify-structure}`) — 58 legacy write verbs (ubo/control/ownership/board) deleted; tables `control_edges`/`entity_ubos`/`cbu_board_controller` were empty (no active write path). W7 migration: `ubo.compute-chains` deleted, `ownership_prong_strategy` substrate. W3 role-basis: `kyc.role.{assign,withdraw}` — obligation basis recorded, never inferred (K-21). W5 obligation lifecycle: `kyc.obligation.{create,satisfy,waive,update-identity,update-screening,update-risk}` + `kyc.person.{approve,reject}` — parallel tracks Q4; existing `screening.*`/tollgate consumed as inputs, not replaced (V&S §9.2). W6 projections: `kyc_control_edge_projection` + `kyc_obligation_projection` + `kyc_subject_rollup_projection` — disposable folds (K-34), `PgKycProjectionDrainer` + `PgKycObligationDrainer` (FOR UPDATE SKIP LOCKED, per-effect-kind, idempotent/convergent). `as_of` on `VerbExecutionContext` (frozen at dispatch). **EOP-DD-KYCUBO-003 (2026-07-01) — determination-logic and approval-gate remediation:** a fresh code-level re-verification found `ubo.determination.freeze` was bypassing `OwnershipProngStrategy` entirely — it resolved every distinct edge-source as a candidate with no threshold, no economic/control axis split, and no structure-class dispatch (a hand-rolled proxy, the substrate strategy was only exercised by an isolated unit test) — and `kyc.person.approve` had no K-23 gate (approved unconditionally). Both fixed: `freeze` now dispatches to the real `DeterminationStrategy` selected via `kyc.subject.classify-structure`/`ubo.determination.select-strategy` (fails loudly, not silently, for any structure class beyond `ownership_prong_strategy`, which is the only one implemented), applies a threshold (`threshold-pct` arg, default 25.0 pending a governed reference-plane table), and records candidates/basis/prong on the freeze event payload (K-1/K-35); `person.approve` now folds obligations and rejects unless `SubjectOverallState::AllTerminal` (K-23). Also fixed in the same pass: a silent `structure-class`/`structure_class` payload-key bug that left `ControlState.structure_class` always `None`, and freeze's lexicon precondition check being dead code (`stream_append` passed `None` instead of its own FQN, so K-14 reconcile-before-fold never actually ran). RED→GREEN proven via `git stash` (`rust/tests/kyc_m3_remediation.rs`, 4 tests). Still open (M2/M4, unauthorised): lexicon-manifest coverage for the 11 obligation verbs, `ubo.edge.pierce-nominee` (K-8 — nominees are not currently pierced), and a real control-prong strategy for fund-LP/LLP/trust structure classes (Success Criterion 2 unproven against the live path). End-to-end: substrate 19/19, store 13/13, `rust/tests/kyc_*.rs` all green, plugin-coverage PASS, dep-gate PASS. Remaining V&S scope (separate programs): W6 case/workstream projection cutover (cases/entity_workstreams becoming folds); W5 screening hook wiring; `kyc_stream` variant in dsl-core `SourceOfTruth` enum (dep bump needed). Full plan + execution log: `docs/todo/EOP-DD-KYCUBO-003_Determination-Logic-and-Approval-Gate-Remediation.md`. All work on branch `codex/phase-1-5-governance-closure` (not yet merged to main).

**In Progress / Parked (⚠️):** Observatory (Phases 1-7 complete — Rust backend types/projection/routes, egui WASM canvas embedded directly in ChatPage cockpit layout (always visible, center column), 5 level renderers, DAG Identity: all endpoints project from `tos.hydrated_state`, universe root node at session start with 7 workspace children + scoping verbs, canvas navigation via hover/click/double-click routes through REPL input, FlightDeck collapsed status bar, NarrationPanel wired into sidebar; Phase 8 diagrams pending), Sage/Coder GATE 5 (existing 43%, Sage+Coder 5% — vocabulary/routing work needed), Three-Step Harness (7.95% exact / 71% grounded — metadata quality is limiter), StateGraph Phase 1 reconciliation (parked pending external correction table)

**Removed (❌):** V1 Staged Runbook (054), ESPER Navigation Crates (065 — retained for reference), ECIR / NounIndex (Tier -1 noun taxonomy — replaced by ConstellationVerbIndex + workspace pack constraints)

---

## Session & Navigation Verbs

| Verb | Purpose |
|------|---------|
| `session.load-cbu / load-galaxy / load-jurisdiction` | Add CBUs to session |
| `session.unload-cbu`, `session.clear` | Remove CBUs |
| `session.undo / redo`, `session.info / list` | History & state |
| `view.universe / book / cbu` | Zoom levels |
| `view.drill / surface / trace / xray / refine` | Navigation within CBU |
| `nav.drill / zoom-out / select` | Observatory semantic navigation |
| `nav.set-cluster-type / set-lens` | Observatory observation controls |
| `nav.history-back / history-forward` | Observatory navigation history |

All user input goes through unified `IntentPipeline` → `HybridVerbSearcher` → semantic match. No separate ESPER path.

**Session recovery:** `session.recover` creates a fresh session pre-loaded with the old session's client group and workspace context, allowing resumption without re-navigating tollgates.

---

## Adding Verbs

> ⚠️ **Read `docs/verb-definition-spec.md` before writing verb YAML.** Serde structs are strict. Invalid YAML silently fails to load.

**Behaviors:** `crud` (generic executor), `plugin` (`SemOsVerbOp` trait), `template` (multi-statement DSL expansion)

**Plugin verb pattern** (Phase 5c-migrate: single `SemOsVerbOp` trait under the Sequencer-owned transaction scope):

```rust
use sem_os_postgres::ops::SemOsVerbOp;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

pub struct MyDomainCreate;

#[async_trait]
impl SemOsVerbOp for MyDomainCreate {
    fn fqn(&self) -> &str { "my-domain.create" }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        // `scope.executor()` → `&mut PgConnection` inside the ambient txn
        // `scope.pool()` → transitional escape for legacy `&PgPool` helpers
        // `ctx.service::<dyn X>()?` → platform service dispatch
        /* ... */
    }
}
```

**Registration** — manual, no inventory/proc-macro magic:
- Ops living in `sem_os_postgres::ops::*` are added to `sem_os_postgres::ops::build_registry()`.
- Ops living in `rust/src/domain_ops/*` (Pattern B — bridge to ob-poc internals) are added to `ob_poc::domain_ops::extend_registry(&mut SemOsVerbOpRegistry)`.
- `ob-poc-web::main` builds the registry via `build_registry() + extend_registry()` and wires it into `ObPocVerbExecutor`.

**Dispatching-fold pattern** (consolidate N specialist verbs → 1, e.g. `cbu.assign-role`, 2026-06-18): to deduplicate the discoverable surface without losing capability, give the canonical verb a `*-type` selector arg + a dispatching `SemOsVerbOp` that routes to the specialist op structs by type. **Retain the specialist structs but DROP their `register()` calls** — they become unregistered dispatch targets, so every write-path is preserved while only one verb is discoverable. Both coverage invariants stay green (no orphan ops: the registry has exactly the verbs declared `behavior: plugin` in YAML). Re-home the folded verbs' `invocation_phrases` onto the canonical, retarget DAG/constellation refs, then `cargo x verbs compile` + repopulate embeddings + prune the removed verbs' stale rows from `verb_pattern_embeddings`.

**Shared fold-verb dispatch helper** (`sem_os_postgres::ops::selector_dispatch`, 2026-07-15): the selector-arg-below-entity-type pattern (e.g. `cbu.assign-role`'s `role-type`, `client-group.entity-manage`'s `action`, `gleif.lookup`'s `target-type`) previously had 3 independent hand-rolled implementations with no shared contract — exactly the duplication that let `cbu.create`'s internal cascade call target an unregistered FQN (`cbu.assign-fund-role`, orphaned by the 2026-06-18 fold) silently break at runtime for a month. Use `resolve_selector(args, arg_name, arms)` (case-insensitive arm lookup, returns `Matched`/`Absent`/`Unrecognized` — pick this when the fold verb must fall back to non-selector behavior, as `cbu.assign-role` does) or `dispatch_selector(...)` (strict — `Absent`/`Unrecognized` both error listing valid values; pick this for a pure fold with no fallback, as `client-group.entity-manage` and `gleif.lookup` do). If an op dispatches through an indirection (e.g. `gleif.lookup`'s `Self::resolve()`, needed because both `pre_fetch` and `execute` resolve the same arm), `cargo x registry-graph`'s fold-verb detector (below) won't see it — it only scans an op's own `execute()` body.

**Selector-arg `valid_values` is mandatory, always the wire string, not a description-only enum** (2026-07-15): every selector/strategy arg dispatched via `selector_dispatch` — or matched via a hand-rolled `match` on a string arg, like `ubo.determination.freeze`'s `strategy` dispatch on `select-strategy`'s recorded value — MUST declare `valid_values` in the verb YAML using the *exact* runtime match string, not a prose description of it. Two real bugs found by auditing `kyc.subject.classify-structure`/`ubo.determination.select-strategy` for this after building `selector_dispatch`: (1) `structure-class` had no `valid_values` at all — the 11-variant `StructureClass` taxonomy (`ob-poc-kyc-substrate/src/fold/control.rs`) existed only in Rust, invisible at the config layer the way CBU's `role-type` `valid_values: [ROLE, OWNERSHIP, ...]` is; (2) `strategy`'s description said `"ownership_prong or smo_fallback"` while the only real `DeterminationStrategy::name()` is `"ownership_prong_strategy"` — a caller or an LLM doing arg extraction from the description alone would pass a string that fails at `freeze` time. `valid_values` is descriptive metadata (feeds Sage arg extraction + SemOS `AttributeDataType::Enum` projection, not a DSL-parse-time hard reject — confirmed via `crates/sem_os_obpoc_adapter/src/scanner.rs:977`), so widening it costs nothing; the risk is entirely on the side of leaving it absent or wrong. When a selector's valid range includes not-yet-implemented arms (e.g. `structure-class`'s `llp`/`trust`/fund classes, real per `StructureClass` but with no control-prong `DeterminationStrategy` behind them — M4, open), list them anyway and say so in `description` — the taxonomy is real even when the strategy behind an arm isn't built yet; that's a different, separately-tracked gap (see KYC/UBO section above).

```bash
cargo x verbs check   # YAML matches DB
cargo x verbs lint    # Tiering rules (reads the canonical SemOS registry manifest)
# After YAML changes — MUST run or new verbs won't be discoverable:
cargo x verbs compile && DATABASE_URL="postgresql:///data_designer" \
  cargo run --release -p ob-semantic-matcher --bin populate_embeddings
```

Verify plugin coverage: `cargo test -p ob-poc --lib -- test_plugin_verb_coverage`

---

## Code Patterns

- **Config Struct Pattern:** Builder-style for types with many optional params. See `ToolHandlersConfig` in `rust/src/mcp/handlers/core.rs`.
- **Centralized DB Access:** All through service modules in `rust/src/database/`. No direct `sqlx::query` outside services.
- **Actor Resolution:** `ActorResolver::from_headers()` (HTTP), `from_env()` (MCP), `from_session_id()` (REPL). Default role: `viewer`.
- **Strum-based enums:** Core enums use `strum` derives (`Display`, `EnumString`, `AsRefStr`) — eliminates manual `as_str()`/`from_str()`.
- **`#[must_use]` on decision types:** `GateResult`, `ValidationReport`, `DryRunReport` must not be silently discarded.

---

## Key Directories

```
ob-poc/
├── bpmn-lite/                  # Consolidated platform repo — own git repo (github.com/adamtc007/bpmn-lite)
│   │                           # 18 workspace crates after B0 consolidation:
│   │                           #   bpmn-lite-{types,compiler,vm,engine,store,store-postgres,authoring,server}
│   │                           #   ffi-{types,catalogue,dispatcher}
│   │                           #   dmn-lite-{types,parser,compiler,engine,analysis,bridge}
│   │                           #   xtask
│   └── CLAUDE.md               # Unified guide for both vocabularies (crate map, A-phases, dmn-lite roadmap)
├── observatory-wasm/             # Observatory egui constellation canvas (WASM, embedded in React)
│   ├── src/                      # Canvas renderer, level painters, observation controls
│   └── pkg/                      # wasm-pack output (WASM + JS glue, served at /observatory/pkg/)
├── ob-poc-ui-react/            # React/TypeScript frontend (PRIMARY UI)
│   ├── src/features/           # Chat, Inspector, Semantic OS, Settings
│   └── dist/                   # Production build (served by Rust)
├── rust/
│   ├── config/verbs/           # 103 YAML verb definitions
│   ├── config/packs/           # 5 V2 REPL journey packs (onboarding, book-setup, kyc-case, deal-lifecycle, product-service-taxonomy)
│   ├── config/sem_os_seeds/    # Domain metadata, constellation maps, state machines
│   ├── config/scenario_index.yaml  # Journey scenarios (assemble-cbu + KYC + cross-border + SemOS + product)
│   ├── crates/
│   │   ├── dsl-core/           # Parser, AST, compiler (no DB)
│   │   ├── dsl-lsp/            # LSP server + Zed extension + tree-sitter grammar
│   │   ├── ob-agentic/         # Onboarding pipeline (Intent→Plan→DSL)
│   │   ├── ob-poc-macros/      # Proc macros (#[derive(IdType)] only)
│   │   ├── ob-poc-web/         # Axum web server (serves React + API)
│   │   ├── inspector-projection/ # Projection schema generation
│   │   ├── sem_os_core/        # Canonical types, ports, service logic
│   │   ├── sem_os_postgres/    # PostgreSQL store implementations
│   │   ├── sem_os_server/      # Standalone REST server + JWT
│   │   ├── sem_os_client/      # Client trait (InProcess + HTTP)
│   │   ├── sem_os_harness/     # Integration test harness
│   │   └── sem_os_obpoc_adapter/ # Verb YAML → seed bundles
│   ├── src/
│   │   ├── agent/              # Orchestrator, verb surface, onboarding state, context envelope
│   │   ├── repl/               # V2 REPL (30 files, always enabled) + session_trace, trace_repository, session_replay
│   │   ├── journey/            # Pack system (router, manifests, handoff)
│   │   ├── domain_ops/         # Pattern B SemOsVerbOp ops that bridge to ob-poc internals (9 files, 119 ops: onboarding, bpmn_lite, template, source_loader, request, gleif, booking_principal, trading_profile + rule_evaluator utility). Registered via `extend_registry()`.
│   │   ├── sem_reg/            # Semantic Registry + stewardship (39 files)
│   │   ├── mcp/                # MCP tools, handlers, verb search, intent pipeline
│   │   ├── bpmn_integration/   # ob-poc ↔ bpmn-lite wiring (12 files)
│   │   ├── calibration/        # Loopback calibration (11 modules)
│   │   ├── cross_workspace/   # Cross-workspace state consistency (10 modules)
│   │   └── api/                # REST routes
│   ├── tests/                  # External test harnesses (public API only)
│   │   └── fixtures/macros/    # YAML macro fixtures for runbook harnesses
│   └── scenarios/suites/       # 9 suites, 44 agentic test scenarios
├── migrations/                 # 128 SQLx migrations
├── docs/                       # Current architecture truth + appendices
├── ai-thoughts/                # Historical design notes referenced by code/docs; not authoritative
└── artifacts/                  # Calibration packs, footprints, peer review
```

---

## Environment Variables

```bash
# Required
DATABASE_URL="postgresql:///data_designer"
AGENT_BACKEND=anthropic
ANTHROPIC_API_KEY="sk-ant-..."
ANTHROPIC_MODEL=claude-sonnet-4-6

# Optional
BPMN_LITE_GRPC_URL=http://localhost:50052   # Enable BPMN integration (existing gRPC verbs)
BPMN_LITE_DATABASE_URL=postgresql:///bpmn_lite  # Enable loader.* + bpmn-controller.* verbs
SEM_OS_MODE=inprocess                        # inprocess | remote
SEM_OS_DATABASE_URL="postgresql:///..."      # For standalone sem_os_server
SEM_OS_JWT_SECRET=dev-secret                 # JWT for sem_os_server
OBPOC_STRICT_SINGLE_PIPELINE=true            # PolicyGate (default: true)
OBPOC_STRICT_SEMREG=true                     # SemReg fail-closed (default: true)
# OBPOC_ALLOW_RAW_EXECUTE removed 2026-04-22 (Slice 3.1) — raw DSL in request
# bodies is now always rejected; no flag can reopen the bypass.
SAGE_FAST_PATH=1                             # Read+structure fast path
BRAVE_SEARCH_API_KEY="..."                   # Research macros
```

---

## Database Practices

```bash
# After schema changes
psql -d data_designer -f your_migration.sql
cd rust && cargo sqlx prepare --workspace
cargo build  # Catches type mismatches
```

| PostgreSQL | Rust |
|------------|------|
| UUID | `Uuid` |
| TIMESTAMPTZ | `DateTime<Utc>` |
| INTEGER / BIGINT | `i32` / `i64` |
| NUMERIC | `BigDecimal` |
| NULLABLE | `Option<T>` |

---

## Error Handling

Never `.unwrap()` in production. Use `?`, `.ok_or_else(|| anyhow!(...))`, `let Some(x) = ... else { continue }`, `match` / `if let`.

---

## Testing

### Backend Test Suites

**Test boundary rule:** Tests are either crate-internal (`#[cfg(test)]` inside `src/`) or external harnesses (`rust/tests/` or `crates/<subcrate>/tests/`). No test crosses crate boundaries — external tests use only the crate's public API. Internal tests may access `pub(crate)` types and `include_str!` source files.

```bash
# Unified pipeline tollgate tests (internal module tests under ob-poc)
cargo test -p ob-poc --lib integration_tests::unified_pipeline_tollgates

# Runbook pipeline harnesses (external, public API only, YAML macro fixtures)
cargo test --test runbook_e2e_test --test runbook_pipeline_test

# Runbook source-scanning invariant tests (internal, include_str!)
cargo test -p ob-poc --lib -- runbook::invariant_tests

# Full REPL V2 suite (internal module tests under ob-poc)
cargo test -p ob-poc --lib integration_tests::repl_v2

# Sub-crate internal capability tests
cargo test -p dsl-runtime --lib
cargo test -p sem_os_postgres --lib
cargo test -p ob-semantic-matcher --lib

# Intent hit rate (needs DATABASE_URL)
DATABASE_URL="postgresql:///data_designer" cargo test --features database --test intent_hit_rate -- --ignored --nocapture
```

**Test fixture macros:** `rust/tests/fixtures/macros/*.yaml` — YAML macro definitions loaded by external harnesses via `load_macro_registry_from_dir`. Avoids constructing internal `MacroSchema` types in external tests.

### Chrome DevTools MCP — Live UI Testing

Automated browser testing via Chrome DevTools MCP. Claude Code can navigate, type, click, screenshot, inspect DOM, and verify console errors against the live UI.

**Setup:** Chrome with remote debugging enabled + `chrome-devtools-mcp` in `.mcp.json`.

**Capabilities:**
- **Smoke tests:** Automated tollgate flow (scope → workspace → journey → pack → verb)
- **Screenshot regression:** Capture expected states, compare on changes
- **Demo animations:** Scripted user flows at human-readable pace for presentations
- **Bulk phrase testing:** Fire 100+ utterances through the live UI, verify each gate renders
- **Console error detection:** Zero-error verification after every flow step
- **Network inspection:** Verify API response shapes match frontend expectations

**Test fixture:** `tests/fixtures/ui_smoke_test.toml` — 12 test cases + 1 demo sequence covering all 3 tollgates.

**Usage in Claude Code:**
```
@chrome navigate to http://localhost:3000 and create a new session
@chrome type "Allianz" and press Enter, then screenshot
@chrome click the CBU workspace button and verify journey options appear
@chrome check for console errors
```

---

## Verification Strategy — Which Tool Answers Which Question (xtask)

Three different mechanisms answer three different classes of "is this code
dead / does this dispatch twice / does this actually work" question. Picking
the wrong one either produces false positives (flagging live code as dead)
or silently misses real findings (a tool that can't see the thing it's being
asked about). The dividing line is dispatch shape, not question type:

1. **Static analysis — for anything closed-enum/match dispatched.**
   `dead_code = "deny"` (workspace-wide, every crate's `[lints.rust]`),
   `cargo +nightly public-api` (per-crate ratchet against committed
   `audits/surface/<crate>.txt` snapshots — `scripts/check-public-api-surface.sh`
   for the exact-match gate, `scripts/check-no-widening.sh` for an
   additions-only guard during a deletion sweep), and rust-analyzer/editor
   call hierarchy for interactive one-off tracing. This tier works because
   the compiler (and any tool built on its same AST/type info) can resolve
   every call site at compile time — `ExecutionResult`, `CrudOperation`,
   orchestrator `match` arms, anything reached by a named function call.
   It does **not** work across a `dyn Trait` boundary: a call through
   `&dyn Trait` has no static edge back to a concrete impl, so this tier
   run against `dyn`-dispatched code produces a "dead code" report that's
   actually just the entire live surface (confirmed 2026-07-15 before
   building anything — see tier 2).

2. **Registry-data extraction — for anything behind `dyn` in a
   runtime-keyed registry.** `SemOsVerbOp` is `Arc<dyn SemOsVerbOp>` in a
   `HashMap<String, _>` (`sem_os_postgres::ops::SemOsVerbOpRegistry`),
   dispatched by an FQN string the DSL compiler emits from YAML — the only
   static edge is `registry.register(Arc::new(ConcreteType))`, a
   *construction*, not an *invocation*, so tier 1 tooling is structurally
   blind here. The fix relocates the analysis from the dispatch site
   (unresolvable) to the registration site (fully resolvable — a plain
   function call with a string-bearing concrete type as its argument,
   visible in the same compiled workspace that erases it to `dyn`
   everywhere else). `cargo x registry-graph`
   (`xtask/src/registry_graph.rs`) implements this today as static `syn`
   extraction of every `build_registry()`/`extend_registry()` call site,
   resolved to FQNs via each type's `fqn()` impl (direct, or via one of
   17 local `macro_rules!` helpers, or two `const`-table loop-registration
   special cases), diffed against the YAML `behavior: plugin` verb set —
   a completeness diff between two declared corpora, not a call-graph
   walk. (A live *registry pre-run* — actually calling
   `build_registry()`/`extend_registry()` and reading `.fqn()` off the
   real `dyn` objects, optionally capturing `std::any::type_name::<T>()`
   at a generic call site before erasure — is the lower-maintenance
   version of the same principle: it can't miss a new macro shape the way
   the static extractor can, since running the real registration code
   doesn't require recognizing its syntax. The static extractor was
   chosen first because it needed no new registry API; revisit if the
   macro-shape surface keeps growing.) Verified 2026-07-15: 766 registered
   ops ↔ 766 YAML `plugin` verbs, exact match, 0 dead code / 0 missing
   registrations / 0 dual-routing — cross-validated against the
   independent `test_plugin_verb_coverage` test, which also passes.
   Extended 2026-07-15 to the one intra-registry composition mechanism
   that exists (`SemOsChildDispatcher::dispatch_child`, confirmed the sole
   registry-callback path and confirmed 100% literal-FQN, never a
   runtime-computed string — see the dead-code-candidates check above for
   why that distinction matters): `cargo x registry-graph` now also
   extracts every parent→child composition edge, flags dangling child FQNs
   (caught the `cbu.create`→`cbu.assign-fund-role` break — see
   `selector_dispatch` below), and detects fold verbs (ops using the
   strict `dispatch_selector` shape) to flag any composition edge that
   omits the required selector arg. Known scope gap, documented in the
   tool's own report output: fold-verb detection only scans an op's own
   `execute()` body, not helper functions it delegates through.

3. **Harness / tracing — last resort, for genuine cross-process or
   concurrency questions neither tier above can answer.** Both tiers 1
   and 2 are static: they describe what the compiled workspace *can*
   reach, not what actually happens across a real request, a real DB
   transaction, or concurrent sessions. Reach for this tier only when the
   question is inherently about runtime behavior across a boundary static
   analysis can't see through — the Agentic Scenario Harness
   (`cargo x harness run`, see Quick Start), Chrome DevTools MCP live UI
   smoke tests, or `session_trace`/`session_replay` — not as a default
   first move for "is this code used."

Full context: `docs/research/control-plane-ownership-ledger.md`
("Dead code & dual-routing static sweep").

---

## Domain Quick Reference

| Domain | Verbs | Purpose |
|--------|-------|---------|
| `cbu` | 25 | Client Business Unit lifecycle |
| `entity` | 30 | Natural/legal person management |
| `session` | 16 | Scope, navigation, history |
| `view` | 15 | Navigation verbs |
| `trading-profile` | 30 | Trading matrix, CA policy |
| `kyc` | 20 | KYC case management |
| `investor` | 15 | Investor register, holdings |
| `custody` | 40 | Settlement, safekeeping |
| `gleif` | 15 | LEI lookup, hierarchy import |
| `research.*` | 30+ | External source workflows |
| `contract` | 14 | Legal contracts, rate cards, subscriptions |
| `document` | 7 | Document solicitation, verification |
| `deal` | 30 | Deal lifecycle, rate card negotiation |
| `billing` | 14 | Fee billing profiles, periods |
| `ownership` | 4 | Ownership graph pipeline |
| `registry` | 20 | SemReg object CRUD |
| `changeset` | 14 | Changeset authoring |
| `governance` | 9 | Publish gates, impact, rollback |
| `schema` | 5 | Schema introspection |
| `agent` | 4+ | Agent mode/policy, telemetry |
| `sem_reg.*` | ~32 | Semantic Registry MCP tools |
| `nav` | 7 | Observatory semantic navigation (drill, zoom, select, lens, history) |
| `shared-atom` | 8+8 macros | Cross-workspace shared atom registry, replay, acknowledge, batch ops |
| `remediation` | 4+2 macros | Remediation event lifecycle (defer, revoke, confirm, audit trail) |

---

## Domain Annexes

**Detailed implementation docs extracted from CLAUDE.md into topic-specific annexes:**

| When working on... | Read this annex |
|--------------------|-----------------|
| DSL pipeline, verb search, embeddings, intent resolution, disambiguation, teaching, promotion, scenarios, AffinityGraph, discovery | `docs/annex-dsl-and-intent.md` |
| Semantic OS, SemReg, context resolution, ABAC, stewardship, governed authoring, CCIR, verb surface, scanner, domain-pack reload, **v1.3 cross-workspace runtime stack** (DagRegistry, GateChecker, DerivedStateEvaluator, CascadePlanner, GatePipeline) | `docs/annex-sem-os.md` |
| Sem OS domain-pack YAML ownership, reload index, timestamp/hash refresh, publication boundary | `docs/architecture/sem-os-domain-pack-taxonomy-reload.md` |
| BPMN-Lite service, fiber VM, race semantics, gRPC, orchestration, bpmn_integration | `docs/annex-bpmn-lite.md` |
| V2 REPL, packs, scoring, preconditions, context stack, golden corpus, replay tuner | `docs/annex-repl-v2.md` |
| Macros: operator vocabulary, expansion engine, MacroIndex, lint, composite macros, state DAG, pack mapping | `docs/annex-macros.md` |
| Contracts, deals, billing, client groups, documents, entity linking, inspector, lexicon, lookup, playbooks, transactional execution | `docs/annex-domain-features.md` |
| React frontend details, Zed extension, LSP, ob-agentic onboarding pipeline | `docs/annex-frontend-and-tools.md` |
| ACP: stdio JSON-RPC server, REST surface, `AcpFacade`, projection live-overlay model, persona modes | `docs/annex-acp.md` |
| Observatory: egui WASM app, OrientationContract, GraphSceneModel, navigation verbs | `docs/observatory-implementation-plan.md` |
| **Catalogue Platform v1.3** — DAG taxonomies, cross_workspace_constraints (Mode A blocking), derived_cross_workspace_state (Mode B aggregation/tollgate), parent_slot + state_dependency (Mode C cascade), TransitionArgs verb metadata, GatePipeline | `docs/todo/catalogue-platform-refinement-v1_3.md` |

**Pre-existing annexes (unchanged):**

| When working on... | Read this annex |
|--------------------|-----------------|
| Semantic pipeline details | `docs/agent-semantic-pipeline.md` |
| Agent/MCP pipeline | `docs/agent-architecture.md` |
| Session & navigation | `docs/session-visualization-architecture.md` |
| Data model (CBU/Entity/UBO) | `docs/strategy-patterns.md` §1 |
| Verb authoring | `docs/verb-definition-spec.md` |
| Entity model & schema | `docs/entity-model-ascii.md` |
| Schema overview (living doc) | `migrations/OB_POC_SCHEMA_ENTITY_OVERVIEW.md` |
| DSL pipeline flow | `docs/dsl-verb-flow.md` |
| Research workflows | `docs/research-agent-annex.md` |
| V2 REPL invariants | `docs/INVARIANT-VERIFICATION.md` |

### AI-Thoughts (Historical Notes)

These files are transitory historical notes. Use them for implementation background only; canonical architecture truth lives in this file, `docs/semos_arhitecture.md`, and the annexes listed above.

| Topic | Document |
|-------|----------|
| Group/UBO ownership | `ai-thoughts/019-group-taxonomy-intra-company-ownership.md` |
| Research workflows | `ai-thoughts/020-research-workflows-external-sources.md` |
| Entity disambiguation | `ai-thoughts/025-entity-disambiguation-ux.md` |
| Trading matrix pivot | `ai-thoughts/027-trading-matrix-canonical-pivot.md` |
| Entity resolution | `ai-thoughts/033-entity-resolution-wiring-plan.md` |
| REPL state model | `ai-thoughts/034-repl-state-model-dsl-agent-protocol.md` |
| Session-runsheet-viewport | `ai-thoughts/035-session-runsheet-viewport-integration.md` |
| Solar navigation | `ai-thoughts/038-solar-navigation-unified-design.md` |
| Lexicon service | `ai-thoughts/072-lexicon-service-implementation-plan.md` |
| Entity linking | `ai-thoughts/073-entity-linking-implementation-plan.md` |

## Java 25 DOP CBU Port Strategy

The Client Business Unit (CBU) status transition and validation engine has been ported from Rust to a Java 25 Data-Oriented Programming (DOP) architecture.

### Architectural Rules
1. **Purity Bound:** The `com.ob.poc.cbu.model` package contains pure functional business logic. No database access, Spring Framework annotations, Hibernate, or ORM frameworks are permitted within this package.
2. **Sealed Class Gate Discipline Pattern:**
   - Transition guards and state eligibility checks switching over sealed status types (`ValidationState`, `OperationalState`, `DispositionState`) **MUST NOT** use runtime string comparisons (e.g., matching on `.rawStatus().toUpperCase()`).
   - Every switch expression **MUST be default-free** (no `default ->` or `case ... default` branches). This guarantees compile-time exhaustiveness checks.
   - Every switch expression **MUST** explicitly enumerate every permitted sub-record type of the sealed interface, along with a `case null ->` branch.
   - The Java compiler acts as the exhaustiveness validator: any future addition of a new state to the sealed hierarchy must immediately trigger compiler errors at all switch guard locations, preventing unhandled states at runtime.
3. **State Machines:**
   - `ValidationState`: `ValidationPending`, `Validated`, `ValidationFailed`, `UpdatePendingProof`, `Evidenced`.
   - `OperationalState`: `PreValidated`, `OperationallyActive`, `Suspended`, `Restricted`, `WindingDown`, `Offboarded`, `Dormant`, `Archived`.
   - `DispositionState`: `Active`, `UnderRemediation`, `SoftDeleted`, `HardDeleted`.
4. **Optimistic Guarding:**
   - Database status updates in `CbuExecutor` must capture the rowcount of the update effect.
   - If a guarded update affects 0 rows (due to a concurrent modification or stale state), the transaction must be rolled back and a `Failure` outcome returned.
5. **Differential Verification:** All updates, queries, and status transitions are validated via `CbuPortTest` through differential testing against the Rust reference implementation.

---

## Trigger Phrases

When you see these in a task, read the corresponding annex first:

| Phrase | Read |
|--------|------|
| "add verb", "create verb", "verb YAML" | `docs/verb-definition-spec.md` |
| "React", "frontend", "chat UI", "scope panel", "constellation panel" | `docs/annex-frontend-and-tools.md` |
| "entity model", "CBU", "UBO", "holdings" | `docs/strategy-patterns.md` §1 |
| "schema overview", "table structure", "ER diagram", "mermaid" | `migrations/OB_POC_SCHEMA_ENTITY_OVERVIEW.md` |
| "agent", "MCP", "verb_search", "intent pipeline", "orchestrator" | `docs/annex-dsl-and-intent.md` |
| "session", "scope", "navigation", "ESPER", "ViewState" | `docs/session-visualization-architecture.md` |
| "DSL pipeline", "PolicyGate", "single pipeline" | `docs/annex-dsl-and-intent.md` |
| "embeddings", "Candle", "BGE", "populate_embeddings" | `docs/annex-dsl-and-intent.md` |
| "promotion", "teaching", "learning", "phrase", "blocklist" | `docs/annex-dsl-and-intent.md` |
| "disambiguation", "VerbOption", "intent tier", "clarification" | `docs/annex-dsl-and-intent.md` |
| "ScenarioIndex", "MacroIndex", "CompoundSignals", "ConstellationVerbIndex" | `docs/annex-dsl-and-intent.md` |
| "AffinityGraph", "DiagramModel", "MermaidRenderer", "DomainMetadata" | `docs/annex-dsl-and-intent.md` |
| "discovery", "registry.discover-dsl", "schema.generate" | `docs/annex-dsl-and-intent.md` |
| "semantic registry", "sem_reg", "semantic os", "context resolution" | `docs/annex-sem-os.md` |
| "ABAC", "security label", "governance tier", "trust class", "proof rule" | `docs/annex-sem-os.md` |
| "stewardship", "changeset", "guardrails", "show loop", "focus" | `docs/annex-sem-os.md` |
| "authoring", "propose", "validate", "dry-run", "publish", "AgentMode" | `docs/annex-sem-os.md` |
| "CCIR", "ContextEnvelope", "PruneReason", "AllowedVerbSetFingerprint", "TOCTOU" | `docs/annex-sem-os.md` |
| "SessionVerbSurface", "verb surface", "FailClosed", "safe-harbor" | `docs/annex-sem-os.md` |
| "GroundedActionSurface", "pipeline leak", "TOCTOU recheck" | `docs/annex-sem-os.md` |
| "v1.3", "DagRegistry", "GateChecker", "GatePipeline", "TransitionArgs", "transition_args" | `docs/annex-sem-os.md` + `docs/todo/catalogue-platform-refinement-v1_3.md` |
| "cross_workspace_constraints", "derived_cross_workspace_state", "parent_slot", "state_dependency", "tollgate", "operationally_active" | `docs/annex-sem-os.md` |
| "DerivedStateEvaluator", "DerivedStateProjector", "CascadePlanner", "SqlPredicateResolver", "SlotStateProvider", "PostgresChildEntityResolver" | `docs/annex-sem-os.md` |
| "DAG taxonomy", "dag_taxonomies", "overall_lifecycle", "dual_lifecycle", "category_gated", "periodic_review_cadence" | `docs/annex-sem-os.md` |
| "scanner", "drift detection", "bootstrap", "seed bundle" | `docs/annex-sem-os.md` |
| "domain pack", "domain-pack reload", "reload index", "taxonomy reload", "publish_required", "domain_pack_reload_index" | `docs/architecture/sem-os-domain-pack-taxonomy-reload.md` + `docs/annex-sem-os.md` |
| "shared atom", "cross-workspace", "staleness propagation", "constellation replay", "remediation event" | `docs/annex-cross-workspace-state-consistency.md` |
| "RebuildContext", "shared fact version", "workspace fact ref", "produces_shared_facts" | `rust/src/cross_workspace/` |
| "BPMN", "bpmn-lite", "fiber VM", "orchestration", "durable workflow" | `docs/annex-bpmn-lite.md` |
| "race", "boundary timer", "cancel", "ghost signal", "terminate" | `docs/annex-bpmn-lite.md` |
| "WorkflowDispatcher", "JobWorker", "EventBridge", "correlation", "parked token" | `docs/annex-bpmn-lite.md` |
| "PendingDispatch", "queue resilience", "dispatch worker" | `docs/annex-bpmn-lite.md` |
| "EndTerminate", "error boundary", "ErrorRoute", "BusinessRejection" | `docs/annex-bpmn-lite.md` |
| "IncCounter", "BrCounterLt", "bounded loop", "ForkInclusive", "JoinDynamic" | `docs/annex-bpmn-lite.md` |
| "PostgresProcessStore", "bpmn migrations", "authoring pipeline" | `docs/annex-bpmn-lite.md` |
| "REPL", "V2 REPL", "orchestrator v2", "context stack", "pack" | `docs/annex-repl-v2.md` |
| "scoring", "preconditions", "golden corpus", "replay tuner" | `docs/annex-repl-v2.md` |
| "VerbSearchIntentMatcher", "IntentMatcher", "3-pronged" | `docs/annex-repl-v2.md` |
| "compile_invocation", "CompiledRunbook", "RunbookStore", "execute_runbook" | `docs/annex-repl-v2.md` |
| "FocusMode", "DecisionLog", "ExclusionSet" | `docs/annex-repl-v2.md` |
| "macro", "operator vocabulary", "structure.setup", "MacroSchema", "expands-to" | `docs/annex-macros.md` |
| "invoke-macro", "MacroExpansionStep", "MacroIndex", "macro lint", "MACRO0" | `docs/annex-macros.md` |
| "screening-ops", "screening.full", "macro search overrides", "macro audit" | `docs/annex-macros.md` |
| "SequenceValidator", "CompoundSignals", "FixpointExpansion", "macro DAG" | `docs/annex-macros.md` |
| "PACK001", "workspace bleed", "mode-tags", "workspace_accepts_mode_tag", "fail closed" | `docs/annex-macros.md` |
| "new workspace", "add workspace", "WorkspaceKind", "workspace checklist" | `docs/annex-macros.md` |
| "constraint cascade" | `docs/annex-domain-features.md` |
| "contract", "deal", "billing", "rate card", "subscription" | `docs/annex-domain-features.md` |
| "client group", "alias", "anchor", "resolver" | `docs/annex-domain-features.md` |
| "entity linking", "mention extraction", "EntityLinkingService" | `docs/annex-domain-features.md` |
| "lexicon", "LexiconService", "bincode snapshot" | `docs/annex-domain-features.md` |
| "lookup service", "verb-first", "LookupService" | `docs/annex-domain-features.md` |
| "document", "requirement", "task queue", "cargo ref", "rejection" | `docs/annex-domain-features.md` |
| "inspector", "projection", "node_id", "ref_value" | `docs/annex-domain-features.md` |
| "transactional execution", "advisory lock", "expansion report" | `docs/annex-domain-features.md` |
| "skeleton build", "KYC case transition", "import run" | `docs/annex-domain-features.md` |
| "OnboardingStateView", "constellation", "forward_verbs", "revert_verbs" | `docs/annex-domain-features.md` |
| "playbook", "LSP", "language server", "Zed extension", "tree-sitter" | `docs/annex-frontend-and-tools.md` |
| "onboarding pipeline", "RequirementPlanner", "ob-agentic" | `docs/annex-frontend-and-tools.md` |
| "invariant", "P-1", "P-2", "P-3", "P-4", "P-5" | `docs/INVARIANT-VERIFICATION.md` |
| "dead code", "unused", "dual routing", "registry-graph", "SemOsVerbOpRegistry", "no-widening", "cargo public-api" | Verification Strategy section above, `docs/research/control-plane-ownership-ledger.md` |
| "runbook plan", "RunbookPlan", "multi-workspace execution", "plan compiler" | `rust/src/runbook/plan_compiler.rs`, `rust/src/runbook/plan_types.rs` |
| "session trace", "TraceEntry", "TraceOp", "trace replay", "session_replay" | `rust/src/repl/session_trace.rs`, `rust/src/repl/session_replay.rs` |
| "plan executor", "advance_plan_step", "forward ref", "EntityBinding" | `rust/src/runbook/plan_executor.rs`, `rust/src/runbook/plan_types.rs` |
| "narration", "StepNarration", "PlanNarration", "effect narration" | `rust/src/runbook/narration.rs` |
| "NarrationEngine", "NarrationPayload", "contextual query", "what's next", "suggested_next", "narration boost" | `rust/src/agent/narration_engine.rs`, `rust/crates/ob-poc-types/src/narration.rs` |
| "VerbOutput", "verb output", "outputs declaration" | `rust/crates/sem_os_core/src/verb_contract.rs` |
| "stack machine", "workspace stack", "writes_since_push", "is_peek" | `rust/src/repl/types_v2.rs`, `rust/src/repl/session_v2.rs` |
| "observatory", "Observatory", "OrientationContract", "GraphSceneModel", "ViewLevel", "egui WASM", "DAG identity", "viewport state" | `docs/observatory-implementation-plan.md` |
| "nav.drill", "nav.zoom-out", "nav.select", "navigation verbs", "observation lens" | `rust/config/verbs/navigation.yaml`, `rust/src/domain_ops/navigation_ops.rs` |
| "graph scene", "SceneNode", "SceneEdge", "LayoutStrategy", "DrillTarget" | `rust/crates/ob-poc-types/src/graph_scene.rs`, `rust/crates/sem_os_core/src/observatory/graph_scene_projection.rs` |

---

## Deprecated / Removed

| Removed | Replaced By |
|---------|-------------|
| `ViewMode` enum (5 modes) | Unit struct (always TRADING) |
| `OpenAIEmbedder` / `all-MiniLM-L6-v2` | `CandleEmbedder` / `bge-small-en-v1.5` |
| `ob-poc-ui` (egui) / `esper_*` crates | React frontend (`ob-poc-ui-react/`) |
| V1 REPL / `ReplState` / `ClientContext` | V2 REPL (`orchestrator_v2.rs`, `ContextStack`) |
| `ob-poc-graph` / `viewport` crate | React + REST API |
| `SemRegVerbPolicy` | `SemOsContextEnvelope` |
| Direct DSL bypass (`dsl:` prefix) | SemReg-filtered pipeline |
| `IntentPipeline` (V1 agent chat) | `ReplOrchestratorV2.process()` |
| V1 Staged Runbook (054) | V2 REPL pack-guided runbook |
| `manco` domain name | Renamed to `ownership` domain |
| `CbuSession` / `cbu_session_routes.rs` | Unified pipeline (`session_scoped_router`) |
| `agent_dsl_routes.rs` (DSL parse/resolve/generate) | Unified REPL pipeline |
| `agent_learning_routes.rs` (corrections/disambiguation) | Unified REPL pipeline |
| `vnext-repl` feature flag | Always enabled (flag deprecated) |
| Legacy `chat_session()` fallback in `session_input` | `try_route_through_repl()` |
| ECIR / NounIndex (Tier -1 noun taxonomy, `noun_index.rs`, `noun_index.yaml`) | ConstellationVerbIndex (Tier -0.5) + workspace pack constraints |
