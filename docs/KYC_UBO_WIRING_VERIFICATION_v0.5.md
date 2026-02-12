# KYC/UBO Wiring Verification — Supplement to TODO v0.5

**Purpose:** The main TODO (KYC_UBO_TODO_v0.5.md) covers verbs, handlers, migrations, and tests. This supplement covers the **downstream wiring** — everything that consumes or renders the data those handlers produce. Without these checks, the backend works but nothing else does.

**When to run:** After each Phase in the main TODO, run the corresponding wiring checks below.

---

## 1. Rust Struct / SQLx Typed Query Compilation

**Risk:** Adding columns to `entity_relationships` and creating new tables will cause SQLx compile-time failures if Rust model structs don't match.

**After Phase 0 (migrations):**

```
□ entity_relationships model struct updated with:
  - import_run_id: Option<Uuid>
  - evidence_hint: Option<String>
  - confidence: String (NOT Option — now NOT NULL)
  - Remove any DEFAULT 'HIGH' from the struct if it has one

□ New model structs created for:
  - GraphImportRun (graph_import_runs)
  - CaseImportRun (case_import_runs)
  - KycCase (kyc.cases)
  - EntityWorkstream (kyc.entity_workstreams)
  - UboDeterminationRun (kyc.ubo_determination_runs)
  - UboRegistryEntry (kyc.ubo_registry)
  - UboEvidence (kyc.ubo_evidence)
  - OutreachPlan (kyc.outreach_plans)
  - OutreachItem (kyc.outreach_items)
  - TollgateEvaluation (kyc.tollgate_evaluations)

□ ALL existing queries that SELECT from entity_relationships still compile
  - Search for: sqlx::query_as!, sqlx::query!, any typed query referencing entity_relationships
  - Check: any query using SELECT * or column lists — add new columns or use explicit column lists

□ cargo build --all-targets passes with zero SQLx errors
```

**How to find affected queries:**
```bash
grep -rn "entity_relationships" rust/src/ --include="*.rs" | grep -i "query"
grep -rn "from.*entity_relationships" rust/src/ --include="*.rs"
```

---

## 2. Projection Generator / Snapshot Model

**Risk:** The projection system (Snapshot → Projection → JSON → TypeScript) may not include new provenance fields, causing the UI to show stale/incomplete data.

**After Phase 1:**

```
□ Snapshot model includes new entity_relationships fields:
  - import_run_id, confidence, evidence_hint, source, source_ref
  - If the snapshot serializer uses field-by-field construction (not SELECT *),
    add the new fields

□ Projection generator handles new KYC objects:
  - If projection generator iterates known entity types, add:
    - KYC Case (or defer to Phase 2 when cases actually exist)
    - Workstreams
    - UBO Registry entries

□ Projection YAML/JSON schema updated if it has explicit field lists

□ Check: does the projection generator use the RenderPolicy pattern?
  - If yes: ensure new relationship_type values and provenance fields
    are not filtered out by render policies
```

**How to find:**
```bash
grep -rn "projection" rust/src/ --include="*.rs" | grep -i "relationship\|edge\|entity_rel"
find rust/src -name "*.rs" -path "*projection*" -o -name "*.rs" -path "*snapshot*"
```

---

## 3. TypeScript / React Types

**Risk:** TypeScript types mirror the projection contract. New fields need TS type updates or the UI silently ignores them.

**After Phase 1:**

```
□ ob-poc-ui/src/types/projection.ts updated:
  - EntityRelationship type gains: importRunId?, confidence, evidenceHint?, source, sourceRef?
  - New types added: KycCase, EntityWorkstream, UboRegistryEntry, etc.
    (or deferred until API endpoints serve them)

□ Any React component rendering entity relationships:
  - Check it doesn't break on new fields
  - Ideally: show confidence badge/indicator on edges
  - Ideally: show source provenance on hover/detail

□ If there's a graph/tree visualization component:
  - Edge rendering should handle confidence (color-code: HIGH=green, MEDIUM=amber, LOW=red?)
  - Import run provenance should be accessible in edge detail view
```

**How to find:**
```bash
grep -rn "relationship\|edge\|EntityRel" ob-poc-ui/src/ --include="*.ts" --include="*.tsx"
find ob-poc-ui/src -name "*.ts" -o -name "*.tsx" | xargs grep -l "confidence\|source\|provenance"
```

---

## 4. Verb YAML Invocation Phrases + BGE Re-embedding

**Risk:** New verbs without invocation_phrases are invisible to the semantic intent pipeline. The BGE embedder won't find them.

**After Phase 1 (new verb YAMLs created):**

