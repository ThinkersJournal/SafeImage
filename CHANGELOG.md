# Changelog

All notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Pre-1.0, a breaking
change raises the second number (0.1.x to 0.2.0).

## [0.2.3] - 2026-10-10

### Added
- Layer 4, `report` (S2-4): the generic report mechanism. `ReportDestination` (profile-supplied
  build, request and interpret), `ReportState` and the pure `next_step` / `apply` transition
  functions, a closed `DestinationAnswer` in which `Unknown` is never success, backoff of 2, 4, 8,
  16 then 30 minutes, withdrawal and human-retry rules, credential slots with generic setup
  prompts, `SetupState::NoDestinationsConfigured` distinct from `Ready`, and redacting `Secret`,
  `Credentials`, `Person` and `BuiltReport`. It names no destination.
- 21 more hand mutations (49 native + 5 WASM-boundary), all killed.

### Docs
- `docs/DESIGN.md`: section 3.4 trait snippet matches the implementation; Q7 recorded as passed on
  real workerd by the Community lane.

## [0.2.2] - 2026-10-10

### Added
- Layer 2, `matcher` (S2-3): `MatchRequest` (1 to 8 hashes), `MatchOutcome` ordered
  `Clean` < `Unavailable` < `Match`, closed `Unavailable` reasons, `strongest`, the sans-I/O
  `ProviderProtocol` with plain-data `HttpRequestSpec`/`HttpResponseSpec`, `classify_status`,
  `RetryPolicy::decide`, `MockMatcher`, and `conformance_failures`, a fail-closed suite a provider
  profile runs against itself. No provider is implemented here (profiles live outside the repo).
- 10 more hand mutations (28 native + 5 WASM-boundary), all killed.

## [0.2.1] - 2026-10-10

### Added
- `safeimage-wasm` (workspace member): raw WebAssembly exports for layer 1. No imports, no
  generated glue; `si_alloc`, `si_free`, `si_pdq`, `si_pdq_dihedral`, `si_abi_version`; typed
  negative error codes; panics abort. Output layout documented in `wasm/src/lib.rs`.
- `Hash256::to_bytes` (32 bytes, word 0 first, little-endian).
- CI job `WASM module ...`: builds the module, checks it has no imports and the expected exports,
  runs it under Node against the reference fixtures bit for bit, and uploads it as an artifact.
- `docs/provenance/pdq-vetting.md` section 8: upstream `pdq/wasm` read and credited, not used.

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
