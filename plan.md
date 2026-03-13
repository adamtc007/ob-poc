# Document Polymorphism Implementation Plan

Status: Approved planning draft

Inputs:
- [`docs/todo/Document_Polymorphism_Architecture_v1.0_1.md`](/Users/adamtc007/Developer/ob-poc/docs/todo/Document_Polymorphism_Architecture_v1.0_1.md)
- [`docs/todo/document_polymorphism_repo_impl_plan.md`](/Users/adamtc007/Developer/ob-poc/docs/todo/document_polymorphism_repo_impl_plan.md)

Locked decisions:
- Canonical document runtime is built on `documents` + `document_versions`.
- `document_catalog` is fully retired and any retained metadata is migrated into the new runtime and/or SemOS registry.
- This is a hard rip-and-replace.
- No long-lived compatibility surface is allowed.
- UBO proof tables are out of scope for the first delivery.
- `RequirementProfile`, `ProofObligation`, and `EvidenceStrategy` are SemOS-governed in the first wave.
- The DSL surface switches directly to the new plane-specific verbs.

## 1. Current Modules / Files Involved

### Document runtime

- [`migrations/049_workflow_task_queue_documents.sql`](/Users/adamtc007/Developer/ob-poc/migrations/049_workflow_task_queue_documents.sql)
  - Current operational runtime:
  - `document_requirements`
  - `documents`
  - `document_versions`
  - `document_events`
  - `v_requirements_with_latest_version`
  - `v_documents_with_status`
  - Trigger coupling from version verification state back into requirement status

- [`rust/src/domain_ops/document_ops.rs`](/Users/adamtc007/Developer/ob-poc/rust/src/domain_ops/document_ops.rs)
  - Current write/read behavior for solicitation, upload, verify, reject, and “missing” queries

- [`rust/config/verbs/document.yaml`](/Users/adamtc007/Developer/ob-poc/rust/config/verbs/document.yaml)
  - Current public DSL/verb contract

### Requirement / bundle layer

- [`rust/src/document_bundles/types.rs`](/Users/adamtc007/Developer/ob-poc/rust/src/document_bundles/types.rs)
- [`rust/src/document_bundles/registry.rs`](/Users/adamtc007/Developer/ob-poc/rust/src/document_bundles/registry.rs)
- [`rust/src/document_bundles/service.rs`](/Users/adamtc007/Developer/ob-poc/rust/src/document_bundles/service.rs)
- [`rust/src/domain_ops/docs_bundle_ops.rs`](/Users/adamtc007/Developer/ob-poc/rust/src/domain_ops/docs_bundle_ops.rs)
- [`rust/config/document_bundles/baseline.yaml`](/Users/adamtc007/Developer/ob-poc/rust/config/document_bundles/baseline.yaml)

### Legacy document catalog / metadata universe

- [`rust/src/database/document_service.rs`](/Users/adamtc007/Developer/ob-poc/rust/src/database/document_service.rs)
- [`rust/config/sem_os_seeds/domain_metadata.yaml`](/Users/adamtc007/Developer/ob-poc/rust/config/sem_os_seeds/domain_metadata.yaml)
  - Still centered on `document_catalog`, `document_types`, and attribute mapping tables

### Assertion-adjacent / proof-adjacent universe

- [`rust/migrations/202412_ubo_convergence.sql`](/Users/adamtc007/Developer/ob-poc/rust/migrations/202412_ubo_convergence.sql)
- [`rust/src/domain_ops/ubo_graph_ops.rs`](/Users/adamtc007/Developer/ob-poc/rust/src/domain_ops/ubo_graph_ops.rs)
- [`rust/config/workflows/kyc_convergence.yaml`](/Users/adamtc007/Developer/ob-poc/rust/config/workflows/kyc_convergence.yaml)

### Observation / attribute extraction universe

- [`rust/config/verbs/observation/observation.yaml`](/Users/adamtc007/Developer/ob-poc/rust/config/verbs/observation/observation.yaml)
- [`rust/src/domain_ops/observation_ops.rs`](/Users/adamtc007/Developer/ob-poc/rust/src/domain_ops/observation_ops.rs)
- [`rust/config/verbs/attribute.yaml`](/Users/adamtc007/Developer/ob-poc/rust/config/verbs/attribute.yaml)

### Workflow / context / StateGraph surfaces

