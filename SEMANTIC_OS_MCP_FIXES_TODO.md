# Semantic OS — MCP Integration & Verb/Entity Contract Hardening (Claude Code TODO)

**Repo:** `ob-poc`  
**Scope:** Semantic OS / `sem_reg` + MCP server integration + safety/consistency fixes  
**Primary refs:** `CLAUDE.md`, `rust/src/sem_reg/*`, `rust/src/mcp/*`, migrations `078..086`

---

## Goals

1) **Single tool surface**: Semantic OS tools are exposed through the **actual MCP server** (`tools/list`, `tools/call`) — no parallel “fake MCP” registry.  
2) **Security correctness**: ABAC / `security_label` is **enforced** for all SemReg tool reads.  
3) **Registry correctness**: snapshot publishing is atomic; context resolution doesn’t truncate; object identity is stable; scanner supports drift updates.  
4) **Operational robustness**: remove stringly-typed debug serialization and stop in-place “snapshot edits” that violate immutability.

---

## Non-goals (for this patch)

- Full “durable execution semantics” in SemReg verb contracts (park/resume, correlation) — we’ll add **contract fields + scaffolding** only where necessary.
- Re-architect the whole agent loop. This patch focuses on plumbing + correctness.

---

## Deliverables

- [ ] SemReg tools appear in MCP `tools/list` and work via `tools/call`
- [ ] SemReg tool handlers enforce ABAC
- [ ] Snapshot publish is atomic (transaction / CTE) and cannot produce “no active snapshot”
- [ ] `object_id` is deterministic by `(object_type, fqn)`; scanner can publish successors on change
- [ ] Context resolution loads **all** active snapshots (no 1000 truncation)
- [ ] Enum-to-wire strings are stable (no `Debug` format dependencies)
- [ ] xtask backfill no longer mutates snapshot rows in-place (publish successor snapshots instead)
- [ ] Minimal tests for the above

---

## Phase 0 — Prep / Code map (quick)

**Key files**
- MCP server:
  - `rust/src/mcp/server.rs`
  - `rust/src/mcp/tools.rs`
  - `rust/src/mcp/handlers/core.rs`
  - `rust/src/mcp/protocol.rs` (tool schema / JSON-RPC shapes)
  - `rust/src/bin/dsl_mcp.rs`

- Semantic registry:
  - `rust/src/sem_reg/agent/mcp_tools.rs` (existing SemReg tool specs + dispatch)
  - `rust/src/sem_reg/store.rs` (SnapshotStore publish)
  - `rust/src/sem_reg/context_resolution.rs` (truncation bug)
  - `rust/src/sem_reg/scanner.rs` (object_id v4 + no drift publish)
  - `rust/xtask/src/sem_reg.rs` (in-place updates)

---

## Phase 1 — Make SemReg tools real MCP tools (single tool surface)

### 1.1 Add SemReg tools to MCP `tools/list`
- [ ] Create `rust/src/mcp/tools_sem_reg.rs`
  - [ ] Export `pub fn sem_reg_tools() -> Vec<protocol::Tool>`
  - [ ] Convert SemReg tool specs into MCP `protocol::Tool` entries.
    - Preferred approach: define MCP tools directly (name/description/inputSchema) rather than attempting to “auto-convert” from the current `SemRegToolSpec` shape.
    - Keep tool names stable: `semreg.describe`, `semreg.search`, `semreg.list`, `semreg.taxonomy.tree`, etc.
- [ ] Update `rust/src/mcp/tools.rs::get_tools()` to append `tools_sem_reg::sem_reg_tools()`

**Acceptance**
- Running MCP server returns SemReg tools inside `tools/list`

### 1.2 Route SemReg tools through MCP `tools/call`
- [ ] Update `rust/src/mcp/handlers/core.rs::dispatch()`
  - [ ] Add a branch for SemReg tool names (prefix `semreg.`)
  - [ ] Call into a new adapter entrypoint:
    - `sem_reg::agent::mcp_adapter::dispatch_mcp_tool_call(ctx, tool_name, arguments_json) -> protocol::ToolCallResult`

