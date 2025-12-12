// WASM ↔ HTML Bridge
// Handles communication between the egui/WASM graph and HTML panels
export class WasmBridge {
    constructor(callbacks) {
        this.canvas = null;
        this.wasmReady = false;
        this.callbacks = callbacks;
        this.setupListeners();
    }
    setupListeners() {
        // Listen for entity selection from WASM
        window.addEventListener('egui-entity-selected', (e) => {
            const entityId = e.detail?.id;
            if (entityId) {
                this.callbacks.onEntitySelected(entityId);
            }
        });
        // Listen for CBU changes from WASM
        window.addEventListener('egui-cbu-changed', (e) => {
            const cbuId = e.detail?.id;
            if (cbuId) {
                this.callbacks.onCbuChanged(cbuId);
            }
        });
        // Listen for WASM ready signal
        window.addEventListener('egui-ready', () => {
            this.wasmReady = true;
            console.log('[Bridge] WASM graph ready');
        });
    }
    setCanvas(canvas) {
        this.canvas = canvas;
    }
    isReady() {
        return this.wasmReady;
    }
    // Called by HTML panels to focus an entity in the graph
    focusEntity(entityId) {
        window.dispatchEvent(new CustomEvent('focus-entity', {
            detail: { id: entityId },
        }));
    }
    // Called by HTML panels to load a different CBU
    loadCbu(cbuId) {
        console.log('[Bridge] loadCbu:', cbuId);
        window.dispatchEvent(new CustomEvent('load-cbu', {
            detail: { id: cbuId },
        }));
    }
    // Called by HTML panels to change view mode
    setViewMode(mode) {
        window.dispatchEvent(new CustomEvent('set-view-mode', {
            detail: { mode },
        }));
    }
}
