# KYC UBO Reconciliation Engine — **As‑Is Implementation Review** + **Gaps to Target State**

**Status:** Draft — for Opus peer review  
**Date:** February 2026  
**Repo snapshot reviewed:** `kyc-review.tar.gz` (Rust + schema + docs)

---

## 1. Vision (Target State)

KYC/UBO in OB‑POC is a **deterministic convergence engine** over a sparse “graph of claims”:

1) Ingest the client’s alleged ownership/control structure (incomplete, sometimes wrong).  
2) Normalize + link entities (instance resolution).  
3) Compute derived ownership/control facts (direct + indirect, basis-aware).  
4) Detect gaps/contradictions and emit a typed **ObligationSet** (proof + data gaps).  
5) Convert obligations into durable actions (research, document requests, human review).  
6) Re-evaluate until acceptance criteria (“converged enough for decision”) are met.

**Boundary principle:**  
- **Workflow/BPMN‑lite** = durable waits/correlation/timers.  
- **UBO/KYC logic** = graph reconciliation + evidence sufficiency + auditable explain traces.  
- **Policy** = configuration-driven decision mapping (DMN optional).

---

## 2. Problem Statement

KYC/UBO onboarding is not primarily process routing; it’s **world-model verification**:

- Client submissions are **allegations** (claims) about entities, people, and relationships.
- Evidence arrives as documents/extractions/registries/screenings (**observations**).
- We must reconcile allegations vs observations and track convergence of the ownership/control graph.
- Progress is made by converting *alleged* claims into *proven/verified* claims and resolving discrepancies.

---

## 3. What Exists Today (Current Implementation)

This snapshot already implements a large portion of the convergence model, split across:

### 3.1 Canonical Relationship Graph + Edge Convergence (DB-backed)

**Edges live in:**  
- `"ob-poc".entity_relationships`  
  - `relationship_type`: `ownership | control | trust_role | ...`  
  - `percentage`, `ownership_type`, `control_type`, temporal fields, provenance notes  
- `"ob-poc".cbu_relationship_verification`  
  - per (CBU, relationship) verification state + proof link  
  - `status`: `unverified | alleged | pending | proven | disputed | waived`  
  - `alleged_percentage`, `observed_percentage`, `proof_document_id`

**Computed views (already very “ObligationSet-like” for edges):**
- `"ob-poc".cbu_convergence_status` / `"ob-poc".ubo_convergence_status` — rollups of edge states
- `"ob-poc".cbu_ownership_graph` — edge graph joined to verification state for UI/analysis
- `"ob-poc".ubo_missing_proofs` — edges missing a proof document + suggested proof type  
- `"ob-poc".ubo_expired_proofs` — edges with invalid/expired proofs

**Declarative gate audit:**
- `"ob-poc".ubo_assertion_log` — logs assertion outcomes (gates) with JSON failure details.

**Interpretation:** the *edge* side of the “graph of claims” is already strongly modeled and has first‑class lifecycle and reporting.

---

### 3.2 Node-Level Claims, Evidence, and Discrepancies (DB-backed)

A parallel evidence model exists for **attributes and identity/proofs**:

- `"ob-poc".client_allegations`  
  - `alleged_value` + `verification_status` (`PENDING/IN_PROGRESS/VERIFIED/...`)
  - linkage to `case_id`, `workstream_id`, and `verified_by_observation_id`
- `"ob-poc".attribute_observations`  
  - typed value columns + provenance (`source_type`, `source_document_id`, confidence, authoritative flag)
  - supports supersession / effective dates / multiple observations per attribute
- `"ob-poc".observation_discrepancies`  
  - structured discrepancy records (severity, type, resolution workflow)
- `"ob-poc".detected_patterns`  
  - for adversarial/pattern findings (persisted by verification ops)

There is also an attribute dictionary/registry with applicability and reconciliation metadata:
- `"ob-poc".attribute_registry` includes `reconciliation_rules`, `acceptable_variation_threshold`,
  `requires_authoritative_source`, etc.

**Interpretation:** node-level “claim vs evidence vs discrepancy” exists and is richer than the edge model; it’s already close to the “compiler pipeline” vision.

---

### 3.3 KYC Case/Workstream + Document Collection Tracking (KYC schema)

The KYC workflow substrate is present:

