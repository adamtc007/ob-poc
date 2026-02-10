# CLAUDE CODE SESSION: AUTHORING PHASE B — VERB CONTRACTS + LINTS (v0.1)

**Read IMPL-PHASE-B-VERB-CONTRACTS.md first.** It has the full design.

## Prerequisites check

```bash
cargo test -p bpmn-lite-core 2>&1 | tail -5
# Must show 91+ passed
```

## Non-negotiable constraints

1. **No changes to existing files** except `src/authoring/mod.rs` (add module declarations) and `Cargo.toml` if new deps needed.
2. **Lints are optional** — `compile_from_yaml()` and `compile_from_dto()` continue working without a ContractRegistry. Lints are an additive layer.
3. **L1 (flag provenance) requires backward BFS** from the gateway node through DTO edges. NOT forward from Start.
4. **Catch-all `"*"` is always valid** for L2 — never flag it as an unknown error code.
5. **L3 (correlation) is Warning only** — never Error. Correlation provenance is hard to prove statically.

## Execution plan — 4 steps

1. **contracts.rs** — VerbContract, CorrelationContract, ContractRegistry (with `new()`, `register()`, `get()`, `load_from_strings()`)
2. **lints.rs** — `lint_contracts()`, LintDiagnostic, LintLevel, rules L1–L5
3. **mod.rs** — Add `pub mod contracts; pub mod lints;`
4. **Tests** — 10 T-LINT tests in a `#[cfg(test)]` module in lints.rs

## CRITICAL: L1 backward BFS algorithm

For each gateway edge with a `condition`, find the gateway node's ID from `edge.from`. Then BFS backward through DTO edges (reverse direction: find edges where `to == current_node`, add `from` to queue). Collect all ServiceTask node IDs reachable backward. Look up their contracts. Union their `writes_flags`. Check the condition flag is in the union.

```rust
fn upstream_flags(dto: &WorkflowGraphDto, gateway_id: &str, registry: &ContractRegistry) -> HashSet<String> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(gateway_id.to_string());
    let mut flags = HashSet::new();

    while let Some(node_id) = queue.pop_front() {
        if !visited.insert(node_id.clone()) { continue; }
        // Find all edges where `to == node_id` (incoming edges)
        for edge in &dto.edges {
            let from_id = edge.from.split('.').next().unwrap_or(&edge.from);
            if edge.to == node_id {
                queue.push_back(from_id.to_string());
            }
        }
        // If this node is a ServiceTask, collect its writes_flags
        for node in &dto.nodes {
            if let NodeDto::ServiceTask { id, task_type, .. } = node {
                if id == &node_id {
                    if let Some(contract) = registry.get(task_type) {
                        flags.extend(contract.writes_flags.iter().cloned());
                    }
                }
            }
        }
    }
    flags
}
```

## CRITICAL: Test structure

Each test should construct a minimal DTO + ContractRegistry inline (not from YAML files). Use the existing dto.rs types directly. Example pattern:

```rust
#[test]
fn t_lint_1_flag_provenance_valid() {
    let dto = WorkflowGraphDto {
        meta: test_meta(),
        nodes: vec![
            NodeDto::Start { id: "start".into() },
            NodeDto::ServiceTask { id: "task_a".into(), task_type: "test.write-flag".into(), bpmn_id: None },
            NodeDto::ExclusiveGateway { id: "gate".into() },
            NodeDto::ServiceTask { id: "task_b".into(), task_type: "test.noop".into(), bpmn_id: None },
            NodeDto::ServiceTask { id: "task_c".into(), task_type: "test.noop".into(), bpmn_id: None },
            NodeDto::End { id: "end".into(), terminate: false },
        ],
        edges: vec![
            EdgeDto { from: "start".into(), to: "task_a".into(), ..Default::default() },
            EdgeDto { from: "task_a".into(), to: "gate".into(), ..Default::default() },
            EdgeDto { from: "gate".into(), to: "task_b".into(), condition: Some(FlagCondition { flag: "orch_x".into(), op: FlagOp::IsTrue, value: None }), ..Default::default() },
            EdgeDto { from: "gate".into(), to: "task_c".into(), is_default: true, ..Default::default() },
            EdgeDto { from: "task_b".into(), to: "end".into(), ..Default::default() },
            EdgeDto { from: "task_c".into(), to: "end".into(), ..Default::default() },
        ],
    };
    let mut reg = ContractRegistry::new();
    reg.register(VerbContract {
        task_type: "test.write-flag".into(),
        description: "".into(),
        writes_flags: vec!["orch_x".into()],
        ..Default::default()
    });
    reg.register(VerbContract { task_type: "test.noop".into(), description: "".into(), ..Default::default() });

    let results = lint_contracts(&dto, &reg, false);
    let errors: Vec<_> = results.iter().filter(|d| d.level == LintLevel::Error).collect();
    assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
}
```

## CRITICAL: EdgeDto needs Default derive

If EdgeDto doesn't already have Default, add it (or use a constructor). Tests need `..Default::default()` for clean construction.

Check: `grep -n "Default" bpmn-lite-core/src/authoring/dto.rs`

If not present, add `#[derive(Default)]` to EdgeDto (with `from: String::new(), to: String::new()` defaults). Or add a helper `fn edge(from, to) -> EdgeDto`.

## CRITICAL: VerbContract needs Default derive

For test ergonomics, VerbContract should derive Default so tests can use `..Default::default()`.

## Progress gates

- Step 1 (contracts.rs compiles) → 25% → IMMEDIATELY proceed to Step 2
- Step 2 (lints.rs compiles) → 60% → IMMEDIATELY proceed to Step 3
- Step 3 (mod.rs updated) → 65% → IMMEDIATELY add tests
- Tests added → 90% → Run `cargo test -p bpmn-lite-core`
- All green → 100% → Print DONE signal

## Verification

```bash
cargo test -p bpmn-lite-core 2>&1
```
All existing (91) must pass. Plus:
```bash
cargo test -p bpmn-lite-core -- t_lint 2>&1
```
Expected: `test result: ok. 10 passed`

```bash
cargo check --features postgres 2>&1
```
Must compile clean.

## Done signal

```
PHASE B COMPLETE — Verb contracts + lints operational.
10/10 T-LINT tests passing. Total: 101 tests.
Next: Phase C (BPMN export + ir_to_dto).
```
