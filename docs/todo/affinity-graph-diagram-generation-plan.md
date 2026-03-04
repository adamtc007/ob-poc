# AffinityGraph & Schema-Driven Diagram Generation (v2)

## Context

The revised spec (`sem-os-research-extension-review-v2.md`) replaces the original plan with a fundamentally different architecture. Instead of diagram generation from physical schema introspection enriched by DomainMetadata YAML, the v2 spec introduces the **AffinityGraph** — a pre-computed bidirectional index of verb↔data relationships built entirely from active registry snapshots. Diagrams become projections of this graph rather than standalone features.

**Why this matters:** The Semantic Registry already knows which verbs produce/consume which entities, which attributes come from which tables, and which verbs map to CRUD operations. The AffinityGraph makes this implicit knowledge queryable — enabling "what verbs touch the `kyc.cases` table?" and "what data does `cbu.create` read/write?" without any new database tables or migrations.

**Prerequisite status:** Step 0 work (DomainMetadata enrichment of `reads_from`/`writes_to` on VerbContractBody) is partially implemented in uncommitted changes. This work remains relevant — the enriched fields are data sources for AffinityGraph Pass 1.

---

## Deliverables

1. **AffinityGraph** — bidirectional index in `sem_os_core/src/affinity/` (types, 5-pass builder, 10 query methods)
2. **Diagram model + MermaidRenderer** — in `sem_os_core/src/diagram/` (model types, enrichment, Mermaid rendering)
3. **9 new verbs** — 6 affinity navigation (`registry.*`) + 3 diagram generation (`schema.*`) with CustomOp handlers
4. **CoreService integration** — `get_affinity_graph()` method, cached `Arc<RwLock<AffinityGraph>>`, outbox rebuild hook
5. **Step 0 completion** — finish + test DomainMetadata enrichment (seed bundle populates reads_from/writes_to)
6. **Tests** — 30+ unit tests (builder, queries, renderer) + integration tests

---

## Architecture

```
Registry Snapshots (active)
    │
    ├─ VerbContracts ──── Pass 1 ──► Forward edges (verb→data)
    ├─ EntityTypeDefs ─── Pass 2 ──► Entity↔table bimaps
    ├─ AttributeDefs ──── Pass 3 ──► Reverse edges (data→verb)
    ├─ DerivationSpecs ── Pass 4 ──► Lineage edges
    └─ RelationshipTypeDefs Pass 5 ► Entity↔entity edges
                                          │
                                          ▼
                                   AffinityGraph
                                   (in-memory, cached)
                                          │
                          ┌───────────────┼───────────────┐
                          ▼               ▼               ▼
                   Query Surface    Diagram Projection  Discovery
                   (10 methods)     (ERD, VerbFlow)     (Phase 2)
```

---

## Module Placement

**AffinityGraph core:** `rust/crates/sem_os_core/src/affinity/` — pure value types + builder + queries. No DB dependency (port-trait isolated). Uses existing `SnapshotStore` + `ObjectStore` port traits.

**Diagram model + renderers:** `rust/crates/sem_os_core/src/diagram/` — DiagramModel types, enrichment merge, MermaidRenderer. Pure Rust, no DB.

**CustomOp handlers:** `rust/src/domain_ops/affinity_ops.rs` (6 ops) + additions to `rust/src/domain_ops/sem_reg_schema_ops.rs` (3 ops). These live in the monolith because they need `PgPool` + `ExecutionContext`.

**Verb YAML:** Appended to `rust/config/verbs/sem-reg/registry.yaml` (6 affinity verbs) and `rust/config/verbs/sem-reg/schema.yaml` (3 diagram verbs).

```
sem_os_core/src/
├── affinity/           # NEW
│   ├── mod.rs          # AffinityGraph struct + build() + re-exports
│   ├── types.rs        # AffinityEdge, DataRef, TableRef, ColumnRef, AffinityKind, etc.
│   ├── builder.rs      # 5-pass builder from snapshots
│   └── query.rs        # 10 query methods (verbs_for_*, data_for_*, governance)
│
├── diagram/            # NEW
│   ├── mod.rs          # Re-exports
│   ├── model.rs        # DiagramModel, DiagramEntity, DiagramAttribute, DiagramRelationship
│   ├── enrichment.rs   # merge(physical_tables, affinity_graph, entity_defs) → DiagramModel
│   └── mermaid.rs      # MermaidRenderer (erDiagram + graph LR + graph TD)
│
rust/src/domain_ops/
├── affinity_ops.rs     # NEW — 6 CustomOp handlers for registry.* verbs
└── sem_reg_schema_ops.rs  # MODIFIED — add 3 diagram CustomOp handlers
```

