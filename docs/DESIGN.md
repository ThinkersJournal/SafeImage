# SafeImage: crate boundary

**Status:** 0.1.0, design only. Nothing in this document is built. Written 2026-10-10 by the SafeImage lane, for
the PM's gate and for the Community lane to agree the first move against (STEP 2).

**What this decides:** what is inside the crate, what stays in the applications that use it, and the interfaces
between them. It does not decide implementation order inside a layer or any wire detail the sources already fix.

**Sources read** (all at the dates shown; the Community documents are in `ThinkersJournal/community`, public):

| Source | Read at | Used for |
|---|---|---|
| `docs/superpowers/specs/2026-10-01-csam-reporting-pipeline-design.md` (rev 3) | `origin/main` `e5916a6`, 2026-10-10 | intake, case states, report drain, preservation, provider terms (its §11) |
| `docs/superpowers/specs/2026-10-07-upload-scan-design.md` (rev 3 + rev-4 amendments) | same | PDQ, decode seam, fail-closed table, evidence states |
| `docs/superpowers/plans/2026-10-01-csam-ncmec-pipeline.md` | same | which plan tasks the crate would replace |
| the self-scanning options document and the provider-terms reading notes (portfolio level, not in this repo) | 2026-10-10 | the option space and the terms the design must meet |

**Rules this repo keeps, from the PM's task and the portfolio rules:**
- The hash-match provider's name appears in no file, test name or commit here. The neutral alias is
  **"hash-match provider"** (code identifier: `provider`). CI scans for it (`.github/forbidden-patterns.txt`).
- No credential, token, key or hash list is ever in the repository, including test data.
- No legal conclusion is asserted. There is no attorney on this portfolio; where a reading of the law shapes a
  mechanism it is marked **Reading:** and the operator owns it.

---

## 1. Goals and non-goals

**Goals**
1. One reviewed implementation of known-image matching, evidence handling and CyberTipline reporting, usable by
   any application that accepts images (first consumer: Thinker's Journal Community; next: the inference
   server Lightbulb and the Fuel engine, for pre-model screening).
2. Make the dangerous mistakes unrepresentable: an unavailable scanner reading as "clean", a match being lost
   because a weaker answer arrived later, a classifier flag being filed as if it were a hash match, a credential
   reaching a log.
3. Assume nothing about the host: no database, object store, queue, web framework or async runtime in the core.

**Non-goals**
- Distributing hash lists, or any code that fetches one without operator-supplied credentials.
- Decoding images. The crate takes a decoded frame with explicit dimensions (§3.1).
- Training or evaluating classifiers. That needs the material itself, which no project here may possess.
- Moderation policy (who may ban, what a user is told). That is the application's.

---

## 2. Invariants that cut across layers

These are properties of the types, not of documentation. Each has a test in the layer that owns it.

| # | Invariant | Where it comes from |
|---|---|---|
| I1 | **Fail closed.** A match outcome has a distinct `Unavailable` arm; no function converts it to `Clean`. A caller that cannot scan must hold the image. | Community upload-scan §6.1; provider terms: suspension or outage must never publish |
| I2 | **A match outranks every other answer** for the same image (any rotation variant, any provider). Ordering: `Match` > `Unavailable` > `Clean`. | upload-scan §3.5 / PM ruling N1 |
| I3 | **Match data never trains or prompts a model.** No type in layers 2–4 implements a conversion into a classifier input, label or prompt, and layer 5 has no import path from them. | provider terms (no AI use of match data) |
| I4 | **Known-hash and classifier signals are different types** with no `From` between them. Only a known-hash signal can carry "report at match"; a classifier signal never can. | pipeline §3.1 kinds |
| I5 | **Credentials are opaque.** Secret wrappers have a redacting `Debug`/`Display`, no `Serialize`, and are never read from the environment or disk by the crate. | provider terms (credentials not shared or logged) |
| I6 | **No image bytes in errors, logs or `Debug` output.** Frames and files are wrapped in a type that prints length only. | upload-scan §10.2 |
| I7 | **Preservation only grows.** The preservation end date is a pure function of recorded events and is monotonic: no event moves it earlier. | pipeline §5.1, P2, P5 |
| I8 | **Retention has a named purpose.** Match data held by layer 3 carries a purpose tag; storage adapters can enumerate by it. | provider terms (retention limited to purpose; audit and legal retention named) |
| I9 | **Two-person rule is checked in the crate**, not by the caller: the clearing and release operations take two distinct actor identities or return an error. | pipeline §7.2 |

