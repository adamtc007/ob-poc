# Agentic Interface Plan: Deterministic DSL.CRUD.CBU Generation

## The Core Problem

LLMs are stochastic by nature. We need to constrain them to produce valid, parseable DSL that passes through Forth every time.

## Architecture: Constrained Generation Pipeline

User Intent -> Intent Classification -> Template Selection -> Slot Filling -> DSL Validation -> Forth Execution

### Layer 1: Intent Classification

Fixed set of CBU lifecycle operations:
- cbu.create (required: cbu-name, jurisdiction)
- cbu.submit (required: cbu-id)
- cbu.approve (required: cbu-id)
- cbu.reject (required: cbu-id)
- cbu.suspend (required: cbu-id)
- cbu.reactivate (required: cbu-id)
- cbu.close (required: cbu-id)
- cbu.attach-entity (required: cbu-id, entity-id, role)
- cbu.update (required: cbu-id, attributes)

### Layer 2: RAG Context

Retrieve EBNF grammar, valid vocabulary, and examples for slot filling.

### Layer 3: Structured Output

Force JSON response with intent, slots, and generated DSL.

### Layer 4: Validation

1. Syntax Check (NomDslParser)
2. Schema Check (required slots, valid values)
3. State Check (valid transitions)

### Layer 5: Prompt Engineering

Temperature 0, constrained vocabulary, few-shot examples.

## Implementation Phases

1. Constrained Template System
2. RAG Integration
3. Full LLM Generation with Guardrails
4. Fine-tuning (Optional)

## Current State

E2E test harness passes tests 1-4. Forth engine working.

## Files to Review

- rust/src/forth_engine/ - Core engine
- rust/src/cbu_model_dsl/ - CBU Model
- rust/src/database/crud_executor.rs - Execution
- rust/src/services/agentic_dsl_crud.rs - Needs refactor
- rust/src/bin/e2e_cbu_flow_test.rs - Test harness
