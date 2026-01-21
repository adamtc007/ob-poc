# Intent Pipeline Fixes - Implementation TODO

## Validation Summary

All issues below have been **verified against actual code**:

| Issue | File | Line(s) | Verdict |
|-------|------|---------|---------|
| A: All strings → Unresolved | intent_pipeline.rs | 251-252 | ✅ CONFIRMED |
| B: Missing args dropped | intent_pipeline.rs | 255 | ✅ CONFIRMED |
| C: Unresolved refs lack metadata | intent_pipeline.rs | 374-380 | ✅ CONFIRMED |
| D: Verb search returns Option | verb_search.rs | 312-341 | ✅ CONFIRMED |
| E: Arrays/objects stringified | intent_pipeline.rs | 256 | ✅ CONFIRMED |
| F: No audit trail | intent_pipeline.rs | - | ✅ CONFIRMED |
| G: Embedding computed 4x | verb_search.rs | 322,351,387,421 | ✅ CONFIRMED |
| H: Threshold hardcoded 0.5 | verb_search.rs | 390 | ✅ CONFIRMED |
| I: Two competing sources | verb_search.rs | 354 vs 390 | ✅ CONFIRMED |
| J: LIMIT 1 prevents ambiguity | verb_service.rs | 106,144 | ✅ CONFIRMED |
| K: Commit uses arg_key not ref_id | ast.rs | 896-923 | ⚠️ NEEDS IMPL |

---

## Critical Corrections Applied (from peer review)

| Original | Correction | Why |
|----------|------------|-----|
| `:name → Unresolved` | `:name → String` | `:name` has no lookup config - only fields with explicit `lookup` in YAML need resolution |
| Missing args → collect and continue | Missing args → **fail early** | Don't waste work on DSL compile. Return `NeedsUserInput` immediately. Include verb trace. |
| Write manual AST traversal | Use existing walker, extend it | `ast.rs:896` has `find_unresolved_ref_locations`. Add `path`, `search_column`, `ref_id` fields. |
| Overload `arg_key` with dotted paths | Separate `path: Vec<String>` field | Keeps `arg_key` clean, `path` explicit for nested structures |
| Delete `agent.invocation_phrases` | **Demote** to top-k union | May have curated phrases. Merge both sources, don't let it act as LIMIT 1 winner. |
| `AMBIGUITY_MARGIN = 0.10` | `AMBIGUITY_MARGIN = 0.05` | 10% too aggressive. Also add guard: second must be viable (>= threshold - 0.02). |
| Change shared `ArgumentValue` | Create pipeline-local `IntentArgValue` | Avoid cascading serde/DB/UI changes. Use `BTreeMap` for maps. |
| `.map().transpose().await?` | `if let Some(e) = &self.embedder { ... }` | Original pattern won't compile - `embed()` returns Future, not Result |
| (not mentioned) | **Add Problem K: ref_id commit** | Without this, lists with multiple EntityRefs break. Commit must use `ref_id`. |
| (not mentioned) | Verify `ref_id` uniqueness in lists | Each EntityRef needs unique span → unique ref_id |
| (not mentioned) | Verify DSL grammar supports `[]` and `{}` | Check `parser.rs` before implementing list/map formatting |

---

## Overview

This document contains fixes for the intent pipeline identified during peer review.
All fixes are surgical — they don't change architecture, just fix correctness bugs.

**Files to modify:**
- `rust/src/mcp/intent_pipeline.rs`
- `rust/src/mcp/verb_search.rs`
- `rust/src/dsl_v2/types.rs` (if ArgumentValue needs new variants)

**Priority Order:** A → B → C → D → E

---

## Problem A: All LLM Strings Treated as Unresolved (CRITICAL)

### Current Bug

In `intent_pipeline.rs`, the `extract_arguments` function treats ALL string values from the LLM as needing entity resolution:

```rust
// CURRENT (BUGGY) - around line 251-252
let value = match &arg["value"] {
    Value::String(s) => ArgumentValue::Unresolved(s.clone()),  // ← BUG
    // ...
};
```

This means `:jurisdiction "LU"` and `:status "ACTIVE"` get marked as unresolved even though they're just literal strings that don't need lookup.

### The Fix

Use the verb schema (`RuntimeArg.lookup`) to determine if a string needs resolution:

```rust
// FIXED VERSION
// In extract_arguments(), after parsing the LLM response:

let mut arguments = Vec::new();
if let Some(args) = parsed["arguments"].as_array() {
    for arg in args {
        let name = arg["name"].as_str().unwrap_or_default().to_string();
        if name.is_empty() {
            continue;
        }
        
        // Find the arg definition from verb schema
        let arg_def = verb_def.args.iter().find(|a| a.name == name);
        let needs_lookup = arg_def
            .map(|a| a.lookup.is_some())
            .unwrap_or(false);
        
        let value = match &arg["value"] {
            Value::String(s) => {
                if needs_lookup {
                    // This arg has lookup config - needs entity resolution
                    let entity_type = arg_def
                        .and_then(|a| a.lookup.as_ref())
                        .and_then(|l| l.entity_type.clone());
                    ArgumentValue::Unresolved {
                        value: s.clone(),
                        entity_type,
                    }
                } else {
                    // Plain string literal - no resolution needed
                    ArgumentValue::String(s.clone())
                }
            }
            Value::Number(n) => ArgumentValue::Number(n.as_f64().unwrap_or(0.0)),
            Value::Bool(b) => ArgumentValue::Boolean(*b),
            Value::Null => {
                // Handle in Problem B
                continue;
            }
            Value::Array(arr) => {
                // Handle in Problem E
                ArgumentValue::List(arr.iter().filter_map(|v| convert_json_value(v, arg_def)).collect())
            }
            Value::Object(obj) => {
                // Handle in Problem E
                ArgumentValue::Map(obj.iter().map(|(k, v)| (k.clone(), convert_json_value(v, None))).collect())
            }
        };
        
        arguments.push(IntentArgument {
            name,
            value,
            resolved: false,
        });
    }
}
```

### Required Changes to ArgumentValue Enum

**IMPORTANT: Create a pipeline-local type to avoid cascading changes.**

Changing `dsl_v2/types.rs::ArgumentValue` can cause ripples across:
- Serde serialization boundaries
- DB JSON blobs
- UI payloads
- Existing tests

**Recommendation: Define `IntentArgValue` in intent_pipeline.rs:**

