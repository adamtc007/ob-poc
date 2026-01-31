/* tslint:disable */
/* eslint-disable */

/**
 * Initialize the UI app - does NOT use #[wasm_bindgen(start)] to avoid
 * conflict with ob-poc-graph's start function.
 */
export function init_ui(): void;

/**
 * Start the full egui application
 *
 * Called from JavaScript after WASM is loaded.
 * Canvas ID should be the ID of an HTML canvas element.
 */
export function start_app(canvas_id: string): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly init_ui: () => [number, number];
    readonly start_app: (a: number, b: number) => [number, number];
    readonly wasm_bindgen__closure__destroy__h3f6fc726a6cabe18: (a: number, b: number) => void;
    readonly wasm_bindgen__closure__destroy__h73d1be253f0eab79: (a: number, b: number) => void;
    readonly wasm_bindgen__closure__destroy__h200d8bc474dfeb60: (a: number, b: number) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h8c2f96938904af5f: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h9a1c0af59a71c980: (a: number, b: number) => [number, number];
    readonly wasm_bindgen__convert__closures_____invoke__ha528f2cdf3437c28: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h4e79a815969ae889: (a: number, b: number, c: any) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
