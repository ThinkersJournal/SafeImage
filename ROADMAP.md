# Roadmap

Written against `origin/main` 0.3.6. Order matters; each item is one PR with its own version.
Decisions that are not ours are marked **gate** and name who decides. Nothing here is a promise of dates.

## Where things stand

- Layers 1 to 5 exist, are tested, and are bit-exact against the reference on the fixtures
  (`docs/DESIGN.md` section 3). `wasm/` exposes layers 1 to 4 (ABI version 4; layer 5 stays Rust only).
- The repository is already public. What has **not** happened is a release: `safeimage` is
  `publish = false`, and PR #15 (draft) is the change that removes that line.
- No consumer depends on the crate yet. Community's adoption steps 3 to 5 wait on the gates below.

## Path to the first release (the "public flip")

The repository is public; the flip left is "a consumer can depend on a published version".

| # | Step | Gate | Status |
|---|---|---|---|
| F1 | Registry and who publishes (Q8): crates.io, the PM publishes at the first code release | CireSnave, board 174 | **ruled 2026-10-16** |
| F2 | Where profiles live (Q9): deploy-time operator configuration, never a public repository | CireSnave, board 174 | **ruled 2026-10-16** |
| F3 | How a TypeScript consumer obtains the WASM (Q10): an npm package built from the CI artifact (plan: `docs/NPM-PACKAGE.md`) | CireSnave, board 174 | **ruled 2026-10-16**; CI job `npm-package` built (0.3.20), publish not built |
| F4 | Written answer on naming the hash-match provider in public text (G4) | CireSnave, board 222 | **ruled 2026-10-22: neutral until the provider consents in writing.** Nothing public names a provider; CI enforces it. Not a gate for the first release |
| F5 | Provenance close-out (portfolio rule 6.4a); the derived list is below | this lane, PM at gate | done: P1 and P2 landed in 0.3.11 |
| F6 | Merge #15 (`publish = false` removed, package metadata), then the PM publishes the crate from a clean `git archive` of `main`; CireSnave approves the npm run | PM, CireSnave | #15 ready, awaiting gate |

### F5: provenance, derived from rule 6.4a against this tree (2026-10-14, `main` 587226a)

Third-party-derived files: `src/hash/pdq.rs`, `downscale.rs`, `hash256.rs` (Meta, BSD-3-Clause),
`src/hash/torben.rs` (public domain, Mogensen and Devillard). Checked:

- Each carries a header naming the upstream file, the pinned commit and the upstream blob SHA, and the
  Meta copyright line (`torben.rs` carries the public-domain statement instead, as upstream does).
- `src/hash/LICENSE` is the upstream licence verbatim: `git hash-object` gives `9406c626...`, the blob
  recorded in `docs/provenance/pdq-vetting.md`. `NOTICE` lists every derived file and the credit.
- No upstream image, data file or hash list is in the tree (vetting record section 5). The reference
  harness is our own MIT OR Apache-2.0 file and links the unmodified upstream sources at build time;
  `build.sh` verifies each upstream blob SHA before use.

Closed in 0.3.11 (kept for the record):

| # | Item | Action |
|---|---|---|
| P1 | The vetting record proves equality by outputs (reference fixtures, mutation suite) but has no per-file list of how each port **differs** from its upstream file, which rule 6.4a asks for | Done: `docs/provenance/pdq-vetting.md` section 9 |
| P2 | `NOTICE` still speaks of derived files in the future tense ("will carry") | Done: `NOTICE` reads in the present tense |

Nothing in this repository names a provider or a report destination, and nothing will until a provider agrees in writing (F4); CI enforces it.

## Adoption

### Community (TypeScript on workerd)

Its own plan is the migration table in its `2026-10-10-safeimage-split.md`; the SafeImage side of each step:

| Step | SafeImage supplies | Exists today | Needs |
|---|---|---|---|
| 2 | PDQ in the WASM module | yes (Q7 passed 2026-10-10) | F3: the npm package |
| 3 | Layer 2: `strongest`, request building and interpretation, retry policy | Rust, and WASM exports (0.3.13) | F3: the npm package; operator configuration for `request`/`interpret` (F2) |
| 4 | Layer 4 generic half | Rust, and WASM exports (0.3.14) | F3: the npm package; operator configuration to run it (F2) |
| 5 | Layer 3 pure half | Rust, and WASM exports (0.3.15) | F3: the npm package |

### Humboldt (Rust)

At `origin/main` of Humboldt the only mentions of SafeImage are in its CI and `deny.toml` (shared job
shape). No upload surface or requirement for hash matching is documented there, so there is nothing to
adopt yet. If it gains one, it depends on the crates.io version directly (F6) and needs no WASM.
Scoping that is Humboldt's lane to start; this item stays open until it does.

## Work in this repository, in order

Items not blocked by F1 to F3 are marked **ready**.

