# CLAUDE TODO: Journey UI Enhancements

## Status: Follow-up from Agent Context Implementation

The core semantic stage functionality is complete and working:
- ✅ Config: `config/ontology/semantic_stage_map.yaml`
- ✅ Types: `ob-poc-types/src/semantic_stage.rs`
- ✅ State derivation: `database/semantic_state_service.rs`
- ✅ Agent prompt injection: `agent_service.rs::derive_semantic_context()`
- ✅ API endpoint: `GET /api/session/:id/context` returns `SemanticState`
- ✅ UI display: `panels/context.rs` shows stage list with icons

## Missing Pieces

### 1. Stage Click → Focus (Priority: HIGH)

**Current:** Stage rows display but are not clickable.  
**Needed:** Click stage → set focus → filter agent verbs.

**File:** `rust/crates/ob-poc-ui/src/panels/context.rs`

```rust
// CURRENT (line 318)
fn render_stage_row(ui: &mut Ui, stage: &StageWithStatus) -> Option<ContextPanelAction> {
    let action = None;  // ← Never set
    // ...
    action
}

// NEEDED
fn render_stage_row(ui: &mut Ui, stage: &StageWithStatus) -> Option<ContextPanelAction> {
    let mut action = None;

    ui.horizontal(|ui| {
        // ... existing icon and label code ...
        
        // Make entire row clickable
        if ui.response().clicked() {
            action = Some(ContextPanelAction::SwitchScope {
                scope_type: "stage".to_string(),
                scope_id: stage.code.clone(),
            });
        }
    });

    action
}
```

**Also needed:**
- [ ] Add `POST /api/session/:id/focus` endpoint to set stage focus
- [ ] Store focus in session context
- [ ] Pass focus to agent prompt builder to filter verbs
- [ ] Highlight focused stage in UI

### 2. Focus API Endpoint (Priority: HIGH)

**File:** `rust/src/api/agent_routes.rs`

```rust
/// POST /api/session/:id/focus - Set stage focus
#[derive(Debug, Deserialize)]
pub struct SetFocusRequest {
    pub stage_code: String,
}

#[derive(Debug, Serialize)]
pub struct SetFocusResponse {
    pub success: bool,
    pub stage_code: String,
    pub relevant_verbs: Vec<String>,
}

async fn set_session_focus(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<SetFocusRequest>,
) -> Result<Json<SetFocusResponse>, StatusCode> {
    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;
    
    // Set focus in session context
    session.context.stage_focus = Some(req.stage_code.clone());
    
    // Get relevant verbs for this stage from registry
    let registry = SemanticStageRegistry::load_default()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let stage = registry.get_stage(&req.stage_code)
        .ok_or(StatusCode::NOT_FOUND)?;
    
    // TODO: Get verbs from stage config
    let relevant_verbs = vec![]; // Placeholder
    
    Ok(Json(SetFocusResponse {
        success: true,
        stage_code: req.stage_code,
        relevant_verbs,
    }))
}
```

Add to router:
```rust
.route("/api/session/:id/focus", post(set_session_focus))
```

### 3. Verb Filtering by Stage (Priority: MEDIUM)

**Current:** Agent sees all ~340 verbs.  
**Needed:** Agent sees only verbs relevant to focused stage.

**Config enhancement:** `config/ontology/semantic_stage_map.yaml`

```yaml
stages:
  - code: KYC_REVIEW
    name: "KYC Review"
    # ... existing fields ...
    relevant_verbs:  # ← ADD THIS
      - kyc-case.create
      - kyc-case.read
      - entity-workstream.create
      - entity-workstream.complete
      - doc-request.create
      - screening.run
```

**File:** `rust/src/api/agent_service.rs`

In `build_vocab_prompt()` or intent extraction, filter to relevant verbs:
```rust
fn get_verbs_for_stage(&self, stage_code: Option<&str>) -> Vec<&VerbInfo> {
    let registry = SemanticStageRegistry::load_default().ok()?;
    
    match stage_code {
        Some(code) => {
            let stage = registry.get_stage(code)?;
            // Filter verb registry to stage.relevant_verbs
            self.verb_registry.filter_to(&stage.relevant_verbs)
        }
        None => {
            // Return all verbs if no focus
            self.verb_registry.all()
        }
    }
}
```

### 4. DAG Visualization (Priority: LOW - Enhancement)

**Current:** List view with status icons.  
**Desired:** Graph visualization showing dependencies.

```
Current:                          Desired:
┌─────────────────┐              ┌─────────────────────────────────┐
│ ✓ Client Setup  │              │      ┌─────────┐                │
│ ✓ Product Sel   │              │      │ CLIENT  │ ✓              │
│ ◐ KYC Review !  │              │      │  SETUP  │                │
│ ○ Instrument    │              │      └────┬────┘                │
│ ⊘ Lifecycle     │              │           │                     │
└─────────────────┘              │    ┌──────┴──────┐              │
                                 │    ▼             ▼              │
                                 │ ┌─────┐     ┌─────────┐         │
                                 │ │ KYC │ ◐   │ PRODUCT │ ✓       │
                                 │ └──┬──┘     └────┬────┘         │
                                 │    │             │              │
                                 │    └──────┬──────┘              │
                                 │           ▼                     │
                                 │    ┌──────────────┐             │
                                 │    │  INSTRUMENT  │ ○           │
                                 │    └──────────────┘             │
                                 └─────────────────────────────────┘
```

**Options:**
1. Use existing `ob-poc-graph` crate (egui graph rendering)
2. Simple egui canvas drawing with nodes/edges
3. Keep list view, add indentation for dependencies

**Recommendation:** Start with enhanced list view (show dependency depth via indentation), upgrade to graph later if needed.

---

## Implementation Order

1. **Stage click handler** - Make rows clickable, return action
2. **Focus API endpoint** - Store focus in session
3. **Agent prompt filtering** - Pass focus to verb filter
4. **UI focus highlight** - Show which stage is focused
5. (Later) DAG visualization

## Files to Modify

| File | Change |
|------|--------|
| `panels/context.rs` | Add click handler to stage rows |
| `api/agent_routes.rs` | Add `/api/session/:id/focus` endpoint |
| `api/session.rs` | Add `stage_focus: Option<String>` to context |
| `api/agent_service.rs` | Filter verbs based on focus |
| `semantic_stage_map.yaml` | Add `relevant_verbs` to each stage |

## Success Criteria

```
User clicks "KYC Review" stage in UI
  → API call: POST /api/session/:id/focus {stage_code: "KYC_REVIEW"}
  → Session stores focus
  → Next agent prompt includes only KYC-related verbs
  → Agent generates DSL using kyc-case.*, entity-workstream.* verbs
  → UI highlights KYC Review stage as focused
```

## Test Scenario

1. Select CBU with products
2. See journey progress (current: works ✓)
3. Click "KYC Review" stage (TODO: implement)
4. Chat: "Start the KYC process"
5. Agent should generate `kyc-case.create` (not random verbs)
6. Stage should show as focused in UI