```rust
// In intent_pipeline.rs - PIPELINE LOCAL, not shared with DSL runtime

use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum IntentArgValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Reference(String),      // @symbol reference
    Uuid(String),           // Resolved UUID
    Unresolved {            // Needs entity resolution
        value: String,
        entity_type: Option<String>,
    },
    Missing {               // Required but not extracted
        arg_name: String,
        required: bool,
    },
    List(Vec<IntentArgValue>),
    Map(BTreeMap<String, IntentArgValue>),  // BTreeMap for stable ordering/lookup
}
```

**Why BTreeMap instead of Vec<(String, ...)>?**
- Simpler lookup by key
- Stable serialization order
- Cleaner JSON output

Keep the DSL AST's `ArgumentValue` unchanged. The pipeline uses its own type for intent extraction, then converts to DSL syntax during assembly.

### Verification

After this fix:
- `(cbu.create :name "Apex Fund" :jurisdiction "LU" :client "Allianz")` should have:
  - `:name` → `String("Apex Fund")` — NO lookup config, it's just a name field
  - `:jurisdiction` → `String("LU")` — NO lookup config for jurisdiction
  - `:client` → `Unresolved { value: "Allianz", entity_type: Some("entity") }` — HAS lookup config

**CRITICAL**: Don't assume `:name` needs resolution! Only fields with explicit `lookup` config in YAML need entity resolution. Most string fields (`:name`, `:notes`, `:description`, `:reason`) are just literals.

---

## Problem B: Missing Required Args Silently Dropped (CRITICAL)

### Current Bug

```rust
// CURRENT (BUGGY) - around line 255
Value::Null => continue,  // Required args just vanish!
```

If the LLM can't extract a required argument, it returns `null`. Currently this is silently skipped, and the error only surfaces later during DSL validation with a confusing message.

### The Fix

Track missing required args explicitly:

```rust
// FIXED VERSION
Value::Null => {
    let is_required = arg_def.map(|a| a.required).unwrap_or(false);
    if is_required {
        // Track missing required arg explicitly
        arguments.push(IntentArgument {
            name: name.clone(),
            value: ArgumentValue::Missing {
                arg_name: name,
                required: true,
            },
            resolved: false,
        });
    }
    // Skip optional nulls
    continue;
}
```

### Update PipelineResult

Add a field to surface missing args clearly:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResult {
    pub intent: StructuredIntent,
    pub verb_candidates: Vec<VerbSearchResult>,
    pub dsl: String,
    pub valid: bool,
    pub validation_error: Option<String>,
    pub unresolved_refs: Vec<UnresolvedRef>,
    pub missing_required: Vec<String>,  // ← NEW
}
```

### Update assemble_dsl

Handle the Missing variant:

```rust
fn assemble_dsl(&self, intent: &StructuredIntent) -> Result<(String, Vec<UnresolvedRef>, Vec<String>)> {
    let mut dsl = format!("({}", intent.verb);
    let mut unresolved = Vec::new();
    let mut missing_required = Vec::new();

    for arg in &intent.arguments {
        let value_str = match &arg.value {
            ArgumentValue::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
            ArgumentValue::Number(n) => n.to_string(),
            ArgumentValue::Boolean(b) => b.to_string(),
            ArgumentValue::Reference(r) => format!("@{}", r),
            ArgumentValue::Uuid(u) => format!("\"{}\"", u),
            ArgumentValue::Unresolved { value, entity_type } => {
                unresolved.push(UnresolvedRef {
                    param_name: arg.name.clone(),
                    search_value: value.clone(),
                    entity_type: entity_type.clone(),
                });
                format!("\"{}\"", value.replace('"', "\\\""))
            }
            ArgumentValue::Missing { arg_name, required } => {
                if *required {
                    missing_required.push(arg_name.clone());
                }
                continue; // Don't include in DSL
            }
            ArgumentValue::List(items) => {
                // Handle in Problem E
                format_list(items)
            }
            ArgumentValue::Map(entries) => {
                // Handle in Problem E
                format_map(entries)
            }
        };
        dsl.push_str(&format!(" :{} {}", arg.name, value_str));
    }

    dsl.push(')');
    Ok((dsl, unresolved, missing_required))
}
```

### CRITICAL: Fail Early on Missing Required Args

Don't assemble DSL and try to validate if required args are missing. Return early:

```rust
// In process() method, BEFORE assemble_dsl:

// Check for missing required args FIRST
let missing_required: Vec<String> = intent.arguments
    .iter()
    .filter_map(|arg| match &arg.value {
        IntentArgValue::Missing { arg_name, required: true } => Some(arg_name.clone()),
        _ => None,
    })
    .collect();

// FAIL EARLY - do NOT:
// - assemble DSL
// - parse DSL
// - enrich AST
// - validate
// This avoids wasted work and confusing "validation_error" noise
if !missing_required.is_empty() {
    return Ok(PipelineResult {
        intent,
        verb_candidates: candidates,  // ← Still include verb selection for debugging
        dsl: String::new(),           // No DSL generated
        valid: false,
        validation_error: Some(format!(
            "Missing required arguments: {}",
            missing_required.join(", ")
        )),
        unresolved_refs: vec![],
        missing_required,
        outcome: PipelineOutcome::NeedsUserInput,
        verb_selection_trace: Some(trace),  // ← Include trace even on early exit
    });
}

// Only NOW assemble and validate DSL
let (dsl, unresolved, _) = self.assemble_dsl(&intent)?;
```

**Key point:** Include `verb_candidates` and `verb_selection_trace` even on early exits so the user/agent can see what verb was chosen and debug if needed.

### Add PipelineOutcome Enum

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PipelineOutcome {
    /// DSL ready for execution (may have unresolved refs)
    Ready,
    /// Missing required arguments - need user input
    NeedsUserInput,
    /// Ambiguous verb selection - need clarification
    NeedsClarification,
    /// No matching verb found
    NoMatch,
}
```

### Verification

After this fix:
- If LLM returns `{"name": "lei", "value": null}` for required `:lei`, the pipeline result should show:
  - `missing_required: ["lei"]`
  - Clear error message: "Missing required argument: lei"

---

## Problem C: Derive Unresolved Refs from Enriched AST (IMPORTANT)

### Current Bug

`assemble_dsl()` builds its own `UnresolvedRef` list with `entity_type: None`, but the enrichment pass (`enrichment.rs`) already has all the metadata from YAML config.

### The Fix

Use the **existing walker utilities** in `ast.rs` instead of writing new traversal code:

```rust
// ast.rs already has this at line 896:
pub fn find_unresolved_ref_locations(program: &Program) -> Vec<UnresolvedRefLocation>
```

But the existing walker is incomplete - it needs to be extended to:
1. Include `ref_id` and `search_column`
2. Handle lists with multiple EntityRefs
3. Handle nested maps

