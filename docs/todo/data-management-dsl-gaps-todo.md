# Data Management DSL Gap Fixes — Supplemental Claude Code TODO

**Reference:** `docs/UTTERANCE_PIPELINE_ANALYSIS_AND_ARCHITECTURE_v0.3.md` (RC-6, Part II §Observation Plane)  
**Context:** The Sage/Coder architecture requires the data management (Structure plane) to support a complete research → design → create → govern workflow. The DSL plumbing exists end-to-end BUT two verbs are missing and the Sage needs body type schemas for LLM-assisted definition authoring.  
**Dependency:** Can run IN PARALLEL with Phase 1 (Sage Skeleton) and Phase 2 (Coder Rewrite). No blocking dependency. These verbs exist independently of the Sage — they're useful from MCP and direct DSL too.

---

## Execution Protocol

- Each phase MUST complete fully before the GATE check.
- At each GATE: run the specified command. If it fails, fix before proceeding.
- At each GATE: print `DATAMGMT PHASE N COMPLETE — N% done` and the E-invariant.
- → IMMEDIATELY proceed to the next phase after a passing GATE.
- Do NOT stop between phases. Do NOT ask for confirmation.

**E-invariant (must hold at every GATE):**
`cargo check -p ob-poc 2>&1 | tail -1` shows no errors AND
`cargo test --lib -p ob-poc 2>&1 | tail -1` shows `test result: ok`.

---

## Phase 1: `registry.object-schema` Verb (35%)

**Goal:** A read-only verb that returns the JSON Schema for any registry object type (`attribute_def`, `entity_type_def`, `verb_contract`, `view_def`, `policy_rule`, `taxonomy_def`, `relationship_type_def`, `derivation_spec`, `observation_def`, `membership`, `document_type_def`). This tells the Sage (or any LLM) exactly what fields a definition body must contain, with types, required/optional, and descriptions.

### 1A: Verb YAML definition

**File:** `rust/config/verbs/sem-reg/registry.yaml` — add under `registry` domain verbs:

```yaml
      object-schema:
        description: Return the JSON Schema for a registry object type definition body, showing all fields, types, required/optional, and descriptions. Used by AI agents to formulate valid definitions for changeset.add-item.
        metadata:
          tier: diagnostics
          source_of_truth: operational
          scope: global
          noun: registry_schema
          category: registry_query
          context: scripted_ok
          side_effects: facts_only
          tags: [stewardship, schema, data-management]
          subject_kinds: []
          phase_tags: [stewardship, data-management, data]
        behavior: plugin
        handler: RegistryObjectSchemaOp
        invocation_phrases:
          - "show object schema"
          - "what fields does an entity type definition have"
          - "describe the structure of an attribute definition"
          - "show me the schema for a verb contract"
          - "what goes in an entity type def"
          - "registry object body schema"
          - "how do I define a new attribute"
          - "what fields are needed for a verb contract"
        args:
          - name: object-type
            type: string
            required: true
            description: "Registry object type: attribute_def, entity_type_def, verb_contract, view_def, policy_rule, taxonomy_def, relationship_type_def, derivation_spec, observation_def, membership, document_type_def"
        returns:
          type: record
          capture: false
```

### 1B: Handler implementation

**File:** `rust/src/domain_ops/registry_ops.rs` (or wherever the existing `RegistryDescribeObjectOp`, `RegistrySearchOp` etc. live — find with `grep -rn "RegistryDescribeObjectOp" rust/src/`)

Create `RegistryObjectSchemaOp` handler. It does NOT call the database — it returns a statically built JSON Schema derived from the Rust types.

Implementation approach — build the schemas at compile time or startup from the `sem_os_core` body types:

