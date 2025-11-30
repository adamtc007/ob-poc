// DSL Viewer - Main Application Class
// TypeScript application class for visualizing persisted agent-generated DSL

import {
  AppState,
  AppEvent,
  DslViewerApi,
  UiConfig,
  ExecutionStepInfo,
  DEFAULT_APP_STATE,
} from "./types";

/**
 * Main application class for the DSL Viewer
 * Manages state, UI components, and API interactions
 */
export class DslVisualizerApp {
  private state: AppState;
  private apiClient: DslViewerApi;
  private uiConfig: UiConfig;
  private mountElement: HTMLElement | null = null;
  private eventListeners: Map<string, EventListener> = new Map();

  constructor(apiClient: DslViewerApi, uiConfig: UiConfig) {
    this.apiClient = apiClient;
    this.uiConfig = uiConfig;
    this.state = { ...DEFAULT_APP_STATE };
  }

  /**
   * Mount the application to a DOM element
   */
  async mount(element: HTMLElement): Promise<void> {
    this.mountElement = element;
    this.setupEventListeners();
    this.render();

    // Load initial data
    await this.loadInstances();
  }

  /**
   * Handle application events and update state
   */
  private handleEvent(event: AppEvent): void {
    console.log("Handling event:", event.type);

    switch (event.type) {
      case "LOAD_INSTANCES":
        this.state = {
          ...this.state,
          instances: event.payload,
          loading: false,
          error: null,
        };
        break;

      case "SELECT_INSTANCE":
        this.state = {
          ...this.state,
          selectedInstance: event.payload,
          selectedVersion: null,
          displayData: null,
          versionHistory: [],
          error: null,
        };
        this.loadHistory(event.payload);
        break;

      case "SELECT_VERSION":
        this.state = {
          ...this.state,
          selectedVersion: event.payload,
          error: null,
        };
        if (this.state.selectedInstance) {
          this.loadDsl(this.state.selectedInstance, event.payload);
        }
        break;

      case "LOAD_DSL_START":
        this.state = {
          ...this.state,
          loading: true,
          error: null,
        };
        break;

      case "LOAD_DSL_SUCCESS":
        this.state = {
          ...this.state,
          displayData: event.payload,
          loading: false,
          error: null,
        };
        break;

      case "LOAD_HISTORY_SUCCESS":
        this.state = {
          ...this.state,
          versionHistory: event.payload,
          loading: false,
          error: null,
        };
        // Auto-select latest version
        if (event.payload.length > 0) {
          const latestVersion = Math.max(
            ...event.payload.map((v) => v.version),
          );
          this.handleEvent({ type: "SELECT_VERSION", payload: latestVersion });
        }
        break;

      case "LOAD_DSL_ERROR":
        this.state = {
          ...this.state,
          loading: false,
          error: event.payload,
        };
        break;

      case "CLEAR_ERROR":
        this.state = {
          ...this.state,
          error: null,
        };
        break;

      case "REFRESH":
        this.loadInstances();
        break;

      case "UPDATE_UI_CONFIG":
        this.uiConfig = { ...this.uiConfig, ...event.payload };
        this.saveUiConfig();
        break;
    }

    this.render();
  }

  /**
   * Load available DSL instances from the API
   */
  private async loadInstances(): Promise<void> {
    try {
      this.handleEvent({ type: "LOAD_DSL_START" });
      const instances = await this.apiClient.listInstances();
      this.handleEvent({ type: "LOAD_INSTANCES", payload: instances });

      // Auto-select first instance if available
      if (instances.length > 0) {
        this.handleEvent({
          type: "SELECT_INSTANCE",
          payload: instances[0].businessReference,
        });
      }
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Failed to load instances";
      this.handleEvent({ type: "LOAD_DSL_ERROR", payload: message });
    }
  }

  /**
   * Load version history for selected instance
   */
  private async loadHistory(businessRef: string): Promise<void> {
    try {
      this.handleEvent({ type: "LOAD_DSL_START" });
      const history = await this.apiClient.getHistory(businessRef);
      this.handleEvent({ type: "LOAD_HISTORY_SUCCESS", payload: history });
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Failed to load history";
      this.handleEvent({ type: "LOAD_DSL_ERROR", payload: message });
    }
  }

