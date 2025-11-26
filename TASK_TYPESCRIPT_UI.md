# Task: Convert UI from JavaScript to TypeScript

## Objective

Replace the inline JavaScript in `static/index.html` with a proper TypeScript project using Vite. Type definitions mirror the Rust API types exactly for end-to-end type safety.

## Current State

- `rust/static/index.html` — 783 lines with inline CSS + JavaScript
- JavaScript is untyped, difficult to debug, no compile-time checking
- All API types implicit (VerbIntent, ChatResponse, etc.)

## Target Architecture

```
rust/
├── static/                    # DELETE after migration
│   └── index.html            
└── ui/                        # NEW TypeScript project
    ├── package.json
    ├── tsconfig.json
    ├── vite.config.ts
    ├── index.html
    └── src/
        ├── main.ts            # Entry point
        ├── api.ts             # API client with typed methods
        ├── types.ts           # Type definitions matching Rust
        ├── state.ts           # Simple state management
        ├── ui.ts              # DOM manipulation functions
        └── style.css          # Extracted CSS
```

## Type Definitions

### Create `rust/ui/src/types.ts`

These types MUST match the Rust API exactly:

```typescript
// ============================================================================
// Core Types (matching rust/src/api/intent.rs)
// ============================================================================

/** Parameter value types in intents */
export type ParamValue = 
  | string 
  | number 
  | boolean 
  | ParamValue[] 
  | Record<string, ParamValue>;

/** A single verb intent extracted from natural language */
export interface VerbIntent {
  /** The verb to execute, e.g., "cbu.ensure" */
  verb: string;
  /** Parameters with literal values */
  params: Record<string, ParamValue>;
  /** References to previous results, e.g., {"cbu-id": "@last_cbu"} */
  refs: Record<string, string>;
  /** Optional ordering hint */
  sequence?: number;
}

/** Sequence of intents from LLM extraction */
export interface IntentSequence {
  intents: VerbIntent[];
  reasoning?: string;
  confidence?: number;
}

/** Error from intent validation */
export interface IntentError {
  code: string;
  message: string;
  param?: string;
}

/** Result of validating an intent */
export interface IntentValidation {
  valid: boolean;
  intent: VerbIntent;
  errors: IntentError[];
  warnings: string[];
}

/** Assembled DSL from validated intents */
export interface AssembledDsl {
  statements: string[];
  combined: string;
  intent_count: number;
}

// ============================================================================
// Session Types (matching rust/src/api/session.rs)
// ============================================================================

/** Session lifecycle states */
export type SessionState = 
  | "new" 
  | "pending_validation" 
  | "ready_to_execute" 
  | "executing" 
  | "executed" 
  | "closed";

/** Message role */
export type MessageRole = "user" | "agent" | "system";

/** A message in the conversation */
export interface ChatMessage {
  id: string;
  role: MessageRole;
  content: string;
  timestamp: string;
  intents?: VerbIntent[];
  dsl?: string;
}

/** Context maintained across the session */
export interface SessionContext {
  last_cbu_id?: string;
  last_entity_id?: string;
  cbu_ids: string[];
  entity_ids: string[];
  domain_hint?: string;
  named_refs: Record<string, string>;
}

/** Result of executing a single DSL statement */
export interface ExecutionResult {
  statement_index: number;
  dsl: string;
  success: boolean;
  message: string;
  entity_id?: string;
  entity_type?: string;
}

// ============================================================================
// API Request/Response Types
// ============================================================================

export interface CreateSessionRequest {
  domain_hint?: string;
}

export interface CreateSessionResponse {
  session_id: string;
  created_at: string;
  state: SessionState;
}

export interface ChatRequest {
  message: string;
}

export interface ChatResponse {
  message: string;
  intents: VerbIntent[];
  validation_results: IntentValidation[];
  assembled_dsl?: AssembledDsl;
  session_state: SessionState;
  can_execute: boolean;
}

export interface SessionStateResponse {
  session_id: string;
  state: SessionState;
  message_count: number;
  pending_intents: VerbIntent[];
  assembled_dsl: string[];
  combined_dsl: string;
  context: SessionContext;
  messages: ChatMessage[];
  can_execute: boolean;
}

export interface ExecuteRequest {
  dry_run?: boolean;
}

export interface ExecuteResponse {
  success: boolean;
  results: ExecutionResult[];
  errors: string[];
  new_state: SessionState;
}

export interface ClearResponse {
  state: SessionState;
  message: string;
}

// ============================================================================
// UI State
// ============================================================================

export interface AppState {
  sessionId: string | null;
  sessionState: SessionState | null;
  messages: ChatMessage[];
  intents: VerbIntent[];
  validations: IntentValidation[];
  assembledDsl: string[];
  canExecute: boolean;
  loading: boolean;
  error: string | null;
}
```