- `kyc.cases` — case lifecycle (`INTAKE → DISCOVERY → ASSESSMENT → REVIEW → ...`)
- `kyc.entity_workstreams` — per-entity work items with `status`, blockers, EDD flags, etc.
- `kyc.doc_requests` — document requirements lifecycle (`REQUIRED/RECEIVED/VERIFIED/WAIVED/...`)
- `kyc.outreach_requests` — counterparty/outreach tracking (UBO disclosure, share register, etc.)
- `kyc.research_*` tables — research decisions/actions/anomalies/corrections (agentic research audit)
- `kyc.ownership_snapshots` + `kyc.ownership_reconciliation_*` — ownership snapshot + reconciliation runs/findings

**Interpretation:** KYC “durable case management” is modeled; doc requirements are already first‑class and auditable.

---

### 3.4 Implemented Custom Ops (Rust) that matter for KYC/UBO

#### UBO computation & traversal
- `rust/src/domain_ops/ubo_analysis.rs`
  - `ubo.calculate` — recursive ownership rollup against `entity_relationships`
  - `ubo.trace-chains` — calls `"ob-poc".compute_ownership_chains(...)` (ownership + control path support)
  - `ubo.list-owners` — temporal-aware listing of owners

#### UBO lifecycle maintenance (convergence model hygiene)
- `rust/src/domain_ops/ubo_graph_ops.rs`
  - `ubo.convergence-supersede` — atomically end old edge + create new edge + reset verification
  - `ubo.transfer-control` — move control edges with audit trail
  - `ubo.mark-deceased`, `ubo.waive-verification` — governance lifecycle events

#### Ownership snapshots & reconciliation (register-style)
- `rust/src/domain_ops/ownership_ops.rs`
  - `ownership.compute` — calls `kyc.fn_derive_ownership_snapshots(issuer, as_of)`
  - `ownership.snapshot.list` — list snapshots with filtering
  - reconciliation ops create runs + findings in `kyc.ownership_reconciliation_*`

#### Verification (adversarial model)
- `rust/src/domain_ops/verify_ops.rs` + `rust/src/verification/*`
  - pattern detection, evasion analysis on doc request history
  - confidence aggregation
  - registry verification stubs (interfaces exist; real API calls are TODO/stubs)

#### Readiness / “tollgate” style checks
- `rust/src/domain_ops/tollgate_ops.rs`
  - computes coverage metrics (ownership verification %, doc completeness %, screening %, red flags)
  - returns blocking issues + recommendations (currently as strings)

#### BPMN-lite integration (durable orchestration)
- `rust/src/bpmn_integration/*`
  - workflow dispatcher/worker, correlation records, parked tokens, retry queue
- `rust/src/domain_ops/bpmn_lite_ops.rs`
  - DSL verbs to `compile/start/signal/cancel/inspect` a process instance

**Interpretation:** the platform already has the pieces to support “verbs start a durable flow, then park and resume on signals”.

---

### 3.5 Document Ops: Catalog is solid; Extraction is intentionally incomplete

- `rust/src/domain_ops/document_ops.rs`
  - `document.catalog` is idempotent and wired to `document_types`
  - `document.extract` is **explicitly TODO** (placeholder: sets extraction status)

This is a key practical gap because KYC evidence convergence depends on turning docs into observations.

---

## 4. What’s Missing (Gaps to Reach the Target State)

This section is the “delta” between the architecture vision and what the current implementation delivers.

### Gap A — No first-class **ObligationSet** object spanning nodes + edges

**As-is:**  
- Edge “obligations” exist implicitly via views like `ubo_missing_proofs`, and via `cbu_relationship_verification.status`.
- Node obligations exist implicitly via `client_allegations.verification_status`, discrepancies, and required attributes.

**Missing:**  
A single typed, queryable **ObligationSet** (or equivalent) that:
- is emitted deterministically by reconciliation passes,
- references `node_id | edge_id | allegation_id | discrepancy_id`,
- has `severity`, `policy_basis`, and `explain`,
- is directly convertible into `kyc.doc_requests`, `kyc.outstanding_requests`, research tasks, or review queues.

**Why it matters:**  
Today, “what’s missing?” is split across views, statuses, and ad hoc checks. The target architecture wants one canonical “gap payload” to drive orchestration and UI.