### Step 1: Extend UnresolvedRefLocation

```rust
// In ast.rs, update the struct:
#[derive(Debug, Clone)]
pub struct UnresolvedRefLocation {
    pub statement_index: usize,
    pub arg_key: String,              // Top-level arg name only (e.g., "clients")
    pub path: Vec<String>,            // ← NEW: nested location (e.g., ["0", "entity"])
    pub entity_type: String,
    pub search_text: String,
    pub search_column: Option<String>,  // From LookupConfig
    pub ref_id: Option<String>,         // Span-based ID for commit targeting
}
```

**Why separate `path` instead of dotted `arg_key`?**
- `arg_key` stays clean as the top-level argument name
- `path` is explicit for nested structures (lists use indices, maps use keys)
- Commit uses `ref_id` anyway - path is purely for debugging/display
- Avoids confusion about whether "clients.0" is an arg name or a path

### Step 2: Extend the Walker to Handle Lists

```rust
// In ast.rs, replace find_unresolved_ref_locations:
pub fn find_unresolved_ref_locations(program: &Program) -> Vec<UnresolvedRefLocation> {
    let mut results = Vec::new();

    for (stmt_idx, stmt) in program.statements.iter().enumerate() {
        if let Statement::VerbCall(vc) = stmt {
            for arg in &vc.arguments {
                collect_unresolved_from_node(
                    &arg.value, 
                    &arg.key, 
                    vec![],  // Start with empty path
                    stmt_idx, 
                    &mut results
                );
            }
        }
    }

    results
}

fn collect_unresolved_from_node(
    node: &AstNode,
    arg_key: &str,
    path: Vec<String>,  // ← Current path for nested structures
    stmt_idx: usize,
    results: &mut Vec<UnresolvedRefLocation>,
) {
    match node {
        AstNode::EntityRef {
            entity_type,
            search_column,
            value,
            resolved_key,
            ref_id,
            ..
        } => {
            if resolved_key.is_none() {
                results.push(UnresolvedRefLocation {
                    statement_index: stmt_idx,
                    arg_key: arg_key.to_string(),
                    path: path.clone(),
                    entity_type: entity_type.clone(),
                    search_text: value.clone(),
                    search_column: Some(search_column.clone()),
                    ref_id: ref_id.clone(),
                });
            }
        }
        AstNode::List { items, .. } => {
            for (idx, item) in items.iter().enumerate() {
                let mut child_path = path.clone();
                child_path.push(idx.to_string());  // List index as path segment
                collect_unresolved_from_node(item, arg_key, child_path, stmt_idx, results);
            }
        }
        AstNode::Map { entries, .. } => {
            for (key, value) in entries {
                let mut child_path = path.clone();
                child_path.push(key.clone());  // Map key as path segment
                collect_unresolved_from_node(value, arg_key, child_path, stmt_idx, results);
            }
        }
        _ => {}
    }
}
```

### Step 3: Use Walker in Intent Pipeline

```rust
// In intent_pipeline.rs process() method:

// Step 5: Parse and enrich to get full unresolved ref metadata
let unresolved_refs = self.extract_unresolved_from_dsl(&dsl)?;

// ...

fn extract_unresolved_from_dsl(&self, dsl: &str) -> Result<Vec<UnresolvedRef>> {
    use crate::dsl_v2::{parse_program, registry};
    use crate::dsl_v2::enrichment::enrich_program;
    use crate::dsl_v2::ast::find_unresolved_ref_locations;  // ← USE EXISTING WALKER
    
    let ast = parse_program(dsl).map_err(|e| anyhow!("Parse error: {:?}", e))?;
    let reg = registry();
    let enriched = enrich_program(ast, reg);
    
    // Use canonical walker - handles lists, maps, nested structures
    let locations = find_unresolved_ref_locations(&enriched.program);
    
    // Map to API type
    Ok(locations
        .into_iter()
        .map(|loc| UnresolvedRef {
            param_name: loc.arg_key,
            search_value: loc.search_text,
            entity_type: Some(loc.entity_type),
            search_column: loc.search_column,
            ref_id: loc.ref_id,
        })
        .collect())
}
```

### Why Use Existing Walker?

Writing manual traversal will miss:
- Nested maps inside lists
- Multiple unresolved refs under a single arg key
- Statement-level metadata needed for commit targeting

The canonical walker handles all edge cases.

### Verification

After this fix, `UnresolvedRef` should have full metadata:
```rust
UnresolvedRef {
    param_name: "clients",        // ← arg key
    search_value: "Allianz",
    entity_type: Some("entity"),
    search_column: Some("name"),
    ref_id: Some("0:15-25"),      // ← span-based, stable for commit
}
```

---

## Problem D: Verb Search Top-K and Ambiguity Detection (IMPORTANT)

### Current Behavior

The semantic search methods return `Option<Match>` (single best match):

```rust
// verb_search.rs lines 312-341
async fn search_user_learned_semantic(...) -> Result<Option<VerbSearchResult>>
async fn search_learned_semantic(...) -> Result<Option<VerbSearchResult>>
```

### The Fix

1. Update VerbService methods to return `Vec<Match>` with top-k
2. Add ambiguity detection before calling LLM

### Update VerbService Interface

In your `database/verb_service.rs`:

```rust
// Change from:
pub async fn find_user_learned_semantic(...) -> Result<Option<SemanticMatch>>

// To:
pub async fn find_user_learned_semantic_topk(
    &self,
    user_id: Uuid,
    embedding: &[f32],
    threshold: f32,
    limit: usize,  // ← NEW
) -> Result<Vec<SemanticMatch>>
```

### Update HybridVerbSearcher

```rust
// Add constants at top of verb_search.rs
const AMBIGUITY_MARGIN: f32 = 0.05;  // If top two scores within 5%, it's ambiguous (start conservative)

// Update search method signature
pub async fn search(
    &self,
    query: &str,
    user_id: Option<Uuid>,
    domain_filter: Option<&str>,
    limit: usize,
) -> Result<VerbSearchOutcome> {  // ← Changed return type
    // ... existing code to collect results ...
    
    // After collecting all results and sorting by score:
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    
    // Check if top result meets threshold
    if results.is_empty() || results[0].score < self.semantic_threshold {
        return Ok(VerbSearchOutcome::NoMatch {
            suggestions: results.into_iter().take(5).collect(),
        });
    }
    
    // Check for ambiguity - with extra guard
    // Only ambiguous if BOTH top candidates are strong enough to matter
    if results.len() >= 2 {
        let top = results[0].score;
        let second = results[1].score;
        let margin = top - second;
        
        // Extra guard: second must also be "close enough" to be a real contender
        // (prevents "top=0.92, second=0.55" from being ambiguous due to float noise)
        let second_is_viable = second >= self.semantic_threshold - 0.02;
        
        if second_is_viable && margin < AMBIGUITY_MARGIN {
            // Ambiguous - return candidates for user clarification
            return Ok(VerbSearchOutcome::Ambiguous {
                candidates: results.into_iter().take(3).collect(),
                margin,
            });
        }
    }
    
    // Clear winner
    Ok(VerbSearchOutcome::Matched {
        selected: results.remove(0),
        alternatives: results.into_iter().take(4).collect(),
    })
}
```

