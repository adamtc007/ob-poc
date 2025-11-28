# Task: Template UI Integration

## Objective

Wire up the existing EntityPicker component and template APIs into a working UI for end-to-end testing of the template → DSL → execute pipeline.

## Current State

**Backend (Ready):**
- `GET /api/templates` — list templates ✅
- `GET /api/templates/:id` — get template definition ✅
- `POST /api/templates/:id/render` — render to DSL ✅
- `GET /api/entities/search?q=...&types=...` — entity search ✅
- `POST /api/session/:id/execute` — execute DSL (existing) ✅

**Frontend (Exists but not wired):**
- `EntityPicker.ts` — typeahead component ✅
- `api.ts` — has `searchEntities`, `listTemplates`, `getTemplate`, `renderTemplate` ✅
- `ui.ts` — still shows old chat interface ❌

## Target UI Layout

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Template Picker                                              [New Session] │
│  [Select Template... ▼]                                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────┐  ┌──────────────────────────────┐ │
│  │ TEMPLATE FORM                       │  │ DSL PREVIEW                  │ │
│  │                                     │  │                              │ │
│  │  CBU Name: [________________]       │  │ (cbu.ensure                  │ │
│  │                                     │  │   :cbu-name "Apex Capital"   │ │
│  │  Client Type: [COMPANY ▼]           │  │   :client-type "COMPANY"     │ │
│  │                                     │  │   :jurisdiction "GB")        │ │
│  │  Jurisdiction: [GB ▼]               │  │                              │ │
│  │                                     │  │                              │ │
│  │  Nature & Purpose:                  │  │                              │ │
│  │  [____________________________]     │  │                              │ │
│  │                                     │  │                              │ │
│  │  [Render DSL]                       │  │  [Execute DSL]               │ │
│  └─────────────────────────────────────┘  └──────────────────────────────┘ │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │ EXECUTION LOG                                                           ││
│  │ > Created CBU "Apex Capital" (id: abc123...)                            ││
│  │ > Session context updated: last_cbu_id = abc123...                      ││
│  └─────────────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────────┘
```

## Implementation

### 1. Update `rust/ui/src/types.ts`

Add to AppState:

```typescript
export interface AppState {
  // Existing fields...
  sessionId: string | null;
  sessionState: SessionState | null;
  messages: ChatMessage[];
  intents: VerbIntent[];
  validations: IntentValidation[];
  assembledDsl: string[];
  canExecute: boolean;
  loading: boolean;
  error: string | null;
  
  // NEW: Template state
  templates: TemplateSummary[];
  selectedTemplateId: string | null;
  selectedTemplate: FormTemplate | null;
  formValues: Record<string, unknown>;
  renderedDsl: string | null;
  executionLog: string[];
}
```

### 2. Update `rust/ui/src/state.ts`

Add initial state and updaters:

```typescript
const initialState: AppState = {
  // Existing...
  sessionId: null,
  sessionState: null,
  messages: [],
  intents: [],
  validations: [],
  assembledDsl: [],
  canExecute: false,
  loading: false,
  error: null,
  
  // NEW
  templates: [],
  selectedTemplateId: null,
  selectedTemplate: null,
  formValues: {},
  renderedDsl: null,
  executionLog: [],
};

// Add helper functions:
export function setTemplates(templates: TemplateSummary[]): void {
  setState({ templates });
}

export function selectTemplate(template: FormTemplate | null): void {
  setState({
    selectedTemplateId: template?.id ?? null,
    selectedTemplate: template,
    formValues: template ? getDefaultValues(template) : {},
    renderedDsl: null,
  });
}

export function setFormValue(name: string, value: unknown): void {
  const state = getState();
  setState({
    formValues: { ...state.formValues, [name]: value },
  });
}

export function setRenderedDsl(dsl: string | null): void {
  setState({ renderedDsl: dsl });
}

export function addLogEntry(entry: string): void {
  const state = getState();
  setState({
    executionLog: [...state.executionLog, `[${new Date().toLocaleTimeString()}] ${entry}`],
  });
}

