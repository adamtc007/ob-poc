# Gate A Review Replan

Status: approved replan proposal; production remediation still blocked.

Date: 2026-05-10

## Review Decision

Gate A is replanned into two checkpoints:

| Checkpoint | Purpose | Status |
| --- | --- | --- |
| Gate A1 - audit sign-off | Accept the static audit outputs for SemOS metadata, route hygiene, and crate boundaries as sufficient to plan remediation. | Accepted for planning. |
| Gate A2 - empirical baseline | Run the frozen fixtures against current Sage and repo-aware scoring before behavior-changing remediation starts. | Still required. |

This replan preserves the original intent: no production envelope schema, signing/build pipeline, or Sage runtime rewiring starts before baseline evidence and remediation approval exist.

## Accepted For Gate B Planning

The following Gate A artefacts are accepted as adequate to draft a unified remediation plan:

- `semos-metadata-inventory.md`
- `semos-gap-matrix.md`
- `macro-tier-classification.md`
- `workbook-plan-model-recommendation.md`
- `cross-dag-composition-recommendation.md`
- `semos-enrichment-work-plan.md`
- `utterance-route-path-inventory.md`
- `ghost-route-source-enumeration.md`
- `utterance-connection-point-map.md`
- `verb-dispatch-bypass-inventory.md`
- `routing-feature-flag-inventory.md`
- `legacy-test-rip-scope.md`
- `hygiene-rip-first-remediation-plan.md`
- `quarantine-register.md`
- `visibility-inventory.md`
- `workspace-dependency-graph-current.md`
- `workspace-dependency-graph-target.md`
- `crate-decomposition-recommendation.md`
- `super-crate-findings.md`
- `crate-rip-and-replace-migration-plan.md`
- `pub-lint-ci-enforcement-spec.md`

## Deferred But Still Blocking

These items are not waived; they are moved to explicit pre-remediation checks.

| Item | Required before | Decision |
| --- | --- | --- |
| Current Sage fixture scoring | Any behavior-changing route, resolver, macro, or envelope remediation | Must run or receive explicit reviewer waiver. |
| Repo-aware fixture scoring | Slice 1 acceptance threshold finalization | Must run or be replaced by a documented local baseline. |
| Byte-equality build evidence | Production envelope generation/signing | Deferred to the first deterministic build spike; not required for Gate B planning. |

## Gate B Planning Scope

Gate B planning may start immediately and should produce one unified remediation plan covering:

1. Slice 1 metadata gaps for `onboarding-request`, `cbu-maintenance`, and `product-service-taxonomy`.
2. Macro decisions: project, lift, retire, or quarantine.
3. Workbook-plan lift decisions for pack templates and workflow-plan fixtures.
4. Route decisions for `/api/session/:id/input`, `/api/session/:id/execute`, ACP prompt handlers, REPL fallback, and direct resolver calls.
5. Test decisions: refactor, delete, or keep as lower-level executor/regression coverage.
6. Crate-boundary migration sequence, starting with root `ob-poc` visibility and diagnostics/registry boundaries.
7. Quarantine exclusions and retirement dates.

## Non-Negotiable Decisions

These decisions are accepted for the replan:

- `POST /api/session/:id/input` is the only Slice 1 production HTTP utterance ingress candidate.
- `/api/session/:id/execute` is not a valid utterance baseline path.
- Research macros under `rust/config/macros/research` are excluded from Slice 1 production context projection.
- Pack templates are not macros until lifted into workbook-plan entities or registry-grade macro records.
- Direct REPL fallback cannot remain a production utterance bypass after Slice 1.
- The root `ob-poc` crate is the first public-surface audit target.

## Next Actions

1. Draft `gate-b-unified-remediation-plan.md`.
2. Add a fixture runner or manual scoring protocol for `baseline-fixtures-v1.md`.
3. Decide whether byte-equality evidence uses an existing command or waits for the deterministic build spike.
4. Review and approve quarantine exclusions before any production wiring begins.

Next action status:

- `gate-b-unified-remediation-plan.md` exists.
- `baseline-fixtures-v1.jsonl`, `run_current_sage_baseline.sh`, and `baseline-scoring-protocol.md` exist for Gate A2 execution.
