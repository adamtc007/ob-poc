# CLAUDE CODE SESSION: IMPL-PHASE-5A-INCLUSIVE-GATEWAY (v0.1.2)

You are implementing the Inclusive Gateway (OR gateway) for the bpmn-lite engine.
Phases 1 (Race), 2 (Boundary Timer), 3 (Cancel Pending), 2A (Non-interrupting + Cycles), 5.1 (Terminate), 5.2 (Error Routing), 5.3 (Bounded Loops), and 4 (PostgresProcessStore) are complete and passing.

## Instructions

Read `IMPL-PHASE-5A-INCLUSIVE-GATEWAY.md` in the project root. It contains:
- Section A: Context (dynamic fork/join for conditional parallel work)
- Section B: 13 design decisions
- Section C: 8 edit steps
- Section D: 6 test implementations
- Section E-G: Verification, done signal, scope boundary

## What inclusive gateway does

BPMN inclusive gateway = "evaluate conditions on each outgoing flow, spawn a fiber for every truthy condition, wait for all spawned branches to converge." Unlike AND gateway (static fork count), the branch count is determined at runtime from orchestration flags.

Pattern: "Always verify identity. If high_risk, also run EDD. If pep_flagged, also run PEP screening. Wait for all started checks."

## 5 non-negotiable constraints

1. **ForkInclusive is a new opcode, NOT a modification of Fork.** Existing Fork (AND gateway) is untouched. Zero regression risk for parallel gateways.
2. **JoinDynamic is a new opcode, NOT a modification of Join.** Reads `expected` from `instance.join_expected[id]` instead of bytecode. Existing Join untouched.
3. **join_expected on ProcessInstance.** `join_expected: BTreeMap<JoinId, u16>`. Written by ForkInclusive at runtime. Read by JoinDynamic. Persisted via save_instance (already in place from Phase 5.3).
4. **Zero conditions matched + no default → incident.** ForkInclusive creates an Incident with ContractViolation, sets instance Failed, returns Parked. If default_target is Some, it spawns one fiber to the default.
5. **Lowering must pre-scan** for converging inclusive gateways to allocate join_ids BEFORE processing diverging gateways (diverging needs join_id, converging allocates it, but topo order visits diverging first).

## Execution plan — 8 steps, do ALL of them

1. **types.rs** — `InclusiveBranch` struct, `ForkInclusive` + `JoinDynamic` on Instr, `join_expected: BTreeMap<JoinId, u16>` on ProcessInstance. Fix ALL construction sites and match arms.
2. **ir.rs** — `GatewayInclusive { id, name, direction }` on IRNode. Fix `id()` method and all IRNode match sites.
3. **events.rs** — `InclusiveForkTaken { gateway_id, branches_taken, join_id, expected }` on RuntimeEvent.
4. **vm.rs** — ForkInclusive handler (evaluate conditions, set join_expected, spawn fibers, delete parent) + JoinDynamic handler (read expected from instance, arrive, release or consume).
5. **parser.rs** — Remove `inclusiveGateway` from forbidden list if present. Parse like `parallelGateway` with direction detection. Create `IRNode::GatewayInclusive`. Handle `default` attribute.
6. **lowering.rs** — Pre-scan converging inclusive gateways → allocate join_ids. Diverging: build InclusiveBranch per edge, emit ForkInclusive. Converging: emit JoinDynamic. Update estimate_instr_count.
7. **verifier.rs** — Inclusive gateway rules: diverging ≥2 outgoing, converging ≥2 incoming + 1 outgoing.
8. **Tests** — 6 T-IG tests from Section D.

## CRITICAL: Step 1 — ProcessInstance + Instr cascade

Adding `join_expected` to ProcessInstance and two opcodes to Instr breaks construction sites everywhere.

```bash
grep -rn "ProcessInstance {" bpmn-lite-core/src/
grep -rn "Instr::" bpmn-lite-core/src/
```

