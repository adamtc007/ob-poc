# TASK: DSL Viewer UI - Wire Frontend to API

## Goal

Repurpose the `phase6-web-client` to consume the new DSL Viewer API endpoints and display:
1. List of persisted DSL instances (by business_reference)
2. DSL source code with syntax highlighting
3. Execution plan showing dependency ordering
4. Version history navigation

---

## Part 1: API Endpoints (Already Implemented)

The server provides these endpoints at `http://localhost:8080`:

```
GET /api/dsl/list                    → { instances: [...], total: N }
GET /api/dsl/show/:business_ref      → { dsl_source, execution_plan, ... }
GET /api/dsl/show/:business_ref/:ver → { dsl_source, execution_plan, ... }
GET /api/dsl/history/:business_ref   → { versions: [...] }
```

### Response Shapes

```typescript
// GET /api/dsl/list
interface DslListResponse {
  instances: DslInstanceSummary[];
  total: number;
}

interface DslInstanceSummary {
  instance_id: string;
  business_reference: string;
  domain_name: string;
  current_version: number;
  updated_at: string | null;
}

// GET /api/dsl/show/:ref
interface DslShowResponse {
  business_reference: string;
  domain_name: string;
  version: number;
  dsl_source: string;
  ast_json: object | null;
  execution_plan: ExecutionStepInfo[];
  compilation_status: string;
  created_at: string | null;
}

interface ExecutionStepInfo {
  step: number;
  verb: string;
  bind_as: string | null;
  injections: string[];  // e.g., ["cbu-id ← $0"]
}

// GET /api/dsl/history/:ref
interface DslHistoryResponse {
  business_reference: string;
  versions: DslVersionSummary[];
}

interface DslVersionSummary {
  version: number;
  operation_type: string;
  compilation_status: string;
  created_at: string | null;
}
```

---

## Part 2: UI Design

### 2.1 Layout

```
┌─────────────────────────────────────────────────────────────────┐
│  DSL Viewer                                        [↻ Refresh]  │
├─────────────────────────────────────────────────────────────────┤
│  Instance: [▼ e2e-test-cbu-creation        ]  Version: [▼ v1 ]  │
├────────────────────────────┬────────────────────────────────────┤
│                            │                                    │
│  📄 DSL Source             │  📋 Execution Plan                 │
│  ─────────────             │  ──────────────────                │
│                            │                                    │
│  ;; CBU Creation           │  Step 0: cbu.create                │
│  (cbu.create               │    → binds: @gamma                 │
│    :name "Gamma Holdings"  │                                    │
│    :client-type "COMPANY"  │  Step 1: entity.create-proper...   │
│    :jurisdiction "DE"      │    → binds: @mike                  │
│    :as @gamma)             │                                    │
│                            │  Step 2: cbu.assign-role           │
│  (entity.create-proper...  │    ← injects: cbu-id from $0       │
│    :first-name "Mike"      │    ← injects: entity-id from $1    │
│    ...                     │                                    │
│                            │                                    │
└────────────────────────────┴────────────────────────────────────┘
│  ✓ Compiled | 3 steps | Created: 2025-01-15 10:30:00            │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Interactions

| Element | Action |
|---------|--------|
| Instance dropdown | Load list from `/api/dsl/list`, on change fetch `/api/dsl/show/:ref` |
| Version dropdown | Populated from `/api/dsl/history/:ref`, on change fetch specific version |
| Refresh button | Re-fetch current selection |
| DSL Source panel | Read-only, monospace font, line numbers optional |
| Execution Plan panel | Formatted list showing step → verb → bindings → injections |

---

## Part 3: Implementation

### 3.1 Replace `types.ts`

**File:** `phase6-web-client/src/types.ts`

```typescript
// DSL Viewer Types - Matches Rust API responses

export interface DslInstanceSummary {
  instance_id: string;
  business_reference: string;
  domain_name: string;
  current_version: number;
  updated_at: string | null;
}

export interface DslShowResponse {
  business_reference: string;
  domain_name: string;
  version: number;
  dsl_source: string;
  ast_json: object | null;
  execution_plan: ExecutionStepInfo[];
  compilation_status: string;
  created_at: string | null;
}

