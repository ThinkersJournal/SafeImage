# @thinkersjournal/safeimage

The SafeImage WebAssembly module (layers 1 to 4), built in CI from the repository's own sources.
Bytes in, bytes out, no imports. The calling convention and the encodings are in
`docs/WASM-BOUNDARY.md` of the repository. The package carries the module, a typed wrapper over it (no
pointers, no byte layouts for the host to write) and the encodings the wrapper uses. No dependencies.

```ts
import { SafeImage, SafeImageError, wasmUrl } from "@thinkersjournal/safeimage";
import { readFile } from "node:fs/promises";

const si = await SafeImage.create(await readFile(wasmUrl)); // or a compiled WebAssembly.Module

const { hash } = si.pdq(rgb, width, height);
const verdict = si.strongest([providerA, providerB]); // MatchOutcome values
let report = si.reportNew(files.length, true);
const step = si.reportNextStep(report, now);
```

Calls are synchronous. Times are Unix seconds: `number | bigint` in, `bigint` out. State (a report, an
evidence case) is a plain object the host stores and passes back in; the module keeps none.

A refusal is a thrown `SafeImageError` with a numeric `code` and a `codeName`. **It always means hold**:
a host never treats an error as clean, cleared or deleted. A wrong argument type or range is a `TypeError`
or `RangeError` instead (the caller's bug, not a refusal). Unknown tags fail closed: an unknown outcome
decodes to `Unavailable`, never `Clean`.

`encode*` and `decode*` functions are exported for hosts that store or forward the messages.
`instantiate` returns the raw `si_*` exports and refuses a module whose ABI version differs from the package's.

## Checking what you downloaded

`PROVENANCE.json` records the commit, the CI run, the module's SHA-256, its ABI version and the
compiler. The `.wasm` is the file that run checked against the reference fixtures; compare its SHA-256
with `PROVENANCE.json`, and run `npm audit signatures` for the registry's provenance statement.

Licences: `(MIT OR Apache-2.0) AND BSD-3-Clause`; see `LICENSE-MIT`, `LICENSE-APACHE` and `NOTICE`.
