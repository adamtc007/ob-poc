# TODO: LLM Hallucination Tightening

## Status: SUPERSEDED

The enum constraint approach for code values has been superseded by **unified resolution** - see `TODO-UNIFIED-CODE-RESOLUTION.md`.

**Key insight**: Codes (products, roles, jurisdictions) should go through the same EntityGateway resolution as entities, not be constrained via JSON schema enums.

## Remaining Items (Still Valid)

The following items from this doc are still relevant:

```rust
// In process_chat(), after LLM returns intents
fn validate_parameter_names(&self, intents: &[VerbIntent]) -> Vec<String> {
    let reg = registry();
    let mut warnings = Vec::new();
    
    for intent in intents {
        if let Some(verb_config) = reg.get_verb(&intent.verb) {
            let valid_params: HashSet<_> = verb_config.args
                .iter()
                .map(|a| a.name.as_str())
                .collect();
            
            for param_name in intent.params.keys() {
                if !valid_params.contains(param_name.as_str()) {
                    // Try to suggest correct name
                    let suggestion = find_closest_param(&valid_params, param_name);
                    warnings.push(format!(
                        "Unknown parameter '{}' for verb '{}'. Did you mean '{}'?",
                        param_name, intent.verb, suggestion
                    ));
                }
            }
        }
    }
    warnings
}
```

### 3. Add @result_N Validation

In the DAG builder or before DSL construction:

```rust
fn validate_result_references(intents: &[VerbIntent]) -> Result<(), String> {
    for (idx, intent) in intents.iter().enumerate() {
        for ref_value in intent.refs.values() {
            if let Some(n) = parse_result_ref(ref_value) {
                if n == 0 {
                    return Err(format!(
                        "Intent {}: @result_0 is invalid. References are 1-indexed.",
                        idx + 1
                    ));
                }
                if n > idx {
                    return Err(format!(
                        "Intent {}: @result_{} references a later intent. Forward references not allowed.",
                        idx + 1, n
                    ));
                }
            }
        }
    }
    Ok(())
}
```

### 4. Add Confidence Threshold Enforcement

In the prompt AND in code:

**Prompt addition:**
```markdown
## Confidence Rules (ENFORCED)

If your confidence is below 0.70, you MUST:
1. Set `needs_clarification: true`
2. Provide a clarification question
3. Return empty intents array

The system will reject low-confidence intents without clarification.
```

**Code enforcement:**
```rust
// After parsing LLM response
let confidence = tool_result.arguments["confidence"].as_f64().unwrap_or(0.0);
let needs_clarification = tool_result.arguments["needs_clarification"].as_bool().unwrap_or(false);

if confidence < 0.70 && !needs_clarification {
    feedback_context = format!(
        "Your confidence ({:.2}) is below 0.70 but you didn't request clarification. \
         Either increase confidence with better interpretation or ask for clarification.",
        confidence
    );
    continue; // Retry
}
```

### 5. Add Negative Examples to Prompt

Create `/rust/src/api/prompts/negative_examples.md`:

```markdown
# Common Mistakes to Avoid

## ❌ Wrong Parameter Names

```json
// WRONG - camelCase
{"params": {"clientType": "fund"}}

// RIGHT - kebab-case
{"params": {"client-type": "fund"}}
```

## ❌ Wrong Code Values

```json
// WRONG - abbreviation
{"params": {"product": "CUST"}}

// RIGHT - full code
{"params": {"product": "CUSTODY"}}

// WRONG - full name
{"params": {"jurisdiction": "Luxembourg"}}

// RIGHT - ISO code
{"params": {"jurisdiction": "LU"}}
```

## ❌ Wrong @result References

```json
// WRONG - 0-indexed
{"refs": {"cbu-id": "@result_0"}}

// RIGHT - 1-indexed
{"refs": {"cbu-id": "@result_1"}}

// WRONG - forward reference (from intent 2 to intent 3)
[
  {"verb": "...", "refs": {}},
  {"verb": "...", "refs": {"cbu-id": "@result_3"}},  // Can't reference future
  {"verb": "...", "refs": {}}
]
```
```

### 6. Improve Retry Feedback Specificity

```rust
// Instead of generic "Validation: Unknown product code"
fn format_validation_feedback(error: &ValidationError) -> String {
    match error.code {
        "UNKNOWN_PARAM" => {
            let suggestions = find_similar_params(&error.actual, &error.valid_options);
            format!(
                "Unknown parameter '{}' for verb '{}'. \
                 Valid parameters: {:?}. Did you mean '{}'?",
                error.actual, error.verb, error.valid_options, suggestions.first()
            )
        }
        "INVALID_CODE" => {
            format!(
                "Invalid {} code '{}'. Must be one of: {:?}",
                error.field, error.actual, error.valid_options
            )
        }
        _ => error.message.clone()
    }
}
```

## Implementation Priority

| Change | Effort | Impact | Priority |
|--------|--------|--------|----------|
| Enum constraints for codes | Medium | High | P1 |
| Parameter name validation | Low | Medium | P1 |
| @result_N validation | Low | Medium | P1 |
| Negative examples in prompt | Low | Medium | P2 |
| Confidence enforcement | Low | Low | P2 |
| Improved retry feedback | Medium | Medium | P3 |

## Files to Modify

1. `src/api/agent_service.rs` - `build_intent_tool()`, add validation layers
2. `src/api/prompts/negative_examples.md` - Create new file
3. `src/api/prompts/INTEGRATION.md` - Add Layer 9b for negative examples
4. `src/dsl_v2/csg_linter.rs` - Enhanced error messages with suggestions
