/* tslint:disable */
/* eslint-disable */
export function start(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly start: () => void;
  readonly wasm_bindgen__convert__closures_____invoke__hdfcd365507308ceb: (a: number, b: number) => void;
  readonly wasm_bindgen__closure__destroy__hc3e7be372032db00: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h82d3f5aaee324d41: (a: number, b: number, c: any) => void;
  readonly wasm_bindgen__closure__destroy__he96b87792db9cdaa: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__hb09d37411ce7b925: (a: number, b: number, c: any) => void;
  readonly wasm_bindgen__closure__destroy__h3c348e801e36b1f9: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h09c550ab9332879d: (a: number, b: number) => [number, number];
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
