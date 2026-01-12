# Phase 1 Fix: Wire Disambiguation to ChatResponse

## Problem

In `rust/src/api/agent_routes.rs` line 2041, the `disambiguation_request` is hardcoded to `None`:

```rust
Ok(Json(ChatResponse {
    message: response.message,
    dsl: dsl_state,
    session_state: to_session_state_enum(&response.session_state),
    commands: response.commands,
    disambiguation_request: None,  // ← BUG: Ignores response.disambiguation
}))
```

The agent_service correctly returns `disambiguation: Some(DisambiguationRequest)` but it's never mapped.

## Additional Issue: Type Mismatch

- **Server type:** `crate::api::session::DisambiguationRequest` uses `Uuid`
- **Client type:** `ob_poc_types::DisambiguationRequest` uses `String`

Need conversion function.

---

## Fix 1: Add Conversion Function

Add to `rust/src/api/agent_routes.rs` (near the other conversion functions like `to_session_state_enum`):

```rust
/// Convert server-side DisambiguationRequest to ob_poc_types version
fn convert_disambiguation_request(
    server: &crate::api::session::DisambiguationRequest,
) -> ob_poc_types::DisambiguationRequest {
    ob_poc_types::DisambiguationRequest {
        request_id: server.request_id.to_string(),
        items: server
            .items
            .iter()
            .map(|item| convert_disambiguation_item(item))
            .collect(),
        prompt: server.prompt.clone(),
    }
}

fn convert_disambiguation_item(
    server: &crate::api::session::DisambiguationItem,
) -> ob_poc_types::DisambiguationItem {
    match server {
        crate::api::session::DisambiguationItem::EntityMatch {
            param,
            search_text,
            matches,
        } => ob_poc_types::DisambiguationItem::EntityMatch {
            param: param.clone(),
            search_text: search_text.clone(),
            matches: matches.iter().map(convert_entity_match).collect(),
        },
        crate::api::session::DisambiguationItem::InterpretationChoice { text, options } => {
            ob_poc_types::DisambiguationItem::InterpretationChoice {
                text: text.clone(),
                options: options.iter().map(convert_interpretation).collect(),
            }
        }
    }
}

fn convert_entity_match(
    server: &crate::api::session::EntityMatchOption,
) -> ob_poc_types::EntityMatch {
    ob_poc_types::EntityMatch {
        entity_id: server.entity_id.to_string(),
        name: server.name.clone(),
        entity_type: server.entity_type.clone(),
        jurisdiction: server.jurisdiction.clone(),
        context: server.context.clone(),
        score: server.score.map(|s| s as f64),
    }
}

fn convert_interpretation(
    server: &crate::api::session::Interpretation,
) -> ob_poc_types::Interpretation {
    ob_poc_types::Interpretation {
        id: server.id.clone(),
        label: server.label.clone(),
        description: server.description.clone(),
        effect: server.effect.clone(),
    }
}
```

---

## Fix 2: Wire Disambiguation in Response

Change line ~2041 in `chat_session()`:

```rust
// Return response using API types (single source of truth)
Ok(Json(ChatResponse {
    message: response.message,
    dsl: dsl_state,
    session_state: to_session_state_enum(&response.session_state),
    commands: response.commands,
    disambiguation_request: response.disambiguation.as_ref().map(convert_disambiguation_request),
}))
```

---

## Fix 3: Ensure ob_poc_types Has Correct Types

Verify `ob-poc-types/src/lib.rs` has:

```rust
/// Disambiguation request - sent when user input is ambiguous
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisambiguationRequest {
    pub request_id: String,
    pub items: Vec<DisambiguationItem>,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DisambiguationItem {
    EntityMatch {
        param: String,
        search_text: String,
        matches: Vec<EntityMatch>,
    },
    InterpretationChoice {
        text: String,
        options: Vec<Interpretation>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMatch {
    pub entity_id: String,
    pub name: String,
    pub entity_type: String,
    #[serde(default)]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interpretation {
    pub id: String,
    pub label: String,
    pub description: String,
    #[serde(default)]
    pub effect: Option<String>,
}
```

---

## Verification

After applying fixes:

1. `cargo build` - should compile
2. Run server + UI
3. Type: `select allianz lux`
4. **Expected:** Disambiguation modal appears with CBU matches
5. Select one → graph loads

---

## Files to Modify

| File | Change |
|------|--------|
| `rust/src/api/agent_routes.rs` | Add conversion functions, wire `disambiguation_request` |
| `ob-poc-types/src/lib.rs` | Verify types exist (should already) |

---

## Summary

One-line root cause: `disambiguation_request: None` hardcoded when it should be `response.disambiguation.as_ref().map(convert_disambiguation_request)`.
