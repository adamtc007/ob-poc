# IMPL-PHASE-C: BPMN Export + ir_to_dto (v0.1)

**Prerequisites:** Phase A complete, Phase B complete (contracts + lints)
**Goal:** Close the round-trip: DTO → BPMN XML for Camunda Modeler review, and IR → DTO for BPMN XML import normalization.
**Outcome:** Workflows can be exported to valid Camunda 8 BPMN XML and re-imported without semantic loss.

---

## A) What This Phase Builds

### A1. dto_to_bpmn_xml() — DTO → BPMN XML export

Generates valid BPMN 2.0 XML from a WorkflowGraphDto. The output is a Camunda 8 compatible process definition with:
- Standard BPMN elements (startEvent, endEvent, serviceTask, gateways, intermediateCatchEvent, boundaryEvent)
- Zeebe extension elements for taskDefinition, headers
- Sequence flows with optional conditionExpression (orch_flags only)
- Best-effort diagram interchange (DI) layout for Camunda Modeler positioning

```rust
// bpmn-lite-core/src/authoring/export_bpmn.rs

/// Export a WorkflowGraphDto to BPMN 2.0 XML string.
/// The output is valid Camunda 8 BPMN with Zeebe extension elements.
pub fn dto_to_bpmn_xml(dto: &WorkflowGraphDto) -> Result<String> { ... }
```

### A2. ir_to_dto() — IR → DTO normalization

Converts an IRGraph (from BPMN XML import) back to a WorkflowGraphDto. This enables:
- Import BPMN XML → parse to IR → normalize to DTO → store as template
- Round-trip: BPMN XML → IR → DTO → BPMN XML (semantic preservation)

```rust
// bpmn-lite-core/src/authoring/ir_to_dto.rs

/// Convert an IRGraph to a WorkflowGraphDto.
/// Used for normalizing BPMN XML imports into the DTO format.
pub fn ir_to_dto(ir: &IRGraph, meta: &TemplateMeta) -> Result<WorkflowGraphDto> { ... }
```

### A3. bpmn_id generation

```rust
// bpmn-lite-core/src/authoring/export_bpmn.rs (internal helper)

/// Generate a stable BPMN element ID from a node ID.
/// Format: sanitize(node_id) + "_" + short_hash(template_key, node_id)
fn generate_bpmn_id(template_key: &str, node_id: &str) -> String {
    let sanitized = sanitize_bpmn_id(node_id);
    let hash = short_hash(template_key, node_id);
    format!("{}_{}", sanitized, hash)
}

/// Replace non-alphanumeric characters with '_', ensure valid BPMN ID.
/// BPMN IDs must start with a letter or underscore.
fn sanitize_bpmn_id(id: &str) -> String {
    let mut result = String::new();
    for (i, c) in id.chars().enumerate() {
        if c.is_ascii_alphanumeric() || c == '_' {
            result.push(c);
        } else {
            result.push('_');
        }
    }
    // Ensure starts with letter or underscore
    if result.is_empty() || result.chars().next().unwrap().is_ascii_digit() {
        result.insert(0, '_');
    }
    result
}

/// 8-char hex hash for ID uniqueness.
fn short_hash(template_key: &str, node_id: &str) -> String {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;
    let mut h = DefaultHasher::new();
    template_key.hash(&mut h);
    node_id.hash(&mut h);
    format!("{:08x}", h.finish() as u32)
}
```

---

## B) BPMN XML Structure

### B1. Document skeleton

