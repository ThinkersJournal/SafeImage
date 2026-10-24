// SPDX-License-Identifier: MIT OR Apache-2.0
use alloc::string::String;
use alloc::vec::Vec;

/// Most [`MatchDetail`] entries a [`KnownMatch`] keeps; the rest are dropped by [`KnownMatch::new`].
pub const MAX_DETAILS: usize = 64;
/// Longest string kept in any detail field (bytes); longer values are truncated at a character
/// boundary by [`MatchDetail::new`].
pub const MAX_DETAIL_FIELD_BYTES: usize = 256;

/// What the provider says a matched item is. Declared weakest first: the derived order is the
/// order [`strongest`] uses to break ties between two matches.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Classification {
    /// A value this crate does not know. Kept verbatim so a new provider vocabulary is held,
    /// never dropped; the host decides what it means.
    Other(String),
    /// Harmful or abusive material that is not necessarily the most serious category.
    HarmfulAbusive,
    /// The most serious category.
    Csam,
}

/// How close the match was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MatchType {
    /// A near match (the provider's own threshold; no distance is exposed).
    Near,
    /// An exact match.
    Exact,
}

/// A digest of the matched known item, as the provider reports it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest {
    /// Algorithm label as the provider states it (for example `sha256`).
    pub algorithm: String,
    /// The value, as text.
    pub value: String,
}

/// Provider-supplied detail about one matched known item, kept so a case file can preserve it.
/// Opaque to the crate; every field is length capped.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct MatchDetail {
    /// Digests of the known item.
    pub digests: Vec<Digest>,
    /// When the known item was listed, as the provider states it.
    pub listed_at: Option<String>,
}

fn cap(mut s: String) -> String {
    if s.len() > MAX_DETAIL_FIELD_BYTES {
        let mut end = MAX_DETAIL_FIELD_BYTES;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
    s
}

impl MatchDetail {
    /// Builds a detail with every string capped at [`MAX_DETAIL_FIELD_BYTES`] and at most
    /// [`MAX_DETAILS`] digests.
    pub fn new(digests: Vec<Digest>, listed_at: Option<String>) -> Self {
        MatchDetail {
            digests: digests
                .into_iter()
                .take(MAX_DETAILS)
                .map(|d| Digest {
                    algorithm: cap(d.algorithm),
                    value: cap(d.value),
                })
                .collect(),
            listed_at: listed_at.map(cap),
        }
    }
}

/// A known-item match. Whether it becomes a case of one kind or another is the host's decision
/// (a `KindPolicy` function); this type exposes the two facts the provider gives and nothing more.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KnownMatch {
    /// What the provider says it is.
    pub classification: Classification,
    /// Exact or near.
    pub match_type: MatchType,
    /// Provider detail, capped at [`MAX_DETAILS`] entries.
    pub details: Vec<MatchDetail>,
}

impl KnownMatch {
    /// Builds a match, dropping detail entries past [`MAX_DETAILS`].
    pub fn new(
        classification: Classification,
        match_type: MatchType,
        mut details: Vec<MatchDetail>,
    ) -> Self {
        details.truncate(MAX_DETAILS);
        KnownMatch {
            classification,
            match_type,
            details,
        }
    }
}

/// Why no answer could be obtained. A closed set: every reason maps to an alarm class in the
/// host, and none of them is ever "clean".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unavailable {
    /// The provider is not configured (no credentials, no profile, empty script).
    NotConfigured,
    /// Scanning is switched off by the host.
    Disabled,
    /// The request timed out.
    Timeout,
    /// The provider answered with a server error.
    ServerError,
    /// The provider throttled the request; respect it, do not retry in a loop.
    Throttled,
    /// The provider refused the credentials (bad, suspended or terminated). Raise at once.
    CredentialsRefused,
    /// The answer was over the byte cap, did not parse to a documented shape, or was
    /// self-contradictory (for example "no match" carrying a match type).
    MalformedResponse,
    /// The provider's test value arrived in production: a misconfiguration.
    TestValueInProduction,
}

/// What a "no known match" answer was based on. **"No known match" is not clearance**: the duty
/// to keep the material off the service stays with the operator, and nothing in this crate
/// treats `Clean` as a decision.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CleanBasis {
    /// The provider's list version at the time, if it reports one.
    pub list_version: Option<String>,
    /// Seconds since the Unix epoch when the answer was obtained, as the host's clock says
    /// (the crate has no clock).
    pub checked_at: Option<u64>,
}

/// The answer to one lookup.
///
/// Declared weakest first so the derived order is the strength order of the design's invariant
/// I2: `Clean` < `Unavailable` < `Match`. A failure is **never** weaker than a clean answer:
/// an unavailable scanner must hold the image, not publish it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MatchOutcome {
    /// No known match. Not clearance (see [`CleanBasis`]).
    Clean(CleanBasis),
    /// No answer could be obtained. Fail closed.
    Unavailable(Unavailable),
    /// A known match.
    Match(KnownMatch),
}

impl MatchOutcome {
    /// True only for [`MatchOutcome::Clean`]. Provided so a caller's "may publish" test cannot
    /// accidentally be written as `!is_match()`.
    pub fn is_clean(&self) -> bool {
        matches!(self, MatchOutcome::Clean(_))
    }
}

/// The strongest of several answers for the same image (any rotation variant, any provider).
/// A match outranks every other answer; an unavailable answer outranks a clean one; ties between
/// matches break by classification, then exact over near, then details, so the result does not
/// depend on the order of `outcomes`. An empty input is `Unavailable(NotConfigured)`.
pub fn strongest<I: IntoIterator<Item = MatchOutcome>>(outcomes: I) -> MatchOutcome {
    outcomes
        .into_iter()
        .max()
        .unwrap_or(MatchOutcome::Unavailable(Unavailable::NotConfigured))
}