### Add VerbSearchOutcome Enum

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerbSearchOutcome {
    /// Clear winner - proceed to LLM extraction
    Matched {
        selected: VerbSearchResult,
        alternatives: Vec<VerbSearchResult>,
    },
    /// Top candidates too close - ask user to clarify
    Ambiguous {
        candidates: Vec<VerbSearchResult>,
        margin: f32,
    },
    /// No matches above threshold
    NoMatch {
        suggestions: Vec<VerbSearchResult>,
    },
}
```

### Update IntentPipeline to Handle Outcomes

```rust
// In process() method:

let search_outcome = self.verb_searcher.search(instruction, None, domain_hint, 5).await?;

match search_outcome {
    VerbSearchOutcome::Matched { selected, alternatives } => {
        // Proceed with LLM extraction using selected.verb
        let intent = self.extract_arguments(instruction, &selected.verb, verb_def, selected.score).await?;
        // ... rest of pipeline
    }
    VerbSearchOutcome::Ambiguous { candidates, margin } => {
        // Return early - ask user to clarify
        return Ok(PipelineResult {
            intent: StructuredIntent::empty(),
            verb_candidates: candidates,
            dsl: String::new(),
            valid: false,
            validation_error: Some(format!(
                "Ambiguous intent - top matches within {:.0}% of each other. Please clarify.",
                margin * 100.0
            )),
            unresolved_refs: vec![],
            missing_required: vec![],
            outcome: PipelineOutcome::NeedsClarification,
        });
    }
    VerbSearchOutcome::NoMatch { suggestions } => {
        return Err(anyhow!(
            "No matching verbs found. Did you mean: {}?",
            suggestions.iter().map(|s| &s.verb).take(3).collect::<Vec<_>>().join(", ")
        ));
    }
}
```

### Verification

After this fix:
- "create a cbu" with scores `[cbu.create: 0.92, entity.create: 0.88]` → Ambiguous (margin 0.04)
- "create a new cbu for allianz" with scores `[cbu.create: 0.95, entity.create: 0.72]` → Matched

---

## Problem E: Proper Array/Map Coercion (MODERATE)

### Current Bug

```rust
// CURRENT (BUGGY) - line 256
_ => ArgumentValue::Unresolved(arg["value"].to_string()),
```

Arrays and objects get stringified: `[{"a": 1}]` → `"[{\"a\":1}]"` — loses structure.

### The Fix

Add proper coercion for complex types:

```rust
/// Convert JSON value to ArgumentValue using verb arg definition
fn convert_json_value(
    value: &Value,
    arg_def: Option<&RuntimeArg>,
) -> Option<ArgumentValue> {
    let needs_lookup = arg_def
        .map(|a| a.lookup.is_some())
        .unwrap_or(false);
    let entity_type = arg_def
        .and_then(|a| a.lookup.as_ref())
        .and_then(|l| l.entity_type.clone());
    
    match value {
        Value::Null => None,
        
        Value::Bool(b) => Some(ArgumentValue::Boolean(*b)),
        
        Value::Number(n) => Some(ArgumentValue::Number(n.as_f64().unwrap_or(0.0))),
        
        Value::String(s) => {
            // Check if it looks like a UUID
            if uuid::Uuid::parse_str(s).is_ok() {
                Some(ArgumentValue::Uuid(s.clone()))
            } else if needs_lookup {
                Some(ArgumentValue::Unresolved {
                    value: s.clone(),
                    entity_type,
                })
            } else {
                Some(ArgumentValue::String(s.clone()))
            }
        }
        
        Value::Array(arr) => {
            let items: Vec<ArgumentValue> = arr
                .iter()
                .filter_map(|v| convert_json_value(v, arg_def))
                .collect();
            Some(ArgumentValue::List(items))
        }
        
        Value::Object(obj) => {
            let entries: Vec<(String, ArgumentValue)> = obj
                .iter()
                .filter_map(|(k, v)| {
                    convert_json_value(v, None).map(|av| (k.clone(), av))
                })
                .collect();
            Some(ArgumentValue::Map(entries))
        }
    }
}
```

### Update assemble_dsl for Lists/Maps

```rust
fn format_list(items: &[ArgumentValue]) -> String {
    let formatted: Vec<String> = items.iter().map(|item| format_value(item)).collect();
    format!("[{}]", formatted.join(" "))
}

fn format_map(entries: &[(String, ArgumentValue)]) -> String {
    let formatted: Vec<String> = entries
        .iter()
        .map(|(k, v)| format!(":{} {}", k, format_value(v)))
        .collect();
    format!("{{{}}}", formatted.join(" "))
}

fn format_value(value: &IntentArgValue) -> String {
    match value {
        IntentArgValue::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
        IntentArgValue::Number(n) => n.to_string(),
        IntentArgValue::Boolean(b) => b.to_string(),
        IntentArgValue::Reference(r) => format!("@{}", r),
        IntentArgValue::Uuid(u) => format!("\"{}\"", u),
        IntentArgValue::Unresolved { value, .. } => format!("\"{}\"", value.replace('"', "\\\"")),
        IntentArgValue::Missing { .. } => "nil".to_string(),
        IntentArgValue::List(items) => format_list(items),
        IntentArgValue::Map(entries) => format_map(entries),
    }
}
```

**⚠️ VERIFY: DSL Grammar Supports [] and {}**

Before implementing, confirm your DSL parser accepts:
- Lists: `[item1 item2 item3]`
- Maps: `{:key1 value1 :key2 value2}`

If your grammar expects different forms (e.g., `(list ...)` or `(map ...)`), adjust the formatter accordingly. Check `parser.rs` for the actual syntax.

### Verification

After this fix:
- LLM returns `{"filters": [{"field": "status", "value": "ACTIVE"}]}`
- Produces: `ArgumentValue::List([ArgumentValue::Map([("field", String("status")), ("value", String("ACTIVE"))])])`
- DSL output: `:filters [{:field "status" :value "ACTIVE"}]`

---

## Problem F: Audit Trail for Determinism (NICE-TO-HAVE)

### What to Add

Record verb selection reasoning in the pipeline result:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbSelectionTrace {
    pub selected_verb: String,
    pub selected_score: f32,
    pub source_tier: VerbSearchSource,
    pub candidates: Vec<VerbCandidate>,
    pub semantic_threshold: f32,
    pub ambiguity_margin: f32,
    pub embedder_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbCandidate {
    pub verb: String,
    pub score: f32,
    pub source: VerbSearchSource,
}
```