```xml
<?xml version="1.0" encoding="UTF-8"?>
<bpmn:definitions xmlns:bpmn="http://www.omg.org/spec/BPMN/20100524/MODEL"
                  xmlns:bpmndi="http://www.omg.org/spec/BPMN/20100524/DI"
                  xmlns:dc="http://www.omg.org/spec/DD/20100524/DC"
                  xmlns:di="http://www.omg.org/spec/DD/20100524/DI"
                  xmlns:zeebe="http://camunda.org/schema/zeebe/1.0"
                  xmlns:modeler="http://camunda.org/schema/modeler/1.0"
                  id="Definitions_1"
                  targetNamespace="http://bpmn.io/schema/bpmn"
                  exporter="bpmn-lite"
                  exporterVersion="0.1.0">
  <bpmn:process id="{process_key}" isExecutable="true">
    <!-- elements -->
  </bpmn:process>
  <bpmndi:BPMNDiagram id="BPMNDiagram_1">
    <bpmndi:BPMNPlane id="BPMNPlane_1" bpmnElement="{process_key}">
      <!-- shapes and edges -->
    </bpmndi:BPMNPlane>
  </bpmndi:BPMNDiagram>
</bpmn:definitions>
```

### B2. Element mapping (DTO → BPMN)

| NodeDto variant | BPMN element | Notes |
|-----------------|--------------|-------|
| Start | `<bpmn:startEvent>` | |
| End { terminate: false } | `<bpmn:endEvent>` | |
| End { terminate: true } | `<bpmn:endEvent>` + `<bpmn:terminateEventDefinition/>` | |
| ServiceTask | `<bpmn:serviceTask>` + zeebe:taskDefinition | `type` = task_type |
| ExclusiveGateway | `<bpmn:exclusiveGateway>` | `default` attr = default flow ID |
| ParallelGateway { Diverging } | `<bpmn:parallelGateway>` | |
| ParallelGateway { Converging } | `<bpmn:parallelGateway>` | |
| InclusiveGateway { Diverging } | `<bpmn:inclusiveGateway>` | |
| InclusiveGateway { Converging } | `<bpmn:inclusiveGateway>` | |
| TimerWait | `<bpmn:intermediateCatchEvent>` + `<bpmn:timerEventDefinition>` | ISO 8601 duration |
| MessageWait | `<bpmn:intermediateCatchEvent>` + `<bpmn:messageEventDefinition>` | zeebe:subscription |
| HumanWait | Same as MessageWait | name = "human.{task_kind}" |
| RaceWait | `<bpmn:eventBasedGateway>` + catch events per arm | See B3 |
| BoundaryTimer | `<bpmn:boundaryEvent>` + `<bpmn:timerEventDefinition>` | `attachedToRef` = host bpmn_id |

### B3. RaceWait → event-based gateway pattern

RaceWait compiles to BPMN's event-based gateway pattern:

```xml
<bpmn:eventBasedGateway id="{race_bpmn_id}"/>
<!-- For each arm: -->
<bpmn:intermediateCatchEvent id="{arm_bpmn_id}">
  <!-- Timer or Message definition -->
</bpmn:intermediateCatchEvent>
<bpmn:sequenceFlow sourceRef="{race_bpmn_id}" targetRef="{arm_bpmn_id}"/>
<bpmn:sequenceFlow sourceRef="{arm_bpmn_id}" targetRef="{arm_target}"/>
```

### B4. Condition expressions

FlagCondition → BPMN conditionExpression. Keep it minimal — Camunda needs something parseable but our engine doesn't use FEEL:

| FlagOp | BPMN expression |
|--------|-----------------|
| IsTrue | `= orch_high_risk` |
| IsFalse | `= not(orch_high_risk)` |
| Eq { I64: 3 } | `= orch_retry_count = 3` |
| Neq { Str: "blocked" } | `= orch_status != "blocked"` |

These are FEEL expressions for Camunda compatibility. The bpmn-lite engine's own parser reads them back as flag conditions via the existing condition parser.

### B5. Error routing in BPMN

Error edges become BPMN error boundary events on service tasks:

```xml
<bpmn:serviceTask id="{task_bpmn_id}" name="{task_type}">
  <bpmn:extensionElements>
    <zeebe:taskDefinition type="{task_type}"/>
  </bpmn:extensionElements>
</bpmn:serviceTask>
<bpmn:boundaryEvent id="{error_boundary_id}" attachedToRef="{task_bpmn_id}">
  <bpmn:errorEventDefinition errorRef="{error_code}"/>
</bpmn:boundaryEvent>
<bpmn:sequenceFlow sourceRef="{error_boundary_id}" targetRef="{error_target}"/>
```

