# OB-POC — Companion Appendix
> **Last reconciled:** 2026-03-08  
> **Companion to:** `OB_POC_SCHEMA_ENTITY_OVERVIEW_refocused.md`  
> **Purpose:** map the *cross-cutting platform subsystems* (Documents/Evidence, Semantic Registry, Agent learning, Runbooks/BPMN, Events/Feedback) back onto the **three main taxonomies**:
> 1) **Deal Map (Deal Record)**  
> 2) **Onboarding Request**  
> 3) **KYC / UBO**

This appendix is written for “system navigation”: when you’re looking at a Deal Map / Onboarding Request / KYC case and ask:
- *Where are the documents?*  
- *Where do verb definitions come from?*  
- *Where are agent corrections stored?*  
- *Where is durable workflow state parked?*  
- *Where do we audit / debug failures?*

---

## 0) Quick orientation: the two schemas

- **`"ob-poc"`** — single business entity model (~304 tables). Contains all business objects, execution artifacts, agent learning, teams/access, feedback, events, and reference data. Former `agent`, `teams`, `feedback`, `events`, `sessions`, `ob_kyc`, `ob_ref` schemas consolidated here.
- **`sem_reg`** — SemOS metadata dictionary (25 tables incl. stewardship). Versioned definition store: snapshots, changesets, outbox, plus basis records, conflict resolution, focus states, templates, verb implementation bindings.
- **`sem_reg_authoring`** — governed authoring pipeline (6 tables). Validation reports, governance audit, batch publish, artifact storage/archive.
- **`sem_reg_pub`** — published “active snapshot set” projections used at runtime (4 tables).

**SemOS domain_metadata** provides 100% coverage: every table has governance_tier, classification, pii flags, and verb data footprint mappings.

---

# A) Documents & Evidence Library

### The intent
Documents aren’t just files. In OB‑POC, a document is:
1) **a stored artefact** (S3/MinIO key + hash)  
2) **typed** (document type / family / domain)  
3) **extractable** (structured extraction → attributes/observations)  
4) **evidence-bearing** (can satisfy requirements for onboarding or KYC gates)

### Core tables (document layer)
- **`"ob-poc".document_catalog`** — the document instance registry
  - `doc_id` (PK), `storage_key`, `file_hash_sha256`, `mime_type`, `file_size_bytes`
  - `extracted_data`, `extraction_status`, `extraction_confidence`
  - optional scoping: `cbu_id`, `entity_id`, `document_type_id`, `document_type_code`
- **`"ob-poc".document_types`** — the document type definition set
  - `type_id` (PK), `type_code`, `category`, `domain`, `required_attributes`, `applicability`, `semantic_context`
  - embeddings for semantic matching: `embedding` (768-dim) + model metadata

### Attribute binding (proof semantics)
Documents become “evidence” by linking them to the attribute dictionary:
- **`"ob-poc".document_attribute_links`**
  - binds a `document_type_id` to an `attribute_id`
  - describes proof semantics: `direction`, `is_authoritative`, `proof_strength`, applicability filters (entity types, jurisdictions, client types…)
  - also carries extraction hints: `extraction_field_path`, `extraction_method`, `extraction_hints`
- **`"ob-poc".document_attribute_mappings`**
  - mapping-level details for extraction: `field_location`, `field_name`, thresholds, patterns

### Where documents attach into the 3 main taxonomies

#### 1) Deal Map
Commercial documents get attached to the deal:
- **`"ob-poc".deal_documents(deal_id, document_id, document_type, document_status, version…)`**

Typical examples:
- fee schedules, term sheets, price confirmations, draft contract markups, change memos.

#### 2) Onboarding Request
Operational onboarding evidence tends to attach at the **CBU** level (and then is “consumed” by the onboarding request as a dependency):
- **`"ob-poc".cbu_evidence(cbu_id, document_id, evidence_type, verification_status…)`**

Examples:
- SSI confirmations, account-opening forms, tax forms, operational attestations, mandate letters.