---

## Step 0: Complete Seed Bundle Enrichment (existing uncommitted work)

**Status:** Partially implemented. Functions written but never compiled/tested.

**Files to finish:**
- `rust/crates/sem_os_obpoc_adapter/src/metadata.rs` — DomainMetadata types + YAML loader (exists, untracked)
- `rust/crates/sem_os_obpoc_adapter/src/scanner.rs` — `enrich_verb_contracts()` + `enrich_entity_types()` (exists, modified)
- `rust/crates/sem_os_obpoc_adapter/src/lib.rs` — `build_seed_bundle_with_metadata()` (exists, modified)

**Work remaining:**
1. Add `pub mod metadata;` to adapter's `lib.rs` if not present
2. Ensure `domain_metadata.yaml` path is configurable (not hardcoded)
3. Compile and fix any errors
4. Run existing 5 unit tests + add any missing coverage
5. Run `cargo test -p sem_os_obpoc_adapter`

**Why this matters for v2:** The enriched `reads_from`/`writes_to` on VerbContractBody and `read_by_verbs`/`written_by_verbs` on EntityTypeDefBody are consumed by AffinityGraph Pass 1 and Pass 2 as supplemental edge sources.

---

## Step 1: AffinityGraph Types (`sem_os_core/src/affinity/types.rs`)

All types: `Debug, Clone, Serialize, Deserialize`. `DataRef`, `TableRef`, `ColumnRef`: additionally `Hash, Eq, PartialEq`.

| Type | Fields | Purpose |
|------|--------|---------|
| `AffinityGraph` | `edges`, `verb_to_data` (fwd index), `data_to_verb` (rev index), `entity_to_table`, `table_to_entity`, `attribute_to_column`, `derivation_edges` | Top-level bidirectional index |
| `AffinityEdge` | `verb_fqn`, `data_ref`, `affinity_kind`, `provenance` | Single verb↔data relationship |
| `AffinityKind` | `Produces`, `Consumes`, `CrudRead`, `CrudInsert`, `CrudUpdate`, `CrudDelete`, `ArgLookup{arg_name}`, `ProducesAttribute`, `ConsumesAttribute{arg_name}` | Edge classification |
| `AffinityProvenance` | `VerbProduces`, `VerbConsumes`, `VerbCrudMapping`, `VerbArgLookup`, `AttributeSource`, `AttributeSink`, `DerivationSpec` | Where edge was derived from |
| `DataRef` | enum: `Table(TableRef)`, `Column(ColumnRef)`, `EntityType(String)`, `Attribute(String)` | Unified data asset reference |
| `TableRef` | `schema: String`, `table: String` | Physical table reference |
| `ColumnRef` | `schema: String`, `table: String`, `column: String` | Physical column reference |
| `DerivationEdge` | `from_attribute: String`, `to_attribute: String`, `spec_fqn: String` | Attribute lineage |
| `VerbAffinity` | `verb_fqn`, `affinity_kind`, `provenance` | Query result: verb with edge context |
| `DataAffinity` | `data_ref`, `affinity_kind`, `provenance` | Query result: data with edge context |
| `DataFootprint` | `tables`, `columns`, `attributes`, `entity_types` | Transitive data reach of a verb |

**Tests:** Serde round-trip for all types, `DataRef` hash/eq correctness.

---

## Step 2: AffinityGraph Builder (`sem_os_core/src/affinity/builder.rs`)

Entry point: `AffinityGraph::build(snapshots: &[SnapshotRow]) -> Result<Self>`

Takes a flat list of active `SnapshotRow` (all 13 object types), deserializes the relevant body types, and runs 5 passes:

### Pass 1: VerbContracts → Forward Edges
For each `VerbContractBody`:
- `produces.entity_type` → `AffinityEdge { kind: Produces, data_ref: EntityType(fqn) }`
- `consumes[]` → `AffinityEdge { kind: Consumes, data_ref: EntityType(fqn) }`
- `crud_mapping` → match operation to `CrudRead|Insert|Update|Delete`, `data_ref: Table(schema, table)`
- `args[].lookup` → `AffinityEdge { kind: ArgLookup{arg_name}, data_ref: Table(schema, table) }`
- `reads_from[]` → `AffinityEdge { kind: CrudRead, data_ref: Table("", table) }` (supplemental)
- `writes_to[]` → `AffinityEdge { kind: CrudInsert, data_ref: Table("", table) }` (supplemental)

