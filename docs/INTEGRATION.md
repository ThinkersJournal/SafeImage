# Integrating SafeImage into a host application

For the engineer wiring the crate into an upload path. `docs/DESIGN.md` says why the layers are shaped
as they are; this page says what you call, in what order, and what stays yours. The runnable version of
everything below is [`examples/end_to_end.rs`](../examples/end_to_end.rs) (`cargo run --example end_to_end`).

## The one idea

SafeImage is **sans-I/O**. It never opens a socket, reads a clock, touches a disk or spawns anything.
Every function takes its inputs as arguments (including the current time as a number) and returns a
value. **You** perform each exchange and persist each result. If you remember one thing: the crate
decides, the host acts.

## What you supply

| You supply | Why the crate cannot |
|---|---|
| A **decoded frame**: RGB bytes with explicit width and height (`hash::Frame::new`) | Decoding is host-specific and security-sensitive. The crate does no decoding. |
| The **transport**: send the `HttpRequestSpec` it builds, return an `HttpResponseSpec` | No network in the crate. Apply credentials in your transport; the crate never reads them from the environment or disk. |
| The **clock**: a `u64` timestamp on every call that needs one | No clock in the crate, so every decision is reproducible in a test. |
| **Storage**: persist the records the crate returns | The crate cannot own your transaction. Records are plain data; nothing in the crate can delete. |
| **Profiles**: the `ProviderProtocol` and `ReportDestination` implementations for the services you use | Provider and destination specifics are never in this repository (DESIGN section 3.6). |
| **Identity**: a stable id per human for the two-person checks | The crate compares opaque ids; it cannot see that one person has two accounts (DESIGN section 11). |

## Call order for one upload

1. **Hash.** `hash::pdq_dihedral(&frame)` returns the eight orientation hashes and a quality byte.
   Or go through the screening entry point below, which refuses an unhashable frame for you.
2. **Build the request.** `screening::match_request(&frame, &policy)` returns a `MatchRequest`
   or refuses a frame it cannot hash (that is "cannot scan", never "clean").
3. **Ask the provider.** `provider.request(&req)` gives you the bytes to send; you send them;
   `provider.interpret(&req, &response)` gives a `MatchOutcome`.
4. **Decide.** `screening::screen(&frame, outcome, None, &policy)` returns `Screening::Allow` (carrying
   the screened frame a model may receive) or `Screening::Hold(..)`.
5. **If matched**, open an evidence case (`evidence::CaseRecord`) and apply file events as your
   storage reports them (`apply_file_event`). Two distinct people are required to `clear`.
6. **If reporting**, run the step machine: `report::next_step(&state, now)`, build and send the
   destination request, `dest.interpret(..)`, then `report::apply(&state, &step, &answer, now)`;
   persist the new state; repeat until `Status::Finished`.
7. **Retention.** `evidence::preserve_until(..)` is a pure function of the recorded events and
   never moves earlier. Keep what it says to keep; the crate cannot delete it.

## Failing closed (your side of the contract)

- `MatchOutcome` is ordered `Clean < Unavailable < Match`. **Never treat `Unavailable` as clean.**
  A scan that could not complete holds the image; it does not publish it.
- A `Clean` answer means "no known match", not "acceptable". It is not clearance.
- Safety-relevant enums are exhaustive on purpose: match them with no wildcard arm and let the
  compiler tell you when a variant is added. Error and informational enums are `#[non_exhaustive]`:
  keep a fallback arm, and make it fail closed.
- Match data goes to the evidence and report layers only. Do not feed it to a model, a label or a prompt;
  `classifier` accepts a frame and nothing else, by construction.

## What to persist

| Record | Where it comes from | Note |
|---|---|---|
| The case (`CaseRecord`) and each file's `EvidenceState` | layer 3 | Write the state returned by each transition in the same transaction as the side effect it describes. |
| The report state (`ReportState`) and the `Event` returned by `apply` | layer 4 | Every send counts toward the preservation clock (DESIGN section 3.3). |
| Which orientation hashes you computed | layer 1 | Store the crate version beside them: a crate upgrade can change hashes. |

## Logging

Redaction covers this crate's own `Debug` output only (DESIGN section 11). Log slot keys and status
codes, never credential values, never frames, never `Person` fields.

## From TypeScript (WebAssembly)

`wasm/` exports layers 1 to 4 (ABI version 4; layer 5 stays Rust only) over raw linear memory with no
imports, instantiated synchronously from a compiled module. The encodings and error codes are in
`docs/WASM-BOUNDARY.md`, checked by `wasm/test.mjs` and replayed from `wasm/vectors/`. CI packs the module
with a typed wrapper as an npm package (`docs/NPM-PACKAGE.md`; `wasm/npm/README.md` shows the calls), so a
TypeScript host calls methods and does not write pointer glue. A TypeScript host should not port the layers
by hand.
