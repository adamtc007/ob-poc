# Sage Eval Harness

This workspace contains the v0.1 Sage Eval Harness implementation.

## Crates

- `ob-poc-eval`: CLI binary for harness commands.
- `ob-poc-eval-schema`: shared serde schemas for cases, runs, and scorecards.
- `ob-poc-eval-runner`: replay runner skeleton.
- `ob-poc-eval-scorer`: gates-first scorer skeleton.
- `ob-poc-eval-fixtures`: test cases and seed-bundle fixture helpers.

## First Slice

The initial implementation lands the crate skeleton, the `EvalCase` schema,
and one YAML-authored test case under `ob-poc-eval-fixtures/test_cases/`.

Validate the fixture with:

```bash
cargo run -p ob-poc-eval -- case validate ob-poc-eval-fixtures/test_cases/cbu_promote_active.yaml
```

Run the workspace checks with:

```bash
cargo check
cargo test
```