- Every `ProcessInstance { ... }` needs `join_expected: BTreeMap::new()`
- Every exhaustive `match instr` needs ForkInclusive and JoinDynamic arms

**Run `cargo check` after Step 1. Fix ALL errors before proceeding.**

**ALSO run `cargo check --features postgres` — Phase 4 (PostgresProcessStore) is now in the codebase and must compile cleanly with the new ProcessInstance fields.**

**NOTE:** Phase 4's `store_postgres.rs` was written expecting `join_expected` on ProcessInstance and the `process_instances` table already has a `join_expected JSONB` column. If `join_expected` was already added to ProcessInstance during Phase 4 implementation, Step 1 may only need to verify it exists (not add it). Check first:
```bash
grep -n "join_expected" bpmn-lite-core/src/types.rs
```

## CRITICAL: ForkInclusive handler — incident path

When zero conditions match and default_target is None, the handler must:
1. Create an Incident (ContractViolation)
2. Save incident to store
3. Emit IncidentCreated event
4. Set fiber.wait = WaitState::Incident
5. Save fiber
6. Set instance.state = ProcessState::Failed
7. Return Ok(TickOutcome::Parked(...))

Check how fail_job creates incidents for the pattern:
```bash
grep -B5 -A20 "Incident {" bpmn-lite-core/src/engine.rs | head -40
```

The incident `service_task_id` should use the gateway's BPMN element ID from `program.debug_map.get(&fiber.pc)`, falling back to `format!("inclusive_fork_pc_{}", fiber.pc)` if not found. The ForkInclusive handler needs access to `program` (the CompiledProgram) for this lookup.

The incident requires: incident_id, process_instance_id, fiber_id, service_task_id, bytecode_addr, error_class, message, retry_count, created_at, resolved_at, resolution. Check the Incident struct:
```bash
grep -B2 -A15 "pub struct Incident" bpmn-lite-core/src/types.rs
```

## CRITICAL: ForkInclusive — condition evaluation

Conditions use instance.flags. An unset flag defaults to false. The is_truthy helper:
- `Value::Bool(true)` → true
- `Value::Bool(false)` → false
- `Value::I64(n)` → n != 0
- Anything else → false

Check if is_truthy exists or define it inline:
```bash
grep -rn "is_truthy\|truthy" bpmn-lite-core/src/
```

If it doesn't exist, define it in vm.rs:
```rust
fn is_truthy(val: &Value) -> bool {
    match val {
        Value::Bool(b) => *b,
        Value::I64(n) => *n != 0,
        _ => false,
    }
}
```

## CRITICAL: ForkInclusive — parent fiber deletion pattern

ForkInclusive follows the same pattern as Fork: spawn children, delete parent fiber, return TickOutcome::Ended. The parent fiber does NOT advance to join — only children arrive at JoinDynamic.

## CRITICAL: ForkInclusive — InclusiveForkTaken.gateway_id

The `gateway_id` in the `InclusiveForkTaken` event MUST use the same debug_map lookup as the incident path — NOT a raw `format!("pc_{}", fiber.pc)`. Use:
```rust
let gateway_id = program.debug_map.get(&fiber.pc)
    .cloned()
    .unwrap_or_else(|| format!("inclusive_fork_pc_{}", fiber.pc));
```
This makes audit logs human-meaningful (shows BPMN element ID like "ig_fork" instead of "pc_3").

## CRITICAL: JoinDynamic — mirrors Join but THREE key differences

1. `expected` comes from `instance.join_expected.get(&id)` instead of the instruction field
2. **Non-release path: do NOT save the fiber before deleting it.** Just delete_fiber and return Ended. The save-then-delete pattern cancels itself out. Barrier state is represented by join_arrive counter + events, not stored parked fibers.
3. **Release path: event ordering matters for PITR.** The sequence MUST be:
   - join_arrive → append JoinArrived
   - if count >= expected:
     - join_reset
     - `instance.join_expected.remove(&id)` (clean up key)
     - append JoinReleased
     - THEN set fiber.pc = next (after event is recorded)
   - if count < expected:
     - delete_fiber (no save)
     - return Ended