---

## 3. The five layers

Dependency direction is strictly downward in this list, except layer 5, which depends on layer 1 only.

```
                 application (Community, Lightbulb, ...)
                          │ supplies: decoded frames, storage, transport, policy, secrets
   ┌──────────────────────┼───────────────────────────────────────────┐
   │ 4 report   ──► 3 evidence ──► 2 matcher ──► 1 hash               │
   │                                                    ▲             │
   │ 5 classifier seam  ────────────────────────────────┘             │
   │   (no edge to 2, 3 or 4; the application joins them)             │
   └──────────────────────────────────────────────────────────────────┘
```

Each layer ships as a Cargo feature (`hash`, `matcher`, `evidence`, `report`, `classifier`). `hash` has no
dependencies beyond `core`/`alloc`. Higher layers add only what they need; no layer pulls a runtime.

### 3.1 Layer 1: hashing and normalisation (pure)

**Does:** perceptual hash of a decoded frame, and the arithmetic on hashes.

- `Frame`: raw RGB8, `width`, `height`, stride-checked on construction (`width*height*3 == len`, else error).
  This is the *only* image input to the crate. The decoder is the host's (an out-of-process image service, a
  platform image-transform API, the `image` crate, anything); the crate never sees a container format.
- `pdq(frame) -> Pdq { hash: Hash256, quality: u8 }` and `pdq_dihedral(frame) -> [Pdq; 8]` (the eight
  orientations from one DCT, as the reference does).
- `Hash256`: 32 bytes, `hamming(&Hash256) -> u32`, base64 and hex codecs (the provider interface takes base64).
- Constants as **named, overridable defaults**, never hidden literals: `MATCH_DISTANCE_DEFAULT = 31`,
  `QUALITY_FLOOR_DEFAULT = 49` (the reference's published recommendations; a provider applies its own and
  the crate must not pretend otherwise).
- Frame normalisation rule (reference downscale to 512 only when a side exceeds 512, no upscale) is specified
  here as `normalise_geometry(w, h)` so every consumer's decoder is checked against one definition. The
  first consumer's decoder-service note (native size at or below 512) is the rule.
- Original-bytes digests (`md5`, `sha1`, `sha256`) of an upload, behind a `legacy-digests` feature because MD5
  and SHA-1 exist only to compare with lists that use them, never for integrity.

**Does not:** decode, resize with a library, touch a file, or know what a provider is.

**Correctness control:** a fixture suite of hashes produced by the reference implementation for **generated**
images (never upstream data files unless their licence is read and recorded in `NOTICE`), plus the
reference's own tolerance: quality ≥ 80 within distance ≤ 10, byte-exact on identical RGB input.

**Replaces (first consumer, see §6):** its planned TypeScript PDQ port (not built, not released).

### 3.2 Layer 2: matcher interface, operator-supplied credentials, providers behind a trait

**Does:** one way to ask "is this hash known?", whoever answers.

The core is **sans-I/O**: it never opens a socket, reads a clock or awaits. A provider is a pair of pure
functions, and the host performs the exchange in between (§5):

```rust
pub trait ProviderProtocol {
    /// Pure. Builds the request the host must send; credentials are applied by the host's transport.
    fn request(&self, req: &MatchRequest) -> Result<HttpRequestSpec, ProtocolError>;
    /// Pure. Never panics, never returns Clean on a failure (I1). The byte cap is applied before parsing.
    fn interpret(&self, req: &MatchRequest, resp: &HttpResponseSpec) -> MatchOutcome;
}

/// Optional `async` feature: a convenience driver, not the contract.
pub trait HashMatcher {
    fn lookup(&self, req: &MatchRequest) -> impl Future<Output = MatchOutcome>;
}

pub struct MatchRequest { hashes: Vec<Hash256> /* 1..=8 variants */, kind: HashKind /* Pdq | Md5 | ... */ }

pub enum MatchOutcome {                      // ordered by strength (I2)
    Match(KnownMatch),                       // classification + exact/near + provider details
    Unavailable(Unavailable),                // timeout, 5xx, throttled, bad body, credentials refused, test value
    Clean(CleanBasis),                       // "no known match" is NOT clearance (see below)
}
```