Add to `PipelineResult`:

```rust
pub struct PipelineResult {
    // ... existing fields ...
    pub verb_selection_trace: Option<VerbSelectionTrace>,  // ← NEW
}
```

---

## Testing Checklist

After implementing all fixes, verify:

### Problem A Tests
```rust
#[tokio::test]
async fn test_string_literal_not_marked_unresolved() {
    // Setup pipeline with verb that has :jurisdiction (no lookup) and :client (has lookup)
    let result = pipeline.process("create cbu Apex in Luxembourg", None).await.unwrap();
    
    // :jurisdiction should be String, not Unresolved
    let jurisdiction = result.intent.arguments.iter().find(|a| a.name == "jurisdiction");
    assert!(matches!(jurisdiction.unwrap().value, ArgumentValue::String(_)));
    
    // :client should be Unresolved with entity_type
    let client = result.intent.arguments.iter().find(|a| a.name == "client");
    assert!(matches!(client.unwrap().value, ArgumentValue::Unresolved { entity_type: Some(_), .. }));
}
```

### Problem B Tests
```rust
#[tokio::test]
async fn test_missing_required_tracked() {
    // Force LLM to return null for required :lei
    let result = pipeline.process("create entity", None).await.unwrap();
    
    assert!(result.missing_required.contains(&"lei".to_string()));
    assert!(!result.valid);
}
```

### Problem C Tests
```rust
#[tokio::test]
async fn test_unresolved_refs_have_full_metadata() {
    let result = pipeline.process("view book for Allianz", None).await.unwrap();
    
    let ref_ = &result.unresolved_refs[0];
    assert!(ref_.entity_type.is_some());
    assert!(ref_.search_column.is_some());
    assert!(ref_.ref_id.is_some());
}
```

### Problem D Tests
```rust
#[tokio::test]
async fn test_ambiguous_verb_detection() {
    // Query that matches multiple verbs closely
    let outcome = searcher.search("create new client", None, None, 5).await.unwrap();
    
    assert!(matches!(outcome, VerbSearchOutcome::Ambiguous { .. }));
}
```

### Problem E Tests
```rust
#[tokio::test]
async fn test_array_value_preserved() {
    // Verb with array argument
    let result = pipeline.process("batch create with items A, B, C", None).await.unwrap();
    
    let items = result.intent.arguments.iter().find(|a| a.name == "items");
    assert!(matches!(items.unwrap().value, ArgumentValue::List(_)));
}
```

---

## Problem G: Repeated Embedding Computation (PERFORMANCE CRITICAL)

### Current Bug

The embedding is computed **4 separate times** for the same query:

```rust
// verb_search.rs - FOUR separate embed() calls for same query!
// Line 322: search_user_learned_semantic
let query_embedding = embedder.embed(query).await?;

// Line 351: search_learned_semantic  
let query_embedding = embedder.embed(query).await?;

// Line 387: search_global_semantic
let query_embedding = embedder.embed(query).await?;

// Line 421: check_blocklist
let query_embedding = embedder.embed(query).await?;
```

Even with Candle (~5-10ms per embed), that's 20-40ms wasted per search.

### The Fix

Compute embedding once at the top of `search()` and pass it through:

```rust
impl HybridVerbSearcher {
    pub async fn search(
        &self,
        query: &str,
        user_id: Option<Uuid>,
        domain_filter: Option<&str>,
        limit: usize,
    ) -> Result<VerbSearchOutcome> {
        // Normalize ONCE at the start (also used for exact matching)
        let normalized = query.trim().to_lowercase();
        
        // Compute embedding ONCE at the start
        let query_embedding = if self.has_semantic_capability() {
            Some(self.embedder.as_ref().unwrap().embed(query).await?)
        } else {
            None
        };
        
        // ... exact match paths use `normalized` ...
        
        // Pass embedding reference to semantic methods
        if let Some(ref embedding) = query_embedding {
            if let Some(result) = self.search_user_learned_semantic_with_embedding(
                user_id.unwrap(), 
                embedding
            ).await? {
                // ...
            }
        }
        
        // ... rest of search logic using &query_embedding ...
    }
    
    // Update method signatures to take embedding reference
    async fn search_user_learned_semantic_with_embedding(
        &self,
        user_id: Uuid,
        query_embedding: &[f32],  // ← Now takes reference, no embed() call
    ) -> Result<Option<VerbSearchResult>> {
        let verb_service = self.verb_service.as_ref().unwrap();
        
        let result = verb_service
            .find_user_learned_semantic(user_id, query_embedding, self.semantic_threshold)
            .await?;
        // ...
    }
}
```

### Also Cache Normalized Query

Computing `normalized = query.trim().to_lowercase()` once and reusing it:
- Ensures consistency between exact and semantic comparisons
- Can be used as cache key if you add embedding caching later
- Reduces subtle bugs from different normalization in different paths

### VerbService Already Accepts &[f32]

Good news - the DB methods already take `&[f32]`:

```rust
// verb_service.rs - already correct!
pub async fn find_user_learned_semantic(
    &self,
    user_id: Uuid,
    query_embedding: &[f32],  // ← Already takes reference
    threshold: f32,
) -> Result<Option<SemanticMatch>, sqlx::Error>
```

The fix is entirely in `verb_search.rs` - compute once, pass through.

### Verification

After this fix:
- Single query should have exactly ONE `embed()` call
- Search latency should drop by ~15-30ms

---

## Problem H: Inconsistent Similarity Thresholds (CRITICAL)

### Current Bug

```rust
// verb_search.rs
// Learned semantic paths use semantic_threshold (0.80):
.find_user_learned_semantic(user_id, &query_embedding, self.semantic_threshold)  // 0.80
.find_global_learned_semantic(&query_embedding, self.semantic_threshold)          // 0.80

// But primary semantic lookup uses HARDCODED 0.5:
.search_verb_patterns_semantic(&query_embedding, limit, 0.5)  // ← HARDCODED!
```