- [`rust/config/workflows/kyc_onboarding.yaml`](/Users/adamtc007/Developer/ob-poc/rust/config/workflows/kyc_onboarding.yaml)
- [`rust/config/workflows/kyc_case.yaml`](/Users/adamtc007/Developer/ob-poc/rust/config/workflows/kyc_case.yaml)
- [`rust/config/stategraphs/cbu.yaml`](/Users/adamtc007/Developer/ob-poc/rust/config/stategraphs/cbu.yaml)
- [`rust/src/stategraph/mod.rs`](/Users/adamtc007/Developer/ob-poc/rust/src/stategraph/mod.rs)

## 2. Schema Changes

### Canonical target direction

- Use `documents` as the logical-document base.
- Replace or evolve `document_versions` into Plane 1 `document_artifacts`.
- Retire `document_catalog` entirely after metadata migration.
- Introduce SemOS-governed requirement semantics in wave 1 rather than operational placeholder tables.

### New or renamed runtime tables

- `logical_documents`
  - Canonical successor to current `documents`
  - Stable logical identity by subject + document type + lineage

- `document_artifacts`
  - Canonical successor to current `document_versions`
  - Immutable submissions
  - Revision chain
  - Intrinsic verification state
  - Classification snapshot
  - Supersession metadata

- `document_artifact_events`
  - Artifact lifecycle audit/event history

- `document_assertions`
  - Generic Plane 2 fact store
  - Artifact-bound, logical-document-aware, attribute-bound

- `document_assertion_events`
  - Approval, rejection, supersession, expiry audit trail

- `document_context_acceptances`
  - Plane 3 business/context decisions
  - Supports artifact, assertion, and strategy subjects

### SemOS-governed first-wave objects

- `RequirementProfile`
- `ProofObligation`
- `EvidenceStrategy`
- Any related policy/versioned metadata needed for requirement computation

These should not be implemented as a temporary parallel `ob-poc` operational authoring layer.

### Tables retained only during migration execution

- `document_requirements`
- `documents`
- `document_versions`
- `document_events`
- `document_catalog`

All of these are migration sources or cutover intermediates, not long-lived compatibility surfaces.

## 3. Read-Model / Projection Changes

### New projections

- `v_document_artifact_current`
  - Latest active artifact per logical document

- `v_document_assertion_current`
  - Current approved/pending assertions per artifact and attribute

- `v_document_context_status`
  - Full context acceptance map

- `v_document_requirement_matrix`
  - Obligation-driven coverage projection
  - Mandatory coverage
  - Overall coverage
  - Category coverage
  - Active strategy
  - Component status

- `v_document_obligation_signals`
  - StateGraph-facing signal projection
  - `doc_mandatory_coverage`
  - `doc_overall_coverage`
  - `doc_identity_coverage`
  - `doc_financial_coverage`
  - `doc_compliance_coverage`
  - `doc_has_expired_artifacts`
  - `doc_has_rejected_artifacts`
  - `doc_has_unapproved_assertions`
  - `doc_outstanding_solicitations`

### Existing projections to remove or replace

- `v_requirements_with_latest_version`
- `v_documents_with_status`

### Projection ownership

- Compute document obligation signals in the document read-model/projection layer.
- Keep `stategraph::walk_graph` as a consumer of signals, not the owner of document semantics.

## 4. DSL / Verb / API Changes

### New document verbs

- `document.verify-artifact`
- `document.list-assertions`
- `document.approve-assertion`
- `document.reject-assertion`
- `document.accept-in-context`
- `document.reject-in-context`
- `document.waive-in-context`
- `document.compute-requirements`
- `document.evidence-strategies`

### Existing verbs to remove or rewrite

- `document.verify`
  - Replace with plane-specific artifact verification semantics

- `document.reject`
  - Replace with separate artifact rejection vs context rejection verbs

- `document.missing-for-entity`
  - Re-express as a compatibility read over requirement computation during cutover, then retire

- `document.catalog`
  - Remove with `document_catalog` retirement

### Handler/module refactor

Split current document behavior into explicit modules:

- `artifact_ops.rs`
- `assertion_ops.rs`
- `context_ops.rs`
- `requirement_ops.rs`

### API changes

- Add DTOs and route responses for:
  - `RequirementMatrix`
  - `ProofAssertion`
  - `ContextAcceptance`
  - `ArtifactVerificationResult`

- Update noun/verb discovery and DSL surface:
  - [`rust/config/noun_index.yaml`](/Users/adamtc007/Developer/ob-poc/rust/config/noun_index.yaml)
  - [`rust/config/verbs/document.yaml`](/Users/adamtc007/Developer/ob-poc/rust/config/verbs/document.yaml)

## 5. Migration and Backfill Needs

### One-time migration path

- Build explicit field mapping from:
  - `document_requirements`
  - `documents`
  - `document_versions`
  - `document_catalog`
  - `document_events`

