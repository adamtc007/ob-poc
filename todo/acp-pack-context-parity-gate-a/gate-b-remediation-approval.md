# Gate B Remediation Approval Packet

Status: approved in-session; route hygiene, targeted Slice 1 routing, template trace surfacing, and draft-expected DSL draft surfacing implemented and measured.

Purpose: turn Gate A evidence into a single implementation authorization. Approving this packet permits behavior-changing remediation within the scope below.

## Evidence Accepted

- `baseline-results-current-sage.md`
- `baseline-results-repo-aware.md`
- `baseline-gap-analysis.md`
- `semos-gap-matrix.md`
- `macro-tier-classification.md`
- `utterance-route-path-inventory.md`
- `verb-dispatch-bypass-inventory.md`
- `quarantine-register.md`
- `visibility-inventory.md`
- `crate-decomposition-recommendation.md`

## Approved Slice 1 Scope

Packs:

- `onboarding-request`
- `cbu-maintenance`
- `product-service-taxonomy`

Primary targets:

- `pack_hit >= 83.3%`
- `verb_hit >= 88.4%`
- draft-expected `first_pass_valid_dsl_draft >= 70.0%`
- invented verbs/macros remain `0`
- prose-only failures remain `0`
- refusal quality `2` on all 10 refusal fixtures

## Implementation Decisions

| Area | Decision |
| --- | --- |
| Utterance ingress | `POST /api/session/:id/input` is the only Slice 1 production HTTP utterance ingress. |
| Raw DSL execute | `/api/session/:id/execute` stays outside utterance routing and must refuse/avoid normal utterance baseline paths. |
| Ghost-route bait | Raw DSL, legacy execute, `direct.dsl`, legacy pipeline, and old chat-route bait must produce structured refusals. |
| Pack trace | Every selected Slice 1 route must record `acp_trace.pack_id` when a pack was selected. |
| Macro projection | Project registry-grade Slice 1 macros only; research macros remain quarantined. |
| Pack templates | Lift or trace pack templates as workbook-plan/template selections; do not pretend they are registry macros. |
| First-pass drafts | Draft-expected fixtures must emit parseable, pack-legal DSL drafts where required bindings are present. |
| Crate boundary | Root `ob-poc` public surface is the first visibility target; broad crate migration waits until route hygiene stabilizes. |

## Authorized First Implementation Batch

Approved first implementation batch:

1. [x] Add explicit structured refusal classification for ghost-route bait utterances.
2. [x] Ensure `acp_trace.pack_id` is populated for Slice 1 selected verbs where the route has pack context.
3. [x] Add regression coverage for the refusal fixtures and pack-trace expectation.
4. [x] Re-run `cargo check` after the edit batch and re-run the 36-fixture current-Sage baseline after the batch.

Latest measurement:

- Current-Sage run: `baseline-runs/current-sage-20260510T123212Z`
- `pack_hit`: `31/36`, up from `16/36`
- `verb_hit`: `31/31`, up from `19/31`
- refusal quality `2`: `10/10`, up from `0/10`
- `first_pass_valid_dsl_draft`: `8/36`, up from `0/36`
- draft-expected `first_pass_valid_dsl_draft`: `5/5`, up from `0/5`
- remaining non-null verb misses: `0`

## Out Of Scope Until Later Gates

- Production ACP envelope schema.
- Signing or deterministic envelope build pipeline.
- Broad crate decomposition.
- Research macro projection.
- Runtime state-instance projection.

## Approval

Reviewer decision required:

```text
Approve Gate B remediation: yes
Reviewer: user, in session
Date: 2026-05-10
Notes: Continue into remediation; stop after measured batches for replan as needed.
```