function getDefaultValues(template: FormTemplate): Record<string, unknown> {
  const values: Record<string, unknown> = {};
  for (const slot of template.slots) {
    if (slot.default_value !== undefined) {
      values[slot.name] = slot.default_value;
    }
  }
  return values;
}
```

### 3. Update `rust/ui/src/index.html`

Replace content with new layout:

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>DSL Template Editor</title>
</head>
<body>
  <div class="app">
    <!-- Header -->
    <header class="header">
      <div class="header-left">
        <h1>DSL Template Editor</h1>
        <span id="session-info" class="session-info">No session</span>
        <span id="session-state" class="session-state"></span>
      </div>
      <div class="header-right">
        <select id="template-select" class="template-select">
          <option value="">Select Template...</option>
        </select>
        <button id="new-session-btn" class="btn btn-primary">New Session</button>
      </div>
    </header>

    <!-- Main Content -->
    <main class="main-content">
      <!-- Left: Template Form -->
      <section class="panel form-panel">
        <h2>Template Form</h2>
        <div id="form-container" class="form-container">
          <p class="placeholder-text">Select a template to begin</p>
        </div>
        <div class="form-actions">
          <button id="render-btn" class="btn btn-secondary" disabled>Render DSL</button>
        </div>
      </section>

      <!-- Right: DSL Preview -->
      <section class="panel dsl-panel">
        <h2>DSL Preview</h2>
        <pre id="dsl-preview" class="dsl-preview"><span class="placeholder-text">DSL will appear here</span></pre>
        <div class="dsl-actions">
          <button id="execute-btn" class="btn btn-success" disabled>Execute DSL</button>
        </div>
      </section>
    </main>

    <!-- Footer: Execution Log -->
    <footer class="log-panel">
      <h3>Execution Log</h3>
      <div id="execution-log" class="execution-log"></div>
    </footer>
  </div>

  <script type="module" src="/src/main.ts"></script>
</body>
</html>
```

### 4. Update `rust/ui/src/style.css`

Add new styles (append to existing):

```css
/* =============================================================================
   Template Editor Layout
   ============================================================================= */

.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: #f5f5f5;
}

.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 20px;
  background: #fff;
  border-bottom: 1px solid #ddd;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 16px;
}

.header-left h1 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
}

.header-right {
  display: flex;
  gap: 12px;
  align-items: center;
}

.template-select {
  padding: 8px 12px;
  border: 1px solid #ddd;
  border-radius: 6px;
  font-size: 14px;
  min-width: 200px;
}

.main-content {
  display: flex;
  flex: 1;
  gap: 16px;
  padding: 16px;
  overflow: hidden;
}

.panel {
  flex: 1;
  background: #fff;
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(0,0,0,0.1);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.panel h2 {
  margin: 0;
  padding: 12px 16px;
  font-size: 14px;
  font-weight: 600;
  background: #f9f9f9;
  border-bottom: 1px solid #eee;
}

.form-container {
  flex: 1;
  padding: 16px;
  overflow-y: auto;
}

.form-actions, .dsl-actions {
  padding: 12px 16px;
  border-top: 1px solid #eee;
  display: flex;
  gap: 8px;
}

.dsl-preview {
  flex: 1;
  margin: 0;
  padding: 16px;
  font-family: 'SF Mono', Monaco, 'Consolas', monospace;
  font-size: 13px;
  line-height: 1.5;
  background: #1e1e1e;
  color: #d4d4d4;
  overflow: auto;
  white-space: pre-wrap;
}

.log-panel {
  background: #fff;
  border-top: 1px solid #ddd;
  max-height: 150px;
}

.log-panel h3 {
  margin: 0;
  padding: 8px 16px;
  font-size: 12px;
  font-weight: 600;
  background: #f9f9f9;
  border-bottom: 1px solid #eee;
}

.execution-log {
  padding: 8px 16px;
  font-family: 'SF Mono', Monaco, monospace;
  font-size: 12px;
  max-height: 100px;
  overflow-y: auto;
}

.execution-log .log-entry {
  padding: 2px 0;
  color: #666;
}

.execution-log .log-entry.success {
  color: #2e7d32;
}

.execution-log .log-entry.error {
  color: #c62828;
}

.placeholder-text {
  color: #999;
  font-style: italic;
}

/* =============================================================================
   Form Field Styles
   ============================================================================= */

.form-field {
  margin-bottom: 16px;
}

.form-field label {
  display: block;
  margin-bottom: 6px;
  font-size: 13px;
  font-weight: 500;
  color: #333;
}

.form-field .required-star {
  color: #c62828;
  margin-left: 2px;
}

.form-field input[type="text"],
.form-field input[type="date"],
.form-field input[type="number"],
.form-field select,
.form-field textarea {
  width: 100%;
  padding: 10px 12px;
  border: 1px solid #ddd;
  border-radius: 6px;
  font-size: 14px;
  box-sizing: border-box;
}

.form-field textarea {
  min-height: 80px;
  resize: vertical;
}

.form-field input:focus,
.form-field select:focus,
.form-field textarea:focus {
  outline: none;
  border-color: #2196f3;
  box-shadow: 0 0 0 2px rgba(33, 150, 243, 0.1);
}

.form-field .help-text {
  margin-top: 4px;
  font-size: 12px;
  color: #666;
}

/* =============================================================================
   Buttons
   ============================================================================= */

.btn {
  padding: 10px 16px;
  border: none;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.15s, opacity 0.15s;
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-primary {
  background: #2196f3;
  color: #fff;
}

.btn-primary:hover:not(:disabled) {
  background: #1976d2;
}

.btn-secondary {
  background: #f5f5f5;
  color: #333;
  border: 1px solid #ddd;
}

.btn-secondary:hover:not(:disabled) {
  background: #eee;
}

.btn-success {
  background: #4caf50;
  color: #fff;
}

.btn-success:hover:not(:disabled) {
  background: #388e3c;
}

/* =============================================================================
   Session Info
   ============================================================================= */

.session-info {
  font-size: 12px;
  color: #666;
  padding: 4px 8px;
  background: #f5f5f5;
  border-radius: 4px;
}

.session-info.active {
  background: #e3f2fd;
  color: #1565c0;
}

.session-state {
  font-size: 11px;
  padding: 3px 8px;
  border-radius: 4px;
  text-transform: uppercase;
  font-weight: 600;
}

.session-state.new { background: #e3f2fd; color: #1565c0; }
.session-state.ready_to_execute { background: #e8f5e9; color: #2e7d32; }
.session-state.executed { background: #f3e5f5; color: #7b1fa2; }
```

