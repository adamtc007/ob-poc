# LLM Integration Patterns

## Core Principle

**LLM as Intent Extractor, not Code Generator.**

The LLM understands natural language and extracts structured intent. The system handles resolution, validation, and code generation deterministically.

```
User: "Add John as director of Apex Fund"
            │
            ▼
┌───────────────────────────────────────────┐
│              LLM (black box)              │
│                                           │
│  Good at: Understanding intent            │
│  Bad at: Valid UUIDs, exact codes,        │
│          consistent syntax                │
└───────────────────────────────────────────┘
            │
            ▼
     Structured Intent (JSON)
            │
            ▼
┌───────────────────────────────────────────┐
│         System (deterministic)            │
│                                           │
│  - Resolve "John" → UUID                  │
│  - Resolve "director" → "DIRECTOR"        │
│  - Resolve "Apex Fund" → UUID             │
│  - Build valid DSL                        │
│  - Validate against grammar               │
└───────────────────────────────────────────┘
            │
            ▼
     Valid, executable DSL
```

---

## Pattern 1: Constrained Output via Tool Schema

**Problem**: Free-text LLM output requires fragile parsing and produces inconsistent structure.

**Solution**: Use tool-use (function calling) with JSON schema that constrains output.

```rust
// Tool schema constrains verb to enum
ToolDefinition {
    name: "generate_dsl_intents",
    parameters: json!({
        "properties": {
            "intents": {
                "items": {
                    "properties": {
                        "verb": {
                            "type": "string",
                            "enum": ["cbu.ensure", "cbu.add-product", ...],  // ← Constrained
                        }
                    }
                }
            }
        }
    })
}
```

**Result**: LLM can only select from valid verbs. "Unknown verb" errors eliminated at source.

**Anti-pattern**: 
```
"Please only use verbs from this list: ..."  // LLM ignores, hallucinates anyway
```

---

## Pattern 2: Post-LLM Resolution Layer

**Problem**: LLM hallucinates entity IDs, misspells codes, uses wrong formats.

**Solution**: LLM outputs human-readable references. System resolves to canonical values.

```
LLM outputs:                    System resolves:
─────────────────────────────────────────────────
"John Smith"                 → UUID (via EntityGateway)
"Apex Fund"                  → UUID (via EntityGateway)
"custody"                    → "CUSTODY"
"director"                   → "DIRECTOR"
"Luxembourg"                 → "LU"
```

**Implementation**: Single `resolve_all()` method handles ALL references through EntityGateway.

```rust
// One method, one pattern
match self.resolve_all(intents).await {
    UnifiedResolution::Resolved { intents, corrections } => { ... }
    UnifiedResolution::NeedsDisambiguation { items, .. } => { ... }
    UnifiedResolution::Error(msg) => { /* retry with feedback */ }
}
```

**Key insight**: The Gateway already has fuzzy matching. "fund admin" → "FUND_ACCOUNTING" for free.

**Anti-pattern**:
```rust
// Trust LLM output directly
let product = intent.params["product"];  // Could be "CUST", "custody", "Custody", ...
execute_dsl(product);  // Runtime error
```

---

## Pattern 3: Deterministic Code Generation

**Problem**: LLM-generated code has syntax errors, inconsistent formatting, wrong structure.

**Solution**: LLM outputs structured intent. Rust code generates DSL deterministically.

```rust
// LLM output (structured)
VerbIntent {
    verb: "cbu.add-product",
    params: { "product": "CUSTODY" },
    refs: { "cbu-id": "@apex_fund" },
}

// Rust generates DSL (deterministic)
fn build_dsl_statement(intent: &VerbIntent) -> String {
    let mut parts = vec![format!("({}", intent.verb)];
    for (name, value) in &intent.params {
        parts.push(format!(":{} {}", name, value.to_dsl_string()));
    }
    for (name, ref_name) in &intent.refs {
        parts.push(format!(":{} {}", name, ref_name));
    }
    parts.push(")".to_string());
    parts.join(" ")
}

// Output: (cbu.add-product :product CUSTODY :cbu-id @apex_fund)
```

**Result**: Zero syntax errors. Consistent formatting. Testable.

**Anti-pattern**:
```
LLM: "Here's the DSL: (cbu.add-product :product CUSTODY :cbu-id ...)"
// Might have wrong parentheses, spacing, quoting, etc.
```

---

## Pattern 4: Validation with Retry Feedback

**Problem**: Even constrained output can have semantic errors (wrong param combinations, missing required fields).

**Solution**: Validate generated DSL. Feed errors back to LLM. Retry.

```rust
for attempt in 0..MAX_RETRIES {
    let intents = llm_client.chat_with_tool(&prompt, &message, &tool).await?;
    let resolved = self.resolve_all(intents).await;
    let dsl = build_dsl_program(&resolved.intents);
    
    // Validate
    match linter.validate(&dsl).await {
        Ok(_) => return Ok(dsl),  // Success
        Err(errors) => {
            // Feed errors back to LLM
            feedback = format!("Validation errors:\n{}", errors.join("\n"));
            message = format!("{}\n\n[LINTER FEEDBACK]\n{}", original_message, feedback);
            // Continue retry loop
        }
    }
}
```

**Result**: Self-healing. Most errors fixed within 1-2 retries.

**Key insight**: The linter knows the grammar. The LLM can understand error messages. Together they converge.

---

## Pattern 5: Disambiguation as UX (Not Error)