export interface ExecutionStepInfo {
  step: number;
  verb: string;
  bind_as: string | null;
  injections: string[];
}

export interface DslVersionSummary {
  version: number;
  operation_type: string;
  compilation_status: string;
  created_at: string | null;
}

export interface DslHistoryResponse {
  business_reference: string;
  versions: DslVersionSummary[];
}

export interface DslListResponse {
  instances: DslInstanceSummary[];
  total: number;
}

// App State
export interface AppState {
  instances: DslInstanceSummary[];
  selectedInstance: string | null;  // business_reference
  versions: DslVersionSummary[];
  selectedVersion: number | null;
  displayData: DslShowResponse | null;
  loading: boolean;
  error: string | null;
}

// UI Config
export interface UiConfig {
  fontSize: number;
  showLineNumbers: boolean;
  theme: 'light' | 'dark';
}

// API Config
export interface ApiConfig {
  baseUrl: string;
  timeout: number;
}

// Defaults
export const DEFAULT_API_CONFIG: ApiConfig = {
  baseUrl: 'http://localhost:8080',
  timeout: 10000,
};

export const DEFAULT_UI_CONFIG: UiConfig = {
  fontSize: 14,
  showLineNumbers: true,
  theme: 'light',
};

export const DEFAULT_APP_STATE: AppState = {
  instances: [],
  selectedInstance: null,
  versions: [],
  selectedVersion: null,
  displayData: null,
  loading: false,
  error: null,
};
```

### 3.2 Replace `api.ts`

**File:** `phase6-web-client/src/api.ts`

```typescript
// DSL Viewer API Client

import {
  DslListResponse,
  DslShowResponse,
  DslHistoryResponse,
  ApiConfig,
  DEFAULT_API_CONFIG,
} from './types';

export interface DslViewerApi {
  listInstances(limit?: number, domain?: string): Promise<DslListResponse>;
  showDsl(businessRef: string, version?: number): Promise<DslShowResponse>;
  getHistory(businessRef: string): Promise<DslHistoryResponse>;
  healthCheck(): Promise<boolean>;
}

export class DslViewerApiClient implements DslViewerApi {
  private config: ApiConfig;

  constructor(config: Partial<ApiConfig> = {}) {
    this.config = { ...DEFAULT_API_CONFIG, ...config };
  }

  async listInstances(limit?: number, domain?: string): Promise<DslListResponse> {
    const params = new URLSearchParams();
    if (limit) params.set('limit', limit.toString());
    if (domain) params.set('domain', domain);
    
    const url = `${this.config.baseUrl}/api/dsl/list?${params}`;
    const response = await this.fetchWithTimeout(url);
    
    if (!response.ok) {
      throw new Error(`Failed to list instances: ${response.statusText}`);
    }
    
    return response.json();
  }

  async showDsl(businessRef: string, version?: number): Promise<DslShowResponse> {
    const url = version
      ? `${this.config.baseUrl}/api/dsl/show/${encodeURIComponent(businessRef)}/${version}`
      : `${this.config.baseUrl}/api/dsl/show/${encodeURIComponent(businessRef)}`;
    
    const response = await this.fetchWithTimeout(url);
    
    if (!response.ok) {
      if (response.status === 404) {
        throw new Error(`DSL not found: ${businessRef}`);
      }
      throw new Error(`Failed to fetch DSL: ${response.statusText}`);
    }
    
    return response.json();
  }

  async getHistory(businessRef: string): Promise<DslHistoryResponse> {
    const url = `${this.config.baseUrl}/api/dsl/history/${encodeURIComponent(businessRef)}`;
    const response = await this.fetchWithTimeout(url);
    
    if (!response.ok) {
      if (response.status === 404) {
        throw new Error(`No history found: ${businessRef}`);
      }
      throw new Error(`Failed to fetch history: ${response.statusText}`);
    }
    
    return response.json();
  }

  async healthCheck(): Promise<boolean> {
    try {
      const response = await fetch(`${this.config.baseUrl}/api/agent/health`, {
        signal: AbortSignal.timeout(5000),
      });
      return response.ok;
    } catch {
      return false;
    }
  }

  private async fetchWithTimeout(url: string): Promise<Response> {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.config.timeout);
    