### 5. Create `rust/ui/src/formRenderer.ts`

New file to render template forms dynamically:

```typescript
import { EntityPicker } from './components/EntityPicker';
import type { FormTemplate, SlotDefinition, SlotType, EntityType } from './types';

export interface FormRendererConfig {
  container: HTMLElement;
  template: FormTemplate;
  values: Record<string, unknown>;
  onChange: (name: string, value: unknown) => void;
}

export class FormRenderer {
  private config: FormRendererConfig;
  private pickers: Map<string, EntityPicker> = new Map();

  constructor(config: FormRendererConfig) {
    this.config = config;
    this.render();
  }

  private render(): void {
    this.config.container.innerHTML = '';
    this.pickers.clear();

    for (const slot of this.config.template.slots) {
      const field = this.createField(slot);
      this.config.container.appendChild(field);
    }
  }

  private createField(slot: SlotDefinition): HTMLDivElement {
    const field = document.createElement('div');
    field.className = 'form-field';
    field.dataset.slot = slot.name;

    // Label
    const label = document.createElement('label');
    label.htmlFor = `field-${slot.name}`;
    label.innerHTML = slot.label;
    if (slot.required) {
      label.innerHTML += '<span class="required-star">*</span>';
    }
    field.appendChild(label);

    // Input based on slot type
    const input = this.createInput(slot);
    field.appendChild(input);

    // Help text
    if (slot.help_text) {
      const help = document.createElement('div');
      help.className = 'help-text';
      help.textContent = slot.help_text;
      field.appendChild(help);
    }

    return field;
  }

  private createInput(slot: SlotDefinition): HTMLElement {
    const value = this.config.values[slot.name];
    const slotType = slot.slot_type;

    switch (slotType.type) {
      case 'text':
        return this.createTextInput(slot, slotType, value);
      
      case 'enum':
        return this.createEnumSelect(slot, slotType, value);
      
      case 'entity_ref':
        return this.createEntityPicker(slot, slotType, value);
      
      case 'country':
        return this.createCountrySelect(slot, value);
      
      case 'date':
        return this.createDateInput(slot, value);
      
      case 'percentage':
      case 'integer':
      case 'decimal':
        return this.createNumberInput(slot, slotType, value);
      
      case 'boolean':
        return this.createCheckbox(slot, value);
      
      default:
        return this.createTextInput(slot, { type: 'text' }, value);
    }
  }

  private createTextInput(
    slot: SlotDefinition,
    slotType: { type: 'text'; max_length?: number; multiline?: boolean },
    value: unknown
  ): HTMLElement {
    if (slotType.multiline) {
      const textarea = document.createElement('textarea');
      textarea.id = `field-${slot.name}`;
      textarea.value = String(value ?? '');
      textarea.placeholder = slot.placeholder ?? '';
      if (slotType.max_length) {
        textarea.maxLength = slotType.max_length;
      }
      textarea.addEventListener('input', () => {
        this.config.onChange(slot.name, textarea.value);
      });
      return textarea;
    }

    const input = document.createElement('input');
    input.type = 'text';
    input.id = `field-${slot.name}`;
    input.value = String(value ?? '');
    input.placeholder = slot.placeholder ?? '';
    if (slotType.max_length) {
      input.maxLength = slotType.max_length;
    }
    input.addEventListener('input', () => {
      this.config.onChange(slot.name, input.value);
    });
    return input;
  }

  private createEnumSelect(
    slot: SlotDefinition,
    slotType: { type: 'enum'; options: Array<{ value: string; label: string }> },
    value: unknown
  ): HTMLSelectElement {
    const select = document.createElement('select');
    select.id = `field-${slot.name}`;

    // Add empty option if not required
    if (!slot.required) {
      const empty = document.createElement('option');
      empty.value = '';
      empty.textContent = '-- Select --';
      select.appendChild(empty);
    }

    for (const opt of slotType.options) {
      const option = document.createElement('option');
      option.value = opt.value;
      option.textContent = opt.label;
      if (opt.value === value) {
        option.selected = true;
      }
      select.appendChild(option);
    }

    select.addEventListener('change', () => {
      this.config.onChange(slot.name, select.value);
    });

    return select;
  }

  private createEntityPicker(
    slot: SlotDefinition,
    slotType: { type: 'entity_ref'; allowed_types: EntityType[]; allow_create: boolean },
    value: unknown
  ): HTMLElement {
    const container = document.createElement('div');
    container.id = `field-${slot.name}`;

    const picker = new EntityPicker({
      container,
      allowedTypes: slotType.allowed_types,
      allowCreate: slotType.allow_create,
      placeholder: slot.placeholder ?? `Search ${slotType.allowed_types.join(', ')}...`,
      onSelect: (entity) => {
        this.config.onChange(slot.name, entity?.id ?? null);
      },
      onCreate: (type) => {
        // TODO: Open create dialog
        console.log('Create new:', type);
      },
    });

    this.pickers.set(slot.name, picker);

    return container;
  }

  private createCountrySelect(slot: SlotDefinition, value: unknown): HTMLSelectElement {
    // Common countries - could be expanded
    const countries = [
      { code: 'GB', name: 'United Kingdom' },
      { code: 'US', name: 'United States' },
      { code: 'DE', name: 'Germany' },
      { code: 'FR', name: 'France' },
      { code: 'CH', name: 'Switzerland' },
      { code: 'LU', name: 'Luxembourg' },
      { code: 'IE', name: 'Ireland' },
      { code: 'NL', name: 'Netherlands' },
      { code: 'SG', name: 'Singapore' },
      { code: 'HK', name: 'Hong Kong' },
      { code: 'JE', name: 'Jersey' },
      { code: 'GG', name: 'Guernsey' },
      { code: 'KY', name: 'Cayman Islands' },
      { code: 'BVI', name: 'British Virgin Islands' },
    ];

    const select = document.createElement('select');
    select.id = `field-${slot.name}`;

    const empty = document.createElement('option');
    empty.value = '';
    empty.textContent = '-- Select Country --';
    select.appendChild(empty);

    for (const c of countries) {
      const option = document.createElement('option');
      option.value = c.code;
      option.textContent = `${c.name} (${c.code})`;
      if (c.code === value) {
        option.selected = true;
      }
      select.appendChild(option);
    }

    select.addEventListener('change', () => {
      this.config.onChange(slot.name, select.value);
    });

    return select;
  }

  private createDateInput(slot: SlotDefinition, value: unknown): HTMLInputElement {
    const input = document.createElement('input');
    input.type = 'date';
    input.id = `field-${slot.name}`;
    input.value = String(value ?? '');
    input.addEventListener('change', () => {
      this.config.onChange(slot.name, input.value);
    });
    return input;
  }

  private createNumberInput(
    slot: SlotDefinition,
    slotType: { type: 'percentage' | 'integer' | 'decimal'; min?: number; max?: number },
    value: unknown
  ): HTMLInputElement {
    const input = document.createElement('input');
    input.type = 'number';
    input.id = `field-${slot.name}`;
    input.value = String(value ?? '');
    input.placeholder = slot.placeholder ?? '';

    if (slotType.type === 'percentage') {
      input.min = '0';
      input.max = '100';
      input.step = '0.01';
    } else if (slotType.type === 'integer') {
      input.step = '1';
      if (slotType.min !== undefined) input.min = String(slotType.min);
      if (slotType.max !== undefined) input.max = String(slotType.max);
    } else {
      input.step = '0.01';
    }

    input.addEventListener('input', () => {
      const numValue = slotType.type === 'integer' 
        ? parseInt(input.value, 10) 
        : parseFloat(input.value);
      this.config.onChange(slot.name, isNaN(numValue) ? null : numValue);
    });

    return input;
  }

  private createCheckbox(slot: SlotDefinition, value: unknown): HTMLElement {
    const wrapper = document.createElement('div');
    wrapper.className = 'checkbox-wrapper';

    const input = document.createElement('input');
    input.type = 'checkbox';
    input.id = `field-${slot.name}`;
    input.checked = Boolean(value);
    input.addEventListener('change', () => {
      this.config.onChange(slot.name, input.checked);
    });

    const label = document.createElement('label');
    label.htmlFor = `field-${slot.name}`;
    label.textContent = slot.label;

    wrapper.appendChild(input);
    wrapper.appendChild(label);

    return wrapper;
  }

  // Public: Get all current values
  getValues(): Record<string, unknown> {
    return { ...this.config.values };
  }

  // Public: Check if all required fields are filled
  isValid(): boolean {
    for (const slot of this.config.template.slots) {
      if (slot.required) {
        const value = this.config.values[slot.name];
        if (value === undefined || value === null || value === '') {
          return false;
        }
      }
    }
    return true;
  }

  // Public: Destroy and cleanup
  destroy(): void {
    this.pickers.clear();
    this.config.container.innerHTML = '';
  }
}
```