| # | Item | Why | State |
|---|---|---|---|
| A1 | `docs/INTEGRATION.md`: how a host wires the layers (what the host supplies, the call order, failing closed, what the host stores), written for a Rust host and for the WASM host | Both consumers need it before they can adopt; no code change | **ready** |
| A2 | WASM boundary for layers 2 to 4: a design note first (byte-in, byte-out encoding, typed errors, no panic across the boundary, the acceptance criteria in DESIGN section 5), then the implementation | Community steps 3 to 5 cannot run in a Worker without it | **done** (layers 2 in 0.3.13, 4 in 0.3.14, 3 in 0.3.15) |
| A3 | Conformance vectors for layers 2 to 4 from the Rust tests, in a form a TypeScript host can replay | Lets Community prove its adoption without editing its own tests | **done** (0.3.16): `wasm/vectors/*.txt` and `wasm/vectors/README.md` |
| A0 | P1 and P2 above: provenance differences table, `NOTICE` tense | The one open provenance item | **done** (0.3.11) |
| A4 | Reference cross-check above 512 px: frames of 2048 x 1536, 3000 x 2001 and 4096 x 4096 (exactly `MAX_FRAME_PIXELS`) added to the fixtures; hash, quality and all eight orientations match the reference bit for bit. The workerd run (Q7) is still untested above 512 px: that is the consumer's gate, not the crate's | Closes the crate-side half of the stated limit | **done** (0.3.12) |
| A5 | Re-run `tests/reference/build.sh` and the mutation suite at release candidate, record the head SHA | Release evidence | at F6 |
| A6 | 0.4.0 or 1.0 stability decision: which enums stay exhaustive, what is frozen | Only after a consumer has used the API | after first adoption |
| A7 | Remove the `#![allow(dead_code)]` in `tests/support/mod.rs`: it is shared by several test crates that each use a subset (`make`, `FRAMES`, `dct_input`, `torben_cases`). Cure: make it a `publish = false` workspace member that the tests depend on (its `pub` items are then not dead code in any test crate), or split it per consumer; not another allow | CodeRipper `lint-suppression` | **done** (0.3.24) |
| A8 | Narrow the crate-level `#![allow(unsafe_code)]` in `wasm/src/lib.rs`: `unsafe` occurs only in `lib.rs` and `evidence.rs` (the pointer-taking `extern "C"` exports and `input_slice`). Cure: drop the crate-level allow and put `#[allow(unsafe_code, reason = ...)]` on those items or on one small `mod abi`, so a new `unsafe` anywhere else fails the build. The exports themselves need `unsafe` for the C ABI and cannot be removed | CodeRipper `lint-suppression` | **done** (0.3.23) |
| A9 | Typed TypeScript wrapper in the npm package (encode, call, decode over `si_*`), so Community does not write a private copy | PM ruling 2026-10-22 (board 228) | **done** (0.3.25); next: A10 |
| A10 | OIDC trusted-publishing job `npm-publish.yml` (release tag, `id-token: write`, `npm publish --provenance --access public`, environment `npm-publish`) | PM, 2026-10-22 | **done** (0.3.26) |
| A11 | CodeRipper `licence`, Medium/Medium, 1 finding (`Cargo.toml`, run 40007877667 on `a338ee7`): the SPDX expression names BSD-3-Clause but no licence file is named for it. The text is `src/hash/LICENSE` (upstream, verbatim, blob recorded in the vetting record), already in the package. Verdict: real as a discoverability gap, not a licensing error. Cure: a root `LICENSE-BSD-3-Clause` that is the same blob, listed in `include` and `NOTICE`, with the vetting record's blob check covering both copies. No waiver | CodeRipper portfolio (advisory) | ready |
| A12 | CodeRipper `licence`, Medium/High, 57 of 57 source files without an `SPDX-License-Identifier` in their first 15 lines (first at `examples/end_to_end.rs:1`). Verdict: real. Cure: our own files get `// SPDX-License-Identifier: MIT OR Apache-2.0`; the four derived hash files keep their upstream header and gain `BSD-3-Clause` (`torben.rs`: public domain, `LicenseRef-PublicDomain`), so no credit line is moved or shortened. One mechanical PR, enumerated from `git ls-files`, with a CI check that fails on a new file lacking the line. No waiver | CodeRipper portfolio (advisory) | ready, after A11 |
| A13 | CodeRipper `reachability`, Medium/Medium, 11 findings, all in `safeimage-wasm` (job on PR #36, run 40059266168): `REPORT_APPLY_HEADROOM` (`abi.rs:376`), `CASE_HEADROOM` (`evidence.rs:29`), `kind_from_code` (`evidence.rs:330`), `raw` (`codec.rs:82`), `decode_outcome`, `encode_outcomes`, `encode_two`, `encode_event`, `encode_approval`, `encode_clocks`, `encode_answer`. Read at `a338ee7`: eight have callers only in the crate's own `tests*.rs`, and the two `*_HEADROOM` constants are the documented allocation sizes for hosts (named in doc comments, used by `tests_report.rs`); verdict for those ten: not dead, the tool sees no non-test caller. `kind_from_code` has no caller anywhere: verdict real. Cure: delete `kind_from_code` (or use it, if a host-side decode is wanted); move the encoders that only tests call into a `#[cfg(test)]` module so non-test builds do not carry them; keep the two constants and say in `.coderipper.toml` why, as the waiver reason | CodeRipper portfolio (advisory) | ready |

## Not on this roadmap

- Any hash list, token, key, destination schema or provider endpoint. These are profiles and operator
  configuration (DESIGN section 3.6), never in this tree.
- A decoder, an async driver, or an HTTP client (DESIGN Q6).
- Publishing `safeimage-wasm` to crates.io: it holds the only `unsafe`, and stays `publish = false`.
