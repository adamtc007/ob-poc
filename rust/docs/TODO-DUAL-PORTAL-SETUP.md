# TODO: Dual Portal Setup - Internal + Client

## Goal

Two portals, same backend, testable side-by-side.

```
┌─────────────────────────────────────────────────────────────────┐
│                        SAME BACKEND                              │
│                                                                  │
│   AgentService ─── EntityGateway ─── PostgreSQL                 │
│        │                                                         │
│        ├──────────────────────┬──────────────────────┐          │
│        ▼                      ▼                      ▼          │
│   ┌─────────────┐       ┌─────────────┐       ┌──────────┐     │
│   │ Internal UI │       │ Client UI   │       │ Shared   │     │
│   │ :3000       │       │ :3001       │       │ API      │     │
│   │             │       │             │       │ :8080    │     │
│   └─────────────┘       └─────────────┘       └──────────┘     │
│                                                                  │
│   Analyst: "Request SOW from Pierre"                            │
│                         │                                        │
│                         ▼                                        │
│   Client: "What do you need?" → sees SOW request with WHY       │
└─────────────────────────────────────────────────────────────────┘
```

---

## Architecture

### Option A: Separate React Apps (Recommended for Testing)

```
ob-poc/
├── web/                    # Existing internal portal
│   ├── src/
│   │   ├── App.tsx
│   │   └── components/
│   ├── package.json
│   └── vite.config.ts      # Port 3000
│
├── web-client/             # NEW: Client portal
│   ├── src/
│   │   ├── App.tsx
│   │   └── components/
│   ├── package.json
│   └── vite.config.ts      # Port 3001
│
└── rust/
    └── src/
        └── api/
            ├── agent_service.rs      # Shared, scope-aware
            ├── internal_routes.rs    # /api/internal/*
            └── client_routes.rs      # /api/client/*
```

### Option B: Single App with Route Split

```
ob-poc/
└── web/
    ├── src/
    │   ├── App.tsx
    │   ├── internal/           # /internal/* routes
    │   │   ├── Dashboard.tsx
    │   │   ├── AgentChat.tsx
    │   │   └── CbuGraph.tsx
    │   └── client/             # /client/* routes
    │       ├── ClientPortal.tsx
    │       ├── ClientChat.tsx
    │       └── StatusView.tsx
    └── vite.config.ts
```

**Recommendation**: Option A for clean separation during testing. Can merge later.

---

## API Routes

### Internal API (Existing + Minor Changes)

```
POST /api/internal/chat
  - Full verb palette
  - All CBUs visible
  - Analyst session context
  
GET  /api/internal/cbus
GET  /api/internal/cbu/:id
POST /api/internal/dsl/execute
...existing routes...
```

### Client API (New)

```
# Authentication (client credentials)
POST /api/client/auth/login
  - body: { client_id, credential }
  - returns: { token, accessible_cbus }

# Chat (scoped)
POST /api/client/chat
  - header: Authorization: Bearer <token>
  - body: { message, cbu_id?, disambiguation_response? }
  - Scoped to client's CBUs
  - Client verb palette only

# Status (read-only)
GET  /api/client/status
  - Returns onboarding progress for accessible CBUs

GET  /api/client/outstanding
  - Returns outstanding requests with WHY

GET  /api/client/outstanding/:request_id
  - Returns full detail for one request

# Submissions
POST /api/client/submit-document
  - body: { request_id, document_type, file }
  - Multipart upload

POST /api/client/provide-info
  - body: { request_id, info_type, data }

POST /api/client/add-note
  - body: { request_id, note, expected_date? }

# Escalation
POST /api/client/escalate
  - body: { reason?, preferred_contact? }
  - Creates escalation with full context
```

---

## Backend Changes

### 1. Add Client Scope to AgentService

```rust
// src/api/agent_service.rs

pub struct ClientScope {
    pub client_id: Uuid,
    pub accessible_cbus: Vec<Uuid>,
}

pub struct AgentService {
    pool: Option<PgPool>,
    config: AgentServiceConfig,
    client_scope: Option<ClientScope>,  // NEW
}

impl AgentService {
    /// Create client-scoped agent service
    pub fn for_client(
        pool: PgPool, 
        client_id: Uuid, 
        accessible_cbus: Vec<Uuid>
    ) -> Self {
        Self {
            pool: Some(pool),
            config: AgentServiceConfig::default(),
            client_scope: Some(ClientScope { client_id, accessible_cbus }),
        }
    }
    
    /// Check if internal or client mode
    pub fn is_client_mode(&self) -> bool {
        self.client_scope.is_some()
    }
}
```

