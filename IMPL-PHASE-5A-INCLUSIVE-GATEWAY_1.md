# IMPL-PHASE-5A: Inclusive Gateway (v0.1.2)

**Session scope:** Add BPMN inclusive gateway (OR gateway). Diverging: evaluate conditions on outgoing flows, spawn a fiber per truthy condition. Converging: wait for all actually-spawned branches, then release. Dynamic fork/join where branch count is determined at runtime.
**Estimated edits:** 7 files modified, 0 new files.
**Prerequisite:** Phases 1–3, 2A, 5.1, 5.2, 5.3, 4 (PostgresProcessStore) complete.
**Verification:** `cargo test -p bpmn-lite-core` all green + 6 new `T-IG-*` tests passing.

---

## A) CONTEXT — Why this matters

**AND gateway (existing):** Fork spawns ALL branches. Join expects static count (known at compile time). "Do A and B and C, wait for all three."

**XOR gateway (existing):** Evaluate conditions, take first matching branch. One path only. "If high_risk do A, else do B."

**Inclusive gateway (new):** Evaluate conditions on each outgoing flow. Spawn fibers for ALL truthy conditions. Join waits for however many were actually spawned. "Always do identity check. If high_risk, also do EDD. If pep_flagged, also do PEP screening. Wait for all started checks."

The inclusive gateway is the natural pattern for conditional parallel work in onboarding — the number of parallel tracks depends on entity attributes determined at runtime.

### Existing mechanisms

```
Fork:     spawns N children (static N), deletes parent fiber, returns Ended
Join:     join_arrive → if count >= expected (static) → release, else delete fiber + Ended
FlagKey:  interned string → u32, used for condition evaluation
```

### Why not reuse Fork + Join

Fork has no condition evaluation — it always spawns ALL branches. Join has static `expected` baked into bytecode. We need dynamic fork (conditional) + dynamic join (runtime expected).

---

## B) DESIGN DECISIONS (resolved — do not deviate)