```rust
Instr::JoinDynamic { id, next } => {
    let expected = instance.join_expected.get(id).copied()
        .ok_or_else(|| anyhow!("JoinDynamic: no expected count for join_id {}", id))?;
    let count = self.store.join_arrive(instance.instance_id, *id).await?;
    self.store.append_event(instance.instance_id,
        &RuntimeEvent::JoinArrived { join_id: *id, fiber_id: fiber.fiber_id },
    ).await?;
    if count >= expected {
        self.store.join_reset(instance.instance_id, *id).await?;
        instance.join_expected.remove(id);
        self.store.append_event(instance.instance_id,
            &RuntimeEvent::JoinReleased { join_id: *id, next_pc: *next, released_fiber_id: fiber.fiber_id },
        ).await?;
        fiber.pc = *next;
        fiber.wait = WaitState::Running;
        Ok(TickOutcome::Continue)
    } else {
        self.store.delete_fiber(instance.instance_id, fiber.fiber_id).await?;
        Ok(TickOutcome::Ended)
    }
}
```

## CRITICAL: Lowering — pre-scan for join_id allocation

The diverging inclusive gateway needs the join_id that belongs to the converging gateway. But topo order visits diverging first. Solution:

```rust
// Before main emission loop, pre-allocate join_ids for converging inclusive gateways:
let mut inclusive_join_ids: HashMap<String, JoinId> = HashMap::new();
for &node_idx in &topo_order {
    if let IRNode::GatewayInclusive { direction: GatewayDirection::Converging, id, .. } = &graph[node_idx] {
        let jid = join_id_counter;
        join_id_counter += 1;
        inclusive_join_ids.insert(id.clone(), jid);
    }
}
```

Then when emitting the diverging gateway, find the paired converging gateway via graph traversal (BFS from fork, find first downstream GatewayInclusive Converging). Since the verifier enforces single inclusive pair per process, there's only one possible match — no ambiguity.

Always emit `default_target: None` in ForkInclusive (default flow parsing not in scope for 5A).

**Explicit clarification:** `default_target` exists in the `ForkInclusive` opcode struct so that hand-constructed bytecode tests can test default flow behavior. But the **compiler** (parser + lowering) MUST always emit `default_target: None` in Phase 5A. Do NOT implement BPMN `default` attribute parsing — that is a future enhancement.

## CRITICAL: Lowering — finding the paired converging gateway

For the diverging inclusive gateway, you need to find which converging inclusive gateway it pairs with. Strategy:

```rust
// BFS from diverging gateway to find the first converging inclusive gateway downstream
fn find_paired_join(graph: &IRGraph, fork_idx: NodeIndex) -> Option<String> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    for succ in graph.neighbors(fork_idx) {
        queue.push_back(succ);
    }
    while let Some(node) = queue.pop_front() {
        if !visited.insert(node) { continue; }
        if let IRNode::GatewayInclusive { direction: GatewayDirection::Converging, id, .. } = &graph[node] {
            return Some(id.clone());
        }
        for succ in graph.neighbors(node) {
            queue.push_back(succ);
        }
    }
    None
}
```

Then look up the join_id from inclusive_join_ids map.

## CRITICAL: Lowering — diverging inclusive gateway branches

For each outgoing edge from the diverging gateway:
- Edge has condition → `InclusiveBranch { condition_flag: Some(intern_flag(condition.flag_name)), target }`
- Edge has NO condition → `InclusiveBranch { condition_flag: None, target }` (unconditional)

Use the same flag interning mechanism as XOR gateway (`intern_flag` or `flag_intern` map).

## CRITICAL: Parser — inclusiveGateway direction detection

Use the same pattern as parallelGateway:
```rust
"inclusiveGateway" if *in_process => {
    let id = get_attr(e, "id")?;
    let name = get_attr_opt(e, "name").unwrap_or_default();
    let dir_str = get_attr_opt(e, "gatewayDirection").unwrap_or_default();
    let direction = match dir_str.as_str() {
        "Converging" => GatewayDirection::Converging,
        _ => GatewayDirection::Diverging,
    };
    let idx = graph.add_node(IRNode::GatewayInclusive { id: id.clone(), name, direction });
    element_map.insert(id, idx);
}
```

