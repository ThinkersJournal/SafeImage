export declare const ABI_VERSION: 4;
export declare const wasmUrl: URL;

/** The module's exports: linear memory plus the `si_*` functions (docs/WASM-BOUNDARY.md). */
export interface SafeImageExports {
  memory: WebAssembly.Memory;
  si_abi_version(): number;
  si_alloc(len: number): number;
  si_free(ptr: number, len: number): void;
  [name: `si_${string}`]: (...args: number[]) => number | bigint | void;
}

/** Throws if the module's ABI version differs from `ABI_VERSION`. */
export declare function instantiate(source: BufferSource | WebAssembly.Module): Promise<SafeImageExports>;
