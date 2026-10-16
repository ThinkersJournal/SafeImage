# Changelog

All notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Pre-1.0, a breaking
change raises the second number (0.1.x to 0.2.0).

## [0.3.18] - 2026-10-16

### Docs
- Board 174 rulings recorded: Q8 (crates.io, the PM publishes at the first code release), Q9 option B
  (destination and provider specifics are deploy-time operator configuration; the proposed profiles
  repository is dropped) and Q10 (an npm package built from the CI artifact). `docs/NPM-PACKAGE.md` plans
  the CI job; nothing publishes. `ROADMAP.md` F1 to F3 and the adoption table updated.

## [0.3.17] - 2026-10-14

### Docs
- `ROADMAP.md`: A0 and F5 marked done (P1 and P2 landed in 0.3.11); the adoption table and the
  "where things stand" line now say layers 2 to 4 have WASM exports.

## [0.3.16] - 2026-10-14

### Tests
- `wasm/vectors/layer2.txt` gains vectors for `si_classify_status`, `si_within_cap`, `si_retry_decide` and
  `si_match_request` (they were asserted inline in `test.mjs`, where a host could not replay them);
  `layer3.txt` gains `case_evidence`, `may_submit` and three refusal vectors (same person, clearing a
  sighting, deciding twice). `wasm/vectors/README.md` documents every line kind.
  Roadmap item A3 is done.

## [0.3.15] - 2026-10-14

### Added
- WebAssembly module (ABI version 4): layer 3 exports `si_evidence_transition`, `si_case_evidence`,
  `si_report_file_disposition`, `si_report_may_submit`, `si_preservation_period_secs`,
  `si_preserve_until`, `si_case_apply_file_event`, `si_case_confirm`, `si_case_clear`,
  `si_case_set_hold`, `si_case_lift_hold`, `si_case_begin_destruction_on_request`,
  `si_case_begin_destruction_expired`, `si_case_reaper_eligibility` and `si_check_release_hold`
  (`wasm/src/evidence.rs`, `wasm/src/evidence_codec.rs`). The two-person rule is checked on decode:
  equal ids are `ERR_SAME_PERSON`. Destruction is recorded and nothing is deleted; there is no delete
  export. Every unknown tag is a refusal. Closes roadmap item A2.
- `wasm/vectors/layer3.txt`, replayed by `wasm/test.mjs`.

## [0.3.14] - 2026-10-14

### Added
- WebAssembly module (ABI version 3): layer 4 exports `si_report_new`, `si_report_next_step`,
  `si_report_apply`, `si_report_sync_credentials`, `si_report_request_retract`,
  `si_report_retry_failed`, `si_backoff_secs` and `si_deletion_window_elapsed`, over the same versioned
  encoding (`wasm/src/report_codec.rs`). An unknown answer tag is handled as transient, never as success;
  an unknown state or step tag is a refusal. Credentials never cross: the boundary takes a boolean.
- `wasm/vectors/layer4.txt`, replayed by `wasm/test.mjs`.

## [0.3.13] - 2026-10-14

### Added
- WebAssembly module (`safeimage-wasm`, ABI version 2): layer 2 exports `si_strongest`,
  `si_classify_status`, `si_within_cap`, `si_retry_decide` and `si_match_request`, over a hand-rolled
  versioned binary encoding (`wasm/src/codec.rs`; docs/WASM-BOUNDARY.md section 3). Unknown outcome tags
  decode to `Unavailable(MalformedResponse)`, never `Clean`; every refusal is a negative code; structured
  inputs are capped at 4 MiB. Layer 1 exports and layout are unchanged.
- `wasm/vectors/layer2.txt`: layer 2 vectors generated from the Rust tests and replayed by `wasm/test.mjs`.

## [0.3.12] - 2026-10-14

### Tests
- Reference fixtures gain three large frames (2048 x 1536, 3000 x 2001, 4096 x 4096 = `MAX_FRAME_PIXELS`).
  The crate and the WebAssembly module match the upstream reference bit for bit on all of them; fixtures regenerated from the
  pinned reference, additions only.

## [0.3.11] - 2026-10-14

### Docs
- `docs/provenance/pdq-vetting.md` section 9: how each derived file differs from its upstream file at the
  pinned commit. `NOTICE` describes the derived files in the present tense.

## [0.3.10] - 2026-10-14

### Docs
- `docs/WASM-BOUNDARY.md`: proposal for the WebAssembly boundary of layers 2 to 4 (what crosses,
  encoding options, rules). No code.

## [0.3.9] - 2026-10-14

### Docs
- `docs/INTEGRATION.md`: what a host supplies, the call order for one upload, failing closed, what to
  persist, and the current WebAssembly scope.

## [0.3.8] - 2026-10-14