**Problem**: "John" matches 3 people. LLM guesses wrong one.

**Solution**: Surface ambiguity to user. Let them choose.

```rust
UnifiedResolution::NeedsDisambiguation { items, .. } => {
    // Return to UI for user selection
    return Ok(AgentChatResponse {
        disambiguation: Some(DisambiguationRequest {
            items: vec![
                EntityMatch { 
                    param: "entity-id", 
                    search_text: "John",
                    matches: [
                        { name: "John Smith", id: uuid1 },
                        { name: "John Doe", id: uuid2 },
                        { name: "Johnny B. Good", id: uuid3 },
                    ]
                }
            ],
            prompt: "Which John did you mean?"
        }),
        ..
    });
}
```

**Result**: User is in control. No wrong guesses. Builds trust.

**Anti-pattern**:
```
LLM: "I'll assume you mean John Smith since he's the most recently mentioned."
// Wrong assumption, corrupted data
```

---

## Pattern 6: Context Injection over Instructions

**Problem**: "Remember to check if the entity exists before creating" - LLM forgets, ignores.

**Solution**: Inject available data into context. LLM sees what exists.

```rust
// Pre-resolve available entities
let pre_resolved = self.pre_resolve_entities().await;

// Inject into prompt
let context = format!(r#"
## Existing CBUs (use exact names)
  - "Apex Capital" (id: uuid1)
  - "Beta Fund" (id: uuid2)

## Existing Entities
  - "John Smith" (id: uuid3)
  - "Sarah Chen" (id: uuid4)

## Note on Codes
Product codes, roles, and jurisdictions will be auto-resolved.
Use natural language (e.g., "custody", "director", "Luxembourg").
"#);
```

**Also inject session state**:
```rust
// What's currently active
let session_context = format!(
    "[SESSION: Active CBU is {} (@cbu). {} entities bound.]",
    cbu_name, binding_count
);
```

**And semantic state** (workflow progress):
```rust
// Derived from database
let semantic_context = state.to_prompt_context();
// Shows: stage progress, blockers, missing entities, next actions
```

**Result**: LLM operates on real data, not rules about data.

---

## Pattern 7: Confidence Scoring

**Problem**: LLM confidently wrong. No signal for uncertainty.

**Solution**: Require confidence score. Threshold triggers clarification.

```json
{
    "intents": [...],
    "explanation": "Adding director role",
    "confidence": 0.85
}
```

**Scoring guide in prompt**:
```
0.95-1.0: Unambiguous, all info present
0.85-0.94: Clear intent, entity lookup needed
0.70-0.84: Some inference, minor assumptions
0.50-0.69: Significant assumptions - consider asking
<0.50: Multiple interpretations - MUST ask for clarification
```

**Enforcement**:
```rust
if confidence < 0.70 && !needs_clarification {
    feedback = "Low confidence without clarification request. Ask or improve interpretation.";
    continue; // Retry
}
```

---

## The Full Pipeline

```
┌─────────────────────────────────────────────────────────────────┐
│                        process_chat()                            │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 1. PROMPT ENRICHMENT                                      │   │
│  │    ├─ System prompt (role, verbs, constraints)           │   │
│  │    ├─ Pre-resolved entities (what exists)                │   │
│  │    ├─ Session context (@cbu, bindings)                   │   │
│  │    ├─ Semantic state (workflow progress)                 │   │
│  │    └─ KYC case context (if active)                       │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                   │
│                              ▼                                   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 2. LLM CALL (tool-use)                                   │   │
│  │    └─ Returns: structured intents + confidence           │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                   │
│                              ▼                                   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ 3. POST-PROCESSING                                        │   │
│  │    ├─ resolve_all() → entities + codes via Gateway       │   │
│  │    ├─ Disambiguation → user if ambiguous                 │   │
│  │    ├─ build_dsl_program() → deterministic Rust           │   │
│  │    └─ CSG Linter → semantic validation                   │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                   │
│                    ┌─────────┴─────────┐                        │
│                    ▼                   ▼                        │
│               [Valid DSL]      [Errors → Retry Loop]            │
│                    │                   │                        │
│                    ▼                   └───► Back to step 2     │
│            Ready for execution              with feedback       │
└─────────────────────────────────────────────────────────────────┘
```

---

## Comparison: OB-POC vs Common Approaches

| Aspect | OB-POC Pattern | Common (Naive) Approach |
|--------|----------------|------------------------|
| Output format | Tool-use JSON schema | Free text + regex |
| Vocabulary | Enum constraint | Prompt instructions |
| Entity refs | Gateway resolution | LLM guesses IDs |
| Code values | Gateway resolution | Trusted raw |
| Code generation | Deterministic Rust | LLM writes code |
| Validation | Linter + retry | Hope / regex check |
| Ambiguity | User disambiguation | LLM picks one |
| Context | Inject real data | Rules in prompt |
| Confidence | Scored + threshold | Not tracked |

---

## Summary

1. **Constrain at the boundary** - Tool schema limits what LLM can output
2. **Never trust values** - Everything through resolution layer
3. **Deterministic generation** - LLM → intent, Rust → code
4. **Validate and retry** - Linter feedback closes the loop
5. **Disambiguate, don't guess** - User decides on ambiguity
6. **Show, don't tell** - Inject available data, not rules
7. **Track confidence** - Surface uncertainty, require clarification

The LLM is a powerful intent extractor. Keep it in that role. Surround it with deterministic systems that handle the hard parts: resolution, generation, validation.