### 2. Add Client Routes

```rust
// src/api/client_routes.rs

use axum::{
    routing::{get, post},
    Router, Extension, Json,
    extract::State,
};

pub fn client_routes() -> Router<AppState> {
    Router::new()
        .route("/chat", post(client_chat))
        .route("/status", get(client_status))
        .route("/outstanding", get(client_outstanding))
        .route("/outstanding/:id", get(client_outstanding_detail))
        .route("/submit-document", post(client_submit_document))
        .route("/provide-info", post(client_provide_info))
        .route("/add-note", post(client_add_note))
        .route("/escalate", post(client_escalate))
        .layer(middleware::from_fn(verify_client_token))
}

async fn client_chat(
    State(state): State<AppState>,
    Extension(client): Extension<AuthenticatedClient>,
    Json(request): Json<AgentChatRequest>,
) -> Result<Json<AgentChatResponse>, ApiError> {
    // Create client-scoped agent service
    let service = AgentService::for_client(
        state.pool.clone(),
        client.client_id,
        client.accessible_cbus.clone(),
    );
    
    // Get or create session
    let mut session = state.sessions
        .get_or_create_client_session(client.client_id)
        .await?;
    
    // Process (same pipeline, different scope)
    let response = service
        .process_chat(&mut session, &request, state.llm_client.clone())
        .await?;
    
    Ok(Json(response))
}

async fn client_outstanding(
    State(state): State<AppState>,
    Extension(client): Extension<AuthenticatedClient>,
) -> Result<Json<Vec<ClientOutstandingRequest>>, ApiError> {
    let requests = sqlx::query_as!(
        ClientOutstandingRequest,
        r#"
        SELECT 
            r.request_id,
            r.request_type,
            r.request_subtype,
            e.name as entity_name,
            r.reason_for_request,
            r.compliance_context,
            r.acceptable_document_types,
            r.status,
            r.due_date,
            r.client_notes
        FROM kyc.outstanding_requests r
        JOIN kyc.entity_workstreams w ON r.workstream_id = w.workstream_id
        JOIN kyc.cases c ON w.case_id = c.case_id
        WHERE c.cbu_id = ANY($1)
          AND r.client_visible = true
          AND r.status != 'FULFILLED'
        ORDER BY r.due_date NULLS LAST
        "#,
        &client.accessible_cbus
    )
    .fetch_all(&state.pool)
    .await?;
    
    Ok(Json(requests))
}
```

### 3. Add Client Verb Registry

```yaml
# config/verbs/client.yaml

domain: client
description: "Client-facing operations"

verbs:
  - verb: get-status
    description: "Get onboarding status"
    args: []
    
  - verb: get-outstanding
    description: "Get outstanding requests with WHY"
    args: []
    
  - verb: submit-document
    description: "Submit document for a request"
    args:
      - name: request-id
        type: ref
        required: true
      - name: document-type
        type: code
        required: true
      - name: file-reference
        type: string
        required: true
        
  - verb: provide-info
    description: "Provide information for a request"
    args:
      - name: request-id
        type: ref
        required: true
      - name: info-type
        type: code
        required: true
      - name: data
        type: object
        required: true
        
  - verb: add-note
    description: "Add note to a request"
    args:
      - name: request-id
        type: ref
        required: true
      - name: note
        type: string
        required: true
      - name: expected-date
        type: date
        required: false
        
  - verb: escalate
    description: "Request human assistance"
    args:
      - name: reason
        type: string
        required: false
```

### 4. Add Client System Prompt

```markdown
# config/prompts/client_system.md

You are a client-facing onboarding assistant.

## Your Role
- Help clients understand what's needed and WHY
- Accept documents and information
- Be clear, patient, and professional
- Explain regulations in plain English

## Current Client Context
{client_context}

## Outstanding Items
{outstanding_with_why}

## Available Actions
You can help the client:
- See their onboarding status
- Understand why each document is needed
- Submit documents
- Provide information through guided collection
- Add notes about expected delivery
- Connect with their relationship manager

## You Cannot
- See other clients' data
- Waive regulatory requirements
- Execute internal operations
- Access CBUs they don't own
```

---

## Client Portal UI

### Minimal Viable UI (for testing)

```tsx
// web-client/src/App.tsx

import { useState } from 'react';
import { ChatPanel } from './components/ChatPanel';
import { StatusSidebar } from './components/StatusSidebar';

export function App() {
    const [messages, setMessages] = useState<Message[]>([]);
    const [outstanding, setOutstanding] = useState<OutstandingRequest[]>([]);
    
    return (
        <div className="flex h-screen">
            {/* Left: Status overview */}
            <StatusSidebar 
                outstanding={outstanding}
                onRefresh={() => fetchOutstanding()}
            />
            
            {/* Right: Chat */}
            <ChatPanel
                messages={messages}
                onSend={handleSend}
                onUpload={handleUpload}
            />
        </div>
    );
}
```