- [ ] Add module `rust/src/sem_reg/agent/mcp_adapter.rs`
  - [ ] Parse MCP `arguments` into the existing `ToolCallArgs` types where possible.
  - [ ] Or define MCP-first input structs for each tool and call the corresponding internal handler functions.
  - [ ] Always return MCP `ToolCallResult` (match your `protocol` module).

**Notes**
- Avoid “two dispatchers”. The MCP handler should be *the* entrypoint.
- If you keep `sem_reg/agent/mcp_tools.rs`, rename it to `tools_internal.rs` or similar to avoid protocol confusion.

**Acceptance**
- `tools/call` for `semreg.describe` returns a valid MCP result with content.

### 1.3 Decide: keep or replace `sem_reg/agent/mcp_tools.rs`
- [ ] Option A (recommended): keep it as internal implementation, but:
  - [ ] Rename types to remove “MCP” from internal spec layer (e.g., `SemRegToolSpec` → `ToolSpecInternal`)
  - [ ] Expose *only* MCP-facing tool schema from `mcp/tools_sem_reg.rs`
- [ ] Option B: fully rewrite tools as MCP-first and delete the internal spec layer.

Pick A unless time is tight.

---

## Phase 2 — Enforce ABAC/security in SemReg tool handlers (data leak fix)

### 2.1 Implement a single enforcement helper
- [ ] Add `rust/src/sem_reg/security/enforce.rs` (or similar)
  - [ ] `fn enforce_read(actor: &ActorContext, label: &SecurityLabel, purpose: Purpose) -> Result<(), AccessDenied>`
  - [ ] Define `Purpose` values you need now: `ToolRead`, `AgentReasoning`, `Export`, etc.
  - [ ] Implement conservative defaults:
    - If label missing/unparseable → deny or “mask-by-default” depending on your policy
    - Respect handling controls like `NoLlmExternal`, `MaskByDefault`

### 2.2 Apply enforcement across all SemReg tool reads
- [ ] In every tool handler that returns snapshot definitions or derived data (describe/search/list/taxonomy/…):
  - [ ] Load security label for each snapshot result
  - [ ] Call `enforce_read(ctx.actor, &label, Purpose::ToolRead)`
  - [ ] If denied:
    - [ ] Either omit the item from results, or include a redacted entry:
      - `{"snapshot_id": "...", "fqn":"...", "redacted": true, "reason":"AccessDenied"}`
    - Decide one consistent pattern and apply everywhere.

**Acceptance**
- A snapshot labeled `NoLlmExternal` cannot be returned via tool calls to an actor without privilege.

---

## Phase 3 — Make snapshot publish atomic (correctness)

### 3.1 Transactional publish
- [ ] In `rust/src/sem_reg/store.rs`:
  - [ ] Change publish to run in a `sqlx::Transaction`:
    - begin tx
    - supersede predecessor (if any)
    - insert successor
    - commit
  - [ ] Refactor store methods to accept `&mut Transaction<'_>` where needed (or duplicate `_tx` variants).

### 3.2 Add DB constraint to prevent multiple actives
- [ ] Add a migration (new file, next number after 086)
  - [ ] Create partial unique index:
    - `UNIQUE (object_id) WHERE effective_until IS NULL`
  - [ ] (Optional) also enforce `effective_since` non-null and monotonicity if desired.

**Acceptance**
- A failed publish cannot leave the registry with zero active snapshot for an object.
- DB prevents multiple active snapshots for the same `object_id`.

---

## Phase 4 — Stable identity + scanner drift publishing

### 4.1 Deterministic `object_id`
- [ ] Add `rust/src/sem_reg/ids.rs`
  - [ ] Define a constant UUID namespace for SemReg IDs (document it).
  - [ ] `fn object_id_for(object_type: &str, fqn: &str) -> Uuid` using UUID v5:
    - input: `"{object_type}:{fqn}"`
