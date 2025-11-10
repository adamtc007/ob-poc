// Phase 6: DSL/AST Visualization - Main Application Class
// TypeScript application class that manages the entire DSL/AST visualizer

import {
  AppState,
  AppEvent,
  DslManagerApi,
  UiConfig,
  DslDomain,
  DslRequest,
  DEFAULT_APP_STATE
} from './types';

/**
 * Main application class for the DSL/AST Visualizer
 * Manages state, UI components, and API interactions
 */
export class DslVisualizerApp {
  private state: AppState;
  private apiClient: DslManagerApi;
  private uiConfig: UiConfig;
  private mountElement: HTMLElement | null = null;
  private eventListeners: Map<string, EventListener> = new Map();

  constructor(apiClient: DslManagerApi, uiConfig: UiConfig) {
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
    await this.loadDomains();
  }

  /**
   * Handle application events and update state
   */
  private handleEvent(event: AppEvent): void {
    console.log('🔄 Handling event:', event.type, event.payload);

    switch (event.type) {
      case 'LOAD_DOMAINS':
        this.state = {
          ...this.state,
          domains: event.payload,
          loading: false,
          error: null
        };
        break;

      case 'SELECT_DOMAIN':
        this.state = {
          ...this.state,
          selectedDomain: event.payload,
          selectedVersion: null,
          dslSource: '',
          astText: '',
          error: null
        };
        this.updateVersionSelector();
        break;

      case 'SELECT_VERSION':
        this.state = {
          ...this.state,
          selectedVersion: event.payload,
          error: null
        };
        if (this.state.selectedDomain) {
          this.loadDslContent();
        }
        break;

      case 'LOAD_DSL_START':
        this.state = {
          ...this.state,
          loading: true,
          error: null
        };
        break;

      case 'LOAD_DSL_SUCCESS':
        this.state = {
          ...this.state,
          dslSource: event.payload.dslSource,
          astText: event.payload.astText,
          loading: false,
          error: null,
          lastUpdated: new Date()
        };
        break;

      case 'LOAD_DSL_ERROR':
        this.state = {
          ...this.state,
          loading: false,
          error: event.payload
        };
        break;

      case 'CLEAR_ERROR':
        this.state = {
          ...this.state,
          error: null
        };
        break;

      case 'UPDATE_UI_CONFIG':
        this.uiConfig = { ...this.uiConfig, ...event.payload };
        this.saveUiConfig();
        break;
    }

    this.render();
  }