This means:
- A 0.52 similarity match from `verb_pattern_embeddings` can win
- But a 0.78 match from learned phrases is rejected
- Totally inconsistent behavior

### The Fix

Use `semantic_threshold` consistently, or define explicit fallback threshold:

```rust
// Option 1: Use same threshold everywhere
.search_verb_patterns_semantic(&query_embedding, limit, self.semantic_threshold)

// Option 2: Define explicit fallback threshold (preferred)
impl HybridVerbSearcher {
    semantic_threshold: f32,       // 0.80 - high confidence
    fallback_threshold: f32,       // 0.65 - cold start acceptable  
    blocklist_threshold: f32,      // 0.75
}

// Then in search_global_semantic:
.search_verb_patterns_semantic(&query_embedding, limit, self.fallback_threshold)
```

### Recommended Thresholds

```rust
const SEMANTIC_THRESHOLD: f32 = 0.80;    // Learned matches - high confidence
const FALLBACK_THRESHOLD: f32 = 0.65;    // Cold start - lower but reasonable
const AMBIGUITY_MARGIN: f32 = 0.05;      // Top-2 gap for ambiguity (start conservative)
const BLOCKLIST_THRESHOLD: f32 = 0.75;   // Block confidence
```

**Note on margin:** 0.05 (5 points) is conservative. If you get too few "ambiguous" cases, increase to 0.08. If too many interruptions, decrease to 0.03. Start conservative.

### Verification

After this fix:
- No more 0.52 matches sneaking through
- Clear policy: learned > 0.80, cold start > 0.65

---

## Problem I: Two Competing Global Semantic Sources (ARCHITECTURAL)

### Current Behavior

The search flow has TWO global semantic sources:

```
1. user learned exact (DB)
2. global learned exact (in-memory)
3. user learned semantic (DB, LIMIT 1)        ← agent.user_learned_phrases
4. global learned semantic (DB, LIMIT 1)      ← agent.invocation_phrases  ⚠️
5. blocklist check
6. global semantic (DB, LIMIT K)              ← verb_pattern_embeddings   ⚠️
```

Steps 4 and 6 are BOTH global semantic sources competing:
- `agent.invocation_phrases` - returns LIMIT 1, checked BEFORE primary
- `verb_pattern_embeddings` - returns LIMIT K, the actual canonical source

A mediocre match (0.81) in `invocation_phrases` blocks consultation of the primary table.

### The Fix: DEMOTE, Don't Delete

**Don't delete `agent.invocation_phrases` blindly!** It might contain:
- Curated high-quality phrases
- User-feedback boosted entries
- A faster index for common queries

**Safer approach: Demote it to participate in top-k union:**

```rust
// REVISED search order:
pub async fn search(...) -> Result<VerbSearchOutcome> {
    // 1. User learned exact (DB) - highest priority
    // 2. Global learned exact (in-memory)
    // 3. User learned semantic (DB, top-k)
    // 4. Global semantic - UNION of:
    //    - agent.invocation_phrases (top-k, same threshold)
    //    - verb_pattern_embeddings (top-k, same threshold)
    //    Then dedupe by verb, keep highest score
    // 5. Apply blocklist filtering across ALL candidates
    // 6. Apply threshold + margin policy
    // 7. Return outcome (Matched/Ambiguous/NoMatch)
}
```

### Option A: Merge into Single Query (Preferred)

If both tables have same schema, union them:

```rust
// In verb_service.rs
pub async fn search_all_global_semantic(
    &self,
    query_embedding: &[f32],
    limit: usize,
    threshold: f32,
) -> Result<Vec<SemanticMatch>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, f64, String, Option<String>)>(
        r#"
        WITH combined AS (
            -- Invocation phrases (curated)
            SELECT phrase, verb, 1 - (embedding <=> $1::vector) as similarity, 
                   'invocation' as source_tier, NULL as category
            FROM agent.invocation_phrases
            WHERE embedding IS NOT NULL
              AND 1 - (embedding <=> $1::vector) > $3
            
            UNION ALL
            
            -- Verb pattern embeddings (canonical)
            SELECT pattern_phrase, verb_name, 1 - (embedding <=> $1::vector), 
                   'pattern' as source_tier, category
            FROM "ob-poc".verb_pattern_embeddings
            WHERE embedding IS NOT NULL
              AND 1 - (embedding <=> $1::vector) > $3
        )
        SELECT phrase, verb, similarity, source_tier, category
        FROM combined
        ORDER BY similarity DESC
        LIMIT $2
        "#,
    )
    .bind(query_embedding)
    .bind(limit as i32)
    .bind(threshold)
    .fetch_all(&self.pool)
    .await?;
    
    // ... map to SemanticMatch with source_tier field
}
```

**⚠️ Metric Semantics Warning:**

Both tables MUST use the same distance metric. You're computing:
```sql
similarity = 1 - (embedding <=> vector)
```

This is correct for **cosine distance** in pgvector (because `<=>` returns cosine distance = 1 - cosine_similarity).

If any table uses a different operator (e.g., `<->` for L2), the scores will be incomparable nonsense. Verify both tables use cosine.

**Source Tracking:**

Keep `source_tier` and `category` as separate fields:
- `source_tier`: "invocation" vs "pattern" (which table)
- `category`: domain/grouping from verb_pattern_embeddings (semantic category)

Don't overload `category` to mean both - it makes debugging harder.

### Option B: Keep Separate, Query Both, Merge in Rust

```rust
// In verb_search.rs
async fn search_global_semantic(&self, embedding: &[f32], limit: usize) -> Result<Vec<VerbSearchResult>> {
    let verb_service = self.verb_service.as_ref().unwrap();
    
    // Query BOTH sources with same threshold
    let invocation_matches = verb_service
        .find_global_learned_semantic_topk(embedding, self.fallback_threshold, limit)
        .await?;
    
    let pattern_matches = verb_service
        .search_verb_patterns_semantic(embedding, limit, self.fallback_threshold)
        .await?;
    
    // Merge and dedupe by verb, keeping highest score
    let mut by_verb: HashMap<String, VerbSearchResult> = HashMap::new();
    
    for m in invocation_matches.into_iter().chain(pattern_matches) {
        by_verb.entry(m.verb.clone())
            .and_modify(|existing| {
                if m.similarity > existing.score as f64 {
                    existing.score = m.similarity as f32;
                }
            })
            .or_insert(/* ... */);
    }
    
    let mut results: Vec<_> = by_verb.into_values().collect();
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    results.truncate(limit);
    
    Ok(results)
}
```

### What NOT To Do