    try {
      const response = await fetch(url, { signal: controller.signal });
      clearTimeout(timeoutId);
      return response;
    } catch (error) {
      clearTimeout(timeoutId);
      if (error instanceof DOMException && error.name === 'AbortError') {
        throw new Error('Request timed out');
      }
      throw error;
    }
  }
}

// Factory
export function createApiClient(config?: Partial<ApiConfig>): DslViewerApi {
  return new DslViewerApiClient(config);
}
```

### 3.3 Replace `app.ts`

**File:** `phase6-web-client/src/app.ts`

```typescript
// DSL Viewer Application

import {
  AppState,
  DslInstanceSummary,
  DslShowResponse,
  DslVersionSummary,
  UiConfig,
  DEFAULT_APP_STATE,
  DEFAULT_UI_CONFIG,
} from './types';
import { DslViewerApi } from './api';

export class DslVisualizerApp {
  private state: AppState;
  private api: DslViewerApi;
  private config: UiConfig;
  private container: HTMLElement | null = null;

  constructor(api: DslViewerApi, config: Partial<UiConfig> = {}) {
    this.api = api;
    this.config = { ...DEFAULT_UI_CONFIG, ...config };
    this.state = { ...DEFAULT_APP_STATE };
  }

  async mount(element: HTMLElement): Promise<void> {
    this.container = element;
    this.render();
    await this.loadInstances();
  }

  private async loadInstances(): Promise<void> {
    this.setState({ loading: true, error: null });
    
    try {
      const response = await this.api.listInstances(50);
      this.setState({ 
        instances: response.instances,
        loading: false 
      });
      
      // Auto-select first instance if available
      if (response.instances.length > 0) {
        await this.selectInstance(response.instances[0].business_reference);
      }
    } catch (error) {
      this.setState({ 
        loading: false, 
        error: error instanceof Error ? error.message : 'Failed to load instances' 
      });
    }
  }

  private async selectInstance(businessRef: string): Promise<void> {
    this.setState({ 
      selectedInstance: businessRef, 
      loading: true, 
      error: null 
    });
    
    try {
      // Load history first
      const history = await this.api.getHistory(businessRef);
      this.setState({ versions: history.versions });
      
      // Load latest version
      const displayData = await this.api.showDsl(businessRef);
      this.setState({ 
        displayData,
        selectedVersion: displayData.version,
        loading: false 
      });
    } catch (error) {
      this.setState({ 
        loading: false, 
        error: error instanceof Error ? error.message : 'Failed to load DSL' 
      });
    }
  }

  private async selectVersion(version: number): Promise<void> {
    if (!this.state.selectedInstance) return;
    
    this.setState({ loading: true, error: null });
    
    try {
      const displayData = await this.api.showDsl(this.state.selectedInstance, version);
      this.setState({ 
        displayData,
        selectedVersion: version,
        loading: false 
      });
    } catch (error) {
      this.setState({ 
        loading: false, 
        error: error instanceof Error ? error.message : 'Failed to load version' 
      });
    }
  }

  private setState(partial: Partial<AppState>): void {
    this.state = { ...this.state, ...partial };
    this.render();
  }

  private render(): void {
    if (!this.container) return;
    
    this.container.innerHTML = this.template();
    this.attachEventListeners();
  }

  private template(): string {
    return `
      <div class="dsl-viewer">
        <header class="header">
          <h1>🔍 DSL Viewer</h1>
          <button id="refresh-btn" class="btn" ${this.state.loading ? 'disabled' : ''}>
            ↻ Refresh
          </button>
        </header>

        ${this.state.error ? `<div class="error-banner">❌ ${this.state.error}</div>` : ''}

        <div class="controls">
          <div class="control-group">
            <label for="instance-select">Instance:</label>
            <select id="instance-select" ${this.state.loading ? 'disabled' : ''}>
              ${this.renderInstanceOptions()}
            </select>
          </div>
          <div class="control-group">
            <label for="version-select">Version:</label>
            <select id="version-select" ${this.state.loading ? 'disabled' : ''}>
              ${this.renderVersionOptions()}
            </select>
          </div>
        </div>

        <main class="main-content">
          <div class="panel dsl-panel">
            <div class="panel-header">📄 DSL Source</div>
            <div class="panel-content">
              <pre class="code-editor">${this.escapeHtml(this.state.displayData?.dsl_source || 'Select an instance to view DSL...')}</pre>
            </div>
          </div>

          <div class="panel plan-panel">
            <div class="panel-header">📋 Execution Plan</div>
            <div class="panel-content">
              ${this.renderExecutionPlan()}
            </div>
          </div>
        </main>

        <footer class="status-bar">
          ${this.renderStatusBar()}
        </footer>

        <style>${this.styles()}</style>
      </div>
    `;
  }

