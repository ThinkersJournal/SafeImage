# Changelog

All notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Pre-1.0, a breaking
change raises the second number (0.1.x to 0.2.0).

## [0.2.0] - 2026-10-10

### Added
- Layer 1, `hash`: `Frame`, `Hash256`, `pdq`, `pdq_dihedral`, `MATCH_DISTANCE_DEFAULT`,
  `QUALITY_FLOOR_DEFAULT`. A hand port of the Meta PDQ reference at the pinned commit
  (`docs/provenance/pdq-vetting.md`); `no_std` plus `alloc`, one dependency (`libm`).
- Reference fixtures: unmodified upstream sources built by `tests/reference/build.sh`; hashes,
  qualities, all eight dihedral hashes, the DCT stage and the median routine agree bit for bit
  on 18 synthetic frames.
- `tests/reference/mutants.py`: 18 hand-picked mutations, all killed.
- CI job that builds the library for `wasm32-unknown-unknown`.

### Licence
- Meta-derived files under `src/hash/` keep upstream's licence (`src/hash/LICENSE`); the manifest
  licence expression is now `(MIT OR Apache-2.0) AND BSD-3-Clause`.

## [0.1.2] - 2026-10-10

### Added
- `docs/provenance/pdq-vetting.md`: upstream pin, licence reading and scope for the PDQ port (S2-1 step 0).

## [0.1.1] - 2026-10-10

### Added
- `docs/PLAN-step2.md`: the first code tasks, in order, with acceptance criteria.

## [0.1.0] - 2026-10-10

### Added
- Repository skeleton: licence files, `NOTICE`, `README.md`.
- `docs/DESIGN.md`: the crate boundary, in five layers, and what stays in the
  consuming applications.
- CI: formatting, clippy with warnings denied, and tests on Linux and Windows.
- An empty library crate so CI has something to build. It contains no behaviour.
