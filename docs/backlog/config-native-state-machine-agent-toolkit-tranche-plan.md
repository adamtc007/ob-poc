# Configuration-Native State Machine Agent Toolkit Tranche Plan

Status: approved for implementation

Source inputs:

- `configuration_native_state_machine_agent_toolkit_vision_scope_v0_7.md`
- `configuration_native_state_machine_agent_toolkit_implementation_plan_v0_2.md`

This plan adapts the source documents to the current `ob-poc` codebase. It is intentionally tranche based so each slice preserves existing REPL, runbook, SemOS, and React UI behavior while adding the ACP-oriented workflow.

## Codebase-Checked Assumptions

- SemOS crates already exist under `rust/crates/sem_os_*`.
- KYC state transitions already exist in SemOS seed configuration.
- Runbook compilation, immutable compiled runbook storage, execution events, session traces, and replay already exist.
- The React UI already has runbook plan review flows.
- ACP is not implemented in this repo.
- SemOS does not yet expose the required `simulate_transition()` capability.
- Existing `RunbookPlan` and `CompiledRunbook` types are useful foundations, but they are not yet the Execution Workbook contract described by the source documents.
- Existing runbook approval is not sufficient for v1.0 mutation because it is not bound to workbook hash, state snapshot, evidence references, and approval token semantics.
- Existing Journey Pack code is not the same abstraction as the source documents' Domain Pack. New work must avoid conflating those concepts.
- Legacy `public.derived_attributes` removal does not mean derived-value persistence is absent; the current schema has canonical derived-value/dependency persistence under the application schema.

## Regression Baseline

The baseline gate must remain green before and after ACP-facing work:

- `env RUSTC_WRAPPER= cargo test -p ob-poc --lib runbook`
- `env RUSTC_WRAPPER= cargo test -p ob-poc-types gated_envelope`
- `env RUSTC_WRAPPER= cargo test -p sem_os_core`
- `npm run test:run`

New ACP/workbook tranches must add focused tests before broadening behavior. UI-facing tranches must add component and browser regression coverage for existing chat, inspector, runbook review, and trace surfaces.

## Tranche 0: Baseline And Decisions

Goal: make the existing baseline reliable and record implementation choices.

Deliverables:

- Fix brittle SemOS fixture-count assertions so adding valid seed maps does not break the gate.
- Confirm ACP transport/library choice in a spike before production adapter work.
- Use `kyc-case.update-status` as the first dry-run and restricted-mutation candidate on an isolated fixture case.
- Keep V&S Domain Pack separate from existing Journey Pack code.
- Preserve the existing runbook execution gate as the only path to mutation.

Acceptance:

- Baseline regression commands pass.
- No product behavior changes.

## Tranche 1: Domain Pack V0 And Discovery Contract

Goal: add a typed Domain Pack layer and the minimum discovery contract.

Deliverables:

- New Domain Pack manifest type with allowed transitions, probe declarations, materiality metadata, classification policy, and compatibility tier.
- Validator with stable diagnostics.
- SemOS-backed discovery interface aligned to the source documents' minimum discovery schema.
- First native compiled ob-poc Domain Pack.

Tests:

- Valid ob-poc pack passes.
- Malformed packs fail with stable diagnostics.
- Existing Journey Pack behavior remains unchanged.
- Undeclared probes are refused.

## Tranche 2: SemOS Simulation Foundation

Goal: implement non-mutating state-transition simulation.

Deliverables:

- `simulate_transition()` for the selected KYC transition.
- No persistent first-class state writes during simulation.
- Derived-value behavior included in the simulated result.
- Stale-state and configuration-version comparison primitives.

Tests:

- Simulation is deterministic.
- Simulation produces no committed database writes.
- Simulated result matches real execution on an isolated fixture.
- Illegal transitions and undeclared probes are refused.
- Transaction boundaries are tested so transitional pool access does not leak writes.

## Tranche 3: Execution Workbook On Existing Runbook Infrastructure

Goal: introduce the workbook contract without replacing working runbook machinery.

Deliverables:

- Execution Workbook type or workbook-compatible extension around existing runbook types.
- Immutable workbook hash.
- Binding to configuration version, state snapshot, evidence references, LLM trace references, actor/session, and supersession chain.
- Workbook validation library.

Tests:

- Hash reproducibility.
- Canonical serialization property tests.
- Supersession-chain reconstruction.
- Stale workbook state handling.
- Free-text transition references are rejected.

## Tranche 4: DSL Coder Runtime And REPL Adapter Dry-Run

Goal: add the DSL Coder validation boundary in dry-run mode.

Deliverables:

- Common validation runtime.
- REPL adapter that validates workbook, calls SemOS simulation, and returns semantic diff.
- Central refusal codes.
- Dry-run only.

Tests:

- Validation contract coverage.
- Invalid workbook refused before execution.
- Sage cannot mutate state or bypass DSL Coder.
- Unknown transition references are rejected.
- Existing runbook execution tests still pass.

## Tranche 5: ACP Adapter And UI Preservation

Goal: introduce ACP as an adapter and preserve current UI behavior.

Deliverables:

- Zed ACP session lifecycle adapter.
- Sage context assembly from SemOS discovery.
- Workbook and dry-run trace rendering through existing React patterns.

Tests:

- Vitest component tests for runbook/workbook review.
- API client mock tests for compile, approve, dry-run, status, and trace calls.
- Browser regression tests for chat, inspector, runbook review, trace, and ACP dry-run flow.
- Sage runtime has no mutation credentials.

## Tranche 6: Audit, LLM Trace, And Context Policy

Goal: make LLM and context use auditable.

Deliverables:

- Wrapper around existing LLM client usage.
- Prompt/response hashes, model/provider, latency, and token counts where available.
- Context classification and redaction before prompt assembly.
- O3 source-attribution metric instrumentation.

Tests:

- No raw LLM calls outside the wrapper.
- Sensitive fields are absent from prompt payloads.
- Trace chain links session, workbook, validation, and dry-run.
- O3 attribution is recorded.

## Tranche 7: MVP-DryRun Gate

Goal: end-to-end non-mutating vertical slice.

Acceptance:

- ACP to Sage to SemOS discovery to workbook to DSL Coder validation to SemOS dry-run to semantic diff works.
- No mutation is possible from the MVP path.
- Existing UI and system behavior remains green under the regression gate.

## Tranche 8: Restricted Mutation V1.0

Goal: enable one low-risk HITL-approved transition.

Deliverables:

- Approval token bound to workbook hash, state snapshot, evidence references, actor, expiry, and approval text.
- Mutation via the existing runbook and SemOS gate.
- Intended vs predicted vs actual semantic diff.
- Approval invalidation on state drift.

Tests:

- Unapproved execution refused.
- Token replay refused.
- Drifted state invalidates approval.
- Non-enabled transitions refused.
- Trace chain reconstructs the mutation.

## Tranche 9: Deferred Reuse Proof

Goal: prove reuse with a second dry-run-only pack.

Acceptance:

- Second pack loads and validates.
- Discovery and workbook generation work without common runtime changes.
- No mutation support required.
