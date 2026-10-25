# Patent sweep: SafeImage 0.3.35 @ 351273b

Date: 2026-10-25; Run by: SafeImage lane, Claude Sonnet 5.5; Method: AI-assisted web search and read of
Google Patents pages; **NOT legal advice; NOT a freedom-to-operate opinion.** It lowers the chance of shipping
something that plainly overlaps an existing patent. It does not clear the project. Covers the `0.3` version
line (crate `safeimage`, WebAssembly module `safeimage-wasm`) as of the commit above.

## Ideas swept

Mechanisms, not features, as claim-like sentences (see `docs/DESIGN.md` for each).

1. A method that computes a 256-bit perceptual hash of a decoded frame (DCT of a downscaled luminance image,
   thresholded to bits), with a quality score, and compares two hashes by Hamming distance against a configurable
   match threshold. (PDQ, taken from the published upstream reference; layer 1.)
2. A method that derives the hashes of all eight orientations (four rotations, each with and without a flip)
   of a frame from one transform, so one lookup covers rotated and mirrored copies. (Layer 1.)
3. A method that orders match answers `Match > Unavailable > Clean` and combines the answers for many
   orientations or providers with a single `strongest` function, so a later weaker answer never replaces a match.
   (Layer 2.)
4. A fail-closed rule: no function converts "scanner unavailable" into "clean", and a response that does not parse
   to a documented shape, or that says "no match" while carrying a match type, is "unavailable". (Layer 2.)
5. A sans-I/O provider protocol: pure `request` and `interpret` functions around a host-performed HTTP exchange,
   with a byte cap before parsing and a pure retry decision. (Layers 2 and 4.)
6. A state machine for evidence files (total transition table, state of a case derived from its files, never
   stored, worst file state wins). (Layer 3.)
7. A preservation end date computed as a pure, monotonic function of recorded events (one year after the latest
   match, send or finish). (Layer 3.)
8. A two-person rule enforced by a constructor that refuses equal actor identities, required by the type system
   for clearing a case, lifting a legal hold, declaring "file without evidence" and destruction. (Layer 3.)
9. A hold check that refuses release of an evidence key before its preservation date even with two people, while
   allowing release of a serving key on a cleared case. (Layer 3.)
10. A destruction record that makes a case refuse every later write while the crate itself has no delete capability.
    (Layer 3.)
11. A report state machine (`next_step`, `apply`) over a generic step vocabulary with a closed answer enum in which
    an unknown answer is transient, never success, and "already finished" is success. (Layer 4.)
12. Separate types for known-hash signals and classifier signals with no conversion between them, and a screening
    decision that blocks a frame from reaching a model when it matched, holds on any failure, and admits a frame
    only by issuing an unforgeable `ScreenedFrame` token. (Layer 5.)
13. Credentials as opaque wrappers with redacting `Debug`/`Display`, no `Serialize` and no environment reads.
    (Cross-cutting.)
14. A raw WebAssembly module with no imports and a length-prefixed byte ABI over layers 1 to 4. (`wasm/`.)

## Queries (source, query, date)

All 2026-10-25. Source is Google Patents (through a web search restricted to `patents.google.com`) unless stated.
Hit pages were then fetched from Google Patents and the claims read from the page text.

| # | Idea | Query |
|---|---|---|
| Q1 | 1 | `patent perceptual image hash discrete cosine transform 256-bit hash hamming distance matching known images Facebook` |
| Q2 | 1 | web: `Meta Facebook PDQ perceptual hash ThreatExchange patent grant license` |
| Q3 | 1, 3 | `patent robust image hashing detecting known child sexual abuse images hash matching upload scanning` |
| Q4 | 1 | `patent robust hash image "child exploitation" OR "illegal images" hash database matching upload provider Microsoft Dartmouth Farid` |
| Q5 | 2 | `patent image hash comparing 8 orientations dihedral rotations flips single DCT perceptual hash matching` |
| Q6 | 4 | `patent fail-closed content scanning unavailable scanner treat as match hold upload quarantine until scan result` |
| Q7 | 6 to 10 | `patent two-person approval rule evidence preservation legal hold state machine content moderation reporting` |
| Q8 | 8 | `patent dual authorization "two different administrators" approve deletion of record separation of duties four-eyes data retention` |
| Q9 | licence | fetch of the upstream ThreatExchange `LICENSE` on its default branch, read for patent language |

