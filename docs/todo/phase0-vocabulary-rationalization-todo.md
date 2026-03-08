# Phase 0: Vocabulary Rationalization — Execution TODO

**Source of truth:** [phase0-vocabulary-decisions-resolved.md](/Users/adamtc007/Developer/ob-poc/docs/todo/phase0-vocabulary-decisions-resolved.md)

**Objective:** reduce exact phrase collisions, collapse redundant domains, merge type-parameterized verb families, and bring the registry into a shape where Sage+Coder can compete with the existing pipeline.

**Execution rule:** follow the owner decisions exactly. If implementation details in code/config conflict with those decisions, treat that as a blocker to resolve explicitly, not a reason to improvise.

**Verification gate after each implementation batch:** `RUSTC_WRAPPER= cargo check -p ob-poc`

---

## Phase 0A: Preparation

### 0A.1 Build change inventory
- Enumerate all verb config files touched by the owner decisions:
  - `rust/config/verbs/kyc/case-screening.yaml`
  - `rust/config/verbs/kyc/doc-request.yaml`
  - `rust/config/verbs/matrix-overlay.yaml`
  - `rust/config/verbs/lifecycle.yaml`
  - `rust/config/verbs/service-resource.yaml`
  - `rust/config/verbs/registry/fund-vehicle.yaml`
  - `rust/config/verbs/fund.yaml`
  - `rust/config/verbs/entity.yaml`
  - `rust/config/verbs/ubo.yaml`
  - `rust/config/verbs/sla.yaml`
  - `rust/config/verbs/trading-profile.yaml`
  - `rust/config/verbs/refdata/*.yaml`
- Enumerate dependent references:
  - `rust/config/noun_index.yaml`
  - `rust/config/agent/verb_index.yaml`
  - `rust/config/sem_os_seeds/domain_metadata.yaml`
  - `rust/config/ontology/semantic_stage_map.yaml`
  - `rust/config/verb_schemas/intent_tiers.yaml`
  - `rust/config/verb_schemas/taxonomy.yaml`
  - macro/workflow/template files under `rust/config/verb_schemas/`, `rust/config/workflows/`, and `rust/config/verbs/templates/`
  - fixture/test references under `rust/tests/`

### 0A.2 Baseline measurements
- Record current verb/domain counts.
- Record current collision set if an existing analyzer exists.
- Preserve current GATE 5 artifacts for before/after comparison.

---

## Phase 0B: Domain Merges and Deletions

### 0B.1 Delete `case-screening`, fold behavior into `screening`
**Owner decision:** delete `case-screening` entirely.

Implementation:
- Remove `case-screening` domain from `rust/config/verbs/kyc/case-screening.yaml`.
- Move or recreate required verbs under `screening`:
  - `run`
  - `complete`
  - `review-hit`
  - `list-by-workstream`
- Rewrite templates, workflows, tests, and prompts that still reference:
  - `case-screening.run`
  - `case-screening.complete`
  - `case-screening.review-hit`
  - `case-screening.list-by-workstream`

Acceptance:
- No runtime/config references to `case-screening.*` remain.

### 0B.2 Merge `lifecycle` into `service-resource`
**Owner decision:** `lifecycle` domain is merged into `service-resource`.

Implementation:
- Move lifecycle read/list/resource-type/resource-requirement verbs into `service-resource`.
- Preserve semantics by renaming verbs only where needed to avoid collisions.
- Rewrite callers and metadata references from `lifecycle.*` to `service-resource.*`.

Open implementation note:
- `lifecycle` currently models instrument-lifecycle taxonomy, not generic service resources. Verify whether the merged verbs remain under `service-resource` with explicit `resource-family`/`domain` args or whether the service-resource domain absorbs them as lifecycle-flavored verbs.

Acceptance:
- `rust/config/verbs/lifecycle.yaml` is retired or emptied.
- Runtime references use `service-resource.*`.

### 0B.3 Merge `fund-vehicle` and `fund-compartment` into `fund`
**Owner decision:** both merge into `fund`.

Implementation:
- Move `fund-vehicle` and `fund-compartment` verbs from `rust/config/verbs/registry/fund-vehicle.yaml` into `rust/config/verbs/fund.yaml`.
- Ensure canonical post-merge verbs include at least:
  - `fund.list-subfunds`
  - any read/create/ensure operations that currently live under `fund-vehicle` / `fund-compartment`
- Rewrite references in noun index, verb index, stage maps, templates, and tests.

Acceptance:
- No remaining runtime references to `fund-vehicle.*` or `fund-compartment.*`.

### 0B.4 Merge `doc-request` into `document`
**Owner decision:** `doc-request` merges into `document`.