Retry semantics are stored in zeebe extension elements (header `retries`).

### B6. Sequence flow IDs

Generated deterministically: `flow_{from_bpmn_id}_to_{to_bpmn_id}`

### B7. Default flow attribute

For ExclusiveGateway, the `default` attribute on the gateway element references the sequence flow ID of the default edge:

```xml
<bpmn:exclusiveGateway id="risk_decision_abc123" default="flow_risk_decision_abc123_to_standard_path_def456"/>
```

---

## C) DI Layout (best-effort)

Camunda Modeler requires diagram interchange (DI) elements to render. A simple auto-layout:

### C1. Algorithm: left-to-right topological

1. Topological sort of nodes (using DTO edges)
2. Assign X position: topo_rank * 200px (left-to-right spacing)
3. Assign Y position: within same rank, space vertically at 100px intervals
4. Node dimensions: Start/End 36x36, Task 100x80, Gateway 50x50
5. Sequence flow waypoints: simple horizontal connectors with one bend point for vertical offsets

### C2. DI elements

```xml
<bpmndi:BPMNShape id="{bpmn_id}_di" bpmnElement="{bpmn_id}">
  <dc:Bounds x="{x}" y="{y}" width="{w}" height="{h}"/>
</bpmndi:BPMNShape>

<bpmndi:BPMNEdge id="{flow_id}_di" bpmnElement="{flow_id}">
  <di:waypoint x="{x1}" y="{y1}"/>
  <di:waypoint x="{x2}" y="{y2}"/>
</bpmndi:BPMNEdge>
```

The layout is functional, not beautiful. Camunda Modeler allows manual rearrangement after import.

---

## D) ir_to_dto() — IR Graph to DTO Conversion

### D1. Node mapping (IR → DTO)

| IRNode | NodeDto | Notes |
|--------|---------|-------|
| StartEvent { id } | Start { id } | |
| EndEvent { id, terminate } | End { id, terminate } | |
| ServiceTask { id, task_type, .. } | ServiceTask { id, task_type, bpmn_id } | |
| GatewayXor { id, direction } | ExclusiveGateway { id } | Direction inferred from edge analysis |
| GatewayAnd { id, direction } | ParallelGateway { id, direction, join } | Join resolved by finding paired converging |
| GatewayInclusive { id, direction } | InclusiveGateway { id, direction, join } | Join resolved by finding paired converging |
| IntermediateCatchTimer { id, ms } | TimerWait { id, ms } | |
| IntermediateCatchMessage { id, name, corr } | MessageWait { id, name, corr_key_source } | |
| EventBasedGateway + catches | RaceWait { id, arms } | Reconstruct arms from downstream catches |
| BoundaryTimer { id, host, ms, interrupting } | BoundaryTimer { id, host, ms, interrupting } | |

### D2. Edge extraction

Walk IR graph edges. For each edge:
- If source is a diverging gateway and edge has a condition → `EdgeDto { condition: Some(...) }`
- If edge has error routing data → `EdgeDto { on_error: Some(...) }`
- If edge is the default flow from an XOR → `EdgeDto { is_default: true }`
- Otherwise → simple sequence flow `EdgeDto { from, to }`

### D3. Gateway join pairing

For Parallel and Inclusive gateways, the diverging gateway needs a `join` reference. Algorithm:
- For each converging gateway, find the diverging gateway that pairs with it
- Use the same BFS approach as the verifier: from diverging, find first downstream converging of same type

### D4. Limitations

- IR may contain lowering artifacts (error routing opcodes, counter loops) that don't map cleanly back to DTO
- `ir_to_dto()` operates on the pre-lowering IR (the IRGraph from parser, not post-lowering bytecode)
- Some information loss is expected: DTO → IR → DTO may normalize but should preserve semantics

---

## E) File Ownership

