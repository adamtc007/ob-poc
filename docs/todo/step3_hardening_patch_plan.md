# Step 3 Hardening Patch Plan

## Scope
Small tactical hardening pass on the current three-step pipeline while StateGraph work continues.

## Fixes
1. Action-class hard gate
- Enforce create/update/delete/read compatibility before scoring.
- Add screening-run/check action class so list/read verbs cannot win for operational screening utterances.

2. Create-without-existing-entity bypass
- Fresh create/open/set-up utterances must not require Step 1 entity grounding when they are creating a new object.
- Treat named payloads in these utterances as arguments, not existing scope.

3. Domain-keyword hard prefilter
- Narrow candidate verbs by explicit domain keywords before semantic scoring.
- Initial domains: screening, kyc/case, fund/struct, document.

## Gate
- `cargo check -p ob-poc`
- Focused unit tests for each hardening rule
- 176-row SemTaxonomy harness rerun