  /**
   * Load DSL content for selected instance and version
   */
  private async loadDsl(businessRef: string, version: number): Promise<void> {
    try {
      this.handleEvent({ type: "LOAD_DSL_START" });
      const displayData = await this.apiClient.showDsl(businessRef, version);
      this.handleEvent({ type: "LOAD_DSL_SUCCESS", payload: displayData });
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Failed to load DSL";
      this.handleEvent({ type: "LOAD_DSL_ERROR", payload: message });
    }
  }

  /**
   * Set up event listeners for UI interactions
   */
  private setupEventListeners(): void {
    // Instance selector change
    const instanceChangeHandler = (event: Event) => {
      const select = event.target as HTMLSelectElement;
      this.handleEvent({ type: "SELECT_INSTANCE", payload: select.value });
    };
    this.eventListeners.set("instance-change", instanceChangeHandler);

    // Version selector change
    const versionChangeHandler = (event: Event) => {
      const select = event.target as HTMLSelectElement;
      this.handleEvent({
        type: "SELECT_VERSION",
        payload: parseInt(select.value),
      });
    };
    this.eventListeners.set("version-change", versionChangeHandler);

    // Refresh button click
    const refreshHandler = () => {
      this.handleEvent({ type: "REFRESH" });
    };
    this.eventListeners.set("refresh", refreshHandler);

    // Error dismiss
    const errorDismissHandler = () => {
      this.handleEvent({ type: "CLEAR_ERROR" });
    };
    this.eventListeners.set("error-dismiss", errorDismissHandler);

    // Font size change
    const fontSizeHandler = (event: Event) => {
      const input = event.target as HTMLInputElement;
      const fontSize = parseInt(input.value);
      this.handleEvent({ type: "UPDATE_UI_CONFIG", payload: { fontSize } });
    };
    this.eventListeners.set("font-size", fontSizeHandler);
  }

  /**
   * Render the application UI
   */
  private render(): void {
    if (!this.mountElement) return;

    this.mountElement.innerHTML = this.getTemplate();
    this.attachEventListeners();
  }

