# SafeImage: crate boundary

**Status:** written 2026-10-10 as the 0.1.0 design, before any code, for the PM's gate and for the Community lane to
agree the first move against (STEP 2). Layers 1 to 5 are now built (see `ROADMAP.md` and `CHANGELOG.md`); a line below
that the code does not match is a row in `ROADMAP.md` (A15 to A21), open until it is built or retracted.

**What this decides:** what is inside the crate, what stays in the applications that use it, and the interfaces
between them. It does not decide implementation order inside a layer or any wire detail the sources already fix.

**Sources read** (all at the dates shown; the Community documents are in `ThinkersJournal/community`, public):

| Source | Read at | Used for |
|---|---|---|
| `docs/superpowers/specs/2026-10-01-csam-reporting-pipeline-design.md` (rev 3) | `origin/main` `e5916a6`, 2026-10-10 | intake, case states, report drain, preservation, provider terms (its §11) |
| `docs/superpowers/specs/2026-10-07-upload-scan-design.md` (rev 3 + rev-4 amendments) | same | PDQ, decode seam, fail-closed table, evidence states |
| the consumer's CSAM pipeline implementation plan (same directory family) | same | which plan tasks the crate would replace |
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
1. One reviewed implementation of known-image matching, evidence handling and report filing, usable by
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
| I3 | **Match data never trains or prompts a model.** No type in layers 2–4 implements a conversion into a classifier input, label or prompt; the `classifier` module imports only the frame type, and `Classifier::classify` takes only a frame. The `screening` module consumes a match outcome for a decision and never passes it on. | provider terms (no AI use of match data) |
| I4 | **Known-hash and classifier signals are different types** with no `From` between them. Only a known-hash signal can carry "report at match"; a classifier signal never can. | pipeline §3.1 kinds |
| I5 | **Credentials are opaque.** Secret wrappers have a redacting `Debug`/`Display`, no `Serialize`, and are never read from the environment or disk by the crate. | provider terms (credentials not shared or logged) |
| I6 | **No image bytes in errors, logs or `Debug` output.** Frames and files are wrapped in a type that prints length only. | upload-scan §10.2 |
| I7 | **Preservation only grows.** The preservation end date is a pure function of recorded events and is monotonic: no event moves it earlier. | pipeline §5.1, P2, P5 |
| I8 | **Retention has a named purpose (planned, not built: ROADMAP A20).** Match data held by layer 3 is to carry a purpose tag that storage adapters can enumerate by. No such type exists yet. | provider terms (retention limited to purpose; audit and legal retention named) |
| I9 | **Two-person rule is checked in the crate**, not by the caller. One set of operations takes two distinct actor identities or returns an error: CLEAR of a known-hash case, CLEAR of a removal-request case, the `file_without_evidence` decision, setting and lifting an evidence legal hold, and destruction. | pipeline §7.2, §5.3, P7; upload-scan §5.7 |

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

The layers are modules of one crate with no Cargo features (the only feature is `reference-support`, for the
reference-comparison tests); each layer adds only what it needs, and no layer pulls a runtime.

### 3.1 Layer 1: hashing and normalisation (pure)

**Does:** perceptual hash of a decoded frame, and the arithmetic on hashes.

- `Frame`: raw RGB8, `width`, `height`, stride-checked on construction (`width*height*3 == len`, else error).
  This is the *only* image input to the crate. The decoder is the host's (an out-of-process image service, a
  platform image-transform API, the `image` crate, anything); the crate never sees a container format.
- `pdq(frame) -> Pdq { hash: Hash256, quality: u8 }` and `pdq_dihedral(frame) -> [Pdq; 8]` (the eight
  orientations from one DCT, as the reference does).
- `Hash256`: 32 bytes, `hamming_distance(&Hash256) -> u32`, a hex codec. A profile that needs base64 encodes the 32 bytes itself.
- Constants as **named, overridable defaults**, never hidden literals: `MATCH_DISTANCE_DEFAULT = 31`,
  `QUALITY_FLOOR_DEFAULT = 49` (the reference's published recommendations; a provider applies its own and
  the crate must not pretend otherwise).
- Frame normalisation rule (reference downscale to 512 only when a side exceeds 512, no upscale) is not yet
  a function in this crate (planned, not built: ROADMAP A15); until it is, the rule lives in each host's decoder.
  The first consumer's decoder-service note (native size at or below 512) is the rule.