❌ Don't let `invocation_phrases` act as LIMIT 1 winner ahead of canonical table
❌ Don't use different thresholds for the two tables
❌ Don't blindly delete if it has curated content

### Verification

After this fix:
- Single unified global semantic search
- Both sources contribute to top-k candidates
- Best match wins regardless of source

---

## Problem J: LIMIT 1 Prevents Ambiguity Detection (CRITICAL)

This is the DB-layer manifestation of Problem D. The VerbService methods need to return top-k:

### Current Bug

```rust
// verb_service.rs
// Both return LIMIT 1:
pub async fn find_user_learned_semantic(...) -> Result<Option<SemanticMatch>>
pub async fn find_global_learned_semantic(...) -> Result<Option<SemanticMatch>>
```

### The Fix

Add top-k variants:

```rust
// verb_service.rs - ADD these methods

/// Find user-learned phrases by semantic similarity (top-k)
pub async fn find_user_learned_semantic_topk(
    &self,
    user_id: Uuid,
    query_embedding: &[f32],
    threshold: f32,
    limit: usize,
) -> Result<Vec<SemanticMatch>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, f32, f64)>(
        r#"
        SELECT phrase, verb, confidence, 1 - (embedding <=> $1::vector) as similarity
        FROM agent.user_learned_phrases
        WHERE user_id = $2
          AND embedding IS NOT NULL
          AND 1 - (embedding <=> $1::vector) > $3
        ORDER BY embedding <=> $1::vector
        LIMIT $4
        "#,
    )
    .bind(query_embedding)
    .bind(user_id)
    .bind(threshold)
    .bind(limit as i32)
    .fetch_all(&self.pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(phrase, verb, confidence, similarity)| SemanticMatch {
            phrase,
            verb,
            similarity,
            confidence: Some(confidence),
            category: None,
        })
        .collect())
}
```

Note: Move threshold filtering INTO the SQL query (line 7: `AND 1 - (embedding <=> $1::vector) > $3`) instead of filtering in Rust. This is more efficient.

### Verification

After this fix:
- Can detect ambiguity by comparing top-1 vs top-2 scores
- Enables the margin-based policy from Problem D

---

## Problem K: Commit Resolution by ref_id (CRITICAL for Lists/Maps)

### The Problem

You've added `ref_id` to `UnresolvedRef` and `UnresolvedRefLocation`. But if the resolution/commit path still uses `(statement_index, arg_key)` to write `resolved_key` back, **lists with multiple EntityRefs will break**.

Example:
```clojure
(batch.create :clients ["Allianz" "BlackRock" "Vanguard"])
```

All three are under `arg_key: "clients"`. If commit uses arg_key, you can only resolve one.

### The Fix

The commit endpoint must:
1. Accept `ref_id` (e.g., `"0:15-25"`)
2. Find the exact `EntityRef` node by `ref_id`
3. Write `resolved_key` to that specific node

### Step 1: Commit API Takes ref_id

```rust
// In your resolution endpoint/handler:
#[derive(Debug, Deserialize)]
pub struct ResolveEntityRequest {
    pub ref_id: String,           // ← Key by ref_id, not arg_key
    pub resolved_key: String,     // The UUID
}

pub async fn resolve_entity(req: ResolveEntityRequest, ast: &mut Program) -> Result<()> {
    // Find and update the specific EntityRef by ref_id
    commit_resolution_by_ref_id(ast, &req.ref_id, &req.resolved_key)
}
```

### Step 2: Add Commit Walker to AST

```rust
// In ast.rs
pub fn commit_resolution_by_ref_id(
    program: &mut Program,
    target_ref_id: &str,
    resolved_key: &str,
) -> bool {
    for stmt in &mut program.statements {
        if let Statement::VerbCall(vc) = stmt {
            for arg in &mut vc.arguments {
                if commit_to_node(&mut arg.value, target_ref_id, resolved_key) {
                    return true;
                }
            }
        }
    }
    false
}

fn commit_to_node(node: &mut AstNode, target_ref_id: &str, resolved_key: &str) -> bool {
    match node {
        AstNode::EntityRef { ref_id, resolved_key: rk, .. } => {
            if ref_id.as_deref() == Some(target_ref_id) {
                *rk = Some(resolved_key.to_string());
                return true;
            }
        }
        AstNode::List { items, .. } => {
            for item in items {
                if commit_to_node(item, target_ref_id, resolved_key) {
                    return true;
                }
            }
        }
        AstNode::Map { entries, .. } => {
            for (_, value) in entries {
                if commit_to_node(value, target_ref_id, resolved_key) {
                    return true;
                }
            }
        }
        _ => {}
    }
    false
}
```

### Why This Matters

Without ref_id-based commit:
- `(batch.create :clients ["Allianz" "BlackRock"])` → user resolves "Allianz" → ??? which one gets the UUID?
- Lists with multiple refs will be broken
- Maps with multiple ref values will be broken

With ref_id-based commit:
- Each EntityRef has unique `ref_id` like `"0:42-50"`
- Commit targets exact node
- Lists/maps work correctly

### Implementation Notes

**1. Ensure ref_id is ALWAYS populated during enrich:**

Check `enrichment.rs` - every `AstNode::EntityRef` creation must set `ref_id`:

```rust
// In enrichment.rs, verify this pattern:
AstNode::EntityRef {
    entity_type,
    search_column: config.search_key.primary_column().to_string(),
    value: s,
    resolved_key: None,
    span: arg_span,
    ref_id: Some(format!("{}:{}-{}", stmt_index, arg_span.start, arg_span.end)),  // ← MUST be set
    explain: None,
}
```

**2. Return clear error if ref_id not found:**

```rust
pub fn commit_resolution_by_ref_id(...) -> Result<(), CommitError> {
    if !found {
        return Err(CommitError::RefIdNotFound {
            ref_id: target_ref_id.to_string(),
            hint: "ref_id may have changed after AST modification - re-extract unresolved refs",
        });
    }
    Ok(())
}
```

**3. Verify ref_id uniqueness in lists:**

Each EntityRef in a list should have a unique span, thus unique ref_id. But verify in tests:

```rust
#[test]
fn test_ref_id_unique_in_list() {
    let dsl = r#"(batch.create :clients ["Allianz" "BlackRock" "Vanguard"])"#;
    let ast = parse_and_enrich(dsl);
    let unresolved = find_unresolved_ref_locations(&ast);
    
    // All ref_ids should be unique
    let ref_ids: HashSet<_> = unresolved.iter()
        .filter_map(|u| u.ref_id.as_ref())
        .collect();
    assert_eq!(ref_ids.len(), unresolved.len(), "ref_ids must be unique");
}
```