Not searched at all: USPTO full text, Espacenet, WIPO PATENTSCOPE directly; non-US filings except where Google
Patents lists a family; queries for ideas 5, 11, 12, 13 and 14 (see Limitations).

## Hits

"Independent claims read" means the full text of the independent claims was read on the Google Patents page,
unless the row says otherwise. Status is as Google Patents prints it, which it says is an assumption.

| # | patent | title | assignee | status | claim(s) read | overlap | our difference | verdict |
|---|---|---|---|---|---|---|---|---|
| 1 | US10062083B2 | Method and system for clustering and classifying online visual information | Curalate Inc | active, adjusted expiry 2035-08-31 | 1, 7, 13, 17 (13 and 17 partly) | a DCT-style fingerprint of an image turned into a number and compared to others to find perceptually similar images | every independent claim also requires clustering similar images and assigning a canonical identifier stored in a database (13 and 17 add brand-image social-media ingest). SafeImage never clusters, assigns no cluster identifier and stores nothing | clear |
| 2 | US9628805B2 | Tunable multi-part perceptual image hashing | Avast Software s.r.o., now Gen Digital Inc. | active, adjusted expiry 2035-06-12 | 1, 7, 13 | DCT of image data, features from coefficients, encoded as a hash string, a distance between two hashes | each independent claim requires the image to be an icon, four named features per coefficient (sign, magnitude, neighbour variance, differential to a reference average), a weighted distance, and a malware-suspicion use. PDQ keeps one sign bit per coefficient against the median and compares by plain Hamming distance; no malware use | clear |
| 3 | US7421128B2 | System and method for hashing digital images | Microsoft Corp | expired 2021-12-08 | 1, 5, 6, 7, 8, 11, 12 (summarised from the page, not read claim by claim) | single hash value of an image, similar images give the same value | expired | clear (expired) |
| 4 | US7925044B2 (published as US20080008348A1) | Detecting online abuse in images | MarkMonitor Inc, now Opsec Online Ltd | active, expiry 2029-07-19 | 1 of the published application and the first lines of 1 of the grant | fingerprint of an image compared with a template database | claims require finding a "blob" in the suspect image, a fingerprint signal set per blob, candidate templates picked by blob characteristics, and several similarity measures. SafeImage hashes the whole frame and has no blob step | clear |
| 5 | US8281087B2 | Method, system, and program implementing retention policies to archive records | Google LLC (page lists it so) | expired 2024-10-03 | 1, 8, 14 | refuse a removal request if the record has a hold state or an unexpired retention period; resembles `check_release_hold` | expired | clear (expired) |
| 6 | US8396838B2 | Legal compliance, electronic discovery and electronic document handling | Commvault Systems Inc | expired (fees) | 1, 4, 10 (first lines) | hold placed on documents by an undelete flag that blocks deletion | expired; and the claims require data-management servers, tagging by criteria, custodians | clear (expired) |
| 7 | US8417776B2 | Online evidence collection | Vere Software Inc | expired (fees) | 1, 12, 17 (first lines) | preserving evidence of an investigation | claims need an external time stamp and investigator activity capture; expired | clear (expired) |
| 8 | US9231955B1 | Multiparty authorization for controlling resource access | EMC Corp | active | 1 in full, 2 to 4 first lines | an operation on a resource needs the authorisation of several parties | claim 1 requires access-control metadata marking the resource as multiparty-controlled, a server-side authorisation module, and a cache of valid tokens, with a fall-back to obtaining authorisation. `TwoPeople::new` is a constructor that rejects equal actor ids; the crate has no resource metadata, token or cache and performs no authorisation call | clear, on the claim 1 elements only; the other independent claims were not read |
| 9 | US12689630B2 | Method and system for supporting dual party control authorization for sensitive operations | Dell Products L.P. | active, expiry 2044-11-20 | 1, first line of 2 | operation needing approval from a second person | claim 1 requires assigning a security-officer role by a data-protection module, a retention-lock mode, and an operation of synchronising a network time protocol | clear, on claim 1 only |
| 10 | US11361101B2 | Multi-party authentication and authorization | Airwatch LLC | active | 1 | several people needed | claim 1 requires a wrong credential from a first user and a minimum number of second users within a proximity of the device | clear, on claim 1 only |
| 11 | US7080408B1 | Delayed-delivery quarantining of network communications having suspicious contents | McAfee LLC | expired 2024-03-19 | 1 | content held until scanned | expired | clear (expired) |
| 12 | US12417413B2 (published as US20230401492A1) | Content moderation | not confirmed (the page lists several parties) | granted, active | 1 | moderation result from machine-learning models, content held for human review | claim 1 combines several models using a combination logic chosen from a behaviour profile. SafeImage's classifier seam combines nothing and ships no model | clear, on claim 1 only |
| 13 | US10997260B2 | Extensible moderation framework | Salesforce, Inc. | active | 1, 15, 16 (first lines) | moderation rules for posted content | rule templates over user and keyword criteria lists; SafeImage has neither | clear |

No hit was found for ideas 2, 4, 6 to 10 and 11 that has live claims on them; the rows above are the closest
found, not a proof that none exists.

## Needs review (to the PM)

No hit has the verdict `needs-review`. One open item is not a hit, and it is the PM's to place on the decision board:

- **The hash itself (idea 1).** SafeImage implements PDQ from the upstream reference. The upstream licence file
  read for this sweep (Q9) is a BSD-style licence with no patent grant and no `PATENTS` file. No patent assigned
  to the upstream publisher that covers the hash was found by Q1, Q2, Q3 or Q5, but those searches were
  general web searches, not an assignee-and-inventor query on a patent database. A patent on the perceptual hashes
  used for this kind of matching by other large companies (a different algorithm from PDQ) was likewise not found
  by Q4. Absence in these searches is weak evidence. A search by assignee and inventor in USPTO and Espacenet is the
  next step, and so is a lawyer's freedom-to-operate review before any wider release.

## Limitations

- AI-assisted: the search tool is a general web search, narrowed to Google Patents pages, which returns about ten
  results per query and sometimes returns the wrong document for a number (one search returned a nuclear boration
  patent for a retention-policy number; the retention patent was re-fetched under its correct number, US8281087B2).
  Every number in the table was fetched and its claims read from the page text.
- Where the table says "on claim 1 only" or "first lines", dependent claims and other independent claims were not
  read in full. A narrower claim that survives in a dependent claim cannot be ruled out by that.
- No positive control for the absences beyond this: Q1 finds the Curalate and Avast hash patents, so the
  query shape does reach patents on DCT image hashes. Nothing similar was run for ideas 5, 11, 12, 13 and 14, which are
  ordinary software engineering patterns (sans-I/O, retry, state machines, opaque secret types, a WebAssembly ABI);
  they were not searched and carry **no verdict**.
- Google Patents prints status and expiry as assumptions. A patent shown as expired for unpaid fees may be revived.
- Only English-language results and patents Google Patents indexes. Pending applications are not covered unless
  they came up.
- Patent claims are read as ordinary English by a model. Claim construction is a legal task.
- Re-sweep when the project adds a new family of capability (for example `ListMatcher`, ROADMAP A18, or
  `normalise_geometry`, A15), and at least yearly.