```
□ Every new verb YAML has invocation_phrases:
  - kyc.create-case: ["create a kyc case", "start onboarding", "open a case", ...]
  - kyc.update-status: ["update case status", "move case to assessment", ...]
  - evidence.require: ["require evidence", "what evidence do we need", ...]
  - evidence.link: ["link document to evidence", "attach proof", ...]
  - (etc. for all ~34 new verbs)

□ Each verb YAML has description field (used by MCP tool descriptions)

□ After all YAMLs committed:
  - Run populate_embeddings to re-embed the full verb corpus with BGE
  - Verify: new verbs appear in pgvector verb_embeddings table
  - Test: semantic search for "start a case" returns kyc.create-case

□ Agent verb classification metadata:
  - Verify YAML parser doesn't reject the new fields:
    category, context, side_effects
  - Verify VerbSpec struct in Rust can deserialize these fields
```

**How to verify:**
```bash
# Check all new YAMLs have invocation_phrases
for f in config/verbs/kyc/*.yaml config/verbs/research/workflow.yaml; do
  echo "=== $f ==="
  grep -c "invocation_phrases" "$f"
done

# After re-embedding, check verb count
psql -c "SELECT count(*) FROM verb_embeddings WHERE verb_name LIKE 'kyc.%' OR verb_name LIKE 'evidence.%' OR verb_name LIKE 'research.import-run.%'"
```

---

## 5. DAG / Session Pipeline / Template Expansion

**Risk:** The skeleton build template verb (§7.7) uses `(set @symbol ...)` bindings and inline expressions. The DAG phase calculator and template expander must handle these correctly.

**After Phase 2 (skeleton build template created):**

```
□ Template expansion handles:
  - $arg substitution (case-id, subject-id, as-of, etc.)
  - (set @symbol (verb ...)) binding — return value captured
  - (verb :arg @symbol) — bound symbol passed as arg
  - Inline expression: (entity.get-lei $subject-id) — evaluates to a value

□ DAG phase ordering:
  - Phase 1 verbs (imports) must complete before Phase 2 (validate)
  - Phase 2 must complete before Phase 3 (derive)
  - Phase 3 must complete before Phase 4 (plan)
  - Phase 4 must complete before Phase 5 (gate)
  - produces/consumes metadata on verbs must form correct dependency graph

□ Verify: execution_plan.rs compute_phases() correctly sequences the
  skeleton build expanded body
  - Within Phase 1: GLEIF run and CH run can be parallel (no dependency)
  - Between phases: strict ordering

□ Run sheet displays correctly:
  - Each expanded verb shows in the run sheet with status
  - @symbol bindings visible in run sheet context
  - Failure in any phase → downstream marked SKIPPED

□ Transaction boundary:
  - Is the entire skeleton build one transaction? Or per-import-run?
  - Spec says: each import run is a "rollbackable graph patch" — suggests
    per-import-run transactions, not one big tx
  - Verify executor supports this (multiple commit points within template)
```

**How to verify:**
```bash
# Check template expansion
grep -rn "template\|expand" rust/src/dsl_v2/ --include="*.rs" | grep -iv test
# Check DAG phasing
grep -rn "compute_phases\|produces\|consumes" rust/src/dsl_v2/ --include="*.rs"
# Check session bindings
grep -rn "set.*@\|substitute_symbol" rust/src/dsl_v2/ --include="*.rs"
```

---

## 6. MCP Tool Definitions

**Risk:** New verbs exposed via MCP need tool definitions or the agent can't invoke them.

**After Phase 1:**

```
□ Check: are MCP tools auto-generated from verb YAML, or manually defined?
  - If auto-generated: verify the generator handles new YAML fields
    (category, context, side_effects)
  - If manually defined: add tool definitions for key verbs:
    - kyc.create-case, kyc.update-status
    - kyc.skeleton.build (the big one)
    - evidence.require, evidence.link, evidence.verify
    - agent task verbs that trigger KYC workflows

□ MCP tool parameter schemas match verb args:
  - case-id: UUID
  - subject-id: UUID
  - as-of: date string
  - status: enum matching CHECK constraints

□ Agent orchestration prompts:
  - /prompts/research/orchestration/*.md — do they reference new verbs?
  - If the agent uses prompt templates to decide which verb to call,
    those templates need updating for the KYC verb surface
```

---

## 7. Agent Session / ScopeGate

**Risk:** The agent session currently gates on client group selection (ScopeGate). KYC cases may need an additional gate or context.

**After Phase 2:**