### Pass 2: EntityTypeDefs → Entity↔Table Bimaps
For each `EntityTypeDefBody`:
- `db_table` → insert into `entity_to_table` and `table_to_entity`
- Record `required_attributes` and `optional_attributes` for later attribute matching

### Pass 3: AttributeDefs → Reverse Edges
For each `AttributeDefBody`:
- `source.producing_verb` → `AffinityEdge { kind: ProducesAttribute, verb_fqn: producing_verb, data_ref: Attribute(fqn) }`
- `source.{schema, table, column}` → insert into `attribute_to_column`
- `sinks[]` → `AffinityEdge { kind: ConsumesAttribute{arg_name}, verb_fqn: consuming_verb, data_ref: Attribute(fqn) }`

### Pass 4: DerivationSpecs → Lineage Edges
For each `DerivationSpecBody`:
- For each `input.attribute_fqn` → `DerivationEdge { from: input_fqn, to: output_attribute_fqn, spec_fqn }`

### Pass 5: RelationshipTypeDefs → Entity↔Entity
For each `RelationshipTypeDefBody`:
- Record `source_entity_type_fqn ↔ target_entity_type_fqn` with cardinality + edge_class
- (Used by diagram enrichment for relationship rendering, not stored as AffinityEdges)

After all passes: build `verb_to_data` and `data_to_verb` indexes from `edges` vec.

**Tests (8+):**
- `test_build_empty_snapshots` — no panic on empty input
- `test_pass1_verb_produces` — VerbContractBody with produces → Produces edge
- `test_pass1_crud_mapping` — crud_mapping.operation → correct AffinityKind
- `test_pass1_arg_lookup` — args[].lookup → ArgLookup edge
- `test_pass2_entity_table_bimap` — entity↔table bidirectional lookup
- `test_pass3_attribute_source_sink` — ProducesAttribute and ConsumesAttribute edges
- `test_pass4_derivation_lineage` — DerivationEdge creation
- `test_bidirectional_index` — verb_to_data and data_to_verb consistency

---

## Step 3: AffinityGraph Queries (`sem_os_core/src/affinity/query.rs`)

### 6 Core Queries

```rust
impl AffinityGraph {
    /// Find all verbs that read/write/produce/consume a given table.
    pub fn verbs_for_table(&self, schema: &str, table: &str) -> Vec<VerbAffinity>;

    /// Find all verbs that produce or consume a given attribute.
    pub fn verbs_for_attribute(&self, attr_fqn: &str) -> Vec<VerbAffinity>;

    /// Find all verbs operating on a given entity type (via entity→table→verbs).
    pub fn verbs_for_entity_type(&self, entity_fqn: &str) -> Vec<VerbAffinity>;

    /// Find all data assets a verb touches (tables, columns, attributes, entities).
    pub fn data_for_verb(&self, verb_fqn: &str) -> Vec<DataAffinity>;

    /// Transitive data footprint: follow arg lookups and derivations up to `depth` hops.
    pub fn data_footprint(&self, verb_fqn: &str, depth: u32) -> DataFootprint;

    /// Find verbs sharing data dependencies (same table/attribute).
    pub fn adjacent_verbs(&self, verb_fqn: &str) -> Vec<(String, Vec<DataRef>)>;
}
```

### 4 Governance Queries

```rust
impl AffinityGraph {
    /// Tables with no verb affinity (ungoverned data).
    pub fn orphan_tables(&self, known_tables: &[TableRef]) -> Vec<TableRef>;

    /// Verbs with no data affinity (disconnected operations).
    pub fn orphan_verbs(&self) -> Vec<String>;

    /// Attributes with source but no sinks (written but never read).
    pub fn write_only_attributes(&self) -> Vec<String>;

    /// Attributes with sinks but no source (read before written).
    pub fn read_before_write_attributes(&self) -> Vec<String>;
}
```

**Note:** `orphan_tables` takes `known_tables` as parameter — the caller provides the physical schema tables from `extract_schema()`. The AffinityGraph itself doesn't know all physical tables.

**Tests (10+):**
- `test_verbs_for_table_finds_crud` — CRUD verb found via table ref
- `test_verbs_for_table_finds_lookup` — ArgLookup verb found via table ref
- `test_verbs_for_attribute_both_directions` — producing and consuming verbs found
- `test_verbs_for_entity_type_via_table` — entity→table→verbs transitive lookup
- `test_data_for_verb` — returns all data assets for a verb
- `test_data_footprint_depth_1` — no transitivity
- `test_data_footprint_depth_2` — follows arg lookups one level
- `test_adjacent_verbs_shared_table` — two verbs sharing a table
- `test_orphan_tables` — table with no verb affinity detected
- `test_write_only_attributes` — attribute with source but no sinks