  /**
   * Get the main application template
   */
  private getTemplate(): string {
    return `
      <div class="app-container">
        <header class="app-header">
          <h1>DSL Viewer</h1>
          <div class="controls">
            <div class="control-group">
              <label for="instance-select">Onboarding:</label>
              <select id="instance-select">
                ${this.renderInstanceOptions()}
              </select>
            </div>
            <div class="control-group">
              <label for="version-select">Version:</label>
              <select id="version-select">
                ${this.renderVersionOptions()}
              </select>
            </div>
            <button id="refresh-btn" ${this.state.loading ? "disabled" : ""}>
              Refresh
            </button>
          </div>
        </header>

        ${this.state.error ? this.renderErrorBanner() : ""}

        <main class="main-content">
          <div class="panel dsl-panel">
            <div class="panel-header">DSL Source</div>
            <div class="panel-content">
              <pre class="code-display" style="font-size: ${this.uiConfig.fontSize}px;">${this.escapeHtml(this.state.displayData?.dslSource || "Select an onboarding instance to view DSL...")}</pre>
            </div>
          </div>

          <div class="panel-divider"></div>

          <div class="panel plan-panel">
            <div class="panel-header">Execution Plan</div>
            <div class="panel-content">
              ${this.renderExecutionPlan()}
            </div>
          </div>
        </main>

        <footer class="status-bar">
          <div class="status-left">
            ${this.renderStatusText()}
          </div>
          <div class="status-right">
            <label>Font: </label>
            <input id="font-size" type="range" min="10" max="20" value="${this.uiConfig.fontSize}">
            <span>${this.uiConfig.fontSize}px</span>
          </div>
        </footer>
      </div>

      <style>
        * {
          margin: 0;
          padding: 0;
          box-sizing: border-box;
        }

        body, html {
          height: 100%;
          font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
          background-color: #1a1a2e;
          color: #eee;
        }

        .app-container {
          height: 100vh;
          display: flex;
          flex-direction: column;
        }

        .app-header {
          background: #16213e;
          padding: 1rem 2rem;
          border-bottom: 1px solid #0f3460;
          display: flex;
          justify-content: space-between;
          align-items: center;
        }

        .app-header h1 {
          font-size: 1.5rem;
          color: #e94560;
        }

        .controls {
          display: flex;
          gap: 1.5rem;
          align-items: center;
        }

        .control-group {
          display: flex;
          align-items: center;
          gap: 0.5rem;
        }

        .control-group label {
          font-size: 0.875rem;
          color: #aaa;
        }

        select, button {
          padding: 0.5rem 1rem;
          border: 1px solid #0f3460;
          border-radius: 4px;
          background: #1a1a2e;
          color: #eee;
          font-size: 0.875rem;
        }

        select {
          min-width: 200px;
        }

        button {
          background: #0f3460;
          cursor: pointer;
          transition: background-color 0.2s;
        }

        button:hover:not(:disabled) {
          background: #e94560;
        }

        button:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }

        .error-banner {
          background: #e94560;
          color: white;
          padding: 0.75rem 2rem;
          display: flex;
          align-items: center;
          justify-content: space-between;
        }

        .error-banner button {
          background: transparent;
          border: 1px solid white;
          color: white;
          padding: 0.25rem 0.5rem;
        }

        .main-content {
          flex: 1;
          display: flex;
          min-height: 0;
        }

        .panel {
          flex: 1;
          display: flex;
          flex-direction: column;
          overflow: hidden;
        }

        .panel-header {
          background: #0f3460;
          padding: 0.75rem 1rem;
          font-weight: 600;
          font-size: 0.9rem;
          text-transform: uppercase;
          letter-spacing: 0.5px;
          color: #e94560;
        }

        .panel-content {
          flex: 1;
          overflow: auto;
          background: #16213e;
        }

        .panel-divider {
          width: 4px;
          background: #0f3460;
        }

        .code-display {
          margin: 0;
          padding: 1rem;
          font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
          line-height: 1.6;
          color: #b8c5d9;
          white-space: pre-wrap;
          word-break: break-word;
        }

        .execution-plan {
          padding: 1rem;
        }

        .plan-step {
          background: #1a1a2e;
          border: 1px solid #0f3460;
          border-radius: 6px;
          padding: 0.75rem 1rem;
          margin-bottom: 0.75rem;
        }

        .plan-step:hover {
          border-color: #e94560;
        }

        .step-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 0.5rem;
        }

        .step-number {
          background: #e94560;
          color: white;
          padding: 0.25rem 0.5rem;
          border-radius: 4px;
          font-size: 0.75rem;
          font-weight: bold;
        }

        .step-verb {
          font-family: 'Monaco', 'Menlo', monospace;
          font-size: 0.95rem;
          color: #4db8ff;
        }

        .step-binding {
          color: #ffc107;
          font-family: 'Monaco', 'Menlo', monospace;
          font-size: 0.85rem;
        }

        .step-injections {
          margin-top: 0.5rem;
          padding-left: 1rem;
        }

        .injection {
          color: #888;
          font-family: 'Monaco', 'Menlo', monospace;
          font-size: 0.8rem;
          margin: 0.25rem 0;
        }

        .injection::before {
          content: "\\2190  ";
          color: #e94560;
        }

        .empty-plan {
          color: #666;
          text-align: center;
          padding: 2rem;
        }

        .status-bar {
          background: #0f3460;
          padding: 0.5rem 2rem;
          font-size: 0.8rem;
          display: flex;
          justify-content: space-between;
          align-items: center;
          border-top: 1px solid #16213e;
        }

        .status-left {
          color: #aaa;
        }

        .status-right {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          color: #888;
        }

        .status-right input[type="range"] {
          width: 80px;
          background: transparent;
        }

        .loading {
          color: #ffc107;
        }

        .success {
          color: #28a745;
        }

        @media (max-width: 900px) {
          .main-content {
            flex-direction: column;
          }

          .panel-divider {
            height: 4px;
            width: 100%;
          }

          .controls {
            flex-wrap: wrap;
            gap: 0.75rem;
          }
        }
      </style>
    `;
  }

  /**
   * Render instance selector options
   */
  private renderInstanceOptions(): string {
    if (this.state.instances.length === 0) {
      return '<option value="">Loading instances...</option>';
    }

    return this.state.instances
      .map(
        (inst) =>
          `<option value="${inst.businessReference}" ${inst.businessReference === this.state.selectedInstance ? "selected" : ""}>
          ${inst.businessReference} (v${inst.currentVersion})
        </option>`,
      )
      .join("");
  }