- [ ] Update scanner (`rust/src/sem_reg/scanner.rs`) to use deterministic IDs for:
  - verbs
  - attributes
  - entities (if applicable)
  - any other cataloged object types

**Acceptance**
- Scanning the same YAML on two machines produces the same `object_id` values.

### 4.2 Scanner should publish successor snapshots when YAML changes
- [ ] In `run_onboarding_scan()`:
  - [ ] For each FQN:
    - [ ] Load active snapshot for that `object_id`
    - [ ] Compute new `definition_hash` (stable hashing of canonical JSON bytes)
    - [ ] If no active snapshot → insert as new active
    - [ ] If hash differs → publish successor snapshot (predecessor_id set)
  - [ ] Set `change_type`:
    - minimal: `non_breaking` by default
    - (optional) compute diff to mark `breaking` when args removed/renamed.

**Acceptance**
- Editing YAML and re-running scan results in a new active snapshot, not drift.

---

## Phase 5 — Fix context resolution truncation bug

### 5.1 Remove 1000-item silent truncation
- [ ] In `rust/src/sem_reg/context_resolution.rs`:
  - [ ] Replace single call `list_active(limit=1000, offset=0)` with pagination loop:
    - fetch page
    - append results
    - stop when returned < limit
  - [ ] Add a test that fails if truncation occurs.

**Acceptance**
- Context resolution loads all active verbs (>= 1083) without loss.

---

## Phase 6 — Replace stringly typed debug serialization

### 6.1 Stable wire names for enums
- [ ] In the types used by scanner:
  - Arg type enum
  - Behavior enum
  - Anything else serialized into SemReg definitions
- [ ] Add `as_wire_str()` functions or `serde` rename attributes.
- [ ] Replace `format!("{:?}", x).to_lowercase()` with stable mapping.

**Acceptance**
- Changing Debug formatting does not change persisted semantic definitions.

---

## Phase 7 — Remove in-place snapshot mutation in xtask backfill

### 7.1 Replace `UPDATE snapshots SET security_label=...` with successor publish
- [ ] In `rust/xtask/src/sem_reg.rs`:
  - For each snapshot missing label:
    - [ ] Publish a successor snapshot that copies the definition and sets `security_label`
    - [ ] Predecessor becomes superseded
  - [ ] If this is intended as a one-time migration: document it in the xtask help text.

**Acceptance**
- Snapshots remain immutable in principle: updates occur via successor snapshots only.

---

## Phase 8 — Tests (minimal but meaningful)

### 8.1 Unit tests
- [ ] `sem_reg::ids` deterministic UUID v5 test:
  - same input → same output
  - different object_type → different output
- [ ] publish atomicity test:
  - simulate failure between supersede and insert (or use tx rollback) → predecessor remains active
- [ ] pagination test:
  - seed > 1000 snapshots and ensure context resolution returns all

### 8.2 MCP integration tests (smoke)
- [ ] Start MCP server in test mode (or directly call handler functions)
- [ ] Assert `tools/list` includes `semreg.describe`
- [ ] Call `tools/call semreg.describe` and validate response shape

### 8.3 ABAC enforcement tests
- [ ] Create a snapshot labeled `NoLlmExternal`
- [ ] Call tool as unprivileged actor → redacted/denied
- [ ] Call tool as privileged actor → allowed

---

## Implementation Notes / Pitfalls

- Keep tool names stable. Client integrations will hardcode them.
- Decide *one* redaction strategy (omit vs redacted stub) and apply consistently.
- Prefer canonical JSON hashing (stable ordering) before hashing for `definition_hash`.
- If you add the partial unique index, ensure your publish path is compatible (tx order matters).

---

## “Done” Checklist

- [ ] `cargo test` passes
- [ ] MCP server lists SemReg tools
- [ ] SemReg tool calls work end-to-end
- [ ] ABAC is enforced everywhere
- [ ] Publishing is atomic + DB prevents multiple actives
- [ ] Scanner is deterministic + drift publishing works
- [ ] Context resolution no longer truncates
- [ ] No in-place snapshot mutation in xtask