### B1. InclusiveBranch struct

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InclusiveBranch {
    pub condition_flag: Option<FlagKey>,  // None = unconditional (always taken)
    pub target: Addr,
}
```

`None` means "always take this branch" — the unconditional flow. This handles: "always verify identity (no condition), conditionally run EDD (if flag 3 is truthy), conditionally run PEP (if flag 7 is truthy)."

### B2. New opcode: ForkInclusive

```rust
ForkInclusive {
    branches: Box<[InclusiveBranch]>,
    join_id: JoinId,
    default_target: Option<Addr>,
},
```

- `branches`: evaluated in order, all truthy conditions spawn fibers
- `join_id`: the converging inclusive gateway's join barrier ID
- `default_target`: fallback if zero conditions match (BPMN `default` flow). If None and zero match → incident.

### B3. New opcode: JoinDynamic

```rust
JoinDynamic {
    id: JoinId,
    next: Addr,
},
```

Same semantics as `Join` but reads `expected` from `instance.join_expected[id]` instead of bytecode. Keeps existing `Join` opcode untouched — zero regression risk for AND gateways.

### B4. ProcessInstance.join_expected

```rust
pub struct ProcessInstance {
    // ... existing fields ...
    pub join_expected: BTreeMap<JoinId, u16>,
}
```

Written by ForkInclusive at runtime. Read by JoinDynamic. Not touched by Fork/Join (AND gateways).

All existing ProcessInstance constructions need `join_expected: BTreeMap::new()`.

### B5. VM handler: ForkInclusive

```rust
Instr::ForkInclusive { branches, join_id, default_target } => {
    let mut taken_targets = Vec::new();

    for branch in branches.iter() {
        let take = match branch.condition_flag {
            None => true,  // unconditional — always taken
            Some(key) => {
                let val = instance.flags.get(&key).cloned()
                    .unwrap_or(Value::Bool(false));
                is_truthy(&val)
            }
        };
        if take {
            taken_targets.push(branch.target);
        }
    }

    // Zero conditions matched
    if taken_targets.is_empty() {
        if let Some(default) = default_target {
            taken_targets.push(*default);
        } else {
            // No default — incident
            let incident_id = Uuid::now_v7();
            let gateway_id = program.debug_map.get(&fiber.pc)
                .cloned()
                .unwrap_or_else(|| format!("inclusive_fork_pc_{}", fiber.pc));
            let incident = Incident {
                incident_id,
                process_instance_id: instance.instance_id,
                fiber_id: fiber.fiber_id,
                service_task_id: gateway_id.clone(),
                bytecode_addr: fiber.pc,
                error_class: ErrorClass::ContractViolation,
                message: "Inclusive gateway: no conditions matched and no default flow".into(),
                retry_count: 0,
                created_at: now_ms(),
                resolved_at: None,
                resolution: None,
            };
            self.store.save_incident(&incident).await?;
            self.store.append_event(instance.instance_id,
                &RuntimeEvent::IncidentCreated {
                    incident_id,
                    service_task_id: gateway_id,
                    job_key: None,
                },
            ).await?;
            fiber.wait = WaitState::Incident { incident_id };
            self.store.save_fiber(instance.instance_id, fiber).await?;
            instance.state = ProcessState::Failed { incident_id };
            return Ok(TickOutcome::Parked(WaitState::Incident { incident_id }));
        }
    }

    // Set dynamic join expected
    instance.join_expected.insert(*join_id, taken_targets.len() as u16);

    // Spawn fibers
    let mut child_ids = Vec::new();
    for &target in &taken_targets {
        let child_id = Uuid::now_v7();
        let child = Fiber::new(child_id, target);
        self.store.save_fiber(instance.instance_id, &child).await?;
        self.store.append_event(instance.instance_id,
            &RuntimeEvent::FiberSpawned {
                fiber_id: child_id,
                pc: target,
                parent: Some(fiber.fiber_id),
            },
        ).await?;
        child_ids.push(child_id);
    }

    // Emit InclusiveForkTaken event
    let gateway_id = program.debug_map.get(&fiber.pc)
        .cloned()
        .unwrap_or_else(|| format!("inclusive_fork_pc_{}", fiber.pc));
    self.store.append_event(instance.instance_id,
        &RuntimeEvent::InclusiveForkTaken {
            gateway_id,
            branches_taken: taken_targets.clone(),
            join_id: *join_id,
            expected: taken_targets.len() as u16,
        },
    ).await?;

    // Delete parent fiber (same pattern as Fork)
    self.store.delete_fiber(instance.instance_id, fiber.fiber_id).await?;
    Ok(TickOutcome::Ended)
}
```

### B6. VM handler: JoinDynamic

```rust
Instr::JoinDynamic { id, next } => {
    let expected = instance.join_expected.get(id).copied()
        .ok_or_else(|| anyhow!("JoinDynamic: no expected count for join_id {}", id))?;

    let count = self.store.join_arrive(instance.instance_id, *id).await?;

    self.store.append_event(instance.instance_id,
        &RuntimeEvent::JoinArrived {
            join_id: *id,
            fiber_id: fiber.fiber_id,
        },
    ).await?;

    if count >= expected {
        // All branches arrived — release
        self.store.join_reset(instance.instance_id, *id).await?;
        instance.join_expected.remove(id);  // clean up dynamic expected
        self.store.append_event(instance.instance_id,
            &RuntimeEvent::JoinReleased {
                join_id: *id,
                next_pc: *next,
                released_fiber_id: fiber.fiber_id,
            },
        ).await?;
        // Advance pc AFTER event is recorded (PITR determinism)
        fiber.pc = *next;
        fiber.wait = WaitState::Running;
        Ok(TickOutcome::Continue)
    } else {
        // Wait for more — consume this fiber (do NOT save before delete)
        self.store.delete_fiber(instance.instance_id, fiber.fiber_id).await?;
        Ok(TickOutcome::Ended)
    }
}
```

This mirrors the existing Join handler, but reads `expected` from instance state. Two fixes from peer review:
- **Non-release path:** fiber is consumed directly (no save-then-delete, which cancels itself out). Barrier state is represented by join_arrive counter + events, not by stored parked fibers.
- **Release path:** join_reset + JoinReleased event are appended BEFORE fiber.pc advances. This keeps point-in-time recovery deterministic.
- **Cleanup:** join_expected.remove(&id) on release prevents key leakage over process lifetime.

### B7. IRNode::GatewayInclusive

```rust
GatewayInclusive {
    id: String,
    name: String,
    direction: GatewayDirection,
},
```

Mirrors GatewayAnd structure. Fix `id()` method and all match sites.

### B8. RuntimeEvent::InclusiveForkTaken

```rust
InclusiveForkTaken {
    gateway_id: String,
    branches_taken: Vec<Addr>,
    join_id: JoinId,
    expected: u16,
},
```

Records which branches were taken and the dynamic join count for audit/replay. `gateway_id` is resolved from `program.debug_map.get(&fiber.pc)` with fallback to `format!("inclusive_fork_pc_{}", fiber.pc)` — same pattern as the incident path. This ensures audit logs show the BPMN element ID, not raw PC addresses.

### B9. Parser: parse `<inclusiveGateway>`

**Remove from forbidden list** (if present — check if parser has `inclusiveGateway` in the error list).

**Parse like parallelGateway but with gatewayDirection:**
```xml
<bpmn:inclusiveGateway id="ig1" gatewayDirection="Diverging"/>
```

Create `IRNode::GatewayInclusive { id, name, direction }`. Same direction detection as parallelGateway.

**Conditions on outgoing flows:** Already parsed for XOR gateways. Inclusive gateway outgoing flows use the same `<bpmn:conditionExpression>` syntax. The parser already collects these as `IREdge { condition }`.

**Default flow:** The BPMN spec allows a `default` attribute on inclusive gateways. **Phase 5A does NOT parse this attribute.** The `default_target` field exists in the ForkInclusive opcode for direct bytecode construction (tests), but the compiler always emits `default_target: None`. Default flow parsing is a future enhancement. If zero conditions match at runtime, the engine creates an incident.

### B10. Lowering: ForkInclusive + JoinDynamic

**Diverging inclusive gateway:**

1. For each outgoing edge:
   - If edge has condition → intern flag key, build `InclusiveBranch { condition_flag: Some(key), target }`
   - If edge has no condition → `InclusiveBranch { condition_flag: None, target }` (unconditional)
2. Find the matching converging inclusive gateway's join_id (allocated at converging node)
3. Emit `Instr::ForkInclusive { branches, join_id, default_target: None }` (default parsing not in scope for 5A)

**Converging inclusive gateway:**

1. Allocate join_id (same counter as AND join)
2. Find sole outgoing successor
3. Emit `Instr::JoinDynamic { id: join_id, next }`

**Pairing diverging/converging:** The lowering needs to find the corresponding converging gateway for each diverging one. Strategy: when processing the converging gateway, allocate a join_id. When processing the diverging gateway, look ahead to find the converging gateway and use its join_id.

This is the same challenge AND gateways face. Looking at the current AND lowering: the diverging gateway emits Fork (no join_id needed), and the converging gateway emits Join (allocates join_id independently). Fork doesn't reference join_id.

For inclusive, the diverging gateway MUST reference join_id (to set dynamic expected). So we need a mapping: `inclusive_gateway_id → join_id`. The converging gateway allocates join_id. The diverging gateway reads it.

**Implementation:** Two-pass within lowering. First pass: process converging inclusive gateways, allocate join_ids, build map `gateway_id → join_id`. Second pass: process diverging inclusive gateways, look up join_id from map.

Actually, simpler: the lowering already does a topo-ordered pass. If the converging gateway has a known address before the diverging gateway emits code, we can look it up. But topo order visits nodes from start to end — the diverging gateway comes BEFORE the converging one.

**Solution:** Pre-allocate join_ids for all converging inclusive gateways in a pre-scan before instruction emission:

```rust
// Before main emission loop:
let mut inclusive_join_ids: HashMap<String, JoinId> = HashMap::new();
for &node_idx in &topo_order {
    if matches!(&graph[node_idx], IRNode::GatewayInclusive { direction: GatewayDirection::Converging, .. }) {
        let id = graph[node_idx].id().to_string();
        let join_id = join_id_counter;
        join_id_counter += 1;
        inclusive_join_ids.insert(id, join_id);
    }
}
```

Then during diverging gateway emission, the lowering needs to find which converging gateway this diverging gateway pairs with. BPMN doesn't explicitly pair them — it's structural. The convention: follow all outgoing paths from the diverging gateway until they reconverge at the same converging inclusive gateway.

**Simplification for v1:** The verifier enforces at most one diverging and one converging inclusive gateway per process (rule B11.4). This makes pairing unambiguous — there's only one possible pair. The BFS is still used for correctness (confirming the converging gateway IS downstream of the diverging one), but ambiguity is eliminated at compile time.

For multi-pair support (future), pairing would require explicit gateway correlation or structural analysis. Not in scope for 5A.

### B11. Verifier rules

1. GatewayInclusive Diverging must have ≥ 2 outgoing edges
2. GatewayInclusive Converging must have ≥ 2 incoming edges
3. GatewayInclusive Converging must have exactly 1 outgoing edge
4. **Single inclusive pair per process (v1 constraint):** at most 1 diverging inclusive gateway and at most 1 converging inclusive gateway per process. If multiple exist → compile error. This makes the diverging↔converging pairing in lowering unambiguous and safe. Nested or multi-pair inclusive gateways are a future extension.
5. All-unconditional outgoing flows are valid (just unusual — effectively an AND gateway). No error or warning emitted.

### B12. estimate_instr_count

```rust
IRNode::GatewayInclusive { .. } => 1,  // ForkInclusive or JoinDynamic
```

### B13. Persist join_expected

Phase 5.3 already switched tick_instance to call `save_instance(&instance)` which persists the full ProcessInstance. This automatically covers join_expected. No additional persistence work.

---

## C) EDIT SEQUENCE (execute in this exact order)

### Step 1: types.rs — InclusiveBranch, ForkInclusive, JoinDynamic, join_expected

**Add InclusiveBranch struct:**
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InclusiveBranch {
    pub condition_flag: Option<FlagKey>,
    pub target: Addr,
}
```