### Verification

```rust
#[test]
fn test_commit_resolution_in_list() {
    let dsl = r#"(batch.create :clients ["Allianz" "BlackRock"])"#;
    let mut ast = parse_and_enrich(dsl);
    
    // Find unresolved refs
    let unresolved = find_unresolved_ref_locations(&ast);
    assert_eq!(unresolved.len(), 2);
    
    // Resolve first one by ref_id
    let ref_id_0 = &unresolved[0].ref_id.clone().unwrap();
    commit_resolution_by_ref_id(&mut ast, ref_id_0, "uuid-allianz");
    
    // Verify only first one resolved
    let still_unresolved = find_unresolved_ref_locations(&ast);
    assert_eq!(still_unresolved.len(), 1);
    assert_eq!(still_unresolved[0].search_text, "BlackRock");
}
```

---

## Implementation Order

1. **Problem G** (20 min) - Compute embedding once, pass through - EASY WIN
2. **Problem H** (10 min) - Fix hardcoded 0.5 threshold - EASY WIN
3. **Problem A** (30 min) - Fix string classification using verb schema
4. **Problem B** (20 min) - Track missing required args + fail early
5. **Problem C** (30 min) - Use existing AST walker for unresolved refs
6. **Problem E** (30 min) - Array/map coercion
7. **Problem I** (30 min) - Demote/merge competing global semantic sources
8. **Problem J + D** (60 min) - Top-k verb search with ambiguity detection
9. **Problem K** (30 min) - ref_id-based commit for lists/maps
10. **Problem F** (30 min) - Audit trail (optional)

Total: ~5 hours

---

## Files Modified Summary

| File | Changes |
|------|---------|
| `intent_pipeline.rs` | Problems A, B, C, E, F |
| `verb_search.rs` | Problems D, G, H, I, J |
| `verb_service.rs` | Problem J (add top-k methods), Problem I (union query) |
| `ast.rs` | Problem C (extend walker), Problem K (commit by ref_id) |
| `types.rs` or new `intent_types.rs` | ArgumentValue enum updates |

---

## PR Review Rubric (12 Checks)

Use this checklist to verify the implementation doesn't regress:

| # | Check | Pass? |
|---|-------|-------|
| 1 | `:name` stays `String`, not `Unresolved` (unless explicitly lookup-configured) | ☐ |
| 2 | Missing required args return `NeedsUserInput` BEFORE DSL compile | ☐ |
| 3 | Unresolved refs extracted via canonical `find_unresolved_ref_locations` walker | ☐ |
| 4 | Walker handles lists with multiple EntityRefs | ☐ |
| 5 | Walker handles nested maps | ☐ |
| 6 | `ref_id` populated in `UnresolvedRefLocation` | ☐ |
| 7 | Commit resolution uses `ref_id`, not `(stmt_idx, arg_key)` | ☐ |
| 8 | Embedding computed exactly ONCE per search | ☐ |
| 9 | Global semantic uses `fallback_threshold` (0.65), not hardcoded 0.5 | ☐ |
| 10 | Ambiguity margin is 0.05, applies only when top >= threshold AND second is viable | ☐ |
| 11 | No parse/enrich called on `NeedsUserInput` / `NeedsClarification` early exits | ☐ |
| 12 | `ref_id` is unique across multiple unresolved refs in same statement (esp. lists) | ☐ |

**Common Regressions to Watch:**
- `:name` accidentally becoming lookup-driven (spurious entity lookups)
- `ref_id` not threaded to commit (breaks lists/maps)
- Manual AST traversal instead of using canonical walker
- Different thresholds for different semantic sources
- Early exits still calling parse/enrich (wasted work)

---

## Critical End-to-End Test

**This single test proves Problem K is truly fixed:**

```rust
#[tokio::test]
async fn test_partial_resolution_in_list() {
    // Setup: DSL with list containing multiple entity refs
    let dsl = r#"(batch.create :clients ["Allianz" "BlackRock"])"#;
    
    // Parse and enrich
    let mut ast = parse_and_enrich(dsl);
    
    // Find unresolved refs
    let unresolved = find_unresolved_ref_locations(&ast);
    assert_eq!(unresolved.len(), 2);
    assert_eq!(unresolved[0].search_text, "Allianz");
    assert_eq!(unresolved[1].search_text, "BlackRock");
    
    // Resolve ONLY the first one by ref_id
    let allianz_ref_id = unresolved[0].ref_id.clone().unwrap();
    let result = commit_resolution_by_ref_id(
        &mut ast, 
        &allianz_ref_id, 
        "uuid-allianz-123"
    );
    assert!(result.is_ok());
    
    // Verify: first resolved, second still unresolved
    let still_unresolved = find_unresolved_ref_locations(&ast);
    assert_eq!(still_unresolved.len(), 1);
    assert_eq!(still_unresolved[0].search_text, "BlackRock");
    
    // Verify: Allianz now has resolved_key
    // (inspect AST to confirm uuid-allianz-123 is in place)
}
```

**If this test passes, the ref_id commit plumbing is correct.**

---

## Quick Wins First

Start with these - they're 30 minutes total and have huge impact:

### Quick Win 1: Fix Repeated Embedding (Problem G)

```rust
// verb_search.rs - compute embedding ONCE at top of search()

pub async fn search(&self, query: &str, ...) -> Result<VerbSearchOutcome> {
    // Normalize ONCE
    let normalized = query.trim().to_lowercase();
    
    // Compute embedding ONCE (correct async pattern)
    let query_embedding = if let Some(embedder) = &self.embedder {
        Some(embedder.embed(query).await?)
    } else {
        None
    };
    
    // Pass reference everywhere
    if let Some(ref emb) = query_embedding {
        // use emb in all semantic calls
    }
}
```

**Note:** Don't use `.map(|e| e.embed()).transpose().await?` - that won't compile because `embed()` returns a Future, not a Result.

### Quick Win 2: Fix Hardcoded Threshold (Problem H)

```rust
// verb_search.rs line 390 - change this:
.search_verb_patterns_semantic(&query_embedding, limit, 0.5)  // BAD

// to this:
.search_verb_patterns_semantic(&query_embedding, limit, self.fallback_threshold)  // GOOD

// Add field to struct:
pub struct HybridVerbSearcher {
    // ...
    fallback_threshold: f32,  // 0.65 for cold start
}
```

---

## Command to Start

```bash
# Start with Problem G - it's the quickest win
cd ~/ob-poc/rust/src/mcp
# Open verb_search.rs and find the search() method
```
