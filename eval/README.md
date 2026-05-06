# Sage Eval Harness

This workspace contains the v0.1 Sage Eval Harness implementation.

## Crates

- `ob-poc-eval`: CLI binary for harness commands.
- `ob-poc-eval-schema`: shared serde schemas for cases, runs, and scorecards.
- `ob-poc-eval-runner`: replay runner skeleton.
- `ob-poc-eval-scorer`: gates-first scorer skeleton.
- `ob-poc-eval-fixtures`: test cases and seed-bundle fixture helpers.

## Runtime Boundary

The fixture library owns data lifecycle behavior. The runner and developer
tooling both call this library directly.

- `ob-poc-eval-fixtures`: seed-bundle parsing, fixture schema naming,
  cleanout primitives, probe-stub paths, and later spin-up/tear-down.
- `rust/xtask`: human and CI surface for database, bundle, fixture, and probe
  operations.
- `ob-poc-eval-runner`: runtime replay engine; it must not shell out to xtask.

This keeps manual fixture preparation and eval runtime preparation on the same
code path.

## Phase 2 Split

Phase 2 is split into two implementation slices:

- Phase 2a: `ob-poc-eval-fixtures` library behavior for seed bundles,
  ephemeral fixture lifecycle, schema template management, probe stubs, and
  cleanout.
- Phase 2b: `cargo x` developer surface in `rust/xtask` wrapping the Phase 2a
  library for bundle authoring, fixture lifecycle, probe stubs, and DB cleanout.

The repository already uses `cargo x ...` from `rust/`, via
`rust/.cargo/config.toml`, so eval tooling follows that convention instead of
adding a second root-level xtask.

## Xtask Surface

Registered command groups:

```bash
cd rust/

cargo x db cleanout
cargo x db migrate
cargo x db reset
cargo x db apply-bundle <id>
cargo x db verify-snapshot <expected_state_snapshot_id>
cargo x db snapshot <name>
cargo x db restore <name>

cargo x bundle new <id>
cargo x bundle freeze <id>
cargo x bundle list
cargo x bundle inspect <id>

cargo x fixture spin-up --bundle <id> --name <fixture_name>
cargo x fixture tear-down --name <fixture_name>
cargo x fixture list
cargo x fixture cleanout

cargo x probe record --probe lei_lookup -- <args>
cargo x probe stub-list
cargo x probe stub-validate <bundle_id>
```

Only safe read/list and cleanout primitives are wired in this slice. Commands
that require the Phase 2a fixture engine are intentionally registered but return
a typed "Phase 2b pending" error.

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