Implementation:
- Move document-request lifecycle verbs from `rust/config/verbs/kyc/doc-request.yaml` into `rust/config/verbs/document.yaml`.
- Preserve document collection semantics under `document.*`.
- Rewrite workflow/template/test references:
  - `doc-request.create`
  - `doc-request.mark-requested`
  - `doc-request.receive`
  - `doc-request.verify`
  - related request/reject/waive verbs

Acceptance:
- No remaining runtime/config references to `doc-request.*`.

### 0B.5 Delete `product-subscription`
**Owner decision:** delete entire domain as redundant.

Implementation:
- Remove `product-subscription` from `rust/config/verbs/matrix-overlay.yaml`.
- Repoint surviving use cases to:
  - `contract.subscribe`
  - `contract.unsubscribe`
  - `cbu.add-product`
  - `cbu.remove-product`
  - `cbu.list-subscriptions` if absent, create it in `cbu.yaml`
- Rewrite references to:
  - `product-subscription.subscribe`
  - `product-subscription.unsubscribe`
  - `product-subscription.list`
  - any suspend/reactivate flow if still needed under another domain

Acceptance:
- No remaining runtime/config references to `product-subscription.*`.

---

## Phase 0C: Type-Parameterized Verb Merges

### 0C.1 `entity.create-*` -> `entity.create`
**Owner decision:** merge to `entity.create` with `entity-type`.

Target legacy verbs:
- `entity.create-limited-company`
- `entity.create-proper-person`
- `entity.create-trust-discretionary`
- `entity.create-partnership-limited`

Implementation:
- Add/expand `entity.create` in `rust/config/verbs/entity.yaml`.
- Add required `entity-type` arg with controlled values.
- Rewrite references across macros, ontology, tests, fixtures, workflows.

### 0C.2 `entity.ensure-*` -> `entity.ensure`
**Owner decision:** merge to `entity.ensure` with `entity-type`.

Target legacy verbs:
- `entity.ensure-limited-company`
- `entity.ensure-proper-person`
- `entity.ensure-trust-discretionary`
- `entity.ensure-partnership-limited`

Implementation:
- Add/expand `entity.ensure`.
- Rewrite references and macro emitters.

### 0C.3 `ubo.end-*` -> `ubo.end-relationship`
**Owner decision:** merge with `relationship-type`.

Target legacy verbs:
- `ubo.end-control`
- `ubo.end-ownership`
- `ubo.end-trust-role`

### 0C.4 `ubo.delete-*` -> `ubo.delete-relationship`
**Owner decision:** merge with `relationship-type`.

Target legacy verbs:
- `ubo.delete-control`
- `ubo.delete-ownership`
- `ubo.delete-trust-role`

### 0C.5 `sla.bind-to-*` -> `sla.bind`
**Owner decision:** merge with `target-type`.

Target legacy verbs:
- `sla.bind-to-csa`
- `sla.bind-to-isda`
- `sla.bind-to-profile`
- `sla.bind-to-resource`
- `sla.bind-to-service`

### 0C.6 `trading-profile.add-*` -> `trading-profile.add-component`
**Owner decision:** merge with `component-type`.

Target family includes:
- `add-counterparty`
- `add-instrument`
- `add-instrument-class`
- `add-market`
- `add-standing-instruction`
- `add-booking-rule`
- `add-csa-config`
- `add-isda-config`
- remaining add-component variants

### 0C.7 `trading-profile.remove-*` -> `trading-profile.remove-component`
**Owner decision:** merge with `component-type`.

### 0C.8 `fund.create-*` -> `fund.create`
**Owner decision:** merge with `fund-type`.

Target family includes:
- `fund.create-umbrella`
- `fund.create-subfund`
- `fund.create-share-class`
- `fund.create-master`
- `fund.create-feeder`
- `fund.create-standalone`

### 0C.9 `fund.ensure-*` -> `fund.ensure`
**Owner decision:** merge with `fund-type`.

Target family includes:
- `fund.ensure-umbrella`
- `fund.ensure-subfund`
- `fund.ensure-share-class`

Acceptance for Phase 0C:
- Legacy family verbs are removed from runtime config.
- Replacements have stable invocation phrases and explicit parameter constraints.

---

## Phase 0D: Reference Data Consolidation

### 0D.1 Create canonical `refdata.*` domain surface
**Owner decision:** all reference-data CRUD collapses into:
- `refdata.ensure`
- `refdata.read`
- `refdata.list`
- `refdata.deactivate`

Required arg:
- `domain: string`
  - `jurisdiction`
  - `currency`
  - `market`
  - `settlement-type`
  - `ssi-type`
  - `client-type`
  - `screening-type`
  - `risk-rating`
  - `case-type`

