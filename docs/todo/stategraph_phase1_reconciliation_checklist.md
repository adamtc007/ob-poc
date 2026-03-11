# StateGraph Phase 1 Reconciliation Checklist

Purpose: make the metadata triangle internally valid before asking the
three-step pipeline and StateGraph walker to perform.

Execution rule:
- Freeze algorithm changes.
- Apply only metadata/data-contract cleanup.
- Run `cargo check -p ob-poc` after every file edit.

## Bucket A: Impossible Harness Expectations

Status: in progress

- [ ] Normalize `case.open` to the current canonical verb `kyc.open-case`
  - Source of truth:
    - `rust/config/verbs/kyc/kyc.yaml`
  - Affected surface:
    - `rust/tests/fixtures/intent_test_utterances.toml`

- [ ] Normalize `screening.pep-check` to `screening.pep`
  - Source of truth:
    - `rust/config/verbs/screening.yaml`
  - Affected surface:
    - `rust/tests/fixtures/intent_test_utterances.toml`

- [ ] Normalize `screening.sanctions-check` to `screening.sanctions`
  - Source of truth:
    - `rust/config/verbs/screening.yaml`
  - Affected surface:
    - `rust/tests/fixtures/intent_test_utterances.toml`

- [ ] Normalize `screening.media-check` to `screening.adverse-media`
  - Source of truth:
    - `rust/config/verbs/screening.yaml`
  - Affected surface:
    - `rust/tests/fixtures/intent_test_utterances.toml`

- [ ] Resolve the `screening.full` expectation
  - Decision required in codebase:
    - register a canonical full-screening verb
    - or normalize fixture expectations to the current supported surface

- [ ] Resolve the `struct.*` expectation family
  - Current finding:
    - `struct.*` expected verbs exist in the corpus but are not present in the
      current registry
  - Decision required:
    - restore/register them
    - or normalize the fixture to the current canonical surface

## Bucket B: Graph Canonicality

Status: review complete, fix only if drift reappears

- [x] Confirm StateGraph edge verb IDs exist in the registry
  - Current authored graphs use canonical live IDs:
    - `deal.read-record`
    - `kyc.open-case` is not yet present in graph files because no KYC graph has
      been authored yet
  - Files reviewed:
    - `rust/config/stategraphs/cbu.yaml`
    - `rust/config/stategraphs/deal.yaml`
    - `rust/config/stategraphs/document.yaml`
    - `rust/config/stategraphs/entity.yaml`
    - `rust/config/stategraphs/fund.yaml`
    - `rust/config/stategraphs/screening.yaml`
    - `rust/config/stategraphs/ubo.yaml`

- [ ] Add a KYC graph only after canonical verb IDs and phase names are verified

## Bucket C: Phase / Signal Canonicality

Status: in progress

- [ ] Verify deal phase names in graph/state derivation match live DB enums
  - Current DB-facing code uses `deal_status`
  - Files:
    - `rust/src/domain_ops/discovery_ops.rs`
    - future KYC/deal graph YAMLs

- [ ] Verify screening signals are granular enough for graph and Step 2 narrowing
  - Current code now exposes split screening counts, but the checklist remains
    until coverage proves them sufficient

- [ ] Verify document signals are granular enough for graph and Step 2 narrowing

- [ ] Verify `has_incomplete_ubo` uses real verification state, not relationship
  presence

## Bucket D: Invocation Phrase Coverage

Status: in progress

- [ ] CBU domain coverage audit
- [ ] Screening domain coverage audit
- [ ] UBO domain coverage audit
- [ ] Document domain coverage audit
- [ ] Deal domain coverage audit
- [ ] Fund domain coverage audit
- [ ] Entity domain coverage audit
- [ ] KYC domain coverage audit

Per-verb target:
- 3 to 8 distinct invocation phrases
- at least one imperative phrase
- at least one status/read phrase for read verbs
- at least one action/initiation phrase for write verbs

## Bucket E: Harness Contract Hygiene

Status: in progress

- [ ] Remove all expected verbs that do not exist in the current registry
- [ ] Remove all expected route targets that do not exist in the current registry
- [ ] Keep notes explaining any normalization where the old name was a legacy or
  macro-level alias

## Execution Order

1. Fix impossible harness expectations
2. Re-run `cargo check -p ob-poc`
3. Re-run the 176-row harness
4. Only then continue with remaining Phase 1 signal and invocation cleanup