  private renderInstanceOptions(): string {
    if (this.state.instances.length === 0) {
      return '<option value="">No instances found</option>';
    }
    
    return this.state.instances.map(inst => `
      <option value="${inst.business_reference}" 
              ${inst.business_reference === this.state.selectedInstance ? 'selected' : ''}>
        ${inst.business_reference} (${inst.domain_name})
      </option>
    `).join('');
  }

  private renderVersionOptions(): string {
    if (this.state.versions.length === 0) {
      return '<option value="">No versions</option>';
    }
    
    return this.state.versions.map(v => `
      <option value="${v.version}" 
              ${v.version === this.state.selectedVersion ? 'selected' : ''}>
        v${v.version} - ${v.operation_type} (${v.compilation_status})
      </option>
    `).join('');
  }

  private renderExecutionPlan(): string {
    const plan = this.state.displayData?.execution_plan;
    
    if (!plan || plan.length === 0) {
      return '<div class="empty-state">No execution plan available</div>';
    }
    
    return `
      <div class="execution-steps">
        ${plan.map(step => `
          <div class="step">
            <div class="step-header">
              <span class="step-number">Step ${step.step}</span>
              <span class="step-verb">${step.verb}</span>
              ${step.bind_as ? `<span class="step-binding">→ @${step.bind_as}</span>` : ''}
            </div>
            ${step.injections.length > 0 ? `
              <div class="step-injections">
                ${step.injections.map(inj => `
                  <div class="injection">← ${inj}</div>
                `).join('')}
              </div>
            ` : ''}
          </div>
        `).join('')}
      </div>
    `;
  }

  private renderStatusBar(): string {
    if (this.state.loading) {
      return '<span class="loading">Loading...</span>';
    }
    
    const data = this.state.displayData;
    if (!data) {
      return '<span>Ready - Select an instance</span>';
    }
    
    const stepCount = data.execution_plan?.length || 0;
    const created = data.created_at 
      ? new Date(data.created_at).toLocaleString() 
      : 'Unknown';
    
    return `
      <span class="status-item ${data.compilation_status === 'COMPILED' ? 'success' : 'warning'}">
        ${data.compilation_status === 'COMPILED' ? '✓' : '⚠'} ${data.compilation_status}
      </span>
      <span class="status-item">${stepCount} steps</span>
      <span class="status-item">Created: ${created}</span>
    `;
  }

  private attachEventListeners(): void {
    const instanceSelect = document.getElementById('instance-select') as HTMLSelectElement;
    const versionSelect = document.getElementById('version-select') as HTMLSelectElement;
    const refreshBtn = document.getElementById('refresh-btn');

    instanceSelect?.addEventListener('change', (e) => {
      const target = e.target as HTMLSelectElement;
      if (target.value) {
        this.selectInstance(target.value);
      }
    });

    versionSelect?.addEventListener('change', (e) => {
      const target = e.target as HTMLSelectElement;
      const version = parseInt(target.value, 10);
      if (!isNaN(version)) {
        this.selectVersion(version);
      }
    });

    refreshBtn?.addEventListener('click', () => {
      if (this.state.selectedInstance) {
        this.selectInstance(this.state.selectedInstance);
      } else {
        this.loadInstances();
      }
    });
  }

  private escapeHtml(text: string): string {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
  }