---

### Gap B — Automatic “edge proof gaps → doc_requests” bridge is not unified

**As-is:**  
- `ubo_missing_proofs` can tell you “required_proof_type” for each missing edge.
- `kyc.doc_requests` tracks collection/verification of documents.

**Missing:**  
A deterministic planner step (verb) like:

- `ubo.plan-proof-requests(:cbu-id ...) -> {doc_requests_to_create, reasons, correlations}`  
- and a bridge to `document.request` / `kyc.doc_requests` generation with idempotency keys.

**Desired behavior:**  
- Missing edge proof → create doc_request(s) in batch with stable `batch_id` and explain text.
- When proof arrives (upload) → attempt extraction → update edge verification state.

---

### Gap C — Document extraction to observations/verification is stubbed

This is the largest “real world” functional gap.

**As-is:**  
- `document.extract` updates extraction status but does not create observations.
- There are good data structures (`attribute_observations`, `client_allegations`, discrepancy tables).

**Missing:**  
A pipeline:

1) `document.upload` / `document.upload-version` stores metadata + file reference (S3/minio/etc).  
2) `document.extract` produces structured results and **writes `attribute_observations`**.  
3) A reconciliation step compares observations to allegations and:
   - updates `client_allegations.verification_status`
   - creates `observation_discrepancies` when needed  
4) Edge‑level verification updates (ownership/control proofs) when a register/board doc confirms edges:
   - update `"ob-poc".cbu_relationship_verification` status + `observed_percentage`

---

### Gap D — Share class / voting basis integration is “parallel”, not converged

**As-is:**  
- There is a rich capital structure model in `kyc.share_classes`, `kyc.holdings`, `kyc.ownership_snapshots`.
- There is also the generic `entity_relationships` ownership edges with `percentage`.

**Missing:**  
A canonical stance on how these relate:
- Does `entity_relationships` represent the *declared/alleged* graph, while `ownership_snapshots` represents *observed* register truth?
- If so, where is the deterministic “compare + update verification status” step?

**Target:**  
A basis-aware reconciliation pass:
- Inputs: holdings/register snapshots + alleged relationships  
- Outputs: edge verification updates + reconciliation findings + obligations

---

### Gap E — Registry verification is stubbed (interfaces exist)

**As-is:**  
- verification registry module defines types and stub methods (GLEIF, Companies House, EDGAR, etc.)

**Missing:**  
Real connectors + mapping to:
- entity linking (canonical IDs)
- authoritative observations (write to `attribute_observations` with `is_authoritative=true`)
- discrepancy generation when registry contradicts client allegations

---

### Gap F — KYC + BPMN-lite wiring for “client portal upload → resume workflow” needs a concrete contract

**As-is:**  
- BPMN-lite integration is real and sophisticated: correlation records, parked tokens, signal relay.
- KYC has doc_requests and outreach requests.

**Missing:**  
A domain-level correlation contract and the durable verb(s) that own it, e.g.:

- `document.request` or `doc-request.request` routes via BPMN-lite with a `correlation_field` = `request_id`.
- Client portal upload posts `request_id` → triggers BPMN `signal(message="doc_uploaded", payload={request_id,...})`.
- BPMN service task resumes, runs `document.catalog/extract/verify`, then updates `doc_requests` + obligations.

This is mostly **wiring + conventions**, but it must be explicit to avoid “mystery glue”.

---

### Gap G — Readiness/tollgates exist but output is not yet typed/explainable enough for automation

**As-is:**  
- `tollgate_ops` computes readiness metrics and returns `blocking_issues` + `recommended_actions` as strings.

**Missing:**  
- typed blockers that can be re-used as obligations
- stable identifiers for blockers (for UI linking and “resolve this” flows)
- policy basis + explain trace (why a blocker is present)

---

## 5. Proposed “Next Steps” (Minimal-to-Useful Path)

### Step 1 — Introduce a canonical `KycObligation` type + storage
- Rust types: `KycObligation { id, target_ref, kind, severity, policy_basis, explain, created_at, resolved_at }`
- DB table (or materialized view + stable IDs) for obligations **or** structured JSON emitted by a verb.

Start by generating obligations from:
- `ubo_missing_proofs`
- open `observation_discrepancies`
- `doc_requests` not complete
- `screenings` not clear
- unresolved `client_allegations`

