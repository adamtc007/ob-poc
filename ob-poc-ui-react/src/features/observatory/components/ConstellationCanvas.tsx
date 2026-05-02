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
let wasmLoading = false;

/** Load the WASM module via dynamic script injection (bypasses Vite bundler). */
async function loadWasmModule(): Promise<ObservatoryWasmModule> {
  if (wasmModule) return wasmModule;
  if (wasmLoading) {
    // Wait for the in-flight load
    return new Promise((resolve) => {
      const check = setInterval(() => {
        if (wasmModule) {
          clearInterval(check);
          resolve(wasmModule);
        }
      }, 50);
    });
  }

  wasmLoading = true;

  // Load the JS glue via a module script element so it registers on globalThis
  const script = document.createElement("script");
  script.type = "module";
  script.textContent = `
    import init, * as wasm from '/observatory/pkg/observatory_wasm.js';
    await init();
    window.__observatory_wasm = wasm;
    window.dispatchEvent(new Event('observatory-wasm-ready'));
  `;

  return new Promise((resolve, reject) => {
    const onReady = () => {
      if (!window.__observatory_wasm) {
        wasmLoading = false;
        window.removeEventListener("observatory-wasm-ready", onReady);
        reject(new Error("WASM ready event fired without module"));
        return;
      }
      wasmModule = window.__observatory_wasm;
      wasmLoading = false;
      window.removeEventListener("observatory-wasm-ready", onReady);
      resolve(wasmModule);
    };
    window.addEventListener("observatory-wasm-ready", onReady);

    script.onerror = (e) => {
      wasmLoading = false;
      reject(new Error(`Failed to load WASM: ${e}`));
    };

    document.head.appendChild(script);

    // Timeout after 15s
    setTimeout(() => {
      if (!wasmModule) {
        wasmLoading = false;
        reject(new Error("WASM load timeout"));
      }
    }, 15000);
  });
}

export function ConstellationCanvas({
  graphScene,
  viewLevel,
  onAction,
}: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const actionCallbackRef = useRef(onAction);
  const [ready, setReady] = useState(wasmReady);

  useEffect(() => {
    actionCallbackRef.current = onAction;
  }, [onAction]);

  // Initialize WASM canvas
  useEffect(() => {
    let cancelled = false;

    async function init() {
      if (wasmReady) {
        setReady(true);
        return;
      }
      try {
        const wasm = await loadWasmModule();
        if (cancelled) return;

        await wasm.start_canvas("observatory_canvas");
        wasm.on_action((json: string) => {
          try {
            const action = JSON.parse(json) as ObservatoryAction;
            actionCallbackRef.current(action);
          } catch (e) {
            console.error("Failed to parse canvas action:", e);
          }
        });
        wasmReady = true;
        setReady(true);
      } catch (e) {
        console.error("Failed to init observatory WASM:", e);
      }
    }

    init();
    return () => {
      cancelled = true;
    };
  }, []);

  // Push scene to WASM when it changes OR when WASM becomes ready
  useEffect(() => {
    if (ready && wasmModule && graphScene) {
      wasmModule.set_scene(JSON.stringify(graphScene));
    }
  }, [graphScene, ready]);

  // Push view level to WASM when orientation changes OR when WASM becomes ready
  useEffect(() => {
    if (ready && wasmModule) {
      wasmModule.set_view_level(viewLevel);
    }
  }, [viewLevel, ready]);

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
