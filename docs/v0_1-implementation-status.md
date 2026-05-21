# ob-poc v0.1 Implementation Status

Generated: 2026-05-21

## Tranches completed

| Tranche | Description | Tests |
|---|---|---|
| 0 | Design document consolidation | `docs/design/v0.1/` |
| 2 | Unified DSL atom model + parser | 34 tests |
| 4 | bpmn-lite compiler frontend | 12 tests |
| 5 | Resolution pass + 12 decision packs | 8 tests |
| 6 | Journey-persisted runtime core | 10 tests |
| 7 | Multi-token, parallel-join, merge protocol | 13 tests |
| 8 | Integration validation (Examples 11 + 12, pack Sage stub, hardening) | 20 tests |
| 9 | Decision pack catalogue (parallel) | 4 tests |

## Not yet completed

| Tranche | Description | Prerequisite for |
|---|---|---|
| 1 | Pre-refactor SemOS regression baseline | Tranche 3 (SemOS reshape) |
| 3 | SemOS verb reshape (~1,098 verbs) | Highest-risk tranche |
| 10 | Documentation + handoff | Final |

## Tranche 8 detail

Tranche 8 adds integration validation across all prior tranches:

- **Example 11 (complex KYC onboarding):** four tests cover the simplified
  EXAMPLE_11 DSL (exclusive gateway with 3 jurisdiction branches, parallel
  fork/join with 2 branches, merge on 3 fields, error boundary on sign-off,
  non-interrupting SLA timer on intake-form). Tests verify: compilation,
  UK path execution, standard (default) path execution, and a focused
  parallel-join sub-test with manual task completion.

- **Example 12 (pack-authored with provenance):** `instantiate_pack()` stub
  in `bpmn-test-harness/src/lib.rs` expands the `conjunctive-gate` template
  given a parameter map. Two tests verify: DSL validates without errors,
  provenance summary records the correct `pack_id`, and the runtime completes
  on both the enhanced and default paths.

- **Phase 8.5 hardening:** `token_excess_does_not_panic` verifies no panic
  on a parallel fork/join with both branches auto-completing.
  `perf_100_instances_linear` (marked `#[ignore]`) spawns 100 concurrent
  linear-sequence instances.

## Test files

| File | Test count |
|---|---|
| `crates/bpmn-test-harness/tests/runtime_scenarios.rs` | 20 passing, 1 ignored |
| `crates/dsl-bpmn-frontend/tests/worked_examples.rs` | 12 compilation tests |
| `crates/dsl-resolution/tests/` | resolution + pack tests |

## Known limitations (v0.1 GAPs)

1. **Variable-arity template combinator** — packs 3, 4, 5, 6, 7, 8, 10 have
   fixed-arity templates (N=2 or N=3). `for-each` combinator deferred to v0.2.

2. **PostgresJourneyStore** — only `InMemoryJourneyStore` implemented.
   Postgres-backed runtime deferred (migration is ready at
   `migrations/20260521_dsl_journey_runtime.sql`).

3. **SemOS integration** — Tranche 3 (SemOS reshape) not started; existing
   SemOS pipeline unchanged.

4. **Production Sage matching** — `instantiate_pack()` in `bpmn-test-harness`
   is a stub; real confidence-ranked pack matching is v0.2.

5. **Timer service** — `pending_timer` table exists but no timer-firing
   worker; timers leave tokens waiting indefinitely.

6. **Conditional events, ad-hoc subprocess** — explicitly rejected or deferred
   per design.

7. **Example 11 sign-off loop** — the production EXAMPLE_11 DSL has a
   loopback edge (`sign-off → sign-off`) and a `:loop` marker with
   `max-count 3`. The loopback was removed from the test DSL to avoid
   unbounded recursion in `advance_token`. Full loop semantics require a
   loop-counter service in the runtime, deferred to v0.2.

8. **Inclusive gateway dynamic join count** — EXAMPLE_11's `main-join`
   has `expects [main-fork]` but only 2 of the 3 KYC subprocesses connect
   to `main-fork` (uk-kyc / eu-kyc / standard-kyc → main-fork; none of them
   are the parallel-fork). The parallel `main-fork` emits exactly 2 tokens
   (deal-task + im-task), so the join correctly fires after 2 arrivals. The
   third merge field (`kyc-outcome`) is written to instance data by whichever
   KYC subprocess ran before the fork.