### Step 2 — Add a deterministic planner verb: “obligations → actions”
- `kyc.plan-actions(:case-id ...) -> {doc_requests_to_create, research_tasks, review_items}`
- idempotent batch creation (`batch_id`, `idempotency_key`)
- record an explain trace (rule hits / policy profile)

### Step 3 — Implement document extraction “thin slice”
Even a narrow extraction path (passport + register extract) unlocks end-to-end convergence.

- `document.extract` should write `attribute_observations`
- add a reconciliation step:
  - `allegation.verify-against-observations` or `verify.compare-claims`
  - produces discrepancies + updates allegation status

### Step 4 — Add edge proof verification update for ownership/control edges
- From extracted register/board docs:
  - compute `observed_percentage` (or “confirmed”) for relevant edges
  - update `"ob-poc".cbu_relationship_verification` state machine

### Step 5 — Basis-aware bridge between capital snapshots and relationship edges
- define: alleged vs observed vs derived
- implement: `ownership.reconcile-against-edges` (or similar) to populate findings and update verification statuses.

### Step 6 — Wire BPMN-lite to doc_requests (one durable verb end-to-end)
- pick one workflow: “request shareholder register”  
- implement a durable verb (YAML `DurableConfig`) with correlation on `request_id`
- implement the portal callback path that signals BPMN-lite and resumes the REPL/runbook entry.

---

## 6. Notes on “DMN / Drools” in the As‑Is System

The current implementation already supports “DMN-like” decisioning without importing a DMN engine:

- thresholds appear as verb args (e.g., `ubo.calculate :threshold 25`)
- rule evaluation exists as a pure Rust expression evaluator (`domain_ops/rule_evaluator.rs`) for other domains and can be reused for KYC policy mapping
- attribute registry includes reconciliation metadata and authoritative-source requirements

**Recommendation:** keep policy as versioned configuration + pure evaluators. Move to full DMN only if you need external tooling governance.

---

## 7. Appendix — Key Artifacts for Review

### DB Objects (schema.sql)
- `"ob-poc".entity_relationships`
- `"ob-poc".cbu_relationship_verification`
- Views: `"ob-poc".ubo_missing_proofs`, `"ob-poc".ubo_expired_proofs`,
  `"ob-poc".cbu_ownership_graph`, `"ob-poc".ubo_convergence_status`
- `"ob-poc".client_allegations`, `"ob-poc".attribute_observations`, `"ob-poc".observation_discrepancies`
- `kyc.cases`, `kyc.entity_workstreams`, `kyc.doc_requests`, `kyc.outreach_requests`
- `kyc.ownership_snapshots`, `kyc.ownership_reconciliation_runs/findings`

### Rust Modules
- UBO analysis: `rust/src/domain_ops/ubo_analysis.rs`
- UBO lifecycle ops: `rust/src/domain_ops/ubo_graph_ops.rs`
- Ownership snapshots/recon: `rust/src/domain_ops/ownership_ops.rs`
- Verification model: `rust/src/verification/*` + `rust/src/domain_ops/verify_ops.rs`
- Document ops: `rust/src/domain_ops/document_ops.rs` (extraction TODO)
- Tollgates: `rust/src/domain_ops/tollgate_ops.rs`
- BPMN-lite: `rust/src/bpmn_integration/*` + `rust/src/domain_ops/bpmn_lite_ops.rs`
- DSL v2 pipeline: `rust/src/dsl_v2/*`

---

## 8. Opus Review Prompts

1) Does the “edge convergence” model (`cbu_relationship_verification`) capture the right lifecycle for ownership/control proof?  
2) Should node-level “identity proofs” be modeled as obligations in the same format as edge proofs?  
3) Is “ObligationSet” the right abstraction, or should we rely on views + derived JSON outputs?  
4) How should capital structure snapshots (share classes/holdings) map into entity_relationship edges?  
5) What is the minimal end-to-end workflow slice that proves the architecture (doc request → upload → extract → update obligations → converge)?  
6) Any high-risk missing invariants (trusts/nominees/negative control) that should be first-class?  
7) Is BPMN-lite wiring contract clear enough (correlation keys, parked tokens, resume semantics)?  
8) What explain traces are mandatory for audit vs “nice to have”?