```
□ Session context: does the session need a current_case_id?
  - skeleton.build takes :case-id — where does this come from?
  - Option A: user selects case in session (new gate: CaseGate)
  - Option B: case-id passed as arg (no gate, manual)
  - Option C: auto-created from deal/client-group context

□ If CaseGate is needed:
  - Session state machine: EMPTY → SCOPED (client group) → CASED (case) → ready
  - Agent prompt: "Which case would you like to work on?" or "Create a new case?"
  - Fuzzy match on case_ref

□ If not needed now: document the decision and defer
```

---

## 8. API Routes

**Risk:** If the React UI needs to display KYC cases, workstreams, UBO registry — it needs API endpoints.

**After Phase 2:**

```
□ Decide: are KYC objects surfaced via API now or deferred?

If now, add routes:
  □ GET /api/cases — list cases (filtered by client_group_id)
  □ GET /api/cases/:id — case detail with workstreams
  □ GET /api/cases/:id/ubo-registry — UBO entries for case
  □ GET /api/cases/:id/evidence — evidence status for case
  □ GET /api/cases/:id/outreach — outreach plan and items
  □ GET /api/cases/:id/tollgates — tollgate evaluation history
  □ GET /api/cases/:id/import-runs — linked import runs with edge counts

If deferred:
  □ Document that API routes are Phase 5+ work
  □ Ensure DSL REPL is sufficient for testing (it should be)
```

---

## 9. ESPER / Graph Visualization

**Risk:** If ESPER or any graph viz renders ownership/control edges, new provenance metadata should be accessible.

**After Phase 2:**

```
□ Check: does ESPER render entity_relationships edges?
  - If yes: edge rendering should not break on new columns
  - Ideally: confidence shown as edge weight/color
  - Import run ID shown on edge detail/hover

□ Check: does ESPER handle temporal edges (effective_to != NULL)?
  - Superseded import runs soft-end edges — viz should filter for active edges only
  - Filter: WHERE effective_to IS NULL (or effective_to > NOW())

□ Navigation verbs:
  - If ESPER has verbs like esper.focus, esper.zoom:
    - These still work (no breaking changes)
    - New: could add esper.show-case or esper.show-skeleton as nav verbs
      that focus the graph on a case's entity scope (deferred)
```

---

## 10. Existing Test Suite Integrity

**Risk:** The TODO specifies "all 160+ existing tests pass" but doesn't enumerate what could break.

**At every phase boundary:**

```
□ cargo test --all-targets
□ Specific risk areas:
  - Edge upsert tests: behaviour changed to end-and-insert
    - Any test asserting UPDATE behaviour needs updating to expect INSERT
  - Entity relationship query tests: confidence column now NOT NULL
    - Any test inserting entity_relationships without confidence → compile error
  - Graph traversal tests: if they construct entity_relationships fixtures,
    fixtures need confidence field
  - UBO computation tests: same — fixtures need new columns
  - YAML parser tests: new fields (category, context, side_effects) on agent verbs
    - Parser must not reject unknown fields (or must be updated)

□ If any test fails, fix BEFORE proceeding to next task
```

---

## Execution Order Summary

| After Phase | Run These Checks |
|-------------|-----------------|
| Phase 0 (migrations) | §1 (structs), §10 (test suite) |
| Phase 1 (core verbs) | §1, §4 (invocation phrases), §6 (MCP tools), §10 |
| Phase 2 (skeleton build) | §2 (projections), §3 (TS types), §5 (DAG), §7 (session), §8 (API), §9 (ESPER), §10 |
| Phase 3 (derivation) | §4 (re-embed after new verbs), §10 |
| Phase 4 (deal integration) | All checks, full regression |

---

## Quick Diagnostic Commands

```bash
# 1. Does it compile?
cargo build --all-targets 2>&1 | head -50

# 2. Do tests pass?
cargo test 2>&1 | tail -20

# 3. Are new tables present?
psql ob-poc -c "SELECT table_schema, table_name FROM information_schema.tables WHERE table_schema IN ('ob-poc','kyc','ob_ref') ORDER BY 1,2"

# 4. Are new verbs embedded?
psql ob-poc -c "SELECT verb_name FROM verb_embeddings WHERE verb_name LIKE 'kyc.%' OR verb_name LIKE 'evidence.%' OR verb_name LIKE 'research.import%' ORDER BY 1"

# 5. Do new YAMLs parse?
cargo test yaml_parse 2>&1

# 6. Is the skeleton template valid?
cargo test skeleton 2>&1

# 7. React types still compile?
cd ob-poc-ui && npx tsc --noEmit 2>&1 | head -20
```
