# GPT-51 Architecture Overview — OB-POC DSL (for Java/Spring)

## Scope
- Project: `Developer/ob-poc` (OB-POC KYC/UBO onboarding DSL).
- Sources reviewed: Rust DSL v2 (`rust/src/dsl_v2/*`, `rust/src/agentic/*`), YAML configs (`rust/config/verbs.yaml`, `rust/config/csg_rules.yaml`), data dictionary (`rust/src/data_dictionary/*`), schema dump (`schema_export.sql`), DSL specs (`docs/docs_KYC_UBO_DSL_SPEC.md`, `docs/DSL_ARCHITECTURE_REVIEW.md`).
- Focus: DSL-based state management, document/data-dictionary-backed attributes, agentic integration with deterministic validation (CSG + verb schema), and RAG hooks.

## System Components (Spring mental model)
- **DSL front door**: S-expression programs `(domain.verb :k v ...)` → parsed by `dsl_v2::parser` (Nom). Treat as Spring Web/Message controllers that only accept DSL payloads.
- **Config-driven verb registry**: `rust/config/verbs.yaml` (+ split `config/verbs/` support) defines verbs, arg types, CRUD/table mappings; `verb_registry` loads at runtime (analogous to Spring Bean registry + metadata-driven mappers).
- **Context-Sensitive Guardrails (CSG)**: `rust/config/csg_rules.yaml` + `dsl_v2::csg_linter` enforce business/state rules before execution (state machine + applicability). Think of it as a validation interceptor chain.
- **Execution layer**: `dsl_v2::execution_plan` compiles nested DSL into ordered steps; `generic_executor` (when DB feature on) performs CRUD per verb mapping; `custom_ops` plug-ins handle non-CRUD behaviors (screening, graph ops, etc.).
- **Data dictionary service**: `rust/src/data_dictionary/*` validates attribute IDs/values; maps semantic IDs to sources/sinks; supports RAG vectors. Database tables `"ob-poc".attribute_dictionary` and `"ob-poc".dictionary` hold canonical attribute definitions.
- **Agentic pipeline**: `rust/src/agentic/*` orchestrates intent extraction → pattern selection → requirement planning → DSL generation → parse/lint/validate loop, giving deterministic retries instead of free-form execution.

## DSL-as-State & Determinism
- Single grammar, data-driven: verbs + argument schemas live in YAML; grammar in `docs/dsl-grammar.ebnf` + runtime parser.
- State is materialized via executed DSL steps persisted to domain tables (CBU, entities, KYC cases, doc requests, observations, UBO graph). Replayable because the same DSL + config yields identical execution plans.
- Deterministic pipeline: parse → CSG lint → semantic validation (types/refs) → execution plan → CRUD/custom ops. Fail-fast diagnostics (with codes) ensure agents can self-correct.

## Shared Data Model (CBU-centric)
- **CBU anchor**: `cbu.*` verbs map to `"ob-poc".cbus` and role tables; CBU ID threads through KYC cases (`kyc.kase`), entity workstreams, UBO edges, document catalog, and SSI/custody tables.
- **Entity graph**: Entities (`entities`, typed tables) + `cbu_entity_roles` describe relationships; UBO edges (`ubo_relations`) and snapshots capture ownership/control per CBU.
- **Workstreams**: `kyc.case`, `entity_workstreams`, `doc_requests`, `red_flags`, `case_screenings` bind to the same `cbu_id` for consistent lifecycle and audit.

## Data Dictionary & Attributes
- Attribute IDs are first-class: verbs accept attributes via semantic IDs, validated by `DictionaryService`. Tables `attribute_dictionary` / `dictionary` store name, group, domain, mask/type, source, sink, and optional `vector` for semantic search.
- RAG-ready: `schema_export.sql` enables `vector` extension; `data_dictionary::validation` and vector support allow embedding-based retrieval to ground prompts but all outputs are still schema/CSG validated.
- Attribute usage points:
  - `observation.record` / `record-from-document` write values with provenance.
  - Allegations vs observations vs discrepancies let the system reconcile conflicting attribute values with source metadata.
  - Services/custody resources link to `dictionary_group` to derive required attributes per resource type.

## Document & Evidence Integration
- Document domain verbs (see `verbs.yaml` → `document.*`, `doc-request.*`, `observation.*`):
  - `document.catalog` registers a file against CBU/entity; `document.extract` / `extract-to-observations` produce structured observations bound to attribute IDs.
  - `doc-request.*` manages requirements, requests, receipts, verification/waiver, and acceptable types (`requirement_acceptable_docs`), all tied to `cbu_id`/workstream.
  - `observation.record` and `discrepancy.*` capture evidence vs claims; `resolve-conflict` creates golden record lineage.
- Lineage is explicit: document → extraction → observation rows with `source_document_id`/attribute IDs, ensuring traceable KYC/UBO decisions.

## Agentic Integration (Deterministic with RAG)
- **Restricted vocabulary**: Agents generate verbs only from `verbs.yaml`; parser/registry rejects unknown verbs/args.
- **CSG Linter**: Enforces state transitions and applicability (e.g., document type must match entity type, roles must align with client_type). Diagnostics are structured for automated retries.
- **Planner + patterns**: `agentic::planner` and `patterns` expand intents (e.g., onboarding with custody/markets) into full requirement sets before DSL generation—reduces hallucination and ensures completeness.
- **Feedback loop**: `agentic::validator` re-parses and lints LLM output; orchestrator retries with errors as feedback, guaranteeing only valid DSL is persisted/executed.
- **RAG**: Attribute dictionary vectors and prior DSL/state can be supplied to prompts, but execution remains deterministic via parse/lint/schema checks.

## Execution Pipeline (end-to-end)
1. Receive instruction/intent (API, CLI, or agent).
2. If agentic: extract intent → plan requirements → generate candidate DSL with dictionaries/verbs in context.
3. Parse DSL (Nom) → AST.
4. CSG lint (business/state rules) + semantic/attribute validation (dictionary lookup).
5. Compile to execution plan with dependency injection (child verbs can use parent bindings via `:as @sym`).
6. Execute: CRUD via `generic_executor` against PostgreSQL tables; custom ops for graph/rules/screening.
7. Persist state: CBUs/entities/workstreams/requests/observations/UBO edges; audit via versioned data and snapshots.

## Java/Spring Integration Notes
- Treat YAML verb definitions like Spring configuration: generate controllers/services that accept DSL payloads or map REST to DSL verbs; enforce the same parse+CSG+semantics pipeline before hitting persistence.
- Mirror key tables (`cbus`, `cbu_entity_roles`, `kyc.case`, `doc_requests`, `document_catalog`, `observations`, `ubo_relations`, `attribute_dictionary`/`dictionary`, `attribute_values`-style tables) via JPA; keep CBU as the aggregate root.
- Use pgvector for `dictionary.vector` and optionally `document_type_similarities` to ground Spring AI/LangChain4j retrieval, but always re-validate with the DSL engine.
- Map agent feedback loop to Spring orchestration: intent service → planner → LLM → DSL validator → executor; surface CSG diagnostics to clients for self-correction.

## Key References
- Grammar/specs: `docs/dsl-grammar.ebnf`, `docs/docs_KYC_UBO_DSL_SPEC.md`, `docs/DSL_ARCHITECTURE_REVIEW.md`
- Runtime code: `rust/src/dsl_v2/*`, `rust/src/agentic/*`, `rust/src/data_dictionary/*`, `rust/config/verbs.yaml`, `rust/config/csg_rules.yaml`
- Schema: `schema_export.sql` (schemas `"ob-poc"`, `kyc`, `custody`; vector + trigram extensions)
