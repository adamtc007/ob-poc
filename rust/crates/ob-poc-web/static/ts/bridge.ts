// WASM ↔ HTML Bridge
// Handles communication between the egui/WASM graph and HTML panels

export type BridgeCallback = {
  onEntitySelected: (entityId: string) => void;
  onCbuChanged: (cbuId: string) => void;
};

export class WasmBridge {
  private canvas: HTMLCanvasElement | null = null;
  private callbacks: BridgeCallback;
  private wasmReady: boolean = false;

  constructor(callbacks: BridgeCallback) {
    this.callbacks = callbacks;
    this.setupListeners();
  }

  private setupListeners() {
    // Listen for entity selection from WASM
    window.addEventListener('egui-entity-selected', (e: Event) => {
      const entityId = (e as CustomEvent).detail?.id;
      if (entityId) {
        this.callbacks.onEntitySelected(entityId);
      }
    });

    // Listen for CBU changes from WASM
    window.addEventListener('egui-cbu-changed', (e: Event) => {
      const cbuId = (e as CustomEvent).detail?.id;
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

  setCanvas(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
  }

  isReady(): boolean {
    return this.wasmReady;
  }

  // Called by HTML panels to focus an entity in the graph
  focusEntity(entityId: string) {
    window.dispatchEvent(
      new CustomEvent('focus-entity', {
        detail: { id: entityId },
      }),
    );
  }

  // Called by HTML panels to load a different CBU
  loadCbu(cbuId: string) {
    console.log('[Bridge] loadCbu:', cbuId);
    window.dispatchEvent(
      new CustomEvent('load-cbu', {
        detail: { id: cbuId },
      }),
    );
  }

  // Called by HTML panels to change view mode
  setViewMode(mode: 'KYC_UBO' | 'SERVICE_DELIVERY' | 'CUSTODY') {
    window.dispatchEvent(
      new CustomEvent('set-view-mode', {
        detail: { mode },
      }),
    );
  }
}