**Add to Instr enum:**
```rust
    ForkInclusive {
        branches: Box<[InclusiveBranch]>,
        join_id: JoinId,
        default_target: Option<Addr>,
    },
    JoinDynamic {
        id: JoinId,
        next: Addr,
    },
```

**Add to ProcessInstance:**
```rust
    pub join_expected: BTreeMap<JoinId, u16>,
```

**Fix ALL construction sites:**
```bash
grep -rn "ProcessInstance {" bpmn-lite-core/src/
grep -rn "Instr::" bpmn-lite-core/src/
```

ProcessInstance constructions need `join_expected: BTreeMap::new()`.
Exhaustive Instr matches need ForkInclusive and JoinDynamic arms.

### Step 2: ir.rs — GatewayInclusive node

```rust
GatewayInclusive {
    id: String,
    name: String,
    direction: GatewayDirection,
},
```

Fix `id()` method. Fix all IRNode match sites.

### Step 3: events.rs — InclusiveForkTaken event

```rust
InclusiveForkTaken {
    gateway_id: String,
    branches_taken: Vec<Addr>,
    join_id: JoinId,
    expected: u16,
},
```

### Step 4: vm.rs — ForkInclusive + JoinDynamic handlers

Add ForkInclusive handler per B5.
Add JoinDynamic handler per B6.

