import { api } from "./api";
import { FormRenderer } from "./formRenderer";
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
} from "./state";
import type { TemplateSummary } from "./types";
import "./style.css";

// ============================================================================
// DOM References
// ============================================================================

let formRenderer: FormRenderer | null = null;
let lastRenderedTemplateId: string | null = null;

function $(id: string): HTMLElement {
  const el = document.getElementById(id);
  if (!el) throw new Error(`Element not found: ${id}`);
  return el;
}

// ============================================================================
// Render Functions
// ============================================================================

function renderTemplateSelect(templates: TemplateSummary[]): void {
  const select = $("template-select") as HTMLSelectElement;
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
    const optgroup = document.createElement("optgroup");
    optgroup.label = domain.toUpperCase();

    for (const t of domainTemplates) {
      const option = document.createElement("option");
      option.value = t.id;
      option.textContent = t.name;
      optgroup.appendChild(option);
    }

    select.appendChild(optgroup);
  }
}

function renderSessionInfo(): void {
  const state = getState();
  const infoEl = $("session-info");
  const stateEl = $("session-state");

  if (state.sessionId) {
    infoEl.textContent = `Session: ${state.sessionId.substring(0, 8)}...`;
    infoEl.className = "session-info active";
  } else {
    infoEl.textContent = "No session";
    infoEl.className = "session-info";
  }

  if (state.sessionState) {
    stateEl.textContent = state.sessionState.replace(/_/g, " ");
    stateEl.className = `session-state ${state.sessionState}`;
    stateEl.style.display = "inline-block";
  } else {
    stateEl.style.display = "none";
  }
}

function renderForm(): void {
  const state = getState();
  const container = $("form-container");
  const renderBtn = $("render-btn") as HTMLButtonElement;

  // Only re-render if template changed (not on every formValue change)
  if (state.selectedTemplateId === lastRenderedTemplateId && formRenderer) {
    // Just update the button state, don't recreate form
    updateRenderButton();
    return;
  }

  // Template changed - cleanup and recreate
  lastRenderedTemplateId = state.selectedTemplateId;

  if (formRenderer) {
    formRenderer.destroy();
    formRenderer = null;
  }

  if (!state.selectedTemplate) {
    container.innerHTML =
      '<p class="placeholder-text">Select a template to begin</p>';
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
  const renderBtn = $("render-btn") as HTMLButtonElement;
  renderBtn.disabled = !formRenderer || !formRenderer.isValid();
}

function renderDslPreview(): void {
  const state = getState();
  const preview = $("dsl-preview");
  const executeBtn = $("execute-btn") as HTMLButtonElement;

  if (state.renderedDsl) {
    preview.textContent = state.renderedDsl;
    preview.classList.remove("placeholder-text");
    executeBtn.disabled = !state.sessionId;
  } else {
    preview.innerHTML =
      '<span class="placeholder-text">DSL will appear here</span>';
    executeBtn.disabled = true;
  }
}

function renderExecutionLog(): void {
  const state = getState();
  const logEl = $("execution-log");

  logEl.innerHTML = state.executionLog
    .map((entry) => {
      const isError = entry.includes("Error") || entry.includes("Failed");
      const isSuccess = entry.includes("Success") || entry.includes("Created");
      const cls = isError ? "error" : isSuccess ? "success" : "";
      return `<div class="log-entry ${cls}">${entry}</div>`;
    })
    .join("");

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
    const msg = err instanceof Error ? err.message : "Unknown error";
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
    const msg = err instanceof Error ? err.message : "Unknown error";
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
    const msg = err instanceof Error ? err.message : "Unknown error";
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
      const execResponse = await api.execute(state.sessionId, {
        dry_run: false,
      });

      if (execResponse.success) {
        for (const result of execResponse.results) {
          if (result.success) {
            addLogEntry(`Success: ${result.message}`);
          } else {
            addLogEntry(`Failed: ${result.message}`);
          }
        }
        addLogEntry(
          `Execution complete: ${execResponse.results.length} statement(s)`,
        );

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
      if (chatResponse.validation_results) {
        for (const v of chatResponse.validation_results) {
          if (!v.valid) {
            for (const e of v.errors) {
              addLogEntry(`Validation error: ${e.message}`);
            }
          }
        }
      }
    }
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Unknown error";
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
  $("new-session-btn").addEventListener("click", handleNewSession);

  ($("template-select") as HTMLSelectElement).addEventListener(
    "change",
    (e) => {
      const select = e.target as HTMLSelectElement;
      handleTemplateSelect(select.value);
    },
  );

  $("render-btn").addEventListener("click", handleRenderDsl);
  $("execute-btn").addEventListener("click", handleExecuteDsl);
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
    console.error("Failed to load templates:", err);
    addLogEntry("Error: Failed to load templates");
  }

  // Initial render
  render();
}

// Start
document.addEventListener("DOMContentLoaded", init);