```rust
fn execute(&self, args: &BTreeMap<String, ArgumentValue>, _pool: &PgPool) -> Result<Value> {
    let object_type = args.get_string("object-type")?;
    
    let schema = match object_type.as_str() {
        "attribute_def" => attribute_def_schema(),
        "entity_type_def" => entity_type_def_schema(),
        "verb_contract" => verb_contract_schema(),
        "view_def" => view_def_schema(),
        "policy_rule" => policy_rule_schema(),
        "taxonomy_def" => taxonomy_def_schema(),
        "relationship_type_def" => relationship_type_def_schema(),
        // ... etc
        _ => return Err(anyhow!("Unknown object type: {}", object_type)),
    };
    
    Ok(schema)
}
```

For each body type, build a JSON object that describes the fields. You do NOT need a full JSON Schema spec — a pragmatic format is fine:

```json
{
  "object_type": "entity_type_def",
  "description": "Defines an entity type in the semantic registry",
  "fields": {
    "fqn": { "type": "string", "required": true, "description": "Fully qualified name (e.g., 'settlement-instruction')" },
    "name": { "type": "string", "required": true, "description": "Human-readable name" },
    "description": { "type": "string", "required": true, "description": "What this entity represents" },
    "domain": { "type": "string", "required": true, "description": "Domain this entity belongs to (e.g., 'custody', 'cbu')" },
    "db_table": {
      "type": "object", "required": false,
      "description": "Database table mapping",
      "fields": {
        "schema": { "type": "string", "required": true, "description": "PostgreSQL schema name" },
        "table": { "type": "string", "required": true, "description": "Table name" },
        "primary_key": { "type": "string", "required": true, "description": "Primary key column" },
        "name_column": { "type": "string", "required": false, "description": "Column used for display name" }
      }
    },
    "lifecycle_states": {
      "type": "array", "required": false,
      "description": "State machine definition",
      "item_fields": {
        "name": { "type": "string", "required": true },
        "description": { "type": "string", "required": false },
        "terminal": { "type": "boolean", "required": false, "default": false },
        "transitions": {
          "type": "array", "required": false,
          "item_fields": {
            "to": { "type": "string", "required": true, "description": "Target state name" },
            "trigger_verb": { "type": "string", "required": false, "description": "Verb FQN that triggers this transition" },
            "guard": { "type": "string", "required": false, "description": "Precondition expression" }
          }
        }
      }
    },
    "required_attributes": { "type": "array<string>", "required": false, "description": "FQNs of required attributes" },
    "optional_attributes": { "type": "array<string>", "required": false, "description": "FQNs of optional attributes" },
    "parent_type": { "type": "string", "required": false, "description": "FQN of parent entity type for inheritance" },
    "governance_tier": { "type": "string", "required": false, "description": "governed or operational" },
    "security_classification": { "type": "string", "required": false, "description": "public/internal/confidential/restricted" },
    "pii": { "type": "boolean", "required": false, "description": "Whether entity contains PII" }
  },
  "example": {
    "fqn": "settlement-instruction",
    "name": "Settlement Instruction",
    "description": "Standing settlement instruction for a custody account",
    "domain": "custody",
    "db_table": { "schema": "ob-poc", "table": "settlement_instructions", "primary_key": "instruction_id" },
    "lifecycle_states": [
      { "name": "draft", "transitions": [{ "to": "active", "trigger_verb": "entity-settlement.activate" }] },
      { "name": "active", "transitions": [{ "to": "suspended" }, { "to": "cancelled" }] },
      { "name": "suspended", "transitions": [{ "to": "active" }] },
      { "name": "cancelled", "terminal": true }
    ],
    "required_attributes": ["ssi.currency", "ssi.market", "ssi.counterparty"],
    "governance_tier": "governed"
  }
}
```

Build one of these for each of the core body types:
- `AttributeDefBody` (from `sem_os_core::attribute_def`)
- `EntityTypeDefBody` (from `sem_os_core::entity_type_def`)
- `VerbContractBody` (from `sem_os_core::verb_contract`)
- `ViewDefBody` (from `sem_os_core::view_def`)

The other types (policy_rule, taxonomy_def, etc.) can return a stub `{ "object_type": "...", "note": "schema not yet documented" }` for now.

### 1C: Register the handler