### Step 5: parser.rs — Parse inclusiveGateway

Remove from forbidden list if present. Parse like parallelGateway with direction detection. Create `IRNode::GatewayInclusive`.

### Step 6: lowering.rs — Emit ForkInclusive + JoinDynamic

**Pre-scan:** allocate join_ids for converging inclusive gateways.
**Diverging:** build InclusiveBranch per outgoing edge, find paired join_id, emit ForkInclusive.
**Converging:** emit JoinDynamic with pre-allocated join_id.
**estimate_instr_count:** return 1 for GatewayInclusive.

### Step 7: verifier.rs — Inclusive gateway rules

Diverging: ≥2 outgoing. Converging: ≥2 incoming, exactly 1 outgoing.

### Step 8: Tests — see Section D.

---

## D) TESTS

```rust
    // ═══════════════════════════════════════════════════════════
    //  Phase 5A: Inclusive gateway
    // ═══════════════════════════════════════════════════════════

    /// T-IG-1: All conditions truthy → all branches run → join waits for all → completes.
    #[tokio::test]
    async fn t_ig_1_all_branches_taken() {
        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        // Bytecode:
        // 0: ForkInclusive [branch(flag=None→2), branch(flag=0→4), branch(flag=1→6)] join_id=0
        // 1: (unreachable placeholder)
        // 2: ExecNative(identity_check)    — Branch A (unconditional)
        // 3: JoinDynamic(id=0, next=8)
        // 4: ExecNative(edd_check)         — Branch B (if high_risk)
        // 5: JoinDynamic(id=0, next=8)
        // 6: ExecNative(pep_screening)     — Branch C (if pep_flagged)
        // 7: JoinDynamic(id=0, next=8)
        // 8: End
        let program = CompiledProgram {
            bytecode_version: [70u8; 32],
            program: vec![
                Instr::ForkInclusive {
                    branches: Box::new([
                        InclusiveBranch { condition_flag: None, target: 2 },       // always
                        InclusiveBranch { condition_flag: Some(0), target: 4 },    // if high_risk
                        InclusiveBranch { condition_flag: Some(1), target: 6 },    // if pep_flagged
                    ]),
                    join_id: 0,
                    default_target: None,
                },
                Instr::End,   // 1: placeholder (never reached)
                Instr::ExecNative { task_type: 0, argc: 0, retc: 0 },  // 2: identity_check
                Instr::JoinDynamic { id: 0, next: 8 },                  // 3
                Instr::ExecNative { task_type: 1, argc: 0, retc: 0 },  // 4: edd_check
                Instr::JoinDynamic { id: 0, next: 8 },                  // 5
                Instr::ExecNative { task_type: 2, argc: 0, retc: 0 },  // 6: pep_screening
                Instr::JoinDynamic { id: 0, next: 8 },                  // 7
                Instr::End,                                              // 8: done
            ],
            debug_map: BTreeMap::from([
                (2, "identity_check".to_string()),
                (4, "edd_check".to_string()),
                (6, "pep_screening".to_string()),
            ]),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec![
                "identity_check".to_string(),
                "edd_check".to_string(),
                "pep_screening".to_string(),
            ],
            error_route_map: BTreeMap::new(),
        };
        store.store_program(program.bytecode_version, &program).await.unwrap();

        // Start with both flags true → all 3 branches taken
        let instance_id = engine
            .start("test", program.bytecode_version, "{}", [0u8; 32], "corr-1")
            .await.unwrap();

        // Set flags before first tick
        let mut inst = store.load_instance(instance_id).await.unwrap().unwrap();
        inst.flags.insert(0, Value::Bool(true));   // high_risk
        inst.flags.insert(1, Value::Bool(true));   // pep_flagged
        store.save_instance(&inst).await.unwrap();

        // Tick → ForkInclusive evaluates: all 3 taken → 3 fibers spawned
        engine.tick_instance(instance_id).await.unwrap();

        // Assert: InclusiveForkTaken event with expected=3
        let events = store.read_events(instance_id, 0).await.unwrap();
        let fork_event = events.iter().find(|(_, e)| matches!(e, RuntimeEvent::InclusiveForkTaken { .. }));
        assert!(fork_event.is_some(), "Should emit InclusiveForkTaken");

        // Assert: join_expected[0] = 3
        let inst = store.load_instance(instance_id).await.unwrap().unwrap();
        assert_eq!(inst.join_expected.get(&0), Some(&3));

        // Run → 3 jobs activated
        let jobs = engine.run_instance(instance_id).await.unwrap();
        assert_eq!(jobs.len(), 3, "All 3 branches should activate jobs");

        // Complete all 3 jobs
        for job in &jobs {
            let payload = "{}";
            let hash = crate::vm::compute_hash(payload);
            engine.complete_job(&job.job_key, payload, hash, BTreeMap::new()).await.unwrap();
        }

        // Tick until complete — fibers arrive at JoinDynamic, last one releases
        for _ in 0..5 {
            engine.tick_instance(instance_id).await.unwrap();
            let inst = store.load_instance(instance_id).await.unwrap().unwrap();
            if inst.state.is_terminal() { break; }
        }

        let inst = store.load_instance(instance_id).await.unwrap().unwrap();
        assert!(
            matches!(inst.state, ProcessState::Completed { .. }),
            "Expected Completed, got {:?}", inst.state
        );

        // Assert: join_expected cleaned up (no key leakage)
        assert!(inst.join_expected.is_empty(),
            "join_expected should be empty after join release, got {:?}", inst.join_expected);
    }    /// T-IG-2: Only 1 of 3 conditions truthy → 1 branch runs → join waits for 1 → immediate release.
    #[tokio::test]
    async fn t_ig_2_single_branch_taken() {
        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        // Same program as T-IG-1
        let program = CompiledProgram {
            bytecode_version: [71u8; 32],
            program: vec![
                Instr::ForkInclusive {
                    branches: Box::new([
                        InclusiveBranch { condition_flag: None, target: 2 },
                        InclusiveBranch { condition_flag: Some(0), target: 4 },
                        InclusiveBranch { condition_flag: Some(1), target: 6 },
                    ]),
                    join_id: 0,
                    default_target: None,
                },
                Instr::End,
                Instr::ExecNative { task_type: 0, argc: 0, retc: 0 },
                Instr::JoinDynamic { id: 0, next: 8 },
                Instr::ExecNative { task_type: 1, argc: 0, retc: 0 },
                Instr::JoinDynamic { id: 0, next: 8 },
                Instr::ExecNative { task_type: 2, argc: 0, retc: 0 },
                Instr::JoinDynamic { id: 0, next: 8 },
                Instr::End,
            ],
            debug_map: BTreeMap::from([
                (2, "identity_check".to_string()),
                (4, "edd_check".to_string()),
                (6, "pep_screening".to_string()),
            ]),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec![
                "identity_check".to_string(),
                "edd_check".to_string(),
                "pep_screening".to_string(),
            ],
            error_route_map: BTreeMap::new(),
        };
        store.store_program(program.bytecode_version, &program).await.unwrap();

        // Start with flags FALSE → only unconditional branch (A) taken
        let instance_id = engine
            .start("test", program.bytecode_version, "{}", [0u8; 32], "corr-2")
            .await.unwrap();

        // Flags default to false (not set) → only branch A (unconditional) taken

        // Tick → ForkInclusive: only branch A taken → 1 fiber
        engine.tick_instance(instance_id).await.unwrap();

        // Assert: join_expected[0] = 1
        let inst = store.load_instance(instance_id).await.unwrap().unwrap();
        assert_eq!(inst.join_expected.get(&0), Some(&1));

        // Run → 1 job
        let jobs = engine.run_instance(instance_id).await.unwrap();
        assert_eq!(jobs.len(), 1, "Only unconditional branch should spawn");

        // Complete job → JoinDynamic expects 1, 1 arrives → immediate release
        let payload = "{}";
        let hash = crate::vm::compute_hash(payload);
        engine.complete_job(&jobs[0].job_key, payload, hash, BTreeMap::new()).await.unwrap();

        for _ in 0..5 {
            engine.tick_instance(instance_id).await.unwrap();
            let inst = store.load_instance(instance_id).await.unwrap().unwrap();
            if inst.state.is_terminal() { break; }
        }

        let inst = store.load_instance(instance_id).await.unwrap().unwrap();
        assert!(matches!(inst.state, ProcessState::Completed { .. }));
    }

    /// T-IG-3: Zero conditions match, no default → incident.
    #[tokio::test]
    async fn t_ig_3_zero_match_no_default_incident() {
        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        // ALL branches conditional — no unconditional
        let program = CompiledProgram {
            bytecode_version: [72u8; 32],
            program: vec![
                Instr::ForkInclusive {
                    branches: Box::new([
                        InclusiveBranch { condition_flag: Some(0), target: 2 },
                        InclusiveBranch { condition_flag: Some(1), target: 4 },
                    ]),
                    join_id: 0,
                    default_target: None,  // no default!
                },
                Instr::End,
                Instr::ExecNative { task_type: 0, argc: 0, retc: 0 },
                Instr::JoinDynamic { id: 0, next: 6 },
                Instr::ExecNative { task_type: 1, argc: 0, retc: 0 },
                Instr::JoinDynamic { id: 0, next: 6 },
                Instr::End,
            ],
            debug_map: BTreeMap::new(),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec!["task_a".to_string(), "task_b".to_string()],
            error_route_map: BTreeMap::new(),
        };
        store.store_program(program.bytecode_version, &program).await.unwrap();

        let instance_id = engine
            .start("test", program.bytecode_version, "{}", [0u8; 32], "corr-3")
            .await.unwrap();
        // No flags set → all conditions false → zero match

        engine.tick_instance(instance_id).await.unwrap();

        // Assert: instance Failed with incident
        let inst = store.load_instance(instance_id).await.unwrap().unwrap();
        assert!(
            matches!(inst.state, ProcessState::Failed { .. }),
            "Zero match with no default should create incident, got {:?}", inst.state
        );

        let events = store.read_events(instance_id, 0).await.unwrap();
        let has_incident = events.iter().any(|(_, e)| matches!(e, RuntimeEvent::IncidentCreated { .. }));
        assert!(has_incident, "Should emit IncidentCreated");
    }

    /// T-IG-4: Zero conditions match WITH default → default branch runs.
    #[tokio::test]
    async fn t_ig_4_zero_match_with_default() {
        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        let program = CompiledProgram {
            bytecode_version: [73u8; 32],
            program: vec![
                Instr::ForkInclusive {
                    branches: Box::new([
                        InclusiveBranch { condition_flag: Some(0), target: 2 },
                    ]),
                    join_id: 0,
                    default_target: Some(4),  // default branch
                },
                Instr::End,
                Instr::ExecNative { task_type: 0, argc: 0, retc: 0 },  // 2: conditional
                Instr::JoinDynamic { id: 0, next: 6 },
                Instr::ExecNative { task_type: 1, argc: 0, retc: 0 },  // 4: default
                Instr::JoinDynamic { id: 0, next: 6 },
                Instr::End,                                              // 6: done
            ],
            debug_map: BTreeMap::from([
                (2, "conditional_task".to_string()),
                (4, "default_task".to_string()),
            ]),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec!["conditional_task".to_string(), "default_task".to_string()],
            error_route_map: BTreeMap::new(),
        };
        store.store_program(program.bytecode_version, &program).await.unwrap();

        let instance_id = engine
            .start("test", program.bytecode_version, "{}", [0u8; 32], "corr-4")
            .await.unwrap();
        // No flags → condition false → default taken

        engine.tick_instance(instance_id).await.unwrap();

        // Assert: join_expected = 1 (default branch only)
        let inst = store.load_instance(instance_id).await.unwrap().unwrap();
        assert_eq!(inst.join_expected.get(&0), Some(&1));

        // Run → should get default_task job
        let jobs = engine.run_instance(instance_id).await.unwrap();
        assert_eq!(jobs.len(), 1);

        // Complete and finish
        let payload = "{}";
        let hash = crate::vm::compute_hash(payload);
        engine.complete_job(&jobs[0].job_key, payload, hash, BTreeMap::new()).await.unwrap();
        for _ in 0..5 {
            engine.tick_instance(instance_id).await.unwrap();
            let inst = store.load_instance(instance_id).await.unwrap().unwrap();
            if inst.state.is_terminal() { break; }
        }

        let inst = store.load_instance(instance_id).await.unwrap().unwrap();
        assert!(matches!(inst.state, ProcessState::Completed { .. }));
    }

    /// T-IG-5: JoinDynamic releases only after dynamic expected count arrivals.
    #[tokio::test]
    async fn t_ig_5_join_waits_for_dynamic_count() {
        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        // 2 of 3 branches taken → join waits for exactly 2
        let program = CompiledProgram {
            bytecode_version: [74u8; 32],
            program: vec![
                Instr::ForkInclusive {
                    branches: Box::new([
                        InclusiveBranch { condition_flag: None, target: 2 },       // always
                        InclusiveBranch { condition_flag: Some(0), target: 4 },    // if flag_0
                        InclusiveBranch { condition_flag: Some(1), target: 6 },    // if flag_1 (false)
                    ]),
                    join_id: 0,
                    default_target: None,
                },
                Instr::End,
                Instr::ExecNative { task_type: 0, argc: 0, retc: 0 },
                Instr::JoinDynamic { id: 0, next: 8 },
                Instr::ExecNative { task_type: 1, argc: 0, retc: 0 },
                Instr::JoinDynamic { id: 0, next: 8 },
                Instr::ExecNative { task_type: 2, argc: 0, retc: 0 },
                Instr::JoinDynamic { id: 0, next: 8 },
                Instr::End,
            ],
            debug_map: BTreeMap::from([
                (2, "task_a".to_string()),
                (4, "task_b".to_string()),
                (6, "task_c".to_string()),
            ]),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec!["task_a".to_string(), "task_b".to_string(), "task_c".to_string()],
            error_route_map: BTreeMap::new(),
        };
        store.store_program(program.bytecode_version, &program).await.unwrap();

        let instance_id = engine
            .start("test", program.bytecode_version, "{}", [0u8; 32], "corr-5")
            .await.unwrap();

        // Set flag_0=true, flag_1=false → 2 branches taken (unconditional + flag_0)
        let mut inst = store.load_instance(instance_id).await.unwrap().unwrap();
        inst.flags.insert(0, Value::Bool(true));
        // flag 1 not set = false
        store.save_instance(&inst).await.unwrap();

        engine.tick_instance(instance_id).await.unwrap();

        assert_eq!(
            store.load_instance(instance_id).await.unwrap().unwrap().join_expected.get(&0),
            Some(&2)
        );

        // Run → 2 jobs
        let jobs = engine.run_instance(instance_id).await.unwrap();
        assert_eq!(jobs.len(), 2, "Should have 2 jobs (branches A and B)");

        // Complete first job → join has 1/2, should NOT release yet
        let payload = "{}";
        let hash = crate::vm::compute_hash(payload);
        engine.complete_job(&jobs[0].job_key, payload, hash, BTreeMap::new()).await.unwrap();
        engine.tick_instance(instance_id).await.unwrap();

        // Instance still Running (waiting for 2nd branch)
        let inst = store.load_instance(instance_id).await.unwrap().unwrap();
        assert!(matches!(inst.state, ProcessState::Running),
            "Should still be Running, got {:?}", inst.state);

        // Complete second job → join has 2/2, releases
        engine.complete_job(&jobs[1].job_key, payload, hash, BTreeMap::new()).await.unwrap();
        for _ in 0..5 {
            engine.tick_instance(instance_id).await.unwrap();
            let inst = store.load_instance(instance_id).await.unwrap().unwrap();
            if inst.state.is_terminal() { break; }
        }

        let inst = store.load_instance(instance_id).await.unwrap().unwrap();
        assert!(matches!(inst.state, ProcessState::Completed { .. }));

        // Assert: join_expected cleaned up
        assert!(inst.join_expected.is_empty(),
            "join_expected should be empty after join release");
    }

    /// T-IG-6: Full compiler pipeline — parse inclusiveGateway from BPMN XML.
    #[tokio::test]
    async fn t_ig_6_parse_inclusive_gateway() {
        let bpmn_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<bpmn:definitions xmlns:bpmn="http://www.omg.org/spec/BPMN/20100524/MODEL"
                  xmlns:zeebe="http://camunda.org/schema/zeebe/1.0">
  <bpmn:process id="proc_1" isExecutable="true">
    <bpmn:startEvent id="start"/>
    <bpmn:serviceTask id="prep" name="Prep">
      <bpmn:extensionElements><zeebe:taskDefinition type="prep"/></bpmn:extensionElements>
    </bpmn:serviceTask>
    <bpmn:inclusiveGateway id="ig_fork"/>
    <bpmn:serviceTask id="task_a" name="Identity Check">
      <bpmn:extensionElements><zeebe:taskDefinition type="identity_check"/></bpmn:extensionElements>
    </bpmn:serviceTask>
    <bpmn:serviceTask id="task_b" name="EDD Check">
      <bpmn:extensionElements><zeebe:taskDefinition type="edd_check"/></bpmn:extensionElements>
    </bpmn:serviceTask>
    <bpmn:inclusiveGateway id="ig_join"/>
    <bpmn:endEvent id="end"/>
    <bpmn:sequenceFlow id="f1" sourceRef="start" targetRef="prep"/>
    <bpmn:sequenceFlow id="f2" sourceRef="prep" targetRef="ig_fork"/>
    <bpmn:sequenceFlow id="f3" sourceRef="ig_fork" targetRef="task_a"/>
    <bpmn:sequenceFlow id="f4" sourceRef="ig_fork" targetRef="task_b">
      <bpmn:conditionExpression>= high_risk</bpmn:conditionExpression>
    </bpmn:sequenceFlow>
    <bpmn:sequenceFlow id="f5" sourceRef="task_a" targetRef="ig_join"/>
    <bpmn:sequenceFlow id="f6" sourceRef="task_b" targetRef="ig_join"/>
    <bpmn:sequenceFlow id="f7" sourceRef="ig_join" targetRef="end"/>
  </bpmn:process>
</bpmn:definitions>"#;

        let store = Arc::new(MemoryStore::new());
        let engine = BpmnLiteEngine::new(store.clone());

        let result = engine.compile(bpmn_xml).await;
        assert!(result.is_ok(), "Should compile inclusive gateway BPMN: {:?}", result.err());

        let compiled = result.unwrap();
        let program = store.load_program(compiled.bytecode_version).await.unwrap().unwrap();

        // Should contain ForkInclusive and JoinDynamic instructions
        let has_fork_inclusive = program.program.iter()
            .any(|i| matches!(i, Instr::ForkInclusive { .. }));
        assert!(has_fork_inclusive, "Should contain ForkInclusive instruction");

        let has_join_dynamic = program.program.iter()
            .any(|i| matches!(i, Instr::JoinDynamic { .. }));
        assert!(has_join_dynamic, "Should contain JoinDynamic instruction");
    }
```