  private styles(): string {
    return `
      .dsl-viewer {
        height: 100vh;
        display: flex;
        flex-direction: column;
        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
        background: #f5f5f5;
      }

      .header {
        background: linear-gradient(135deg, #2c3e50 0%, #3498db 100%);
        color: white;
        padding: 1rem 2rem;
        display: flex;
        justify-content: space-between;
        align-items: center;
      }

      .header h1 {
        margin: 0;
        font-size: 1.5rem;
      }

      .btn {
        padding: 0.5rem 1rem;
        background: rgba(255,255,255,0.2);
        border: none;
        border-radius: 4px;
        color: white;
        cursor: pointer;
        font-size: 1rem;
      }

      .btn:hover:not(:disabled) {
        background: rgba(255,255,255,0.3);
      }

      .btn:disabled {
        opacity: 0.5;
        cursor: not-allowed;
      }

      .error-banner {
        background: #e74c3c;
        color: white;
        padding: 0.75rem 2rem;
      }

      .controls {
        background: white;
        padding: 1rem 2rem;
        display: flex;
        gap: 2rem;
        border-bottom: 1px solid #ddd;
      }

      .control-group {
        display: flex;
        align-items: center;
        gap: 0.5rem;
      }

      .control-group label {
        font-weight: 500;
        color: #555;
      }

      .control-group select {
        padding: 0.5rem;
        border: 1px solid #ddd;
        border-radius: 4px;
        font-size: 0.9rem;
        min-width: 250px;
      }

      .main-content {
        flex: 1;
        display: flex;
        gap: 0;
        min-height: 0;
        overflow: hidden;
      }

      .panel {
        flex: 1;
        display: flex;
        flex-direction: column;
        background: white;
        overflow: hidden;
      }

      .dsl-panel {
        border-right: 1px solid #ddd;
      }

      .panel-header {
        padding: 0.75rem 1rem;
        background: #f8f9fa;
        border-bottom: 1px solid #ddd;
        font-weight: 600;
        font-size: 1rem;
      }

      .panel-content {
        flex: 1;
        overflow: auto;
        padding: 1rem;
      }

      .code-editor {
        margin: 0;
        font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
        font-size: ${this.config.fontSize}px;
        line-height: 1.5;
        white-space: pre-wrap;
        word-break: break-word;
        color: #2d3436;
      }

      .empty-state {
        color: #888;
        font-style: italic;
        padding: 2rem;
        text-align: center;
      }

      .execution-steps {
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
      }

      .step {
        background: #f8f9fa;
        border: 1px solid #e9ecef;
        border-radius: 4px;
        padding: 0.75rem;
      }

      .step-header {
        display: flex;
        align-items: center;
        gap: 0.75rem;
        font-family: 'Monaco', 'Menlo', monospace;
        font-size: 0.9rem;
      }

      .step-number {
        background: #3498db;
        color: white;
        padding: 0.2rem 0.5rem;
        border-radius: 3px;
        font-size: 0.8rem;
        font-weight: 600;
      }

      .step-verb {
        color: #2c3e50;
        font-weight: 500;
      }

      .step-binding {
        color: #27ae60;
        font-weight: 500;
      }

      .step-injections {
        margin-top: 0.5rem;
        padding-left: 1rem;
        border-left: 2px solid #3498db;
      }

      .injection {
        font-family: 'Monaco', 'Menlo', monospace;
        font-size: 0.85rem;
        color: #e67e22;
        padding: 0.2rem 0;
      }

      .status-bar {
        background: #2c3e50;
        color: white;
        padding: 0.5rem 2rem;
        display: flex;
        gap: 2rem;
        font-size: 0.9rem;
      }

      .status-item {
        display: flex;
        align-items: center;
        gap: 0.3rem;
      }

      .status-item.success {
        color: #2ecc71;
      }

      .status-item.warning {
        color: #f39c12;
      }

      .loading {
        color: #f39c12;
      }

      @media (max-width: 768px) {
        .main-content {
          flex-direction: column;
        }
        
        .dsl-panel {
          border-right: none;
          border-bottom: 1px solid #ddd;
        }
        
        .controls {
          flex-direction: column;
          gap: 1rem;
        }
        
        .control-group select {
          min-width: 100%;
        }
      }
    `;
  }

  // Public methods
  handleGlobalError(error: Error | string): void {
    const message = error instanceof Error ? error.message : error;
    this.setState({ error: message });
  }

  cleanup(): void {
    // Nothing to cleanup currently
  }
}
```

### 3.4 Update `main.ts`

**File:** `phase6-web-client/src/main.ts`

```typescript
// DSL Viewer - Main Entry Point