- Migrate retained `document_catalog` metadata into:
  - new runtime tables, or
  - SemOS registry objects

- Move logical document identity into `logical_documents`.
- Move immutable submissions into `document_artifacts`.
- Backfill assertion rows where extractable historical evidence exists.
- Recompute or seed initial context acceptance rows where current business state already implies acceptance/rejection.

### Out of scope in wave 1

- `proofs`
- `ubo_assertion_log`
- UBO-specific proof runtime convergence

### Cutover milestones

- milestone 1:
  - new schema and migration scripts exist

- milestone 2:
  - all write paths point to new runtime

- milestone 3:
  - all reads point to new projections and new DSL surface

- milestone 4:
  - legacy tables and temporary migration bridges are removed

## 6. Test Strategy

### Schema and migration tests

- migration apply tests
- migration backfill idempotency tests
- retirement validation for `document_catalog`
- supersession chain integrity tests

### Unit tests

- artifact lifecycle
- assertion lifecycle
- context acceptance lifecycle
- requirement matrix computation
- strategy/component evaluation

### Integration tests

- solicit -> upload artifact -> verify artifact -> extract assertions -> accept in context -> recompute requirements
- direct plane-specific verb flows
- StateGraph signal flow on obligation-based document milestones

### Regression tests

- legacy KYC document collection paths
- API responses that previously assumed one document status
- deal and context attachment consumers that must be rewired off `document_catalog`

## 7. Risks / Hidden Couplings

- Multiple document systems already exist in parallel.
- SemOS metadata currently describes the wrong document source of truth.
- Requirement state is currently trigger-coupled to version verification state.
- `document.reject` is semantically overloaded today.
- StateGraph document lane is count-based today, not obligation-based.
- `document_catalog` has hidden FK consumers across the app.
- Observation/assertion boundaries are adjacent but not yet unified.

## 8. Safe Delivery TODO

### Phase 0: Freeze implementation boundary

- [ ] Confirm exact rename/new-table strategy for `documents` -> `logical_documents`
- [ ] Confirm exact rename/new-table strategy for `document_versions` -> `document_artifacts`
- [ ] Confirm first-wave context set:
  - KYC workstream
  - CBU evidence
  - deal document
  - entity verification

### Phase 1: Current-state consolidation

- [ ] Build field-level source-to-target mapping matrix
- [ ] Enumerate all `document_catalog` FK/codepath consumers
- [ ] Enumerate all old document verbs and dependent API routes
- [ ] Enumerate all StateGraph/document signal consumers
- [ ] Enumerate all retained metadata that must leave `document_catalog`

### Phase 2: Plane 1 foundation

- [ ] Implement `logical_documents`
- [ ] Implement `document_artifacts`
- [ ] Implement artifact supersession and revision chain
- [ ] Implement artifact event stream
- [ ] Replace trigger-driven requirement coupling with explicit projection logic

### Phase 3: Plane 2 foundation

- [ ] Implement `document_assertions`
- [ ] Implement assertion event stream
- [ ] Bind assertions to governed attributes
- [ ] Implement list / approve / reject assertion verbs
- [ ] Define assertion carry-forward and supersession rules

### Phase 4: Plane 3 foundation

- [ ] Implement `document_context_acceptances`
- [ ] Implement typed context subject model
- [ ] Implement accept / reject / waive verbs
- [ ] Implement policy version binding persistence
- [ ] Implement context-status projection

### Phase 5: SemOS-governed requirement semantics

- [ ] Implement SemOS-governed `RequirementProfile`
- [ ] Implement SemOS-governed `ProofObligation`
- [ ] Implement SemOS-governed `EvidenceStrategy`
- [ ] Implement requirement computation engine
- [ ] Implement `document.compute-requirements`
- [ ] Implement `document.evidence-strategies`

### Phase 6: StateGraph integration

- [ ] Implement obligation/category signal projection
- [ ] Replace count-based document gate logic
- [ ] Add paper-aligned signals to graph consumers
- [ ] Update graph tests

### Phase 7: Surface cutover

- [ ] Switch DSL surface to plane-specific verbs
- [ ] Switch APIs to new DTOs and projections
- [ ] Remove legacy write paths
- [ ] Update SemOS metadata and verb footprints
- [ ] Run migration validation and replay checks

### Phase 8: Legacy retirement

- [ ] Retire `document_catalog`
- [ ] Retire no-longer-canonical legacy tables
- [ ] Remove obsolete triggers and legacy status coupling
- [ ] Remove deprecated noun/verb mappings
- [ ] Finalize rollback and operational runbook

No coding starts until explicit approval is given.