  /**
   * Render version selector options
   */
  private renderVersionOptions(): string {
    if (this.state.versionHistory.length === 0) {
      return '<option value="">Select instance first</option>';
    }

    return this.state.versionHistory
      .map(
        (v) =>
          `<option value="${v.version}" ${v.version === this.state.selectedVersion ? "selected" : ""}>
          v${v.version} - ${v.operationType}
        </option>`,
      )
      .join("");
  }

  /**
   * Render execution plan panel
   */
  private renderExecutionPlan(): string {
    const plan = this.state.displayData?.executionPlan;

    if (!plan || plan.length === 0) {
      return '<div class="empty-plan">No execution plan available</div>';
    }

    return `
      <div class="execution-plan">
        ${plan.map((step) => this.renderPlanStep(step)).join("")}
      </div>
    `;
  }

  /**
   * Render a single execution plan step
   */
  private renderPlanStep(step: ExecutionStepInfo): string {
    const bindingHtml = step.bindAs
      ? `<span class="step-binding">${step.bindAs}</span>`
      : "";

    const injectionsHtml =
      step.injections.length > 0
        ? `<div class="step-injections">
          ${step.injections.map((inj) => `<div class="injection">${this.escapeHtml(inj)}</div>`).join("")}
        </div>`
        : "";

    return `
      <div class="plan-step">
        <div class="step-header">
          <span class="step-number">Step ${step.step}</span>
          <span class="step-verb">${this.escapeHtml(step.verb)}</span>
          ${bindingHtml}
        </div>
        ${injectionsHtml}
      </div>
    `;
  }

  /**
   * Render error banner
   */
  private renderErrorBanner(): string {
    return `
      <div class="error-banner">
        <span>${this.escapeHtml(this.state.error || "")}</span>
        <button id="dismiss-error">Dismiss</button>
      </div>
    `;
  }

  /**
   * Render status text
   */
  private renderStatusText(): string {
    if (this.state.loading) {
      return '<span class="loading">Loading...</span>';
    }

    const data = this.state.displayData;
    if (data) {
      const stepCount = data.executionPlan?.length || 0;
      return `<span class="success">Loaded v${data.version} | ${stepCount} steps | ${data.compilationStatus}</span>`;
    }

    return "Ready";
  }

  /**
   * Escape HTML to prevent XSS
   */
  private escapeHtml(text: string): string {
    const div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
  }

  /**
   * Attach event listeners to rendered elements
   */
  private attachEventListeners(): void {
    const instanceSelect = document.getElementById("instance-select");
    if (instanceSelect) {
      instanceSelect.addEventListener(
        "change",
        this.eventListeners.get("instance-change")!,
      );
    }

    const versionSelect = document.getElementById("version-select");
    if (versionSelect) {
      versionSelect.addEventListener(
        "change",
        this.eventListeners.get("version-change")!,
      );
    }

    const refreshBtn = document.getElementById("refresh-btn");
    if (refreshBtn) {
      refreshBtn.addEventListener("click", this.eventListeners.get("refresh")!);
    }

    const dismissError = document.getElementById("dismiss-error");
    if (dismissError) {
      dismissError.addEventListener(
        "click",
        this.eventListeners.get("error-dismiss")!,
      );
    }

    const fontSizeInput = document.getElementById("font-size");
    if (fontSizeInput) {
      fontSizeInput.addEventListener(
        "input",
        this.eventListeners.get("font-size")!,
      );
    }
  }

  /**
   * Save UI configuration to localStorage
   */
  private saveUiConfig(): void {
    localStorage.setItem("dsl-viewer-config", JSON.stringify(this.uiConfig));
  }

  /**
   * Handle global application errors
   */
  handleGlobalError(error: Error | string): void {
    const message = error instanceof Error ? error.message : error;
    this.handleEvent({ type: "LOAD_DSL_ERROR", payload: message });
  }

  /**
   * Cleanup resources when app is destroyed
   */
  cleanup(): void {
    this.eventListeners.clear();
  }
}
