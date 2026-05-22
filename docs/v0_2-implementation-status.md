# ob-poc v0.2 Implementation Status

**Tag**: `impl/v0.2` — 2026-05-22

## All tranches complete

| Tranche | Status | Key deliverable |
|---|---|---|
| v0.1 (Tranches 0–10) | Complete | Unified DSL, compiler, runtime, 12 packs, 219 tests |
| v0.2 T0 | Complete | `for-each` combinator, all 12 packs variable-arity |
| v0.2 T1 | Complete | Sage pack matching, 96% top-1 (BagOfWords) |
| v0.2 T2 | Complete | Parameter extraction + confirmation state machine |
| v0.2 T3 | Complete | DSL instantiation + provenance emission |
| v0.2 T4 | Complete | End-to-end Sage orchestrator + REPL + audit log |
| v0.2 T5 | Complete | Camunda 8 migration tool (`dsl-migrate`) |
| v0.2 T6 | Complete | SVG diagram renderer (`dsl-render`) |
| v0.2 T7 | Complete | Metrics, PostgresJourneyStore, retention |
| v0.2 T8 | Complete | Automated compliance pilot harness + release notes |

## Active limitations

1. BGE embeddings not yet wired to Sage pack matcher (BagOfWords baseline)
2. LLM client is MockLlmClient (parameter extraction is heuristic-only)
3. Sage sessions are in-memory (no cross-restart session recovery)
4. React UI has no Sage authoring panel yet
5. PostgresJourneyStore has no `.sqlx/` offline cache (runtime queries only)