---

## Step 4: Diagram Model + MermaidRenderer (`sem_os_core/src/diagram/`)

### Model Types (`model.rs`)

| Type | Purpose |
|------|---------|
| `DiagramModel` | Top-level: entities + relationships + verb_surfaces + metadata |
| `DiagramEntity` | Table/entity with columns, verb surface, governance level |
| `DiagramAttribute` | Column with FK/PK flags, producing/consuming verbs |
| `DiagramRelationship` | FK/entity-relationship with cardinality + triggering verb |
| `GovernanceLevel` | `Full` (entity type + verbs) / `Partial` (some verbs) / `None` |
| `RenderOptions` | `schema_filter`, `domain_filter`, `include_columns`, `show_governance`, `max_tables`, `format` |

### Enrichment (`enrichment.rs`)

```rust
pub fn build_diagram_model(
    tables: &[TableExtract],       // From extract_schema()
    graph: &AffinityGraph,         // From registry snapshots
    options: &RenderOptions,
) -> DiagramModel
```

Pipeline:
1. For each `TableExtract` → resolve entity type via `graph.table_to_entity`
2. Get verb surface via `graph.verbs_for_table()`
3. Annotate columns with attribute FQNs via `graph.attribute_to_column`
4. Classify `GovernanceLevel` based on entity type match + verb count
5. Build relationships from FK data + `RelationshipTypeDef` data
6. Apply filters from `RenderOptions` (domain, schema, max_tables)
7. Sort deterministically (by entity name) for stable output

### MermaidRenderer (`mermaid.rs`)

Three render modes:

1. **`render_erd(model, options) -> String`** — `erDiagram` syntax
   - Entity blocks with columns (PK/FK annotated)
   - Relationships with cardinality
   - Verb surface as comments or entity labels
   - Governance tier color coding

2. **`render_verb_flow(model, verb_fqn, depth) -> String`** — `graph LR` syntax
   - Center verb node with data asset nodes
   - Edges labeled with AffinityKind
   - Adjacent verbs shown at depth > 1

3. **`render_domain_map(model, options) -> String`** — `graph TD` syntax
   - Domains as subgraphs
   - Tables as nodes (with verb count badges)
   - Cross-domain FK edges

**Key detail:** Sanitize hyphenated names (`ob-poc` → `ob_poc`) for Mermaid node IDs. Sort entities and relationships before rendering for deterministic output.

**Tests (12+):**
- `test_enrichment_registered_entity` — entity type matched, governance=Full
- `test_enrichment_unregistered_table` — no entity type, governance=None
- `test_enrichment_verb_surface_populated` — verbs appear on entity
- `test_mermaid_erd_basic` — valid erDiagram syntax with entities + relationships
- `test_mermaid_erd_with_verbs` — verb surface annotations present
- `test_mermaid_verb_flow` — graph LR with verb→data edges
- `test_mermaid_domain_map` — graph TD with domain subgraphs
- `test_mermaid_sanitize_names` — hyphens replaced in node IDs
- `test_mermaid_deterministic_output` — same input → same output
- `test_render_options_filter` — domain/schema/max_tables filters applied
- `test_empty_model` — no panic on empty input
- `test_governance_level_classification` — correct Full/Partial/None

---

## Step 5: CoreService Integration

### New method on `CoreService` trait

```rust
// In sem_os_core/src/service.rs
pub trait CoreService: Send + Sync {
    // ... existing methods ...
    async fn get_affinity_graph(&self) -> Result<Arc<AffinityGraph>>;
}
```

### Cache in CoreServiceImpl

```rust
pub struct CoreServiceImpl {
    // ... existing fields ...
    affinity_graph: Arc<RwLock<Option<AffinityGraph>>>,
}
```

**Lifecycle:**
- On first call to `get_affinity_graph()`: build from active snapshots if cache is empty
- On publish cycle (outbox projection): rebuild and replace cache
- Thread-safe via `Arc<RwLock<>>`

**Build trigger:** After `bootstrap_seed_bundle()` completes AND after any changeset publish. In `CoreServiceImpl::bootstrap_seed_bundle()`, after publishing snapshots, call `self.rebuild_affinity_graph().await?`.