Wire `RegistryObjectSchemaOp` into the CustomOp registry using `#[register_custom_op]` (same pattern as existing registry ops).

### 1D: Unit test

```rust
#[test]
fn registry_object_schema_returns_entity_type_def() {
    // Call handler with object-type = "entity_type_def"
    // Assert result has "fields" key
    // Assert "fqn", "name", "domain" are in fields and marked required
    // Assert "lifecycle_states" is in fields
    // Assert "example" key is present
}
```

**GATE 1:** `cargo check -p ob-poc` passes. `cargo test --lib -p ob-poc -- registry_object_schema` passes. Print `DATAMGMT PHASE 1 COMPLETE — 35% done`.

→ IMMEDIATELY proceed to Phase 2.

---

## Phase 2: `schema.generate-migration` Verb (70%)

**Goal:** Given a registry object body (specifically `entity_type_def` or `attribute_def`), generate the PostgreSQL CREATE TABLE / ALTER TABLE migration SQL. This closes the loop: the Sage designs a new entity type → this verb produces the migration → the migration gets added to a changeset.

### 2A: Verb YAML definition

**File:** `rust/config/verbs/sem-reg/schema.yaml` — add under `schema` domain verbs:

```yaml
      generate-migration:
        description: Generate PostgreSQL migration SQL from a registry object definition. Produces CREATE TABLE for entity_type_def (with columns from required/optional attributes) or ALTER TABLE ADD COLUMN for attribute_def. Output is SQL text suitable for inclusion in a changeset artifact.
        metadata:
          tier: diagnostics
          source_of_truth: operational
          scope: global
          noun: schema_migration
          category: schema_analysis
          context: scripted_ok
          side_effects: facts_only
          tags: [stewardship, schema, data-management]
          subject_kinds: []
          phase_tags: [stewardship, data-management, data]
        behavior: plugin
        handler: SchemaGenerateMigrationOp
        invocation_phrases:
          - "generate migration"
          - "create table SQL for this entity type"
          - "generate SQL migration"
          - "build migration from definition"
          - "produce DDL for entity type"
          - "what SQL do I need for this entity"
          - "generate create table"
          - "schema migration from definition"
        args:
          - name: object-type
            type: string
            required: true
            description: "Registry object type: entity_type_def or attribute_def"
          - name: definition
            type: json
            required: true
            description: Object definition body as JSON (EntityTypeDefBody or AttributeDefBody)
          - name: schema-name
            type: string
            required: false
            default: "ob-poc"
            description: Target PostgreSQL schema for the migration
          - name: if-not-exists
            type: boolean
            required: false
            default: true
            description: Whether to use IF NOT EXISTS in CREATE TABLE
        returns:
          type: record
          capture: false
```

### 2B: Handler implementation

**File:** Create handler `SchemaGenerateMigrationOp`.

