# Gate A Status

Status: Gate A evidence complete for planning; Gate B remediation approval still required

Approved source plans:

- `todo/acp-pack-context-parity-plan-v0.5.md`
- `todo/acp-pack-context-parity-execution-plan-v0.1.md`

Reproducible evidence:

- `todo/acp-pack-context-parity-gate-a/run_audit_inventory.sh`
- `todo/acp-pack-context-parity-gate-a/generated-inventory-current.md`

Current repo state caveat:

- The worktree already contains unrelated Rust/config changes and untracked Rust files. Gate A artifacts must not revert or rewrite those changes.

Gate interpretation:

- Production envelope schema, deterministic signing/build pipeline, and Sage runtime wiring are still blocked by the approved execution plan until W1-W4 are complete and a unified remediation plan is accepted.
- Audit and fixture artifacts under this directory are permitted as throwaway/planning artifacts.

Initial findings:

- Config surface is large enough that manual review is not sufficient: 154 verb YAML files, 12 journey packs, 142 SemOS seed YAML files, 39 stategraph/state-machine YAML files, 29 macro schema/registry YAML files, and 7 workflow YAML files.
- Slice 1 target pack allowed-verb counts are `onboarding-request=17`, `cbu-maintenance=43`, and `product-service-taxonomy=32`.
- `rust/src` has a very large public surface in the current snapshot; the generated inventory counted 17,521 `pub`-prefixed lines under `rust/src`.
- Existing route comments identify `/api/session/:id/input` as the unified utterance endpoint, but the code still contains legacy/fallback terminology and route surfaces that must be classified before envelope wiring.

Gate A deliverable status:

| Workstream | Status | Notes |
| --- | --- | --- |
| W1 baseline | Complete for Gate A2 | Fixtures, measurement schema, current-Sage capture/scoring, repo-aware manual scoring, and gap analysis are present. |
| W2 SemOS metadata audit | Draft complete | Inventory, gap matrix, macro tiers, workbook/cross-DAG recommendations, determinism caveat, and enrichment plan are present. |
| W3 code hygiene audit | Draft complete | Route inventory, ghost-route enumeration, connection map, bypass inventory, feature-flag inventory, test rip scope, remediation plan, and quarantine register are present. |
| W4 crate boundary audit | Draft complete | Visibility inventory, current/target graph, decomposition recommendation, super-crate findings, migration plan, and pub lint spec are present. |

Stop point for replan:

- Gate A1 static audit is accepted for Gate B planning by `gate-a-review-replan.md`.
- Gate A2 current-Sage and repo-aware scoring is complete.
- Byte-equality rebuild evidence is deferred to the first deterministic build spike, but remains blocking for production envelope generation/signing.
- No production remediation or production envelope work has been started in this task directory.

Replan outputs:

- `todo/acp-pack-context-parity-gate-a/gate-a-review-replan.md`
- `todo/acp-pack-context-parity-gate-a/gate-b-unified-remediation-plan.md`