## API Client

### Create `rust/ui/src/api.ts`

```typescript
import type {
  CreateSessionRequest,
  CreateSessionResponse,
  ChatRequest,
  ChatResponse,
  SessionStateResponse,
  ExecuteRequest,
  ExecuteResponse,
  ClearResponse,
} from './types';

const API_BASE = window.location.origin;

class ApiError extends Error {
  constructor(public status: number, message: string) {
    super(message);
    this.name = 'ApiError';
  }
}

async function request<T>(
  method: string,
  path: string,
  body?: unknown
): Promise<T> {
  const options: RequestInit = {
    method,
    headers: { 'Content-Type': 'application/json' },
  };

  if (body !== undefined) {
    options.body = JSON.stringify(body);
  }

  const response = await fetch(`${API_BASE}${path}`, options);

  if (!response.ok) {
    throw new ApiError(response.status, `HTTP ${response.status}`);
  }

  return response.json();
}

export const api = {
  /** Create a new agent session */
  createSession(req: CreateSessionRequest = {}): Promise<CreateSessionResponse> {
    return request('POST', '/api/session', req);
  },

  /** Get current session state */
  getSession(sessionId: string): Promise<SessionStateResponse> {
    return request('GET', `/api/session/${sessionId}`);
  },

  /** Delete a session */
  deleteSession(sessionId: string): Promise<void> {
    return request('DELETE', `/api/session/${sessionId}`);
  },

  /** Send a chat message and extract intents */
  chat(sessionId: string, req: ChatRequest): Promise<ChatResponse> {
    return request('POST', `/api/session/${sessionId}/chat`, req);
  },

  /** Execute accumulated DSL */
  execute(sessionId: string, req: ExecuteRequest = {}): Promise<ExecuteResponse> {
    return request('POST', `/api/session/${sessionId}/execute`, req);
  },

  /** Clear accumulated DSL */
  clear(sessionId: string): Promise<ClearResponse> {
    return request('POST', `/api/session/${sessionId}/clear`);
  },
};
```

## State Management

### Create `rust/ui/src/state.ts`

```typescript
import type { AppState, SessionState, VerbIntent, IntentValidation, ChatMessage } from './types';

type Listener = (state: AppState) => void;

const initialState: AppState = {
  sessionId: null,
  sessionState: null,
  messages: [],
  intents: [],
  validations: [],
  assembledDsl: [],
  canExecute: false,
  loading: false,
  error: null,
};

let state: AppState = { ...initialState };
const listeners: Set<Listener> = new Set();

export function getState(): Readonly<AppState> {
  return state;
}

export function setState(updates: Partial<AppState>): void {
  state = { ...state, ...updates };
  listeners.forEach(fn => fn(state));
}

export function resetState(): void {
  state = { ...initialState };
  listeners.forEach(fn => fn(state));
}

export function subscribe(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

// Convenience updaters
export function setLoading(loading: boolean): void {
  setState({ loading, error: null });
}

export function setError(error: string): void {
  setState({ error, loading: false });
}

export function setSession(sessionId: string, sessionState: SessionState): void {
  setState({
    sessionId,
    sessionState,
    messages: [],
    intents: [],
    validations: [],
    assembledDsl: [],
    canExecute: false,
    error: null,
  });
}

export function addMessage(role: 'user' | 'agent', content: string): void {
  const message: ChatMessage = {
    id: crypto.randomUUID(),
    role,
    content,
    timestamp: new Date().toISOString(),
  };
  setState({ messages: [...state.messages, message] });
}

export function updateFromChatResponse(
  intents: VerbIntent[],
  validations: IntentValidation[],
  assembledDsl: string[],
  sessionState: SessionState,
  canExecute: boolean
): void {
  setState({
    intents,
    validations,
    assembledDsl,
    sessionState,
    canExecute,
    loading: false,
  });
}

export function clearDsl(): void {
  setState({
    intents: [],
    validations: [],
    assembledDsl: [],
    canExecute: false,
  });
}
```

