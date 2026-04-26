# Incident: pre-existing sem_os_core test failures

- **Tier:** 0 (logged, not blocking)
- **Detected:** 2026-04-18 during Phase 1 build verification
- **Detected by:** `cargo test -p sem_os_core`
- **Phase:** 1
- **Drift dimension(s):** D6 (baseline) — the v0.3 §4.1 "1,194 passing unit tests" claim was already stale before Phase 1 started.

## Description

4 `sem_os_core` tests fail on the untouched Phase 0 baseline commit (`6bbf134e`). Confirmed via `git stash` + local rebuild: the failures reproduce before any Phase 1 code changes were applied. They are NOT a regression introduced by this refactor.

Failing tests:

- `enforce::tests::test_allow_internal_for_internal_clearance`
- `enforce::tests::test_allow_restricted_for_restricted_clearance`
- `enforce::tests::test_filter_by_abac_splits_correctly`
- `gates::tests::test_derivation_evidence_grade_operational_allowed_fails`

Error shape: `"Expected Allow, got Discriminant(2)"` — suggests an enum variant count or ordering assumption became inconsistent with the actual `AbacDecision` layout. Unrelated to the execution module Phase 1 touched.

## Remediation plan

Not a Phase 1 responsibility. Logged so the refactor health baseline stops claiming "1194 passing" without caveat. Phase 0h baseline snapshot already noted `workspace_passing: null` — this incident confirms why the direct figure was deferred to the Phase 0h xtask.

These failures are on the refactor's **entry path**, not on the **critical path** of gated changes. Phase 1 proceeds; the fixes belong to whoever owns `sem_os_core::abac::enforce` and `sem_os_core::gates`.

## Closed

Not closed — the 4 tests remain red. Incident retained for audit.

## Lessons learned

- The v0.3 §4.1 test-count claim was stale even before Phase 0 started. The B3 baseline correction ("measured fresh at Phase 0 start") is the correct framing.
- Phase 0h must include a "pre-existing failures allowlist" in the baseline snapshot so regression-drift detection is grounded on the actual starting state, not an idealised one. Otherwise every future phase keeps flagging these as regressions.
- Adding a separate deliverable: **0h step 2.5** — capture the current failing-test allowlist as `docs/refactor-health/baselines/failing-tests-allowlist.yaml`. Any new failure outside this allowlist is a real regression.
