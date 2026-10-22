# npm package for the WebAssembly layers

Ruled 2026-10-16 (board 174, Q10): a TypeScript consumer gets an npm package built from the CI artifact,
not a vendored `.wasm`. This note describes the CI job and the publish workflow. No token is stored in this repository: publishing is by
npm trusted publishing (OIDC), approved per run by CireSnave.

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

## Publishing: `.github/workflows/npm-publish.yml` (built, 0.3.26)

npm trusted publishing (OIDC): **no token is stored anywhere**. The registry trusts one workflow file
(`npm-publish.yml`, bound with `npm trust github`, so the file must not be renamed) running in the
`npm-publish` environment, and exchanges the run's OIDC identity for a one-run credential. It runs only when
a `v*` tag is pushed and only after the environment's required reviewer (CireSnave) approves; the environment
admits `v*` tags only. Steps, in order, each failing the run:

1. The tag equals `v<version in Cargo.toml>`, and the tagged commit is an ancestor of `origin/main`.
2. npm is 11.5.1 or newer (the first version that publishes by OIDC), checked before the registry is touched.
3. Build the module, then run `wasm/test.mjs` (reference fixtures, error codes) and `wasm/npm/test.mjs` (the
   typed wrapper over every conformance vector) against it. This repeats `wasm-artifact` on the tagged commit.
4. `wasm/npm/build.mjs` packs it (the same script CI runs), then the vendor scan with its planted control.
5. `npm publish <tgz> --provenance --access public`. `id-token: write` is granted to this one job only.
6. Read the registry back: `dist.integrity` of the published version must equal the SHA-512 of the tarball built here.
   An exit code is not a state read.

The sequence for a release is the PM's: the crates.io release and the npm publish go out at one version (Q8,
Q10). The PM publishes the crate; CireSnave approves the npm run. A tag that no reviewer approves publishes nothing.

### F4 is not a gate (ruled 2026-10-22)

CireSnave, on board decision 222: *"I'm still waiting to hear back from [the provider] about naming them.  If
they say no, we can't."* (the name is withheld here on purpose) Until a provider agrees in writing, nothing public names a provider or a report
destination, and the package text (README, comments, types) names none. The CI job `No credentials or provider
names in the tree` enforces this on every tracked file (README, docs, CHANGELOG, comments, package sources), and
the `npm-package` job and the publish workflow repeat the scan on the tarball. Both run a planted-name control, so
a pattern that stopped matching fails the job. The denylist is `.github/forbidden-patterns.txt`, written as
character classes so it does not itself spell what it forbids. If a provider consents, the wording is a new
ruling and a new PR.

## Consumer check

A consumer verifies a download by `npm audit signatures` (registry provenance) and by comparing
`PROVENANCE.json.wasm_sha256` with the `.wasm` it received. The same hash is in the CI run named there.