For `entity_type_def`:
1. Parse `definition` as `EntityTypeDefBody`
2. Use `db_table` mapping for table name and schema
3. Generate columns from `required_attributes` + `optional_attributes`:
   - Look up each attribute FQN in the registry (or from the definition's attribute list)
   - Map `AttributeDataType` to PostgreSQL types:
     - String → `TEXT`
     - Integer → `INTEGER`
     - Decimal → `NUMERIC`
     - Boolean → `BOOLEAN`
     - Uuid → `UUID`
     - Date → `DATE`
     - Timestamp → `TIMESTAMPTZ`
     - Json → `JSONB`
     - Enum → `TEXT` with CHECK constraint
   - Required attributes get `NOT NULL`
4. Add standard columns: `{primary_key} UUID PRIMARY KEY DEFAULT gen_random_uuid()`, `created_at TIMESTAMPTZ NOT NULL DEFAULT now()`, `updated_at TIMESTAMPTZ`
5. If lifecycle_states present, add `status TEXT NOT NULL DEFAULT '{first_state}'`
6. Return the SQL as a string in the result

For `attribute_def`:
1. Parse as `AttributeDefBody`
2. Generate `ALTER TABLE {source.schema}.{source.table} ADD COLUMN IF NOT EXISTS {column_name} {pg_type};`
3. If constraints.required, add separate `ALTER TABLE ... ALTER COLUMN ... SET NOT NULL;`

Output format:
```json
{
  "sql": "CREATE TABLE IF NOT EXISTS \"ob-poc\".settlement_instructions (\n  instruction_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),\n  currency TEXT NOT NULL,\n  ...\n);",
  "object_type": "entity_type_def",
  "table": "ob-poc.settlement_instructions",
  "column_count": 8,
  "notes": ["Generated from EntityTypeDefBody definition", "3 required attributes → NOT NULL columns"]
}
```

**Important:** This verb is `side_effects: facts_only` — it generates SQL text, it does NOT execute it. The SQL gets added to a changeset artifact and goes through the governance pipeline. The Sage shows it to the user for confirmation. Nobody runs DDL without governance.

### 2C: Unit tests

```rust
#[test]
fn generate_migration_entity_type_creates_table() {
    let def = json!({
        "fqn": "settlement-instruction",
        "name": "Settlement Instruction",
        "domain": "custody",
        "description": "SSI record",
        "db_table": { "schema": "ob-poc", "table": "settlement_instructions", "primary_key": "instruction_id" },
        "required_attributes": ["ssi.currency", "ssi.market"],
        "optional_attributes": ["ssi.notes"],
        "lifecycle_states": [{ "name": "draft" }, { "name": "active" }]
    });
    // Call handler
    // Assert result["sql"] contains "CREATE TABLE"
    // Assert result["sql"] contains "instruction_id UUID PRIMARY KEY"
    // Assert result["sql"] contains "status TEXT NOT NULL DEFAULT 'draft'"
    // Assert result["column_count"] >= 5
}

#[test]
fn generate_migration_attribute_def_alters_table() {
    let def = json!({
        "fqn": "ssi.settlement_date",
        "name": "settlement_date",
        "domain": "custody",
        "description": "Expected settlement date",
        "data_type": "date",
        "source": { "schema": "ob-poc", "table": "settlement_instructions", "column": "settlement_date" },
        "constraints": { "required": true }
    });
    // Call handler
    // Assert result["sql"] contains "ALTER TABLE"
    // Assert result["sql"] contains "ADD COLUMN IF NOT EXISTS"
    // Assert result["sql"] contains "settlement_date DATE"
    // Assert result["sql"] contains "SET NOT NULL"
}
```

### 2D: Register handler

Wire `SchemaGenerateMigrationOp` into CustomOp registry.

**GATE 2:** `cargo check -p ob-poc` passes. `cargo test --lib -p ob-poc -- generate_migration` passes. Print `DATAMGMT PHASE 2 COMPLETE — 70% done`.

→ IMMEDIATELY proceed to Phase 3.

---

## Phase 3: Attribute Resolution in Migration Generator (85%)

**Goal:** When generating CREATE TABLE from an EntityTypeDefBody, the `required_attributes` and `optional_attributes` are FQN strings like `"ssi.currency"`. The migration generator needs to resolve these to actual column names and types. Two paths:

### 3A: Registry lookup path

If the attribute FQN exists in the registry (as a published `attribute_def` snapshot), look it up:

```rust
// In SchemaGenerateMigrationOp handler:
fn resolve_attribute(fqn: &str, pool: &PgPool) -> Option<(String, String)> {
    // Query sem_reg.active_snapshots for object_type='attribute_def', fqn=fqn
    // Parse body as AttributeDefBody
    // Return (column_name, pg_type)
    // column_name = body.source.column or body.fqn.split('.').last()
    // pg_type = map body.data_type to PostgreSQL type
}
```

### 3B: Inline definition fallback

If the attribute is NOT in the registry (new attribute being defined in the same changeset), allow the caller to provide attribute definitions inline:

Add optional arg to `schema.generate-migration`:
```yaml
          - name: attribute-defs
            type: json
            required: false
            description: Optional inline attribute definitions (array of AttributeDefBody JSON) for attributes not yet in registry
```

The handler checks inline defs first, then falls back to registry lookup, then falls back to `TEXT` with a warning note.

### 3C: Test with mixed resolution

```rust
#[test]
fn generate_migration_resolves_attributes_from_inline_defs() {
    // Provide entity_type_def with required_attributes: ["ssi.currency", "ssi.market"]
    // Provide attribute-defs with definitions for both
    // Assert generated SQL has correct column types from the definitions
}
```

**GATE 3:** `cargo check -p ob-poc` passes. Tests pass. Print `DATAMGMT PHASE 3 COMPLETE — 85% done`.

→ IMMEDIATELY proceed to Phase 4.

---

## Phase 4: End-to-End Workflow Test (100%)

**Goal:** Validate the full research → design → create → govern workflow works with the DSL as it now exists.

### 4A: Write an integration test (or manual test script) that does:

```
1. schema.introspect :schema-name "ob-poc" :table-name "entities"
   → See existing table structure

2. registry.object-schema :object-type "entity_type_def"
   → Get the schema for defining a new entity type

3. schema.generate-migration :object-type "entity_type_def" 
     :definition <EntityTypeDefBody JSON for a test entity>
   → Get the CREATE TABLE SQL

4. changeset.compose :title "Add test entity type"
   → Get changeset-id

5. changeset.add-item :changeset-id <id> :object-type "entity_type_def" 
     :fqn "test-entity" :definition <same JSON>
   → Add the definition to the changeset

6. changeset.add-item :changeset-id <id> :object-type "migration"
     :fqn "create-test-entity-table" :definition <SQL from step 3>
   → Add the migration SQL as an artifact

7. changeset.validate-edit :changeset-id <id>
   → Validate the changeset

8. governance.gate-precheck :changeset-id <id>
   → Check governance gates
```

This test does NOT publish (no DDL execution). It validates the full chain is wirable.

### 4B: Document the workflow

**File:** `docs/data-management-design-workflow.md`

Write a short (1-page) reference showing the Sage/Coder flow for "user wants to define a new entity type":

```
User: "I need a new entity type for settlement instructions"

SAGE (Structure, Read):
  → schema.introspect — examine existing settlement tables
  → registry.object-schema :object-type "entity_type_def" — get body schema

SAGE (LLM formulates definition using body schema as template)

SAGE → User:
  "Here's the proposed entity type: [fields, lifecycle, attributes]
   And the migration SQL: [CREATE TABLE ...]
   Shall I create a changeset?"

User: "Yes, but add settlement_date as required"

SAGE (refines) → CODER:
  changeset.compose → changeset.add-item (entity_type_def)
                    → changeset.add-item (migration SQL)
  → governance.gate-precheck → governance.submit-for-review
```

**GATE 4:** Integration test passes (or manual test documented with outputs). Workflow doc written. Print `DATAMGMT PHASE 4 COMPLETE — 100% done`.

---

## Files Created/Modified Summary

| File | Action | Phase |
|------|--------|-------|
| `rust/config/verbs/sem-reg/registry.yaml` | Modify — add `object-schema` verb | 1A |
| `rust/config/verbs/sem-reg/schema.yaml` | Modify — add `generate-migration` verb | 2A |
| `rust/src/domain_ops/registry_ops.rs` (or equivalent) | Modify — add `RegistryObjectSchemaOp` handler | 1B, 1C |
| `rust/src/domain_ops/schema_ops.rs` (or equivalent) | Modify — add `SchemaGenerateMigrationOp` handler | 2B, 2D, 3A, 3B |
| `docs/data-management-design-workflow.md` | Create | 4B |
| Integration test file (location TBD) | Create | 4A |

## No Dependencies on Sage/Coder Phases

These verbs are useful immediately — from MCP, from direct DSL, and from the existing chat pipeline. They don't require the Sage to exist. But when the Sage IS wired, they become the Sage's primary tools for Structure-plane design workflows.
