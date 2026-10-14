# Roadmap

Written against `origin/main` 0.3.6. Order matters; each item is one PR with its own version.
Decisions that are not ours are marked **gate** and name who decides. Nothing here is a promise of dates.

## Where things stand

- Layers 1 to 5 exist, are tested, and are bit-exact against the reference on the fixtures
  (`docs/DESIGN.md` section 3). `wasm/` exposes layers 1 to 4 (ABI version 4; layer 5 stays Rust only).
- The repository is already public. What has **not** happened is a release: `safeimage` is
  `publish = false`, and PR #15 (draft, 0.3.7) is the change that removes that line.
- No consumer depends on the crate yet. Community's adoption steps 3 to 5 wait on the gates below.

## Path to the first release (the "public flip")

The repository is public; the flip left is "a consumer can depend on a published version".

| # | Step | Gate | Status |
|---|---|---|---|
| F1 | Registry and who publishes (Q8) | CireSnave, board 174 | open |
| F2 | Where profiles live and how a public consumer obtains them (Q9) | CireSnave, board 174 | open |
| F3 | How a TypeScript consumer obtains the WASM: npm package or vendored `.wasm` with SHA (Q10) | CireSnave, board 174 | open |
| F4 | Written answer on naming the hash-match provider in public text (G4) | CireSnave | open |
| F5 | Provenance close-out (portfolio rule 6.4a); the derived list is below | this lane, PM at gate | done: P1 and P2 landed in 0.3.11 |
| F6 | Merge #15 (`publish = false` removed, package metadata) and publish | PM, from a clean `git archive` of `main` | draft, held |

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

Until F1 to F4 have written answers, nothing in this repository names a provider or a report destination.

## Adoption

### Community (TypeScript on workerd)

Its own plan is the migration table in its `2026-10-10-safeimage-split.md`; the SafeImage side of each step:

| Step | SafeImage supplies | Exists today | Needs |
|---|---|---|---|
| 2 | PDQ in the WASM module | yes (Q7 passed 2026-10-10) | F3 to ship it as an artifact |
| 3 | Layer 2: `strongest`, request building and interpretation, retry policy | Rust, and WASM exports (0.3.13) | F3 to ship it as an artifact; a profile for `request`/`interpret` (F2) |
| 4 | Layer 4 generic half | Rust, and WASM exports (0.3.14) | F3 to ship it as an artifact; a profile to run it (F2) |
| 5 | Layer 3 pure half | Rust, and WASM exports (0.3.15) | F3 to ship it as an artifact |

### Humboldt (Rust)

At `origin/main` of Humboldt the only mentions of SafeImage are in its CI and `deny.toml` (shared job
shape). No upload surface or requirement for hash matching is documented there, so there is nothing to
adopt yet. If it gains one, it depends on the crates.io version directly (F6) and needs no WASM.
Scoping that is Humboldt's lane to start; this item stays open until it does.

## Work in this repository, in order

Items not blocked by F1 to F4 are marked **ready**.

| # | Item | Why | State |
|---|---|---|---|
| A1 | `docs/INTEGRATION.md`: how a host wires the layers (what the host supplies, the call order, failing closed, what the host stores), written for a Rust host and for the WASM host | Both consumers need it before they can adopt; no code change | **ready** |
| A2 | WASM boundary for layers 2 to 4: a design note first (byte-in, byte-out encoding, typed errors, no panic across the boundary, the acceptance criteria in DESIGN section 5), then the implementation | Community steps 3 to 5 cannot run in a Worker without it | **done** (layers 2 in 0.3.13, 4 in 0.3.14, 3 in 0.3.15) |
| A3 | Conformance vectors for layers 2 to 4 from the Rust tests, in a form a TypeScript host can replay | Lets Community prove its adoption without editing its own tests | **done** (0.3.16): `wasm/vectors/*.txt` and `wasm/vectors/README.md` |
| A0 | P1 and P2 above: provenance differences table, `NOTICE` tense | The one open provenance item | **done** (0.3.11) |
| A4 | Reference cross-check above 512 px: frames of 2048 x 1536, 3000 x 2001 and 4096 x 4096 (exactly `MAX_FRAME_PIXELS`) added to the fixtures; hash, quality and all eight orientations match the reference bit for bit. The workerd run (Q7) is still untested above 512 px: that is the consumer's gate, not the crate's | Closes the crate-side half of the stated limit | **done** (0.3.12) |
| A5 | Re-run `tests/reference/build.sh` and the mutation suite at release candidate, record the head SHA | Release evidence | at F6 |
| A6 | 0.4.0 or 1.0 stability decision: which enums stay exhaustive, what is frozen | Only after a consumer has used the API | after first adoption |

## Not on this roadmap

- Any hash list, token, key, destination schema or provider endpoint. These are profiles and operator
  configuration (DESIGN section 3.6), never in this tree.
- A decoder, an async driver, or an HTTP client (DESIGN Q6).
- Publishing `safeimage-wasm` to crates.io: it holds the only `unsafe`, and stays `publish = false`.
