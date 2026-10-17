# @thinkersjournal/safeimage

The SafeImage WebAssembly module (layers 1 to 4), built in CI from the repository's own sources.
Bytes in, bytes out, no imports. The calling convention and the encodings are in
`docs/WASM-BOUNDARY.md` of the repository; this package carries only the module and a loader.

```js
import { instantiate, wasmUrl } from "@thinkersjournal/safeimage";
import { readFile } from "node:fs/promises";

const x = await instantiate(await readFile(wasmUrl));
```

`instantiate` refuses a module whose ABI version differs from the package's.

## Checking what you downloaded

`PROVENANCE.json` records the commit, the CI run, the module's SHA-256, its ABI version and the
compiler. The `.wasm` is the file that run checked against the reference fixtures; compare its SHA-256
with `PROVENANCE.json`, and run `npm audit signatures` for the registry's provenance statement.

Licences: `(MIT OR Apache-2.0) AND BSD-3-Clause`; see `LICENSE-MIT`, `LICENSE-APACHE` and `NOTICE`.
