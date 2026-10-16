# STEP 2 plan: first code, in order

**Status:** proposal for the PM's gate, agreed in outline with the Community lane (2026-10-10). It turns
`DESIGN.md` §6 into tasks with acceptance criteria. Nothing here is built.

Rules that apply to every task: no provider, destination or hash-service name in any file, test name, comment
or commit (CI scans for it); no credential, hash list or real-service call; generated test images only; no
dependency outside crates.io; `publish = false` until released; the PM allocates each version number at gate
time (pre-1.0: a breaking change raises the second number).

## Task S2-1: layer 1, PDQ on a decoded frame (`hash` feature)

- **Scope:** `Frame` (raw RGB8 plus explicit width and height, length checked on construction), `Hash256`
  (Hamming distance, hex and base64 codecs), `pdq(frame)`, `pdq_dihedral(frame)` returning the eight
  orientations, quality score, `normalise_geometry(w, h)`, named default constants for the match distance
  and the quality floor. `no_std + alloc`, `#![forbid(unsafe_code)]`.
- **Provenance first (before any code):** pin the upstream commit SHA; read every file in scope at that SHA;
  record the licence that governs it; port by hand, function for function; each derived file's header names
  its upstream file, the SHA and the copyright line; upstream licence text added verbatim beside it;
  `NOTICE` updated in the same PR. No third-party PDQ crate unless traced to its origin first.
- **Acceptance:**
  - byte-exact agreement with the reference implementation's outputs on generated images (fixtures are the
    reference's outputs, committed as data; no upstream image is copied unless its licence is recorded);
  - the reference's own tolerance holds: quality at or above 80 within distance 10;
  - a wrong-stride or wrong-length frame is a typed error, never a hash (a fixture with width and height
    swapped must fail);
  - no panics across the public API (fuzz or property test over arbitrary dimensions);
  - builds for `wasm32-unknown-unknown` in CI.
- **Version:** 0.2.0 (first behaviour; the PM allocates).

## Task S2-2: WASM artifact for the consumer's workerd proof (Q7)

- **Scope:** a thin binding crate or feature exposing `pdq` as bytes in, bytes out, with typed errors; the
  generated glue is imported, not compiled from bytes at runtime.
- **Acceptance (the consumer's criteria, DESIGN §5):** imported-module instantiation; synchronous
  initialisation; no `fetch`, no `new URL(import.meta.url)`, no `node:` imports in the glue; no panic across
  the boundary. CI builds the artifact and publishes it as a workflow artifact for the consumer to load. The
  consumer runs its own workerd gate; its result decides option A or B.
- **Depends on:** S2-1.

## Task S2-3: layer 2, matcher types and mock

- **Scope:** `MatchRequest` (1 to 8 hashes), `MatchOutcome` (`Match` > `Unavailable` > `Clean`), closed
  `Unavailable` reasons, `strongest`, `ProviderProtocol` (sans-I/O `request` and `interpret`),
  `HttpRequestSpec` and `HttpResponseSpec` as plain data, `RetryPolicy::decide`, `MockMatcher`, and a
  conformance suite a profile runs against itself.
- **Acceptance:**
  - a failure never yields `Clean`: a test per `Unavailable` reason;
  - `strongest` is order independent (property test);
  - a "no known match" answer carrying a match type, and any malformed body, are `MalformedResponse`, each a
    named test; the provider's test value is `TestValueInProduction`;
  - a body over the byte cap is refused before parsing;
  - no type in this layer converts into anything in layer 5.
- **Depends on:** S2-1 (hash types). Version: 0.3.0 if S2-1 shipped as 0.2.0 (new public API is additive, but
  pre-1.0 the PM may call it either).

## Task S2-4: layer 4, generic report mechanism

- **Scope:** `ReportInput`, `ReportDestination`, `CredentialSlot`, `DestinationAnswer`, `Step`, backoff,
  the "already finished is success" rule, `AwaitingCredentials`, the setup helper that renders generic
  credential prompts. A test-double destination lives in the test tree and carries no real schema.
- **Acceptance:**
  - `Unknown(code)` is never treated as success;
  - a build with no profile loaded reports `no destinations configured`, distinct from a clean state;
  - credentials never appear in `Debug`, `Display` or error text (a test greps the rendered output of every
    public type holding one);
  - backoff matches the consumer's pinned schedule (2, 4, 8 ... capped at 30 minutes);
  - no destination-specific name, field or code in the repository (CI).
- **Depends on:** S2-3 (`HttpRequestSpec`). The first destination's specifics are deploy-time operator
  configuration (Q9, ruled); they are not part of this task.

## Task S2-5: layer 3, evidence and two-person workflow (last)

**Status: implemented (0.2.5).** 23 mutants, all killed. See DESIGN section 3.3.

- **Scope (rulings of 2026-10-10, DESIGN section 3.3):** plain persisted record types and pure
  functions only. `EvidenceState` and its transition table copied from the consumer's upload-scan
  design section 5.7; the case's evidence state *derived* from its files (failed or missing beats
  pending beats present); case kind and review outcome as the rev-3 spec names them; `report_may_submit`
  and `report_file_disposition`; `preserve_until` in one named function using 366 days in seconds
  (every send counts, never moves earlier); `TwoPeople` for the whole two-person set; hold kinds with
  a CSAM-kind hold non-releasable before `preserve_until` even by two people; a recorded
  `DestructionRecorded` after which the case refuses every other write; retention purposes. No store
  traits, no delete capability, no assumption that the uploader is the media owner.
- **Authoritative sources (Community lane, 2026-10-10):** specs `2026-10-01-csam-reporting-pipeline-design.md`
  (case states, report statuses), `2026-10-07-upload-scan-design.md` section 5.7 (evidence states, drain
  rule), and the consumer's CSAM pipeline implementation plan (same directory family) for column intent only.
- **Acceptance:**
  - the transition table is exhaustive and tested row by row against the consumer's table, including
    "no other transition exists" (every unlisted pair refused) and "destruction changes no state";
  - `preserve_until` never moves earlier for any added event (property test) and is exactly 366 days
    after the latest of match, every send and every finish; a one-day-off mutant is killed;
  - every operation in the two-person set refuses equal ids;
  - a held CSAM-kind hold cannot be released early by one or two people; destruction without a
    record is impossible by construction;
  - every rule above has a mutant that is killed.

## Task S2-6: layer 5, classifier seam (any time)

- **Scope:** `Classifier`, `ClassifierSignal`, `ScreenedFrame` token, `screen(...)`.
- **Acceptance:** a `compile_fail` doctest shows a match outcome cannot be passed as a classifier signal and
  that a model-facing function cannot be called without a `ScreenedFrame`; `Unavailable` yields `Hold`.
- **Depends on:** S2-3.

## What the consumer does meanwhile

Keeps its documents authoritative for its own behaviour, writes its adapters against these traits as they
land, and keeps its own contract tests as the cross-check (a divergence between the crate and the consumer's
tests is a finding for both, not a patch to one side).

## Open items that gate tasks

| Item | Gates |
|---|---|
| Upstream PDQ SHA and licence reading | S2-1 start |
| A consumer schema revision to follow | S2-5 start |
