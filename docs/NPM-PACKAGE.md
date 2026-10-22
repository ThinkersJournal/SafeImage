# npm package for the WebAssembly layers

Ruled 2026-10-16 (board 174, Q10): a TypeScript consumer gets an npm package built from the CI artifact,
not a vendored `.wasm`. This note describes the CI job. **Nothing here publishes**: the npm account and token
are not available to this repository, and publishing is the PM's.

## What the package is

| | |
|---|---|
| **Contents** | `safeimage_wasm.wasm` (the CI artifact, byte for byte), `index.js`, `loader.js`, `safeimage.js`, `codec.js`, `node.js` and `index.d.ts` / `node.d.ts` (the loader, the typed wrapper, the encodings, a Node-only path helper and the types; see "The typed wrapper"), `PROVENANCE.json`, `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`, `README.md` |
| **Name** | proposed `@thinkersjournal/safeimage` (a scope, so the name cannot be squatted). Needs the npm organisation to exist: a question for the PM, not decided here |
| **Version** | equal to the workspace version in `Cargo.toml` (one number per project). The job fails if `package.json` and `Cargo.toml` differ |
| **Scope** | layers 1 to 4 only (layer 5 stays Rust-only). No provider or destination specifics: those are deploy-time operator configuration (Q9) |

## The typed wrapper (0.3.25)

Ruled by the PM on 2026-10-22 (Community adoption, board 228): the package ships the TypeScript-facing
binding itself, so a consumer neither writes pointer-and-length glue nor re-implements the encodings, and
there is one source of truth. It is plain JavaScript with hand-written `index.d.ts` (no build step, no
dependencies), in three files:

| File | Holds |
|---|---|
| `loader.js` | `instantiate` and `ABI_VERSION`: compiles, instantiates with no imports, checks the ABI version |
| `codec.js` | the encodings of `docs/WASM-BOUNDARY.md` section 3 (`encode*` / `decode*`), `SafeImageError` and `CODES` |
| `safeimage.js` | the `SafeImage` class: one method per `si_*` export, owning alloc, copy, call, copy out and free |
| `node.js` | `wasmUrl`, the module's path, for Node hosts only: a separate subpath (`@thinkersjournal/safeimage/node`) so the main entry never uses `import.meta`, which a worker bundler rejects (Community's workerd gate; `build.mjs` fails if a main-entry file uses it). A worker imports `@thinkersjournal/safeimage/safeimage_wasm.wasm` for a compiled module |

Rules the wrapper keeps, each tested by `wasm/npm/test.mjs`:

- **Same bytes as Rust.** The test replays every line of `wasm/vectors/layer2.txt`, `layer3.txt` and
  `layer4.txt` through the typed API (decode the vector's bytes, call the method, encode the result, compare
  with the vector). A line that expects a refusal code must make the wrapper throw a `SafeImageError` with
  that code. CI runs it on the checked module, in the `wasm-artifact` job.
- **An error is a hold.** Every negative return is a thrown `SafeImageError` (`code`, `codeName`). The two
  exports whose negative codes are answers (`caseReaperEligibility`, `checkReleaseHold`) return the reason
  as a string and `null` for "allowed"; any other code throws. A wrong argument type or range is a
  `TypeError` or `RangeError`: the caller's bug, not a refusal.
- **Fail closed on decode.** An unknown outcome tag is `Unavailable(MalformedResponse)`, an unknown answer
  tag is `Unknown`, every other unknown tag throws. Nothing decodes to clean.
- **Views are taken after every allocation**, because an allocation can grow the memory and detach an older view.
- Times are Unix seconds: `number | bigint` in, `bigint` out. Hashes are 32 bytes in the layer 1 layout.
- The module keeps no state. A report state and a case record are plain objects the host stores.

`build.mjs` also imports the packed entry point from the staging directory and instantiates the packed module,
so a file left out of the tarball fails the job.

## The CI job: `npm-package` (built, 0.3.20)

Runs after `wasm-artifact` in `.github/workflows/ci.yml`, on every push and pull request, and builds only:

1. Download the `safeimage-wasm-<sha>` artifact that `wasm-artifact` uploaded (the file that
   `wasm/test.mjs` already checked against the reference fixtures). Never rebuild it here.
2. Write `package.json` from `wasm/npm/package.template.json`; `version` comes from `Cargo.toml`, and the job fails if the two crates differ. The whole step is `wasm/npm/build.mjs`, which also runs locally (`node wasm/npm/build.mjs <wasm> <out> <commit> <run url> <rustc>`).
3. Write `PROVENANCE.json`: `{ commit, workflow_run_url, wasm_sha256, abi_version, rustc }`. The
   ABI version is read from the module's own export, so the file cannot disagree with the binary.
4. `npm pack`, then check the tarball: exactly the listed files, the `.wasm` hash equals the artifact's,
   no file outside the list. Upload the `.tgz` as a build artifact named `safeimage-npm-<sha>`.
5. The vendor grep runs over the tarball's text files, with a planted positive control (the binary module is skipped: its bytes can match a short pattern by chance).

`download-artifact` is pinned to a full commit SHA, as `upload-artifact` is.

## Publishing (not built yet)

A separate job, manual (`workflow_dispatch` on a release tag) and never on pull requests, would run
`npm publish --provenance --access public` with `id-token: write`, so the registry records the
workflow run and commit that built the tarball. Prerequisites, all open:

- the npm organisation and a publish credential, held by the PM and not by this repository's code;
- the first code release of the crate on crates.io (Q8), so both go out at one version;
- F4 answered, since the README text ships in the package.

## Consumer check

A consumer verifies a download by `npm audit signatures` (registry provenance) and by comparing
`PROVENANCE.json.wasm_sha256` with the `.wasm` it received. The same hash is in the CI run named there.