#### 3) KYC / UBO
KYC runs a “request → receive → review/verify” loop:
- **`"ob-poc".doc_requests(workstream_id, doc_type, status, due_date, received_at, verified_at, document_id …)`**
- evidence can then be linked to UBO assertions via:
  - **`"ob-poc".ubo_evidence(ubo_id, document_id, evidence_type, evidence_role, verification_status…)`**
  - and/or **`"ob-poc".kyc_ubo_evidence(ubo_id, document_id, screening_id, relationship_id, determination_run_id, status…)`**

> Practical note: you currently have *two* UBO evidence tables (`ubo_evidence` and `kyc_ubo_evidence`). The appendix treats them as:
> - **`ubo_evidence`** = “human-friendly evidence attachment model” (attestation, verification notes)  
> - **`kyc_ubo_evidence`** = “case‑workflow evidence model” (ties into screenings / relationship edges / determination runs)
>
> If you later unify them, the join points in this appendix still stand.

### “Document extraction → attributes” (how it becomes machine-usable)
This is the pipeline path the agent/DSL can exploit:
- `document_catalog.extracted_data` (JSONB) + mappings/links  
→ derive *typed* attribute values and/or observations in:
- **`"ob-poc".attribute_registry`** (dictionary of governed attributes)  
- **`"ob-poc".attribute_values_typed`** (typed storage)  
- **`"ob-poc".attribute_observations`** (observed/derived facts with provenance)

---

# B) Semantic Registry (SemReg) — versioned definitions that drive runtime behavior

### The intent
SemReg is your “definitions OS”: it versions and governs the things the agent and DSL need to behave deterministically:
- entity/relationship type definitions
- taxonomies and membership rules
- verb contracts (the *agent-discoverable* verb surface)
- document type definitions / evidence requirements
- policy rules, view definitions, derivations

### What exists in the schema
**Authoring / versioning**
- **`sem_reg.snapshot_sets`** — groups “a coherent set of definitions”
- **`sem_reg.snapshots`** — versioned objects (by `object_type` + `object_id`)
- **`sem_reg.changesets`** + **`sem_reg.changeset_entries`** — authoring workflow for changes

**Runtime projection**
- **`sem_reg.outbox_events`** — publish events for snapshot sets (durable projection trigger)
- **`sem_reg_pub.projection_watermark`** — projection watermark (at-least-once processing)
- **`sem_reg_pub.active_*`** — published active definitions
  - `active_verb_contracts`, `active_taxonomies`, `active_entity_types`

### SemReg object types (the “definition universe”)
Your enum `sem_reg.object_type` includes (selection):
- `verb_contract`
- `taxonomy_def`, `taxonomy_node`, `membership_rule`
- `entity_type_def`, `relationship_type_def`
- `document_type_def`, `evidence_requirement`
- `policy_rule`, `view_def`, `derivation_spec`, `observation_def`

### How SemReg maps back to the 3 main taxonomies

#### 1) Deal Map
SemReg controls the *commercial vocabulary* that makes the Deal Map navigable and consistent:
- **Taxonomies** that classify products, deal types, pricing models, contract families
- **View definitions** that define a “Deal Map page” (what nodes/edges to show, default pivots)
- **Policy rules** that enforce commercial constraints (e.g., “product X requires contract template Y”)

#### 2) Onboarding Request
SemReg is the glue between *commercial SKU* and *operational provisioning*:
- `taxonomy_*` defines “product → services/resources” decompositions (or points to where it’s defined)
- `verb_contract` definitions constrain what the agent can do to fulfill onboarding work
- `evidence_requirement` definitions express gating rules (“cannot provision until proofs present”)

#### 3) KYC / UBO
SemReg governs the “what must be proven” and “how do we compute/interpret” layer:
- `evidence_requirement` and `policy_rule` define thresholds and minimum evidence for clearance
- `derivation_spec` and `observation_def` define what can be derived from documents + graph facts
- `view_def` defines the navigable KYC “matrix” (workstreams, missing proofs, UBO candidates)

### Why `sem_reg_pub` matters
Your runtime and agent should primarily read **published** definitions:
- `sem_reg_pub.active_verb_contracts` for intent → verb selection
- `sem_reg_pub.active_taxonomies` for classification & UI navigation
- `sem_reg_pub.active_entity_types` for entity-kind constrained intent resolution

