# SafeImage

Known-image matching, evidence handling and report building for platforms that
accept image uploads.

**Status: pre-1.0, not yet on crates.io.** All five layers exist and are tested;
when to publish is the maintainer's decision, not this repository's.
The boundary and the reasoning are in [`docs/DESIGN.md`](docs/DESIGN.md).

## What it is for

A platform that stores user images owes its users, and the law, a way to keep
child sexual abuse material (CSAM) off the service and to report it when found.
That work is the same across platforms and easy to get subtly wrong, so it
belongs in one reviewed crate rather than being rebuilt inside each application:

1. **Hashing** (`hash`): perceptual hashing (PDQ) of a decoded frame, one hash
   or all eight orientations. Pure, `no_std` plus `alloc`, bit-exact against
   the reference implementation on its fixtures.
2. **Matching** (`matcher`): a matcher interface over known-hash lists and
   hash-match providers, with credentials supplied by the operator. The answer
   is ordered `Clean < Unavailable < Match`, so a failure can never read as clean.
3. **Evidence and review** (`evidence`): evidence states, the preservation
   clock and the two-person rules, as plain records and pure functions. No
   storage is assumed and nothing can delete.
4. **Reports** (`report`): the generic mechanism for filing reports to the
   destinations an operator is registered with, using credentials the operator
   holds with each. Destination specifics are profiles kept outside this
   repository.
5. **Classifier seam** (`classifier`, `screening`): a separate path for
   model-based screening, never mixed with known-hash matching, and the
   pre-model screen that decides whether an image may reach a model.

See it all together: `cargo run --example end_to_end`.

## What it is not

- Not a hash list. No list, token or key is, or will ever be, in this repository.
- Not legal advice. The design records the readings the project has taken and
  marks each one; the operator owns the legal position.
- Not an image decoder. It takes a decoded frame with explicit dimensions.
- Not tied to any application, database, object store, runtime or network:
  the crate is sans-I/O, and the host performs every exchange.
- Not a provider or destination client. Those are *profiles*, kept outside
  this repository; an unconfigured build reports `NoDestinationsConfigured`
  and cannot match or file anything.
- Not a statement that an image is acceptable. "No known match" is not
  clearance.

## Stability

- **Pre-1.0: a breaking change raises the second number** (0.3.x to 0.4.0),
  and every pushed change carries a new version.
- **Safety-relevant enums are exhaustive on purpose** (`MatchOutcome`,
  `Unavailable`, `DestinationAnswer`, `Status`, `Step`, `EvidenceState`,
  `Screening`, `ScreenHold` and the like). Your `match` is compiler-checked,
  and adding a variant is a breaking change.
- **Error and informational enums are `#[non_exhaustive]`**: keep a fallback
  arm, which should fail closed.
- **MSRV** is the `rust-version` in `Cargo.toml` and is raised when a
  dependency needs it; that is noted in the changelog.
- **Features:** none are needed. `reference-support` exists only for this
  repository's own fixture tests and is not API.
- **WebAssembly:** `wasm/` builds a raw module with no imports for hosts that
  cannot run Rust. It is a CI artifact, not a published crate.
- **Read [Known limits](docs/DESIGN.md#11-known-limits-read-before-relying-on-this-crate)
  before relying on redaction or on the two-person checks.**

## How it is checked

Every behaviour has a test and a hand-written mutation that the tests must
kill (`tests/reference/mutants.py`). The hashing is compared bit for bit with
the unmodified upstream reference, which CI rebuilds from the pinned upstream
files on its own runner. CI also builds for `wasm32`, runs the example, scans
the tree for credentials and for any provider or destination name, and runs
`cargo deny`.

## Licence

Code: **MIT OR Apache-2.0**, at your option ([`LICENSE-MIT`](LICENSE-MIT),
[`LICENSE-APACHE`](LICENSE-APACHE)). The files under `src/hash/` derived from
Meta's PDQ reference keep its BSD-style licence
([`src/hash/LICENSE`](src/hash/LICENSE)), so the manifest declares
`(MIT OR Apache-2.0) AND BSD-3-Clause`. Third-party credit and licence notices
are in [`NOTICE`](NOTICE).
