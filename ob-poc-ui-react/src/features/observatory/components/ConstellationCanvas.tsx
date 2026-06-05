/**
 * ConstellationCanvas — embeds the egui WASM canvas inside React.
 *
 * Loads the WASM module at runtime from /observatory/pkg/ (served by the
 * Rust server). Uses dynamic script injection to avoid Rollup/Vite trying
 * to resolve the import at build time.
 */

import { useEffect, useRef, useState } from "react";
import type {
  GraphSceneModel,
  ViewLevel,
  ObservatoryAction,
} from "../../../types/observatory";

interface Props {
  graphScene: GraphSceneModel | null;
  viewLevel: ViewLevel;
  onAction: (action: ObservatoryAction) => void;
}

interface ObservatoryWasmModule {
  on_action(callback: (json: string) => void): void;
  set_scene(sceneJson: string): void;
  set_view_level(viewLevel: ViewLevel): void;
  start_canvas(canvasId: string): Promise<void>;
}

declare global {
  interface Window {
    __observatory_wasm?: ObservatoryWasmModule;
  }
}

// Module-level WASM state (singleton — canvas is initialized once)
let wasmModule: ObservatoryWasmModule | null = null;
let wasmReady = false;
let wasmLoadPromise: Promise<ObservatoryWasmModule> | null = null;
let globalActionCallback: ((json: string) => void) | null = null;

/** Load the WASM module via dynamic script injection (bypasses Vite bundler). */
async function loadWasmModule(): Promise<ObservatoryWasmModule> {
  if (wasmModule) return wasmModule;
  if (wasmLoadPromise) return wasmLoadPromise;

  wasmLoadPromise = new Promise<ObservatoryWasmModule>((resolve, reject) => {
    const script = document.createElement("script");
    script.type = "module";
    script.textContent = `
      import init, * as wasm from '/observatory/pkg/observatory_wasm.js';
      await init();
      window.__observatory_wasm = wasm;
      window.dispatchEvent(new Event('observatory-wasm-ready'));
    `;

    const onReady = () => {
      if (!window.__observatory_wasm) {
        window.removeEventListener("observatory-wasm-ready", onReady);
        reject(new Error("WASM ready event fired without module"));
        wasmLoadPromise = null;
        return;
      }
      wasmModule = window.__observatory_wasm;
      window.removeEventListener("observatory-wasm-ready", onReady);
      resolve(wasmModule);
    };
    window.addEventListener("observatory-wasm-ready", onReady);

    script.onerror = (e) => {
      window.removeEventListener("observatory-wasm-ready", onReady);
      reject(new Error(`Failed to load WASM: ${e}`));
      wasmLoadPromise = null;
    };

    document.head.appendChild(script);

    setTimeout(() => {
      if (!wasmModule) {
        window.removeEventListener("observatory-wasm-ready", onReady);
        reject(new Error("WASM load timeout"));
        wasmLoadPromise = null;
      }
    }, 15000);
  });

  return wasmLoadPromise;
}

export function ConstellationCanvas({
  graphScene,
  viewLevel,
  onAction,
}: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [canvasReady, setCanvasReady] = useState(false);

  // Sync current onAction callback to the global callback handler
  useEffect(() => {
    const handler = (json: string) => {
      try {
        const action = JSON.parse(json) as ObservatoryAction;
        onAction(action);
      } catch (e) {
        console.error("Failed to parse canvas action:", e);
      }
    };
    globalActionCallback = handler;
    return () => {
      if (globalActionCallback === handler) {
        globalActionCallback = null;
      }
    };
  }, [onAction]);

  // Initialize WASM canvas
  useEffect(() => {
    let cancelled = false;

    async function init() {
      try {
        const wasm = await loadWasmModule();
        if (cancelled) return;

        // start_canvas must run for the newly mounted canvas element
        await wasm.start_canvas("observatory_canvas");
        if (cancelled) return;

        if (!wasmReady) {
          wasm.on_action((json: string) => {
            if (globalActionCallback) {
              globalActionCallback(json);
            }
          });
          wasmReady = true;
        }
        setCanvasReady(true);
      } catch (e) {
        console.error("Failed to init observatory WASM:", e);
      }
    }

    init();
    return () => {
      cancelled = true;
      setCanvasReady(false);
    };
  }, []);

  // Push scene to WASM when it changes OR when canvas becomes ready
  useEffect(() => {
    if (canvasReady && wasmModule && graphScene) {
      wasmModule.set_scene(JSON.stringify(graphScene));
    }
  }, [graphScene, canvasReady]);

  // Push view level to WASM when orientation changes OR when canvas becomes ready
  useEffect(() => {
    if (canvasReady && wasmModule) {
      wasmModule.set_view_level(viewLevel);
    }
  }, [viewLevel, canvasReady]);

  return (
    <canvas
      id="observatory_canvas"
      ref={canvasRef}
      className="w-full h-full"
      style={{ display: "block" }}
    />
  );
}

export default ConstellationCanvas;