  /**
   * Load available domains from the API
   */
  private async loadDomains(): Promise<void> {
    try {
      this.handleEvent({ type: 'LOAD_DSL_START' });
      const domains = await this.apiClient.getDomains();
      this.handleEvent({ type: 'LOAD_DOMAINS', payload: domains });

      // Auto-select first domain and version if available
      if (domains.length > 0 && domains[0].versions.length > 0) {
        this.handleEvent({ type: 'SELECT_DOMAIN', payload: domains[0].name });
        this.handleEvent({ type: 'SELECT_VERSION', payload: domains[0].versions[0].version });
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to load domains';
      this.handleEvent({ type: 'LOAD_DSL_ERROR', payload: message });
    }
  }

  /**
   * Load DSL content for selected domain and version
   */
  private async loadDslContent(): Promise<void> {
    if (!this.state.selectedDomain || !this.state.selectedVersion) {
      return;
    }

    try {
      this.handleEvent({ type: 'LOAD_DSL_START' });

      const request: DslRequest = {
        domain: this.state.selectedDomain,
        version: this.state.selectedVersion
      };

      const response = await this.apiClient.getDslAndAst(request);
      this.handleEvent({ type: 'LOAD_DSL_SUCCESS', payload: response });
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to load DSL content';
      this.handleEvent({ type: 'LOAD_DSL_ERROR', payload: message });
    }
  }

  /**
   * Update version selector based on selected domain
   */
  private updateVersionSelector(): void {
    const versionSelect = document.getElementById('version-select') as HTMLSelectElement;
    if (!versionSelect || !this.state.selectedDomain) return;

    const domain = this.state.domains.find(d => d.name === this.state.selectedDomain);
    if (!domain) return;

    versionSelect.innerHTML = '';

    if (domain.versions.length === 0) {
      const option = document.createElement('option');
      option.value = '';
      option.textContent = 'No versions available';
      versionSelect.appendChild(option);
    } else {
      domain.versions.forEach(version => {
        const option = document.createElement('option');
        option.value = version.version;
        option.textContent = `${version.version} (${version.exampleCount} examples)`;
        versionSelect.appendChild(option);
      });

      // Auto-select first version
      if (domain.versions.length > 0) {
        versionSelect.value = domain.versions[0].version;
        this.handleEvent({ type: 'SELECT_VERSION', payload: domain.versions[0].version });
      }
    }
  }

  /**
   * Set up event listeners for UI interactions
   */
  private setupEventListeners(): void {
    // Domain selector change
    const domainChangeHandler = (event: Event) => {
      const select = event.target as HTMLSelectElement;
      this.handleEvent({ type: 'SELECT_DOMAIN', payload: select.value });
    };
    this.eventListeners.set('domain-change', domainChangeHandler);

    // Version selector change
    const versionChangeHandler = (event: Event) => {
      const select = event.target as HTMLSelectElement;
      this.handleEvent({ type: 'SELECT_VERSION', payload: select.value });
    };
    this.eventListeners.set('version-change', versionChangeHandler);

    // Refresh button click
    const refreshHandler = () => {
      if (this.state.selectedDomain && this.state.selectedVersion) {
        this.loadDslContent();
      }
    };
    this.eventListeners.set('refresh', refreshHandler);

    // Error dismiss
    const errorDismissHandler = () => {
      this.handleEvent({ type: 'CLEAR_ERROR' });
    };
    this.eventListeners.set('error-dismiss', errorDismissHandler);

    // Font size change
    const fontSizeHandler = (event: Event) => {
      const input = event.target as HTMLInputElement;
      const fontSize = parseInt(input.value);
      this.handleEvent({ type: 'UPDATE_UI_CONFIG', payload: { fontSize } });
      this.updateEditorFontSize(fontSize);
    };
    this.eventListeners.set('font-size', fontSizeHandler);
  }

  /**
   * Render the application UI
   */
  private render(): void {
    if (!this.mountElement) return;

    this.mountElement.innerHTML = this.getTemplate();
    this.attachEventListeners();
    this.updateEditorContent();
    this.updateStatus();
  }

  /**
   * Get the main application template
   */
  private getTemplate(): string {
    return `
      <div class="app-container">
        <header class="app-header">
          <h1>🚀 Phase 6: DSL/AST Visualization</h1>
          <div class="controls">
            <div class="control-group">
              <label for="domain-select">Domain:</label>
              <select id="domain-select">
                ${this.renderDomainOptions()}
              </select>
            </div>
            <div class="control-group">
              <label for="version-select">Version:</label>
              <select id="version-select">
                <option value="">Select domain first</option>
              </select>
            </div>
            <button id="refresh-btn" ${this.state.loading ? 'disabled' : ''}>
              🔄 Refresh
            </button>
            <div class="control-group">
              <label for="font-size">Font Size:</label>
              <input id="font-size" type="range" min="10" max="24" value="${this.uiConfig.fontSize}">
              <span>${this.uiConfig.fontSize}px</span>
            </div>
          </div>
        </header>

        ${this.state.error ? this.renderErrorBanner() : ''}

        <main class="main-content">
          <div class="panel dsl-panel">
            <div class="panel-header">📄 DSL Source Code</div>
            <div class="panel-content">
              <textarea
                id="dsl-editor"
                class="code-editor"
                readonly
                placeholder="Select a domain and version to view DSL source code..."
                style="font-size: ${this.uiConfig.fontSize}px;"
              ></textarea>
            </div>
          </div>

          <div class="panel-divider"></div>

          <div class="panel ast-panel">
            <div class="panel-header">🌳 Generated AST</div>
            <div class="panel-content">
              <textarea
                id="ast-editor"
                class="code-editor"
                readonly
                placeholder="Generated AST will appear here when DSL is loaded..."
                style="font-size: ${this.uiConfig.fontSize}px;"
              ></textarea>
            </div>
          </div>
        </main>

        <footer class="status-bar">
          <div class="status-left">
            <span id="status-text">Ready</span>
          </div>
          <div class="status-right">
            Phase 6 Web Client v1.0 - TypeScript Edition
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
          background-color: #f5f5f5;
        }

        .app-container {
          height: 100vh;
          display: flex;
          flex-direction: column;
        }

        .app-header {
          background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
          color: white;
          padding: 1rem 2rem;
          box-shadow: 0 2px 10px rgba(0,0,0,0.1);
        }

        .app-header h1 {
          font-size: 1.5rem;
          margin-bottom: 1rem;
        }

        .controls {
          display: flex;
          gap: 1rem;
          align-items: center;
          flex-wrap: wrap;
        }

        .control-group {
          display: flex;
          flex-direction: column;
          gap: 0.25rem;
        }

        .control-group label {
          font-size: 0.875rem;
          font-weight: 500;
        }

        select, button, input {
          padding: 0.5rem;
          border: none;
          border-radius: 4px;
          background: rgba(255,255,255,0.2);
          color: white;
          font-size: 0.875rem;
        }

        select option {
          color: black;
        }

        button {
          background: rgba(255,255,255,0.3);
          cursor: pointer;
          transition: background-color 0.2s;
        }

        button:hover:not(:disabled) {
          background: rgba(255,255,255,0.4);
        }

        button:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }

        input[type="range"] {
          width: 80px;
        }

        .error-banner {
          background: #dc3545;
          color: white;
          padding: 1rem 2rem;
          display: flex;
          align-items: center;
          justify-content: space-between;
        }

        .error-banner button {
          background: rgba(255,255,255,0.2);
          border: none;
          color: white;
          padding: 0.25rem 0.5rem;
          border-radius: 4px;
          cursor: pointer;
        }

        .main-content {
          flex: 1;
          display: flex;
          gap: 0;
          min-height: 0;
        }

        .panel {
          flex: 1;
          background: white;
          display: flex;
          flex-direction: column;
          overflow: hidden;
        }

        .dsl-panel {
          border-right: 1px solid #e9ecef;
        }

        .panel-header {
          background: #f8f9fa;
          padding: 1rem;
          border-bottom: 1px solid #e9ecef;
          font-weight: 600;
          font-size: 1.1rem;
        }

        .panel-content {
          flex: 1;
          overflow: hidden;
        }

        .panel-divider {
          width: 4px;
          background: #dee2e6;
          cursor: col-resize;
          user-select: none;
        }

        .panel-divider:hover {
          background: #6c757d;
        }

        .code-editor {
          width: 100%;
          height: 100%;
          border: none;
          padding: 1rem;
          font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
          line-height: 1.5;
          resize: none;
          outline: none;
          background: #fafafa;
          color: #2d3436;
        }

        .dsl-panel .code-editor {
          background: #f8f9fa;
        }

        .ast-panel .code-editor {
          background: #fff5f5;
        }

        .status-bar {
          background: #343a40;
          color: white;
          padding: 0.5rem 2rem;
          font-size: 0.875rem;
          display: flex;
          justify-content: space-between;
          align-items: center;
        }

        .loading {
          color: #ffc107;
        }

        .success {
          color: #28a745;
        }

        .error {
          color: #dc3545;
        }

        @media (max-width: 768px) {
          .main-content {
            flex-direction: column;
          }

          .panel-divider {
            display: none;
          }

          .controls {
            flex-direction: column;
            align-items: stretch;
            gap: 0.5rem;
          }

          .control-group {
            flex-direction: row;
            justify-content: space-between;
            align-items: center;
          }
        }
      </style>
    `;
  }

  /**
   * Render domain selector options
   */
  private renderDomainOptions(): string {
    if (this.state.domains.length === 0) {
      return '<option value="">Loading domains...</option>';
    }

    return this.state.domains
      .map(domain =>
        `<option value="${domain.name}" ${domain.name === this.state.selectedDomain ? 'selected' : ''}>
          ${domain.name}
        </option>`
      )
      .join('');
  }

  /**
   * Render error banner
   */
  private renderErrorBanner(): string {
    return `
      <div class="error-banner">
        <span>❌ ${this.state.error}</span>
        <button id="dismiss-error">✕</button>
      </div>
    `;
  }

  /**
   * Attach event listeners to rendered elements
   */
  private attachEventListeners(): void {
    const domainSelect = document.getElementById('domain-select');
    if (domainSelect) {
      domainSelect.addEventListener('change', this.eventListeners.get('domain-change')!);
    }

    const versionSelect = document.getElementById('version-select');
    if (versionSelect) {
      versionSelect.addEventListener('change', this.eventListeners.get('version-change')!);
    }

    const refreshBtn = document.getElementById('refresh-btn');
    if (refreshBtn) {
      refreshBtn.addEventListener('click', this.eventListeners.get('refresh')!);
    }

    const dismissError = document.getElementById('dismiss-error');
    if (dismissError) {
      dismissError.addEventListener('click', this.eventListeners.get('error-dismiss')!);
    }

    const fontSizeInput = document.getElementById('font-size');
    if (fontSizeInput) {
      fontSizeInput.addEventListener('input', this.eventListeners.get('font-size')!);
    }
  }

  /**
   * Update editor content
   */
  private updateEditorContent(): void {
    const dslEditor = document.getElementById('dsl-editor') as HTMLTextAreaElement;
    const astEditor = document.getElementById('ast-editor') as HTMLTextAreaElement;

    if (dslEditor) {
      dslEditor.value = this.state.dslSource;
    }

    if (astEditor) {
      astEditor.value = this.state.astText;
    }
  }

  /**
   * Update status bar
   */
  private updateStatus(): void {
    const statusText = document.getElementById('status-text');
    if (!statusText) return;

    if (this.state.loading) {
      statusText.innerHTML = '<span class="loading">🔄 Loading...</span>';
    } else if (this.state.error) {
      statusText.innerHTML = '<span class="error">❌ Error occurred</span>';
    } else if (this.state.lastUpdated) {
      const time = this.state.lastUpdated.toLocaleTimeString();
      statusText.innerHTML = `<span class="success">✅ Updated at ${time}</span>`;
    } else {
      statusText.textContent = 'Ready - Select a domain to begin';
    }
  }

  /**
   * Update editor font size
   */
  private updateEditorFontSize(fontSize: number): void {
    const editors = document.querySelectorAll('.code-editor') as NodeListOf<HTMLTextAreaElement>;
    editors.forEach(editor => {
      editor.style.fontSize = `${fontSize}px`;
    });
  }

  /**
   * Save UI configuration to localStorage
   */
  private saveUiConfig(): void {
    localStorage.setItem('dsl-visualizer-config', JSON.stringify(this.uiConfig));
  }

  /**
   * Handle global application errors
   */
  handleGlobalError(error: Error | string): void {
    const message = error instanceof Error ? error.message : error;
    this.handleEvent({ type: 'LOAD_DSL_ERROR', payload: message });
  }

  /**
   * Cleanup resources when app is destroyed
   */
  cleanup(): void {
    this.eventListeners.clear();
    if (this.apiClient.cancelRequests) {
      this.apiClient.cancelRequests();
    }
  }
}