That lets you keep authoring (changesets/snapshots) separate from runtime safety.

---

# C) Agent learning schema — “utterance → intent → verb” feedback loop

### The intent
The agent schema is the empirical layer: it stores what users actually said, what the system chose, and how it was corrected, so the system can get better without turning the runtime into a probabilistic soup.

### What exists
- **`agent.events`** — session-level trace records
  - user message, parsed intents (JSON), selected verb, generated DSL, corrections, resolution results, execution outcome
- **`agent.invocation_phrases`** — learned mapping `phrase → verb` (+ embedding)
- **`agent.entity_aliases`** — aliasing `alias → canonical_name` (optional `entity_id`)
- **`agent.learning_candidates`** — “possible new learning” queue (reviewable)
- **`agent.user_learned_phrases`** — per-user mappings (high precision personalization)
- **`agent.phrase_blocklist`** — “never interpret phrase X as verb Y” guardrail
- **`agent.learning_audit`** — rollbackable audit trail
- **`agent.lexicon_tokens`** — token-level stats (useful for typo tolerance / domain hints)

### How agent learning maps back to the 3 main taxonomies

#### 1) Deal Map
Agent events during commercial negotiation can be dominated by:
- entity disambiguation (client entities, contracting parties)
- product naming variations
- pricing verbs (“show rate card”, “propose”, “counter”, “supersede”)

This is where `entity_aliases` and `invocation_phrases` pay off fast.

#### 2) Onboarding Request
Operational language tends to include:
- “onboard these CBUs”
- “provision account/SSI/channel”
- “mark blocked, list blockers”
- “generate doc request pack”

This is where `phrase_blocklist` matters: provisioning verbs are high impact.

#### 3) KYC / UBO
KYC language tends to include:
- “find UBOs”, “run determinations”, “show missing proofs”
- “request passports”, “screen all directors”
- “clear case when evidence complete”

This is where learning candidates should be *reviewed* and then promoted into SemReg verb contracts if they represent durable domain behavior.

> Suggested discipline: **Agent learning adjusts discovery** (ranking, aliasing, phrase mapping).  
> **SemReg defines capability** (what verbs exist, their contracts, their governance).

---

# D) Runbooks & BPMN durable workflow — long-running orchestration without “async soup”

### The intent
This subsystem turns your deterministic DSL plan into a durable workflow:
- compile “plan steps” into a runbook
- optionally dispatch into a BPMN runtime
- park/resume execution on external signals (documents, human review, client portal actions)

### What exists (runbook layer)
- **`"ob-poc".staged_runbook`** — “in construction” runbook envelope
- **`"ob-poc".compiled_runbooks`**
  - `compiled_runbook_id`, `session_id`, `version`, `steps` (JSON), `envelope` (JSON), `canonical_hash`
- **`"ob-poc".compiled_runbook_events`** — audit trail of runbook status transitions

### What exists (BPMN integration layer)
- **`"ob-poc".bpmn_pending_dispatches`**
  - durable “to-be-dispatched” records: `process_key`, `verb_fqn`, `dsl_source`, `domain_payload`, `correlation_id`, `correlation_key`
- **`"ob-poc".bpmn_correlations`**
  - binds `correlation_id` ↔ `process_instance_id`, plus `runbook_id` + `entry_id` for traceability
- **`"ob-poc".bpmn_parked_tokens`**
  - parked execution tokens waiting on `expected_signal` with `correlation_key` (business correlation)
- **`"ob-poc".bpmn_job_frames`**
  - active job frames: `task_type`, `worker_id`, `attempts`, `status`

### How durable workflow maps back to the 3 main taxonomies

#### 1) Deal Map
Deal Map events often *spawn* onboarding work:
- a contract becomes `ACTIVE` → “create onboarding requests”
- a rate card becomes `AGREED` → “bind pricing to contracted products”

These are good candidates for a compiled runbook even if BPMN isn’t used (auditability + replay).