import { DslVisualizerApp } from './app';
import { createApiClient } from './api';
import { DEFAULT_API_CONFIG, DEFAULT_UI_CONFIG } from './types';

async function init(): Promise<void> {
  console.log('🔍 DSL Viewer starting...');

  const config = {
    apiBaseUrl: import.meta.env.VITE_API_BASE_URL || DEFAULT_API_CONFIG.baseUrl,
  };

  console.log('📡 API URL:', config.apiBaseUrl);

  const apiClient = createApiClient({ baseUrl: config.apiBaseUrl });

  // Check API health
  const isHealthy = await apiClient.healthCheck();
  if (!isHealthy) {
    console.warn('⚠️ API health check failed - server may be offline');
  } else {
    console.log('✓ API is healthy');
  }

  const app = new DslVisualizerApp(apiClient, DEFAULT_UI_CONFIG);

  const container = document.getElementById('app');
  if (!container) {
    throw new Error('Mount point #app not found');
  }

  await app.mount(container);
  console.log('✓ DSL Viewer mounted');

  // Error handling
  window.addEventListener('unhandledrejection', (event) => {
    console.error('Unhandled rejection:', event.reason);
    app.handleGlobalError(event.reason);
  });
}

// Show loading state
const app = document.getElementById('app');
if (app) {
  app.innerHTML = `
    <div style="
      display: flex;
      align-items: center;
      justify-content: center;
      height: 100vh;
      font-family: sans-serif;
      background: linear-gradient(135deg, #2c3e50 0%, #3498db 100%);
      color: white;
    ">
      <div style="text-align: center;">
        <h1>🔍 DSL Viewer</h1>
        <p>Loading...</p>
      </div>
    </div>
  `;
}

// Start app
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
```

---

## Part 4: Running the UI

### 4.1 Prerequisites

1. Server running at `http://localhost:8080`
2. DSL data in database (from E2E tests or real usage)

### 4.2 Start Dev Server

```bash
cd phase6-web-client
npm install
npm run dev
```

### 4.3 Test Data

If no DSL instances exist, run the E2E tests first:

```bash
cd ../rust
DATABASE_URL="postgresql:///data_designer" cargo run --features database --bin run_e2e_tests
```

Or insert test data:

```sql
-- Create a test DSL instance
INSERT INTO "ob-poc".dsl_instances 
(instance_id, domain_name, business_reference, current_version, status, created_at, updated_at)
VALUES 
(gen_random_uuid(), 'cbu', 'test-viewer-instance', 1, 'ACTIVE', NOW(), NOW());

-- Get the instance_id
SELECT instance_id FROM "ob-poc".dsl_instances WHERE business_reference = 'test-viewer-instance';

-- Insert version with DSL content (use the instance_id from above)
INSERT INTO "ob-poc".dsl_instance_versions
(instance_id, version_number, dsl_content, operation_type, compilation_status, created_at)
VALUES 
('<instance_id>', 1, 
'(cbu.create 
  :name "Test Corp"
  :client-type "COMPANY"
  :jurisdiction "US"
  :as @test)

(entity.create-proper-person
  :first-name "Jane"
  :last-name "Doe"
  :as @jane)

(cbu.assign-role
  :cbu-id @test
  :entity-id @jane
  :role "Director")',
'EXECUTE', 'COMPILED', NOW());
```

---

## Part 5: Implementation Checklist

1. [ ] Replace `phase6-web-client/src/types.ts` with new types
2. [ ] Replace `phase6-web-client/src/api.ts` with new API client
3. [ ] Replace `phase6-web-client/src/app.ts` with new app class
4. [ ] Replace `phase6-web-client/src/main.ts` with new entry point
5. [ ] Run `npm install` (no new deps needed)
6. [ ] Start server: `cargo run --features database --bin agentic_server`
7. [ ] Start UI: `cd phase6-web-client && npm run dev`
8. [ ] Verify:
   - Instance dropdown populates
   - Selecting instance shows DSL + execution plan
   - Version dropdown works
   - Refresh button works

---

## Notes

- The UI is intentionally simple — no external dependencies beyond Vite
- All styling is inline CSS in the `styles()` method
- The execution plan visualization shows the dependency ordering clearly
- Error states are handled gracefully with user-friendly messages
