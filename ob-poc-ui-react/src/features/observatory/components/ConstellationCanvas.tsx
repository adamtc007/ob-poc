/**
 * ConstellationCanvas — embeds the egui WASM canvas inside React.
 *
 * Calls start_canvas(), set_scene(), set_view_level(), on_action()
 * from the observatory-wasm package.
 */

import { useEffect, useRef } from "react";
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

// Module-level WASM state (singleton — canvas is initialized once)
let wasmModule: {
  default: () => Promise<void>;
  start_canvas: (id: string) => Promise<void>;
  set_scene: (json: string) => void;
  set_view_level: (level: string) => void;
  on_action: (callback: (json: string) => void) => void;
} | null = null;
let wasmReady = false;

export function ConstellationCanvas({ graphScene, viewLevel, onAction }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const actionCallbackRef = useRef(onAction);
  actionCallbackRef.current = onAction;

  // Initialize WASM canvas
  useEffect(() => {
    let cancelled = false;

    async function init() {
      if (wasmReady) return;
      try {
        const wasm = await import(
          /* webpackIgnore: true */ "/observatory/pkg/observatory_wasm.js"
        );
        await wasm.default();
        wasmModule = wasm;

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
      } catch (e) {
        console.error("Failed to init observatory WASM:", e);
      }
    }

    init();
    return () => {
      cancelled = true;
    };
  }, []);

  // Push scene to WASM when it changes
  useEffect(() => {
    if (wasmReady && wasmModule && graphScene) {
      wasmModule.set_scene(JSON.stringify(graphScene));
    }
  }, [graphScene]);

  // Push view level to WASM when orientation changes
  useEffect(() => {
    if (wasmReady && wasmModule) {
      wasmModule.set_view_level(viewLevel);
    }
  }, [viewLevel]);

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