## UI Rendering

### Create `rust/ui/src/ui.ts`

```typescript
import type { AppState, VerbIntent, IntentValidation, ParamValue } from './types';

// ============================================================================
// DOM Helpers
// ============================================================================

function $(id: string): HTMLElement {
  const el = document.getElementById(id);
  if (!el) throw new Error(`Element not found: ${id}`);
  return el;
}

function escapeHtml(text: string): string {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

function formatParamValue(value: ParamValue): string {
  if (typeof value === 'object') {
    return JSON.stringify(value);
  }
  return String(value);
}

// ============================================================================
// Component Renderers
// ============================================================================

export function renderSessionInfo(state: AppState): void {
  const infoEl = $('session-info');
  const stateEl = $('session-state');

  if (state.sessionId) {
    infoEl.textContent = `Session: ${state.sessionId.substring(0, 8)}...`;
    infoEl.className = 'session-info active';
  } else {
    infoEl.textContent = 'No active session';
    infoEl.className = 'session-info';
  }

  if (state.sessionState) {
    stateEl.textContent = state.sessionState.replace('_', ' ');
    stateEl.className = `session-state ${state.sessionState}`;
    stateEl.style.display = 'inline-block';
  } else {
    stateEl.style.display = 'none';
  }
}

export function renderChat(state: AppState): void {
  const chat = $('chat');

  if (state.messages.length === 0) {
    chat.innerHTML = '<div class="empty-state">Create a session to start chatting</div>';
    return;
  }

  chat.innerHTML = state.messages
    .map(msg => `
      <div class="message ${msg.role}">
        <div class="message-role">${msg.role === 'user' ? 'You' : 'Agent'}</div>
        ${msg.content}
      </div>
    `)
    .join('');

  chat.scrollTop = chat.scrollHeight;
}

export function renderIntentsPanel(state: AppState): void {
  const panel = $('intents-panel');
  const countEl = $('intent-count');

  if (state.intents.length === 0) {
    panel.innerHTML = '<div class="empty-state">Intents will appear here</div>';
    countEl.style.display = 'none';
    return;
  }

  countEl.textContent = String(state.intents.length);
  countEl.style.display = 'inline-block';

  panel.innerHTML = state.intents
    .map((intent, i) => renderIntentCard(intent, state.validations[i]))
    .join('');
}

function renderIntentCard(intent: VerbIntent, validation?: IntentValidation): string {
  const isValid = validation?.valid ?? true;
  const validClass = isValid ? 'valid' : 'invalid';

  let html = `<div class="intent-card ${validClass}">`;
  html += `<div class="intent-verb">${escapeHtml(intent.verb)}</div>`;

  // Parameters
  const paramEntries = Object.entries(intent.params);
  if (paramEntries.length > 0) {
    html += '<div class="intent-params">';
    for (const [key, value] of paramEntries) {
      html += `
        <div class="intent-param">
          <span class="intent-param-key">:${escapeHtml(key)}</span>
          <span class="intent-param-value">${escapeHtml(formatParamValue(value))}</span>
        </div>
      `;
    }
    html += '</div>';
  }

  // References
  const refEntries = Object.entries(intent.refs);
  if (refEntries.length > 0) {
    html += '<div class="intent-refs">';
    for (const [key, ref] of refEntries) {
      html += `<span>:${escapeHtml(key)} = ${escapeHtml(ref)}</span> `;
    }
    html += '</div>';
  }

  // Validation errors
  if (validation && !validation.valid && validation.errors.length > 0) {
    html += '<div class="validation-errors">';
    html += validation.errors.map(e => escapeHtml(e.message)).join('<br>');
    html += '</div>';
  }

  html += '</div>';
  return html;
}

export function renderDslPreview(state: AppState): void {
  const preview = $('dsl-preview');
  const countEl = $('dsl-count');

  if (state.assembledDsl.length === 0) {
    preview.innerHTML = '<div class="empty-state" style="color: #666;">DSL will appear here</div>';
    countEl.style.display = 'none';
    return;
  }

  countEl.textContent = String(state.assembledDsl.length);
  countEl.style.display = 'inline-block';

  preview.innerHTML = state.assembledDsl
    .map((dsl, i) => `
      <div class="dsl-item">
        <span style="color: #888; font-size: 11px;"># Statement ${i + 1}</span><br>
        ${escapeHtml(dsl)}
      </div>
    `)
    .join('');
}

export function renderButtons(state: AppState): void {
  const messageInput = $('message') as HTMLInputElement;
  const sendBtn = $('send-btn') as HTMLButtonElement;
  const executeBtn = $('execute-btn') as HTMLButtonElement;
  const clearBtn = $('clear-btn') as HTMLButtonElement;

  const hasSession = state.sessionId !== null;
  const isLoading = state.loading;

  messageInput.disabled = !hasSession || isLoading;
  sendBtn.disabled = !hasSession || isLoading;
  executeBtn.disabled = !state.canExecute || isLoading;
  clearBtn.disabled = state.assembledDsl.length === 0 || isLoading;

  if (isLoading) {
    executeBtn.textContent = 'Executing...';
  } else {
    executeBtn.textContent = 'Execute All DSL';
  }
}

export function showLoading(): void {
  const chat = $('chat');
  const loadingDiv = document.createElement('div');
  loadingDiv.id = 'loading-message';
  loadingDiv.className = 'message agent loading';
  loadingDiv.innerHTML = '<div class="message-role">Agent</div>Extracting intents...';
  chat.appendChild(loadingDiv);
  chat.scrollTop = chat.scrollHeight;
}

export function hideLoading(): void {
  const loadingDiv = document.getElementById('loading-message');
  if (loadingDiv) loadingDiv.remove();
}

export function showError(message: string): void {
  const chat = $('chat');
  const errorDiv = document.createElement('div');
  errorDiv.className = 'message agent';
  errorDiv.innerHTML = `
    <div class="message-role">Agent</div>
    <div class="error-message">${escapeHtml(message)}</div>
  `;
  chat.appendChild(errorDiv);
  chat.scrollTop = chat.scrollHeight;
}

// ============================================================================
// Full Render
// ============================================================================

export function render(state: AppState): void {
  renderSessionInfo(state);
  renderChat(state);
  renderIntentsPanel(state);
  renderDslPreview(state);
  renderButtons(state);
}
```

