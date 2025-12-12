// OB-POC Hybrid UI - Main Application Entry Point
import { ChatPanel } from './chat.js';
import { DslPanel } from './dsl.js';
import { AstPanel } from './ast.js';
import { WasmBridge } from './bridge.js';
class App {
    constructor() {
        this.currentCbuId = null;
        // Initialize WASM bridge
        this.wasmBridge = new WasmBridge({
            onEntitySelected: (entityId) => this.handleEntitySelected(entityId),
            onCbuChanged: (cbuId) => this.handleCbuChanged(cbuId),
        });
        // Initialize panels
        this.dslPanel = new DslPanel();
        this.astPanel = new AstPanel({
            onNodeSelected: (nodeId) => this.handleAstNodeSelected(nodeId),
        });
        this.chatPanel = new ChatPanel({
            onDsl: (source) => this.dslPanel.setSource(source),
            onAst: (statements) => this.astPanel.setAst(statements),
            onCanExecute: (can) => this.handleCanExecuteChanged(can),
            onStatusChange: (status) => this.handleStatusChanged(status),
            onCommand: (cmd) => this.handleAgentCommand(cmd),
        });
        // Setup CBU selector
        this.cbuSelector = document.getElementById('cbu-selector');
        this.viewModeSelector = document.getElementById('view-mode');
        this.setupCbuSelector();
        this.setupViewModeSelector();
        console.log('[App] Hybrid UI initialized');
    }
    async setupCbuSelector() {
        try {
            const response = await fetch('/api/cbu');
            const cbus = await response.json();
            // Clear existing options (except placeholder)
            while (this.cbuSelector.options.length > 1) {
                this.cbuSelector.remove(1);
            }
            // Add CBU options
            for (const cbu of cbus) {
                const option = document.createElement('option');
                option.value = cbu.cbu_id;
                option.textContent = `${cbu.name}${cbu.jurisdiction ? ` (${cbu.jurisdiction})` : ''}`;
                this.cbuSelector.appendChild(option);
            }
            this.cbuSelector.addEventListener('change', () => {
                const cbuId = this.cbuSelector.value;
                if (cbuId) {
                    this.loadCbu(cbuId);
                }
            });
        }
        catch (error) {
            console.error('[App] Failed to load CBUs:', error);
        }
    }
    setupViewModeSelector() {
        this.viewModeSelector.addEventListener('change', () => {
            const mode = this.viewModeSelector.value;
            this.wasmBridge.setViewMode(mode);
        });
    }
    async loadCbu(cbuId) {
        this.currentCbuId = cbuId;
        this.chatPanel.setCbuId(cbuId);
        // Tell WASM to load the CBU graph
        this.wasmBridge.loadCbu(cbuId);
        // Also load DSL for this CBU if any exists
        try {
            const response = await fetch(`/api/cbu/${cbuId}/dsl`);
            const data = await response.json();
            if (data.source) {
                this.dslPanel.setSource(data.source);
            }
            // Load AST
            const astResponse = await fetch(`/api/cbu/${cbuId}/ast`);
            const astData = await astResponse.json();
            if (astData.statements) {
                this.astPanel.setAst(astData.statements);
            }
        }
        catch (error) {
            console.error('[App] Failed to load CBU data:', error);
        }
    }
    handleEntitySelected(entityId) {
        console.log('[App] Entity selected in graph:', entityId);
        // Could highlight related DSL/AST nodes here
    }
    handleCbuChanged(cbuId) {
        console.log('[App] CBU changed in graph:', cbuId);
        this.currentCbuId = cbuId;
        this.cbuSelector.value = cbuId;
        this.chatPanel.setCbuId(cbuId);
    }
    handleAstNodeSelected(nodeId) {
        console.log('[App] AST node selected:', nodeId);
        // Could focus related entity in graph or highlight DSL line
    }
    handleCanExecuteChanged(can) {
        if (can) {
            console.log('[App] DSL ready to execute');
        }
    }
    handleAgentCommand(cmd) {
        console.log('[App] Agent command:', cmd);
        switch (cmd.action) {
            case 'show_cbu':
                // Load CBU in graph and update selector
                this.loadCbu(cmd.cbu_id);
                this.cbuSelector.value = cmd.cbu_id;
                break;
            case 'highlight_entity':
                this.wasmBridge.focusEntity(cmd.entity_id);
                break;
            case 'navigate_dsl':
                // Could scroll DSL panel to line
                console.log('[App] Navigate to DSL line:', cmd.line);
                break;
            case 'focus_ast':
                // Could expand/highlight AST node
                console.log('[App] Focus AST node:', cmd.node_id);
                break;
        }
    }
    handleStatusChanged(status) {
        console.log('[App] Session status:', status);
        if (status === 'executed') {
            this.dslPanel.markExecuted();
            // Refresh CBU list in case new one was created
            this.setupCbuSelector();
        }
        else if (status === 'error') {
            this.dslPanel.markError();
        }
    }
}
// Initialize app when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
    new App();
});