**Also check if inclusiveGateway is in the forbidden/unsupported list and remove it:**
```bash
grep -n "inclusiveGateway" bpmn-lite-core/src/compiler/parser.rs
```

Note: `inclusiveGateway` is NOT in the current forbidden list (which has: scriptTask, businessRuleTask, sendTask, receiveTask, manualTask, subProcess, callActivity, eventBasedGateway, complexGateway). It falls through to the `_ => {}` catch-all, meaning it's silently ignored today. **This is dangerous** — a BPMN with inclusiveGateway would "compile" but produce a broken graph with dangling flows.

**IMPORTANT:** Add the parsing BEFORE (or in the same edit as) any other changes that touch the parser. Do NOT leave a window where inclusiveGateway is neither forbidden nor parsed. The existing verifier rule "all nodes reachable from Start" provides a safety net, but explicit parsing is the fix.

## CRITICAL: Condition expression parsing consistency

Inclusive gateway condition parsing MUST use the exact same rules as XOR gateway. Check how XOR currently parses `<conditionExpression>`:
```bash
grep -B5 -A15 "conditionExpression\|condition.*flag" bpmn-lite-core/src/compiler/parser.rs
```
The format is typically `= flag_name` (prefix `=` then flag key). Inclusive gateway edges use the same format. Do NOT invent a new condition syntax.

## CRITICAL: Verifier — GatewayInclusive rules

Add to the existing verify() function, mirroring GatewayAnd checks:

```rust
IRNode::GatewayInclusive { direction: GatewayDirection::Diverging, .. } => {
    let outgoing = graph.edges_directed(idx, Direction::Outgoing).count();
    if outgoing < 2 {
        errors.push(VerifyError {
            message: format!("Inclusive gateway (diverging) must have ≥2 outgoing edges, found {}", outgoing),
            element_id: Some(graph[idx].id().to_string()),
        });
    }
}
IRNode::GatewayInclusive { direction: GatewayDirection::Converging, .. } => {
    let incoming = graph.edges_directed(idx, Direction::Incoming).count();
    let outgoing = graph.edges_directed(idx, Direction::Outgoing).count();
    if incoming < 2 {
        errors.push(VerifyError {
            message: format!("Inclusive gateway (converging) must have ≥2 incoming edges, found {}", incoming),
            element_id: Some(graph[idx].id().to_string()),
        });
    }
    if outgoing != 1 {
        errors.push(VerifyError {
            message: format!("Inclusive gateway (converging) must have exactly 1 outgoing edge, found {}", outgoing),
            element_id: Some(graph[idx].id().to_string()),
        });
    }
}
```

**ALSO add single-pair enforcement (v1 constraint):**
After the per-node checks, count inclusive gateways:

```rust
let diverging_inclusive_count = graph.node_indices()
    .filter(|&idx| matches!(&graph[idx], IRNode::GatewayInclusive { direction: GatewayDirection::Diverging, .. }))
    .count();
let converging_inclusive_count = graph.node_indices()
    .filter(|&idx| matches!(&graph[idx], IRNode::GatewayInclusive { direction: GatewayDirection::Converging, .. }))
    .count();
if diverging_inclusive_count > 1 {
    errors.push(VerifyError {
        message: format!("Multiple diverging inclusive gateways not supported (found {})", diverging_inclusive_count),
        element_id: None,
    });
}
if converging_inclusive_count > 1 {
    errors.push(VerifyError {
        message: format!("Multiple converging inclusive gateways not supported (found {})", converging_inclusive_count),
        element_id: None,
    });
}
```

**Note:** All-unconditional outgoing flows on inclusive gateway are valid (just unusual). No error or warning emitted.