---

## E) VERIFICATION GATE

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

---

## F) DONE SIGNAL

```
PHASE 5A COMPLETE — Inclusive gateway operational.
6/6 T-IG tests passing. Total: 77 core + 6 server.
Engine control flow complete. Next: Authoring Phase A.
```

---

## G) WHAT THIS PHASE DOES NOT DO

- Does NOT implement nested inclusive gateways (one pair per process for v1 — enforced by verifier)
- Does NOT implement multi-instance activity / fan-out (Phase 5B)
- Does NOT implement sub-processes (Phase 5C)
- Does NOT modify existing Fork/Join opcodes (zero regression risk)
- Does NOT parse BPMN `default` attribute on inclusive gateways (default_target exists in opcode for bytecode tests, compiler always emits None)
- join_expected lives on ProcessInstance (persisted via save_instance, already in place)
- join_expected[id] is removed on join release (prevents key leakage)
- ForkInclusive evaluates conditions from instance.flags (same flag interning as XOR)
- JoinDynamic mirrors Join semantics; reads expected from instance state
- JoinDynamic non-release path: fiber consumed directly (no save-then-delete)
- JoinDynamic release path: join_reset + JoinReleased event BEFORE fiber.pc advances (PITR determinism)
- Lowering pre-scans for converging inclusive gateways to allocate join_ids before diverging emit
- "Unconditional" branches use condition_flag: None (always taken)
- All-unconditional outgoing flows are valid BPMN (no verifier error)