### 6. Update `rust/ui/src/main.ts`

Replace with new main logic:

```typescript
import { api } from './api';
import { FormRenderer } from './formRenderer';
import {
  getState,
  setState,
  subscribe,
  setLoading,
  setError,
  setTemplates,
  selectTemplate,
  setFormValue,
  setRenderedDsl,
  addLogEntry,
  setSession,
} from './state';
import type { FormTemplate, TemplateSummary } from './types';
import './style.css';

// ============================================================================
// DOM References
// ============================================================================

let formRenderer: FormRenderer | null = null;

function $(id: string): HTMLElement {
  const el = document.getElementById(id);
  if (!el) throw new Error(`Element not found: ${id}`);
  return el;
}

// ============================================================================
// Render Functions
// ============================================================================

function renderTemplateSelect(templates: TemplateSummary[]): void {
  const select = $('template-select') as HTMLSelectElement;
  select.innerHTML = '<option value="">Select Template...</option>';
  
  // Group by domain
  const byDomain = new Map<string, TemplateSummary[]>();
  for (const t of templates) {
    if (!byDomain.has(t.domain)) {
      byDomain.set(t.domain, []);
    }
    byDomain.get(t.domain)!.push(t);
  }
  
  for (const [domain, domainTemplates] of byDomain) {
    const optgroup = document.createElement('optgroup');
    optgroup.label = domain.toUpperCase();
    
    for (const t of domainTemplates) {
      const option = document.createElement('option');
      option.value = t.id;
      option.textContent = t.name;
      optgroup.appendChild(option);
    }
    
    select.appendChild(optgroup);
  }
}

function renderSessionInfo(): void {
  const state = getState();
  const infoEl = $('session-info');
  const stateEl = $('session-state');
  
  if (state.sessionId) {
    infoEl.textContent = `Session: ${state.sessionId.substring(0, 8)}...`;
    infoEl.className = 'session-info active';
  } else {
    infoEl.textContent = 'No session';
    infoEl.className = 'session-info';
  }
  
  if (state.sessionState) {
    stateEl.textContent = state.sessionState.replace(/_/g, ' ');
    stateEl.className = `session-state ${state.sessionState}`;
    stateEl.style.display = 'inline-block';
  } else {
    stateEl.style.display = 'none';
  }
}

function renderForm(): void {
  const state = getState();
  const container = $('form-container');
  const renderBtn = $('render-btn') as HTMLButtonElement;
  
  // Cleanup previous
  if (formRenderer) {
    formRenderer.destroy();
    formRenderer = null;
  }
  
  if (!state.selectedTemplate) {
    container.innerHTML = '<p class="placeholder-text">Select a template to begin</p>';
    renderBtn.disabled = true;
    return;
  }
  
  // Create new form
  formRenderer = new FormRenderer({
    container,
    template: state.selectedTemplate,
    values: state.formValues,
    onChange: (name, value) => {
      setFormValue(name, value);
      updateRenderButton();
    },
  });
  
  updateRenderButton();
}

function updateRenderButton(): void {
  const renderBtn = $('render-btn') as HTMLButtonElement;
  renderBtn.disabled = !formRenderer || !formRenderer.isValid();
}

function renderDslPreview(): void {
  const state = getState();
  const preview = $('dsl-preview');
  const executeBtn = $('execute-btn') as HTMLButtonElement;
  
  if (state.renderedDsl) {
    preview.textContent = state.renderedDsl;
    preview.classList.remove('placeholder-text');
    executeBtn.disabled = !state.sessionId;
  } else {
    preview.innerHTML = '<span class="placeholder-text">DSL will appear here</span>';
    executeBtn.disabled = true;
  }
}

function renderExecutionLog(): void {
  const state = getState();
  const logEl = $('execution-log');
  
  logEl.innerHTML = state.executionLog
    .map(entry => {
      const isError = entry.includes('Error') || entry.includes('Failed');
      const isSuccess = entry.includes('Success') || entry.includes('Created');
      const cls = isError ? 'error' : isSuccess ? 'success' : '';
      return `<div class="log-entry ${cls}">${entry}</div>`;
    })
    .join('');
  
  // Auto-scroll to bottom
  logEl.scrollTop = logEl.scrollHeight;
}

function render(): void {
  renderSessionInfo();
  renderForm();
  renderDslPreview();
  renderExecutionLog();
}

// ============================================================================
// Event Handlers
// ============================================================================

async function handleNewSession(): Promise<void> {
  try {
    setLoading(true);
    const response = await api.createSession({});
    setSession(response.session_id, response.state);
    addLogEntry(`Session created: ${response.session_id.substring(0, 8)}...`);
  } catch (err) {
    const msg = err instanceof Error ? err.message : 'Unknown error';
    setError(msg);
    addLogEntry(`Error creating session: ${msg}`);
  } finally {
    setLoading(false);
  }
}

async function handleTemplateSelect(templateId: string): Promise<void> {
  if (!templateId) {
    selectTemplate(null);
    return;
  }
  
  try {
    setLoading(true);
    const template = await api.getTemplate(templateId);
    selectTemplate(template);
    addLogEntry(`Template loaded: ${template.name}`);
  } catch (err) {
    const msg = err instanceof Error ? err.message : 'Unknown error';
    setError(msg);
    addLogEntry(`Error loading template: ${msg}`);
  } finally {
    setLoading(false);
  }
}

async function handleRenderDsl(): Promise<void> {
  const state = getState();
  if (!state.selectedTemplateId || !formRenderer) return;
  
  try {
    setLoading(true);
    const values = formRenderer.getValues();
    const response = await api.renderTemplate(state.selectedTemplateId, values);
    setRenderedDsl(response.dsl);
    addLogEntry(`DSL rendered: ${response.verb}`);
  } catch (err) {
    const msg = err instanceof Error ? err.message : 'Unknown error';
    setError(msg);
    addLogEntry(`Error rendering DSL: ${msg}`);
  } finally {
    setLoading(false);
  }
}

async function handleExecuteDsl(): Promise<void> {
  const state = getState();
  if (!state.sessionId || !state.renderedDsl) return;
  
  try {
    setLoading(true);
    addLogEntry(`Executing DSL...`);
    
    // First, we need to send the DSL to the session
    // Use chat endpoint with the rendered DSL
    const chatResponse = await api.chat(state.sessionId, {
      message: `Execute: ${state.renderedDsl}`,
    });
    
    if (chatResponse.can_execute) {
      const execResponse = await api.execute(state.sessionId, { dry_run: false });
      
      if (execResponse.success) {
        for (const result of execResponse.results) {
          if (result.success) {
            addLogEntry(`✓ ${result.message}`);
          } else {
            addLogEntry(`✗ ${result.message}`);
          }
        }
        addLogEntry(`Execution complete: ${execResponse.results.length} statement(s)`);
        
        // Clear for next operation
        setRenderedDsl(null);
        setState({ sessionState: execResponse.new_state });
      } else {
        for (const error of execResponse.errors) {
          addLogEntry(`Error: ${error}`);
        }
      }
    } else {
      addLogEntry(`Cannot execute: validation failed`);
    }
  } catch (err) {
    const msg = err instanceof Error ? err.message : 'Unknown error';
    setError(msg);
    addLogEntry(`Execution error: ${msg}`);
  } finally {
    setLoading(false);
  }
}

// ============================================================================
// Initialization
// ============================================================================

function bindEvents(): void {
  $('new-session-btn').addEventListener('click', handleNewSession);
  
  ($('template-select') as HTMLSelectElement).addEventListener('change', (e) => {
    const select = e.target as HTMLSelectElement;
    handleTemplateSelect(select.value);
  });
  
  $('render-btn').addEventListener('click', handleRenderDsl);
  $('execute-btn').addEventListener('click', handleExecuteDsl);
}

async function init(): Promise<void> {
  // Subscribe to state changes
  subscribe(render);
  
  // Bind events
  bindEvents();
  
  // Load templates
  try {
    const response = await api.listTemplates();
    setTemplates(response.templates);
    renderTemplateSelect(response.templates);
  } catch (err) {
    console.error('Failed to load templates:', err);
    addLogEntry('Error: Failed to load templates');
  }
  
  // Initial render
  render();
}

// Start
document.addEventListener('DOMContentLoaded', init);
```

