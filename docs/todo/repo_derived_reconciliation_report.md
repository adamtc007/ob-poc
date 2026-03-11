# Repo-Derived Reconciliation Report

Date: 2026-03-11

Scope:
- current checked-in registry verbs under `rust/config/verbs/`
- current checked-in graph YAMLs under `rust/config/stategraphs/`
- current utterance fixture expectations under `rust/tests/fixtures/intent_test_utterances.toml`
- current safe-path signal surface in `rust/src/domain_ops/discovery_ops.rs`

Purpose:
- reconstruct the reconciliation state from the codebase alone
- separate repo-provable corrections from corrections that still require the external authoritative reconciliation artifacts

## Executive finding

From the current codebase alone:
- the checked-in StateGraph YAMLs already use current canonical verb IDs
- the strongest provable mismatches are in the utterance fixture expectations, not in the current graph files
- the remaining high-risk corrections still blocked on the external authoritative artifacts are:
  - `struct.*` expected verb family
  - `screening.full`
  - any external graph edge rewrite table
  - any explicit phase enum replacement table

That means the current repo-derived safe corrections are:
1. fixture alias normalization for stale-but-obvious verb names
2. signal enrichment in `entity-context`
3. invocation phrase enrichment in current canonical verb YAMLs

## Current graph status

Checked-in graph files reviewed:
- `rust/config/stategraphs/cbu.yaml`
- `rust/config/stategraphs/deal.yaml`
- `rust/config/stategraphs/document.yaml`
- `rust/config/stategraphs/entity.yaml`
- `rust/config/stategraphs/fund.yaml`
- `rust/config/stategraphs/screening.yaml`
- `rust/config/stategraphs/ubo.yaml`

Repo-derived conclusion:
- no ghost verb IDs were found in these checked-in graph files
- graph verb references are already aligned to current canonical surfaces such as:
  - `deal.read-record`
  - `screening.sanctions`
  - `screening.pep`
  - `screening.adverse-media`
  - `entity.create`
  - `ubo.list-owners`
  - `document.missing-for-entity`

This does not invalidate the external reconciliation report. It means:
- either those 28 wrong IDs were in generated graph candidates not yet checked in
- or they were already corrected in the checked-in graph subset

## Repo-provable fixture mismatches

Applied already:
- `case.open` -> `kyc.open-case`
- `screening.pep-check` -> `screening.pep`
- `screening.sanctions-check` -> `screening.sanctions`
- `screening.media-check` -> `screening.adverse-media`

Still present and not safely auto-remapped from repo truth alone:
- `screening.full`
- `struct.lux.ucits.sicav`
- `struct.ie.ucits.icav`
- `struct.uk.authorised.oeic`
- `struct.us.40act.open-end`
- `struct.lux.pe.scsp`
- `struct.lux.aif.raif`
- `struct.hedge.cross-border`
- `struct.pe.cross-border`

Why these remain blocked:
- the current canonical replacement is not mechanically provable from the checked-in registry alone
- they may require:
  - restoration of missing registry verbs
  - corpus normalization to `fund.*`
  - an authoritative replacement table from the external reconciliation artifacts

## Repo-provable metadata cleanup completed

### Signal enrichment in `entity-context`

Completed in `rust/src/domain_ops/discovery_ops.rs`:
- granular client-group document counts
  - `pending_document_count`
  - `verified_document_count`
  - `rejected_document_count`
  - `catalogued_document_count`
- deal KYC case counts
  - `active_kyc_case_count`
  - `blocked_kyc_case_count`
  - `review_kyc_case_count`
- deal screening counts
  - `screening_active_count`
  - `screening_sanctions_count`
  - `screening_pep_count`
  - `screening_adverse_media_count`
- deal onboarding request counts
  - `active_onboarding_request_count`
  - `completed_onboarding_request_count`
  - `total_onboarding_request_count`
- fund-linked signals from `entity_funds`
  - `fund_entity_count`
  - `nested_fund_count`
  - `feeder_or_master_link_count`
- generic entity fund evidence
  - `is_fund_entity`
  - `is_nested_fund`
  - `has_master_feeder_link`

### Invocation phrase enrichment

Completed in canonical verb YAMLs:
- `cbu.yaml`
- `screening.yaml`
- `ubo.yaml`
- `document.yaml`
- `deal.yaml`
- `fund.yaml`
- `entity.yaml`

## What still requires the external authoritative artifacts

Needed inputs:
1. explicit replacement table for `struct.*`
2. explicit replacement table for `screening.full`
3. external graph correction table, if the generated graph YAMLs differ from the checked-in graph YAMLs
4. explicit phase enum correction list, if any graph nodes or walker logic must be renamed to match real DB enums

## Safe conclusion

From the repo alone, the next safe actions are exhausted.

Do not guess at:
- graph rewrites
- `struct.*` remaps
- `screening.full` remap
- external graph edge correction tables

Wait for the authoritative external artifacts before performing those changes.
