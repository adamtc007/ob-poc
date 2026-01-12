# 031: RAG Cleanup + Full Stack Audit

> **Status:** TODO — For Claude Code
> **Priority:** HIGH
> **Risk Level:** MEDIUM (RAG files are fragile, build currently passes)
> **Constraint:** Do NOT break the build. If unsure, leave it alone.

---

## Context

Claude Code attempted automated cleanup of RAG files but kept breaking them. The files are complex nested Rust code. Build currently passes. Stale references are harmless (unused lookup entries). Risk of breaking outweighs benefit of aggressive cleanup.

**Key principle:** RAG files are the bridge to agent MCP. Do not compromise agent functionality.

---

## Part 1: Safe RAG File Cleanup Strategy

### 1.1 Inventory RAG Files

```bash
# Find all RAG-related files
find rust/ -name "*rag*" -o -name "*vector*" -o -name "*embed*" | grep -v target
find rust/ -name "*.rs" -exec grep -l "qdrant\|embedding\|vector_db\|rag" {} \;
```

**Document:**
- [ ] List all RAG-related source files
- [ ] List all RAG-related config files
- [ ] Identify which files have complex nested structures

### 1.2 Safe Cleanup Rules

**DO:**
- Remove clearly dead code (functions with zero callers, confirmed by grep)
- Remove commented-out blocks older than 30 days
- Fix obvious typos in comments
- Consolidate duplicate imports

**DO NOT:**
- Remove any struct field (even if seemingly unused — may be serialized)
- Remove any public function (may be called via MCP)
- Modify nested match arms or complex pattern matching
- Change any serde attributes
- Remove any `#[allow(...)]` attributes

### 1.3 Verification Protocol

Before ANY change to a RAG file:

```bash
# 1. Build check
cargo build -p ob-agentic 2>&1 | tee /tmp/before.txt

# 2. Make change

# 3. Build check again
cargo build -p ob-agentic 2>&1 | tee /tmp/after.txt

# 4. Diff — should be identical or fewer warnings
diff /tmp/before.txt /tmp/after.txt
```

If build fails after change → **revert immediately**, do not attempt fix.

### 1.4 Stale Reference Handling

Stale lookup entries are harmless. If a reference exists but target is gone:
- [ ] Document it in a `STALE_REFS.md` file
- [ ] Do NOT delete the reference
- [ ] Mark with `// TODO: stale ref, target removed in commit X`

---

## Part 2: Full Stack Round-Trip Audit

### 2.1 Audit Scope

Trace the complete path for a DSL verb execution:

```
User Input (egui/voice/REPL)
    ↓
Session State (Rust structs)
    ↓
DSL Parser (dsl-core)
    ↓
Verb Resolution (verb_registry)
    ↓
Executor (generic_executor / custom_ops)
    ↓
Database (PostgreSQL via SQLx)
    ↓
Result Capture
    ↓
Visualization Structs (server-side)
    ↓
egui Rendering (client-side)
```

### 2.2 DB → Verbs → Rust Audit

**Goal:** Verify all verb YAML definitions have matching Rust handlers (or are pure CRUD).

```bash
# List all verb definitions
find rust/config/verbs -name "*.yaml" -exec grep -h "^      [a-z]" {} \; | sort -u > /tmp/all_verbs.txt

# List all CRUD verbs (no handler needed)
grep -rh "behavior: crud" rust/config/verbs --include="*.yaml" -A5 | grep "^      [a-z]" > /tmp/crud_verbs.txt

# List all plugin verbs (handler required)
grep -rh "behavior: plugin" rust/config/verbs --include="*.yaml" -A2 | grep "handler:" | awk '{print $2}' | sort -u > /tmp/plugin_handlers.txt

# List all implemented handlers
grep -rh "impl CustomOp for" rust/src/dsl_v2/custom_ops/*.rs | awk '{print $4}' | sort -u > /tmp/implemented_handlers.txt

# Find mismatches
diff /tmp/plugin_handlers.txt /tmp/implemented_handlers.txt
```

**Checklist:**
- [ ] Every `behavior: plugin` verb has a matching `impl CustomOp`
- [ ] No orphaned handlers (impl exists but no YAML references it)
- [ ] Handler names match exactly (case-sensitive)

### 2.3 DSL Pipeline Audit

**Files to trace:**

| Stage | File | What to Check |
|-------|------|---------------|
| Parser | `dsl-core/src/parser.rs` | All AST node types handled |
| Compiler | `dsl-core/src/compiler.rs` | All verb forms compile |
| Verb Loader | `src/dsl_v2/verb_loader.rs` | All YAML files load without error |
| Registry | `src/dsl_v2/verb_registry.rs` | Verbs registered correctly |
| Executor | `src/dsl_v2/generic_executor.rs` | CRUD operations map correctly |
| Custom Ops | `src/dsl_v2/custom_ops/*.rs` | All handlers implement trait |

**Verification:**
```bash
# Verify all verbs load
cargo x verify-verbs

# Run DSL test suite
cargo test --features database dsl
```

- [ ] `verify-verbs` passes with zero errors
- [ ] All DSL tests pass
- [ ] No warnings about missing handlers

### 2.4 Agent RAG / Vector DB Audit

**Files to trace:**

| Component | Location | What to Check |
|-----------|----------|---------------|
| Embedding generation | `ob-agentic/src/embed/` | Model loads, vectors generated |
| Vector storage | `ob-agentic/src/qdrant/` | Qdrant connection, upsert/query works |
| RAG retrieval | `ob-agentic/src/rag/` | Context retrieval returns relevant chunks |
| Agent integration | `src/agent/` | RAG results flow to agent loop |