## Main Entry Point

### Create `rust/ui/src/main.ts`

```typescript
import { api } from './api';
import {
  getState,
  subscribe,
  setSession,
  setLoading,
  setError,
  addMessage,
  updateFromChatResponse,
  clearDsl,
  setState,
  resetState,
} from './state';
import {
  render,
  showLoading,
  hideLoading,
  showError,
} from './ui';
import './style.css';

// ============================================================================
// Event Handlers
// ============================================================================

async function handleCreateSession(): Promise<void> {
  try {
    setLoading(true);
    const response = await api.createSession({});
    setSession(response.session_id, response.state);
    addMessage('agent', 'Session started. Describe what you want to create!');
  } catch (err) {
    setError(`Failed to create session: ${err instanceof Error ? err.message : 'Unknown error'}`);
    showError(`Failed to create session: ${err instanceof Error ? err.message : 'Unknown error'}`);
  } finally {
    setLoading(false);
  }
}

async function handleSendMessage(): Promise<void> {
  const state = getState();
  if (!state.sessionId) {
    alert('Create a session first');
    return;
  }

  const input = document.getElementById('message') as HTMLInputElement;
  const message = input.value.trim();
  if (!message) return;

  input.value = '';
  addMessage('user', message);
  showLoading();
  setLoading(true);

  try {
    const response = await api.chat(state.sessionId, { message });
    hideLoading();

    // Build agent response content
    let content = response.message;
    if (response.intents.length > 0) {
      const validCount = response.validation_results.filter(v => v.valid).length;
      const totalCount = response.intents.length;
      const badgeClass = validCount === totalCount ? 'valid' : 'invalid';
      content += ` <span class="badge ${badgeClass}">${validCount}/${totalCount} valid</span>`;
    }
    addMessage('agent', content);

    updateFromChatResponse(
      response.intents,
      response.validation_results,
      response.assembled_dsl?.statements ?? [],
      response.session_state,
      response.can_execute
    );
  } catch (err) {
    hideLoading();
    const errorMsg = err instanceof Error ? err.message : 'Unknown error';
    setError(errorMsg);
    showError(errorMsg);
  }
}

async function handleExecuteDsl(): Promise<void> {
  const state = getState();
  if (!state.sessionId || !state.canExecute) return;

  setLoading(true);

  try {
    const response = await api.execute(state.sessionId, { dry_run: false });

    if (response.success) {
      addMessage('agent', `<div class="success-message">Successfully executed ${response.results.length} DSL statement(s)</div>`);
      clearDsl();
    } else {
      const errors = response.errors.join('<br>');
      addMessage('agent', `<div class="error-message">Execution failed:<br>${errors}</div>`);
    }

    setState({ sessionState: response.new_state });
    await refreshSessionState();
  } catch (err) {
    const errorMsg = err instanceof Error ? err.message : 'Unknown error';
    showError(`Execution error: ${errorMsg}`);
  } finally {
    setLoading(false);
  }
}

async function handleClearDsl(): Promise<void> {
  const state = getState();
  if (!state.sessionId) return;

  try {
    const response = await api.clear(state.sessionId);
    clearDsl();
    setState({ sessionState: response.state });
    addMessage('agent', 'DSL cleared.');
  } catch (err) {
    const errorMsg = err instanceof Error ? err.message : 'Unknown error';
    showError(`Error: ${errorMsg}`);
  }
}

async function refreshSessionState(): Promise<void> {
  const state = getState();
  if (!state.sessionId) return;

  try {
    const response = await api.getSession(state.sessionId);
    setState({
      assembledDsl: response.assembled_dsl,
      sessionState: response.state,
      canExecute: response.can_execute,
    });
  } catch (err) {
    console.error('Failed to refresh session state:', err);
  }
}

// ============================================================================
// Initialization
// ============================================================================

function bindEvents(): void {
  // New Session button
  const newSessionBtn = document.querySelector('.new-session-btn');
  if (newSessionBtn) {
    newSessionBtn.addEventListener('click', handleCreateSession);
  }

  // Send button
  const sendBtn = document.getElementById('send-btn');
  if (sendBtn) {
    sendBtn.addEventListener('click', handleSendMessage);
  }

  // Message input (Enter key)
  const messageInput = document.getElementById('message');
  if (messageInput) {
    messageInput.addEventListener('keypress', (e) => {
      if (e.key === 'Enter') handleSendMessage();
    });
  }

  // Execute button
  const executeBtn = document.getElementById('execute-btn');
  if (executeBtn) {
    executeBtn.addEventListener('click', handleExecuteDsl);
  }

  // Clear button
  const clearBtn = document.getElementById('clear-btn');
  if (clearBtn) {
    clearBtn.addEventListener('click', handleClearDsl);
  }
}

function init(): void {
  // Subscribe to state changes
  subscribe(render);

  // Bind event handlers
  bindEvents();

  // Initial render
  render(getState());

  // Focus message input
  const messageInput = document.getElementById('message');
  if (messageInput) messageInput.focus();
}

// Start the app
document.addEventListener('DOMContentLoaded', init);
```