#### 2) Onboarding Request
This is where durable workflow earns its keep:
- provisioning tasks can be long-running and multi-stage
- blockers require human / client action
- completion is a set of converging signals (resources provisioned + proofs verified + approvals)

Mechanically:
- `deal_onboarding_requests` holds the operational status
- runbook/BPMN holds the execution token state
- correlation keys let external events resume progress deterministically

#### 3) KYC / UBO
KYC is inherently “park/resume”:
- `doc_requests` are created and then wait for uploads + review
- `screenings` can be queued and awaited
- tollgates (`tollgate_evaluations`) act as “policy checkpoints”

The typical integration pattern:
- runbook step creates/updates `doc_requests`
- BPMN parks a token on `expected_signal = document_received` with `correlation_key = <doc_request_id>`
- client portal upload creates/updates the document and resolves the parked token

### Why you have both runbooks *and* BPMN tables
This is a good separation:
- **Runbook** = deterministic plan + audit + replay (your DSL worldview)  
- **BPMN** = durable wait/resume semantics and external task correlation

You can keep BPMN usage “dense and contained” while preserving the platform’s determinism.

---

# E) Events & Feedback — observability that stays tied to the domain

### Generic event log
- **`events.log(event_type, payload, session_id, timestamp)`**  
This is a low-friction “append-only” log for domain and system events.

### Failure inspector
- **`feedback.failures`**, `feedback.occurrences`, `feedback.audit_log`  
This schema exists to keep “why did it fail?” separate from business tables, while still being reproducible.

### Mapping back to the 3 main taxonomies
- **Deal Map:** pricing/contract transitions, deal state changes, anomalies (missing rate card, invalid participant set)
- **Onboarding:** provisioning failures, HOL/blocker classification, idempotency conflicts
- **KYC:** evidence shortfalls, screening mismatches, UBO determination gaps, policy violations

---

# F) Mapping matrix: “where do I look?” (fast lookup)

| Cross-cutting subsystem | Deal Map | Onboarding Request | KYC / UBO |
|---|---|---|---|
| Documents | `deal_documents` + `document_catalog` | `cbu_evidence` + `document_catalog` | `doc_requests` + `ubo_evidence`/`kyc_ubo_evidence` + `document_catalog` |
| Evidence semantics | `document_types` + attribute links | `document_types` + evidence requirements | `document_types` + evidence requirements + tollgates |
| SemReg definitions | product/contract taxonomies, view defs | verb contracts, resource decompositions | evidence requirements, derivations, policies |
| Agent learning | product/entity naming + negotiation verbs | provisioning verbs + blocker language | KYC verbs + entity aliasing + proof wording |
| Runbooks/BPMN | (optional) commercial automations | **primary** durable orchestration | **primary** wait/resume for doc & screening loops |
| Events/Feedback | state transitions + commercial validation errors | provisioning failures + idempotency conflicts | evidence/policy failures + determination mismatches |

---

# G) Concrete join keys (useful when wiring “appendix systems” to aggregates)

### Documents & evidence
- `document_catalog.doc_id` — primary document identifier
- `deal_documents.document_id` — links deal ↔ document
- `doc_requests.document_id` — links request ↔ received document
- `cbu_evidence.document_id`, `ubo_evidence.document_id`, `kyc_ubo_evidence.document_id`

### SemReg
- `sem_reg.snapshot_sets.snapshot_set_id`
- `sem_reg.snapshots.snapshot_id`
- `sem_reg.outbox_events.outbox_seq` (watermark/projection driver)

### Runbooks/BPMN
- `compiled_runbooks.compiled_runbook_id`
- `bpmn_pending_dispatches.correlation_id` ↔ `bpmn_correlations.correlation_id`
- `bpmn_parked_tokens.correlation_key` (business key for resume)
- `bpmn_correlations.domain_correlation_key` (optional domain-level key)

### Agent learning
- `agent.events.session_id` (ties to a runbook session)
- `agent.entity_aliases.entity_id` (bind alias to canonical entity record)

---

## Optional clarifications (only if you want the appendix to be even tighter)
These are **not blockers**; they just let us make the “cross-cutting map” more deterministic:
