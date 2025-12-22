# Phase 3 Baseline - Quick Reference

## Immediate Priority Tasks

### 1. Wire Execution (2-3 days)
```
File: rust/src/agentic/pipeline.rs

Current:
  fn do_execute() → returns placeholder TurnExecutionResult

Required:
  async fn do_execute() → actually calls DslExecutor
  
Steps:
  1. Add tokio async to pipeline
  2. Import DslExecutor from dsl_v2
  3. Parse DSL → Compile → Execute
  4. Return real bindings from execution context
  5. Update session with created symbols
```

### 2. Add Missing Parameter Mappings (2-3 days)
```
File: rust/config/agent/parameter_mappings.yaml

Missing verbs (copy from TODO-PHASE3-BASELINE-COMPLETION.md):
  - investment-manager.set-scope
  - investment-manager.link-connectivity
  - investment-manager.remove
  - investment-manager.find-for-trade
  - pricing-config.list
  - cash-sweep.list
  - sla.apply-template
  - trading-profile.create
  - trading-profile.set-universe
  - trading-profile.visualize
  - trading-profile.validate-matrix
  - cbu-custody.ensure-ssi
  - cbu-custody.ensure-booking-rule
  - cbu-custody.add-universe
```

### 3. Build Evaluation Harness (2 days)
```
New file: rust/src/agentic/evaluation.rs
New file: rust/src/bin/evaluate_agent.rs

Load: config/agent/evaluation_dataset.yaml
Run: For each case, process → check intents → check entities → check DSL
Report: Accuracy metrics, failures
```

## Validation Commands

```bash
# After each task
cd rust
cargo test --lib agentic

# Run specific test module
cargo test --lib agentic::pipeline_tests

# Check all configs load
cargo test --lib test_all_intents_have_mappings

# Run evaluation (after harness built)
cargo run --bin evaluate_agent
```

## Files Modified Checklist

- [ ] `rust/src/agentic/pipeline.rs` - async execution
- [ ] `rust/src/agentic/mod.rs` - exports for evaluation
- [ ] `rust/config/agent/parameter_mappings.yaml` - 15 new verb mappings
- [ ] `rust/config/agent/entity_types.yaml` - new entity types
- [ ] `rust/Cargo.toml` - evaluation binary

## Files Created Checklist

- [ ] `rust/src/agentic/evaluation.rs`
- [ ] `rust/src/bin/evaluate_agent.rs`

## Definition of Done

```
□ cargo test passes
□ All intents have mappings (test verifies)
□ Pipeline executes DSL (integration test)  
□ Evaluation > 85% accuracy
□ Demo works: "Add BlackRock for European equities" → executed
```