**Verification:**
```bash
# Check Qdrant connection
curl http://localhost:6333/collections

# Run agent tests
cargo test --features database agent

# Check embedding dimension consistency
grep -r "embedding_dim\|vector_size\|dimension" rust/crates/ob-agentic/src/
```

- [ ] Qdrant collections exist and have correct schema
- [ ] Embedding dimensions match across all usages
- [ ] RAG queries return non-empty results for known entities

### 2.5 Session State Audit

**Files to trace:**

| Component | Location | What to Check |
|-----------|----------|---------------|
| Session struct | `src/session/state.rs` | All fields initialized |
| Scope management | `src/session/scope.rs` | Scope transitions valid |
| History | `src/session/history.rs` | Back/forward works |
| Persistence | DB tables `session_*` | Schema matches Rust structs |

**Verification:**
```bash
# Check session table schema
psql -d data_designer -c "\d session_scope_state"
psql -d data_designer -c "\d session_scope_history"

# Compare with Rust struct
grep -A50 "pub struct SessionScopeState" rust/src/session/
```

- [ ] All DB columns have matching Rust struct fields
- [ ] All Rust fields are either persisted or marked transient
- [ ] Session round-trips correctly (save → load → compare)

### 2.6 Visualization Structs Audit

**Goal:** Verify server-side viz structs match what egui expects.

**Files to trace:**

| Component | Location | What to Check |
|-----------|----------|---------------|
| Graph types | `ob-poc-graph/src/types.rs` | Node/Edge structs |
| API responses | `src/api/graph.rs` | JSON serialization |
| UI consumption | `ob-poc-ui/src/panels/` | Deserialization matches |

**Verification:**
```bash
# Find all viz-related structs
grep -rh "pub struct.*Node\|pub struct.*Edge\|pub struct.*Graph" rust/crates/ob-poc-graph/src/

# Find all serde derives
grep -rh "#\[derive.*Serialize" rust/crates/ob-poc-graph/src/

# Check API response types
grep -rh "Json<" rust/src/api/graph.rs
```

- [ ] All viz structs have `Serialize` + `Deserialize`
- [ ] Field names match between server and client
- [ ] No `#[serde(skip)]` fields that client expects

### 2.7 egui Rendering Audit

**Files to trace:**

| Panel | Location | What to Check |
|-------|----------|---------------|
| Graph panel | `ob-poc-ui/src/panels/graph.rs` | Renders nodes/edges correctly |
| Context panel | `ob-poc-ui/src/panels/context.rs` | Session state displayed |
| Results panel | `ob-poc-ui/src/panels/results.rs` | DSL output rendered |
| Trading matrix | `ob-poc-ui/src/panels/trading_matrix.rs` | Matrix viz correct |

**Verification:**
- [ ] Each panel has corresponding action enum
- [ ] No direct state mutation in render functions
- [ ] All async data accessed via short lock pattern

---

## Part 3: Reconciliation Checklist

After completing audits, produce a reconciliation report:

### 3.1 Verb ↔ Handler Reconciliation

| Verb | YAML Location | Handler | Status |
|------|---------------|---------|--------|
| (generate from audit) | | | ✅/❌ |

### 3.2 Struct ↔ Schema Reconciliation

| Rust Struct | DB Table | Field Mismatches | Status |
|-------------|----------|------------------|--------|
| (generate from audit) | | | ✅/❌ |

### 3.3 Server ↔ Client Reconciliation

| API Endpoint | Response Type | UI Consumer | Status |
|--------------|---------------|-------------|--------|
| (generate from audit) | | | ✅/❌ |

---

## Part 4: Fix Implementation

For each issue found:

1. **Document** the issue in this file first
2. **Assess risk** — is it breaking or cosmetic?
3. **Create minimal fix** — smallest change that resolves issue
4. **Verify** — build + test before and after
5. **Commit** — one issue per commit with clear message

**Commit message format:**
```
fix(component): Brief description

- What was wrong
- What was changed
- How verified
```

---

## Execution Order

1. [ ] Complete 2.2 (DB → Verbs → Rust) — establishes baseline
2. [ ] Complete 2.3 (DSL Pipeline) — verify execution path
3. [ ] Complete 2.5 (Session State) — verify state management
4. [ ] Complete 2.6 (Visualization Structs) — verify data flow
5. [ ] Complete 2.7 (egui Rendering) — verify UI layer
6. [ ] Complete 2.4 (Agent RAG) — verify agent integration
7. [ ] Complete 1.x (RAG Cleanup) — ONLY after full audit, with extreme caution
8. [ ] Produce reconciliation report (Part 3)
9. [ ] Implement fixes (Part 4)

---

## Output Artifacts

When complete, Claude Code should produce:

1. `docs/stack-audit-report.md` — Full findings
2. `docs/verb-handler-reconciliation.md` — Verb ↔ handler mapping
3. `docs/struct-schema-reconciliation.md` — Rust ↔ DB mapping
4. Updated `CLAUDE.md` — Any new patterns discovered

---

## Abort Conditions

**STOP and ask human if:**
- Build breaks and fix isn't obvious
- More than 10 files need changes for one issue
- RAG functionality stops working
- Agent tests start failing
- Any uncertainty about whether a reference is truly stale
