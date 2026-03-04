# Phase 2 Implementation Plan: Discovery Pipeline

**Goal:** Implement the "Utterance → Intent → DSL Discovery" pipeline to replace the Phase 2 stubs for `registry.discover-dsl` and `schema.generate-discovery-map`. This bridges the gap between natural language user intents and actionable, safe DSL operations.

---

## Step 1: Define Core Discovery Types
**Target Location:** `rust/crates/sem_os_core/src/affinity/discovery.rs` (create new module or add to `types.rs`)

Define the data structures representing the output of the discovery process. These types map directly to the JSON response expected by the `discover-dsl` verb.

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct DiscoveryResponse {
    pub intent_matches: Vec<IntentMatch>,
    pub suggested_sequence: Vec<VerbChainSuggestion>,
    pub disambiguation_needed: Vec<DisambiguationPrompt>,
    pub governance_context: GovernanceContext,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IntentMatch {
    pub verb: String,
    pub score: f32,
    pub matched_phrase: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerbChainSuggestion {
    pub verb: String,
    pub rationale: String,
    pub args: HashMap<String, String>,
    pub data_footprint: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DisambiguationPrompt {
    pub question: String,
    pub lookup: Option<String>,
    pub options: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GovernanceContext {
    pub all_tables_governed: bool,
    pub required_mode: String,
    pub policy_check: Option<String>,
}
```

## Step 2: Implement Utterance → Intent Matching
**Target Location:** `rust/crates/sem_os_core/src/affinity/discovery.rs`

Implement the logic to map a natural language string (`utterance`) to the closest `VerbContract`s using their defined `invocation_phrases`.
- **Task:** Create `fn match_intent(utterance: &str, graph: &AffinityGraph, registry: &Registry) -> Vec<IntentMatch>`.
- **Approach:** Initially implement a lightweight fuzzy-matching/TF-IDF scoring mechanism against all `invocation_phrases` in active verb contracts. (If vector embeddings are already available in `ob-poc` via candle/pgvector, integrate the search query there).
- **Result:** Yields a ranked list of candidate verbs based on the user's utterance.

## Step 3: Implement Verb Chain Synthesis
**Target Location:** `rust/crates/sem_os_core/src/affinity/discovery.rs`

For the top candidate verbs matched in Step 2, traverse the `AffinityGraph` to build a complete DSL sequence.
- **Task:** Create `fn synthesize_chain(primary_verb: &str, graph: &AffinityGraph) -> Vec<VerbChainSuggestion>`.
- **Approach:** 
  - Call `graph.data_for_verb(primary_verb)` to see what tables/entities are needed.
  - Call `graph.adjacent_verbs(primary_verb)` to find prerequisite verbs (e.g., if `cbu-role.assign` requires an `entity-id`, find verbs that output `entity-id` like `entity.create`).
  - Order the verbs logically (prerequisites first).
  - Annotate each step with its expected data footprint.

## Step 4: Implement Disambiguation & CCIR Integration
**Target Location:** `rust/crates/sem_os_core/src/affinity/discovery.rs`

Handle ambiguous intent and missing context.
- **Task:** Create `fn generate_disambiguation(verb: &VerbContractBody, subject_id: Option<Uuid>) -> Vec<DisambiguationPrompt>`.
- **Approach:** 
  - Iterate through the required `args` of the primary verb.
  - If an arg requires a `lookup` (e.g., to the `entities` table) and wasn't provided, formulate a prompt.
  - **Integration:** If `subject_id` is provided in the initial request, pass it to the existing `ccir` (Context Resolution) pipeline to filter out invalid verb paths using ABAC and policy verdicts.

## Step 5: Wire up `AffinityDiscoverDslOp`
**Target Location:** `rust/src/domain_ops/affinity_ops.rs`

Replace the hardcoded "Phase 2 stub" response with the actual discovery engine.
- **Task:** Update `AffinityDiscoverDslOp::execute`.
- **Implementation:** 
  1. Extract `utterance`, `subject-id`, and `max-chain-length` from `dsl_args`.
  2. Call `CoreService::get_affinity_graph()`.
  3. Execute `match_intent` -> `synthesize_chain` -> `generate_disambiguation`.
  4. Pack the results into the `DiscoveryResponse` and return it as a structured DSL `Record`.

## Step 6: Implement `SchemaGenerateDiscoveryMapOp`
**Target Location:** `rust/src/domain_ops/sem_reg_schema_ops.rs` and `sem_os_core/src/diagram/mermaid.rs`

Build the visual projection of the discovery pipeline.
- **Task:** Replace the stub in `SchemaGenerateDiscoveryMapOp::execute`.
- **Implementation:**
  - Introduce a new Mermaid rendering function specifically for the Discovery Map format.
  - The map should visually trace an intent node to its matched verbs, and from those verbs to their data footprints, as described in Section 4.3 of the architecture document.
  - Return the generated markdown text in the standard DSL diagram response format.

---

### Verification
- Add integration tests verifying that `registry.discover-dsl` successfully takes an utterance like "set up depositary" and returns `cbu-role.assign` as the primary intent, alongside `entity.create` as a prerequisite.
- Ensure no regressions in existing `AffinityGraph` capabilities.