| File | Purpose |
|------|---------|
| `bpmn-lite-core/src/authoring/export_bpmn.rs` | dto_to_bpmn_xml(), DI layout, bpmn_id generation |
| `bpmn-lite-core/src/authoring/ir_to_dto.rs` | ir_to_dto(), node/edge mapping |
| `bpmn-lite-core/src/authoring/mod.rs` | Add `pub mod export_bpmn; pub mod ir_to_dto;` |

No changes to engine.rs, dto.rs, validate.rs, dto_to_ir.rs, yaml.rs, contracts.rs, or lints.rs.

---

## F) Tests

### T-EXP-1: Simple sequence → valid BPMN XML

```
DTO: Start → ServiceTask → End
Export to BPMN XML
Assert: valid XML, contains startEvent, serviceTask with zeebe:taskDefinition, endEvent
Assert: sequence flows connect them correctly
```

### T-EXP-2: Inclusive gateway export

```
DTO: inclusive gateway with 3 branches (1 unconditional, 2 conditional)
Export to BPMN XML
Assert: inclusiveGateway elements (diverging + converging)
Assert: conditionExpression on conditional flows
Assert: unconditional flow has no conditionExpression
```

### T-EXP-3: XOR with default flow

```
DTO: exclusive gateway with 2 branches, one default
Export to BPMN XML
Assert: exclusiveGateway has `default` attribute referencing the default flow ID
Assert: default flow has no conditionExpression
```

### T-EXP-4: Terminate end event

```
DTO: End with terminate: true
Export to BPMN XML
Assert: endEvent contains terminateEventDefinition
```

### T-EXP-5: Error boundary export

```
DTO: ServiceTask with on_error edge
Export to BPMN XML
Assert: boundaryEvent attached to serviceTask
Assert: errorEventDefinition with errorRef
```

### T-EXP-6: Round-trip — DTO → BPMN XML → parse → IR → ir_to_dto → DTO

```
Start with a DTO (inclusive gateway + service tasks)
Export to BPMN XML
Re-import via existing compile() parser → get IRGraph
Convert IR to DTO via ir_to_dto()
Assert: node IDs preserved (or mapped), edge topology equivalent
Assert: re-exported BPMN XML has same semantic structure
```

This is the key round-trip test. It doesn't need byte-identical output, but the graph structure and semantics must match.

### T-EXP-7: DI layout present

```
Export any DTO to BPMN XML
Assert: BPMNDiagram element present
Assert: BPMNShape for each node
Assert: BPMNEdge for each sequence flow
Assert: all shapes have valid Bounds (positive x, y, width, height)
```

### T-EXP-8: bpmn_id generation

```
Node id "screen-sanctions" with template_key "kyc.onboarding-ucits"
Assert: generated bpmn_id = "screen_sanctions_{8-char-hash}"
Assert: consistent across calls (deterministic)
Assert: sanitize handles special chars
```

### T-EXP-9: ir_to_dto basic

```
Parse a simple BPMN XML via existing compile() → get IRGraph
Convert to DTO via ir_to_dto()
Assert: correct node kinds and IDs
Assert: edges match original topology
```

### T-EXP-10: RaceWait export (event-based gateway)

```
DTO: RaceWait with Timer + Message arms
Export to BPMN XML
Assert: eventBasedGateway + intermediateCatchEvents
Assert: timer has timerEventDefinition with ISO duration
Assert: message has messageEventDefinition
```

---

## G) Verification Gate

```bash
cargo test -p bpmn-lite-core 2>&1
```

All existing (101 tests after Phase B) must pass. Plus:
```bash
cargo test -p bpmn-lite-core -- t_exp 2>&1
```
Expected: `test result: ok. 10 passed`

```bash
cargo check --features postgres 2>&1
```
Must compile clean.

---

## H) Done Signal

```
PHASE C COMPLETE — BPMN export + ir_to_dto operational.
10/10 T-EXP tests passing. Total: 111 tests.
Next: Phase D (CLI + publish lifecycle).
```