Implementation:
- Build consolidated verbs under `rust/config/verbs/refdata/`.
- Retire per-domain CRUD surfaces:
  - `jurisdiction.*`
  - `currency.*`
  - `market.*`
  - `settlement-type.*`
  - `ssi-type.*`
  - `client-type.*`
  - `screening-type.*`
  - `risk-rating.*`
  - `case-type.*`

Acceptance:
- Refdata runtime surface is four verbs, parameterized by `domain`.

---

## Phase 0E: Collision Cleanup

Apply exact owner decisions after structural merges are complete.

### 0E.1 Canonicalize and differentiate these phrases
- `"who owns this entity"` -> keep on `ubo.list-by-subject`
- `"trace ownership"` -> keep on `control.trace-chain`
- `"who are the beneficial owners"` -> keep on `control.identify-ubos`
- `"run sanctions screening"` -> keep on `screening.sanctions`
- `"add ownership"` -> keep on `ubo.add-ownership`
- `"appoint sub-advisor"` -> keep on `investment-manager.assign`
- `"issue new shares"` -> keep on `capital.issue-shares`
- `"show sub-funds"` -> keep on `fund.list-subfunds`

### 0E.2 Differentiate dual-valid phrases instead of deleting both
- `"subscribe cbu to product"`
  - `contract.subscribe`
  - `cbu.add-product`
- `"unsubscribe cbu from product"`
  - `contract.unsubscribe`
  - `cbu.remove-product`
- `"add share class"`
  - `capital.*` for voting/control share class intent
  - `fund.*` for economic share class intent
- `"list share classes"`
  - same split as above

### 0E.3 Add missing canonical surface if absent
- `cbu.list-subscriptions`

Acceptance:
- Collision phrases are unique or intentionally bifurcated with differentiated vocabulary.

---

## Phase 0F: Description Enrichment

### 0F.1 Rewrite descriptions and invocation phrases after merges
- Ensure merged verbs describe the parameterized shape clearly.
- Remove stale phrases that point at deleted verbs/domains.
- Add discriminating phrases where owner decisions require differentiation.

---

## Phase 0G: Noun Index and Lexicon Updates

### 0G.1 Update `noun_index.yaml`
- Remove deleted domains/verbs.
- Add merged canonical verbs.
- Update aliases for:
  - `entity.create`
  - `entity.ensure`
  - `fund.create`
  - `fund.ensure`
  - `ubo.end-relationship`
  - `ubo.delete-relationship`
  - `sla.bind`
  - `trading-profile.add-component`
  - `trading-profile.remove-component`
  - `refdata.*`

### 0G.2 Update derived lexicon/index files
- `rust/config/agent/verb_index.yaml`
- `rust/config/sem_os_seeds/domain_metadata.yaml`
- `rust/config/ontology/semantic_stage_map.yaml`
- any verb concept or taxonomy files that enumerate old verbs directly

---

## Phase 0H: Verification and Re-embedding

### 0H.1 Static verification
- `RUSTC_WRAPPER= cargo check -p ob-poc`
- `cargo fmt`

### 0H.2 Registry and test verification
- targeted tests for config loading and runtime registry parity
- targeted intent/verb search tests covering merged verbs
- re-run Sage/Coder comparative coverage harness

### 0H.3 Embedding/index refresh
- Rebuild any generated verb/lexicon artifacts required by semantic search.
- Re-run the relevant embedding/index job if the repo has one.

### 0H.4 Success criteria
- Deleted domains are absent from runtime registry.
- No references remain to removed verb FQNs except in migration/changelog docs.
- Collision analyzer reports zero exact collisions for the resolved owner set.
- GATE 5 improves materially over the current `7/134` Sage+Coder baseline.

---

## Implementation Notes

### Repo observations already confirmed
- `case-screening` is defined in `rust/config/verbs/kyc/case-screening.yaml`
- `doc-request` is defined in `rust/config/verbs/kyc/doc-request.yaml`
- `product-subscription` is defined in `rust/config/verbs/matrix-overlay.yaml`
- `lifecycle` is defined in `rust/config/verbs/lifecycle.yaml`
- `service-resource` is defined in `rust/config/verbs/service-resource.yaml`
- `fund-vehicle` / `fund-compartment` are defined in `rust/config/verbs/registry/fund-vehicle.yaml`
- refdata domains already exist as separate files under `rust/config/verbs/refdata/`

### Recommended execution batches
1. Batch 1: domain deletions/merges
2. Batch 2: type-parameterized family merges
3. Batch 3: refdata consolidation
4. Batch 4: phrase cleanup + description enrichment
5. Batch 5: noun index / lexicon refresh
6. Batch 6: full verification and coverage reruns