## Project Configuration

### Create `rust/ui/package.json`

```json
{
  "name": "ob-poc-ui",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "typecheck": "tsc --noEmit"
  },
  "devDependencies": {
    "typescript": "^5.3.0",
    "vite": "^5.0.0"
  }
}
```

### Create `rust/ui/tsconfig.json`

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "module": "ESNext",
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "forceConsistentCasingInFileNames": true
  },
  "include": ["src"]
}
```

### Create `rust/ui/vite.config.ts`

```typescript
import { defineConfig } from 'vite';

export default defineConfig({
  root: '.',
  build: {
    outDir: '../static',
    emptyOutDir: true,
  },
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:3000',
        changeOrigin: true,
      },
    },
  },
});
```

### Create `rust/ui/index.html`

```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>OB-POC Agent Session</title>
  </head>
  <body>
    <div class="header">
      <div>
        <h1>OB-POC Agent Session</h1>
        <div>
          <span id="session-info" class="session-info">No active session</span>
          <span id="session-state" class="session-state" style="display: none"></span>
        </div>
      </div>
      <button class="new-session-btn">New Session</button>
    </div>

    <div class="container">
      <div class="panel">
        <div class="panel-header">Chat</div>
        <div class="chat" id="chat">
          <div class="empty-state">Create a session to start chatting</div>
        </div>
        <div class="input-area">
          <input
            type="text"
            id="message"
            placeholder="Describe what you want to create..."
            disabled
          />
          <button id="send-btn" disabled>Send</button>
        </div>
      </div>

      <div class="panel">
        <div class="panel-header">
          Extracted Intents
          <span class="badge count" id="intent-count" style="display: none">0</span>
        </div>
        <div class="intents-panel" id="intents-panel">
          <div class="empty-state">Intents will appear here</div>
        </div>
      </div>

      <div class="panel">
        <div class="panel-header">
          Assembled DSL
          <span class="badge count" id="dsl-count" style="display: none">0</span>
        </div>
        <div class="dsl-preview" id="dsl-preview">
          <div class="empty-state" style="color: #666">DSL will appear here</div>
        </div>
        <div class="execute-area">
          <button class="execute-btn" id="execute-btn" disabled>Execute All DSL</button>
          <button class="clear-btn" id="clear-btn" disabled>Clear</button>
        </div>
      </div>
    </div>

    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

