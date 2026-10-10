# SafeImage

Known-image matching, evidence handling and report building for platforms that
accept image uploads.

**Status: 0.1.0, design only.** The crate boundary is in
[`docs/DESIGN.md`](docs/DESIGN.md). The library contains no behaviour yet, the
repository is private, and nothing is published.

## What it is for

A platform that stores user images owes its users, and the law, a way to keep
child sexual abuse material (CSAM) off the service and to report it when found.
That work is the same across platforms and easy to get subtly wrong, so it
belongs in one reviewed crate rather than being rebuilt inside each application:

1. **Hashing and normalisation**: perceptual hashing (PDQ) of a decoded frame. Pure.
2. **Matching**: a matcher interface over known-hash lists and hash-match
   providers, with credentials supplied by the operator.
3. **Evidence and review**: evidence states, preservation clock and a
   two-person-rule workflow, over pluggable storage.
4. **Reports**: building and filing NCMEC CyberTipline reports.
5. **Classifier seam**: a separate path for model-based screening, never mixed
   with known-hash matching.

## What it is not

- Not a hash list. No list, token or key is, or will ever be, in this repository.
- Not legal advice. The design records the readings the project has taken and
  marks each one; the operator owns the legal position.
- Not an image decoder. It takes a decoded frame with explicit dimensions.
- Not tied to any application, database or object store.

## Licence

Code: **MIT OR Apache-2.0**, at your option ([`LICENSE-MIT`](LICENSE-MIT),
[`LICENSE-APACHE`](LICENSE-APACHE)). Third-party credit and licence notices are
in [`NOTICE`](NOTICE).
