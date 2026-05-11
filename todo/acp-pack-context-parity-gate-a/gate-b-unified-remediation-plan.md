# Gate B Unified Remediation Plan

Status: planning draft ready for remediation approval; production implementation still requires explicit Gate B approval.

## Slice 1 Scope

Packs:

- `onboarding-request`
- `cbu-maintenance`
- `product-service-taxonomy`

Primary invariant:

```text
utterance ingress -> verified pack/context metadata -> pack-scoped route -> draft/refusal/pending question
```

No production utterance path may dispatch verbs or generate executable DSL outside that route.

## Work Order

### B1. Baseline Evidence

Before behavior-changing code edits:

- Use `baseline-results-current-sage.md`, `baseline-results-repo-aware.md`, and `baseline-gap-analysis.md` as the frozen baseline.
- Do not alter the acceptance threshold after remediation starts without peer review.

### B2. Metadata Remediation

Implement first for Slice 1 only:

- Complete missing verb argument contracts that affect Slice 1.
- Add per-argument binding rules for required pending-question fixtures.
- Add entity-grain read/write effects for allowed and forbidden Slice 1 verbs.
- Add refusal/HITL/dry-run metadata for mutating or policy-gated verbs.
- Add diagnostic codes for ambiguous pack, forbidden verb, missing binding, unsupported macro tier, and legacy route bait.

### B3. Macro and Workbook Decisions

Project:

- Registry-grade Slice 1 macros only after slot, precondition, refusal, dry-run, HITL, and ordered-step checks pass.

Lift:

- `create-cbu`, `add-entity-and-role`, `standard-onboarding-handoff`, and Slice 1 taxonomy templates into workbook-plan entities where they affect route or pending-question behavior.

Quarantine:

- `rust/config/macros/research/*.yaml`
- Pack templates not yet lifted into workbook-plan entities.

### B4. Route Hygiene

Approved target:

- `POST /api/session/:id/input` remains the only Slice 1 production HTTP utterance ingress.

Required route decisions:

| Surface | Gate B decision |
| --- | --- |
| `/api/session/:id/execute` | Keep out of utterance route; remove, admin-scope, or quarantine. |
| `try_route_through_repl` | Replace with envelope-gated route or quarantine before Slice 1. |
| ACP protocol prompt handlers | Route through same pack/context path or mark out of Slice 1. |
| Direct ACP DAG semantic resolver calls | Use as scorer only after envelope-compatible metadata boundary exists, or quarantine. |

### B5. Test and Fixture Cleanup

Keep/refactor:

- Bypass regression tests.
- Raw DSL bait refusal tests.
- Old chat route bait refusal tests.
- Direct DSL no-bypass tests.

Delete or rewrite:

- Tests asserting legacy fallback as expected production behavior.
- Comments/examples that describe retired utterance routing.

Leave alone:

- Lower-level executor tests that bypass utterance routing intentionally.
- Domain fallback examples unrelated to utterance routing.

### B6. Crate Boundary Migration

Sequence:

1. Classify root `ob-poc` public exports used by workspace crates.
2. Reduce internal exports to `pub(crate)` or narrower in batches.
3. Extract or narrow shared diagnostics.
4. Create/read through a registry projection boundary.
5. Move utterance route selection behind a small API that cannot depend on execution/database crates.
6. Add `pub` lint enforcement after the first migrated boundary.

## Stop Conditions

Stop and replan if:

- The baseline shows a fixture category not covered by current metadata recommendations.
- ACP protocol prompt handling cannot be aligned with the unified route.
- Crate migration requires broad production rewrites before Slice 1 route hygiene.
- A quarantined route remains reachable from the UI or public server path.
