# PDQ: upstream vetting note (S2-1, step 0)

**Status:** vetting only. No code has been ported and no upstream file is in this repository. This note is
the record that `docs/PLAN-step2.md` S2-1 requires before any port starts. Read on 2026-10-10.

## 1. What was pinned

| | |
|---|---|
| Upstream | `github.com/facebook/ThreatExchange`, directory `pdq/` |
| Pinned commit | `07b82cb6e87b7e0ac7fc2a01d865df5db10ee1f2` ("[vpdq] [pdq] Misc cleanup (#1924)", committed 2026-02-03T17:42:09Z) |
| Why this commit | the newest commit that touched `pdq/` when read; a port is checked against exactly this tree |
| How it was read | the GitHub contents and trees API at that SHA, content decoded and compared; blob SHAs below are the API's |

Re-pin only by repeating this note's steps and recording the new SHA beside the old one.

## 2. Licence that governs the code

- The repository's root `LICENSE` (blob `9406c626400cf7167750c03101f628a31bfe478d`; the API content,
  hashed locally with `git hash-object`, gives the same blob SHA) is the BSD 3-clause licence "For ThreatExchange
  software", `Copyright (c) Meta Platforms, Inc. and affiliates.`:
  - source redistributions retain the copyright line, the conditions and the disclaimer;
  - binary redistributions reproduce them in the documentation or other materials;
  - the names Facebook and its contributors may not be used to endorse or promote derived products.
- `pdq/` has no licence file of its own. The only other licence files in the repository are under
  `api-reference-examples/java/te-tag-query`, `tmk` and `vpdq`; none is in scope.
- The GitHub API reports the repository licence as `NOASSERTION` (it does not recognise the heading
  "BSD License" as an SPDX identifier). The text is a BSD-3-Clause text; `SPDX-License-Identifier:
  BSD-3-Clause` is used on a derived file **only if** the verbatim text matches the BSD-3-Clause template word
  for word, otherwise the header says "see LICENSE in this directory" (as `DESIGN.md` and `PLAN-step2.md`
  require).

## 3. Files in the proposed scope, with their own headers

Each header was read at the pinned SHA.

| File (under `pdq/cpp/`) | Blob SHA | Header |
|---|---|---|
| `hashing/pdqhashing.cpp` | `82c5e7132c51ae6c7bb73ac645b3903f88b155e1` | Copyright (c) Meta Platforms, Inc. and affiliates. |
| `hashing/pdqhashing.h` | `edb64172f6038ed8b166288aaca450d16ee3e70a` | same |
| `hashing/torben.cpp` | `ae1d059596b9b6f367f647fe1899ee8bd9ae1142` | **"The following code is public domain. Algorithm by Torben Mogensen, implementation by N. Devillard. This code in public domain."** No Meta line |
| `hashing/torben.h` | `acb06097f2aa48259b8f7fc86043ee034c1c59a3` | same public-domain statement |
| `downscaling/downscaling.cpp` | `03a62e3bbc8a1b4d53bfa0a9616e4a117c11a7ab` | Copyright (c) Meta Platforms, Inc. and affiliates. |
| `downscaling/downscaling.h` | `e3eb14b3322171c6080b753a4daab6858ec34715` | same |
| `common/pdqbasetypes.h` | `c6d3a3e685cc2333503ee3ebeeee159bc6b6bc91` | same |
| `common/pdqhashtypes.cpp` | `7e6114c709f78f14829645edee429e8495483ed4` | same |
| `common/pdqhashtypes.h` | `02051b96eb2d5f38be282a1d97c5acb1a9d48cae` | same |
| `common/pdqhamming.h` | `9d91670dce0a369e196b8bf67e6fa4a6ea10e8ff` | same |

Not in scope, and why:
- `common/pdqhamming.cpp` (blob `a59911fdb396976edee70b669e558e246a208a7e`, about 900 kB): a lookup-table
  implementation of Hamming distance. Rust's `u64::count_ones` on four words does the same arithmetic, so
  nothing is ported from it. The reference's table and its regression constants are used only as a *check*
  that the two agree.
- `common/pdqutils.*`: timing and random helpers for the reference's benchmarks and tests.
- `io/` (image file loading through a C++ imaging library): replaced by the host's decoder; the crate takes a
  decoded frame.
- `hashing/README.md`: a benchmark note.
- `pdq/data/`: see section 5.

## 4. Findings that change the design documents

1. **The luma step has two definitions at this SHA, not one.** `DESIGN.md` and the consumer's upload-scan
   design say the RGB-to-luminance step is defined in `downscaling/downscaling.cpp` (about line 82) and only
   called elsewhere. At `07b82cb` there is also a definition of `fillFloatLumaFromRGB` in
   `hashing/pdqhashing.cpp` (about line 71, namespace `facebook::pdq::hashing`, non-const pointer
   parameters). Both use the coefficients 0.299, 0.587, 0.114 (`float`) and the same loop. The port reads both
   and must show they produce identical output on the same input before choosing one; this is a fixture, not
   an assumption.
2. **`README.md` figures** match the design: distance threshold at most 31 and quality threshold at most 49
   (`pdq/README.md` lines 44 and 45). It also states that PDQ "does not guarantee exact rotational
   invariance" and warns against picking a "minimal" dihedral hash, which is why `DESIGN.md` sends all eight.
3. **The public-domain median routine carries no Meta line.** Its port keeps the public-domain statement and
   credits Mogensen and Devillard; it does not take Meta's copyright line.

## 5. Test data

- `pdq/data/` holds JPEG, PNG, WebP and TIFF images (a bridge photograph in several modifications, mode
  conversions generated with an imaging library from `original.jpg`, regression inputs). **No licence or
  origin statement covers them.** The root licence is "for ThreatExchange software"; it does not state that it
  extends to these images, and the source of `original.jpg` is not recorded.
- Decision: **no upstream image is copied into this repository or used as a fixture.** Tests use generated
  images (deterministic patterns made in the test code) and the reference implementation's **outputs** for
  them, produced by building the reference at the pinned SHA on a developer machine and recorded as data
  with the command that produced them.

## 6. Method for the port (restated, so the PR can be checked against it)

1. Port by hand, function for function, in the reference's order, keeping its names in comments.
2. Each derived file's header names its upstream file, the pinned SHA and Meta's copyright line, or the
   public-domain statement for the median routine.
3. The licence text goes in `LICENSE` beside the derived files, verbatim, with the root blob SHA above.
4. `NOTICE` is updated in the same PR as the first derived file.
5. No third-party Rust or JavaScript PDQ package is used. If one is ever proposed, it is traced to its
   origin first.
6. Equality is tested against reference outputs at this pinned SHA only; a different upstream commit is a new
   pin, not a silent update.

## 7. What this note does not establish

- That any behaviour of the reference is correct for the purposes the consumer has. It records what the
  reference is and who owns it.
- That the reference at this SHA matches the SHA the consumer's earlier documents read (they read `main` on
  2026-10-07 and did not record a commit). Section 4 item 1 is a place they differ.
- Anything about the legal effect of the licence beyond its words. No attorney has read it.
