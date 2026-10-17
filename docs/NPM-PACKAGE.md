# npm package for the WebAssembly layers

Ruled 2026-10-16 (board 174, Q10): a TypeScript consumer gets an npm package built from the CI artifact,
not a vendored `.wasm`. This note describes the CI job. **Nothing here publishes**: the npm account and token
are not available to this repository, and publishing is the PM's.

## What the package is

| | |
|---|---|
| **Contents** | `safeimage_wasm.wasm` (the CI artifact, byte for byte), `index.js` and `index.d.ts` (the loader and types; no logic beyond instantiating the module and the ABI check), `PROVENANCE.json`, `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`, `README.md` |
| **Name** | proposed `@thinkersjournal/safeimage` (a scope, so the name cannot be squatted). Needs the npm organisation to exist: a question for the PM, not decided here |
| **Version** | equal to the workspace version in `Cargo.toml` (one number per project). The job fails if `package.json` and `Cargo.toml` differ |
| **Scope** | layers 1 to 4 only (layer 5 stays Rust-only). No provider or destination specifics: those are deploy-time operator configuration (Q9) |

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