### 7. Update `rust/ui/src/api.ts`

Ensure these methods exist (add if missing):

```typescript
// Add to api object if not present:

listTemplates(): Promise<{ templates: TemplateSummary[] }> {
  return request('GET', '/api/templates');
},

getTemplate(id: string): Promise<FormTemplate> {
  return request('GET', `/api/templates/${encodeURIComponent(id)}`);
},

renderTemplate(id: string, values: Record<string, unknown>): Promise<{ dsl: string; verb: string }> {
  return request('POST', `/api/templates/${encodeURIComponent(id)}/render`, { values });
},

searchEntities(params: { q: string; types?: string[]; limit?: number }): Promise<EntitySearchResponse> {
  const searchParams = new URLSearchParams();
  searchParams.set('q', params.q);
  if (params.types?.length) {
    searchParams.set('types', params.types.join(','));
  }
  if (params.limit) {
    searchParams.set('limit', String(params.limit));
  }
  return request('GET', `/api/entities/search?${searchParams}`);
},
```

## Files to Create

| File | Purpose |
|------|---------|
| `rust/ui/src/formRenderer.ts` | Dynamic form renderer for templates |

## Files to Modify

| File | Changes |
|------|---------|
| `rust/ui/index.html` | New layout with template picker |
| `rust/ui/src/types.ts` | Add template state to AppState |
| `rust/ui/src/state.ts` | Add template state management |
| `rust/ui/src/style.css` | New styles for template editor |
| `rust/ui/src/main.ts` | New event handlers and render logic |
| `rust/ui/src/api.ts` | Ensure template/entity API methods exist |

## Testing

1. Build and run:
```bash
cd rust/ui && npm run build
cd .. && cargo run --bin agentic_server --features server
```

2. Open http://localhost:3000

3. Test flow:
   - Click "New Session" → should see session ID
   - Select "Create CBU" from dropdown → form appears
   - Fill in CBU Name, Client Type, Jurisdiction
   - Click "Render DSL" → see DSL in preview
   - Click "Execute DSL" → see execution log

4. Test entity picker:
   - Select "Attach Entity to CBU" template
   - Click in the Entity field → type "test"
   - Should see typeahead results from database

## Success Criteria

- [ ] Template dropdown shows 6 templates grouped by domain
- [ ] Selecting template renders dynamic form
- [ ] Form validates required fields
- [ ] Render DSL button produces valid DSL
- [ ] Execute button runs DSL and shows results in log
- [ ] EntityPicker shows typeahead results
- [ ] Session state updates correctly