### Docs
- `ROADMAP.md`: ordered path to the first release and to adoption by Community and Humboldt, with each
  gate named. 0.3.7 is the held draft PR #15.

## [0.3.6] - 2026-10-11

### CI
- Every remaining third-party action is pinned to a full commit SHA with its tag as a comment:
  `actions/checkout` (v7.0.1) and `actions/upload-artifact` (v7.0.2), the current majors, looked up
  from upstream (8 uses).
- `.github/CODEOWNERS`: the maintainer owns every path, including the forbidden-pattern rule file.

### Docs
- `MATCH_DISTANCE_DEFAULT` and `MockMatcher::always` are documented as deliberate public items.

### Tests
- Both now have a test that uses them. No behaviour change.

## [0.3.4] - 2026-10-10

### Docs
- README: current status, the layers by module name, what the crate is not, a Stability section
  (version rule, which enums are exhaustive and why, MSRV, features, WebAssembly) and how the crate
  is checked.

## [0.3.3] - 2026-10-10

### CI
- `Reference fixtures reproduce from upstream on this runner`: fetches the pinned upstream files
  (each blob SHA checked), builds them unmodified with the runner's own `g++`, regenerates
  `tests/fixtures/reference.txt` and requires it to equal the committed file, with a control that a
  tampered fixture is detected. So the committed fixture is no longer the output of one machine.
- `tests/reference/build.sh`: `SAFEIMAGE_REF_LOCAL_GXX=1` uses the local compiler instead of the
  container (the container path is unchanged and still reproduces the committed fixture).

## [0.3.2] - 2026-10-10

### Added
- `examples/end_to_end.rs`: one image through every layer with scripted stand-ins for a provider
  and a destination profile (no network, clock or disk). CI runs it on Linux and Windows.

## [0.3.1] - 2026-10-10

### CI
- `deny` job (cargo-deny: advisories, licences, bans, sources) with `deny.toml`: an allow list of
  MIT, Apache-2.0 and BSD-3-Clause; wildcards denied; crates.io is the only registry and git
  dependencies are refused. The action is pinned to a full commit SHA.

## [0.3.0] - 2026-10-10

### Changed (breaking)
- Error and informational enums are now `#[non_exhaustive]` (`FrameError`, `HexError`,
  `RequestError`, `ProtocolError`, `HttpMethod`, `IdError`, `SlotKind`, `BuildError`, `RetractError`,
  `report::Event`, `FinishNote`, `SignalError`, `ClassifierError`, `CaseError`, `Ineligible`,
  `ReleaseRefusal`, `DestructionBasis`, `Expect`): a host needs a fallback arm. Safety-relevant
  enums stay exhaustive on purpose (see the crate docs, with doc tests that pin both policies).
- `hash::__reference_support` is behind the `reference-support` feature (not default, not API).
- `safeimage-wasm`: a frame error this table does not name returns `-5` (`ERR_INVALID_FRAME`).

### CI
- Tests and clippy run with `--all-features`; a separate step fails loudly if the feature-gated
  reference test cannot run, so it can never be skipped silently.

### Docs
- The serving-key vs evidence-key hold reading recorded as confirmed; Q10 (how a TypeScript
  consumer obtains the module); NOTICE explains the licence expression.

## [0.2.5] - 2026-10-10

### Added
- Layer 3 (S2-5), `evidence`: plain records and pure functions, no storage assumed. The evidence
  state table (upload-scan design 5.7, 8 transitions, every other pair refused) and the derived case
  state; `TwoPeople` (refuses equal ids); review (`confirm`, `clear` with the per-kind approval rule);
  evidence legal holds (two people, with a reason); hold-release checks (an evidence hold is never
  released before the end, even by two people); destruction **recorded** with no delete capability
  anywhere, after which a case refuses every write; the expiry reaper's eligibility; and
  `preserve_until` (every send counts, never earlier, 366 days in one named function).
- 23 more hand mutations (80 native + 5 WASM-boundary), all killed.
- All five layers of the design now exist.

## [0.2.4] - 2026-10-10

### Added
- Layer 5 (S2-6): `classifier` (`Classifier`, `ClassifierSignal`, `MockClassifier`; imports only the
  frame type, no conversion to or from a match outcome, three `compile_fail` doc tests) and
  `screening` (`match_request`, `screen`, `ScreenedFrame`, `ScreenPolicy`): the pre-model screen. A
  match is `Blocked`, any failure is a `Hold`, only `Clean` allows, and a classifier score can only
  add a hold.
- 8 more hand mutations (57 native + 5 WASM-boundary), all killed.

### Docs
- `docs/DESIGN.md`: new section 11, Known limits (secrets not zeroed on drop, plain strings for
  personal data in memory, opaque-id two-person check, and others); section 3.5 matches the code;
  the S2-5 rulings recorded in section 3.3; `docs/PLAN-step2.md` S2-5 rewritten to match them.

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
