# WebAssembly boundary for layers 2 to 4: design note (PROPOSAL)

**Status:** encoding ruled by the PM on 2026-10-14 (section 3). Layer 2 exports implemented in 0.3.13 (`wasm/src/codec.rs`, `wasm/vectors/layer2.txt`), layer 4 in 0.3.14 (`wasm/src/report_codec.rs`, `wasm/vectors/layer4.txt`), layer 3 in 0.3.15 (`wasm/src/evidence.rs`, `wasm/src/evidence_codec.rs`, `wasm/vectors/layer3.txt`). The implementation half of A2 is complete; layer 5 (`match_request`, `screen`) is not in this table and stays Rust only. Written against `origin/main`
0.3.6. It closes roadmap item A2's design half; the implementation is a separate PR after this is agreed.

## 1. The problem

`wasm/` exports layer 1 only (`si_pdq`, `si_pdq_dihedral`): fixed-size byte buffers in, fixed-size
byte buffers out. Layers 2 to 4 are Rust types with strings, vectors and enums. Community's adoption
steps 3 to 5 cannot run in a Worker until their *pure, profile-independent* parts cross the boundary.
It must meet the acceptance criteria already adopted in `docs/DESIGN.md` section 5: instantiate from an
imported module, synchronous, no imports in the glue, bytes in and bytes out, typed errors and never a
panic across the boundary, built from `no_std + alloc` layers.

## 2. What crosses, and what does not

The traits `ProviderProtocol` and `ReportDestination` are implemented by **profiles**, and profiles are
written by the operator in whatever language the host runs (TypeScript, for Community). They do not
cross. What crosses is everything a profile calls into, which is profile-independent and pure:

| Layer | Crosses (candidate exports) | Stays in the host or profile |
|---|---|---|
| 2 | `strongest`, `classify_status`, `within_cap`, `RetryPolicy::decide`, `MatchRequest` construction (1 to 8 hashes) | `request` / `interpret` of a provider |
| 3 | `transition`, `case_evidence`, `report_file_disposition`, `report_may_submit`, `preserve_until`, `apply_file_event`, `confirm`, `clear`, hold and destruction functions | storage, transactions, the identity behind an actor id |
| 4 | `next_step`, `apply`, `backoff_secs`, `sync_credentials`, `request_retract`, `retry_failed`, `deletion_window_elapsed` | `build`, `request`, `interpret` of a destination; the transport |
| 5 | `match_request`, `screen` (needs a frame and a policy) | the model |

Two things follow. A TypeScript profile gets the same state machine the Rust host gets, which is the
point of the split. And `Secret` and `Credentials` never cross: the boundary takes a boolean
"credentials ready", so a credential never enters WASM memory (`setup_state` takes credentials and so stays host-side).

## 3. Encoding

The state is structured (`ReportState`, `CaseRecord`, vectors of files and clocks), so fixed offsets as
in layer 1 do not scale. Options:

| | A. Hand-rolled versioned binary (tag byte per enum, length-prefixed vectors, little-endian ints) | B. JSON via `serde` + `serde-json-core` | C. A schema tool (CBOR, protobuf) |
|---|---|---|---|
| New dependencies | none | two, both new to `deny.toml` review | one or more |
| `no_std + alloc` | yes | yes | varies |
| Module size | smallest (layer 1 is 25.9 KiB today) | grows by the parser and derive output | grows |
| Host-side decoder | written once per language, small | built in (`JSON.parse`) | a library |
| Drift risk (Rust and TS disagree on a tag) | real; mitigated by shared vectors (A3) | schema-level, easier to see | lowest |
| Exhaustive enums stay exhaustive | by hand: an unknown tag decodes to an error | derive handles it | derive handles it |

**Ruled (PM, 2026-10-14): A**, a hand-rolled versioned binary encoding with a version byte and an
`ERR_UNKNOWN_TAG` that fails closed. Reasons: it keeps the crate dependency-free (`libm` only), which the
whole supply-chain position rests on, and the conformance vectors (item A3) make drift a test failure
rather than a production surprise. B stays the recorded fallback; adopting it would be a new PM ruling.

## 4. Rules the implementation must follow

1. **Never panic.** Every export returns a negative `i32` for a refusal; a bad length, an unknown tag
   or a truncated buffer is an error code, never an `unwrap`. A fuzz-style test feeds truncations of
   every valid vector.
2. **Fail closed on the decode side too.** An unknown outcome tag decodes to `Unavailable`, never to
   `Clean`; an unknown report answer decodes to `DestinationAnswer::Unknown`. Tested per enum.
3. **Time is an argument** (`u64` seconds), as in the Rust API.
4. **ABI version.** `si_abi_version` returns 2 once any of these lands; layer 1's layout is unchanged.
5. **No allocation visible to the host beyond `si_alloc` / `si_free`**, as today, and a documented
   upper bound on input size so the isolate memory limit (128 MB) cannot be reached by a hostile
   buffer (the layer 1 cap is 16.7 million pixels; the new inputs are bytes, not frames).
6. **Same crate version, same artifact.** The module reports the crate version, so a host can pin and
   record it beside any hash it stores (DESIGN section 11).
7. **`unsafe` stays confined to `wasm/`**, as today; the core crate keeps `unsafe_code = "forbid"`.

## 5. Order of work

1. This note agreed (PM).
2. Encoding module and layer 2 exports, with vectors generated from the Rust tests.
3. Layer 4, then layer 3 (last, as Community's own order has it).
4. `wasm/test.mjs` replays every vector; Community runs its workerd gate on the new module.
5. Distribution of the built module is Q10 and is not decided here.

## 6. Not decided here

Q10 (npm package versus a vendored `.wasm`), where profiles live (Q9), and anything that names a
provider or destination. None of those changes the boundary.
