# Conformance vectors for the WebAssembly module

`layer2.txt`, `layer3.txt` and `layer4.txt` are generated from the Rust tests
(`SAFEIMAGE_WRITE_VECTORS=1 cargo test -p safeimage-wasm vectors`); a normal test run fails if a file is
stale. `wasm/test.mjs` replays all three against the built module. A TypeScript host replays them the same
way to show its binding matches, without editing its own tests.

One vector per line, fields separated by single spaces. Lines starting `#` are comments. `hex` is lowercase
hex of the exact bytes; `code` is the export's decimal return value (negative is a refusal). Integers named
`u64` are passed as `BigInt`. `ready`, `found`, `present` and similar flags are `0` or `1`.
An "optional time" is two fields: `present until`.

The encoding is in `wasm/src/codec.rs`, `report_codec.rs` and `evidence_codec.rs`; every message starts with
version byte `01`.

## layer2.txt

| line | fields | replay |
|---|---|---|
| `strongest` | name, input hex, output hex | `si_strongest`; output is the byte count written |
| `strongest_err` | name, input hex, code | `si_strongest` returns the code |
| `classify` | status, code | `si_classify_status(status)` |
| `within_cap` | len, cap, 0 or 1 | `si_within_cap` |
| `retry` | name, max_retries, delay_ms, attempts, reason tag, elapsed_ms, deadline_ms, output hex (9 bytes) | `si_retry_decide` |
| `match_request` | name, input hex, hash count or code | `si_match_request` |

## layer4.txt

`new`, `next`, `apply`, `sync`, `retract`, `retry`, `backoff`, `window`: the field order is in the file's
header comment. A state, step or answer is an encoded message; the output of `apply` is the new state then an
optional event.

## layer3.txt

| line | fields | replay |
|---|---|---|
| `transition` | name, state tag, event hex, new state tag or code | `si_evidence_transition` |
| `disposition` | name, state tag, found, code | `si_report_file_disposition` |
| `case_evidence` | name, states hex, code | `si_case_evidence` |
| `may_submit` | name, state-and-found list hex, 0 or 1 | `si_report_may_submit` |
| `period` | seconds | `si_preservation_period_secs` |
| `preserve` | name, match_at, decided, clocks hex, 9-byte output hex | `si_preserve_until` |
| `file_event`, `confirm`, `clear`, `set_hold`, `lift`, `destroy_req`, `destroy_exp` | name, inputs..., output hex or code | the matching `si_case_*` export |
| `eligible` | name, case hex, present, until, now, shared, code | `si_case_reaper_eligibility` |
| `release` | name, case hex, target, present, until, now, copies held, by hex, code | `si_check_release_hold` |

A refusal is a negative code and means **hold**: nothing in these tables is ever resolved toward "clean",
"cleared" or "deleted" on an error.
