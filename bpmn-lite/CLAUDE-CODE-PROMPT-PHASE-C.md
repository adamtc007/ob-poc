# CLAUDE CODE SESSION: AUTHORING PHASE C — BPMN EXPORT + ir_to_dto (v0.1)

**Read IMPL-PHASE-C-BPMN-EXPORT.md first.** It has the full design.

## Prerequisites check

```bash
cargo test -p bpmn-lite-core 2>&1 | tail -5
# Must show 101+ passed (Phases A + B)
```

## Non-negotiable constraints

1. **No changes to existing parser, verifier, lowering, or vm.** BPMN export is output-only.
2. **XML generation uses string building or quick-xml** — no heavyweight XML libraries. The output is templated BPMN, not arbitrary XML manipulation.
3. **DI layout is best-effort.** Functional for Camunda Modeler import, not pixel-perfect. Left-to-right topological with 200px horizontal spacing.
4. **ir_to_dto() operates on pre-lowering IR** (the IRGraph from parser, not post-lowering bytecode). It reads IRNode variants.
5. **Round-trip is semantic, not byte-identical.** DTO → BPMN → parse → IR → DTO should preserve node topology and edge semantics but may normalize IDs and ordering.

## Execution plan — 5 steps

1. **export_bpmn.rs** — `dto_to_bpmn_xml()`, bpmn_id generation, DI layout, element mapping
2. **ir_to_dto.rs** — `ir_to_dto()`, node mapping, edge extraction, gateway join pairing
3. **mod.rs** — Add `pub mod export_bpmn; pub mod ir_to_dto;`
4. **Cargo.toml** — Add `quick-xml` if using it (or use format! string building — your choice)
5. **Tests** — 10 T-EXP tests

## CRITICAL: Check what XML library is already available

```bash
grep -n "quick-xml\|xml\|roxmltree" bpmn-lite-core/Cargo.toml
grep -rn "use quick_xml\|use roxmltree\|use xml" bpmn-lite-core/src/
```

The existing BPMN parser likely uses `roxmltree` (read-only). For XML generation, either:
- Use `quick-xml` writer (add dep)
- Use `format!()` string building (no dep, fine for templated output)

String building is simpler for this use case — the BPMN output is a fixed structure with variable element IDs.

## CRITICAL: bpmn_id generation

```rust
fn generate_bpmn_id(template_key: &str, node_id: &str) -> String {
    let sanitized = node_id.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect::<String>();
    let sanitized = if sanitized.is_empty() || sanitized.chars().next().unwrap().is_ascii_digit() {
        format!("_{}", sanitized)
    } else {
        sanitized
    };

    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;
    let mut h = DefaultHasher::new();
    template_key.hash(&mut h);
    node_id.hash(&mut h);
    format!("{}_{:08x}", sanitized, h.finish() as u32)
}
```

If a node has `bpmn_id: Some(custom_id)`, use that instead of generating one.

## CRITICAL: Condition → FEEL expression

| FlagOp | Output |
|--------|--------|
| IsTrue | `= {flag}` |
| IsFalse | `= not({flag})` |
| Eq + I64(n) | `= {flag} = {n}` |
| Eq + Str(s) | `= {flag} = "{s}"` |
| Eq + Bool(b) | `= {flag} = {b}` |
| Neq + I64(n) | `= {flag} != {n}` |
| Neq + Str(s) | `= {flag} != "{s}"` |
| Neq + Bool(b) | `= {flag} != {b}` |

These go inside `<bpmn:conditionExpression xsi:type="bpmn:tFormalExpression">`.

## CRITICAL: Timer duration — ms to ISO 8601

Timer ms values must be converted to ISO 8601 duration for BPMN:

```rust
fn ms_to_iso_duration(ms: u64) -> String {
    let secs = ms / 1000;
    let mins = secs / 60;
    let hours = mins / 60;
    let days = hours / 24;

    if days > 0 { format!("P{}D", days) }
    else if hours > 0 { format!("PT{}H", hours) }
    else if mins > 0 { format!("PT{}M", mins) }
    else { format!("PT{}S", secs) }
}
```

## CRITICAL: DI layout algorithm

```rust
fn compute_layout(dto: &WorkflowGraphDto) -> HashMap<String, (f64, f64, f64, f64)> {
    // 1. Build adjacency from edges
    // 2. Topological sort (BFS from start node)
    // 3. Assign ranks (distance from start)
    // 4. X = rank * 200.0, Y = index_within_rank * 100.0
    // 5. Dimensions: Start/End = (36, 36), Task = (100, 80), Gateway = (50, 50)
    // Returns: node_id → (x, y, width, height)
}
```

## CRITICAL: ir_to_dto needs access to IRNode variants

Check what's exported:
```bash
grep -n "pub enum IRNode" bpmn-lite-core/src/compiler/ir.rs
grep -n "pub use" bpmn-lite-core/src/compiler/mod.rs
grep -n "pub mod compiler" bpmn-lite-core/src/lib.rs
```

ir_to_dto needs to iterate `graph.node_indices()` and match on `IRNode` variants. Make sure IRNode and GatewayDirection are accessible from the authoring module.

## CRITICAL: Round-trip test (T-EXP-6) expectations

The round-trip test is:
1. Create a DTO with inclusive gateway + service tasks
2. Export to BPMN XML via `dto_to_bpmn_xml()`
3. Parse the XML via the existing `compile()` parser → get IRGraph
4. Convert IR back to DTO via `ir_to_dto()`
5. Assert: same number of nodes, same node kinds, same edge topology

Do NOT assert byte-identical. Node IDs may differ (parser generates from BPMN element IDs, which are the generated bpmn_ids). The semantic structure must match.

## Progress gates

- Step 1 (export_bpmn.rs compiles) → 30% → IMMEDIATELY proceed to Step 2
- Step 2 (ir_to_dto.rs compiles) → 60% → IMMEDIATELY proceed to Step 3
- Step 3 (mod.rs updated) → 65% → IMMEDIATELY proceed to Step 4
- Step 4 (deps if needed) → 70% → IMMEDIATELY add tests
- Tests added → 90% → Run `cargo test -p bpmn-lite-core`
- All green → 100% → Print DONE signal

## Verification

```bash
cargo test -p bpmn-lite-core 2>&1
```
All existing (101) must pass. Plus:
```bash
cargo test -p bpmn-lite-core -- t_exp 2>&1
```
Expected: `test result: ok. 10 passed`

```bash
cargo check --features postgres 2>&1
```
Must compile clean.

## Done signal

```
PHASE C COMPLETE — BPMN export + ir_to_dto operational.
10/10 T-EXP tests passing. Total: 111 tests.
Next: Phase D (CLI + publish lifecycle).
```
