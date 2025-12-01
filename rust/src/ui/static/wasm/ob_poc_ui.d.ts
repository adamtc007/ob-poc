/* tslint:disable */
/* eslint-disable */
/**
 * WASM entry point - called from JavaScript
 */
export function start(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly start: () => void;
  readonly wasm_bindgen__convert__closures_____invoke__hbf9ae58da275e537: (a: number, b: number) => [number, number];
  readonly wasm_bindgen__closure__destroy__h7f32248e30fc7345: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h70efb84e883dfc7b: (a: number, b: number, c: any) => void;
  readonly wasm_bindgen__closure__destroy__hebcb7c4571565040: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__hd2973217c503a60e: (a: number, b: number, c: any) => void;
  readonly wasm_bindgen__closure__destroy__h455b909860767ac8: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h8ef0eededdf940ca: (a: number, b: number) => void;
  readonly wasm_bindgen__closure__destroy__ha7ea1142cb04e3b5: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h978daf44cbff8dc7: (a: number, b: number, c: any) => void;
  readonly wasm_bindgen__closure__destroy__hf3a398b8c231585d: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__hd8843700ec8d3367: (a: number, b: number) => number;
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
