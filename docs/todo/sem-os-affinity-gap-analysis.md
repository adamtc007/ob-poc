# Sem OS Gap Analysis: Verb↔Data Affinity Extension

**Review Context:** Analyzing the current state of `ob-poc` against the `sem-os-research-extension-review-v2.md` architecture document.

---

## 1. What Has Been Implemented (Phase 1 Complete)
The foundational components from Phase 1 are actively working in the codebase:
- **Affinity Graph Engine:** Core types and builder (`sem_os_core::affinity::types`, `sem_os_core::affinity::builder`) successfully map bidirectional relationships across verbs, entities, attributes, and data stores from active registry snapshots.
- **Data Exploration Verbs:** The navigation ops (`AffinityVerbsForTableOp`, `AffinityDataForVerbOp`, `AffinityAdjacentVerbsOp`, etc.) are wired in `rust/src/domain_ops/affinity_ops.rs` and bound via `registry.yaml`.
- **Governance Gaps:** `registry.governance-gaps` is successfully detecting orphan tables/verbs and identifying governance dead zones.
- **Diagram Projections:** The diagram engine (`sem_os_core::diagram::model`, `sem_os_core::diagram::mermaid`) successfully merges physical schemas and semantic verb surfaces to power `schema.generate-erd` and `schema.generate-verb-flow`.

---

## 2. Gap Analysis (What Remains to be Built)

The architecture plan defines four phases. With Phase 1 complete, here are the low-level tasks and missing pieces broken down by the remaining phases:

### Phase 2: Discovery Pipeline
Currently, `registry.discover-dsl` and `schema.generate-discovery-map` are returning "Phase 2 stub / Not yet implemented" messages. This is the most critical missing functionality.

**Low-Level Tasks (`plan.md` candidates):**
- [ ] **Utterance → Intent Matching:** Implement vector/embedding or keyword-based matching to parse user `utterance` against `VerbContractBody.invocation_phrases`.
- [ ] **Verb Chain Synthesis:** Build the logic in `discover-dsl` to map a matched intent to its primary verb, traverse the `AffinityGraph` using `data_for_verb()` and `adjacent_verbs()`, and deduce the required order of operations (e.g. `entity.create` must precede `cbu-role.assign`).
- [ ] **Disambiguation Engine:** When `discover-dsl` hits multiple viable paths or missing arguments (e.g. `args[].lookup` requirements), it needs to construct missing context questions (Disambiguation Prompts).
- [ ] **CCIR Integration:** If a `subject-id` is provided in the `discover-dsl` call, seamlessly pipe the candidate verbs through the existing Context Resolution (CCIR) pipeline to prune invalid paths via ABAC/Policy evaluations.
- [ ] **Discovery Map Diagram:** Implement the `SchemaGenerateDiscoveryMapOp` to visually project the "Utterance → Intent Cluster → Verbs → Data Footprint" flow (mocked as returning a Phase 2 message in `sem_reg_schema_ops.rs`).

### Phase 3: Schema Connector + "Any Schema"
The existing schema extraction logic is heavily hardcoded to the local `ob-poc` PostgreSQL database structure via `PgPool` queries.

**Low-Level Tasks (`plan.md` candidates):**
- [ ] **`SchemaConnector` Trait Refactoring:** Extract the hardcoded Postgres SQL logic from `rust/crates/sem_os_core/src/schema_extract.rs` into a unified `SchemaConnector` trait.
- [ ] **`schema.connect` Verb Implementation:** Build a new operational verb to allow dynamic, runtime database connection configurations instead of reading static env vars at startup.
- [ ] **Connection Registry YAML:** Add state management for dynamic schemas (possibly storing active connection profiles in a config block).
- [ ] **Dynamic Target Resolution:** Remove `DEFAULT_SCHEMAS` dependencies so the `schema_extract` logic dynamically queries what's configured through the connector trait.

### Phase 4: Multi-Dialect + Self-Service Governance
Once Phase 3 abstracts the SQL interface, the system should branch out to other enterprise environments.

**Low-Level Tasks (`plan.md` candidates):**
- [ ] **Multi-Dialect Connector Implementations:** Add `SchemaConnector` implementations for MySQL, Oracle, and SQL Server.
- [ ] **`schema.scan-and-register` Composite Verb:** Create an automated onboarding verb that runs schema extraction on a new connection and automatically scaffolds starting `EntityTypeDef` and `VerbContract` YAML files.
- [ ] **Enterprise Export Formats:** Expand the `sem_os_core::diagram::render` implementations beyond `mermaid` and `dot` to directly export/sync to Confluence or SharePoint formats.

---

## 3. Next Steps Recommendation
We should immediately focus on **Phase 2: Discovery Pipeline**, specifically tackling the `discover-dsl` utterance-to-intent engine, as it unlocks the "Utterance → Action" capability that heavily differentiates Semantic OS.

If you approve this analysis, I can generate the granular `plan.md` for Phase 2 and begin autonomous implementation.