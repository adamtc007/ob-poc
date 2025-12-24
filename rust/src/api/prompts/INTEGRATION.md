# Prompt Integration Guide

## Layered Prompt Architecture

The system prompt is built from multiple layers, injected in order:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  LAYER 1: Role & Constraints (role.md or inline)                           │
│  "You are a DSL Translation Agent..."                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│  LAYER 2: Structure Rules (output format, JSON schema)                     │
│  Tool calling schema with verb enum constraint                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  LAYER 3: Verb Registry (build_vocab_prompt())                             │
│  Available verbs with params, types, descriptions                          │
├─────────────────────────────────────────────────────────────────────────────┤
│  LAYER 4: DAG Dependencies (dag_dependencies.md) ← NEW                     │
│  How @result_N works, lookup vs refs, dependency patterns                  │
├─────────────────────────────────────────────────────────────────────────────┤
│  LAYER 5: Domain Knowledge (domain_knowledge.md)                           │
│  Market codes, products, roles, jurisdictions                              │
├─────────────────────────────────────────────────────────────────────────────┤
│  LAYER 5b: KYC Async Patterns (kyc_async_patterns.md)                      │
│  Fire-and-forget requests, domain coherence, state queries                 │
├─────────────────────────────────────────────────────────────────────────────┤
│  LAYER 6: Entity Context (pre_resolve_entities output)                     │
│  Available CBUs, entities, products in scope                               │
├─────────────────────────────────────────────────────────────────────────────┤
│  LAYER 7: Session State (bindings, @cbu, history)                          │
│  What's currently active, recent operations                                │
├─────────────────────────────────────────────────────────────────────────────┤
│  LAYER 8: Ambiguity Detection (ambiguity_detection.md)                     │
│  When to ask for clarification, patterns                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│  LAYER 9: Few-Shot Examples (few_shot_examples.md)                         │
│  Input → output patterns, confidence calibration                           │
├─────────────────────────────────────────────────────────────────────────────┤
│  LAYER 10: Error Context (if retrying)                                     │
│  What went wrong, how to fix                                               │
└─────────────────────────────────────────────────────────────────────────────┘
```

## File Locations

```
rust/src/api/prompts/
├── role.md                 # Layer 1: Agent role and constraints
├── dag_dependencies.md     # Layer 4: DAG semantics
├── domain_knowledge.md     # Layer 5: Financial domain knowledge
├── kyc_async_patterns.md   # Layer 5b: KYC fire-and-forget patterns
├── ambiguity_detection.md  # Layer 8: Clarification patterns
└── few_shot_examples.md    # Layer 9: Input/output examples
```

## Integration in agent_service.rs

```rust
fn build_intent_extraction_prompt(session: &AgentSession) -> String {
    let mut prompt = String::new();
    
    // Layer 1: Role
    prompt.push_str(include_str!("prompts/role.md"));
    prompt.push_str("\n\n");
    
    // Layer 2: Structure (inline or via tool schema)
    // Handled by tool calling definition
    
    // Layer 3: Verb Registry
    prompt.push_str(&build_vocab_prompt(&session.verb_registry));
    prompt.push_str("\n\n");
    
    // Layer 4: DAG Dependencies ← ADD THIS
    prompt.push_str(include_str!("prompts/dag_dependencies.md"));
    prompt.push_str("\n\n");
    
    // Layer 5: Domain Knowledge
    prompt.push_str(include_str!("prompts/domain_knowledge.md"));
    prompt.push_str("\n\n");
    
    // Layer 6: Entity Context
    prompt.push_str(&format_pre_resolved_context(&session.pre_resolved));
    prompt.push_str("\n\n");
    
    // Layer 7: Session State
    prompt.push_str(&format_session_context(session));
    prompt.push_str("\n\n");
    
    // Layer 8: Ambiguity Detection
    prompt.push_str(include_str!("prompts/ambiguity_detection.md"));
    prompt.push_str("\n\n");
    
    // Layer 9: Few-Shot Examples
    prompt.push_str(include_str!("prompts/few_shot_examples.md"));
    
    prompt
}
```

## What Each Layer Teaches

| Layer | LLM Learns | Determinism Impact |
|-------|------------|-------------------|
| Role | What it can/can't do | Sets boundaries |
| Structure | Output format | Parseable output |
| Verb Registry | Valid verbs only | No hallucinated verbs |
| **DAG Dependencies** | **@result_N semantics** | **Correct multi-intent** |
| Domain Knowledge | Business terminology | Accurate mapping |
| **KYC Async Patterns** | **Fire-and-forget, state queries** | **Correct async handling** |
| Entity Context | What exists | No hallucinated entities |
| Session State | Current focus | Context-aware refs |
| Ambiguity | When to ask | Fewer wrong guesses |
| Few-Shot | Pattern matching | Consistent format |
| Error Context | How to fix | Successful retry |

## Key DAG Concepts for LLM

The new `dag_dependencies.md` teaches:

1. **@result_N creates edges** - Dependencies between intents
2. **lookups resolved first** - Not DAG edges
3. **Order is automatic** - Don't try to sort
4. **Parallel when possible** - Independent ops
5. **No cycles allowed** - A→B→A is error
6. **Logical output order** - For human review

## Verification

After integration, verify with test cases:

```
Input: "Create fund Apex, add John as director, set up custody"

Expected Output:
- 3 intents
- Intent 2 refs: {"cbu-id": "@result_1"}
- Intent 3 refs: {"cbu-id": "@result_1"}
- Intent 2 lookups: {"entity-id": {"search_text": "John"}}
- No circular dependencies
```

```
Input: "Create HoldCo, create fund under it, add custody to fund"

Expected Output:
- 3 intents (chain)
- Intent 2 refs: {"parent-entity-id": "@result_1"}
- Intent 3 refs: {"cbu-id": "@result_2"}
- Sequential dependency: 1 → 2 → 3
```