## CRITICAL: estimate_instr_count for GatewayInclusive

```rust
IRNode::GatewayInclusive { .. } => 1,  // ForkInclusive or JoinDynamic
```

## CRITICAL: engine.start() takes 5 parameters

```rust
engine.start(process_key, bytecode_version, domain_payload, domain_payload_hash, correlation_id)
```

## CRITICAL: Test T-IG-1 sets flags BEFORE ticking

T-IG-1 starts the instance, then loads it, sets flags, saves, THEN ticks. This is because start() creates the instance with empty flags. The ForkInclusive evaluates flags during tick.

## CRITICAL: Test T-IG-6 is a full compiler pipeline test

T-IG-6 parses BPMN XML → IR → verify → lower → bytecode. It tests that the compiler correctly produces ForkInclusive and JoinDynamic instructions from `<inclusiveGateway>` elements.

## CRITICAL: compute_hash is pub(crate) — accessible from same-crate tests

Tests call `crate::vm::compute_hash(payload)`. This is `pub(crate)` in vm.rs. Since tests are `#[cfg(test)]` modules within `bpmn-lite-core`, `pub(crate)` IS accessible. Do NOT change its visibility or replace it with `[0u8; 32]` in tests that need accurate hashes for complete_job.

## CRITICAL: join_expected cleanup assertion

Tests T-IG-1 and T-IG-5 assert `inst.join_expected.is_empty()` after completion. This catches key leakage if JoinDynamic forgets `instance.join_expected.remove(&id)` on release.

## CRITICAL: Job ordering in tests is NOT guaranteed

Job dequeue order from run_instance is nondeterministic. Tests that complete multiple jobs (T-IG-1, T-IG-5) use `for job in &jobs` loops or complete by index — this is fine because the test logic doesn't depend on which specific job is at which index (both need completing). But do NOT add assertions that assume `jobs[0]` is a specific task type.

## CRITICAL: Do not stop early

Progress gates:
- Step 1 (types.rs compiles) → 12% → IMMEDIATELY proceed to Step 2
- Step 2 (ir.rs) → 20% → IMMEDIATELY proceed to Step 3
- Step 3 (events.rs) → 25% → IMMEDIATELY proceed to Step 4
- Step 4 (vm.rs) → 50% → IMMEDIATELY proceed to Step 5
- Step 5 (parser.rs) → 65% → IMMEDIATELY proceed to Step 6
- Step 6 (lowering.rs) → 80% → IMMEDIATELY proceed to Step 7
- Step 7 (verifier.rs) → 90% → IMMEDIATELY add tests
- Tests added → 95% → Run `cargo test -p bpmn-lite-core`
- All green → 100% → Print DONE signal

## Verification

```bash
cargo test -p bpmn-lite-core 2>&1
```

All existing (71 core + 6 server) must pass. Plus:
```bash
cargo test -p bpmn-lite-core -- t_ig 2>&1
```
Expected: `test result: ok. 6 passed`

Also verify Postgres feature compiles:
```bash
cargo check --features postgres 2>&1
```

## Constraints

- Do NOT modify existing Fork or Join opcodes
- Do NOT modify gRPC proto
- Do NOT add new files (7 files modified only)
- Do NOT add new WaitState variants (reuse existing Incident for zero-match case)
- Do NOT parse BPMN `default` attribute (compiler always emits `default_target: None`)
- join_expected lives on ProcessInstance (persisted via save_instance, already in place)
- join_expected[id] removed on join release (cleanup)
- ForkInclusive deletes parent fiber (same as Fork)
- JoinDynamic non-release path: delete fiber directly (NO save-then-delete)
- JoinDynamic release path: join_reset + JoinReleased event BEFORE advancing fiber.pc
- Unconditional branches use `condition_flag: None`
- All-unconditional outgoing flows are valid (no verifier error)
- Verifier enforces single inclusive pair per process (v1)
- `compute_hash` is `pub(crate)` in vm.rs
- Follow existing style: anyhow::Result, async_trait, Uuid::now_v7()