### Create `rust/ui/src/style.css`

Extract all CSS from the current `static/index.html` `<style>` block into this file.

```css
* {
  box-sizing: border-box;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  max-width: 1600px;
  margin: 0 auto;
  padding: 20px;
  background: #f5f5f5;
}

/* ... rest of CSS from static/index.html ... */
```

## Files to Create

| File | Purpose |
|------|---------|
| `rust/ui/package.json` | NPM project config |
| `rust/ui/tsconfig.json` | TypeScript config |
| `rust/ui/vite.config.ts` | Vite build config |
| `rust/ui/index.html` | HTML template |
| `rust/ui/src/types.ts` | Type definitions matching Rust |
| `rust/ui/src/api.ts` | Typed API client |
| `rust/ui/src/state.ts` | Simple state management |
| `rust/ui/src/ui.ts` | DOM rendering functions |
| `rust/ui/src/main.ts` | Entry point + event handlers |
| `rust/ui/src/style.css` | Extracted CSS |

## Files to Modify

| File | Changes |
|------|---------|
| `rust/static/` | Can be deleted after build, OR keep as Vite build output |

## Server Integration

Update `rust/src/bin/agentic_server.rs` to serve from the correct directory:

```rust
// Before (if needed):
.nest_service("/", ServeDir::new("static"))

// The vite.config.ts outputs to ../static, so no change needed
// OR point to ui/dist if you prefer:
.nest_service("/", ServeDir::new("ui/dist"))
```

## Development Workflow

```bash
# Terminal 1: Start Rust server
cd rust
DATABASE_URL=postgresql://adamtc007@localhost:5432/ob-poc \
ANTHROPIC_API_KEY=your-key \
cargo run --bin agentic_server --features server

# Terminal 2: Start Vite dev server (with proxy)
cd rust/ui
npm install
npm run dev
# Open http://localhost:5173

# For production build:
npm run build
# Outputs to rust/static/
```

## Testing

1. Run `npm run typecheck` — should have zero errors
2. Run `npm run dev` — opens browser with hot reload
3. Click "New Session" — verify API call works
4. Type a message — verify chat flow works
5. Verify intents panel shows extracted intents
6. Verify DSL preview shows assembled DSL
7. Click Execute — verify execution works
8. Run `npm run build` — verify production build

## Success Criteria

- [ ] All types in `types.ts` match Rust API exactly
- [ ] Zero TypeScript errors (`npm run typecheck`)
- [ ] API client has full type safety
- [ ] State management is typed
- [ ] UI renders correctly
- [ ] All functionality from JS version works
- [ ] Dev server proxies to Rust backend
- [ ] Production build outputs to `static/`

## Type Safety Benefits

With TypeScript, you now get:

1. **Compile-time errors** for typos like `intent.verbs` instead of `intent.verb`
2. **Autocomplete** in editor for all API response fields
3. **Refactoring safety** — rename a type and all usages update
4. **Documentation** — types serve as inline documentation
5. **No runtime "undefined is not a function"** surprises