- `KnownMatch.classification` is an enum the *provider's* vocabulary maps into: `Csam`, `HarmfulAbusive`
  (never auto-reported by default; see layer 4), plus `Other(String)` kept verbatim so a new provider value is
  held, not dropped. `match_type: Exact | Near`. Provider-supplied detail is carried as opaque, size-capped
  `Vec<MatchDetail>` (matched file digest, list timestamp) because the case file must preserve it (pipeline §3.5).
- `Unavailable` reasons are a closed enum: `CredentialsRefused`, `Throttled`, `Timeout`, `ServerError`,
  `MalformedResponse`, `TestValueInProduction`, `NotConfigured`, `Disabled`. Each maps to an alarm class in the
  host; the crate does not alarm.
- `CleanBasis` records provider, list version (`updated_at`), request time. **`Clean` is "no known match", not
  clearance**: the type's docs say so, and the layer-3 case workflow has no path that treats `Clean` as a
  decision.
- `strongest(outcomes) -> MatchOutcome` implements I2 and is the only combinator; the multi-variant and
  multi-provider callers use it.
- **Credentials:** `ProviderCredentials` is a secret wrapper (I5) constructed by the host from its own secret
  store. The crate has no `from_env`.
- **Transport:** none in the core. `HttpRequestSpec`/`HttpResponseSpec` are plain data (method, path,
  headers, body; status, body). `interpret` applies a **byte cap before parsing** (default 64 KiB,
  configurable), never follows redirects and processes no entities. Timeouts belong to the host; the crate
  exposes `RetryPolicy::decide(attempt, outcome, elapsed) -> RetryDecision`, a pure function, because the
  consumer's design pins "one retry inside the caller's deadline".
- **Implementations, in dependency order:**
  1. `MockMatcher` (always ships): scripted outcomes, used by every downstream test. Not behind a test cfg, so
     consumers can use it.
  2. `ListMatcher<S: ListSource>`: local Hamming search over a hash list the *operator* syncs (NCMEC hash
     sharing, when credentialed). `ListSource` is a trait; the crate contains no list and no sync client in
     0.1.x.
  3. `ProviderProtocol` implementation for the hash-only HTTP provider. Name-neutral module (`provider`),
     documented endpoints by role (`hash lookup`, `bytes lookup`, `list version`), never by vendor. A
     transport-bound `ProviderMatcher<T>` exists only under the `async` feature.

**Does not:** decide to quarantine, report or ban. Does not log request bodies.

**Replaces (first consumer, see §6):** its planned provider client, mock matcher and `strongest` rule.

### 3.3 Layer 3: evidence store and two-person-rule workflow, pluggable storage

This is the layer with the most coupling to a host, so it is deliberately the thinnest: **the crate owns the
state machines and decision functions; the host owns the transaction and the SQL.**

**Does (pure, over plain data):**
- Case and file state machines with total transition tables: `EvidenceState`
  (`Pending | Present | Failed | Missing | FileWithoutEvidence`), case `Kind`
  (`KnownHash | Classifier | RemovalRequest | ModeratorSighting`), `ReviewOutcome`, priority. The
  transition function takes the current state and an event and returns the next state or a typed refusal;
  there is no setter.
