# ob-poc v0.2 Release Notes

**Tag**: `impl/v0.2`
**Date**: 2026-05-22
**Previous**: `impl/v0.1` (2026-05-21)

## Summary

v0.2 delivers production Sage, full variable-arity decision packs, a Camunda 8 migration tool, an SVG diagram renderer, PostgresJourneyStore, and an automated compliance pilot harness. The v&s paper's central claim — AI-safe participation in real system work — is now backed by working infrastructure.

---

## What's new

### Sage AI authoring (Tranches 0–4)

**All 12 decision packs support full variable arity.** The `for-each` template combinator (Tranche 0) enables packs 3, 4, 5, 6, 7, 8, 10 to generate N atoms from a list parameter — previously limited to fixed-arity forms.

**Sage pack matching** (Tranche 1): utterance → ranked pack candidates with confidence scores. BagOfWords baseline achieves 96% top-1 accuracy on the 50-utterance evaluation set. BGE model integration is a direct `PackEmbedder` upgrade (architecture unchanged).

**Parameter extraction** (Tranche 2): selected pack → parameter proposals with confidence + rationale. Confirmation state machine with edit loop: `Pending → (EditParameter)* → Accepted | Rejected | Cancelled`.

**DSL instantiation** (Tranche 3): confirmed parameters → structural atoms + `(provenance ...)` atom. Compile validation runs the full parse→assemble pipeline before presenting to user.

**End-to-end orchestrator** (Tranche 4): `SageOrchestrator` drives the full state machine (`Listening → Matching → Confirming → Instantiated → Deployed`). `SageSessionStore` provides thread-safe session management for REPL integration. In-memory `SageAuditLog` records all transitions.

### Supporting capabilities

**Camunda 8 migration tool** (Tranche 5, `dsl-migrate`): XML in, bpmn-lite DSL out. Handles all Camunda 8 element types; FEEL expressions marked `[HUMAN-RESOLVE]`; complex gateway rejected with diagnostic; verb resolution against known SemOS verb patterns. Coverage report (Clean/HumanResolve/Rejected/Skipped) + CLI exit code 2 when human-resolve items remain.

**SVG diagram renderer** (Tranche 6, `dsl-render`): bpmn-lite DSL → SVG. BFS topological layout, distinct shapes per node kind (circles for events, rounded rects for tasks, diamonds for gateways with ×/+/○ markers), pack provenance badges (amber square) on covered nodes. No external layout library.

**Operational hardening** (Tranche 7):
- `RuntimeMetrics`: 12 atomic counters wired throughout the engine; Prometheus text format output.
- `PostgresJourneyStore`: full `JourneyStore` impl against the v0.1 migration schema (`20260521_dsl_journey_runtime.sql`); feature-gated (`--features postgres`).
- `RetentionPolicy`: archive_after_days=90, cold_storage_after_years=7; default no-op on InMemoryStore.
- Perf smoke test: 100 concurrent in-memory instances, ~1ms total.

**Automated compliance pilot** (Tranche 8): 7 Rust tests covering 5 scenarios (KYC all-conditions, sanctions block, periodic refresh, jurisdiction routing, manual override), each verifying all 7 v&s audit boxes. Chrome MCP fixture at `tests/fixtures/sage_compliance_pilot.toml` for UI-level validation.

---

## New crates

| Crate | Purpose |
|---|---|
| `dsl-sage` | Pack matching, parameter extraction, confirmation, instantiation, Sage orchestrator |
| `dsl-migrate` | Camunda 8 XML → bpmn-lite DSL migration tool + CLI |
| `dsl-render` | bpmn-lite DSL → SVG diagram renderer |

---

## Test counts

| Crate | Tests |
|---|---|
| `dsl-sage` | 51+ (matching, extraction, confirmation, instantiation, orchestrator, compliance pilot) |
| `dsl-migrate` | 14 |
| `dsl-render` | 12 |
| `bpmn-runtime` | 4 (metrics) |
| `bpmn-test-harness` | 23+ (including perf smoke, metrics integration) |
| v0.1 regressions | 219 (all green) |

---

## Breaking changes

None. All v0.1 APIs are unchanged. New crates are independent additions.

---

## Known gaps (v0.3 candidates)

- BGE model embeddings for pack matching (currently BagOfWords baseline)
- Production LLM client wiring (currently `MockLlmClient`)
- Sage session persistence (currently in-memory only)
- React UI components for Sage authoring panel
- `for-each` type-checking for accessor fields (currently unchecked at v0.2)
- VerbConfig YAML retirement (Tranche 3.7, held from v0.1)

---

## Upgrade notes

No database migrations required for v0.2. The PostgresJourneyStore uses the existing v0.1 schema. Enable with `--features postgres` on `bpmn-runtime`.