**Graceful degradation:** If no snapshots exist yet, return an empty `AffinityGraph`. Never fail the request.

### SemOsClient trait extension

```rust
// In sem_os_client/src/lib.rs
pub trait SemOsClient: Send + Sync {
    // ... existing methods ...
    async fn get_affinity_graph(&self) -> Result<Arc<AffinityGraph>>;
}
```

`InProcessClient` delegates to `CoreService`. `HttpClient` can serialize/deserialize the graph if needed (Phase 3 concern — for now, graph is only available in-process).

---

## Step 6: CustomOp Handlers + Verb YAML

### 6 Affinity Ops (`rust/src/domain_ops/affinity_ops.rs`)

All follow `#[register_custom_op]` pattern. All return typed result structs (no raw `serde_json::json!`).

| Op | Domain | Verb | Key Logic |
|----|--------|------|-----------|
| `AffinityVerbsForTableOp` | `registry` | `verbs-for-table` | `graph.verbs_for_table(schema, table)` |
| `AffinityVerbsForAttributeOp` | `registry` | `verbs-for-attribute` | `graph.verbs_for_attribute(attr_fqn)` |
| `AffinityDataForVerbOp` | `registry` | `data-for-verb` | `graph.data_for_verb(verb_fqn)` or `data_footprint(verb_fqn, depth)` |
| `AffinityAdjacentVerbsOp` | `registry` | `adjacent-verbs` | `graph.adjacent_verbs(verb_fqn)` |
| `AffinityGovernanceGapsOp` | `registry` | `governance-gaps` | `orphan_tables()` + `orphan_verbs()` + `write_only_attributes()` |
| `AffinityDiscoverDslOp` | `registry` | `discover-dsl` | Phase 2 — stub initially, returns NotImplemented |

**Access pattern:** Each op gets the AffinityGraph via `ctx` (the `ExecutionContext` will need access to `CoreService` or a cached `Arc<AffinityGraph>`). The specific wiring depends on how `ExecutionContext` currently provides access to services — follow existing patterns like `sem_reg_schema_ops.rs`.

### 3 Diagram Ops (added to `rust/src/domain_ops/sem_reg_schema_ops.rs`)

| Op | Domain | Verb | Key Logic |
|----|--------|------|-----------|
| `SchemaGenerateErdOp` | `schema` | `generate-erd` | `extract_schema()` + `build_diagram_model()` + `MermaidRenderer.render_erd()` |
| `SchemaGenerateVerbFlowOp` | `schema` | `generate-verb-flow` | `graph.data_for_verb()` + `MermaidRenderer.render_verb_flow()` |
| `SchemaGenerateDiscoveryMapOp` | `schema` | `generate-discovery-map` | Phase 2 — stub initially |

### Verb YAML (9 definitions)

Append to `rust/config/verbs/sem-reg/registry.yaml`:

```yaml
# 6 affinity navigation verbs
verbs-for-table:
  description: "Find all verbs that read, write, or reference a database table"
  behavior: plugin
  metadata: { tier: intent, source_of_truth: registry, phase_tags: [stewardship, data] }
  invocation_phrases: ["what verbs use this table", "which operations touch this table", ...]
  args:
    - name: schema-name
      type: string
      required: true
    - name: table-name
      type: string
      required: true
    - name: include-lookups
      type: boolean
      required: false
      default: true
  returns: { type: record_set }

# ... similar for verbs-for-attribute, data-for-verb, adjacent-verbs, governance-gaps, discover-dsl
```

Append to `rust/config/verbs/sem-reg/schema.yaml`:

```yaml
# 3 diagram generation verbs (generate-erd may already exist — update if so)
generate-erd:
  description: "Generate an entity-relationship diagram with verb surface annotations"
  behavior: plugin
  metadata: { tier: intent, side_effects: facts_only, phase_tags: [stewardship] }
  invocation_phrases: ["generate ERD", "show schema diagram", "draw entity relationships", ...]
  args:
    - name: schema-name
      type: string
      required: true
    - name: format
      type: string
      required: false
      default: "mermaid"
      valid_values: [mermaid, dot]
    - name: show-verb-surface
      type: boolean
      required: false
      default: true
    - name: domain
      type: string
      required: false
    - name: max-tables
      type: integer
      required: false
      default: 50
  returns: { type: record }

# ... similar for generate-verb-flow, generate-discovery-map
```

Each verb needs 8+ invocation phrases for semantic discovery.

---

## Step 7: Wire Module Declarations + Run Pre-Commit

