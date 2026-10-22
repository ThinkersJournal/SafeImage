// Node-only helper: where the module sits inside this package. A separate entry point because the main one
// must not read `import.meta.url` (a worker bundler rejects it). A worker host imports the module itself:
// `import mod from "@thinkersjournal/safeimage/safeimage_wasm.wasm"` gives a compiled WebAssembly.Module.

/** Location of the module inside this package. */
export const wasmUrl = new URL("./safeimage_wasm.wasm", import.meta.url);