- `report_may_submit(files)` and `report_file_disposition(state, object_found)` (upload-scan §5.7, ported 1:1).
- `preserve_until(events) -> Option<Instant>`: `None` means keep everything; otherwise one year after the
  latest of match, every send, every finish (pipeline §5.1, the PM's "every send counts"). Monotonic (I7),
  property-tested.
- `viewed_by_esp(file)`: derived from "served under this case's grant" or an attested sighting, never from a
  button press (pipeline §3.1).
- Two-person rule (I9): `clear(case, actor_a, actor_b)` requires distinct identities for known-hash cases and
  one reviewer for classifier cases, as the pipeline specifies.
- Retention tags (I8) and an `Evidence` record holding digests, provider details and list versions.

**Does (by trait, host implements).** These are *ports* used by the optional `async` driver and by the
in-memory test store; the pure core above never calls them. A host that drives the state machines itself
(for example from TypeScript through a WASM build) uses the transition functions and ignores the traits.

```rust
pub trait EvidenceStore {           // blobs
    fn put(&self, key: &EvidenceKey, bytes: Bytes) -> impl Future<Output = Result<(), StoreError>>;
    fn head(&self, key: &EvidenceKey) -> impl Future<Output = Result<Option<ObjectInfo>, StoreError>>;
    fn get(&self, key: &EvidenceKey) -> impl Future<Output = Result<Bytes, StoreError>>;
    // no delete: destruction is a separate, two-person trait method (below)
}
pub trait CaseStore {               // records; the host makes each call atomic or wraps a unit of work
    fn load(&self, id: CaseId) -> impl Future<Output = Result<Option<Case>, StoreError>>;
    fn save(&self, case: &Case, expected_version: u64) -> impl Future<Output = Result<(), StoreError>>;
}
```

`save` is optimistic (version check), which is how the pipeline's "every drain transition is a guarded write"
is expressed without the crate knowing about `UPDATE ... WHERE status = $1`. Destruction under the statute's
law-enforcement-request exception is a distinct `destroy(case, request_ref, actor_a, actor_b)` that only
exists on a trait the host opts into.

**Does not:** choose a database, assume an object store, or run a scheduler. It ships an in-memory store
(`MemoryStore`) for tests and examples.

**Stays in the application** (not extracted): the database schema and triggers (storage-level DELETE guards
are a *belt* under the crate's *braces*), the intake transaction that also hides content and holds accounts,
the admin routes and UI, alarms and email.

**Replaces (first consumer, see §6):** the pure parts of its intake, confirm/clear and evidence-disposition
tasks. The adapter that persists them remains the application's.

### 3.4 Layer 4: report builder (NCMEC CyberTipline)

**Does:**
- `ReportInput`: reporting person, reported person, incident, files (`digest`, `viewed_by_esp`,
  `publicly_available`), the web pages embedding them. **No IP data field exists** unless a host supplies one
  (an application that stores none leaves the field out; the type makes absence explicit, not defaulted).
- `build_report_xml(&ReportInput) -> Result<Vec<u8>, BuildError>`: request XML is *built* with an escaper and
  never parsed; the optional address group is emitted only when all five parts are present (pipeline §4.4).
  Validated against the NCMEC XSD in the crate's tests once the schema is vendored from NCMEC's own endpoint
  with its licence recorded; until then the builder is covered by the host's exttest run, not claimed correct.
- `parse_response(&[u8]) -> Result<ApiResponse, ParseError>`: byte-capped, entities off, only the documented
  codes (`2000/3100` auth, `4100` validation, `5001` deletion window, `5102` already finished, `1000` server)
  mapped to a closed enum; unknown codes are `Unknown(code)` and are treated as transient, never as success.
- The drain as a **step function**: `next_step(report, now) -> Step`
  (`Submit | Upload(file) | FileInfo(file) | Finish | Retract | Wait(until) | Hold(reason)`), and
  `apply(report, step_result) -> report`. The host performs the I/O and persists between steps, so a retry
  resumes where it stopped (pipeline §4.2). Backoff (2, 4, 8 … capped at 30 minutes), the deletion window
  rule, and `5102 ⇒ finished` live here.
- **Report timing is a parameter, not a constant:** `ReportAtMatch: { KnownHash: bool, ... }` supplied by the
  host. Classifier and removal-request signals are `false` and **cannot** be set true (I4). An operator's
  ruling to report known-hash matches at match time is *its* configuration value.
- Credentials and reporter identity are inputs (I5); there is no default value and an absent one yields
  `AwaitingCredentials`, never a placeholder.

**Does not:** make HTTP calls (host transport), store `request_xml` (it hands the exact bytes back for the
host to preserve), or choose exttest vs production (host-supplied base URL; no default).

**Reading:** the "report at match for a known-hash signal" timing is a reading of 18 U.S.C. §2258A(a), taken
by the first consumer under its owner's ruling. The crate implements the mechanism and does not assert the law.

**Replaces (first consumer, see §6):** its planned XML layer, NCMEC client and the state-machine half of the
drain.

### 3.5 Layer 5: classifier seam, separate from known-hash matching

**Does:** define where a model-based score enters, and keep it apart.

```rust
pub trait Classifier {
    type Error;
    fn classify(&self, frame: &Frame) -> impl Future<Output = Result<ClassifierSignal, Self::Error>>;
}
pub struct ClassifierSignal { score: f32, model: ModelId, threshold_applied: Option<f32> }
```

- `ClassifierSignal` has **no** `From`/`Into` to or from `MatchOutcome` (I4) and no `report_at_match`
  capability. A host that wants "classifier says yes, route to review" writes that mapping itself, in its
  own code, visibly.
- No type from layers 2 to 4 is importable here (I3), enforced by feature layout and a `compile_fail` doctest.
- **Pre-model screening** (Lightbulb, Fuel): `screen(frame, &impl HashMatcher, &impl Classifier) ->
  Screening`, where `Screening` is `Allow | Hold(reason) | Refuse`. It runs the matcher first, treats
  `Unavailable` as `Hold` (I1), and passes the classifier signal alongside without merging it. A `Match` or
  `Hold` means the image **must not be passed to a model**; the type returned from `screen` is the only way
  to obtain the `ScreenedFrame` token that the model-facing API accepts, so the order is checked by the
  compiler.
- Match data never reaches a classifier (I3). A model that *produces* signals (e.g. a safety model hosted by the
  application) is the application's business; the crate ships no model.

**Why a seam and not an implementation:** training or validating a CSAM classifier needs possession of the
material, which is excluded for every project here. Providers of such classifiers are vendor products behind
this trait, later, review-first only (options document §2.6, §2.10).

**Replaces:** nothing today (the classifier source is "later" in the first consumer's documents). It exists now
so Lightbulb/Fuel integrate against the same boundary rather than inventing a second one.

---

## 4. What stays in the applications

| Stays in the application | Why |
|---|---|
| Decoding and resizing | host-specific and security-sensitive |
| SQL, migrations, storage-level guards, transactions spanning content, accounts and cases | the crate cannot own a host's transaction |
| Object storage layout, public vs restricted placement, content addressing | host storage |
| Scheduling and queues, alarm delivery (banner, email), rollout flags | host runtime |
| Admin routes, review UI, user-facing responses (503/422 text), privacy-policy wording | product |
| Moderation policy: bans, terminate, appeals, account holds | product and legal position |
| Secrets, reporter identity, exttest-vs-production selection | the operator's |
| The provider disclosure sentence and any public statement about the provider | the operator's terms |

---

## 5. Runtime reality: the first consumer is TypeScript on an edge-worker runtime

The first consumer (Thinker's Journal Community) is TypeScript on Cloudflare Workers. A Rust crate does not run there unless it is compiled to
WASM or reached as a service. This must be settled before STEP 2 moves anything, and is cheap to settle now
because **none of the Community's scan or report code is built yet** (US1 to US3 and plan Tasks 4 to 9 are
designs; main has the schema for original-upload hashes only).

| Option | What moves | Cost | Risk |
|---|---|---|---|
| **A. Rust on the image service, TS Worker calls it** | layers 1 and 2 run beside the decoder (the consumer's planned decoder service is already the trust boundary for raw frames); the Worker sends a bytes request, gets a signed verdict | the service already exists in its design; one new route | the service holds the provider credential (today a Worker secret), so a secret moves to the service host |
| **B. WASM build of layers 1, 3 (pure), 4** | hashing and state machines compile to `wasm32-unknown-unknown`, loaded in the Worker | one build target; the pure core must stay `no_std + alloc` | workerd WASM limits (128 MB isolate) and unverified glue; layers 1 and 3 are small and have no I/O |
| **C. Keep TS ports, crate is the reference** | crate and TS port both exist; conformance fixtures shared | duplicates work, drifts | the exact failure the pure-Rust approach avoids |

**Recommendation (a position, not a ruling):** B for layers 1, 3 and 4 (pure, small, no I/O: they compile to
WASM with `#![no_std]` discipline), A for the provider client only if the credential should leave Workers.
Layer 2 needs no HTTP client in WASM at all, because a Worker already has `fetch`: the sans-I/O
`ProviderProtocol` builds the request and interprets the response, and the Worker's `fetch` sits between
them. **Design constraint this puts on the crate:** layers 1,
3, 4 and the matcher's *parsing and combination logic* are `no_std + alloc`, with no `std::time`, no threads,
no `tokio`; time and I/O arrive as arguments. This is a constraint the design adopts now whichever option
wins, because it is also what makes the layers testable without a host.

---

## 6. STEP 2 proposal: first move, interface first

Agreed with the Community lane before any code moves; this section is the SafeImage lane's proposal.

1. **Freeze the interface, not the implementation.** Land the *types and traits* of layers 1 and 2 (`Frame`,
   `Hash256`, `Pdq`, `MatchRequest`, `MatchOutcome`, `strongest`, `HashMatcher`, `MockMatcher`) with their
   invariant tests, behaviour-free where possible. The Community's US3 mock and client are then written
   against these.
2. **Port PDQ into layer 1 (this is Community US1, done here).** Same vetting steps (pin upstream SHA, read
   each file's header, licence verbatim, `NOTICE` updated in the same change). Fixtures from generated images.
3. **Layer 4's pure half** (XML builder, response parser, step function), replacing plan Tasks 4 and 5's
   parser and client logic, with the Community supplying the exttest run.
4. **Layer 3's pure half** last, because it is coupled to the Community's still-moving case schema (it has
   revision 3 plus amendments and no code). Do not extract it from a schema that is still changing.
5. Layer 5 is a trait and a token type; it can land any time and costs nothing to the Community.

Each step is a PR in this repo with its own version (pre-1.0: breaking = second number).

---

## 7. Provider and legal terms, as mechanisms

The terms the portfolio has read for the hash-match provider (reading notes, 2024-11-13 terms version, read
2026-10-05) are encoded as behaviour **without naming the provider**:

| Term | Mechanism here |
|---|---|
| Use only to remove or filter CSAM on our own service | no `export`/`share` API on match data; layer 3 retention tagged with purpose (I8) |
| Match data never trains AI or feeds generative AI | I3: no conversion path; layer 5 isolated |
| Credentials not shared or logged | I5 |
| "No known match" is not clearance; duty stays ours | `CleanBasis` docs; no workflow treats `Clean` as a decision |
| Outage or suspension must fail closed | I1 and the `Unavailable` reasons |
| May send removal requests later, even after "no match" | `Kind::RemovalRequest` case entry, same clock as a known-hash match, never auto-filed (pipeline §3.1 d) |
| Notify the provider of a credential leak or breach | operator runbook item; the crate exposes `Unavailable::CredentialsRefused` so a revoked credential is visible |
| Retain match data only as long as the purpose needs | layer 3 purposes: `Audit`, `LegalPreservation`; adapters can enumerate by purpose for deletion |
| Public statements about the provider limited to one sentence | the crate says nothing; CI forbids the name here |

This table records mechanisms. It is not a statement that any operator's use complies; the operator carries
that and re-reads the terms at each release.

---

## 8. Testing and CI

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` on Ubuntu and Windows;
  an MSRV job reading `rust-version` from the manifest (same shape as the portfolio's other Rust CI).
- A tree scan that fails on the provider's name, key headers and token shapes, with positive controls built in
  the job so a broken pattern file cannot pass silently.
- No test uses real abusive material, anything claiming to resemble it, or a real hash from a list. Hash tests
  use generated images and reference-implementation outputs for them. Provider tests use `MockMatcher` and a
  recorded-shape fixture written by hand from the public OpenAPI document, never a live call.
- `cargo deny` (advisories, licences, sources: crates.io only, no `git =` deps) is added with the first
  dependency; 0.1.0 has none to check.

## 9. Release and versioning

- One version for the crate (portfolio rule). Pre-1.0 a breaking change raises the **second** number.
- `publish = false` until the PM and CireSnave release it. The repository stays private; visibility and branch
  protection are the PM's step.
- The PM allocates version numbers at gate time; this PR carries 0.1.0 as the repo's first.
- Dependencies at their latest versions; crates.io sources only.

## 10. Open questions

| # | Question | Who decides | Blocks |
|---|---|---|---|
| Q1 | Option A, B or C in §5 (how a TypeScript consumer uses a Rust crate) | PM with the Community lane | STEP 2 step 2 onward |
| Q2 | Does the provider accept hash-only lookup with multi-variant arrays, and does it match rotations itself? (Community US3's own implementer check.) | Community lane via the provider's API document | `MatchRequest` shape: one hash or eight |
| Q3 | Does the NCMEC hash-sharing credential exist and carry PDQ? Decides whether `ListMatcher` ships in 0.1.x or later | CireSnave, via the PM's board | `ListMatcher` priority only |
| Q4 | Which crate-internal licence mix for Meta-derived files (BSD-3-Clause kept per file, rest MIT OR Apache-2.0)? Proposed as in `NOTICE` | PM | step 2 of §6 |
| Q5 | Does Lightbulb need layer 5's `ScreenedFrame` token at the API boundary, or at its request handler only? | Lightbulb lane | layer 5's final shape |
| Q6 | Sans-I/O core with an optional `async` driver (as drafted in §3.2 to §3.4), or sans-I/O only? Both lanes lean to sans-I/O core; the driver is the open part | SafeImage and Community lanes | whether the `async` feature exists in 0.1.x |
| Q7 | Does a workerd load of the WASM build pass the consumer's workerd gate? Needed before option B is chosen over A | Community lane (proof) | Q1 |
