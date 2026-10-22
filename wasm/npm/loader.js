// Loader for the SafeImage module (re-exported from index.js). It instantiates the module and checks its ABI version; the
// exports are the raw pointer-and-length functions documented in docs/WASM-BOUNDARY.md.

/** ABI version this package was built against; `instantiate` refuses a module that reports another. */
export const ABI_VERSION = 4;

/** Location of the module inside this package. */
export const wasmUrl = new URL("./safeimage_wasm.wasm", import.meta.url);

/**
 * @param {BufferSource | WebAssembly.Module} source the module's bytes, or an already compiled module
 *   (a worker host compiles it at deploy time and passes it in).
 */
export async function instantiate(source) {
  const module = source instanceof WebAssembly.Module ? source : await WebAssembly.compile(source);
  const { exports } = await WebAssembly.instantiate(module, {}); // the module has no imports
  const got = exports.si_abi_version();
  if (got !== ABI_VERSION) throw new Error(`SafeImage ABI mismatch: package ${ABI_VERSION}, module ${got}`);
  return exports;
}
