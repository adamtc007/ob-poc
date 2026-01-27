# Disambiguation Feedback Loop - UI Implementation Plan

> **Status**: Backend complete, UI pending
> **Created**: 2026-01-27
> **Backend PR**: Disambiguation endpoints ready

## Summary

The backend for the verb disambiguation feedback loop is complete. This document outlines the remaining UI work to close the loop.

## What's Done (Backend)

### New Types (`ob-poc-types/src/lib.rs`)

```rust
pub struct VerbDisambiguationRequest {
    pub request_id: String,
    pub original_input: String,
    pub options: Vec<VerbOption>,
    pub prompt: String,
}

pub struct VerbOption {
    pub verb_fqn: String,
    pub description: String,
    pub example: String,
    pub score: f32,
    pub matched_phrase: Option<String>,
}
```

### New Endpoints

| Endpoint | Purpose |
|----------|---------|
| `POST /api/session/:id/select-verb` | User clicks a verb button |
| `POST /api/session/:id/abandon-disambiguation` | User abandons without selecting |

### Response Field

`AgentChatResponse` now includes:
```rust
pub verb_disambiguation: Option<VerbDisambiguationRequest>
```

When `verb_disambiguation` is `Some(...)`, the UI should render buttons instead of executing.

---

## UI Implementation Tasks

### 1. Detect Disambiguation Response

In the REPL panel, after receiving a chat response:

```rust
// In process_chat_response() or equivalent
if let Some(disambiguation) = &response.verb_disambiguation {
    // Don't show normal response
    // Instead, render disambiguation UI
    self.pending_disambiguation = Some(disambiguation.clone());
    return;
}
```

### 2. Render Clickable Buttons

When `pending_disambiguation` is set, render a disambiguation panel:

```rust
fn render_disambiguation_ui(&mut self, ui: &mut Ui, ctx: &AppContext) -> Option<DisambiguationAction> {
    let disambiguation = self.pending_disambiguation.as_ref()?;
    
    ui.vertical(|ui| {
        ui.label(&disambiguation.prompt); // "Which action did you mean?"
        ui.add_space(8.0);
        
        for option in &disambiguation.options {
            let button_text = format!(
                "{}\n{}", 
                option.verb_fqn, 
                option.description
            );
            
            if ui.button(&button_text).clicked() {
                return Some(DisambiguationAction::Select {
                    request_id: disambiguation.request_id.clone(),
                    selected_verb: option.verb_fqn.clone(),
                    all_candidates: disambiguation.options.iter()
                        .map(|o| o.verb_fqn.clone())
                        .collect(),
                });
            }
        }
        
        ui.add_space(8.0);
        if ui.button("Cancel").clicked() {
            return Some(DisambiguationAction::Abandon {
                request_id: disambiguation.request_id.clone(),
                reason: AbandonReason::Cancelled,
            });
        }
    });
    
    None
}
```

### 3. Handle Button Click → POST /select-verb

When user clicks a verb button:

```rust
async fn handle_verb_selection(
    &self,
    session_id: &str,
    original_input: &str,
    selection: DisambiguationAction,
) {
    match selection {
        DisambiguationAction::Select { request_id, selected_verb, all_candidates } => {
            let payload = VerbSelectionRequest {
                request_id,
                original_input: original_input.to_string(),
                selected_verb,
                all_candidates,
            };
            
            // POST to /api/session/{id}/select-verb
            let url = format!("{}/api/session/{}/select-verb", self.base_url, session_id);
            let response = self.client.post(&url)
                .json(&payload)
                .send()
                .await;
            
            // The response will include the DSL execution result
            // Process it as a normal chat response
        }
        DisambiguationAction::Abandon { request_id, reason } => {
            // Handle abandon (see task 4)
        }
    }
}
```

### 4. Handle Abandon → POST /abandon-disambiguation

Abandon should fire when:
- User clicks "Cancel" button
- User types new input (before selecting)
- User navigates away from REPL
- 30-second timeout expires

