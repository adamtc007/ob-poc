# OB-POC Frontend Rewrite — React/TypeScript

**Document Status:** Implementation TODO  
**Author:** Adam (Lead Solution Architect) + Claude  
**Date:** 2026-02-04  
**Purpose:** Complete specification for rewriting OB-POC UI in React/TypeScript

---

## Executive Summary

The OB-POC frontend is being rewritten from egui (Rust) to React/TypeScript to leverage mature web ecosystem tooling, accelerate iteration, and provide a better foundation for both the **Inspector UI** (projection visualization) and **Agent Chat** (conversational DSL interface).

**Key Deliverables:**
1. **Inspector UI** — Tree/table/detail viewer for projection artifacts
2. **Agent Chat** — Conversational interface for DSL execution and entity queries
3. **Shared Shell** — Navigation, theming, state management, API layer

**Architecture Principle:** Rust generates data (projections, chat responses, DSL execution); React renders and handles interaction. Clean boundary at the API layer.

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Technology Stack](#2-technology-stack)
3. [Project Structure](#3-project-structure)
4. [API Contract](#4-api-contract)
5. [Shared Infrastructure](#5-shared-infrastructure)
6. [Inspector UI Components](#6-inspector-ui-components)
7. [Agent Chat Components](#7-agent-chat-components)
8. [State Management](#8-state-management)
9. [Implementation Phases](#9-implementation-phases)
10. [Testing Strategy](#10-testing-strategy)
11. [Deployment](#11-deployment)
12. [Migration Path](#12-migration-path)

---

## 1. Architecture Overview

### 1.1 System Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         OB-POC Rust Backend                             │
├─────────────────────────────────────────────────────────────────────────┤
│  Snapshot Model │ Projection Generator │ DSL Engine │ Agent Orchestrator│
└────────┬────────┴──────────┬───────────┴─────┬──────┴────────┬──────────┘
         │                   │                 │               │
         │ HTTP/WebSocket    │                 │               │
         ▼                   ▼                 ▼               ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         API Layer (Rust Axum)                           │
├─────────────────────────────────────────────────────────────────────────┤
│  GET /api/projections/:id          POST /api/chat/message               │
│  POST /api/projections/generate    GET /api/chat/sessions               │
│  GET /api/projections/validate     WS /api/chat/stream                  │
│  GET /api/entities/:id             POST /api/dsl/execute                │
│  GET /api/snapshots                GET /api/dsl/validate                │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │
                                     │ JSON / Server-Sent Events / WebSocket
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      React/TypeScript Frontend                          │
├──────────────────────────┬──────────────────────────────────────────────┤
│      Inspector UI        │              Agent Chat                      │
│  ┌─────────────────────┐ │ ┌──────────────────────────────────────────┐ │
│  │ Navigation Tree     │ │ │ Chat Message List                        │ │
│  │ Main View (table)   │ │ │ Input Bar + DSL Autocomplete             │ │
│  │ Detail Pane         │ │ │ Session Sidebar                          │ │
│  │ Search              │ │ │ Streaming Response Renderer              │ │
│  │ Breadcrumbs         │ │ │ Inline Projection Links                  │ │
│  └─────────────────────┘ │ └──────────────────────────────────────────┘ │
├──────────────────────────┴──────────────────────────────────────────────┤
│                         Shared Shell                                    │
│  App Layout │ Router │ Theme │ State (Zustand) │ API Client │ Auth      │
└─────────────────────────────────────────────────────────────────────────┘
                                     │
                                     │ Optional: Tauri shell for desktop
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                    Desktop App (Tauri) — Future                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| **Separate frontend repo** | Clean boundary; independent deployment; different dev workflows |
| **HTTP + WebSocket hybrid** | HTTP for CRUD; WebSocket for streaming chat responses |
| **Server-side projection generation** | Keep complex logic in Rust; frontend is pure renderer |
| **Zustand for state** | Minimal boilerplate; good TypeScript support; scales well |
| **Tanstack libraries** | Battle-tested table/query; huge ecosystem |
| **Shadcn/ui + Tailwind** | Copy-paste components; full control; no runtime dependency |

### 1.3 What Stays in Rust

- Snapshot model and persistence
- Projection generator and validator
- DSL parser and executor
- Agent orchestration (intent detection, tool dispatch)
- GLEIF integration
- All business logic

### 1.4 What Moves to React

- All UI rendering
- Navigation state (focus stack, breadcrumbs, expansion)
- Search/filter UI state
- Chat message display and input
- Theme and preferences
- Keyboard shortcuts

---

## 2. Technology Stack

### 2.1 Core

| Category | Choice | Version | Rationale |
|----------|--------|---------|-----------|
| Language | TypeScript | 5.x | Type safety, IDE support |
| Framework | React | 18.x | Ecosystem, concurrent features |
| Build | Vite | 5.x | Fast HMR, ESBuild |
| Router | React Router | 6.x | Standard, nested routes |
| Styling | Tailwind CSS | 3.x | Utility-first, no runtime |
| Components | Shadcn/ui | latest | Accessible, customizable |

### 2.2 Data & State

| Category | Choice | Rationale |
|----------|--------|-----------|
| State management | Zustand | Minimal API, good TS, no boilerplate |
| Server state | TanStack Query | Caching, background refetch, optimistic updates |
| Forms | React Hook Form + Zod | Validation, performance |

### 2.3 Specialized Components

| Category | Choice | Rationale |
|----------|--------|-----------|
| Tree view | react-arborist | Virtualized, keyboard nav, drag-drop ready |
| Table | @tanstack/react-table | Headless, virtualization, sorting/filtering |
| Virtual list | @tanstack/react-virtual | For large lists (chat history, sparse cells) |
| Search | cmdk | Keyboard-first command palette |
| Code editor | Monaco or CodeMirror 6 | DSL syntax highlighting |
| Markdown | react-markdown + remark-gfm | Chat message rendering |
| Icons | Lucide React | Consistent, tree-shakeable |

### 2.4 Chat-Specific

| Category | Choice | Rationale |
|----------|--------|-----------|
| Streaming | EventSource (SSE) or WebSocket | Real-time token streaming |
| Syntax highlighting | Shiki or Prism | DSL + code blocks in chat |
| Copy code | clipboard API + toast | UX for code snippets |

### 2.5 Development

| Category | Choice | Rationale |
|----------|--------|-----------|
| Testing | Vitest + Testing Library | Fast, Jest-compatible |
| E2E | Playwright | Cross-browser, good DX |
| Linting | ESLint + Prettier | Consistency |
| Type checking | tsc --noEmit in CI | Catch errors early |

### 2.6 Optional Future

| Category | Choice | When |
|----------|--------|------|
| Desktop wrapper | Tauri | If native app needed |
| Analytics | PostHog or Plausible | If usage tracking needed |
| Error tracking | Sentry | Production debugging |

---

## 3. Project Structure

```
ob-poc-ui/
├── package.json
├── tsconfig.json
├── vite.config.ts
├── tailwind.config.ts
├── index.html
├── public/
│   └── favicon.svg
├── src/
│   ├── main.tsx                    # Entry point
│   ├── App.tsx                     # Root component + router
│   ├── index.css                   # Tailwind imports + globals
│   │
│   ├── api/                        # API client layer
│   │   ├── client.ts               # Fetch wrapper, base URL, auth
│   │   ├── projections.ts          # Projection endpoints
│   │   ├── chat.ts                 # Chat/agent endpoints
│   │   ├── dsl.ts                  # DSL endpoints
│   │   ├── entities.ts             # Entity lookup endpoints
│   │   └── types.ts                # API response types (generated or manual)
│   │
│   ├── stores/                     # Zustand stores
│   │   ├── inspector.ts            # Inspector navigation state
│   │   ├── chat.ts                 # Chat sessions, messages
│   │   ├── preferences.ts          # Theme, LOD defaults, etc.
│   │   └── index.ts                # Re-exports
│   │
│   ├── hooks/                      # Custom hooks
│   │   ├── useProjection.ts        # TanStack Query wrapper
│   │   ├── useChatStream.ts        # WebSocket/SSE streaming
│   │   ├── useKeyboardNav.ts       # Keyboard shortcut handling
│   │   ├── useFocusStack.ts        # Inspector navigation
│   │   └── useSearch.ts            # Search state + debounce
│   │
│   ├── components/                 # Shared components
│   │   ├── ui/                     # Shadcn/ui primitives (button, dialog, etc.)
│   │   ├── layout/
│   │   │   ├── AppShell.tsx        # Main layout wrapper
│   │   │   ├── Sidebar.tsx         # Left nav
│   │   │   ├── Header.tsx          # Top bar
│   │   │   └── ResizablePanels.tsx # Three-pane layout
│   │   ├── common/
│   │   │   ├── Breadcrumbs.tsx
│   │   │   ├── SearchInput.tsx
│   │   │   ├── CommandPalette.tsx  # cmdk wrapper
│   │   │   ├── LoadingSpinner.tsx
│   │   │   ├── ErrorBoundary.tsx
│   │   │   └── EmptyState.tsx
│   │   └── icons/
│   │       └── GlyphIcon.tsx       # Render glyph by node kind
│   │
│   ├── features/                   # Feature modules
│   │   ├── inspector/
│   │   │   ├── InspectorPage.tsx   # Main inspector route
│   │   │   ├── NavigationTree.tsx  # Left panel tree
│   │   │   ├── MainView.tsx        # Center panel (tree/table/card)
│   │   │   ├── DetailPane.tsx      # Right panel
│   │   │   ├── TableRenderer.tsx   # Matrix slice tables
│   │   │   ├── TreeNode.tsx        # Single tree node
│   │   │   ├── RefLink.tsx         # Clickable $ref
│   │   │   ├── ProvenanceCard.tsx  # Provenance display
│   │   │   ├── NodeCard.tsx        # Generic node detail
│   │   │   ├── EntityCard.tsx      # Entity-specific detail
│   │   │   ├── EdgeCard.tsx        # HoldingEdge/ControlEdge detail
│   │   │   ├── MatrixView.tsx      # Matrix-specific view
│   │   │   ├── PolicyControls.tsx  # LOD, depth, filter UI
│   │   │   ├── PinnedNodes.tsx     # Pinned node sidebar
│   │   │   └── SearchResults.tsx   # Search result list
│   │   │
│   │   ├── chat/
│   │   │   ├── ChatPage.tsx        # Main chat route
│   │   │   ├── ChatSidebar.tsx     # Session list
│   │   │   ├── ChatMessageList.tsx # Virtualized message list
│   │   │   ├── ChatMessage.tsx     # Single message (user/assistant)
│   │   │   ├── ChatInput.tsx       # Input bar + submit
│   │   │   ├── DSLAutocomplete.tsx # DSL syntax suggestions
│   │   │   ├── StreamingMessage.tsx# In-progress assistant message
│   │   │   ├── CodeBlock.tsx       # Syntax-highlighted code
│   │   │   ├── InlineProjection.tsx# Embedded projection link/preview
│   │   │   ├── ToolCallCard.tsx    # Display of tool invocations
│   │   │   ├── EntityMention.tsx   # Clickable entity reference
│   │   │   └── SessionControls.tsx # New session, rename, delete
│   │   │
│   │   └── settings/
│   │       ├── SettingsPage.tsx
│   │       ├── ThemeSettings.tsx
│   │       ├── KeyboardShortcuts.tsx
│   │       └── APISettings.tsx     # Backend URL config
│   │
│   ├── lib/                        # Utilities
│   │   ├── cn.ts                   # Tailwind class merge
│   │   ├── formatters.ts           # Date, number formatting
│   │   ├── nodeId.ts               # NodeId parsing/validation
│   │   ├── refResolver.ts          # $ref resolution helpers
│   │   ├── searchIndex.ts          # Fuse.js index builder
│   │   └── keyboard.ts             # Shortcut definitions
│   │
│   ├── types/                      # TypeScript types
│   │   ├── projection.ts           # InspectorProjection, Node, etc.
│   │   ├── chat.ts                 # ChatSession, ChatMessage, etc.
│   │   ├── dsl.ts                  # DSL types
│   │   └── api.ts                  # API request/response types
│   │
│   └── routes/                     # Route definitions
│       └── index.tsx               # React Router config
│
├── tests/
│   ├── unit/                       # Vitest unit tests
│   ├── integration/                # Component integration tests
│   └── e2e/                        # Playwright E2E tests
│
└── fixtures/                       # Test fixtures (copied from spec)
    ├── inspector_projection_sample.yaml
    ├── inspector_projection_stress.yaml
    └── ...
```

---

## 4. API Contract

### 4.1 Projection Endpoints

```typescript
// GET /api/projections
// List available projections
interface ListProjectionsResponse {
  projections: ProjectionSummary[];
}

interface ProjectionSummary {
  id: string;
  source_hash: string;
  created_at: string;
  chambers: string[];
  node_count: number;
}

// GET /api/projections/:id
// Fetch full projection
interface GetProjectionResponse {
  projection: InspectorProjection;
  validation: ValidationResult;
}

// POST /api/projections/generate
// Generate new projection from snapshot
interface GenerateProjectionRequest {
  snapshot_id: string;
  render_policy: RenderPolicy;
}

interface GenerateProjectionResponse {
  projection_id: string;
  projection: InspectorProjection;
  validation: ValidationResult;
}

// POST /api/projections/validate
// Validate projection without storing
interface ValidateProjectionRequest {
  projection: InspectorProjection;
}

interface ValidateProjectionResponse {
  validation: ValidationResult;
}
```

### 4.2 Chat Endpoints

```typescript
// GET /api/chat/sessions
// List chat sessions
interface ListSessionsResponse {
  sessions: ChatSessionSummary[];
}

interface ChatSessionSummary {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  message_count: number;
  snapshot_context?: string;  // Associated snapshot ID
}

// GET /api/chat/sessions/:id
// Get full session with messages
interface GetSessionResponse {
  session: ChatSession;
}

interface ChatSession {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  snapshot_context?: string;
  messages: ChatMessage[];
}

interface ChatMessage {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  timestamp: string;
  metadata?: {
    tool_calls?: ToolCall[];
    projection_refs?: string[];  // NodeIds referenced
    entity_refs?: string[];      // Entity IDs mentioned
    dsl_blocks?: DSLBlock[];     // Executed DSL
  };
}

interface ToolCall {
  tool: string;
  input: Record<string, unknown>;
  output?: Record<string, unknown>;
  status: 'pending' | 'success' | 'error';
  error?: string;
}

interface DSLBlock {
  source: string;
  parsed: boolean;
  executed: boolean;
  result?: unknown;
  error?: string;
}

// POST /api/chat/sessions
// Create new session
interface CreateSessionRequest {
  title?: string;
  snapshot_context?: string;
}

interface CreateSessionResponse {
  session: ChatSession;
}

// POST /api/chat/sessions/:id/messages
// Send message (non-streaming response)
interface SendMessageRequest {
  content: string;
  include_context?: boolean;
}

interface SendMessageResponse {
  user_message: ChatMessage;
  assistant_message: ChatMessage;
}

// WebSocket: /api/chat/sessions/:id/stream
// Streaming message interface
interface StreamMessageRequest {
  type: 'message';
  content: string;
}

interface StreamChunk {
  type: 'token' | 'tool_start' | 'tool_end' | 'done' | 'error';
  // For 'token':
  token?: string;
  // For 'tool_start'/'tool_end':
  tool_call?: ToolCall;
  // For 'done':
  message?: ChatMessage;
  // For 'error':
  error?: string;
}
```

### 4.3 DSL Endpoints

```typescript
// POST /api/dsl/validate
// Validate DSL without executing
interface ValidateDSLRequest {
  source: string;
  snapshot_context?: string;
}

interface ValidateDSLResponse {
  valid: boolean;
  errors: DSLError[];
  warnings: DSLWarning[];
  parsed_ast?: unknown;  // For debugging
}

// POST /api/dsl/execute
// Execute DSL
interface ExecuteDSLRequest {
  source: string;
  snapshot_id: string;
  dry_run?: boolean;
}

interface ExecuteDSLResponse {
  success: boolean;
  results: DSLExecutionResult[];
  snapshot_modified: boolean;
  new_snapshot_id?: string;
}

interface DSLExecutionResult {
  instruction_id: string;
  verb: string;
  status: 'success' | 'error' | 'skipped';
  affected_entities?: string[];
  error?: string;
}

// GET /api/dsl/completions
// Autocomplete suggestions
interface GetCompletionsRequest {
  prefix: string;
  cursor_position: number;
  snapshot_context?: string;
}

interface GetCompletionsResponse {
  completions: Completion[];
}

interface Completion {
  label: string;
  kind: 'verb' | 'entity' | 'field' | 'value';
  detail?: string;
  insert_text: string;
}
```

### 4.4 Entity Endpoints

```typescript
// GET /api/entities/:id
// Get entity details
interface GetEntityResponse {
  entity: EntityDetail;
}

interface EntityDetail {
  entity_id: string;
  entity_kind: string;
  name: string;
  lei?: string;
  jurisdiction?: string;
  attributes: Record<string, unknown>;
  relationships: EntityRelationship[];
  projection_node_id?: string;  // Link to projection
}

interface EntityRelationship {
  type: string;
  target_entity_id: string;
  target_name: string;
  provenance?: Provenance;
}

// GET /api/entities/search
// Search entities
interface SearchEntitiesRequest {
  query: string;
  kinds?: string[];
  limit?: number;
}

interface SearchEntitiesResponse {
  results: EntitySearchResult[];
}

interface EntitySearchResult {
  entity_id: string;
  entity_kind: string;
  name: string;
  match_field: string;
  score: number;
}
```

---

## 5. Shared Infrastructure

### 5.1 API Client

```typescript
// src/api/client.ts
import { QueryClient } from '@tanstack/react-query';

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:3001';

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60,  // 1 minute
      retry: 1,
    },
  },
});

export async function apiFetch<T>(
  path: string,
  options?: RequestInit
): Promise<T> {
  const url = `${API_BASE}${path}`;
  const response = await fetch(url, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...options?.headers,
    },
  });
  
  if (!response.ok) {
    const error = await response.json().catch(() => ({}));
    throw new APIError(response.status, error.message || 'Request failed');
  }
  
  return response.json();
}

export class APIError extends Error {
  constructor(public status: number, message: string) {
    super(message);
    this.name = 'APIError';
  }
}
```

### 5.2 WebSocket Manager (Chat Streaming)

```typescript
// src/api/chatStream.ts
type StreamHandler = (chunk: StreamChunk) => void;

export class ChatStreamManager {
  private ws: WebSocket | null = null;
  private handlers: Map<string, StreamHandler> = new Map();
  
  connect(sessionId: string): Promise<void> {
    return new Promise((resolve, reject) => {
      const url = `${WS_BASE}/api/chat/sessions/${sessionId}/stream`;
      this.ws = new WebSocket(url);
      
      this.ws.onopen = () => resolve();
      this.ws.onerror = (e) => reject(e);
      this.ws.onmessage = (event) => {
        const chunk: StreamChunk = JSON.parse(event.data);
        this.handlers.forEach(handler => handler(chunk));
      };
    });
  }
  
  send(message: StreamMessageRequest): void {
    this.ws?.send(JSON.stringify(message));
  }
  
  subscribe(id: string, handler: StreamHandler): () => void {
    this.handlers.set(id, handler);
    return () => this.handlers.delete(id);
  }
  
  disconnect(): void {
    this.ws?.close();
    this.ws = null;
  }
}
```

### 5.3 TypeScript Types (from Spec)

```typescript
// src/types/projection.ts

export interface InspectorProjection {
  snapshot: SnapshotMeta;
  render_policy: RenderPolicy;
  ui_hints: UiHints;
  root: Record<string, RefValue>;
  nodes: Record<string, Node>;
}

export interface SnapshotMeta {
  schema_version: number;
  source_hash: string;
  policy_hash: string;
  created_at: string;
  chambers: string[];
}

export interface RenderPolicy {
  lod: 0 | 1 | 2 | 3;
  max_depth: number;
  max_items_per_list: number;
  show: {
    chambers: string[];
    branches: string[];
    node_kinds?: NodeKind[];
  };
  prune: {
    exclude_paths: string[];
    filters: Record<string, unknown>;
  };
}

export interface UiHints {
  shorthand_labels: boolean;
  link_style: 'ref' | 'inline';
  breadcrumb: boolean;
  history: boolean;
}

export interface RefValue {
  $ref: string;
}

export interface Node {
  kind: NodeKind;
  id: string;
  label_short: string;
  label_full?: string;
  glyph?: string;
  summary?: Record<string, unknown>;
  tags?: string[];
  attributes?: Record<string, unknown>;
  provenance?: Provenance;
  branches?: Record<string, RefValue | Record<string, RefValue>>;
  children?: RefValue[];
  links?: Record<string, RefValue | RefValue[]>;
  list?: ListStructure;
  edges?: Record<string, RefValue[]>;
  table?: TableStructure;
  // Entity-specific
  entity_id?: string;
  entity_kind?: string;
  // Edge-specific
  from?: RefValue;
  to?: RefValue;
  metrics?: Record<string, unknown>;
  control_type?: string;
  confidence?: number;
  ambiguity_flags?: string[];
  // Matrix-specific
  axes?: Record<string, string[]>;
  sparse_cells?: SparseCellsStructure;
  focus?: Record<string, RefValue>;
}

export type NodeKind =
  | 'CBU'
  | 'MemberList'
  | 'Entity'
  | 'ProductTree'
  | 'Product'
  | 'Service'
  | 'Resource'
  | 'ProductBinding'
  | 'InstrumentMatrix'
  | 'MatrixSlice'
  | 'SparseCellPage'
  | 'InvestorRegister'
  | 'HoldingEdgeList'
  | 'HoldingEdge'
  | 'ControlRegister'
  | 'ControlTree'
  | 'ControlNode'
  | 'ControlEdge';

export interface Provenance {
  sources: string[];
  asserted_at: string;
  confidence?: number;
  notes?: string;
  evidence_refs?: RefValue[];
}

export interface ListStructure {
  paging: {
    limit: number;
    next: string | null;
  };
  items: RefValue[];
}

export interface TableStructure {
  columns: string[];
  rows: (string | number | boolean)[][];
}

export interface SparseCellsStructure {
  paging?: {
    limit: number;
    next: string | null;
  };
  items: SparseCell[];
}

export interface SparseCell {
  key: string[];
  value: MatrixCellValue;
}

export interface MatrixCellValue {
  enabled: boolean;
  source: 'refdata' | 'dsl' | 'override' | 'default';
  default?: boolean;
  dsl_instruction_id?: string;
  dsl_run_id?: string;
  policy_gate?: string;
  notes_short?: string;
  provenance?: Provenance;
}

export interface ValidationResult {
  valid: boolean;
  errors: ValidationIssue[];
  warnings: ValidationIssue[];
}

export interface ValidationIssue {
  level: 'error' | 'warning' | 'info';
  code: string;
  message: string;
  context?: Record<string, unknown>;
}
```

---

## 6. Inspector UI Components

### 6.1 Component Hierarchy

```
InspectorPage
├── Header
│   ├── Breadcrumbs
│   ├── SearchInput
│   └── PolicyControls (LOD, Depth, Filters)
├── ResizablePanels
│   ├── LeftPanel
│   │   ├── NavigationTree (react-arborist)
│   │   │   └── TreeNode (recursive)
│   │   │       ├── GlyphIcon
│   │   │       └── RefLink (if branches)
│   │   └── PinnedNodes
│   ├── CenterPanel
│   │   └── MainView
│   │       ├── TreeView (default)
│   │       ├── TableView (for MatrixSlice)
│   │       └── CardView (for single entity)
│   └── RightPanel
│       └── DetailPane
│           ├── NodeCard (generic)
│           ├── EntityCard (Entity kind)
│           ├── EdgeCard (HoldingEdge/ControlEdge)
│           ├── ProvenanceCard (if provenance)
│           └── LinksSection (cross-references)
└── StatusBar
    └── Validation status, node count, etc.
```

### 6.2 Key Component Specs

#### NavigationTree

```typescript
interface NavigationTreeProps {
  projection: InspectorProjection;
  rootNodeId: string;
  selectedNodeId: string | null;
  expandedNodeIds: Set<string>;
  onSelect: (nodeId: string) => void;
  onExpand: (nodeId: string) => void;
  onCollapse: (nodeId: string) => void;
}

// Uses react-arborist for virtualization
// Renders TreeNode for each visible node
// Handles keyboard navigation (up/down/left/right/enter)
```

#### TreeNode

```typescript
interface TreeNodeProps {
  node: Node;
  depth: number;
  isSelected: boolean;
  isExpanded: boolean;
  lod: number;
  onSelect: () => void;
  onToggle: () => void;
  onNavigate: (refTarget: string) => void;
}

// Renders based on LOD:
// - LOD 0: glyph + id only
// - LOD 1: glyph + label_short
// - LOD 2: glyph + label_short + summary badges
// - LOD 3: full detail inline

// Shows expand/collapse chevron if has children
// Shows branch indicators for $ref links
```

#### TableRenderer (Matrix Slice)

```typescript
interface TableRendererProps {
  table: TableStructure;
  onCellClick?: (row: number, col: number) => void;
  selectedCell?: { row: number; col: number };
}

// Uses @tanstack/react-table
// Virtualized for large matrices
// Keyboard navigation (arrows, tab)
// Boolean cells render as ✓/✗ with color
```

#### RefLink

```typescript
interface RefLinkProps {
  refValue: RefValue;
  projection: InspectorProjection;
  onNavigate: (nodeId: string) => void;
}

// Renders as clickable link
// Shows target node's label_short on hover
// Click pushes to focus stack
// Right-click shows context menu (copy ID, open in new tab)
```

#### ProvenanceCard

```typescript
interface ProvenanceCardProps {
  provenance: Provenance;
  onSourceClick?: (source: string) => void;
}

// Displays:
// - Sources (list)
// - Asserted date (formatted)
// - Confidence (as percentage + color indicator)
// - Notes (if present)
// - Evidence refs (as clickable links)
```

### 6.3 Navigation State

```typescript
// src/stores/inspector.ts
import { create } from 'zustand';

interface InspectorState {
  // Current projection
  projectionId: string | null;
  projection: InspectorProjection | null;
  validation: ValidationResult | null;
  
  // Navigation
  focusStack: string[];  // NodeIds
  focusIndex: number;
  expandedNodes: Set<string>;
  selectedNodeId: string | null;
  
  // Display
  lod: 0 | 1 | 2 | 3;
  maxDepth: number;
  showOrphans: boolean;
  
  // Search
  searchQuery: string;
  searchResults: string[];
  
  // Pins
  pinnedNodes: string[];
  
  // Actions
  loadProjection: (id: string) => Promise<void>;
  navigateTo: (nodeId: string) => void;
  goBack: () => void;
  goForward: () => void;
  toggleExpand: (nodeId: string) => void;
  setLod: (lod: 0 | 1 | 2 | 3) => void;
  setMaxDepth: (depth: number) => void;
  search: (query: string) => void;
  pinNode: (nodeId: string) => void;
  unpinNode: (nodeId: string) => void;
}

export const useInspectorStore = create<InspectorState>((set, get) => ({
  // ... implementation
}));
```

---

## 7. Agent Chat Components

### 7.1 Component Hierarchy

```
ChatPage
├── ChatSidebar
│   ├── NewSessionButton
│   └── SessionList
│       └── SessionItem (for each session)
│           ├── Title
│           ├── Timestamp
│           └── ContextBadge (if has snapshot)
├── ChatMain
│   ├── ChatHeader
│   │   ├── SessionTitle (editable)
│   │   ├── SnapshotContextSelector
│   │   └── SessionActions (delete, export)
│   ├── ChatMessageList (virtualized)
│   │   └── ChatMessage (for each message)
│   │       ├── UserMessage
│   │       │   └── Markdown content
│   │       └── AssistantMessage
│   │           ├── Markdown content
│   │           ├── CodeBlock (for DSL/code)
│   │           ├── ToolCallCard (if tool_calls)
│   │           ├── InlineProjection (if projection_refs)
│   │           └── EntityMention (clickable)
│   └── ChatInput
│       ├── TextArea (auto-resize)
│       ├── DSLAutocomplete (if typing DSL)
│       ├── AttachButton (for files)
│       └── SendButton
└── StreamingIndicator (when response in progress)
```

### 7.2 Key Component Specs

#### ChatMessageList

```typescript
interface ChatMessageListProps {
  messages: ChatMessage[];
  streamingMessage?: Partial<ChatMessage>;
  onEntityClick: (entityId: string) => void;
  onProjectionRefClick: (nodeId: string) => void;
  onRetry?: (messageId: string) => void;
}

// Virtualized with @tanstack/react-virtual
// Auto-scrolls to bottom on new messages
// Shows streaming message at bottom during generation
// Handles mixed content (text, code, tools)
```

#### ChatMessage

```typescript
interface ChatMessageProps {
  message: ChatMessage;
  isStreaming?: boolean;
  onEntityClick: (entityId: string) => void;
  onProjectionRefClick: (nodeId: string) => void;
}

// User messages: simple bubble, right-aligned
// Assistant messages: left-aligned with avatar
// Content parsed as Markdown
// Code blocks get syntax highlighting + copy button
// Entity mentions rendered as chips (clickable)
// Tool calls rendered as expandable cards
```

#### StreamingMessage

```typescript
interface StreamingMessageProps {
  tokens: string[];
  pendingToolCalls: ToolCall[];
  onCancel?: () => void;
}

// Displays tokens as they arrive
// Shows typing indicator
// Shows tool calls as they start/complete
// Cancel button to abort generation
```

#### DSLAutocomplete

```typescript
interface DSLAutocompleteProps {
  value: string;
  cursorPosition: number;
  onSelect: (completion: Completion) => void;
  onClose: () => void;
}

// Fetches completions from /api/dsl/completions
// Keyboard navigation (up/down/enter/escape)
// Shows completion kind icons (verb, entity, field)
// Debounced API calls
```

#### ToolCallCard

```typescript
interface ToolCallCardProps {
  toolCall: ToolCall;
  onExpand?: () => void;
}

// Displays tool name + status badge
// Expandable to show input/output
// Error state with message
// Links to affected entities/projections
```

#### InlineProjection

```typescript
interface InlineProjectionProps {
  nodeId: string;
  projection?: InspectorProjection;
  onNavigate: () => void;
}

// Shows node label + glyph as chip
// Hover shows preview tooltip
// Click navigates to Inspector with node focused
// "Open in Inspector" action
```

### 7.3 Chat State

```typescript
// src/stores/chat.ts
import { create } from 'zustand';

interface ChatState {
  // Sessions
  sessions: ChatSessionSummary[];
  currentSessionId: string | null;
  currentSession: ChatSession | null;
  
  // Streaming
  isStreaming: boolean;
  streamTokens: string[];
  pendingToolCalls: ToolCall[];
  
  // Input
  inputValue: string;
  
  // Actions
  loadSessions: () => Promise<void>;
  createSession: (title?: string, snapshotContext?: string) => Promise<void>;
  selectSession: (sessionId: string) => Promise<void>;
  sendMessage: (content: string) => Promise<void>;
  cancelStream: () => void;
  deleteSession: (sessionId: string) => Promise<void>;
  renameSession: (sessionId: string, title: string) => Promise<void>;
  setInput: (value: string) => void;
}

export const useChatStore = create<ChatState>((set, get) => ({
  // ... implementation
}));
```

### 7.4 Chat ↔ Inspector Integration

```typescript
// When user clicks entity mention in chat:
const handleEntityClick = (entityId: string) => {
  // 1. Find entity's projection node
  const nodeId = `entity:uuid:${entityId}`;
  
  // 2. Navigate to Inspector
  navigate(`/inspector/${projectionId}?focus=${nodeId}`);
  
  // 3. Or open in side panel (if enabled)
  openInspectorPanel(projectionId, nodeId);
};

// When user clicks "Show in Chat" from Inspector:
const handleShowInChat = (nodeId: string) => {
  // 1. Get or create chat session with snapshot context
  const sessionId = getOrCreateSession(projectionId);
  
  // 2. Pre-fill input with entity reference
  const node = projection.nodes[nodeId];
  setInput(`Tell me about ${node.label_short}`);
  
  // 3. Navigate to Chat
  navigate(`/chat/${sessionId}`);
};
```

---

## 8. State Management

### 8.1 Store Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Zustand Stores                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │ inspectorStore  │  │   chatStore     │  │ prefsStore  │ │
│  ├─────────────────┤  ├─────────────────┤  ├─────────────┤ │
│  │ projection      │  │ sessions        │  │ theme       │ │
│  │ focusStack      │  │ currentSession  │  │ lod default │ │
│  │ expandedNodes   │  │ streamState     │  │ shortcuts   │ │
│  │ selectedNode    │  │ inputValue      │  │ sidebar     │ │
│  │ lod, maxDepth   │  │                 │  │             │ │
│  │ searchQuery     │  │                 │  │             │ │
│  │ pinnedNodes     │  │                 │  │             │ │
│  └─────────────────┘  └─────────────────┘  └─────────────┘ │
│                                                             │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ Selectors / Actions
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   React Components                          │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ TanStack Query (server state)
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      API Layer                              │
└─────────────────────────────────────────────────────────────┘
```

### 8.2 State Boundaries

| State Type | Location | Persistence |
|------------|----------|-------------|
| **Server state** (projections, sessions) | TanStack Query cache | Refetch on mount |
| **Navigation state** (focus, expanded) | Zustand store | Session only |
| **UI preferences** (theme, LOD defaults) | Zustand + localStorage | Persisted |
| **Ephemeral UI** (dropdowns, modals) | Component state | None |

### 8.3 Persistence

```typescript
// src/stores/preferences.ts
import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface PreferencesState {
  theme: 'light' | 'dark' | 'system';
  defaultLod: 0 | 1 | 2 | 3;
  defaultMaxDepth: number;
  sidebarWidth: number;
  showOrphans: boolean;
  // ... other preferences
}

export const usePreferencesStore = create<PreferencesState>()(
  persist(
    (set) => ({
      theme: 'system',
      defaultLod: 2,
      defaultMaxDepth: 3,
      sidebarWidth: 280,
      showOrphans: false,
    }),
    {
      name: 'obpoc-preferences',
    }
  )
);
```

---

## 9. Implementation Phases

### Phase 1: Foundation (Week 1)

**Goal:** Project setup, API client, basic shell.

**Deliverables:**
1. Vite + React + TypeScript project setup
2. Tailwind + Shadcn/ui configuration
3. API client with error handling
4. Basic routing (Inspector, Chat, Settings)
5. App shell layout (header, sidebar, main)
6. Type definitions from spec

**Acceptance Criteria:**
- [ ] `npm run dev` starts development server
- [ ] Routes navigate correctly
- [ ] API client can fetch projection list
- [ ] Basic theme toggle works

**Files to Create:**
```
src/main.tsx
src/App.tsx
src/api/client.ts
src/api/projections.ts
src/types/projection.ts
src/components/layout/AppShell.tsx
src/routes/index.tsx
```

---

### Phase 2: Inspector Core (Week 2)

**Goal:** Load and display projection, basic navigation.

**Deliverables:**
1. Projection loading with TanStack Query
2. Navigation tree with react-arborist
3. Detail pane (generic node card)
4. $ref link navigation
5. Focus stack (back/forward)
6. Breadcrumbs

**Acceptance Criteria:**
- [ ] Load sample fixture and display tree
- [ ] Click node to select and show details
- [ ] Click $ref to navigate
- [ ] Back button returns to previous node
- [ ] Breadcrumbs show path and allow jump

**Files to Create:**
```
src/features/inspector/InspectorPage.tsx
src/features/inspector/NavigationTree.tsx
src/features/inspector/DetailPane.tsx
src/features/inspector/NodeCard.tsx
src/features/inspector/RefLink.tsx
src/features/inspector/Breadcrumbs.tsx
src/stores/inspector.ts
src/hooks/useProjection.ts
src/hooks/useFocusStack.ts
```

---

### Phase 3: Inspector Polish (Week 3)

**Goal:** LOD controls, search, specialized cards.

**Deliverables:**
1. LOD selector with live update
2. Depth slider
3. Search with fuse.js
4. EntityCard, EdgeCard, ProvenanceCard
5. Pinned nodes
6. Keyboard shortcuts

**Acceptance Criteria:**
- [ ] Changing LOD updates displayed fields
- [ ] Search finds nodes by label/ID
- [ ] Entity cards show all entity fields
- [ ] Edge cards show from/to/provenance
- [ ] Pins persist in session
- [ ] Arrow keys navigate tree

**Files to Create:**
```
src/features/inspector/PolicyControls.tsx
src/features/inspector/SearchResults.tsx
src/features/inspector/EntityCard.tsx
src/features/inspector/EdgeCard.tsx
src/features/inspector/ProvenanceCard.tsx
src/features/inspector/PinnedNodes.tsx
src/hooks/useSearch.ts
src/hooks/useKeyboardNav.ts
src/lib/searchIndex.ts
```

---

### Phase 4: Matrix/Table Rendering (Week 4)

**Goal:** Table views for matrix slices.

**Deliverables:**
1. TableRenderer with @tanstack/react-table
2. MatrixView (slice selector + table)
3. Cell selection and navigation
4. Virtualization for large tables
5. Boolean cell rendering (✓/✗)

**Acceptance Criteria:**
- [ ] MatrixSlice renders as table
- [ ] Arrow keys navigate cells
- [ ] Large matrices virtualize smoothly
- [ ] Slice switcher works (by_mic, by_entity)

**Files to Create:**
```
src/features/inspector/TableRenderer.tsx
src/features/inspector/MatrixView.tsx
src/features/inspector/SliceSelector.tsx
```

---

### Phase 5: Chat Foundation (Week 5)

**Goal:** Basic chat UI with sessions.

**Deliverables:**
1. Chat sidebar with session list
2. ChatMessageList (virtualized)
3. ChatInput with submit
4. Session CRUD (create, rename, delete)
5. Message display (markdown)

**Acceptance Criteria:**
- [ ] Create new chat session
- [ ] Load existing session
- [ ] Send message, receive response
- [ ] Messages render as markdown
- [ ] Session list updates

**Files to Create:**
```
src/features/chat/ChatPage.tsx
src/features/chat/ChatSidebar.tsx
src/features/chat/ChatMessageList.tsx
src/features/chat/ChatMessage.tsx
src/features/chat/ChatInput.tsx
src/features/chat/SessionControls.tsx
src/stores/chat.ts
src/api/chat.ts
```

---

### Phase 6: Chat Streaming (Week 6)

**Goal:** Real-time streaming responses.

**Deliverables:**
1. WebSocket connection manager
2. StreamingMessage component
3. Token-by-token rendering
4. Cancel button
5. Tool call display

**Acceptance Criteria:**
- [ ] Messages stream in real-time
- [ ] Cancel stops generation
- [ ] Tool calls show as cards
- [ ] Reconnect on disconnect

**Files to Create:**
```
src/api/chatStream.ts
src/features/chat/StreamingMessage.tsx
src/features/chat/ToolCallCard.tsx
src/hooks/useChatStream.ts
```

---

### Phase 7: Chat Rich Content (Week 7)

**Goal:** DSL integration, entity mentions, projection links.

**Deliverables:**
1. DSLAutocomplete
2. CodeBlock with syntax highlighting
3. EntityMention (clickable chips)
4. InlineProjection (clickable + preview)
5. Chat ↔ Inspector navigation

**Acceptance Criteria:**
- [ ] DSL autocomplete suggests verbs/entities
- [ ] Code blocks highlight DSL syntax
- [ ] Entity mentions link to Inspector
- [ ] Projection refs show preview
- [ ] "Open in Inspector" works

**Files to Create:**
```
src/features/chat/DSLAutocomplete.tsx
src/features/chat/CodeBlock.tsx
src/features/chat/EntityMention.tsx
src/features/chat/InlineProjection.tsx
src/api/dsl.ts
```

---

### Phase 8: Settings & Polish (Week 8)

**Goal:** Settings page, theme, final polish.

**Deliverables:**
1. Settings page
2. Theme switcher (light/dark/system)
3. Keyboard shortcuts page
4. API URL configuration
5. Error boundaries
6. Loading states
7. Empty states

**Acceptance Criteria:**
- [ ] Theme persists across sessions
- [ ] Settings update immediately
- [ ] Errors show friendly messages
- [ ] Loading shows spinners
- [ ] Empty states guide user

**Files to Create:**
```
src/features/settings/SettingsPage.tsx
src/features/settings/ThemeSettings.tsx
src/features/settings/KeyboardShortcuts.tsx
src/features/settings/APISettings.tsx
src/components/common/ErrorBoundary.tsx
src/components/common/LoadingSpinner.tsx
src/components/common/EmptyState.tsx
```

---

### Phase 9: Testing & Documentation (Week 9)

**Goal:** Test coverage, documentation.

**Deliverables:**
1. Unit tests for stores and utilities
2. Component tests for key UI
3. E2E tests for critical flows
4. README with setup instructions
5. Component documentation (Storybook optional)

**Acceptance Criteria:**
- [ ] >70% unit test coverage
- [ ] E2E tests pass for: load projection, navigate, send chat
- [ ] README has clear setup steps
- [ ] API contract documented

---

### Phase 10: Desktop Wrapper (Optional, Week 10)

**Goal:** Tauri desktop app.

**Deliverables:**
1. Tauri project setup
2. Deep link handling (`obpoc://`)
3. File system access (load local projections)
4. Menu bar integration
5. Auto-update (optional)

**Acceptance Criteria:**
- [ ] Desktop app launches
- [ ] Deep links open correct view
- [ ] Can load local YAML files
- [ ] Native menus work

---

## 10. Testing Strategy

### 10.1 Unit Tests (Vitest)

**Target:** Stores, utilities, type guards

```typescript
// tests/unit/stores/inspector.test.ts
import { describe, it, expect } from 'vitest';
import { useInspectorStore } from '@/stores/inspector';

describe('InspectorStore', () => {
  it('navigateTo pushes to focus stack', () => {
    const store = useInspectorStore.getState();
    store.navigateTo('cbu:0');
    store.navigateTo('entity:uuid:fund_001');
    
    expect(store.focusStack).toEqual(['cbu:0', 'entity:uuid:fund_001']);
    expect(store.focusIndex).toBe(1);
  });
  
  it('goBack decrements focus index', () => {
    const store = useInspectorStore.getState();
    store.navigateTo('cbu:0');
    store.navigateTo('entity:uuid:fund_001');
    store.goBack();
    
    expect(store.focusIndex).toBe(0);
    expect(store.selectedNodeId).toBe('cbu:0');
  });
});
```

### 10.2 Component Tests (Testing Library)

**Target:** Key UI components

```typescript
// tests/integration/inspector/NavigationTree.test.tsx
import { render, screen, fireEvent } from '@testing-library/react';
import { NavigationTree } from '@/features/inspector/NavigationTree';
import sampleProjection from '@fixtures/inspector_projection_sample.yaml';

describe('NavigationTree', () => {
  it('renders root node', () => {
    render(<NavigationTree projection={sampleProjection} rootNodeId="cbu:0" />);
    expect(screen.getByText('CBU: Allianz AM — IE Platform')).toBeInTheDocument();
  });
  
  it('expands node on click', () => {
    render(<NavigationTree projection={sampleProjection} rootNodeId="cbu:0" />);
    fireEvent.click(screen.getByText('CBU: Allianz AM — IE Platform'));
    expect(screen.getByText('Members (3)')).toBeInTheDocument();
  });
});
```

### 10.3 E2E Tests (Playwright)

**Target:** Critical user flows

```typescript
// tests/e2e/inspector.spec.ts
import { test, expect } from '@playwright/test';

test.describe('Inspector', () => {
  test('loads projection and navigates', async ({ page }) => {
    await page.goto('/inspector/sample');
    
    // Wait for tree to load
    await expect(page.getByText('CBU: Allianz AM')).toBeVisible();
    
    // Expand and navigate
    await page.getByText('CBU: Allianz AM').click();
    await page.getByText('Members (3)').click();
    await page.getByText('Fund: Allianz IE ETF SICAV').click();
    
    // Verify detail pane
    await expect(page.getByText('entity:uuid:fund_001')).toBeVisible();
    await expect(page.getByText('ETF')).toBeVisible();  // tag
    
    // Verify breadcrumb
    await expect(page.getByRole('navigation')).toContainText('CBU');
    await expect(page.getByRole('navigation')).toContainText('Members');
    await expect(page.getByRole('navigation')).toContainText('Fund');
  });
  
  test('back button works', async ({ page }) => {
    await page.goto('/inspector/sample');
    await page.getByText('CBU: Allianz AM').click();
    await page.getByText('Members (3)').click();
    
    await page.getByRole('button', { name: 'Back' }).click();
    
    await expect(page.getByTestId('detail-pane')).toContainText('CBU');
  });
});
```

```typescript
// tests/e2e/chat.spec.ts
import { test, expect } from '@playwright/test';

test.describe('Chat', () => {
  test('sends message and receives response', async ({ page }) => {
    await page.goto('/chat');
    
    // Create new session
    await page.getByRole('button', { name: 'New Chat' }).click();
    
    // Send message
    await page.getByRole('textbox').fill('What entities are in the CBU?');
    await page.getByRole('button', { name: 'Send' }).click();
    
    // Wait for response
    await expect(page.getByText('assistant', { exact: false })).toBeVisible({ timeout: 30000 });
    
    // Verify response content
    await expect(page.getByRole('main')).toContainText('Allianz');
  });
});
```

---

## 11. Deployment

### 11.1 Development

```bash
# Start frontend dev server
cd ob-poc-ui
npm run dev

# Start backend API server (separate terminal)
cd ob-poc
cargo run -- serve --port 3001
```

### 11.2 Production Build

```bash
# Build frontend
npm run build

# Output in dist/
# Deploy to any static host (Netlify, Vercel, S3+CloudFront)
```

### 11.3 Environment Variables

```bash
# .env.development
VITE_API_URL=http://localhost:3001

# .env.production
VITE_API_URL=https://api.obpoc.example.com
```

### 11.4 Docker (Optional)

```dockerfile
# Dockerfile
FROM node:20-alpine AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM nginx:alpine
COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/nginx.conf
EXPOSE 80
```

---

## 12. Migration Path

### 12.1 Parallel Development

During migration, both UIs can coexist:

```
ob-poc/
├── rust/                    # Rust backend + egui (existing)
├── ob-poc-ui/              # New React frontend
└── docs/
    └── MIGRATION.md
```

### 12.2 Feature Parity Checklist

| Feature | egui Status | React Status | Notes |
|---------|-------------|--------------|-------|
| Load projection | ✅ | ⬜ | Phase 2 |
| Tree navigation | ✅ | ⬜ | Phase 2 |
| Detail pane | ✅ | ⬜ | Phase 2 |
| LOD controls | ✅ | ⬜ | Phase 3 |
| Search | ⬜ | ⬜ | Phase 3 |
| Matrix tables | ⬜ | ⬜ | Phase 4 |
| Chat sessions | ⬜ | ⬜ | Phase 5 |
| Chat streaming | ⬜ | ⬜ | Phase 6 |
| DSL autocomplete | ⬜ | ⬜ | Phase 7 |

### 12.3 Cutover Plan

1. **Week 1-4:** Develop React Inspector in parallel
2. **Week 5-7:** Develop React Chat in parallel
3. **Week 8:** Internal testing, bug fixes
4. **Week 9:** Switch default UI to React
5. **Week 10:** Archive egui code (keep for reference)

---

## Appendix A: Package.json

```json
{
  "name": "ob-poc-ui",
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "test": "vitest",
    "test:e2e": "playwright test",
    "lint": "eslint src --ext ts,tsx",
    "typecheck": "tsc --noEmit"
  },
  "dependencies": {
    "react": "^18.2.0",
    "react-dom": "^18.2.0",
    "react-router-dom": "^6.22.0",
    "@tanstack/react-query": "^5.17.0",
    "@tanstack/react-table": "^8.11.0",
    "@tanstack/react-virtual": "^3.0.0",
    "react-arborist": "^3.4.0",
    "zustand": "^4.5.0",
    "fuse.js": "^7.0.0",
    "cmdk": "^0.2.0",
    "react-markdown": "^9.0.0",
    "remark-gfm": "^4.0.0",
    "shiki": "^1.0.0",
    "lucide-react": "^0.312.0",
    "clsx": "^2.1.0",
    "tailwind-merge": "^2.2.0",
    "class-variance-authority": "^0.7.0",
    "yaml": "^2.3.0"
  },
  "devDependencies": {
    "@types/react": "^18.2.0",
    "@types/react-dom": "^18.2.0",
    "@vitejs/plugin-react": "^4.2.0",
    "typescript": "^5.3.0",
    "vite": "^5.0.0",
    "tailwindcss": "^3.4.0",
    "postcss": "^8.4.0",
    "autoprefixer": "^10.4.0",
    "vitest": "^1.2.0",
    "@testing-library/react": "^14.1.0",
    "@testing-library/jest-dom": "^6.2.0",
    "@playwright/test": "^1.41.0",
    "eslint": "^8.56.0",
    "@typescript-eslint/eslint-plugin": "^6.19.0",
    "@typescript-eslint/parser": "^6.19.0",
    "eslint-plugin-react-hooks": "^4.6.0"
  }
}
```

---

## Appendix B: Keyboard Shortcuts Reference

| Shortcut | Context | Action |
|----------|---------|--------|
| `↑` / `↓` | Tree | Navigate up/down |
| `←` / `→` | Tree | Collapse/expand |
| `Enter` | Tree | Select / follow $ref |
| `Backspace` | Inspector | Go back |
| `Alt+←` | Inspector | Go back |
| `Alt+→` | Inspector | Go forward |
| `/` | Global | Focus search |
| `Ctrl+K` | Global | Command palette |
| `Ctrl+P` | Inspector | Pin current node |
| `1`-`4` | Inspector | Set LOD 0-3 |
| `Escape` | Global | Close modal / clear |
| `Ctrl+Enter` | Chat | Send message |
| `Ctrl+.` | Chat | Cancel streaming |

---

## Appendix C: Color Tokens (Tailwind)

```typescript
// tailwind.config.ts
export default {
  theme: {
    extend: {
      colors: {
        // Semantic colors for node kinds
        'node-cbu': '#3B82F6',        // blue-500
        'node-entity': '#10B981',     // emerald-500
        'node-product': '#8B5CF6',    // violet-500
        'node-matrix': '#F59E0B',     // amber-500
        'node-register': '#EC4899',   // pink-500
        'node-edge': '#6B7280',       // gray-500
        
        // Confidence indicators
        'confidence-high': '#10B981',   // emerald-500
        'confidence-medium': '#F59E0B', // amber-500
        'confidence-low': '#EF4444',    // red-500
        
        // Source indicators
        'source-refdata': '#3B82F6',
        'source-dsl': '#8B5CF6',
        'source-override': '#F59E0B',
        'source-default': '#9CA3AF',
      }
    }
  }
};
```

---

*End of specification.*