### Key Components

```tsx
// StatusSidebar.tsx
// Shows outstanding items with WHY, progress bar

// ChatPanel.tsx  
// Chat interface with file drop zone

// OutstandingCard.tsx
// Single request with WHY, accepts, status

// GuidedCollection.tsx
// Step-by-step Q&A for structured data

// UploadZone.tsx
// Drag-drop document upload
```

---

## Test Setup

### 1. Create Test Client Credentials

```sql
-- Test client for Allianz
INSERT INTO client_portal.clients (client_id, name, email, accessible_cbus)
VALUES (
    'a1b2c3d4-...',
    'Allianz Reinsurance AG',
    'onboarding@allianz.com',
    ARRAY['<allianz-cbu-uuid>']
);

INSERT INTO client_portal.credentials (client_id, credential_hash)
VALUES ('a1b2c3d4-...', crypt('test-password', gen_salt('bf')));
```

### 2. Run Both Portals

```bash
# Terminal 1: Backend
cd rust && cargo run --bin ob-poc-web

# Terminal 2: Internal Portal
cd web && npm run dev
# → http://localhost:3000

# Terminal 3: Client Portal
cd web-client && npm run dev
# → http://localhost:3001
```

### 3. Test Scenario

```
INTERNAL (localhost:3000):
1. Load CBU "Allianz Reinsurance"
2. Chat: "Request source of wealth documentation for Pierre Dupont"
3. → Creates outstanding request with WHY

CLIENT (localhost:3001):
1. Login as Allianz
2. Chat: "What do you need from us?"
3. → See SOW request with WHY explanation
4. Upload document
5. → Request status updates

INTERNAL:
6. See submission appear
7. Approve/request more
```

---

## File Structure (Final)

```
ob-poc/
├── rust/
│   └── src/
│       ├── api/
│       │   ├── agent_service.rs    # +ClientScope
│       │   ├── internal_routes.rs  # Existing
│       │   ├── client_routes.rs    # NEW
│       │   └── client_auth.rs      # NEW
│       └── bin/
│           └── ob-poc-web.rs       # Mount both route sets
│
├── config/
│   ├── verbs/
│   │   ├── cbu.yaml
│   │   ├── entity.yaml
│   │   ├── kyc-case.yaml
│   │   └── client.yaml             # NEW
│   └── prompts/
│       ├── internal_system.md      # Renamed from existing
│       └── client_system.md        # NEW
│
├── web/                            # Internal portal (existing)
│   └── ...
│
└── web-client/                     # Client portal (NEW)
    ├── src/
    │   ├── App.tsx
    │   ├── api/
    │   │   └── client.ts
    │   └── components/
    │       ├── ChatPanel.tsx
    │       ├── StatusSidebar.tsx
    │       ├── OutstandingCard.tsx
    │       └── UploadZone.tsx
    ├── package.json
    ├── tsconfig.json
    └── vite.config.ts
```

---

## Implementation Order

| Step | Task | Effort |
|------|------|--------|
| 1 | Add `ClientScope` to AgentService | 2 hrs |
| 2 | Create `client.yaml` verb registry | 1 hr |
| 3 | Create `client_system.md` prompt | 1 hr |
| 4 | Add `client_routes.rs` | 4 hrs |
| 5 | Scaffold `web-client/` React app | 2 hrs |
| 6 | Basic ChatPanel + StatusSidebar | 4 hrs |
| 7 | Document upload integration | 2 hrs |
| 8 | Test client credentials | 1 hr |
| **Total** | | **~2 days** |

---

## Quick Start Commands

```bash
# 1. Create client portal scaffold
cd ob-poc
npm create vite@latest web-client -- --template react-ts
cd web-client
npm install axios tailwindcss

# 2. Add client routes to backend
# (edit rust/src/bin/ob-poc-web.rs)

# 3. Run the test
./scripts/run-dual-portals.sh
```

---

## Success Criteria

- [ ] Internal: Create outstanding request with WHY
- [ ] Client: See request with WHY explanation
- [ ] Client: Chat naturally about what's needed
- [ ] Client: Upload document, tagged to request
- [ ] Internal: See submission in workstream
- [ ] Client: Partial progress persists across sessions
- [ ] Client: Can escalate to human with context