1. Add `pub mod affinity;` and `pub mod diagram;` to `sem_os_core/src/lib.rs`
2. Add `pub mod affinity_ops;` to `rust/src/domain_ops/mod.rs`
3. Register module in `rust/src/lib.rs` if needed
4. Run `cargo x pre-commit` — fix clippy/format issues
5. Verify `cargo test -p sem_os_core` passes
6. Verify `cargo test --lib -- test_plugin_verb_coverage` passes (all 9 plugin verbs have ops)

---

## Step 8: Integration Tests

**File:** `rust/tests/affinity_integration.rs` (all `#[cfg(feature = "database")]` + `#[ignore]`)

1. `test_affinity_graph_from_live_registry` — bootstrap seed bundle → build AffinityGraph → verify edges exist
2. `test_verbs_for_table_cbus` — query `verbs_for_table("ob-poc", "cbus")` → expect `cbu.create`, `cbu.list`, etc.
3. `test_data_for_verb_cbu_create` — query `data_for_verb("cbu.create")` → expect `Table("ob-poc", "cbus")`
4. `test_adjacent_verbs` — `cbu.create` and `cbu.list` share `cbus` table
5. `test_governance_gaps` — identify tables with no verb affinity
6. `test_diagram_erd_generation` — full pipeline: extract_schema → build_diagram_model → render_erd → valid Mermaid

---

## Step 9: Run populate_embeddings

After adding 9 verb YAML definitions with invocation phrases:

```bash
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings
```

---

## Implementation Order

| Step | What | Files | Est. |
|------|------|-------|------|
| 0 | Finish seed bundle enrichment (compile + test) | adapter: lib.rs, scanner.rs, metadata.rs | 30m |
| 1 | AffinityGraph types | `sem_os_core/src/affinity/types.rs` + `mod.rs` | 30m |
| 2 | AffinityGraph builder (5 passes) | `sem_os_core/src/affinity/builder.rs` | 1h |
| 3 | AffinityGraph queries (10 methods) | `sem_os_core/src/affinity/query.rs` | 45m |
| 4 | DiagramModel + MermaidRenderer | `sem_os_core/src/diagram/` (4 files) | 1.5h |
| 5 | CoreService integration | `service.rs`, `lib.rs` (both crates) | 30m |
| 6 | CustomOps + verb YAML (9 verbs) | `affinity_ops.rs`, `schema_ops.rs`, YAML | 1.5h |
| 7 | Wire modules + pre-commit | `lib.rs` files, cargo x pre-commit | 30m |
| 8 | Integration tests | `affinity_integration.rs` | 45m |
| 9 | Run populate_embeddings | CLI | 5m |

---

## Verification

```bash
# Step 0: Seed bundle enrichment tests
cargo test -p sem_os_obpoc_adapter

# Steps 1-4: Core unit tests (no DB)
cargo test -p sem_os_core -- affinity
cargo test -p sem_os_core -- diagram

# Step 7: Pre-commit (format + clippy + unit tests)
cargo x pre-commit

# Step 6: Plugin verb coverage
cargo test --lib -- test_plugin_verb_coverage

# Step 8: Integration tests (requires DB)
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test affinity_integration -- --ignored --nocapture

# Step 9: Populate embeddings
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings
```

---

## Risk Mitigations

| Risk | Mitigation |
|------|------------|
| Large snapshot batch load | AffinityGraph::build() processes all types in single pass — no repeated DB queries |
| No active snapshots yet | Graceful: empty AffinityGraph returned, queries return empty results |
| Hyphenated names break Mermaid | Sanitize all node IDs (replace non-alphanumeric with `_`) |
| Diagram too large (200+ tables) | `RenderOptions.max_tables` + `domain_filter` for scoping |
| CustomOps need AffinityGraph access | Thread `Arc<AffinityGraph>` via ExecutionContext or AgentState — follow existing pattern |
| Step 0 uncommitted changes may conflict | Compile and test Step 0 first before proceeding |

---

## Deferred to Phase 2+

- `discover-dsl` verb (utterance → verb chain synthesis) — stub in Phase 1
- `generate-discovery-map` verb — stub in Phase 1
- DOT/PlantUML/D2 renderers — Mermaid only in Phase 1
- SchemaConnector trait abstraction (multi-dialect support) — Phase 3
- Multi-dialect connectors (MySQL, Oracle, SQL Server) — Phase 4
- AffinityGraph rebuild on outbox projection cycle — can use on-demand build initially
