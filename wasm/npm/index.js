// SafeImage for TypeScript and JavaScript hosts: the loader, the typed wrapper and the encodings.
// `SafeImage.create(module)` is what most hosts want; `instantiate` returns the raw `si_*` exports.
// Nothing here computes a URL from the module's own location, so the entry point bundles for a worker; Node
// hosts that want the module's path import `wasmUrl` from "@thinkersjournal/safeimage/node".
export { ABI_VERSION, instantiate } from "./loader.js";
export { SafeImage, hashToHex, hashFromHex } from "./safeimage.js";
export * from "./codec.js";