- Original-bytes digests (`md5`, `sha1`) are not provided: no consumer needs them to compare with a list, and
  they would exist only for that, never for integrity. A host that needs one computes it (ROADMAP A17, retracted).

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

// An async driver trait (`HashMatcher`) is deliberately NOT in 0.1.x: the first consumer needs none (Q6).

pub struct MatchRequest { hashes: Vec<Hash256> /* 1..=8 variants */, kind: HashKind /* Pdq | Md5 | ... */ }

pub enum MatchOutcome {                      // ordered by strength (I2)
    Match(KnownMatch),                       // classification + exact/near + provider details
    Unavailable(Unavailable),                // timeout, 5xx, throttled, bad body, credentials refused, test value
    Clean(CleanBasis),                       // "no known match" is NOT clearance (see below)
}
```

- `KnownMatch.classification` is an enum the *provider's* vocabulary maps into: `Csam`, `HarmfulAbusive`,
  plus `Other(String)` kept verbatim so a new provider value is held, not dropped. `match_type: Exact | Near`.
  **The crate exposes these two facts and nothing more:** whether a near match, or a harmful-abusive
  classification, becomes a `KnownHash` or `Classifier` case is the host's `KindPolicy` function (the
  consumer's decisions Q3 and D5), with no default baked in. Provider-supplied detail is carried as opaque, size-capped
  `Vec<MatchDetail>` (matched file digest, list timestamp) because the case file must preserve it (pipeline §3.5).
- `Unavailable` reasons are a closed enum: `CredentialsRefused`, `Throttled`, `Timeout`, `ServerError`,
  `MalformedResponse`, `TestValueInProduction`, `NotConfigured`, `Disabled`. Each maps to an alarm class in the
  host; the crate does not alarm. `interpret` has two cases that are easy to omit and are each a named
  test: a **"no known match" answer that carries an exact or near match type** is `MalformedResponse`, and
  so is any body that does not parse to a documented shape; the provider's **test value** is
  `TestValueInProduction`.
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
  2. *Planned, not built (ROADMAP A18):* `ListMatcher<S: ListSource>`: local Hamming search over a hash list the *operator* syncs from a
     source they are credentialed with. `ListSource` is a trait; the crate contains no list and no sync client, and
     neither type exists yet.
  3. Concrete `ProviderProtocol` implementations are **profiles** (section 3.6), not part of this repository:
     endpoint paths and field names identify a provider just as a name does. The public crate has the trait,
     the request/response data types, the mock, and a *conformance suite* a profile runs against itself. No
     transport-bound matcher exists in 0.1.x.

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
  there is no setter. The file table is the consumer's upload-scan §5.7, copied rather than re-derived:
  `Pending→Present`; `Pending→Failed` (inline retries exhausted, or the scheduled tick after 10 minutes);
  `Present→Missing` (the daily check); `Failed|Missing→Present` (a later upload of the same bytes);
  `Failed|Missing→FileWithoutEvidence` (**two different admins**); `FileWithoutEvidence→Present`. **Destruction
  changes no state.** A case's evidence state is *derived* from its files, never stored: any `Failed` or
  `Missing` beats any `Pending` beats `Present`.
- `report_may_submit(files)` and `report_file_disposition(state, object_found)` (upload-scan §5.7, ported 1:1).
- `preserve_until(events) -> Option<Instant>`: `None` means keep everything; otherwise one year after the
  latest of match, every send, every finish (pipeline §5.1, the PM's "every send counts"). Monotonic (I7),
  property-tested.
- *Planned, not built (ROADMAP A19):* `viewed_by_esp(file)`, to be derived from "served under this case's grant" or an attested sighting, never from a
  button press (pipeline §3.1).
- Two-person rule (I9): each operation in I9's set takes `TwoPeople::new(actor_a, actor_b)`, which fails on
  equal identities. Classifier-case clearance takes one reviewer, as the pipeline specifies.
- Retention tags (I8) are planned, not built (ROADMAP A20). An `Evidence` record holds digests, provider details and list versions.

**Implemented (S2-5), per the rulings below: plain records and pure functions only.** There are no
store traits and no in-memory store in 0.x; the host persists a `CaseRecord`, calls a function, and
writes the returned record with a compare-and-set on what it read (this is how the pipeline's "every
transition is a guarded write" is met without the crate knowing any SQL). Store traits come when a
host adapter exists to test them against.

| Area | API |
|---|---|
| Evidence states | `EvidenceState`, `EvidenceEvent`, `transition` (the section 5.7 table, 8 transitions, every other pair refused), `case_evidence` (derived, never stored), `report_file_disposition`, `report_may_submit` |
| People | `ActorId`, `TwoPeople::new` (refuses equal ids) |
| Review | `CaseRecord`, `CaseKind`, `ReviewOutcome`, `confirm`, `clear` with `ClearApproval` (two people for known-hash and removal-request, one or two for a classifier case, never a sighting) |
| Holds | `set_evidence_legal_hold`, `lift_evidence_legal_hold` (two people, with a reason), `check_release_hold` for `HoldTarget::ServingKey` and `HoldTarget::EvidenceKey` |
| Destruction | `begin_destruction_on_request` (two people, with the request reference), `reaper_eligibility`, `begin_destruction_expired`; the case then refuses every write. **Recorded only: no function in the crate deletes anything** (a test scans the sources) |
| Clock | `preserve_until(match_at, decided, &[ReportClock])`, `preservation_period_secs()` (366 days) |

**Reading (confirmed by the PM, 2026-10-10; it is also what the written spec says, so it is a reading, not an exception).** The ruling "a CSAM-kind hold is non-releasable before `preserve_until`
even by two people" is applied to the **evidence-key** hold. The **serving-key** hold is released at a
CLEAR, as the rev-3 spec's P2 and section 7.2 step 2 require, but only on a cleared case and only once
the evidence copies are held and verified. Applying the ruling to the serving key would stop a clear
from ever restoring content.

**Does not:** choose a database, assume an object store, or run a scheduler.

**S2-5 rulings (PM, 2026-10-10), which bound what this layer builds:**
1. *Storage boundary.* Plain persisted record types and pure transition and decision functions only;
   the host persists and does the compare-and-set. Store traits only when an adapter exists to test
   against.
2. *Schema to follow.* The consumer's written documents as they stand on its `main` at the time of
   building, copied by name: the rev-3 pipeline spec for case states and report statuses; the
   upload-scan design section 5.7 for evidence states and the drain rule (rev 3 has none; copy from
   5.7 only); the plan's DDL only for column intent, never migration numbers. Do not assume the
   uploader is the media owner: after anonymisation an owner is a bare id.
3. *"1 year".* `preserve_until` uses 366 days in seconds, in one named function, as an application of
   the existing rule "where unclear, take the reading that keeps evidence longer". It is not a new
   legal number; a later ruling changes that one place.
4. *Destruction and holds.* The crate records a destruction (`DestructionRecorded`; the case then
   refuses every other write) and has **no delete capability anywhere**; the host deletes bytes.
   A hold of the CSAM kind is non-releasable before `preserve_until`, even by two people.
5. *Two people.* `TwoPeople::new(a, b)` fails on equal opaque ids. Known limit: one human with two
   accounts defeats it; the host must map one human to one id (section 11).

**Stays in the application** (not extracted): the database schema and triggers (storage-level DELETE guards
are a *belt* under the crate's *braces*), the intake transaction that also hides content and holds accounts,
the admin routes and UI, alarms and email.

**Replaces (first consumer, see §6):** the pure parts of its intake, confirm/clear and evidence-disposition
tasks. The adapter that persists them remains the application's.

### 3.4 Layer 4: report builder for a report destination profile

**Capability claim (the only one this repository makes):** the crate can file reports to the destinations an
operator is registered with, using credentials the operator holds with each. Whether any duty to report
exists, and to whom, differs by jurisdiction and operator; that is the operator's, never the crate's.

**The crate names no destination.** It defines the mechanism; each destination is a **profile** kept outside
this repository (§3.6).

**Does (generic, in this repository):**
- `ReportInput`: reporting person, reported person, incident, files (`digest`, `viewed_by_esp`,
  `publicly_available`), the pages embedding them. **No IP data field exists** unless a host supplies one
  (the type makes absence explicit, not defaulted).
- `trait ReportDestination`: what a profile supplies. It is sans-I/O:
  ```rust
  pub trait ReportDestination {
      fn id(&self) -> &DestinationId;                        // operator-chosen label, opaque to the crate
      fn credential_slots(&self) -> &[CredentialSlot];       // from the LOADED profile (section 3.6)
      fn build(&self, r: &ReportInput) -> Result<BuiltReport, BuildError>;  // no credentials in
      fn request(&self, step: &Step, report: &ReportState, built: &BuiltReport)
          -> Result<HttpRequestSpec, ProtocolError>;                         // none in the spec
      fn interpret(&self, step: &Step, resp: &HttpResponseSpec) -> DestinationAnswer;
      // what is due next is the crate's own pure `next_step(&ReportState, now)`; the transition is
      // `apply(&ReportState, &Step, &DestinationAnswer, now)`; credentials are applied by the host's
      // transport by slot key (implemented in section S2-4; the earlier `plan` method was folded
      // into those two free functions because the state machine is the same for every destination)
  }
  ```
- `BuiltReport` hands back the **exact bytes** to be sent so the host can preserve them; the crate stores
  nothing. Request documents are *built* with an escaper and never parsed.
- `DestinationAnswer` is a closed enum every profile maps into: `Accepted`, `Transient`, `AuthRefused`,
  `Invalid(detail)`, `AlreadyFinished`, `WindowExpired`, `Unknown(code)`. `Unknown` is treated as transient and
  **never as success**. Responses are byte-capped before parsing, with entities off.
- A generic `Step` vocabulary (`Submit | Upload(file) | Describe(file) | Finish | Retract | Wait(until) |
  Hold(reason)`), the guarded-write rule for transitions, backoff (2, 4, 8 ... capped at 30 minutes), and the
  rule that an answer meaning "already finished" is success with the code kept. The host performs the I/O and
  persists between steps, so a retry resumes where it stopped.
- **Report timing is a parameter, and it is two switches** (the consumer's R1): whether a signal queues a
  report at the match (`ReportAtMatch`, supplied by the host) and when filing happens (the host's schedule;
  the step function only says what is due). Classifier and removal-request signals are `false` and **cannot**
  be set true (I4).
- Credentials and reporter identity are inputs (I5). There is no default value; an absent one yields
  `AwaitingCredentials`, never a placeholder.

**Does not:** make HTTP calls, choose a test or production endpoint (profile data plus a host-supplied base
URL; no default), or know any destination's schema, field names, endpoints or result codes.

**Reading:** the timing of a report relative to a match is a reading of the applicable reporting statute,
taken by the first consumer under its owner's ruling. The crate implements the mechanism and asserts no law.

**Replaces (first consumer, see section 6):** its planned destination client, document layer and the
state-machine half of its drain, as a profile plus this generic layer.

### 3.5 Layer 5: classifier seam, separate from known-hash matching

**Implemented (S2-6).** Two modules, so the boundary is visible in the file layout:

- `classifier`: `Classifier` (a sync trait that takes **only a frame**), `ClassifierSignal` (a score
  in 0.0 to 1.0, a model label, the model's own threshold if it reports one), `MockClassifier`.
  This module imports nothing from layers 2 to 4 (a test reads its source to keep it so), and a
  signal has no conversion to or from a match outcome (two `compile_fail` doc tests, plus a source
  check that no `From`/`Into` impl exists). So match data cannot reach a classifier through this
  crate (I3) and a score cannot be filed as a match (I4).
- `screening`: the one place the two meet, as a pure decision over values the host already has.
  `match_request` hashes the frame in all eight orientations and holds a frame at or below the
  quality floor; the host performs the provider exchange; `screen(frame, outcome, signal, policy)`
  decides. A match is `Blocked` (the frame must not reach a model; the host quarantines and reports
  under its own rules), any failure is a `Hold`, and only `Clean` yields `Allow` with a
  `ScreenedFrame` token. A classifier score can only *add* a hold (`ScreenPolicy::classifier_hold_at`,
  off by default); it never rescues a match or a failure. The match outcome is consumed for the
  decision; it is never passed to a classifier.

A model-facing API that takes `&ScreenedFrame` cannot be called with an unscreened frame, because
the token has private fields and no constructor (a `compile_fail` doc test).

**Why a seam and not an implementation:** training or validating a CSAM classifier needs
possession of the material, which is excluded for every project here. Classifiers are vendor
products behind this trait, later, review-first only (options document 2.6, 2.10). The crate
ships no model.

### 3.6 Profiles: where destination and provider specifics live

A **profile** carries everything that identifies a specific destination or hash-match provider: its schema, field
mapping, endpoints, result codes, and the human-readable labels of its credential slots. None of it is in this
repository, in any file, test name, comment or commit; CI enforces that (section 8).

| | |
|---|---|
| **Destination profile** | implements `ReportDestination` (section 3.4) |
| **Provider profile** | implements `ProviderProtocol` (section 3.2) |
| **Form and home** | deploy-time operator configuration (Q9, ruled 2026-10-16): the operator supplies the destination and provider specifics when deploying; they are never in a public repository, and the crate asks for credentials generically |
| **Vendored schemas** | live only with the operator's configuration, never here; their licences are recorded there |

**Asking for credentials without naming anyone.** The crate exposes:
- `credential_slots()` from each *loaded* profile: `CredentialSlot { key, kind: Secret | Text | Url, required }`
  plus an optional `label` that only the profile supplies;
- a setup/doctor helper that renders generic prompts from them, e.g. *"Credentials for reporting destination
  `<id>`, slot `<key>` (your registration documents say what to enter)"*, where `<id>` is the operator's own
  label. A profile's `label`, when loaded, replaces the generic text, so a specific name appears only where
  the operator already holds the profile;
- the same mechanism for provider profiles.

A build with no profile loaded prompts for nothing and reports `no destinations configured`, a distinct and
loud state: an unconfigured crate must not look like a clean one.

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
| Secrets, reporter identity, test-vs-production selection | the operator's |
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

**Acceptance criteria for any WASM build** (from the consumer's workerd gate, 2026-10-10; the first code PR
meets them before option B can be chosen):
- instantiated from an imported module: the host does not compile WASM from bytes at runtime;
- synchronous initialisation; no `fetch`, no `new URL(import.meta.url)`, no `node:` imports in the glue;
- pure bytes in, bytes out; typed errors, never a panic across the boundary;
- built from `no_std + alloc` layers only (section 3).

---

## 6. STEP 2 proposal: first move, interface first

Agreed with the Community lane before any code moves; this section is the SafeImage lane's proposal.

1. **Freeze the interface, not the implementation.** Land the *types and traits* of layers 1 and 2 (`Frame`,
   `Hash256`, `Pdq`, `MatchRequest`, `MatchOutcome`, `strongest`, `HashMatcher`, `MockMatcher`) with their
   invariant tests, behaviour-free where possible. The Community's US3 mock and client are then written
   against these.
2. **Port PDQ into layer 1 (this is Community US1, done here).** Same vetting steps (pin upstream SHA, read
   each file's header, licence verbatim, `NOTICE` updated in the same change). Fixtures from generated images.
3. **Layer 4's generic half** (`ReportDestination`, `Step`, answer enum, backoff) here, and the first
   destination profile in the operator's deploy-time configuration, with the Community supplying the test-environment run.
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
| Use only to remove or filter CSAM on our own service | no `export`/`share` API on match data; purpose-tagged retention (I8) is planned, not built (A20) |
| Match data never trains AI or feeds generative AI | I3: no conversion path; layer 5 isolated |
| Credentials not shared or logged | I5 |
| "No known match" is not clearance; duty stays ours | `CleanBasis` docs; no workflow treats `Clean` as a decision |
| Outage or suspension must fail closed | I1 and the `Unavailable` reasons |
| May send removal requests later, even after "no match" | `Kind::RemovalRequest` case entry, same clock as a known-hash match, never auto-filed (pipeline §3.1 d) |
| Notify the provider of a credential leak or breach | operator runbook item; the crate exposes `Unavailable::CredentialsRefused` so a revoked credential is visible |
| Retain match data only as long as the purpose needs | planned, not built (A20): purposes `Audit`, `LegalPreservation`, enumerable by adapters for deletion. Today only `preserve_until` and the reaper rules bound retention |
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
- `cargo deny` (advisories, licences, sources: crates.io only, no `git =` deps) is in CI (`libm` is the one dependency).

## 9. Release and versioning

- One version for the crate (portfolio rule). Pre-1.0 a breaking change raises the **second** number.
- `safeimage` is published to crates.io by the PM (Q8); `safeimage-wasm` stays `publish = false`. The repository is
  public and `main` is protected.
- The PM allocates version numbers at gate time; this PR carries 0.1.0 as the repo's first.
- Dependencies at their latest versions; crates.io sources only.

## 10. Open questions

| # | Question | Who decides | Blocks |
|---|---|---|---|
| Q1 | **Decided 2026-10-10 (PM):** sans-I/O core, with a wasm-bindgen build of the pure layers for the TypeScript consumer; proof is the consumer's workerd gate (Q7). Option A (crate on the decoder service) stays available. | PM | - |
| Q2 | Does the provider accept hash-only lookup with multi-variant arrays, and does it match rotations itself? (Community US3's own implementer check.) | Community lane via the provider's API document | `MatchRequest` shape: one hash or eight |
| Q3 | *Withdrawn from this repository on 2026-10-10; kept in the operator's private notes.* | - | - |
| Q4 | Which crate-internal licence mix for Meta-derived files (BSD-3-Clause kept per file, rest MIT OR Apache-2.0)? Proposed as in `NOTICE` | PM | step 2 of §6 |
| Q5 | Does Lightbulb need layer 5's `ScreenedFrame` token at the API boundary, or at its request handler only? | Lightbulb lane | layer 5's final shape |
| Q6 | **Resolved by both lanes:** sans-I/O core only in 0.1.x; no `async` driver or transport-bound matcher until a consumer asks | - | - |
| Q7 | **Passed 2026-10-10** (Community lane, real workerd via vitest-pool-workers, `CompiledWasm`): synchronous `Instance`, no imports, exact exports, 18 reference frames bit-exact, error codes returned with no trap, no memory growth over 20 dihedral calls at 512x512; about 10 ms per call on a local `wrangler dev` (at the free plan CPU cap), 25.9 KiB upload. Option B is viable; the binding needed no change. Not tested: frames above 512 px, SIMD or other targets | Community lane | - |
| Q8 | **Ruled 2026-10-16 (board 174, CireSnave):** `safeimage` is published to crates.io at the first code release (the PM publishes), and an npm package is built from the WASM layers (`docs/NPM-PACKAGE.md`). `safeimage-wasm` stays `publish = false` | CireSnave | - |
| Q9 | **Ruled 2026-10-16 (board 174, CireSnave), option B:** destination and provider specifics are deploy-time operator configuration, never in a public repository; the crate asks for credentials generically (section 3.6). No profiles repository | CireSnave | - |
| Q10 | **Ruled 2026-10-16 (board 174, CireSnave):** a TypeScript consumer gets an npm package built from the CI artifact, not a vendored `.wasm` (`docs/NPM-PACKAGE.md`) | CireSnave | - |

---

## 11. Known limits (read before relying on this crate)

These are things the crate does **not** do, stated so an operator does not assume otherwise. Each is
either impossible without `unsafe` or a dependency, outside what a library can see, or a measured
boundary.

| Limit | Consequence | What the operator does |
|---|---|---|
| **Secrets are not zeroed on drop.** `Secret` redacts `Debug`, has no `Display`, `Serialize` or `Clone`, but its bytes remain in memory until the allocator reuses them (zeroing needs `unsafe` or a dependency). | A memory dump of a running process can contain a credential. | Treat the process as the trust boundary; keep dumps and swap protected. |
| **`Person` and `Address` hold plain strings in memory.** Only their `Debug` output is redacted. | Personal data is in process memory and in anything the host serialises itself. | Do not log or serialise `ReportInput` yourself; build the document through the profile. |
| **Redaction is of this crate's own output.** | A host that formats a credential or a person's fields into its own log line is not protected. | Log slot keys and status codes, never values. |
| **Two-person checks compare opaque ids.** | One human with two accounts passes as two people. Nothing in a library can see that. | Map one human to one id in the host's authentication, and audit the mapping. |
| **A frame whose width and height were swapped but whose byte length still matches cannot be detected.** | A wrongly oriented buffer hashes to a different, wrong value, which can fail to match. | The decoder must report the dimensions of the buffer it actually produced; test it on rotated and flipped inputs. |
| **Frames up to 16.7 million pixels are accepted.** Working memory is about 8 bytes per pixel on top of the frame. | A large frame can exhaust a small runtime. | Cap the decode size lower in the host (the first consumer caps at 512 pixels). |
| **The DCT matrix uses `libm` cosine, the reference uses the platform's.** Bit-exact agreement is proven on the fixture frames (the matrix feeds every hash), not for all inputs. | A different compiler or platform is a different fixture run. | Re-run `tests/reference/build.sh` when the toolchain or the pinned reference changes. |
| **A "no known match" answer is not clearance.** | Nothing here proves an image is acceptable. | Keep the other layers of defence; the duty stays with the operator. |
| **Retry and backoff numbers are defaults.** | They are the first consumer's pinned values, not a statement about any destination's rules. | Set them from the destination's own documentation. |
| **No destination or provider is implemented in this repository.** | An unconfigured build reports `NoDestinationsConfigured`; it cannot file or match anything. | Load a profile (section 3.6). |
| **No legal conclusion is asserted.** | Mechanisms follow readings marked as readings in the first consumer's design; there is no attorney on this portfolio. | The operator owns the legal position. |
