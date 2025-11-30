# Plan: Unified Agent UI

## Goal
Single URL (http://localhost:3000) with:
1. Agent chat interface (prompt → response)
2. DSL generation from natural language
3. DSL preview/editing
4. DSL execution with results

## Current State
- Backend serves static UI at `/` from `rust/static/`
- Existing UI is template-based only (no chat)
- `/api/agent/generate` endpoint listed but NOT implemented
- Session APIs work: create, get, execute, chat (but chat doesn't generate DSL)
- Validate API works

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Unified Agent UI                              │
│                  http://localhost:3000                          │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │   Chat Panel    │  │   DSL Editor    │  │  Results Panel  │  │
│  │                 │  │                 │  │                 │  │
│  │ User: Create... │  │ (entity.create  │  │ entity_id:      │  │
│  │ Agent: I'll...  │  │   :first-name   │  │   abc-123...    │  │
│  │                 │  │   "John" ...)   │  │                 │  │
│  │ [Send]          │  │                 │  │ [Execution Log] │  │
│  └─────────────────┘  │ [Validate] [Run]│  │                 │  │
│                       └─────────────────┘  └─────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

## Implementation Steps

### Step 1: Implement `/api/agent/generate` endpoint
- Accept: `{ "instruction": "Create a person..." }`
- Use LLM (if configured) or rule-based generation
- Return: `{ "dsl": "(entity.create-proper-person ...)", "explanation": "..." }`

### Step 2: Create new unified HTML/JS UI
- Replace `rust/static/index.html` with new design
- Three-panel layout: Chat | DSL Editor | Results
- Pure HTML/CSS/JS (no build step needed)

### Step 3: Wire up API calls
- Chat sends to `/api/agent/generate` → shows DSL
- Validate button calls `/api/agent/validate`
- Execute button calls `/api/session/:id/execute`
- Results panel shows execution output

### Step 4: Session management
- Auto-create session on page load
- Show session ID in header
- Chat history persisted in session

## API Flow

```
User types: "Create a person John Smith born 1980"
     │
     ▼
POST /api/agent/generate
     { "instruction": "Create a person John Smith born 1980" }
     │
     ▼
Response:
     {
       "dsl": "(entity.create-proper-person :first-name \"John\" ...)",
       "explanation": "Creating a person entity with the provided details"
     }
     │
     ▼
DSL appears in editor panel
     │
     ▼
User clicks "Execute"
     │
     ▼
POST /api/session/:id/execute
     { "dsl": "(entity.create-proper-person ...)" }
     │
     ▼
Response:
     {
       "success": true,
       "results": [{ "entity_id": "uuid-here" }]
     }
```

## Files to Create/Modify

1. `rust/src/api/agent_routes.rs` - Add generate endpoint
2. `rust/static/index.html` - New unified UI
3. `rust/static/assets/app.js` - UI logic
4. `rust/static/assets/style.css` - Styling

## Decision: LLM vs Rule-based Generation

For the `/api/agent/generate` endpoint:
- **Option A**: Use configured LLM (Claude/OpenAI) - best quality
- **Option B**: Rule-based pattern matching - works offline

Recommend: Try LLM first (ANTHROPIC_API_KEY already checked at startup), fallback to templates/rules if not configured.