```rust
async fn handle_abandon(
    &self,
    session_id: &str,
    original_input: &str,
    request_id: &str,
    candidates: Vec<String>,
    reason: AbandonReason,
) {
    let payload = AbandonDisambiguationRequest {
        request_id: request_id.to_string(),
        original_input: original_input.to_string(),
        candidates,
        abandon_reason: Some(reason),
    };
    
    let url = format!("{}/api/session/{}/abandon-disambiguation", self.base_url, session_id);
    let _ = self.client.post(&url)
        .json(&payload)
        .send()
        .await;
    
    // Clear disambiguation state
    self.pending_disambiguation = None;
}
```

### 5. Timeout Handling

Add a timer when disambiguation is shown:

```rust
struct DisambiguationState {
    request: VerbDisambiguationRequest,
    original_input: String,
    shown_at: Instant,
}

const DISAMBIGUATION_TIMEOUT: Duration = Duration::from_secs(30);

fn check_disambiguation_timeout(&mut self) {
    if let Some(state) = &self.disambiguation_state {
        if state.shown_at.elapsed() > DISAMBIGUATION_TIMEOUT {
            // Auto-abandon with timeout reason
            self.trigger_abandon(AbandonReason::Timeout);
        }
    }
}
```

### 6. Input Interception

When user has pending disambiguation and starts typing:

```rust
fn handle_input_change(&mut self, new_input: &str) {
    if self.disambiguation_state.is_some() && !new_input.is_empty() {
        // User is typing new input - abandon current disambiguation
        self.trigger_abandon(AbandonReason::TypedNewInput);
    }
}
```

---

## State Machine

```
┌─────────────┐
│   NORMAL    │
└─────────────┘
      │
      │ Chat response with verb_disambiguation
      ▼
┌─────────────┐
│ DISAMBIG    │───────┐
│   SHOWN     │       │
└─────────────┘       │
      │               │
      │               │ Timeout (30s)
      │               │ New input typed
      │               │ Cancel clicked
      │               │ Navigate away
      │               ▼
      │         ┌─────────────┐
      │         │  ABANDON    │──► POST /abandon-disambiguation
      │         └─────────────┘
      │
      │ User clicks verb button
      ▼
┌─────────────┐
│   SELECT    │──► POST /select-verb ──► Process response ──► NORMAL
└─────────────┘
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `ob-poc-ui/src/panels/repl_panel.rs` | Add disambiguation UI rendering |
| `ob-poc-ui/src/state.rs` | Add `pending_disambiguation` field to panel state |
| `ob-poc-ui/src/api.rs` | Add `select_verb` and `abandon_disambiguation` API calls |
| `ob-poc-ui/src/app.rs` | Process disambiguation actions from panel |

---

## API Payloads

### POST /select-verb

Request:
```json
{
  "request_id": "uuid-...",
  "original_input": "list all cbus",
  "selected_verb": "cbu.list",
  "all_candidates": ["cbu.list", "cbu.search", "session.list"]
}
```

Response: Standard `AgentChatResponse` with execution results.

### POST /abandon-disambiguation

Request:
```json
{
  "request_id": "uuid-...",
  "original_input": "list all cbus",
  "candidates": ["cbu.list", "cbu.search", "session.list"],
  "abandon_reason": "timeout"
}
```

Response: `{ "acknowledged": true }`

---

## Testing Checklist

- [ ] Query "list all cbus" → shows disambiguation UI with buttons
- [ ] Click [cbu.list] → executes verb, response shown
- [ ] After selection, query "list all cbus" again → should match without disambiguation (learned)
- [ ] Click [Cancel] → clears UI, records negative signals
- [ ] Type new input while disambiguation shown → auto-abandons
- [ ] Wait 30s → auto-abandons with timeout
- [ ] Navigate to different panel → auto-abandons

---

## Notes

- The backend records learning signals with confidence=0.95 for explicit selections
- Generated phrase variants (plural normalization, verb swaps) are recorded at confidence=0.85
- Rejected alternatives are added to blocklist to improve future discrimination
- Abandonment records negative signals for ALL candidates (user found none acceptable)
